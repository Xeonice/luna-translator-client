//! Real writer/SQLite/process fault tests. These are component evidence, not a
//! declaration that the complete STORAGE acceptance matrix has passed.
#[allow(dead_code)]
#[path = "../../tests/storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use super::*;
use std::{
    path::PathBuf,
    process::{Child, Command as ProcessCommand, Stdio},
    sync::Barrier,
    time::Instant,
};

fn directory(label: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.build/storage-faults")
        .join(format!(
            "{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    acceptance_evidence::directory(label, &dir)
}
fn manual(operation: &str, text: &str) -> CommandEnvelope {
    CommandEnvelope {
        operation_id: operation.into(),
        expected_revision: None,
        command: Command::ManualInput {
            text: text.into(),
            source_language: "ja".into(),
        },
    }
}
struct ChildProcess(Child);
impl Drop for ChildProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn child(mode: &str, dir: &Path) -> ChildProcess {
    let log = std::fs::File::create(dir.join("child.log")).unwrap();
    let child = ProcessCommand::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::database::fault_tests::subprocess_entry",
            "--nocapture",
        ])
        .env("LUNA_STORAGE_FAULT_MODE", mode)
        .env("LUNA_STORAGE_FAULT_DIRECTORY", dir)
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .unwrap();
    let mut child = ChildProcess(child);
    let start = Instant::now();
    while !dir.join("ready").exists() {
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!(
                "fault child stopped early ({status}): {}",
                std::fs::read_to_string(dir.join("child.log")).unwrap()
            );
        }
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "fault child did not reach its real barrier"
        );
        thread::sleep(Duration::from_millis(10));
    }
    child
}
fn ready_and_wait(dir: &Path) -> ! {
    std::fs::write(dir.join("ready"), "actual barrier reached").unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn subprocess_entry() {
    let Ok(mode) = std::env::var("LUNA_STORAGE_FAULT_MODE") else {
        return;
    };
    let dir = PathBuf::from(std::env::var_os("LUNA_STORAGE_FAULT_DIRECTORY").unwrap());
    let path = dir.join("profile.sqlite3");
    if mode == "external_reserved_lock" {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        ready_and_wait(&dir);
    }
    let store = SqliteStore::open(&path, "fault-profile", "child-instance", 100).unwrap();
    if mode == "lease" {
        store
            .dispatch(manual("child-committed", "子プロセスの原文"), 101)
            .unwrap();
        ready_and_wait(&dir);
    }
    if mode == "after_business_commit" {
        store
            .dispatch(manual("business-committed", "確定した原文🦀"), 101)
            .unwrap();
        ready_and_wait(&dir);
    }
    if mode == "before_actor_transaction_commit" {
        // This barrier exercises the production writer and real multi-table SQLite
        // transaction. It does not claim to interrupt every application use case.
        store.call(move |db| {
            let tx = db.conn.transaction().map_err(err)?;
            tx.execute("INSERT INTO games VALUES('uncommitted-game','未確定',NULL,'ja',NULL,1,101,101,NULL)", []).map_err(err)?;
            tx.execute("INSERT INTO game_targets(target_id,game_id,environment_kind,environment_key,verification_state,row_revision,updated_at_ms) VALUES('uncommitted-target','uncommitted-game','unresolved','unresolved:transaction','unresolved',1,101)", []).map_err(err)?;
            tx.execute("UPDATE profile_meta SET commit_revision=commit_revision+1 WHERE singleton=1", []).map_err(err)?;
            ready_and_wait(&dir);
            #[allow(unreachable_code)]
            { tx.commit().map_err(err) }
        }).unwrap();
    }
    panic!("unknown child fault mode: {mode}");
}

#[test]
fn writer_queue_rejects_the_129th_pending_job_and_recovers_without_losing_accepted_jobs() {
    let dir = directory("bounded-queue");
    let store = SqliteStore::open(
        dir.join("profile.sqlite3"),
        "fault-profile",
        "queue-instance",
        100,
    )
    .unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let blocker = store.clone();
    let blocked = thread::spawn(move || {
        blocker.call(move |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let barrier = Arc::new(Barrier::new(130));
    let (outcome_tx, outcome_rx) = mpsc::channel();
    let mut threads = Vec::new();
    for n in 0..129 {
        let s = store.clone();
        let b = barrier.clone();
        let tx = outcome_tx.clone();
        threads.push(thread::spawn(move || {
            b.wait();
            tx.send(s.dispatch(
                manual(&format!("bounded-{n}"), &format!("原文 {n}")),
                101 + n,
            ))
            .unwrap();
        }));
    }
    drop(outcome_tx);
    barrier.wait();
    let rejected = outcome_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(rejected.unwrap_err().code, "storage_queue_full");
    assert!(
        outcome_rx.try_recv().is_err(),
        "accepted jobs must await the blocked writer"
    );
    release_tx.send(()).unwrap();
    blocked.join().unwrap().unwrap();
    let results: Vec<_> = outcome_rx.into_iter().collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(results.len(), 128);
    assert!(results.into_iter().all(|r| r.is_ok()));
    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.commit_revision, 128);
    let first = store.history(None, None, 100).unwrap();
    let second = store.history(None, first.next_cursor, 100).unwrap();
    assert_eq!(first.units.len() + second.units.len(), 128);
    assert!(second.next_cursor.is_none());
    assert!(
        !store
            .dispatch(manual("queue-after-release", "回復"), 500)
            .unwrap()
            .replayed
    );
    acceptance_evidence::record(
        "bounded-writer-queue",
        &[("STORAGE-06", 1)],
        serde_json::json!({"actual_test":"writer_queue_rejects_the_129th_pending_job_and_recovers_without_losing_accepted_jobs"}),
    );
}

#[test]
fn actual_readonly_connection_failure_keeps_committed_state_and_allows_same_operation_retry() {
    let dir = directory("readonly");
    let store = SqliteStore::open(
        dir.join("profile.sqlite3"),
        "fault-profile",
        "readonly-instance",
        100,
    )
    .unwrap();
    store.dispatch(manual("baseline", "保存済み"), 101).unwrap();
    let before = store.snapshot().unwrap();
    store
        .call(|db| db.conn.execute_batch("PRAGMA query_only=ON").map_err(err))
        .unwrap();
    let failure = store
        .dispatch(manual("readonly-retry", "失敗してから回復"), 102)
        .unwrap_err();
    assert!(failure.message.contains("readonly"), "{failure:?}");
    assert_eq!(
        store.snapshot().unwrap().commit_revision,
        before.commit_revision
    );
    assert_eq!(store.history(None, None, 100).unwrap().units.len(), 1);
    store
        .call(|db| db.conn.execute_batch("PRAGMA query_only=OFF").map_err(err))
        .unwrap();
    let receipt = store
        .dispatch(manual("readonly-retry", "失敗してから回復"), 103)
        .unwrap();
    assert!(
        !receipt.replayed,
        "failed transactions must not persist a success receipt"
    );
    assert_eq!(receipt.revision, before.commit_revision + 1);
    acceptance_evidence::record(
        "readonly-retry",
        &[("STORAGE-07", 1)],
        serde_json::json!({"actual_test":"actual_readonly_connection_failure_keeps_committed_state_and_allows_same_operation_retry"}),
    );
}

#[test]
fn another_process_reserved_lock_times_out_then_the_real_writer_recovers() {
    let dir = directory("external-busy");
    let store = SqliteStore::open(
        dir.join("profile.sqlite3"),
        "fault-profile",
        "busy-instance",
        100,
    )
    .unwrap();
    store.dispatch(manual("baseline", "保存済み"), 101).unwrap();
    let lock = child("external_reserved_lock", &dir);
    let started = Instant::now();
    let failure = store
        .dispatch(manual("busy-retry", "回復した原文"), 102)
        .unwrap_err();
    assert_eq!(failure.code, "storage_busy");
    assert!(started.elapsed() >= Duration::from_millis(2500));
    assert!(started.elapsed() < Duration::from_secs(12));
    assert_eq!(store.snapshot().unwrap().commit_revision, 1);
    drop(lock);
    let receipt = store
        .dispatch(manual("busy-retry", "回復した原文"), 103)
        .unwrap();
    assert!(!receipt.replayed);
    assert_eq!(receipt.revision, 2);
    acceptance_evidence::record(
        "cross-process-busy",
        &[("STORAGE-06", 1)],
        serde_json::json!({"actual_test":"another_process_reserved_lock_times_out_then_the_real_writer_recovers"}),
    );
}

#[test]
fn killed_process_releases_actual_os_lease_and_separate_profiles_remain_isolated() {
    let dir = directory("os-lease");
    let lease = child("lease", &dir);
    let path = dir.join("profile.sqlite3");
    assert_eq!(
        SqliteStore::open(&path, "fault-profile", "second-instance", 102)
            .err()
            .unwrap()
            .code,
        "profile_in_use"
    );
    let other = SqliteStore::open(
        dir.join("other-profile.sqlite3"),
        "other-profile",
        "other-instance",
        102,
    )
    .unwrap();
    assert!(other.history(None, None, 100).unwrap().units.is_empty());
    drop(lease);
    let reopened = SqliteStore::open(&path, "fault-profile", "after-crash-instance", 103).unwrap();
    let snapshot = reopened.snapshot().unwrap();
    assert_eq!(snapshot.profile_generation, 2);
    assert_eq!(snapshot.server_instance_id, "after-crash-instance");
    assert_eq!(
        reopened.history(None, None, 100).unwrap().units[0].source_text,
        "子プロセスの原文"
    );
    assert!(other.history(None, None, 100).unwrap().units.is_empty());
    acceptance_evidence::record(
        "os-lease-profile-isolation",
        &[("STORAGE-15", 1)],
        serde_json::json!({"actual_test":"killed_process_releases_actual_os_lease_and_separate_profiles_remain_isolated"}),
    );
}

#[test]
fn process_kill_before_sqlite_actor_commit_rolls_back_all_related_rows_and_revision() {
    let dir = directory("before-commit-crash");
    let process = child("before_actor_transaction_commit", &dir);
    drop(process);
    let store = SqliteStore::open(
        dir.join("profile.sqlite3"),
        "fault-profile",
        "restarted",
        103,
    )
    .unwrap();
    let counts = store
        .call(|db| {
            Ok((
                db.conn
                    .query_row::<u32, _, _>("SELECT count(*) FROM games", [], |r| r.get(0))
                    .map_err(err)?,
                db.conn
                    .query_row::<u32, _, _>("SELECT count(*) FROM game_targets", [], |r| r.get(0))
                    .map_err(err)?,
                db.conn
                    .query_row::<u32, _, _>(
                        "SELECT count(*) FROM pragma_foreign_key_check",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(err)?,
            ))
        })
        .unwrap();
    assert_eq!(counts, (0, 0, 0));
    assert_eq!(store.snapshot().unwrap().commit_revision, 0);
    acceptance_evidence::record(
        "kill-before-commit",
        &[("STORAGE-09", 1)],
        serde_json::json!({"actual_test":"process_kill_before_sqlite_actor_commit_rolls_back_all_related_rows_and_revision"}),
    );
}

#[test]
fn process_kill_after_actual_business_commit_keeps_unit_receipt_and_revision_together() {
    let dir = directory("after-commit-crash");
    let process = child("after_business_commit", &dir);
    drop(process);
    let store = SqliteStore::open(
        dir.join("profile.sqlite3"),
        "fault-profile",
        "restarted",
        103,
    )
    .unwrap();
    assert_eq!(store.snapshot().unwrap().commit_revision, 1);
    let replay = store
        .dispatch(manual("business-committed", "確定した原文🦀"), 104)
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.revision, 1);
    assert_eq!(
        store
            .unit(replay.result_id.unwrap(), 1)
            .unwrap()
            .source_text,
        "確定した原文🦀"
    );
    let counts = store
        .call(|db| {
            Ok((
                db.conn
                    .query_row::<u32, _, _>("SELECT count(*) FROM reading_units", [], |r| r.get(0))
                    .map_err(err)?,
                db.conn
                    .query_row::<u32, _, _>("SELECT count(*) FROM operation_receipts", [], |r| {
                        r.get(0)
                    })
                    .map_err(err)?,
                db.conn
                    .query_row::<u32, _, _>(
                        "SELECT count(*) FROM pragma_foreign_key_check",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(err)?,
            ))
        })
        .unwrap();
    assert_eq!(counts, (1, 1, 0));
    acceptance_evidence::record(
        "kill-after-commit",
        &[("STORAGE-09", 1)],
        serde_json::json!({"actual_test":"process_kill_after_actual_business_commit_keeps_unit_receipt_and_revision_together"}),
    );
}

#[test]
fn online_backup_during_wal_writes_restores_one_exact_committed_business_prefix() {
    let dir = directory("online-wal-backup");
    let path = dir.join("profile.sqlite3");
    let store = SqliteStore::open(&path, "fault-profile", "wal-writer", 100).unwrap();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (continue_tx, continue_rx) = mpsc::sync_channel(1);
    let writer = store.clone();
    let thread = thread::spawn(move || {
        for n in 0..100 {
            writer
                .dispatch(
                    manual(&format!("wal-{n}"), &format!("原文 {n} 🦀")),
                    101 + n,
                )
                .unwrap();
            if n == 9 {
                started_tx.send(()).unwrap();
                continue_rx.recv().unwrap();
            }
        }
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        std::fs::metadata(path.with_extension("sqlite3-wal"))
            .unwrap()
            .len()
            > 0
    );
    continue_tx.send(()).unwrap();
    let backup = dir.join("online-backup.sqlite3");
    store.backup(&backup).unwrap();
    thread.join().unwrap();
    let restored = SqliteStore::open(&backup, "fault-profile", "restored-instance", 1000).unwrap();
    let snapshot = restored.snapshot().unwrap();
    assert!((10..=100).contains(&snapshot.commit_revision));
    let mut history = restored.history(None, None, 200).unwrap().units;
    history.sort_by(|a, b| a.created_at_ms.total_cmp(&b.created_at_ms));
    assert_eq!(history.len(), snapshot.commit_revision as usize);
    for (n, row) in history.iter().enumerate() {
        assert_eq!(row.source_text, format!("原文 {n} 🦀"));
    }
    let receipts = restored
        .call(|db| {
            db.conn
                .query_row::<u32, _, _>("SELECT count(*) FROM operation_receipts", [], |r| r.get(0))
                .map_err(err)
        })
        .unwrap();
    assert_eq!(receipts, snapshot.commit_revision);
    assert_eq!(store.snapshot().unwrap().commit_revision, 100);
}
