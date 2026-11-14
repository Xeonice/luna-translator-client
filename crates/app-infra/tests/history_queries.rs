//! Real Repository pagination and filtering. Cursors are opaque: tests only round-trip or misuse returned values.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
    ports::ProductRepository,
};
use app_infra::storage::SqliteStore;
use rusqlite::params;
use std::{collections::BTreeSet, path::PathBuf};

struct Database {
    store: SqliteStore,
    path: PathBuf,
}
impl Database {
    fn new(profile: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "luna-history-query-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        Self {
            store: SqliteStore::open(&path, profile, "history-test-server", 1).unwrap(),
            path,
        }
    }
    fn command(&self, command: Command, now: i64) -> CommandReceipt {
        self.store
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
    fn manual(&self, text: &str, now: i64) -> ReadingUnit {
        let id = self
            .command(
                Command::ManualInput {
                    text: text.into(),
                    source_language: "ja".into(),
                },
                now,
            )
            .result_id
            .unwrap();
        self.store.unit(id, 1).unwrap()
    }
    fn game(&self, name: &str) -> String {
        let input:GameInput=serde_json::from_value(serde_json::json!({"displayName":name,"sourceLanguage":"ja","target":{"environmentKind":"crossover","environmentKey":format!("crossover:/private/{name}"),"bottleLocator":format!("/private/{name}"),"launchPath":format!("C:\\{name}.exe"),"canonicalExePath":format!("C:\\{name}.exe"),"targetPathKey":format!("c:/{name}.exe"),"arguments":[],"architecture":"x86","verificationState":"verified"}})).unwrap();
        self.command(Command::SaveGame(Box::new(input)), 2)
            .result_id
            .unwrap()
    }
    fn hook(&self, game: Option<&str>, name: &str, now: i64) -> String {
        let session = format!("history-session-{name}");
        let library_target = game.map(|id| self.store.snapshot().unwrap().games.into_iter()
            .find(|record| record.game_id == id).expect("history fixture game must exist").target);
        self.store
            .open_hook_session(
                HookSessionInput {
                    session_id: session.clone(),
                    game_id: game.map(str::to_owned),
                    environment_key: library_target.as_ref().map(|target| target.environment_key.clone())
                        .unwrap_or_else(|| format!("crossover:/private/{name}")),
                    target_exe_path: library_target.as_ref().map(|target| target.canonical_exe_path.clone()
                        .expect("history fixture game must have a full target path"))
                        .unwrap_or_else(|| format!("C:\\{name}.exe")),
                    windows_pid: 100,
                    process_created_u64: "100".into(),
                    architecture: "x86".into(),
                    broker_instance_id: format!("broker-{name}"),
                    resource: ResourceSnapshotInput {
                        snapshot_id: "controlled-history-core".into(),
                        kind: "core".into(),
                        version: "test-input-only".into(),
                        manifest_sha256: content_hash("history-core"),
                        contract_version: "1".into(),
                        metadata_json: "{}".into(),
                    },
                    target_identity_sha256: content_hash(name),
                },
                now,
            )
            .unwrap();
        session
    }
    fn event(&self, session: &str, sequence: u32, text: &str, now: i64) -> Option<String> {
        self.store
            .receive_hook_text(
                HookTextInput {
                    session_id: session.into(),
                    event_id: format!("{session}-event-{sequence}"),
                    sequence_u64: sequence.to_string(),
                    source: HookSourceInput {
                        source_id: format!("{session}-source"),
                        host_instance_id: format!("{session}-host"),
                        hook_address_u64: "10".into(),
                        context_u64: "0".into(),
                        subcontext_u64: "0".into(),
                        hook_code: "HW".into(),
                        hook_name: "TextOutW".into(),
                        encoding: "utf-16le".into(),
                        stream_identity_sha256: content_hash(session),
                        identity_extensions_json: "{}".into(),
                    },
                    text: text.into(),
                    source_language: "ja".into(),
                },
                now,
            )
            .unwrap()
    }
    fn hook_unit(&self, game: Option<&str>, name: &str, now: i64) -> ReadingUnit {
        let session = self.hook(game, name, now);
        assert!(self.event(&session, 1, "候補", now).is_none());
        self.command(
            Command::SelectSource {
                source_id: format!("{session}-source"),
            },
            now,
        );
        let id = self
            .event(&session, 2, &format!("{name}の本文。"), now)
            .unwrap();
        self.store.unit(id, 1).unwrap()
    }
    /// There is no public text-edit command. Seed an additional legal immutable
    /// processor revision in this real candidate DB, then query it only through
    /// the production Repository. No query/cursor implementation is replaced.
    fn additional_revision(
        &self,
        original: &ReadingUnit,
        revision: u32,
        text: &str,
        now: i64,
    ) -> ReadingUnit {
        let conn = rusqlite::Connection::open(&self.path).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn.execute("INSERT INTO reading_unit_revisions(unit_id,text_revision,session_id,source_text,source_byte_length,source_sha256,source_language,processing_revision,processing_snapshot_json,selection_revision,created_at_ms) SELECT unit_id,?2,session_id,?3,?4,?5,source_language,'history-fixture-revision','{}',selection_revision,?6 FROM reading_unit_revisions WHERE unit_id=?1 AND text_revision=1",params![original.unit_id,revision,text,text.len() as i64,content_hash(text),now]).unwrap();
        self.store.unit(original.unit_id.clone(), revision).unwrap()
    }
}
fn page(
    store: &SqliteStore,
    filter: HistoryFilter,
    cursor: Option<String>,
    limit: u32,
) -> HistoryPage {
    store
        .history_page(HistoryQuery {
            filter,
            cursor,
            limit,
        })
        .unwrap()
}
fn sessions(
    store: &SqliteStore,
    filter: HistorySessionFilter,
    cursor: Option<String>,
    limit: u32,
) -> HistorySessionsPage {
    store
        .history_sessions(HistorySessionsQuery {
            filter,
            cursor,
            limit,
        })
        .unwrap()
}
fn key(unit: &ReadingUnit) -> (i64, String, u32) {
    (
        unit.created_at_ms as i64,
        unit.unit_id.clone(),
        unit.text_revision,
    )
}
fn all_pages(store: &SqliteStore, filter: HistoryFilter, limit: u32) -> Vec<ReadingUnit> {
    let mut result = vec![];
    let mut cursor = None;
    let mut seen = BTreeSet::new();
    loop {
        let next = page(store, filter.clone(), cursor, limit);
        result.extend(next.units);
        match next.next_cursor {
            Some(value) => {
                assert!(seen.insert(value.clone()), "cursor failed to advance");
                cursor = Some(value)
            }
            None => break,
        }
    }
    result
}
#[test]
fn filters_compose_across_games_source_kinds_and_exact_sessions() {
    let db = Database::new("history-filters");
    let a = db.game("history-a");
    let b = db.game("history-b");
    let a1 = db.hook_unit(Some(&a), "a-one", 10);
    let a2 = db.hook_unit(Some(&a), "a-two", 20);
    let b1 = db.hook_unit(Some(&b), "b-one", 30);
    let unbound = db.hook_unit(None, "unbound", 40);
    let manual = db.manual("手入力の本文。", 50);
    assert_eq!(all_pages(&db.store, HistoryFilter::default(), 2).len(), 5);
    let game = all_pages(
        &db.store,
        HistoryFilter {
            game_id: Some(a.clone()),
            ..Default::default()
        },
        1,
    );
    assert_eq!(
        game.iter()
            .map(|u| u.unit_id.clone())
            .collect::<BTreeSet<_>>(),
        [a1.unit_id.clone(), a2.unit_id.clone()]
            .into_iter()
            .collect()
    );
    let hook = all_pages(
        &db.store,
        HistoryFilter {
            source_kind: Some(HistorySourceKind::Hook),
            ..Default::default()
        },
        2,
    );
    assert_eq!(hook.len(), 4);
    assert!(hook.iter().any(|u| u.unit_id == unbound.unit_id));
    let manual_page = page(
        &db.store,
        HistoryFilter {
            source_kind: Some(HistorySourceKind::Manual),
            ..Default::default()
        },
        None,
        10,
    );
    assert_eq!(manual_page.units[0].unit_id, manual.unit_id);
    assert_eq!(manual_page.units.len(), 1);
    let exact = page(
        &db.store,
        HistoryFilter {
            game_id: Some(a),
            source_kind: Some(HistorySourceKind::Hook),
            session_id: Some(a1.session_id.clone()),
        },
        None,
        10,
    );
    assert_eq!(exact.units.len(), 1);
    assert_eq!(exact.units[0].unit_id, a1.unit_id);
    let impossible = page(
        &db.store,
        HistoryFilter {
            game_id: Some(b),
            source_kind: Some(HistorySourceKind::Hook),
            session_id: Some(a1.session_id),
        },
        None,
        10,
    );
    assert!(impossible.units.is_empty());
    assert!(impossible.next_cursor.is_none());
    assert!(
        page(
            &db.store,
            HistoryFilter {
                game_id: Some("unknown-game".into()),
                ..Default::default()
            },
            None,
            10
        )
        .units
        .is_empty()
    );
    assert!(hook.iter().any(|u| u.unit_id == b1.unit_id));
}
#[test]
fn keyset_handles_equal_timestamps_multiple_revisions_and_concurrent_newer_inserts() {
    let db = Database::new("history-keyset");
    let mut expected = Vec::new();
    for i in 0..7 {
        expected.push(db.manual(&format!("同じ時刻の文{i}。"), 100));
    }
    let revision = db.additional_revision(&expected[2], 2, "同一単位の改訂版。", 100);
    expected.push(revision);
    let revision = db.additional_revision(&expected[4], 3, "同一単位の別改訂版。", 100);
    expected.push(revision);
    expected.sort_by_key(|u| std::cmp::Reverse(key(u)));
    let first = page(&db.store, HistoryFilter::default(), None, 2);
    assert_eq!(
        first.units.iter().map(key).collect::<Vec<_>>(),
        expected[..2].iter().map(key).collect::<Vec<_>>()
    );
    let inserted = db.manual("ページを開いた後の新着。", 101);
    let mut rest = first.units;
    let mut cursor = first.next_cursor;
    while let Some(value) = cursor {
        let next = page(&db.store, HistoryFilter::default(), Some(value), 2);
        rest.extend(next.units);
        cursor = next.next_cursor;
    }
    assert_eq!(
        rest.iter().map(key).collect::<Vec<_>>(),
        expected.iter().map(key).collect::<Vec<_>>()
    );
    assert!(!rest.iter().any(|u| u.unit_id == inserted.unit_id));
    assert_eq!(
        rest.iter().map(key).collect::<BTreeSet<_>>().len(),
        expected.len()
    );
    acceptance_evidence::record(
        "keyset-paging-concurrent-revisions",
        &[("STORAGE-14", 1)],
        serde_json::json!({"actual_test":"keyset_handles_equal_timestamps_multiple_revisions_and_concurrent_newer_inserts"}),
    );
}
#[test]
fn cursor_is_bound_to_profile_filter_and_query_kind_without_dependence_on_encoding() {
    let db = Database::new("history-cursor-a");
    let other = Database::new("history-cursor-b");
    for text in ["一。", "二。", "三。"] {
        db.manual(text, 100);
        other.manual(text, 100);
    }
    let units = page(&db.store, HistoryFilter::default(), None, 1)
        .next_cursor
        .unwrap();
    let directory = sessions(&db.store, HistorySessionFilter::default(), None, 1)
        .next_cursor
        .unwrap();
    let query = |filter, cursor| HistoryQuery {
        filter,
        cursor: Some(cursor),
        limit: 1,
    };
    assert_eq!(
        other
            .store
            .history_page(query(HistoryFilter::default(), units.clone()))
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(
        db.store
            .history_page(query(
                HistoryFilter {
                    source_kind: Some(HistorySourceKind::Manual),
                    ..Default::default()
                },
                units.clone()
            ))
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(
        db.store
            .history_page(query(
                HistoryFilter {
                    game_id: Some("another-game".into()),
                    ..Default::default()
                },
                units.clone()
            ))
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(
        db.store
            .history_page(query(
                HistoryFilter {
                    session_id: Some("another-session".into()),
                    ..Default::default()
                },
                units.clone()
            ))
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(
        db.store
            .history_page(query(HistoryFilter::default(), directory.clone()))
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(
        db.store
            .history_sessions(HistorySessionsQuery {
                filter: HistorySessionFilter::default(),
                cursor: Some(units.clone()),
                limit: 1
            })
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(
        other
            .store
            .history_sessions(HistorySessionsQuery {
                filter: HistorySessionFilter::default(),
                cursor: Some(directory.clone()),
                limit: 1
            })
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(
        db.store
            .history_sessions(HistorySessionsQuery {
                filter: HistorySessionFilter {
                    source_kind: Some(HistorySourceKind::Manual),
                    ..Default::default()
                },
                cursor: Some(directory.clone()),
                limit: 1
            })
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(
        db.store
            .history_sessions(HistorySessionsQuery {
                filter: HistorySessionFilter {
                    game_id: Some("another-game".into()),
                    ..Default::default()
                },
                cursor: Some(directory),
                limit: 1
            })
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    // Page size is not filter scope: changing it must remain a valid continuation.
    assert_eq!(
        page(&db.store, HistoryFilter::default(), Some(units), 10)
            .units
            .len(),
        2
    );
}
#[test]
fn malformed_oversized_cursors_and_invalid_identity_filters_fail_explicitly() {
    let db = Database::new("history-invalid");
    db.manual("保存済み。", 100);
    for value in [
        "".to_owned(),
        "null".into(),
        "[]".into(),
        "{}".into(),
        "not a cursor".into(),
        "0".into(),
        "{\"kind\":\"unknown\"}".into(),
        " ".repeat(65_537),
    ] {
        assert_eq!(
            db.store
                .history_page(HistoryQuery {
                    filter: HistoryFilter::default(),
                    cursor: Some(value.clone()),
                    limit: 10
                })
                .unwrap_err()
                .code,
            "invalid_cursor"
        );
        assert_eq!(
            db.store
                .history_sessions(HistorySessionsQuery {
                    filter: HistorySessionFilter::default(),
                    cursor: Some(value),
                    limit: 10
                })
                .unwrap_err()
                .code,
            "invalid_cursor"
        );
    }
    for filter in [
        HistoryFilter {
            game_id: Some("".into()),
            ..Default::default()
        },
        HistoryFilter {
            session_id: Some("bad\0id".into()),
            ..Default::default()
        },
    ] {
        assert_eq!(
            db.store
                .history_page(HistoryQuery {
                    filter,
                    cursor: None,
                    limit: 1
                })
                .unwrap_err()
                .code,
            "invalid_id"
        );
    }
    assert_eq!(
        db.store
            .history_sessions(HistorySessionsQuery {
                filter: HistorySessionFilter {
                    game_id: Some("bad\nid".into()),
                    ..Default::default()
                },
                cursor: None,
                limit: 1
            })
            .unwrap_err()
            .code,
        "invalid_id"
    );
}
#[test]
fn session_directory_excludes_empty_hooks_pages_equal_times_and_filters_actual_reading_sources() {
    let db = Database::new("history-directory");
    let a = db.game("directory-a");
    let b = db.game("directory-b");
    let ua = db.hook_unit(Some(&a), "directory-hook-a", 100);
    let ub = db.hook_unit(Some(&b), "directory-hook-b", 100);
    let manual = db.manual("手動セッション。", 100);
    let empty = db.hook(None, "empty-hook", 100);
    let mut cursor = None;
    let mut found = vec![];
    loop {
        let next = sessions(&db.store, HistorySessionFilter::default(), cursor, 1);
        found.extend(next.sessions);
        cursor = next.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    let mut ids = vec![
        ua.session_id.clone(),
        ub.session_id.clone(),
        manual.session_id.clone(),
    ];
    ids.sort_by(|a, b| b.cmp(a));
    assert_eq!(
        found
            .iter()
            .map(|s| s.session_id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    assert!(!found.iter().any(|s| s.session_id == empty));
    assert!(found.iter().all(|s| s.started_at_ms == 100.));
    let hook = sessions(
        &db.store,
        HistorySessionFilter {
            source_kind: Some(HistorySourceKind::Hook),
            ..Default::default()
        },
        None,
        100,
    );
    assert_eq!(hook.sessions.len(), 2);
    let manual_only = sessions(
        &db.store,
        HistorySessionFilter {
            source_kind: Some(HistorySourceKind::Manual),
            ..Default::default()
        },
        None,
        100,
    );
    assert_eq!(manual_only.sessions.len(), 1);
    assert_eq!(manual_only.sessions[0].session_id, manual.session_id);
    let game = sessions(
        &db.store,
        HistorySessionFilter {
            game_id: Some(a),
            source_kind: Some(HistorySourceKind::Hook),
        },
        None,
        100,
    );
    assert_eq!(game.sessions.len(), 1);
    assert_eq!(game.sessions[0].session_id, ua.session_id);
    let impossible = sessions(
        &db.store,
        HistorySessionFilter {
            game_id: Some(b),
            source_kind: Some(HistorySourceKind::Manual),
        },
        None,
        100,
    );
    assert!(impossible.sessions.is_empty());
    assert!(impossible.next_cursor.is_none());
}
#[test]
fn repository_pages_are_bounded_and_limit_plus_one_does_not_emit_spurious_cursor() {
    let db = Database::new("history-bounds");
    for i in 0..103 {
        db.manual(&format!("有界履歴{i}。"), 100);
    }
    let first = page(&db.store, HistoryFilter::default(), None, u32::MAX);
    assert_eq!(first.units.len(), 100);
    let next = page(&db.store, HistoryFilter::default(), first.next_cursor, 100);
    assert_eq!(next.units.len(), 3);
    assert!(next.next_cursor.is_none());
    assert_eq!(
        page(&db.store, HistoryFilter::default(), None, 0)
            .units
            .len(),
        1
    );
    let directory = sessions(&db.store, HistorySessionFilter::default(), None, u32::MAX);
    assert_eq!(directory.sessions.len(), 100);
    let next = sessions(
        &db.store,
        HistorySessionFilter::default(),
        directory.next_cursor,
        100,
    );
    assert_eq!(next.sessions.len(), 3);
    assert!(next.next_cursor.is_none());
    assert_eq!(
        sessions(&db.store, HistorySessionFilter::default(), None, 0)
            .sessions
            .len(),
        1
    );
    let exact = Database::new("history-exact");
    for text in ["一。", "二。"] {
        exact.manual(text, 1);
    }
    assert!(
        page(&exact.store, HistoryFilter::default(), None, 2)
            .next_cursor
            .is_none()
    );
    assert!(
        sessions(&exact.store, HistorySessionFilter::default(), None, 2)
            .next_cursor
            .is_none()
    );
}
#[test]
fn legacy_history_continues_old_cursor_semantics_for_internal_context_callers() {
    let db = Database::new("history-legacy");
    for text in ["一。", "二。", "三。"] {
        db.manual(text, 100);
    }
    let first = db.store.history(None, None, 1).unwrap();
    let legacy_cursor = first.next_cursor.unwrap();
    let next = db
        .store
        .history(None, Some(legacy_cursor.clone()), 10)
        .unwrap();
    assert_eq!(next.units.len(), 2);
    assert_eq!(
        db.store
            .history_page(HistoryQuery {
                filter: HistoryFilter::default(),
                cursor: Some(legacy_cursor),
                limit: 1
            })
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    let new_cursor = page(&db.store, HistoryFilter::default(), None, 1)
        .next_cursor
        .unwrap();
    assert_eq!(
        db.store
            .history(None, Some(new_cursor), 10)
            .unwrap_err()
            .code,
        "invalid_cursor"
    );
    assert_eq!(db.store.snapshot().unwrap().history.units.len(), 3);
}

#[test]
fn every_record_and_detail_carries_authoritative_source_beyond_first_fifty_sessions() {
    let db = Database::new("history-source-summary");
    let old_manual = db.manual("古い手入力。", 10);
    let old_unbound_hook = db.hook_unit(None, "older-unbound-hook", 20);
    db.command(Command::Disconnect, 30);
    let game = db.game("source-summary-game");
    let old_game_hook = db.hook_unit(Some(&game), "older-game-hook", 40);
    db.command(Command::Disconnect, 41);
    for i in 0..55 {
        db.manual(&format!("新しい会話{i}。"), 100 + i);
    }
    let directory = sessions(&db.store, HistorySessionFilter::default(), None, 50);
    assert_eq!(directory.sessions.len(), 50);
    assert!(directory.next_cursor.is_some());
    assert!(
        !directory
            .sessions
            .iter()
            .any(|s| s.session_id == old_manual.session_id
                || s.session_id == old_unbound_hook.session_id)
    );
    let history = page(&db.store, HistoryFilter::default(), None, 100);
    assert_eq!(history.units.len(), 58);
    assert!(history.units.iter().all(|u| {
        u.source_summary
            .as_ref()
            .is_some_and(|s| s.session_id == u.session_id && s.game_id == u.game_id)
    }));
    for (original, kind, started, ended) in [
        (&old_manual, HistorySourceKind::Manual, 10., None),
        (&old_unbound_hook, HistorySourceKind::Hook, 20., Some(30.)),
        (&old_game_hook, HistorySourceKind::Hook, 40., Some(41.)),
    ] {
        let listed = history
            .units
            .iter()
            .find(|u| u.unit_id == original.unit_id)
            .unwrap();
        let detail = db
            .store
            .unit(original.unit_id.clone(), original.text_revision)
            .unwrap();
        let summary = listed.source_summary.as_ref().unwrap();
        assert_eq!(detail.source_summary.as_ref(), Some(summary));
        assert_eq!(summary.source_kind, kind);
        assert_eq!(summary.game_id, original.game_id);
        assert_eq!(summary.started_at_ms, started);
        assert_eq!(summary.ended_at_ms, ended);
        assert_eq!(
            summary.state,
            if kind == HistorySourceKind::Hook {
                "stopped"
            } else {
                "active"
            }
        );
    }
    let manual = history
        .units
        .iter()
        .find(|u| u.unit_id == old_manual.unit_id)
        .unwrap();
    let hook = history
        .units
        .iter()
        .find(|u| u.unit_id == old_unbound_hook.unit_id)
        .unwrap();
    assert!(manual.game_id.is_none() && hook.game_id.is_none());
    assert_ne!(
        manual.source_summary.as_ref().unwrap().source_kind,
        hook.source_summary.as_ref().unwrap().source_kind
    );
}

#[test]
fn older_serialized_reading_units_without_source_summary_remain_readable() {
    let db = Database::new("history-legacy-source-summary");
    let unit = db.manual("既存の保存内容。", 10);
    let mut old = serde_json::to_value(&unit).unwrap();
    assert!(
        old.as_object_mut()
            .unwrap()
            .remove("sourceSummary")
            .is_some()
    );
    let restored: ReadingUnit = serde_json::from_value(old.clone()).unwrap();
    assert!(restored.source_summary.is_none());
    assert_eq!(restored.source_text, unit.source_text);
    assert_eq!(restored.session_id, unit.session_id);
    let frozen: LearningObject =
        serde_json::from_value(serde_json::json!({"unit":old,"tokenIndex":null})).unwrap();
    assert!(frozen.unit.source_summary.is_none());
    assert_eq!(frozen.unit.unit_id, unit.unit_id);
}

#[test]
fn current_projection_waits_for_the_running_hook_session_body() {
    let db = Database::new("current-waiting");
    let previous_game = db.game("current-previous-game");
    let next_game = db.game("current-next-game");
    assert_ne!(previous_game, next_game);
    let old = db.hook_unit(Some(&previous_game), "previous-game", 10);
    let manual = db.manual("以前の手入力。", 20);
    let session = db.hook(Some(&next_game), "new-game", 30);
    let opening = db.store.snapshot().unwrap();
    assert_eq!(opening.session.as_ref().unwrap().state, "opening");
    assert!(opening.current.is_none());
    assert!(
        opening
            .history
            .units
            .iter()
            .any(|u| u.unit_id == old.unit_id)
    );
    assert!(
        opening
            .history
            .units
            .iter()
            .any(|u| u.unit_id == manual.unit_id)
    );

    assert!(db.event(&session, 1, "未選択の候補。", 31).is_none());
    let active = db.store.snapshot().unwrap();
    assert_eq!(active.session.as_ref().unwrap().state, "active");
    assert!(active.current.is_none());
    db.command(
        Command::SelectSource {
            source_id: format!("{session}-source"),
        },
        32,
    );
    assert!(db.store.snapshot().unwrap().current.is_none());

    let id = db.event(&session, 2, "この会話の本文。", 33).unwrap();
    let current = db.store.snapshot().unwrap().current.unwrap();
    assert_eq!(current.session_id, session);
    assert_eq!(current.game_id.as_deref(), Some(next_game.as_str()));
    assert_eq!(current.unit_id, id);
    assert_eq!(current.source_text, "この会話の本文。");
}

#[test]
fn current_projection_uses_exact_session_beyond_the_global_history_page() {
    let db = Database::new("current-independent-page");
    let current = db.hook_unit(None, "still-running", 10);
    for index in 0..55 {
        let manual = db.manual(&format!("別の手入力{index}。"), 100 + index);
        assert_eq!(db.store.unit(manual.unit_id.clone(), 1).unwrap(), manual);
    }
    let snapshot = db.store.snapshot().unwrap();
    assert_eq!(snapshot.history.units.len(), 50);
    assert!(
        !snapshot
            .history
            .units
            .iter()
            .any(|u| u.unit_id == current.unit_id)
    );
    assert_eq!(snapshot.current.as_ref().unwrap().unit_id, current.unit_id);
    assert_eq!(
        snapshot.current.as_ref().unwrap().session_id,
        current.session_id
    );
    assert!(snapshot.history.next_cursor.is_some());
    assert_eq!(all_pages(&db.store, HistoryFilter::default(), 10).len(), 56);

    let updated = db.event(&current.session_id, 3, "次の本文。", 200).unwrap();
    let snapshot = db.store.snapshot().unwrap();
    assert_eq!(snapshot.current.as_ref().unwrap().unit_id, updated);
    assert_eq!(snapshot.current.as_ref().unwrap().source_text, "次の本文。");
}

#[test]
fn current_projection_keeps_manual_input_and_disconnected_restore_semantics() {
    let db = Database::new("current-manual-restore");
    assert!(db.store.snapshot().unwrap().current.is_none());
    let first = db.manual("ゲームなしの手入力。", 10);
    assert_eq!(db.store.snapshot().unwrap().current.unwrap(), first);
    let hook = db.hook_unit(None, "reading", 20);
    let manual = db.manual("取文中の独立した手入力。", 30);
    assert_eq!(db.store.unit(manual.unit_id.clone(), 1).unwrap(), manual);
    assert_eq!(db.store.snapshot().unwrap().current.unwrap(), hook);
    db.command(Command::Disconnect, 40);
    assert_eq!(
        db.store.snapshot().unwrap().session.unwrap().state,
        "stopped"
    );
    assert_eq!(db.store.snapshot().unwrap().current.unwrap(), manual);
    let later = db.manual("切断後の手入力。", 50);
    assert_eq!(db.store.snapshot().unwrap().current.unwrap(), later);
}

#[test]
fn current_projection_preserves_same_session_last_sentence_across_pause_disconnect_and_reopen() {
    let path;
    let expected;
    {
        let db = Database::new("current-last-sentence");
        let old = db.hook_unit(None, "previous", 10);
        expected = db.hook_unit(None, "current", 20);
        assert_ne!(old.session_id, expected.session_id);
        db.command(Command::SetCapturePaused { paused: true }, 21);
        assert_eq!(
            db.store.snapshot().unwrap().current.as_ref(),
            Some(&expected)
        );
        db.command(Command::Disconnect, 22);
        let snapshot = db.store.snapshot().unwrap();
        assert_eq!(snapshot.current.as_ref().unwrap().unit_id, expected.unit_id);
        assert_eq!(
            snapshot.current.as_ref().unwrap().session_id,
            expected.session_id
        );
        path = db.path;
    }
    let reopened = SqliteStore::open(&path, "current-last-sentence", "reopened", 30).unwrap();
    let snapshot = reopened.snapshot().unwrap();
    assert_eq!(snapshot.current.as_ref().unwrap().unit_id, expected.unit_id);
    assert_eq!(
        snapshot.current.as_ref().unwrap().session_id,
        expected.session_id
    );
    assert_eq!(
        snapshot.current.as_ref().unwrap().source_text,
        expected.source_text
    );
}

#[test]
fn current_projection_uses_the_latest_visible_revision_of_the_current_sentence() {
    let db = Database::new("current-visible-revision");
    let session = db.hook(None, "growing", 10);
    assert!(db.event(&session, 1, "候補", 11).is_none());
    db.command(
        Command::SelectSource {
            source_id: format!("{session}-source"),
        },
        12,
    );
    let unit_id = db.event(&session, 2, "雨", 13).unwrap();
    assert_eq!(
        db.store.snapshot().unwrap().current.unwrap().text_revision,
        1
    );
    assert_eq!(db.event(&session, 3, "雨がやんだ。", 14).unwrap(), unit_id);
    let snapshot = db.store.snapshot().unwrap();
    let current = snapshot.current.unwrap();
    assert_eq!(current.unit_id, unit_id);
    assert_eq!(current.text_revision, 2);
    assert_eq!(current.source_text, "雨がやんだ。");
    assert_eq!(snapshot.history.units.len(), 1);
    assert_eq!(snapshot.history.units[0].text_revision, 2);
    assert_eq!(db.store.unit(unit_id, 1).unwrap().source_text, "雨");
}

#[test]
fn current_projection_failure_and_crash_preserve_existing_global_restore_priority() {
    for body in [false, true] {
        let db = Database::new("current-failed-restore");
        db.manual("以前の手入力。", 10);
        if body {
            db.hook_unit(None, "failed-with-body", 20);
        } else {
            db.hook(None, "failed-without-body", 20);
        }
        let manual = db.manual("独立した手入力。", 30);
        let active = db.store.snapshot().unwrap();
        assert_eq!(active.current.is_some(), body);
        if let Some(current) = active.current {
            assert_eq!(current.session_id, active.session.unwrap().session_id);
        }
        db.store
            .record_diagnostic(
                Diagnostic {
                    code: "capture_failed".into(),
                    stage: "test".into(),
                    message: "controlled terminal transition".into(),
                    recoverable: true,
                },
                40,
            )
            .unwrap();
        let failed = db.store.snapshot().unwrap();
        assert_eq!(failed.session.unwrap().state, "failed");
        assert_eq!(failed.current.unwrap(), manual);
    }

    let path;
    let manual;
    {
        let db = Database::new("current-crash-restore");
        db.hook_unit(None, "crashed", 20);
        manual = db.manual("再起動しても残る手入力。", 30);
        assert_ne!(
            db.store.snapshot().unwrap().current.unwrap().unit_id,
            manual.unit_id
        );
        path = db.path;
    }
    let reopened = SqliteStore::open(&path, "current-crash-restore", "reopened", 40).unwrap();
    let snapshot = reopened.snapshot().unwrap();
    assert_eq!(snapshot.session.unwrap().state, "crashed");
    // Recovery terminates every unclosed input session, including manual
    // sessions. Reading identity/content stay fixed; the source summary must
    // describe the recovered database rather than the pre-restart state.
    let mut recovered_manual = manual;
    let summary = recovered_manual.source_summary.as_mut().unwrap();
    assert_eq!(summary.state, "active");
    assert!(summary.ended_at_ms.is_none());
    summary.state = "crashed".into();
    summary.ended_at_ms = Some(40.0);
    assert_eq!(snapshot.current.unwrap(), recovered_manual);
}

