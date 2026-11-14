//! Real SQLite tests for scope-authoritative queries and per-head CAS.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod evidence;
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
    ports::ProductRepository,
};
use app_infra::storage::SqliteStore;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Barrier},
};
struct Fixture {
    store: SqliteStore,
    path: PathBuf,
    a: String,
    b: String,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "luna-scoped-preferences-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let store = SqliteStore::open(&path, "scope-profile", "scope-server", 1).unwrap();
        let add = |name: &str| {
            store
                .dispatch(
                    CommandEnvelope {
                        operation_id: format!("game-{name}"),
                        expected_revision: None,
                        command: Command::SaveGame(Box::new(GameInput {
                            game_id: None,
                            row_revision: None,
                            display_name: name.into(),
                            original_title: None,
                            source_language: Some("ja".into()),
                            cover_asset_ref: None,
                            target: GameTarget {
                                environment_kind: "crossover".into(),
                                environment_key: format!("crossover:/private/{name}"),
                                bottle_locator: Some(format!("/private/{name}")),
                                launch_path: Some(format!("C:\\{name}.exe")),
                                canonical_exe_path: Some(format!("C:\\{name}.exe")),
                                target_path_key: Some(format!("c:/{name}.exe")),
                                working_directory: None,
                                arguments: vec![],
                                architecture: Some("x86".into()),
                                verification_state: "verified".into(),
                            },
                        })),
                    },
                    2,
                )
                .unwrap()
                .result_id
                .unwrap()
        };
        let a = add("a");
        let b = add("b");
        Self { store, path, a, b }
    }
    fn save(&self, op: &str, p: Preferences, head: Option<u32>) -> AppResult<CommandReceipt> {
        self.store.dispatch(
            CommandEnvelope {
                operation_id: op.into(),
                expected_revision: Some(0),
                command: Command::SaveScopedPreferences {
                    preferences: p,
                    expected_head_revision: head,
                },
            },
            10,
        )
    }
}
#[test]
fn scope_query_distinguishes_missing_saved_head_and_is_read_only() {
    let f = Fixture::new();
    let before = f.store.snapshot().unwrap().commit_revision;
    let global = f.store.preferences_scope("global".into()).unwrap();
    assert!(global.saved.is_none());
    assert!(global.head_revision.is_none());
    assert_eq!(global.effective, Preferences::default());
    assert!(global.inherited_draft.is_none());
    let local = f.store.preferences_scope(format!("game:{}", f.a)).unwrap();
    assert!(local.saved.is_none());
    assert_eq!(local.effective, global.global);
    assert!(local.overridden_fields.is_empty());
    let draft = local.inherited_draft.unwrap();
    assert_eq!(draft.scope_key, format!("game:{}", f.a));
    assert!(draft.overrides.is_empty());
    assert_eq!(f.store.snapshot().unwrap().commit_revision, before);
    for bad in ["game:", "other", "global:extra"] {
        assert_eq!(
            f.store.preferences_scope(bad.into()).unwrap_err().code,
            "invalid_scope"
        );
    }
    assert_eq!(
        f.store
            .preferences_scope("game:unknown".into())
            .unwrap_err()
            .code,
        "game_missing"
    );
    f.store
        .dispatch(
            CommandEnvelope {
                operation_id: "remove-b".into(),
                expected_revision: None,
                command: Command::ArchiveGame {
                    game_id: f.b.clone(),
                    row_revision: 1,
                },
            },
            11,
        )
        .unwrap();
    assert_eq!(
        f.store
            .preferences_scope(format!("game:{}", f.b))
            .unwrap_err()
            .code,
        "game_missing"
    );
    evidence::record(
        "schema-two-scoped-query-saved-head",
        &[("STORAGE-19", 15), ("V2-PRODUCERS", 1)],
        serde_json::json!({"producer":"SqliteStore.preferences_scope","scope":"explicit global/game","query_is_read_only":true}),
    );
}
#[test]
fn scoped_head_cas_ignores_unrelated_reading_commits_but_preserves_idempotency_and_conflicts() {
    let f = Fixture::new();
    let scope = format!("game:{}", f.a);
    let p = Preferences {
        scope_key: scope.clone(),
        source_font_pt: 30.,
        overrides: BTreeMap::from([("sourceFontPt".into(), true)]),
        ..Default::default()
    };
    let first = f.save("first", p.clone(), None).unwrap();
    assert!(!first.replayed);
    for n in 0..12 {
        f.store
            .dispatch(
                CommandEnvelope {
                    operation_id: format!("unrelated-line-{n}"),
                    expected_revision: None,
                    command: Command::ManualInput {
                        text: format!("無関係な原文{n}。"),
                        source_language: "ja".into(),
                    },
                },
                20 + n,
            )
            .unwrap();
    }
    let next = Preferences {
        source_font_pt: 31.,
        ..p.clone()
    };
    f.save("second", next.clone(), Some(1)).unwrap();
    let before = f.store.snapshot().unwrap().commit_revision;
    let replay = f.save("first", p.clone(), None).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.revision, first.revision);
    assert_eq!(
        f.save("stale", p.clone(), Some(1)).unwrap_err().code,
        "preference_head_conflict"
    );
    assert_eq!(
        f.save("missing", p.clone(), None).unwrap_err().code,
        "preference_head_conflict"
    );
    assert_eq!(
        f.save("first", next, None).unwrap_err().code,
        "operation_conflict"
    );
    assert_eq!(f.store.snapshot().unwrap().commit_revision, before);
    let view = f.store.preferences_scope(scope).unwrap();
    assert_eq!(view.head_revision, Some(2));
    assert_eq!(view.saved.unwrap().source_font_pt, 31.);
    let c = rusqlite::Connection::open(&f.path).unwrap();
    let count: i64 = c
        .query_row("SELECT count(*) FROM preference_revisions", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 2);
    evidence::record(
        "schema-two-scoped-head-cas-idempotency",
        &[("STORAGE-19", 3), ("STORAGE-20", 7)],
        serde_json::json!({"producer":"SqliteStore.dispatch.SaveScopedPreferences","unrelated_reading_commits":12,"operation_replay_and_conflict":true,"immutable_versions":count}),
    );
}
#[test]
fn explicit_scope_does_not_return_another_active_game_and_roundtrips_saved_values() {
    let f = Fixture::new();
    let global = Preferences {
        source_font_pt: 20.,
        ..Default::default()
    };
    f.save("global", global.clone(), None).unwrap();
    for (game, pt, op) in [(&f.a, 25., "a-local"), (&f.b, 35., "b-local")] {
        f.save(
            op,
            Preferences {
                scope_key: format!("game:{game}"),
                source_font_pt: pt,
                overrides: BTreeMap::from([("sourceFontPt".into(), true)]),
                ..Default::default()
            },
            None,
        )
        .unwrap();
    }
    f.store
        .open_hook_session(
            HookSessionInput {
                session_id: "active-a".into(),
                game_id: Some(f.a.clone()),
                environment_key: "crossover:/private/a".into(),
                target_exe_path: "C:\\a.exe".into(),
                windows_pid: 100,
                process_created_u64: "1".into(),
                architecture: "x86".into(),
                broker_instance_id: "controlled".into(),
                resource: ResourceSnapshotInput {
                    snapshot_id: "core".into(),
                    kind: "core".into(),
                    version: "fixture".into(),
                    manifest_sha256: content_hash("core"),
                    contract_version: "1".into(),
                    metadata_json: "{}".into(),
                },
                target_identity_sha256: content_hash("target"),
            },
            40,
        )
        .unwrap();
    assert_eq!(f.store.snapshot().unwrap().preferences.source_font_pt, 25.);
    let scope = format!("game:{}", f.b);
    let view = f.store.preferences_scope(scope.clone()).unwrap();
    assert_eq!(view.effective.source_font_pt, 35.);
    assert_eq!(view.global, global);
    assert_eq!(view.saved.as_ref().unwrap().source_font_pt, 35.);
    assert_eq!(view.overridden_fields, vec![PreferenceField::SourceFontPt]);
    drop(f.store);
    let reopened = SqliteStore::open(&f.path, "scope-profile", "reopened-scope", 100).unwrap();
    assert_eq!(reopened.preferences_scope(scope).unwrap(), view);
    evidence::record(
        "schema-two-scoped-isolation-reopen",
        &[("STORAGE-19", 6), ("V2-PRODUCERS", 1)],
        serde_json::json!({"producer":"SqliteStore.preferences_scope","active_game_differs_from_requested":true,"reopened_from_real_sqlite":true}),
    );
}
#[test]
fn false_zero_empty_and_legacy_segmentation_survive_until_explicit_inherited_draft_save() {
    let f = Fixture::new();
    let global = Preferences {
        segmentation_enabled: true,
        show_ruby: true,
        source_font_pt: 22.,
        hud_background_opacity: 0.7,
        shortcuts: BTreeMap::from([("toggleHud".into(), "Alt+Super+H".into())]),
        overrides: BTreeMap::from([
            ("hud.compact".into(), true),
            ("hud.alwaysOnTop".into(), true),
        ]),
        ..Default::default()
    };
    f.save("global", global.clone(), None).unwrap();
    let local = Preferences {
        scope_key: format!("game:{}", f.a),
        segmentation_enabled: false,
        show_ruby: false,
        source_font_pt: 99.,
        hud_background_opacity: 0.,
        shortcuts: BTreeMap::new(),
        overrides: BTreeMap::from([
            ("showRuby".into(), true),
            ("sourceFontPt".into(), false),
            ("hudBackgroundOpacity".into(), true),
            ("shortcuts".into(), true),
            ("reading.segmentationEnabled".into(), false),
            ("hud.compact".into(), false),
            ("hud.alwaysOnTop".into(), false),
        ]),
        ..Default::default()
    };
    f.save("local", local.clone(), None).unwrap();
    let view = f.store.preferences_scope(local.scope_key.clone()).unwrap();
    assert_eq!(view.saved, Some(local.clone()));
    assert!(!view.effective.segmentation_enabled);
    assert!(!view.effective.show_ruby);
    assert_eq!(view.effective.source_font_pt, 22.);
    assert_eq!(view.effective.hud_background_opacity, 0.);
    assert!(view.effective.shortcuts.is_empty());
    assert!(!view.effective.overrides["hud.compact"]);
    assert!(
        view.overridden_fields
            .contains(&PreferenceField::SegmentationEnabled)
    );
    assert!(
        view.overridden_fields
            .contains(&PreferenceField::HudAlwaysOnTop)
    );
    assert!(
        !view
            .overridden_fields
            .contains(&PreferenceField::SourceFontPt)
    );
    let inherited = view.inherited_draft.unwrap();
    let mut expected_inherited = global.clone();
    expected_inherited.scope_key = local.scope_key.clone();
    expected_inherited.overrides.clear();
    assert_eq!(inherited, expected_inherited);
    assert!(inherited.overrides.is_empty());
    assert_eq!(
        f.store
            .preferences_scope(local.scope_key.clone())
            .unwrap()
            .saved,
        Some(local.clone())
    );
    f.save("inherit", inherited, view.head_revision).unwrap();
    let restored = f.store.preferences_scope(local.scope_key.clone()).unwrap();
    assert!(restored.overridden_fields.is_empty());
    assert_eq!(
        restored.effective.segmentation_enabled,
        global.segmentation_enabled
    );
    let mut expected_effective = global.clone();
    expected_effective.scope_key = local.scope_key.clone();
    assert_eq!(restored.effective, expected_effective);
    assert_eq!(restored.effective.source_font_pt, 22.);
    assert_eq!(restored.effective.shortcuts, global.shortcuts);
    f.save(
        "global-next",
        Preferences {
            source_font_pt: 24.,
            ..global
        },
        Some(1),
    )
    .unwrap();
    assert_eq!(
        f.store
            .preferences_scope(local.scope_key)
            .unwrap()
            .effective
            .source_font_pt,
        24.
    );
    let c = rusqlite::Connection::open(&f.path).unwrap();
    let old: String = c
        .query_row(
            "SELECT overrides_json FROM preference_revisions WHERE preference_revision_id=?1",
            [view.preference_revision_id.unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert!(old.contains("reading.segmentationEnabled"));
    evidence::record(
        "schema-two-scoped-inherit-false-zero-empty",
        &[("STORAGE-19", 20), ("V2-PRODUCERS", 1)],
        serde_json::json!({"producer":"SqliteStore.preferences_scope and dispatch.SaveScopedPreferences","explicit_false_zero_empty_preserved":true,"legacy_mask_preserved_until_explicit_save":true,"older_immutable_revision_retained":true}),
    );
}
#[test]
fn competing_scope_writers_have_one_winner_and_one_explicit_head_conflict() {
    let f = Fixture::new();
    let store = Arc::new(f.store);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = vec![];
    for n in 0..2 {
        let store = store.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store.dispatch(
                CommandEnvelope {
                    operation_id: format!("racing-{n}"),
                    expected_revision: Some(0),
                    command: Command::SaveScopedPreferences {
                        preferences: Preferences {
                            source_font_pt: 20. + f64::from(n),
                            ..Default::default()
                        },
                        expected_head_revision: None,
                    },
                },
                100,
            )
        }));
    }
    barrier.wait();
    let results = workers
        .into_iter()
        .map(|w| w.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .map(|e| e.code.as_str())
            .collect::<Vec<_>>(),
        vec!["preference_head_conflict"]
    );
    assert_eq!(
        store
            .preferences_scope("global".into())
            .unwrap()
            .head_revision,
        Some(1)
    );
    evidence::record(
        "schema-two-scoped-writer-race",
        &[("STORAGE-20", 3)],
        serde_json::json!({"producer":"SqliteStore single writer","competing_writers":2,"winners":1,"loser_code":"preference_head_conflict"}),
    );
}
