//! Actual SQLite/pipeline regressions for RW-17..23; no live-game acceptance claimed.
#[allow(dead_code)]
#[path = "storage_acceptance_support/frozen_hosts.rs"]
mod frozen_hosts;
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
    ports::{
        ProductRepository,
        runtime::{HookEventInput, HookStageInput},
    },
};
use app_infra::storage::{SqliteStore, profile_migration_sha256, session_records_migration_sha256};
use rusqlite::Connection;
use std::path::PathBuf;
struct Fixture {
    store: SqliteStore,
    path: PathBuf,
    session: String,
}
fn hook_session(id: &str) -> HookSessionInput {
    HookSessionInput {
        session_id: id.into(),
        game_id: None,
        environment_key: "controlled-env".into(),
        target_exe_path: "C:\\controlled.exe".into(),
        windows_pid: 100,
        process_created_u64: "18446744073709551615".into(),
        architecture: "x86".into(),
        broker_instance_id: format!("broker-{id}"),
        resource: ResourceSnapshotInput {
            snapshot_id: "controlled-core".into(),
            kind: "core".into(),
            version: "fixture".into(),
            manifest_sha256: content_hash("controlled-core"),
            contract_version: "abi1-jsonl2".into(),
            metadata_json: "{}".into(),
        },
        target_identity_sha256: content_hash(id),
    }
}
fn raw(session: &str, n: u64, text: &str) -> HookTextInput {
    HookTextInput {
        session_id: session.into(),
        event_id: format!("raw-{session}-{n}"),
        sequence_u64: n.to_string(),
        source: HookSourceInput {
            source_id: format!("source-{session}"),
            host_instance_id: format!("broker-{session}"),
            hook_address_u64: "18446744073709551615".into(),
            context_u64: "0".into(),
            subcontext_u64: "0".into(),
            hook_code: "controlled-hook".into(),
            hook_name: "controlled".into(),
            encoding: "core-decoded-unicode".into(),
            stream_identity_sha256: content_hash(session),
            identity_extensions_json: "{}".into(),
        },
        text: text.into(),
        source_language: "ja".into(),
    }
}
fn query(session: &str, dataset: SessionDataset) -> SessionRecordQuery {
    SessionRecordQuery {
        session_id: session.into(),
        dataset,
        order: SessionRecordOrder::OldestFirst,
        filter: SessionRecordFilter::default(),
        cursor: None,
        page_size: 50,
    }
}
fn control(op: Option<&str>, session: Option<&str>, id: &str) -> HookEventInput {
    HookEventInput {
        event_id: id.into(),
        operation_id: op.map(str::to_owned),
        session_id: session.map(str::to_owned),
        source: None,
        wire_sequence: None,
        observed_at_ms: 1,
        detail: HookStageInput {
            level: "info".into(),
            stage: "resource".into(),
            code: "resource_verified".into(),
            message: "资源核对完成".into(),
        },
        lost_count: None,
        integrity: "intact".into(),
    }
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "luna-session-batch-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let store = SqliteStore::open(&path, "records-profile", "records-server", 1).unwrap();
        let session = "session-a".to_string();
        store
            .begin_capture_operation("operation-a".into(), content_hash("request-a"), 1)
            .unwrap();
        store
            .record_hook_event(control(Some("operation-a"), None, "before-session"))
            .unwrap();
        store.open_hook_session(hook_session(&session), 2).unwrap();
        store
            .finish_capture_operation("operation-a".into(), Some(session.clone()), 3)
            .unwrap();
        Self {
            store,
            path,
            session,
        }
    }
    fn command(&self, command: Command, n: i64) {
        self.store
            .dispatch(
                CommandEnvelope {
                    operation_id: format!("cmd-{n}"),
                    expected_revision: None,
                    command,
                },
                n,
            )
            .unwrap();
    }
    fn scalar(&self, sql: &str) -> i64 {
        Connection::open(&self.path)
            .unwrap()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }
}
#[test]
fn paused_text_is_durable_without_body_and_resume_does_not_replay() {
    let f = Fixture::new();
    f.store
        .ingest_hook_text(raw(&f.session, 1, "候補"), 4)
        .unwrap();
    f.command(
        Command::SelectSource {
            source_id: format!("source-{}", f.session),
        },
        5,
    );
    f.store
        .ingest_hook_text_with_policy(raw(&f.session, 2, "暂停期间。"), false, 6)
        .unwrap();
    assert_eq!(f.scalar("SELECT count(*) FROM raw_events"), 2);
    assert_eq!(f.scalar("SELECT count(*) FROM reading_units"), 0);
    f.store
        .ingest_hook_text_with_policy(raw(&f.session, 3, "恢复之后。"), true, 7)
        .unwrap();
    assert_eq!(f.scalar("SELECT count(*) FROM reading_units"), 1);
    let p = f
        .store
        .session_records(query(&f.session, SessionDataset::Extraction), 8)
        .unwrap();
    assert_eq!(p.records.len(), 3);
    assert!(
        matches!(&p.records[1],SessionRecord::Extraction{reason,text,body_refs,..}if reason=="text_paused"&&text.as_deref()==Some("暂停期间。")&&body_refs.is_empty())
    );
    assert!(
        matches!(&p.records[2],SessionRecord::Extraction{reason,body_refs,..}if reason=="body"&&body_refs.len()==1)
    );
}
#[test]
fn entire_dataset_search_stable_paging_and_live_new_count() {
    let f = Fixture::new();
    for n in 1..=121 {
        f.store
            .ingest_hook_text_with_policy(
                raw(&f.session, n, &format!("原文第{n}句。")),
                false,
                n as i64 + 4,
            )
            .unwrap();
    }
    let q = query(&f.session, SessionDataset::Extraction);
    let first = f.store.session_records(q.clone(), 200).unwrap();
    assert_eq!(first.records.len(), 50);
    assert_eq!(first.snapshot.matching_count, "121");
    f.store
        .ingest_hook_text_with_policy(raw(&f.session, 122, "后来新增。"), false, 201)
        .unwrap();
    let mut refresh = q.clone();
    refresh.cursor = Some(first.snapshot.refresh_cursor.clone());
    let refreshed = f.store.session_records(refresh, 202).unwrap();
    assert_eq!(
        refreshed.snapshot.high_watermark,
        first.snapshot.high_watermark
    );
    assert_eq!(refreshed.snapshot.newer_count, "1");
    let mut seen = first.records.len();
    let mut next = first.next_cursor.clone();
    while let Some(c) = next {
        let mut more = q.clone();
        more.cursor = Some(c);
        let page = f.store.session_records(more, 203).unwrap();
        seen += page.records.len();
        next = page.next_cursor;
    }
    assert_eq!(seen, 121);
    let mut searched = q.clone();
    searched.filter.search = Some("第121句".into());
    let hits = f.store.session_records(searched.clone(), 204).unwrap();
    assert_eq!(hits.snapshot.matching_count, "1");
    assert_eq!(hits.records.len(), 1);
    searched.cursor = Some(first.snapshot.refresh_cursor);
    assert_eq!(
        f.store.session_records(searched, 205).unwrap_err().code,
        "invalid_cursor"
    );
    let mut invalid = q;
    invalid.cursor = Some("{\"snapshot\":\"forged\",\"after\":\"0\"}".into());
    assert_eq!(
        f.store.session_records(invalid, 205).unwrap_err().code,
        "invalid_cursor"
    );
}
#[test]
fn pre_session_facts_bridge_and_more_than_100_controls_remain() {
    let f = Fixture::new();
    for n in 0..150 {
        f.store
            .record_hook_event(control(None, Some(&f.session), &format!("control-{n}")))
            .unwrap();
    }
    let mut q = query(&f.session, SessionDataset::HookEvents);
    q.page_size = 200;
    let p = f.store.session_records(q, 20).unwrap();
    assert_eq!(p.snapshot.matching_count, "151");
    assert!(
        matches!(&p.records[0],SessionRecord::HookEvent{event_id,session_id,..}if event_id=="before-session"&&session_id.as_deref()==Some(f.session.as_str()))
    );
    f.store
        .begin_capture_operation(
            "failed-before-session".into(),
            content_hash("failed-request"),
            100,
        )
        .unwrap();
    f.store
        .record_hook_event(control(
            Some("failed-before-session"),
            None,
            "failed-pre-resource",
        ))
        .unwrap();
    f.store
        .finish_capture_operation("failed-before-session".into(), None, 99)
        .unwrap();
    assert_eq!(
        f.scalar("SELECT count(*) FROM hook_events WHERE operation_id='failed-before-session'"),
        1
    );
    assert_eq!(
        f.scalar(
            "SELECT ended_at_ms FROM capture_operations WHERE operation_id='failed-before-session'"
        ),
        99
    );
}
#[test]
fn diagnostic_view_filters_before_paging_counts_search_and_pinned_export() {
    let f = Fixture::new();
    for n in 0..115 {
        let mut event = control(None, Some(&f.session), &format!("diagnosis-{n}"));
        event.detail.stage = "injection".into();
        event.detail.code = "hook_inserted".into();
        event.detail.message = format!("Leaf引擎 Hook {n} · address_u64=18446744073709551615");
        f.store.record_hook_event(event).unwrap();
    }
    // More than the UI's 50-row window arrives after all useful diagnostics.
    // The authoritative query must still expose all of those earlier facts.
    for n in 1..=130 {
        f.store
            .ingest_hook_text_with_policy(
                raw(&f.session, n, &format!("未选择の本文{n}。")),
                false,
                n as i64 + 10,
            )
            .unwrap();
    }
    for (n, (level, integrity, lost_count)) in [
        ("warning", "intact", None),
        ("error", "intact", None),
        ("info", "lost", Some("18446744073709551615")),
        ("info", "unknown", None),
    ]
    .into_iter()
    .enumerate()
    {
        let mut event = control(
            None,
            Some(&f.session),
            &format!("protected-disposition-{n}"),
        );
        event.detail.level = level.into();
        event.detail.stage = "capture".into();
        event.detail.code = "text_unselected".into();
        event.detail.message = "必须保留的异常或完整性事实".into();
        event.integrity = integrity.into();
        event.lost_count = lost_count.map(str::to_owned);
        f.store.record_hook_event(event).unwrap();
    }
    let mut q = query(&f.session, SessionDataset::HookEvents);
    q.order = SessionRecordOrder::NewestFirst;
    q.filter.stage = Some("diagnostics".into());
    let first = f.store.session_records(q.clone(), 200).unwrap();
    assert_eq!(first.snapshot.matching_count, "120");
    assert_eq!(first.records.len(), 50);
    assert!(first.records.iter().all(|record| matches!(record, SessionRecord::HookEvent { code, level, integrity, .. } if code != "text_unselected" || level != "info" || integrity != "intact")));
    assert_eq!(first.completeness.status, "incomplete");
    let mut all_diagnostics = first.records.clone();
    let mut token = first.next_cursor.clone();
    while let Some(cursor) = token {
        q.cursor = Some(cursor);
        let next = f.store.session_records(q.clone(), 201).unwrap();
        assert_eq!(next.snapshot.matching_count, "120");
        all_diagnostics.extend(next.records);
        token = next.next_cursor;
    }
    assert_eq!(all_diagnostics.len(), 120);
    assert!(all_diagnostics.iter().any(
        |r| matches!(r, SessionRecord::HookEvent { event_id, .. } if event_id == "diagnosis-0")
    ));
    let mut search = q.clone();
    search.cursor = None;
    search.filter.search = Some("Leaf引擎 Hook 0 ·".into());
    let result = f.store.session_records(search, 202).unwrap();
    assert_eq!(result.snapshot.matching_count, "1");
    assert!(
        matches!(&result.records[0], SessionRecord::HookEvent { message, .. } if message.contains("18446744073709551615"))
    );
    let mut warning = q.clone();
    warning.cursor = None;
    warning.filter.level = Some("warning".into());
    assert_eq!(
        f.store
            .session_records(warning, 203)
            .unwrap()
            .snapshot
            .matching_count,
        "1"
    );
    let mut frozen = q.clone();
    frozen.cursor = Some(first.snapshot.refresh_cursor.clone());
    let plan = f
        .store
        .prepare_session_export(
            SessionExportRequest {
                query: frozen.clone(),
                privacy_confirmed: true,
            },
            204,
        )
        .unwrap();
    assert_eq!(plan.snapshot.matching_count, "120");
    f.store
        .ingest_hook_text_with_policy(raw(&f.session, 131, "遅い未選択回調。"), false, 205)
        .unwrap();
    assert_eq!(
        f.store
            .session_records(frozen.clone(), 206)
            .unwrap()
            .snapshot
            .newer_count,
        "0"
    );
    let mut late = control(None, Some(&f.session), "late-diagnostic");
    late.detail.message = "晚到接入诊断".into();
    f.store.record_hook_event(late).unwrap();
    assert_eq!(
        f.store
            .session_records(frozen.clone(), 207)
            .unwrap()
            .snapshot
            .newer_count,
        "1"
    );
    let mut export_cursor = None;
    let mut exported = Vec::new();
    loop {
        let page = f
            .store
            .session_export_page(plan.export_id.clone(), export_cursor, 37, 208)
            .unwrap();
        exported.extend(page.records);
        export_cursor = page.next_cursor;
        if export_cursor.is_none() {
            break;
        }
    }
    assert_eq!(exported.len(), 120);
    assert!(exported.iter().all(
        |r| !matches!(r, SessionRecord::HookEvent { event_id, .. } if event_id == "late-diagnostic")
    ));
    assert!(exported.iter().any(|r| matches!(r, SessionRecord::HookEvent { lost_count, .. } if lost_count.as_deref() == Some("18446744073709551615"))));
    f.store.close_session_export(plan.export_id).unwrap();
    let mut all = frozen;
    all.filter.stage = None;
    assert_eq!(
        f.store.session_records(all.clone(), 209).unwrap_err().code,
        "invalid_cursor"
    );
    all.cursor = None;
    let full = f.store.session_records(all.clone(), 210).unwrap();
    assert_eq!(full.snapshot.matching_count, "252");
    let plan = f
        .store
        .prepare_session_export(
            SessionExportRequest {
                query: all,
                privacy_confirmed: true,
            },
            211,
        )
        .unwrap();
    let mut token = None;
    let mut full_count = 0;
    loop {
        let page = f
            .store
            .session_export_page(plan.export_id.clone(), token, 200, 212)
            .unwrap();
        full_count += page.records.len();
        token = page.next_cursor;
        if token.is_none() {
            break;
        }
    }
    assert_eq!(full_count, 252);
    let raw = f
        .store
        .session_records(query(&f.session, SessionDataset::Extraction), 213)
        .unwrap();
    assert_eq!(raw.snapshot.matching_count, "131");
    assert_eq!(f.scalar("SELECT count(*) FROM raw_event_payloads"), 131);
}

#[test]
fn diagnostics_is_only_a_hook_query_view_not_a_persisted_or_cross_dataset_stage() {
    let f = Fixture::new();
    for dataset in [SessionDataset::Body, SessionDataset::Extraction] {
        let mut q = query(&f.session, dataset);
        q.filter.stage = Some("diagnostics".into());
        assert_eq!(
            f.store.session_records(q, 5).unwrap_err().code,
            "invalid_session_query"
        );
    }
    let mut q = query(&f.session, SessionDataset::HookEvents);
    q.filter.stage = Some("unknown-view".into());
    assert_eq!(
        f.store.session_records(q, 6).unwrap_err().code,
        "invalid_session_query"
    );
    let mut event = control(None, Some(&f.session), "illegal-query-stage");
    event.detail.stage = "diagnostics".into();
    assert_eq!(
        f.store.record_hook_event(event).unwrap_err().code,
        "invalid_hook_event"
    );
    assert_eq!(
        f.scalar("SELECT count(*) FROM hook_events WHERE stage='diagnostics'"),
        0
    );
}
#[test]
fn local_sequence_crosses_u32_and_exhaustion_is_explicit() {
    let f = Fixture::new();
    f.store
        .ingest_hook_text(raw(&f.session, 1, "一"), 4)
        .unwrap();
    let c = Connection::open(&f.path).unwrap();
    c.execute("INSERT INTO raw_events SELECT 'wide-seed',session_id,source_id,'2',4294967296,event_kind,payload_byte_length,payload_sha256,observed_at_ms,integrity_state FROM raw_events WHERE event_id='raw-session-a-1'",[]).unwrap();
    f.store
        .ingest_hook_text(raw(&f.session, 3, "二"), 5)
        .unwrap();
    assert_eq!(
        f.scalar("SELECT max(ingest_sequence) FROM raw_events"),
        4294967297
    );
    c.execute("INSERT INTO raw_events SELECT 'last-seed',session_id,source_id,'4',9223372036854775807,event_kind,payload_byte_length,payload_sha256,observed_at_ms,integrity_state FROM raw_events WHERE event_id='raw-session-a-1'",[]).unwrap();
    assert_eq!(
        f.store
            .ingest_hook_text(raw(&f.session, 5, "拒绝越界"), 6)
            .unwrap_err()
            .code,
        "sequence_exhausted"
    );
    assert_eq!(
        f.scalar("SELECT count(*) FROM raw_events WHERE event_id='raw-session-a-5'"),
        0
    );
}
#[test]
fn export_is_fixed_range_cancel_is_non_destructive_and_active_raw_is_protected() {
    let f = Fixture::new();
    for n in 1..=3 {
        f.store
            .ingest_hook_text_with_policy(raw(&f.session, n, "未选流原文。"), false, n as i64 + 3)
            .unwrap();
    }
    let q = query(&f.session, SessionDataset::Extraction);
    assert_eq!(
        f.store
            .prepare_session_export(
                SessionExportRequest {
                    query: q.clone(),
                    privacy_confirmed: false
                },
                10
            )
            .unwrap_err()
            .code,
        "export_confirmation_required"
    );
    let plan = f
        .store
        .prepare_session_export(
            SessionExportRequest {
                query: q,
                privacy_confirmed: true,
            },
            10,
        )
        .unwrap();
    f.store
        .ingest_hook_text_with_policy(raw(&f.session, 4, "导出之后。"), false, 11)
        .unwrap();
    let p = f
        .store
        .session_export_page(plan.export_id.clone(), None, 200, 12)
        .unwrap();
    assert_eq!(p.records.len(), 3);
    assert_eq!(p.snapshot.high_watermark, plan.snapshot.high_watermark);
    let receipt = f
        .store
        .maintain_storage(
            app_core::domain::retention::RetentionPolicy {
                raw_payload_before_ms: Some(100),
                raw_payload_byte_limit: Some(0),
                max_rows_per_batch: 100,
                ..Default::default()
            },
            Default::default(),
            "active-retention".into(),
            20,
        )
        .unwrap();
    assert_eq!(receipt.removed_raw_payloads, 0);
    f.store
        .close_session_export(plan.export_id.clone())
        .unwrap();
    f.store
        .close_session_export(plan.export_id.clone())
        .unwrap();
    assert_eq!(f.scalar("SELECT count(*) FROM raw_event_payloads"), 4);
    assert_eq!(
        f.store
            .session_export_page(plan.export_id, None, 200, 22)
            .unwrap_err()
            .code,
        "export_missing"
    );
}
#[test]
fn v1_upgrade_preserves_rewound_legacy_times_and_creates_validated_backup() {
    let path = std::env::temp_dir().join(format!(
        "luna-migration-v2-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let c = Connection::open(&path).unwrap();
    c.execute_batch(include_str!("../src/storage/migrations/001_profile.sql"))
        .unwrap();
    c.execute_batch(include_str!("../src/storage/migrations/001_config.sql"))
        .unwrap();
    c.execute(
        "INSERT INTO schema_migrations VALUES(1,?1,1,'old',0)",
        [profile_migration_sha256()],
    )
    .unwrap();
    c.execute(
        "INSERT INTO profile_meta VALUES(1,'migration-profile',0,1,'old','active',1,1)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO resource_snapshots VALUES('r','core','v1',?1,'1','{}',1)",
        [content_hash("legacy-resource")],
    )
    .unwrap();
    c.execute("INSERT INTO sessions VALUES('legacy',NULL,'hook','stopped','env','path',1,'1','x86','broker','r',?1,100,99)",[content_hash("legacy-identity")]).unwrap();
    drop(c);
    // Run the identical v1 dataset through real frozen Store2/Store3 and the current Store4.
    // Neither a renamed expectation nor manually applied SQL substitutes for the old host.
    let old_path = path.with_file_name(format!(
        "{}-old-host.sqlite3",
        path.file_stem().unwrap().to_string_lossy()
    ));
    std::fs::copy(&path, &old_path).unwrap();
    let three_path = path.with_file_name(format!(
        "{}-old-three-host.sqlite3",
        path.file_stem().unwrap().to_string_lossy()
    ));
    std::fs::copy(&path, &three_path).unwrap();
    let old_host = frozen_hosts::FrozenHost::two();
    old_host.open(&old_path, "migration-profile");
    let three_host = frozen_hosts::FrozenHost::three();
    three_host.open(&three_path, "migration-profile");
    let four_path = path.with_file_name(format!("{}-old-four-host.sqlite3", path.file_stem().unwrap().to_string_lossy()));
    std::fs::copy(&path, &four_path).unwrap();
    let four_host = frozen_hosts::FrozenHost::four();
    four_host.open(&four_path, "migration-profile");
    let four_backup = four_path.with_file_name(format!("{}.pre-v4-100.sqlite3", four_path.file_name().unwrap().to_string_lossy()));
    let three_backup = three_path.with_file_name(format!(
        "{}.pre-v3-300.sqlite3",
        three_path.file_name().unwrap().to_string_lossy()
    ));
    let old_backup = old_path.with_file_name(format!(
        "{}.pre-v2-300.sqlite3",
        old_path.file_name().unwrap().to_string_lossy()
    ));
    let store = SqliteStore::open(&path, "migration-profile", "new", 200).unwrap();
    let backup = path.with_file_name(format!(
        "{}.pre-v5-200.sqlite3",
        path.file_name().unwrap().to_string_lossy()
    ));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (migrated, saved, contract) in [
        (
            &old_path,
            &old_backup,
            "docs/architecture/database/v2/schema-contract.json",
        ),
        (
            &three_path,
            &three_backup,
            "docs/architecture/database/v3/schema-contract.json",
        ),
        (
            &path,
            &backup,
            "docs/architecture/database/v5/schema-contract.json",
        ),
        (
            &four_path,
            &four_backup,
            "docs/architecture/database/v4/schema-contract.json",
        ),
    ] {
        assert!(saved.is_file());
        let original = Connection::open(saved).unwrap();
        assert_eq!(
            original
                .query_row::<i64, _, _>("SELECT max(version) FROM schema_migrations", [], |r| r
                    .get(0))
                .unwrap(),
            1
        );
        assert_eq!(
            original
                .query_row::<(i64, i64), _, _>(
                    "SELECT started_at_ms,ended_at_ms FROM sessions WHERE session_id='legacy'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?))
                )
                .unwrap(),
            (100, 99)
        );
        let c = Connection::open(migrated).unwrap();
        assert_eq!(
            c.query_row::<String, _, _>(
                "SELECT migration_sha256 FROM schema_migrations WHERE version=2",
                [],
                |r| r.get(0)
            )
            .unwrap(),
            session_records_migration_sha256()
        );
        assert_eq!(c.query_row::<(i64,i64),_,_>("SELECT started_at_ms,ended_at_ms FROM capture_operations WHERE session_id='legacy'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap(),(100,99));
        let reviewed: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(contract)).unwrap()).unwrap();
        assert_eq!(
            c.query_row::<u32, _, _>("SELECT max(version) FROM schema_migrations", [], |r| r
                .get(0))
                .unwrap(),
            reviewed["migration_version"].as_u64().unwrap() as u32
        );
    }
    drop(store);
    SqliteStore::open(&path, "migration-profile", "again", 201).unwrap();
}

#[cfg(unix)]
mod inherited_profile_lease {
    use super::*;
    use std::{
        io::{Read, Write},
        os::{fd::AsRawFd, raw::c_int, unix::net::UnixStream},
        time::Duration,
    };

    unsafe extern "C" {
        fn fork() -> c_int;
        fn read(fd: c_int, buffer: *mut u8, count: usize) -> isize;
        fn write(fd: c_int, buffer: *const u8, count: usize) -> isize;
        fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
        fn _exit(status: c_int) -> !;
    }

    struct HeldDescriptors {
        pid: Option<c_int>,
        channel: UnixStream,
    }
    impl HeldDescriptors {
        fn start() -> Self {
            let (channel, child) = UnixStream::pair().unwrap();
            channel
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let child_fd = child.as_raw_fd();
            // SAFETY: the child only uses async-signal-safe syscalls until _exit;
            // it never enters SQLite, the allocator, or copied Rust thread locks.
            let pid = unsafe { fork() };
            assert!(pid >= 0, "{}", std::io::Error::last_os_error());
            if pid == 0 {
                let mut byte = 1_u8;
                // SAFETY: child_fd is inherited and both buffers remain valid.
                // The blocking read keeps inherited leases alive until released.
                unsafe {
                    if write(child_fd, &byte, 1) != 1 {
                        _exit(11);
                    }
                    if read(child_fd, &mut byte, 1) != 1 {
                        _exit(12);
                    }
                    _exit(0);
                }
            }
            drop(child);
            let mut held = Self {
                pid: Some(pid),
                channel,
            };
            let mut ready = [0];
            held.channel.read_exact(&mut ready).unwrap();
            assert_eq!(ready, [1]);
            held
        }

        fn release(&mut self) -> std::io::Result<c_int> {
            let Some(pid) = self.pid.take() else {
                return Ok(0);
            };
            let release = self.channel.write_all(&[1]);
            let mut status = 0;
            loop {
                // SAFETY: pid is our single unreaped child and status is valid.
                let waited = unsafe { waitpid(pid, &mut status, 0) };
                if waited == pid {
                    release?;
                    return Ok(status);
                }
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }

        fn finish(mut self) {
            assert_eq!(self.release().unwrap(), 0);
        }
    }
    impl Drop for HeldDescriptors {
        fn drop(&mut self) {
            let _ = self.release();
        }
    }

    #[test]
    fn last_store_owner_releases_profile_while_fork_child_still_holds_descriptors() {
        let directory =
            std::env::temp_dir().join(format!("luna-inherited-profile-{}", uuid::Uuid::new_v4()));
        let path = directory.join("profile.sqlite3");
        let store = SqliteStore::open(&path, "inherited-profile", "first", 1).unwrap();
        let mut preferences = store.snapshot().unwrap().preferences;
        preferences.hud_background_opacity = 0.42;
        store
            .dispatch(
                CommandEnvelope {
                    operation_id: "persist-before-close".into(),
                    expected_revision: None,
                    command: Command::SavePreferences {
                        preferences: preferences.clone(),
                    },
                },
                2,
            )
            .unwrap();
        let remaining_owner = store.clone();
        let child = HeldDescriptors::start();
        drop(store);
        assert_eq!(
            SqliteStore::open(&path, "inherited-profile", "blocked", 3)
                .err()
                .unwrap()
                .code,
            "profile_in_use",
            "a live clone must keep the original owner's lease"
        );
        let independent = SqliteStore::open(
            directory.join("independent.sqlite3"),
            "independent-profile",
            "independent",
            3,
        )
        .unwrap();
        drop(remaining_owner);
        // No child release, sleep, or retry precedes this production reopen.
        let reopened = SqliteStore::open(&path, "inherited-profile", "reopened", 4).unwrap();
        let snapshot = reopened.snapshot().unwrap();
        assert_eq!(snapshot.preferences, preferences);
        assert_eq!(snapshot.profile_generation, 2);
        assert_eq!(snapshot.server_instance_id, "reopened");
        child.finish();
        drop(reopened);
        drop(independent);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn failed_profile_initialization_preserves_input_and_releases_its_lease() {
    let path = std::env::temp_dir().join(format!(
        "luna-invalid-profile-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('original');",
        )
        .unwrap();
    drop(connection);
    let original = std::fs::read(&path).unwrap();
    assert_eq!(
        SqliteStore::open(&path, "failed-profile", "rejected", 1)
            .err()
            .unwrap()
            .code,
        "unsupported_schema"
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(&path).unwrap();
    let valid = SqliteStore::open(&path, "failed-profile", "after-failure", 2).unwrap();
    assert_eq!(valid.snapshot().unwrap().profile_generation, 1);
    drop(valid);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_file(path.with_extension("sqlite3.lock")).unwrap();
}

#[test]
fn reading_preferences_typed_columns_roundtrip_and_cycle_only_mode() {
    let f = Fixture::new();
    let p = Preferences {
        segmentation_enabled: false,
        reading_display_mode: ReadingDisplayMode::Source,
        hud_background_opacity: 0.24,
        ..Default::default()
    };
    f.command(
        Command::SavePreferences {
            preferences: p.clone(),
        },
        30,
    );
    assert_eq!(f.store.snapshot().unwrap().preferences, p);
    f.command(Command::CycleReadingDisplayMode, 31);
    let updated = f.store.snapshot().unwrap().preferences;
    assert_eq!(
        updated.reading_display_mode,
        ReadingDisplayMode::Translation
    );
    assert_eq!(updated.segmentation_enabled, p.segmentation_enabled);
    assert_eq!(updated.show_ruby, p.show_ruby);
    assert_eq!(updated.hud_background_opacity, p.hud_background_opacity);
    for opacity in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        let p = Preferences {
            hud_background_opacity: opacity,
            ..Default::default()
        };
        assert!(app_core::domain::validate_preferences(&p).is_err());
    }
    let c = Connection::open(&f.path).unwrap();
    assert!(
        c.execute(
            "UPDATE reading_preference_settings SET hud_background_opacity=0.5",
            []
        )
        .is_err()
    );
    assert!(
        c.execute(
            "INSERT INTO reading_preference_settings VALUES('missing',1,'both',0.5)",
            []
        )
        .is_err()
    );
}
#[test]
fn newest_first_pages_prepend_and_export_freezes_old_range_in_ascending_order() {
    let f = Fixture::new();
    for n in 1..=123 {
        f.store
            .ingest_hook_text_with_policy(
                raw(&f.session, n, &format!("行{n}。")),
                false,
                n as i64 + 4,
            )
            .unwrap();
    }
    let mut q = query(&f.session, SessionDataset::Extraction);
    q.order = SessionRecordOrder::NewestFirst;
    let first = f.store.session_records(q.clone(), 200).unwrap();
    assert_eq!(first.loaded_range.first.as_deref(), Some("74"));
    assert_eq!(first.loaded_range.last.as_deref(), Some("123"));
    f.store
        .ingest_hook_text_with_policy(raw(&f.session, 124, "新行。"), false, 201)
        .unwrap();
    let mut next = q.clone();
    next.cursor = first.next_cursor.clone();
    let second = f.store.session_records(next, 202).unwrap();
    assert_eq!(second.loaded_range.first.as_deref(), Some("24"));
    assert_eq!(second.loaded_range.last.as_deref(), Some("73"));
    let mut next = q.clone();
    next.cursor = second.next_cursor;
    let third = f.store.session_records(next, 203).unwrap();
    assert_eq!(third.loaded_range.first.as_deref(), Some("1"));
    assert_eq!(third.loaded_range.last.as_deref(), Some("23"));
    assert!(third.next_cursor.is_none());
    let mut refresh = q.clone();
    refresh.cursor = Some(first.snapshot.refresh_cursor.clone());
    let fixed = f.store.session_records(refresh.clone(), 204).unwrap();
    assert_eq!(fixed.snapshot.high_watermark, "123");
    assert_eq!(fixed.snapshot.newer_count, "1");
    assert_eq!(fixed.loaded_range.first.as_deref(), Some("74"));
    let mut wrong_order = refresh.clone();
    wrong_order.order = SessionRecordOrder::OldestFirst;
    assert_eq!(
        f.store.session_records(wrong_order, 205).unwrap_err().code,
        "invalid_cursor"
    );
    let latest = f.store.session_records(q, 205).unwrap();
    assert_eq!(latest.loaded_range.first.as_deref(), Some("75"));
    assert_eq!(latest.loaded_range.last.as_deref(), Some("124"));
    let plan = f
        .store
        .prepare_session_export(
            SessionExportRequest {
                query: refresh,
                privacy_confirmed: true,
            },
            206,
        )
        .unwrap();
    assert_eq!(plan.order, SessionRecordOrder::OldestFirst);
    assert_eq!(plan.snapshot.high_watermark, "123");
    assert_eq!(plan.snapshot.matching_count, "123");
    let mut token = None;
    let mut sequences = Vec::new();
    loop {
        let p = f
            .store
            .session_export_page(plan.export_id.clone(), token, 50, 207)
            .unwrap();
        for r in p.records {
            if let SessionRecord::Extraction { sequence, .. } = r {
                sequences.push(sequence.parse::<u64>().unwrap())
            } else {
                panic!("wrong dataset")
            }
        }
        if p.next_cursor.is_none() {
            break;
        }
        token = p.next_cursor;
    }
    assert_eq!(sequences, (1..=123).collect::<Vec<_>>());
}
#[test]
fn current_game_preferences_merge_and_preview_are_authoritative_and_read_only() {
    let f = Fixture::new();
    let target = GameTarget {
        environment_kind: "crossover".into(),
        environment_key: "crossover:/private/controlled".into(),
        bottle_locator: Some("/private/controlled".into()),
        launch_path: Some("C:\\controlled.exe".into()),
        canonical_exe_path: Some("C:\\controlled.exe".into()),
        target_path_key: Some("c:/controlled.exe".into()),
        working_directory: None,
        arguments: vec![],
        architecture: Some("x86".into()),
        verification_state: "verified".into(),
    };
    let game = f
        .store
        .dispatch(
            CommandEnvelope {
                operation_id: "new-game".into(),
                expected_revision: None,
                command: Command::SaveGame(Box::new(GameInput {
                    game_id: None,
                    row_revision: None,
                    display_name: "受控游戏".into(),
                    original_title: None,
                    source_language: Some("ja".into()),
                    cover_asset_ref: None,
                    target,
                })),
            },
            30,
        )
        .unwrap()
        .result_id
        .unwrap();
    let global = Preferences {
        source_font_pt: 21.,
        hud_background_opacity: 0.4,
        shortcuts: std::collections::BTreeMap::from([(
            "toggleHud".into(),
            "CmdOrCtrl+Alt+H".into(),
        )]),
        ..Default::default()
    };
    f.command(
        Command::SavePreferences {
            preferences: global.clone(),
        },
        31,
    );
    let local = Preferences {
        scope_key: format!("game:{game}"),
        show_ruby: false,
        hud_background_opacity: 0.9,
        overrides: std::collections::BTreeMap::from([("showRuby".into(), true)]),
        ..Default::default()
    };
    f.command(
        Command::SavePreferences {
            preferences: local.clone(),
        },
        32,
    );
    let mut session = hook_session("session-with-game");
    session.game_id = Some(game);
    session.environment_key = "crossover:/private/controlled".into();
    f.store.open_hook_session(session, 33).unwrap();
    let actual = f.store.snapshot().unwrap().preferences;
    assert!(!actual.show_ruby);
    assert_eq!(actual.source_font_pt, 21.);
    assert_eq!(actual.hud_background_opacity, 0.4);
    assert_eq!(actual.shortcuts, global.shortcuts);
    let before = f.store.snapshot().unwrap().commit_revision;
    let draft = Preferences {
        source_font_pt: 27.,
        show_ruby: true,
        hud_background_opacity: 0.3,
        ..global
    };
    let preview = f.store.preview_preferences(draft).unwrap();
    assert_eq!(preview.source_font_pt, 27.);
    assert!(!preview.show_ruby);
    assert_eq!(preview.hud_background_opacity, 0.3);
    assert_eq!(f.store.snapshot().unwrap().commit_revision, before);
    assert_eq!(f.store.snapshot().unwrap().preferences, actual);
}
#[test]
fn terminal_received_raw_remains_a_fact_without_reviving_body_or_integrity() {
    let f = Fixture::new();
    f.store
        .ingest_hook_text(raw(&f.session, 1, "候选。"), 4)
        .unwrap();
    f.command(
        Command::SelectSource {
            source_id: format!("source-{}", f.session),
        },
        5,
    );
    let mut lost = control(None, Some(&f.session), "reported-loss");
    lost.integrity = "lost".into();
    lost.lost_count = Some("18446744073709551615".into());
    lost.detail.code = "queue_overflow".into();
    f.store.record_hook_event(lost).unwrap();
    f.store
        .record_diagnostic(
            Diagnostic {
                code: "integrity_lost".into(),
                stage: "capture".into(),
                message: "受控丢失".into(),
                recoverable: true,
            },
            6,
        )
        .unwrap();
    assert_eq!(
        f.store
            .ingest_hook_text(raw(&f.session, 2, "严格端口拒绝。"), 7)
            .unwrap_err()
            .code,
        "stale_capture_session"
    );
    let outcome = f
        .store
        .ingest_hook_text_with_policy(raw(&f.session, 3, "已经收到，仍须留存。"), true, 8)
        .unwrap();
    assert!(outcome.visible_version.is_none());
    assert!(outcome.finalized_versions.is_empty());
    assert_eq!(f.scalar("SELECT count(*) FROM reading_units"), 0);
    let page = f
        .store
        .session_records(query(&f.session, SessionDataset::Extraction), 9)
        .unwrap();
    assert_eq!(page.records.len(), 2);
    assert!(page.completeness.known_loss);
    assert_ne!(page.completeness.status, "complete_received_range");
    assert!(
        matches!(&page.records[1],SessionRecord::Extraction{reason,text,..}if reason=="text_inactive"&&text.as_deref()==Some("已经收到，仍须留存。"))
    );
}
#[test]
fn cursor_tampering_and_control_identity_changes_fail_explicitly() {
    let f = Fixture::new();
    for n in 1..=3 {
        f.store
            .ingest_hook_text_with_policy(raw(&f.session, n, "固定原文。"), false, n as i64 + 3)
            .unwrap();
    }
    let mut q = query(&f.session, SessionDataset::Extraction);
    q.page_size = 1;
    let first = f.store.session_records(q.clone(), 10).unwrap();
    let mut tampered: serde_json::Value =
        serde_json::from_str(&first.snapshot.refresh_cursor).unwrap();
    tampered["after"] = "2".into();
    q.cursor = Some(tampered.to_string());
    assert_eq!(
        f.store.session_records(q, 11).unwrap_err().code,
        "invalid_cursor"
    );
    let event = control(None, Some(&f.session), "immutable-control");
    f.store.record_hook_event(event.clone()).unwrap();
    f.store.record_hook_event(event.clone()).unwrap();
    let mut changed = event;
    changed.integrity = "lost".into();
    assert_eq!(
        f.store.record_hook_event(changed).unwrap_err().code,
        "event_identity_conflict"
    );
}
#[test]
fn export_pin_protects_inactive_session_payloads_and_fixed_body_revisions() {
    let f = Fixture::new();
    f.store
        .ingest_hook_text(raw(&f.session, 1, "候选。"), 4)
        .unwrap();
    f.command(
        Command::SelectSource {
            source_id: format!("source-{}", f.session),
        },
        5,
    );
    let unit = f
        .store
        .ingest_hook_text(raw(&f.session, 2, "保存的正文。"), 6)
        .unwrap()
        .visible_version
        .unwrap();
    f.command(Command::Disconnect, 7);
    let body = f
        .store
        .prepare_session_export(
            SessionExportRequest {
                query: query(&f.session, SessionDataset::Body),
                privacy_confirmed: true,
            },
            8,
        )
        .unwrap();
    let policy = app_core::domain::retention::RetentionPolicy {
        raw_payload_before_ms: Some(100),
        raw_payload_byte_limit: Some(0),
        max_rows_per_batch: 100,
        ..Default::default()
    };
    assert_eq!(
        f.store
            .maintain_storage(policy.clone(), Default::default(), "pin-payload".into(), 9)
            .unwrap()
            .removed_raw_payloads,
        0
    );
    let deletion = app_core::domain::retention::RetentionPolicy {
        history_unit_ids: vec![unit.unit_id.clone()],
        max_rows_per_batch: 100,
        ..Default::default()
    };
    assert_eq!(
        f.store
            .maintain_storage(deletion, Default::default(), "pin-body".into(), 10)
            .unwrap_err()
            .code,
        "reading_object_in_use"
    );
    f.store.close_session_export(body.export_id).unwrap();
    assert_eq!(
        f.store
            .maintain_storage(
                policy,
                Default::default(),
                "explicit-inactive-clean".into(),
                11
            )
            .unwrap()
            .removed_raw_payloads,
        2
    );
    let page = f
        .store
        .session_records(query(&f.session, SessionDataset::Extraction), 12)
        .unwrap();
    assert_eq!(page.completeness.status, "cleaned");
    assert_eq!(page.completeness.cleaned_payload_count, "2");
    assert_eq!(
        f.store
            .unit(unit.unit_id, unit.text_revision)
            .unwrap()
            .source_text,
        "保存的正文。"
    );
}
#[test]
fn unconfirmed_write_integrity_survives_normal_disconnect_and_reopen() {
    let f = Fixture::new();
    f.store
        .ingest_hook_text_with_policy(raw(&f.session, 1, "原始事实。"), false, 4)
        .unwrap();
    assert_eq!(
        f.store
            .ingest_hook_text_with_policy(raw(&f.session, 1, "冲突事实。"), false, 5)
            .unwrap_err()
            .code,
        "event_identity_conflict"
    );
    assert_eq!(
        f.store
            .session_records(query(&f.session, SessionDataset::Extraction), 6)
            .unwrap()
            .completeness
            .status,
        "unknown"
    );
    f.command(Command::Disconnect, 7);
    let path = f.path.clone();
    let session = f.session.clone();
    drop(f.store);
    let reopened = SqliteStore::open(&path, "records-profile", "records-server-reopen", 8).unwrap();
    let p = reopened
        .session_records(query(&session, SessionDataset::Extraction), 9)
        .unwrap();
    assert_eq!(p.completeness.status, "unknown");
}

// A frozen range pins membership, not a serialized ReadingUnit. These tests
// exercise public Repository commands and model-completion ports against real
// SQLite; the controlled completion text is not a live-model acceptance claim.
fn refresh_command(f: &Fixture, command: Command, now: i64) -> CommandReceipt {
    f.store
        .dispatch(
            CommandEnvelope {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: None,
                command,
            },
            now,
        )
        .unwrap()
}
fn configure_refresh_translation(f: &Fixture) {
    let configuration: Configuration = serde_json::from_value(serde_json::json!({
        "revisionId": "",
        "connections": [{"connectionId":"refresh-connection","displayName":"Controlled completion port","protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:1/v1","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":true}],
        "models": [{"modelId":"refresh-model","connectionId":"refresh-connection","actualModelId":"controlled-no-network","displayName":"Controlled model","selectionSource":"manual","capabilities":{},"parameters":{}}],
        "bindings": [{"bindingId":"refresh-binding","scopeKey":"global","purpose":"translation","bindingMode":"override","enabled":true,"outputMode":"text","sourceLanguage":"ja","targetLanguage":"zh-CN","context":{},"schedule":{},"parameters":{},"engines":[{"engineId":"refresh-engine","modelId":"refresh-model","enabled":true,"parameters":{}}]}],
        "templates": [], "termSets": []
    })).unwrap();
    let revision = refresh_command(
        f,
        Command::SaveConfiguration {
            configuration,
            expected_head_revision: f
                .store
                .snapshot()
                .unwrap()
                .configuration
                .draft_head_revision,
        },
        10,
    )
    .result_id
    .unwrap();
    refresh_command(
        f,
        Command::ApplyConfiguration {
            revision_id: revision,
            expected_head_revision: f
                .store
                .snapshot()
                .unwrap()
                .configuration
                .applied_head_revision,
        },
        11,
    );
}
fn refresh_select_source(f: &Fixture, session: &str, now: i64) {
    let discovery = f
        .store
        .ingest_hook_text(raw(session, 1, "未選択の候補"), now)
        .unwrap();
    assert!(discovery.visible_version.is_none());
    refresh_command(
        f,
        Command::SelectSource {
            source_id: format!("source-{session}"),
        },
        now + 1,
    );
}
fn refresh_ingest_body(
    f: &Fixture,
    session: &str,
    sequence: u64,
    text: &str,
    now: i64,
) -> ReadingUnit {
    let outcome = f
        .store
        .ingest_hook_text(raw(session, sequence, text), now)
        .unwrap();
    let version = outcome.visible_version.expect("selected body is visible");
    assert!(outcome.finalized_versions.contains(&version));
    f.store
        .unit(version.unit_id, version.text_revision)
        .unwrap()
}
fn refresh_complete_translation(
    f: &Fixture,
    task: &app_core::domain::TaskDispatch,
    text: &str,
    now: i64,
) {
    f.store
        .complete_task(
            app_core::domain::TaskCompletion {
                task_id: task.task_id.clone(),
                attempt_id: task.attempt_id.clone(),
                text: text.into(),
                output_mode: "text".into(),
                structured: serde_json::json!({}),
                target_language: "zh-CN".into(),
                provider_label: "Controlled completion port".into(),
            },
            now,
        )
        .unwrap();
}
fn refresh_body_identity(page: &SessionRecordPage) -> serde_json::Value {
    serde_json::Value::Array(
        page.records
            .iter()
            .map(|record| match record {
                SessionRecord::Body {
                    sequence,
                    unit,
                    source_refs,
                } => serde_json::json!({
                    "sequence": sequence,
                    "sessionId": unit.session_id,
                    "unitId": unit.unit_id,
                    "textRevision": unit.text_revision,
                    "sourceText": unit.source_text,
                    "sourceHash": unit.source_hash,
                    "sourceRefs": source_refs,
                }),
                _ => panic!("body query returned another dataset"),
            })
            .collect(),
    )
}
fn assert_refresh_body_range(
    before: &SessionRecordPage,
    after: &SessionRecordPage,
    newer_count: &str,
) {
    assert_eq!(refresh_body_identity(after), refresh_body_identity(before));
    assert_eq!(after.next_cursor, before.next_cursor);
    assert_eq!(after.loaded_range.first, before.loaded_range.first);
    assert_eq!(after.loaded_range.last, before.loaded_range.last);
    assert_eq!(after.snapshot.snapshot_id, before.snapshot.snapshot_id);
    assert_eq!(
        after.snapshot.refresh_cursor,
        before.snapshot.refresh_cursor
    );
    assert_eq!(
        after.snapshot.high_watermark,
        before.snapshot.high_watermark
    );
    assert_eq!(
        after.snapshot.matching_count,
        before.snapshot.matching_count
    );
    assert_eq!(
        after.snapshot.database_revision,
        before.snapshot.database_revision
    );
    assert_eq!(after.snapshot.newer_count, newer_count);
}
fn refresh_body_units(page: &SessionRecordPage) -> Vec<&ReadingUnit> {
    page.records
        .iter()
        .map(|record| match record {
            SessionRecord::Body { unit, .. } => unit,
            _ => panic!("body query returned another dataset"),
        })
        .collect()
}

#[test]
fn frozen_body_pages_read_late_translations_and_latest_corrections_without_reordering() {
    let f = Fixture::new();
    configure_refresh_translation(&f);
    refresh_select_source(&f, &f.session, 20);
    let body_count = 160_u64;
    for n in 1..=body_count {
        refresh_ingest_body(
            &f,
            &f.session,
            n + 1,
            &format!("固定範囲の本文第{n}句。"),
            100 + n as i64,
        );
    }
    let mut q = query(&f.session, SessionDataset::Body);
    q.order = SessionRecordOrder::NewestFirst;
    let first = f.store.session_records(q.clone(), 400).unwrap();
    assert_eq!(first.snapshot.matching_count, body_count.to_string());
    assert_eq!(first.snapshot.newer_count, "0");
    // Retain only opaque, actually issued cursors. Replaying the first page
    // uses its refresh cursor rather than creating a new range with None.
    let mut first_query = q.clone();
    first_query.cursor = Some(first.snapshot.refresh_cursor.clone());
    let mut pages = vec![(first_query, first)];
    while let Some(cursor) = pages.last().unwrap().1.next_cursor.clone() {
        let mut older_query = q.clone();
        older_query.cursor = Some(cursor);
        let older = f.store.session_records(older_query.clone(), 401).unwrap();
        pages.push((older_query, older));
    }
    assert_eq!(
        pages.len(),
        body_count.div_ceil(q.page_size as u64) as usize
    );
    let identities: std::collections::BTreeSet<_> = pages
        .iter()
        .flat_map(|(_, p)| refresh_body_units(p).into_iter().map(|u| u.unit_id.clone()))
        .collect();
    assert_eq!(identities.len(), body_count as usize);
    assert!(pages.iter().all(|(_, p)| {
        refresh_body_units(p)
            .iter()
            .all(|u| u.translations.is_empty())
    }));

    // One pending model request in every loaded page, including the oldest
    // page outside the snapshot/history cache. No SQL rows are seeded by hand.
    let tasks: Vec<_> = pages
        .iter()
        .enumerate()
        .map(|(index, (_, page))| {
            let unit = refresh_body_units(page)[0];
            let prepared = f
                .store
                .prepare_tasks_for_operation(
                    unit.unit_id.clone(),
                    unit.text_revision,
                    "translation".into(),
                    format!("loaded-page-translation-{index}"),
                    500 + index as i64,
                )
                .unwrap();
            assert_eq!(prepared.len(), 1);
            assert!(prepared[0].cached_result.is_none());
            prepared.into_iter().next().unwrap()
        })
        .collect();
    for (query, before) in &pages {
        let pending = f.store.session_records(query.clone(), 510).unwrap();
        assert_refresh_body_range(before, &pending, "0");
        assert!(
            refresh_body_units(&pending)
                .iter()
                .all(|u| u.translations.is_empty())
        );
    }
    for (index, task) in tasks.iter().enumerate() {
        refresh_complete_translation(
            &f,
            task,
            &format!("已完成的真实持久译文{index}"),
            600 + index as i64,
        );
    }
    for ((query, before), task) in pages.iter().zip(&tasks) {
        let completed = f.store.session_records(query.clone(), 610).unwrap();
        assert_refresh_body_range(before, &completed, "0");
        let unit = refresh_body_units(&completed)
            .into_iter()
            .find(|u| u.unit_id == task.unit.unit_id)
            .unwrap();
        let stored = f
            .store
            .unit(task.unit.unit_id.clone(), task.unit.text_revision)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&unit.translations).unwrap(),
            serde_json::to_value(&stored.translations).unwrap()
        );
        assert_eq!(unit.translations.len(), 1);
        assert!(unit.translations[0].correction.is_none());
    }
    for (index, task) in tasks.iter().enumerate() {
        let unit = f
            .store
            .unit(task.unit.unit_id.clone(), task.unit.text_revision)
            .unwrap();
        let translation_id = unit.translations[0].translation_id.clone();
        for (revision, text) in [format!("第一版校对{index}"), format!("第二版校对{index}")]
            .into_iter()
            .enumerate()
        {
            refresh_command(
                &f,
                Command::SaveCorrection {
                    translation_id: translation_id.clone(),
                    base_revision: revision as u32,
                    text,
                },
                700 + (index * 2 + revision) as i64,
            );
        }
    }
    for ((query, before), (index, task)) in pages.iter().zip(tasks.iter().enumerate()) {
        let corrected = f.store.session_records(query.clone(), 720).unwrap();
        assert_refresh_body_range(before, &corrected, "0");
        let unit = refresh_body_units(&corrected)
            .into_iter()
            .find(|u| u.unit_id == task.unit.unit_id)
            .unwrap();
        let translation = &unit.translations[0];
        assert_eq!(translation.text, format!("已完成的真实持久译文{index}"));
        let correction = translation.correction.as_ref().unwrap();
        assert_eq!(correction.revision, 2);
        assert_eq!(correction.text, format!("第二版校对{index}"));
        assert!(
            refresh_body_units(&corrected)
                .iter()
                .filter(|u| u.unit_id != task.unit.unit_id)
                .all(|u| u.translations.is_empty())
        );
    }
    // This frozen revision intentionally stays old while the actual DB has
    // committed results. It is not an "unchanged record content" token.
    assert!(
        f.store.snapshot().unwrap().commit_revision
            > pages[0]
                .1
                .snapshot
                .database_revision
                .parse::<u32>()
                .unwrap()
    );
}

#[test]
fn frozen_body_cursor_counts_new_body_but_isolates_pause_late_session_and_results() {
    let f = Fixture::new();
    configure_refresh_translation(&f);
    refresh_select_source(&f, &f.session, 20);
    let mut units = Vec::new();
    for n in 1..=3 {
        units.push(refresh_ingest_body(
            &f,
            &f.session,
            n + 1,
            &format!("会話Aの本文{n}。"),
            100 + n as i64,
        ));
    }
    let task = f
        .store
        .prepare_tasks_for_operation(
            units[0].unit_id.clone(),
            units[0].text_revision,
            "translation".into(),
            "late-session-result".into(),
            200,
        )
        .unwrap()
        .remove(0);
    let mut q = query(&f.session, SessionDataset::Body);
    q.order = SessionRecordOrder::NewestFirst;
    q.page_size = 2;
    let first = f.store.session_records(q.clone(), 300).unwrap();
    let mut first_query = q.clone();
    first_query.cursor = Some(first.snapshot.refresh_cursor.clone());
    let mut older_query = q.clone();
    older_query.cursor = first.next_cursor.clone();
    let older = f.store.session_records(older_query.clone(), 301).unwrap();
    assert!(older.next_cursor.is_none());

    refresh_command(&f, Command::SetCapturePaused { paused: true }, 400);
    let paused = f
        .store
        .ingest_hook_text_with_policy(raw(&f.session, 5, "暂停期间不形成正文。"), false, 401)
        .unwrap();
    assert!(paused.visible_version.is_none());
    assert_refresh_body_range(
        &first,
        &f.store.session_records(first_query.clone(), 402).unwrap(),
        "0",
    );
    assert_refresh_body_range(
        &older,
        &f.store.session_records(older_query.clone(), 403).unwrap(),
        "0",
    );
    refresh_command(&f, Command::SetCapturePaused { paused: false }, 500);
    let new = refresh_ingest_body(&f, &f.session, 6, "会話Aの新しい本文。", 501);
    assert_refresh_body_range(
        &first,
        &f.store.session_records(first_query.clone(), 502).unwrap(),
        "1",
    );
    assert_refresh_body_range(
        &older,
        &f.store.session_records(older_query.clone(), 503).unwrap(),
        "1",
    );

    let second = "session-b";
    f.store
        .open_hook_session(hook_session(second), 600)
        .unwrap();
    refresh_select_source(&f, second, 601);
    let foreign = refresh_ingest_body(&f, second, 2, "別の会話Bの本文。", 603);
    let late_input = raw(&f.session, 7, "会話Aの遅いコールバック。");
    // The strict admission port rejects a terminal session. Runtime's
    // already-received callback port still records that fact without body.
    assert_eq!(
        f.store
            .ingest_hook_text(late_input.clone(), 604)
            .unwrap_err()
            .code,
        "stale_capture_session"
    );
    let late_raw = f
        .store
        .ingest_hook_text_with_policy(late_input, true, 605)
        .unwrap();
    assert!(late_raw.visible_version.is_none());
    assert!(late_raw.finalized_versions.is_empty());
    refresh_complete_translation(&f, &task, "迟到结果只属于会话A旧句", 610);
    for (request, before) in [(&first_query, &first), (&older_query, &older)] {
        let refreshed = f.store.session_records(request.clone(), 620).unwrap();
        assert_refresh_body_range(before, &refreshed, "1");
        assert!(
            refresh_body_units(&refreshed)
                .iter()
                .all(|u| u.session_id == f.session
                    && u.unit_id != new.unit_id
                    && u.unit_id != foreign.unit_id)
        );
    }
    let old_page = f.store.session_records(older_query, 621).unwrap();
    let translated = refresh_body_units(&old_page)
        .into_iter()
        .find(|u| u.unit_id == task.unit.unit_id)
        .unwrap();
    assert_eq!(translated.translations.len(), 1);
    assert_eq!(translated.translations[0].text, "迟到结果只属于会话A旧句");
    let foreign_page = f
        .store
        .session_records(query(second, SessionDataset::Body), 622)
        .unwrap();
    let foreign_units = refresh_body_units(&foreign_page);
    assert_eq!(foreign_units.len(), 1);
    assert_eq!(foreign_units[0].unit_id, foreign.unit_id);
    assert!(foreign_units[0].translations.is_empty());
    let mut wrong_session = first_query.clone();
    wrong_session.session_id = second.into();
    assert_eq!(
        f.store
            .session_records(wrong_session, 623)
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    let mut wrong_dataset = first_query;
    wrong_dataset.dataset = SessionDataset::Extraction;
    assert_eq!(
        f.store
            .session_records(wrong_dataset, 624)
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    let current = f.store.session_records(q, 625).unwrap();
    assert_eq!(
        current.snapshot.matching_count,
        (units.len() + 1).to_string()
    );
    assert_eq!(current.snapshot.newer_count, "0");
    assert!(
        refresh_body_units(&current)
            .iter()
            .any(|u| u.unit_id == new.unit_id)
    );
    // A body refresh must not fabricate or discard the paused/late raw facts.
    let extraction = f
        .store
        .session_records(query(&f.session, SessionDataset::Extraction), 626)
        .unwrap();
    assert!(extraction.records.iter().any(|r| matches!(r, SessionRecord::Extraction { reason, text, body_refs, .. } if reason == "text_paused" && text.as_deref() == Some("暂停期间不形成正文。") && body_refs.is_empty())));
    assert!(extraction.records.iter().any(|r| matches!(r, SessionRecord::Extraction { reason, text, body_refs, .. } if reason == "text_inactive" && text.as_deref() == Some("会話Aの遅いコールバック。") && body_refs.is_empty())));
}
