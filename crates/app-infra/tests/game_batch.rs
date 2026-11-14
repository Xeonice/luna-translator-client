use app_core::{contracts::*, domain::content_hash, ports::ProductRepository};
use app_infra::storage::SqliteStore;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

struct Fixture {
    store: SqliteStore,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("luna-game-batch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("profile.sqlite3");
        Self {
            store: SqliteStore::open(&path, "batch-profile", "batch-server", 1).unwrap(),
            path,
        }
    }
    fn send(
        &self,
        op: &str,
        revision: Option<u32>,
        items: Vec<GameBatchItem>,
    ) -> AppResult<CommandReceipt> {
        self.store.dispatch(
            CommandEnvelope {
                operation_id: op.into(),
                expected_revision: revision,
                command: Command::SaveGames { items },
            },
            10,
        )
    }
    fn inspect(&self, sql: &str) -> i64 {
        Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }
}
fn item(key: &str) -> GameBatchItem {
    GameBatchItem {
        item_id: key.into(),
        game: GameInput {
            game_id: None,
            row_revision: None,
            display_name: format!("Game {key}"),
            original_title: None,
            source_language: Some("ja".into()),
            cover_asset_ref: None,
            target: GameTarget {
                environment_kind: "crossover".into(),
                environment_key: "crossover:/bottles/test".into(),
                bottle_locator: Some("/bottles/test".into()),
                launch_path: Some(format!("C:\\Games\\{key}.exe")),
                canonical_exe_path: Some(format!("C:\\Games\\{key}.exe")),
                target_path_key: None,
                working_directory: Some("C:\\Games".into()),
                arguments: vec![
                    "--title=日本語の 空白".into(),
                    "\"literal\"".into(),
                    "".into(),
                ],
                architecture: Some("x86".into()),
                verification_state: "verified".into(),
            },
        },
    }
}
#[test]
fn batch_is_one_commit_with_ordered_stable_results_and_lossless_arguments() {
    let f = Fixture::new();
    let items = vec![item("second"), item("first")];
    let receipt = f.send("once", Some(0), items.clone()).unwrap();
    let results = receipt.game_results.as_ref().unwrap();
    assert_eq!(receipt.revision, 1);
    assert_eq!(receipt.result_id, None);
    assert_eq!(
        results
            .iter()
            .map(|r| r.item_id.as_str())
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    for (result, input) in results.iter().zip(&items) {
        assert_eq!(
            result.game_id,
            format!("game-{}", content_hash(format!("once\0{}", input.item_id)))
        );
        assert_eq!(result.row_revision, 1);
        assert!(!result.reused);
        let game = f
            .store
            .snapshot()
            .unwrap()
            .games
            .into_iter()
            .find(|g| g.game_id == result.game_id)
            .unwrap();
        assert_eq!(game.target.arguments, input.game.target.arguments);
        assert_eq!(
            game.target.working_directory,
            input.game.target.working_directory
        );
    }
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 2);
    assert_eq!(f.inspect("SELECT count(*) FROM game_targets"), 2);
    assert_eq!(
        f.inspect("SELECT count(*) FROM operation_receipts WHERE command_kind='save_games'"),
        1
    );
}
#[test]
fn batch_requires_complete_snapshot_cas_but_replay_returns_original_revision() {
    let f = Fixture::new();
    assert_eq!(
        f.send("missing", None, vec![item("one")]).unwrap_err().code,
        "revision_required"
    );
    assert_eq!(
        f.send("stale", Some(1), vec![item("one")])
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    assert_eq!(f.store.snapshot().unwrap().commit_revision, 0);
    let first = f.send("once", Some(0), vec![item("one")]).unwrap();
    f.send("next", Some(1), vec![item("two")]).unwrap();
    let replay = f.send("once", Some(0), vec![item("one")]).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.revision, first.revision);
    assert_eq!(replay.game_results, first.game_results);
    assert_eq!(f.store.snapshot().unwrap().commit_revision, 2);
    let mut changed = item("one");
    changed.game.display_name = "Changed".into();
    assert_eq!(
        f.send("once", Some(0), vec![changed]).unwrap_err().code,
        "operation_conflict"
    );
    assert_eq!(f.inspect("SELECT count(*) FROM operation_receipts"), 2);
}
#[test]
fn batch_empty_and_oversize_are_rejected_and_exact_upper_boundary_saves() {
    let f = Fixture::new();
    assert_eq!(
        f.send("empty", Some(0), vec![]).unwrap_err().code,
        "invalid_batch_size"
    );
    assert_eq!(
        f.send(
            "too-many",
            Some(0),
            (0..101).map(|n| item(&n.to_string())).collect()
        )
        .unwrap_err()
        .code,
        "invalid_batch_size"
    );
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 0);
    let receipt = f
        .send(
            "full",
            Some(0),
            (0..100).map(|n| item(&n.to_string())).collect(),
        )
        .unwrap();
    assert_eq!(receipt.game_results.unwrap().len(), 100);
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 100);
    assert_eq!(f.store.snapshot().unwrap().commit_revision, 1);
}
#[test]
fn duplicate_item_or_normalized_target_rejects_the_whole_batch() {
    let f = Fixture::new();
    let mut second = item("two");
    second.item_id = "one".into();
    assert_eq!(
        f.send("same-item", Some(0), vec![item("one"), second])
            .unwrap_err()
            .code,
        "duplicate_batch_item"
    );
    let mut alias = item("alias");
    alias.game.target.canonical_exe_path = Some("c:/Games/./ONE.exe".into());
    alias.game.target.bottle_locator = Some("/bottles/other/../test/".into());
    assert_eq!(
        f.send("same-target", Some(0), vec![item("one"), alias])
            .unwrap_err()
            .code,
        "duplicate_batch_target"
    );
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 0);
    assert_eq!(f.inspect("SELECT count(*) FROM operation_receipts"), 0);
}
#[test]
fn invalid_later_project_preserves_no_partial_addition_or_receipt() {
    let f = Fixture::new();
    let mut invalid = item("two");
    invalid.game.display_name = "  ".into();
    assert_eq!(
        f.send("invalid", Some(0), vec![item("one"), invalid])
            .unwrap_err()
            .code,
        "required_name"
    );
    // This invalid architecture reaches the real SQLite CHECK after the first
    // valid project has been inserted, proving transaction rollback, not preflight alone.
    let mut constraint = item("constraint");
    constraint.game.target.verification_state = "unresolved".into();
    constraint.game.target.architecture = Some("arm64".into());
    assert_eq!(
        f.send("constraint", Some(0), vec![item("one"), constraint])
            .unwrap_err()
            .code,
        "storage_failure"
    );
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 0);
    assert_eq!(f.inspect("SELECT count(*) FROM game_targets"), 0);
    assert_eq!(f.inspect("SELECT count(*) FROM operation_receipts"), 0);
    assert_eq!(f.store.snapshot().unwrap().commit_revision, 0);
    assert_eq!(
        f.send("constraint", Some(0), vec![item("one"), item("two")])
            .unwrap()
            .revision,
        1
    );
}
#[test]
fn matching_existing_target_is_reused_without_overwriting_metadata() {
    let f = Fixture::new();
    let mut original = item("one").game;
    let first = f
        .store
        .dispatch(
            CommandEnvelope {
                operation_id: "single".into(),
                expected_revision: None,
                command: Command::SaveGame(Box::new(original.clone())),
            },
            2,
        )
        .unwrap();
    original.game_id = first.result_id;
    original.row_revision = Some(1);
    original.display_name = "User edited title".into();
    f.store
        .dispatch(
            CommandEnvelope {
                operation_id: "edit".into(),
                expected_revision: Some(1),
                command: Command::SaveGame(Box::new(original)),
            },
            3,
        )
        .unwrap();
    let before = f.store.snapshot().unwrap().games;
    let mut duplicate = item("different-item");
    duplicate.game.target.canonical_exe_path = Some("c:/GAMES/one.EXE".into());
    duplicate.game.target.arguments = vec!["do not overwrite".into()];
    let receipt = f
        .send("batch", Some(2), vec![item("new"), duplicate])
        .unwrap();
    let results = receipt.game_results.unwrap();
    assert!(!results[0].reused);
    assert!(results[1].reused);
    assert_eq!(results[1].game_id, before[0].game_id);
    assert_eq!(results[1].row_revision, 2);
    let retained = f
        .store
        .snapshot()
        .unwrap()
        .games
        .into_iter()
        .find(|g| g.game_id == before[0].game_id)
        .unwrap();
    assert_eq!(retained, before[0]);
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 2);
}
#[test]
fn archived_target_can_be_added_as_new_record_without_reviving_old_metadata() {
    let f = Fixture::new();
    let original = f
        .send("first", Some(0), vec![item("one")])
        .unwrap()
        .game_results
        .unwrap()
        .remove(0);
    f.store
        .dispatch(
            CommandEnvelope {
                operation_id: "archive".into(),
                expected_revision: Some(1),
                command: Command::ArchiveGame {
                    game_id: original.game_id.clone(),
                    row_revision: 1,
                },
            },
            11,
        )
        .unwrap();
    let added = f
        .send("new-record", Some(2), vec![item("one")])
        .unwrap()
        .game_results
        .unwrap()
        .remove(0);
    assert!(!added.reused);
    assert_ne!(added.game_id, original.game_id);
    assert_eq!(f.store.snapshot().unwrap().games.len(), 1);
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 2);
    assert_eq!(
        f.inspect("SELECT count(*) FROM games WHERE archived_at_ms IS NOT NULL"),
        1
    );
}
#[test]
fn separate_bottles_and_unresolved_paths_are_not_guessed_into_one_identity() {
    let f = Fixture::new();
    let mut other = item("two");
    other.game.target.canonical_exe_path = item("one").game.target.canonical_exe_path;
    other.game.target.bottle_locator = Some("/bottles/another".into());
    let mut unknown1 = item("unknown1");
    let mut unknown2 = item("unknown2");
    for unknown in [&mut unknown1, &mut unknown2] {
        unknown.game.display_name = "Same filename".into();
        unknown.game.target.environment_kind = "unresolved".into();
        unknown.game.target.canonical_exe_path = None;
        unknown.game.target.verification_state = "unresolved".into();
    }
    let results = f
        .send(
            "distinct",
            Some(0),
            vec![item("one"), other, unknown1, unknown2],
        )
        .unwrap()
        .game_results
        .unwrap();
    assert_eq!(results.len(), 4);
    assert!(results.iter().all(|r| !r.reused));
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 4);
}
#[test]
fn batch_does_not_silently_allow_editing_existing_record() {
    let f = Fixture::new();
    for (game_id, revision) in [(Some("existing".into()), None), (None, Some(1))] {
        let mut edit = item("one");
        edit.game.game_id = game_id;
        edit.game.row_revision = revision;
        assert_eq!(
            f.send("not-bulk-edit", Some(0), vec![edit])
                .unwrap_err()
                .code,
            "batch_add_only"
        );
    }
    assert_eq!(f.inspect("SELECT count(*) FROM games"), 0);
}
#[test]
fn durable_batch_receipt_replays_after_writer_restart_and_legacy_json_stays_compatible() {
    let f = Fixture::new();
    let first = f
        .send("durable", Some(0), vec![item("one"), item("two")])
        .unwrap();
    let path = f.path.clone();
    drop(f);
    let reopened = SqliteStore::open(&path, "batch-profile", "batch-server-2", 20).unwrap();
    let replay = reopened
        .dispatch(
            CommandEnvelope {
                operation_id: "durable".into(),
                expected_revision: Some(0),
                command: Command::SaveGames {
                    items: vec![item("one"), item("two")],
                },
            },
            21,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.game_results, first.game_results);
    assert_eq!(reopened.snapshot().unwrap().games.len(), 2);
    let old =
        serde_json::json!({"replayed":false,"requestId":"old","revision":1,"resultId":"game-old"});
    let decoded: CommandReceipt = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(decoded.game_results, None);
    assert_eq!(serde_json::to_value(decoded).unwrap(), old);
    let receipt = reopened
        .dispatch(
            CommandEnvelope {
                operation_id: "legacy".into(),
                expected_revision: None,
                command: Command::SaveGame(Box::new(item("legacy").game)),
            },
            22,
        )
        .unwrap();
    assert!(receipt.result_id.is_some());
    assert!(
        serde_json::to_value(receipt)
            .unwrap()
            .get("gameResults")
            .is_none()
    );
}
