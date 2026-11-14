use super::*;
#[tokio::test]
async fn missing_worker_is_explicit_and_never_uses_a_fallback_parser() {
    let client = LanguageClient::new(PathBuf::from("/nonexistent-language-resources"));
    assert!(!client.resource_info().available);
    assert!(client.analyze("図書館").await.is_err());
    assert!(client.analyze("a\0b").await.is_err());
    assert!(client.analyze(&"あ".repeat(MAX_TEXT_BYTES)).await.is_err());
}
#[cfg(unix)]
#[tokio::test]
async fn worker_hang_is_killed_then_real_mecab_can_restart() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = std::env::temp_dir().join(format!(
        "luna-language-supervisor-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&fixture).unwrap();
    fn links(source: &std::path::Path, destination: &std::path::Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let target = destination.join(entry.file_name());
            if path.is_dir() {
                links(&path, &target);
            } else {
                std::fs::hard_link(&path, &target).unwrap();
            }
        }
    }
    links(
        &project.join("resources/language"),
        &fixture.join("language"),
    );
    links(
        &project.join("resources/licenses"),
        &fixture.join("licenses"),
    );
    let executable = fixture.join("language-worker");
    let production_worker = project.join("target/release/language-worker");
    assert!(
        production_worker.is_file(),
        "build language-worker --release --locked before the storage/language integration suite"
    );
    std::fs::hard_link(&production_worker, &executable).unwrap();
    let client = LanguageClient::new(fixture.clone());
    let cold = std::time::Instant::now();
    let tokens = client
        .analyze("彼女は図書館にいる。")
        .await
        .expect("real initial MeCab worker");
    assert!(
        tokens
            .iter()
            .any(|token| token.surface == "彼女" && token.reading == "かのじょ")
    );
    let owned_pid = client
        .worker
        .lock()
        .await
        .as_ref()
        .unwrap()
        .child
        .id()
        .unwrap();
    // Stop only the exact test-owned native worker, after real parsing succeeded.
    // No alternative parser or successful language response is simulated.
    assert!(
        std::process::Command::new("/bin/kill")
            .args(["-STOP", &owned_pid.to_string()])
            .status()
            .unwrap()
            .success()
    );
    let started = std::time::Instant::now();
    let error = client
        .analyze("彼女は図書館にいる。")
        .await
        .expect_err("owned hung worker must time out");
    assert!(error.contains("15秒"), "{error}");
    assert!(started.elapsed() >= DEADLINE && started.elapsed() < DEADLINE + Duration::from_secs(5));
    assert!(client.worker.lock().await.is_none());
    let restarted = std::time::Instant::now();
    let tokens = client
        .analyze("彼女は図書館にいる。")
        .await
        .expect("fresh real MeCab worker after timeout");
    assert!(
        tokens
            .iter()
            .any(|token| token.surface == "彼女" && token.reading == "かのじょ")
    );
    assert_ne!(
        client.worker.lock().await.as_ref().unwrap().child.id(),
        Some(owned_pid)
    );
    validate_spans("彼女は図書館にいる。", &tokens).unwrap();
    eprintln!(
        "real language startup+hang+restart total={:?}, restart={:?}",
        cold.elapsed(),
        restarted.elapsed()
    );
    client.stop().await;
    std::fs::remove_dir_all(fixture).unwrap();
}
