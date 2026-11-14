pub mod commands;
pub mod game_assets;
pub mod queries;
pub mod service_configuration;
pub mod reading;
pub mod subscriptions;
pub mod word_card;
#[cfg(test)]
use app_core::contracts::HudPreference;
use app_core::contracts::{AppError, AppResult, Command};
pub(crate) use crate::platform::session_preview_coordinator::binding_observation;
pub(crate) fn binding_context(state: &crate::bootstrap::DesktopState) -> AppResult<binding_observation::Context> {
    use app_core::ports::runtime::HookBackend;
    let snapshot = state.runtime.snapshot()?;
    let active = state.hook.active_session_context()?;
    let session_id = snapshot.session.filter(|s| matches!(s.state.as_str(), "opening"|"connected"|"active"))
        .map(|s|s.session_id);
    let active_target_identity_sha256 = active.filter(|s| Some(&s.session_id)==session_id.as_ref())
        .map(|s|s.target_identity_sha256);
    Ok(binding_observation::Context { session_id, active_target_identity_sha256, ..Default::default() })
}
pub fn command_roles(command: &Command) -> &'static [&'static str] {
    match command {
        Command::CopyModelConfiguration { .. }
        | Command::SaveConfiguration { .. }
        | Command::ApplyConfiguration { .. }
        | Command::SetConnectionSuspended { .. }
        | Command::SetPurposeRuntimeEnabled { .. }
        | Command::RevokeCredential { .. }
        | Command::SavePreferences { .. }
        | Command::SaveScopedPreferences { .. }
        | Command::SaveCredentialPool { .. } => &["settings", "main"],
        Command::SetReadingAnnotations { .. } => &["main", "hud"],
        Command::SetHudPreference { .. } | Command::CycleReadingDisplayMode => {
            &["main", "hud", "settings"]
        }
        Command::SaveGame(_)
        | Command::SaveGames { .. }
        | Command::ArchiveGame { .. }
        | Command::ManualInput { .. }
        | Command::SaveCorrection { .. } => &["main"],
        Command::SetCapturePaused { .. } => &["main", "hud", "learning", "settings"],
        Command::SetTranslationEnabled { .. } => &["main", "hud", "learning", "settings"],
        Command::SelectSource { .. }
        | Command::Disconnect
        | Command::UnbindWindow(_)
        | Command::CancelTask { .. } => &["main", "hud", "learning"],
    }
}
pub fn role(window: &tauri::WebviewWindow, allowed: &[&str]) -> AppResult<()> {
    if allowed.contains(&window.label()) {
        Ok(())
    } else {
        Err(AppError::new(
            "forbidden_window",
            "ipc",
            "此窗口无权执行该操作",
        ))
    }
}
pub async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| AppError::new("application_executor", "ipc", "应用执行器意外停止"))?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_has_narrow_current_session_capture_and_translation_controls() {
        for command in [
            Command::SetCapturePaused { paused: true },
            Command::SetCapturePaused { paused: false },
            Command::SetTranslationEnabled { enabled: true },
            Command::SetTranslationEnabled { enabled: false },
        ] {
            assert_eq!(command_roles(&command), ["main", "hud", "learning", "settings"]);
        }
        for command in [
            Command::SelectSource {
                source_id: "source".into(),
            },
            Command::Disconnect,
            Command::CancelTask { task_id: "task".into() },
        ] {
            assert!(!command_roles(&command).contains(&"settings"));
        }
    }
    #[test]
    fn batch_library_addition_is_restricted_to_main_window() {
        assert_eq!(
            command_roles(&Command::SaveGames { items: vec![] }),
            ["main"]
        );
    }
    #[test]
    fn configuration_commands_are_allowed_in_settings_but_not_reading_windows() {
        let pool=serde_json::from_value(serde_json::json!({"kind":"save_credential_pool","payload":{"pool":{"poolId":null,"expectedRevision":0,"metadata":{},"policy":"selected","members":[{"credentialId":"c1","enabled":true,"weight":1}]}}})).unwrap();
        for command in [
            Command::CopyModelConfiguration {
                model_id: "model".into(),
                new_display_name: "copy".into(),
                expected_head_revision: 0,
            },
            Command::SaveConfiguration {
                configuration: Default::default(),
                expected_head_revision: 0,
            },
            Command::ApplyConfiguration {
                revision_id: "c1".into(),
                expected_head_revision: 0,
            },
            Command::SavePreferences {
                preferences: Default::default(),
            },
            Command::SetConnectionSuspended {
                connection_id: "connection-1".into(),
                suspended: true,
                expected_control_revision: 1,
            },
            Command::SetPurposeRuntimeEnabled {
                scope_key: "global".into(),
                purpose: "grammar".into(),
                enabled: false,
                expected_authorization_epoch: 1,
            },
            Command::RevokeCredential {
                credential_id: "credential-1".into(),
                expected_lifecycle_revision: 2,
                expected_impact_hash: "0".repeat(64),
            },
            pool,
        ] {
            assert_eq!(command_roles(&command), ["settings", "main"]);
        }
        assert_eq!(
            command_roles(&Command::ManualInput {
                text: "雨。".into(),
                source_language: "ja".into()
            }),
            ["main"]
        );
        assert_eq!(
            command_roles(&Command::Disconnect),
            ["main", "hud", "learning"]
        );
        assert_eq!(
            command_roles(&Command::SetReadingAnnotations {
                show_ruby: false,
                show_pos: true
            }),
            ["main", "hud"]
        );
    }
    #[test]
    fn hud_preference_commands_have_narrow_typed_access_without_full_settings() {
        for preference in [
            HudPreference::Segmentation,
            HudPreference::Compact,
            HudPreference::AlwaysOnTop,
        ] {
            assert_eq!(
                command_roles(&Command::SetHudPreference {
                    preference,
                    enabled: false
                }),
                ["main", "hud", "settings"]
            );
        }
        assert!(
            !command_roles(&Command::SavePreferences {
                preferences: Default::default()
            })
            .contains(&"hud")
        );
        assert_eq!(
            command_roles(&Command::CycleReadingDisplayMode),
            ["main", "hud", "settings"]
        );
    }
    #[test]
    fn blocking_commands_keep_the_async_runtime_context() {
        tauri::async_runtime::block_on(async { blocking(tokio_handle_check).await.unwrap() });
    }
    fn tokio_handle_check() -> AppResult<()> {
        // Exercise the same executor used by study/retranslate. A task spawned
        // through Tauri must complete from inside its blocking executor too.
        let handle = tauri::async_runtime::spawn(async { 7 });
        assert_eq!(tauri::async_runtime::block_on(handle).unwrap(), 7);
        Ok(())
    }
}

pub mod google_authorization;
