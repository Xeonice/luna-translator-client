//! First-frame cover consumer. It cannot start, stop or resume preview capture.
use super::session_preview_coordinator::{SessionCoverConsumer, SessionCoverDependencies};
use app_core::contracts::*;

fn subscriber_update(
    result: AppResult<Option<PreviewUpdate>>,
    mut report: impl FnMut(AppError),
) -> Option<PreviewUpdate> {
    match result {
        Ok(update) => update,
        Err(error) => {
            report(error);
            None
        }
    }
}

pub(crate) struct SessionCoverCoordinator;
impl SessionCoverCoordinator {
    pub(crate) fn spawn(dependencies: SessionCoverDependencies, consumer: SessionCoverConsumer) {
        tauri::async_runtime::spawn(async move {
            let mut current_key = String::new();
            let mut cover_done = false;
            let mut cover_conflicts = 0u8;
            // At most one prepared first frame per binding. Transient proof failures
            // retry this same file, never persist a new image on every timer tick.
            let mut pending_cover: Option<(PreviewFrame, PreparedSessionCover)> = None;
            let mut last_error = String::new();
            loop {
                tokio_pause().await;
                if dependencies.is_closing() {
                    continue;
                }
                let identity = dependencies.identity().await;
                let expected = match identity {
                    Ok(Some(value)) => value,
                    Ok(None) => {
                        current_key.clear();
                        cover_done = false;
                        pending_cover = None;
                        continue;
                    }
                    Err(e) => {
                        if e.code != last_error {
                            dependencies.report_failure(e.clone());
                            last_error = e.code;
                        }
                        continue;
                    }
                };
                let key = format!(
                    "{}:{}",
                    expected.0.session_id, expected.0.binding_identity_sha256
                );
                if key != current_key {
                    current_key = key;
                    cover_done = false;
                    cover_conflicts = 0;
                    pending_cover = None;
                    last_error.clear();
                }
                let result = consumer.consumer_update(
                    &expected.0.session_id,
                    !cover_done && pending_cover.is_none(),
                );
                let Some(update) = subscriber_update(result, |e| {
                    if e.code != last_error {
                        dependencies.report_failure(e.clone());
                        last_error = e.code;
                    }
                }) else {
                    continue;
                };
                if cover_done
                    || expected.1.is_none()
                    || update.projection.state != PreviewState::Live
                {
                    continue;
                }
                if pending_cover.is_none() {
                    let Some(frame) = update.frame.filter(|f| {
                        f.lease_id == update.projection.lease_id.as_deref().unwrap_or_default()
                            && f.generation == update.projection.generation
                            && Some(f.target_identity_hash.as_str())
                                == update.projection.target_identity_hash.as_deref()
                    }) else {
                        continue;
                    };
                    let preparation = dependencies.prepare(expected.0.clone(), frame).await;
                    match preparation {
                        Ok(Some(prepared)) => pending_cover = Some(prepared),
                        Ok(None) => {
                            cover_done = true;
                            continue;
                        }
                        Err(e)
                            if matches!(
                                e.code.as_str(),
                                "cover_image_invalid" | "cover_image_bounds"
                            ) =>
                        {
                            continue;
                        }
                        Err(e) => {
                            if e.code != last_error {
                                dependencies.report_failure(e.clone());
                            }
                            last_error = e.code;
                            cover_done = true;
                            continue;
                        }
                    }
                }
                let (frame, prepared) = pending_cover.as_ref().unwrap().clone();
                let installed = dependencies
                    .verify_and_commit(consumer.clone(), expected.0.session_id, frame, prepared)
                    .await;
                match installed {
                    Ok(CoverInstallOutcome::Installed | CoverInstallOutcome::AlreadyPresent) => {
                        cover_done = true;
                        pending_cover = None;
                    }
                    Ok(CoverInstallOutcome::RevisionConflict) if cover_conflicts == 0 => {
                        cover_conflicts += 1;
                        pending_cover = None;
                    }
                    Ok(_) => {
                        cover_done = true;
                        pending_cover = None;
                    }
                    Err(e)
                        if matches!(
                            e.code.as_str(),
                            "cover_image_invalid"
                                | "cover_image_bounds"
                                | "game_cover_preview_stale"
                        ) =>
                    {
                        pending_cover = None;
                    }
                    Err(e) if e.code == "game_cover_verification" => {
                        if e.code != last_error {
                            dependencies.report_failure(e.clone());
                        }
                        last_error = e.code;
                    }
                    Err(e) => {
                        if e.code != last_error {
                            dependencies.report_failure(e.clone());
                        }
                        last_error = e.code;
                        cover_done = true;
                        pending_cover = None;
                    }
                }
            }
        });
    }
}

async fn tokio_pause() {
    // Reuse the existing runtime's bounded timer without adding a desktop dependency.
    let _ = tauri::async_runtime::spawn_blocking(|| {
        std::thread::sleep(std::time::Duration::from_millis(1000))
    })
    .await;
}

#[cfg(test)]
mod session_cover_coordinator_tests {
    use super::*;
    use crate::platform::window_preview::{PreviewPairing, WindowPreviewService};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct PairResource(Arc<AtomicUsize>);
    impl Drop for PairResource {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn service() -> (Arc<WindowPreviewService>, Arc<AtomicUsize>) {
        let drops = Arc::new(AtomicUsize::new(0));
        let resource = PairResource(drops.clone());
        let pairing = Arc::new(PreviewPairing {
            _token: "owned-test-pair".into(),
            _keepalive: Arc::new(move || {
                let _keep_owned = &resource;
                Ok(())
            }),
        });
        (
            WindowPreviewService::coordinator_test_fixture(pairing),
            drops,
        )
    }
    fn failure(code: &str) -> AppError {
        AppError::new(code, "preview", "test read failure")
    }
    #[test]
    fn actual_subscriber_errors_and_absence_never_release_the_producer() {
        let (service, drops) = service();
        let consumer = SessionCoverConsumer::for_test(service.clone());
        let before = service.coordinator_test_projection();
        let mut diagnostics = Vec::new();
        for code in [
            "capture_cleanup_pending",
            "preview_state",
            "game_window_stale",
        ] {
            assert!(subscriber_update(Err(failure(code)), |e| diagnostics.push(e.code)).is_none());
            let after = service.coordinator_test_projection();
            assert_eq!(after.lease_id, before.lease_id);
            assert_eq!(after.generation, before.generation);
            assert_eq!(drops.load(Ordering::SeqCst), 0);
        }
        assert!(
            subscriber_update(consumer.consumer_update("missing-owner", true), |_| panic!(
                "missing is not an error"
            ))
            .is_none()
        );
        assert!(
            subscriber_update(consumer.consumer_update("owner", false), |_| panic!(
                "present"
            ))
            .is_some()
        );
        assert_eq!(
            diagnostics,
            [
                "capture_cleanup_pending",
                "preview_state",
                "game_window_stale"
            ]
        );
        // Dropping a read-only subscriber is not producer shutdown either.
        let detached = consumer.clone();
        drop(detached);
        let retained = consumer.consumer_update("owner", false).unwrap().unwrap();
        assert_eq!(retained.projection.lease_id, before.lease_id);
        assert_eq!(retained.projection.generation, before.generation);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        // Explicit owner release, as distinct from a subscriber read, still frees it.
        service.stop_consumer("owner").unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn actual_poisoned_cache_read_reports_without_fabricating_missing_or_restarting() {
        let (service, drops) = service();
        let consumer = SessionCoverConsumer::for_test(service.clone());
        let before = service.coordinator_test_projection();
        service.coordinator_test_poison();
        let mut diagnostics = Vec::new();
        assert!(
            subscriber_update(consumer.consumer_update("owner", true), |e| diagnostics
                .push(e.code))
            .is_none()
        );
        assert_eq!(diagnostics, ["preview_state"]);
        let after = service.coordinator_test_projection();
        assert_eq!(after.lease_id, before.lease_id);
        assert_eq!(after.generation, before.generation);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
}
