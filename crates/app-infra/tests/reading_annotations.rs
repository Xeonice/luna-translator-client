//! The HUD can patch annotations, never replay an entire stale preferences form.
use app_core::{contracts::*, ports::ProductRepository};
use app_infra::storage::SqliteStore;
use std::collections::BTreeMap;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("luna-annotations-{}", uuid::Uuid::new_v4())))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn dispatch(store: &SqliteStore, operation: &str, command: Command) -> CommandReceipt {
    store
        .dispatch(
            CommandEnvelope {
                operation_id: operation.into(),
                expected_revision: None,
                command,
            },
            2,
        )
        .unwrap()
}

#[test]
fn annotation_patch_preserves_every_other_stored_preference_and_survives_restart() {
    let root = Directory::new();
    let path = root.0.join("reader.sqlite3");
    let mut expected = Preferences {
        theme: "light".into(),
        transparency_mode: "reduced".into(),
        source_font_pt: 29.,
        translation_font_pt: 35.,
        hud_width_pt: 701.,
        shortcuts: BTreeMap::from([("toggleHud".into(), "CmdOrCtrl+Alt+H".into())]),
        overrides: BTreeMap::from([("showRuby".into(), true)]),
        ..Preferences::default()
    };
    {
        let store = SqliteStore::open(&path, "annotations", "server-a", 1).unwrap();
        let original = dispatch(
            &store,
            "settings-change",
            Command::SavePreferences {
                preferences: expected.clone(),
            },
        );
        let receipt = dispatch(
            &store,
            "hud-annotations",
            Command::SetReadingAnnotations {
                show_ruby: false,
                show_pos: true,
            },
        );
        expected.show_ruby = false;
        expected.show_pos = true;
        assert_eq!(store.snapshot().unwrap().preferences, expected);
        assert!(receipt.revision > original.revision);
        let reader = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let old_flags:(bool,bool) = reader.query_row("SELECT show_ruby,show_pos FROM preference_revisions WHERE preference_revision_id=?1",[original.result_id.unwrap()],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(old_flags, (true, false)); // Existing immutable revision is unchanged.
    }
    let reopened = SqliteStore::open(&path, "annotations", "server-b", 10).unwrap();
    assert_eq!(reopened.snapshot().unwrap().preferences, expected);
}

#[test]
fn first_annotation_patch_uses_defaults_and_replay_creates_no_extra_revision() {
    let root = Directory::new();
    let store =
        SqliteStore::open(root.0.join("reader.sqlite3"), "annotations", "server", 1).unwrap();
    let command = Command::SetReadingAnnotations {
        show_ruby: false,
        show_pos: true,
    };
    let receipt = dispatch(&store, "annotations-once", command.clone());
    let after = store.snapshot().unwrap();
    assert_eq!(
        after.preferences,
        Preferences {
            show_ruby: false,
            show_pos: true,
            ..Preferences::default()
        }
    );
    let replay = dispatch(&store, "annotations-once", command);
    assert!(replay.replayed);
    assert_eq!(replay.result_id, receipt.result_id);
    assert_eq!(
        store.snapshot().unwrap().commit_revision,
        after.commit_revision
    );
}

#[test]
fn annotation_contract_cannot_replace_other_preferences_from_a_hud_payload() {
    let command: Command = serde_json::from_value(serde_json::json!({"kind":"set_reading_annotations","payload":{"showRuby":false,"showPos":true,"theme":"light","shortcuts":{"toggleHud":"attacker-value"}}})).unwrap();
    let encoded = serde_json::to_value(&command).unwrap();
    assert_eq!(encoded["payload"].as_object().unwrap().len(), 2);
    let root = Directory::new();
    let store =
        SqliteStore::open(root.0.join("reader.sqlite3"), "annotations", "server", 1).unwrap();
    dispatch(&store, "narrow-payload", command);
    assert_eq!(
        store.snapshot().unwrap().preferences,
        Preferences {
            show_ruby: false,
            show_pos: true,
            ..Preferences::default()
        }
    );
}
