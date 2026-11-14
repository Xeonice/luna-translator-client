use app_core::{
    contracts::*,
    domain::{
        content_hash,
        model_config::{service_codec::decode_service_graph_import, service_graph::*},
    },
    ports::{ServiceConfigurationRepository, service_configuration::CompiledServiceDraft},
};
use app_infra::{providers::builtin_registry, storage::SqliteStore};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
fn setup() -> (PathBuf, SqliteStore) {
    let dir = std::env::temp_dir().join(format!("service-selection-v4-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("profile.sqlite3");
    let store = SqliteStore::open(&path, "profile", "server", 100).unwrap();
    store
        .configure_service_directory(Arc::new(builtin_registry().unwrap()))
        .unwrap();
    (path, store)
}
fn graph(revision: &str, model_name: &str) -> ConfigurationV3 {
    decode_service_graph_import(&serde_json::to_vec(&json!({"revisionId":revision,"connections":[{"connectionId":"c","displayName":"Local test","protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:8181/v1","proxyMode":"inherit","networkMetadata":{},"availabilityIntent":true,"adapterBinding":{"protocol":{"protocolId":"openai_compatible","contractRevision":1},"profile":{"profileId":"openai-chat-text","profileRevision":1}},"httpOptions":{"protocol":"openai_compatible"},"origin":{"kind":"custom"}}],"models":[{"defaultTemplates":{"translation":"t"},"modelId":"m","connectionId":"c","actualModelId":model_name,"displayName":model_name,"selectionSource":"manual","capabilities":{},"parameters":{},"templatePurpose":"translation","requestDefaults":{"translation":{"purpose":"translation","languages":{"kind":"http","source":{"kind":"inherit_source"},"target":{"kind":"inherit_preference"}},"context":{"enabled":false,"maxSentences":0,"mode":"original"},"execution":{"timeoutMs":30000,"concurrency":1,"retries":0},"cache":{"enabled":false},"fallback":[]}}}],"templates":[{"templateId":"t","purpose":"translation","displayName":"translation","sourceKind":"custom","body":{"user":"Translate {{source}}"},"variables":["source"],"compatibility":{}}],"bindings":[],"termSets":[],"offlineEngineConfigs":[],"codecVersion":3})).unwrap()).unwrap()
}
fn save(
    store: &SqliteStore,
    g: ConfigurationV3,
    op: &str,
    head: u32,
) -> AppResult<ServiceDraftReceipt> {
    let generation = store.service_configuration()?.profile_generation;
    store.save_service_configuration(
        &CompiledServiceDraft {
            operation_id: op.into(),
            request_hash: content_hash(op),
            expected_profile_generation: generation,
            expected_draft_head_revision: head,
            resource: EngineTarget::Http {
                model_id: "m".into(),
            },
            graph: g,
        },
        110 + head as i64,
    )
}
fn select(
    store: &SqliteStore,
    source: SavedServiceRef,
    op: &str,
    head: u32,
    selection: u32,
) -> AppResult<ServiceSelectionReceipt> {
    store.select_saved_services(
        &SelectSavedServicesInput {
            operation_id: op.into(),
            expected_profile_generation: store.service_configuration()?.profile_generation,
            scope: ServiceScope::Global {},
            purpose: CatalogPurpose::Translation,
            expected_selection_revision: selection,
            expected_applied_head_revision: head,
            main: source,
            comparisons: vec![],
        },
        200 + head as i64,
    )
}
#[test]
fn saved_graph_is_draft_only_reopens_and_replays_before_cas() {
    let (path, store) = setup();
    let g = graph("saved-a", "a");
    let receipt = save(&store, g.clone(), "save-a", 0).unwrap();
    let state = store.service_configuration().unwrap();
    assert_eq!(state.draft.revision_id(), "saved-a");
    assert!(state.applied.is_none());
    assert_eq!(state.draft_head_revision, 1);
    assert!(state.runtime_purposes.is_empty());
    assert_eq!(
        store
            .service_draft_replay("save-a", &content_hash("save-a"), state.profile_generation)
            .unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(save(&store, g, "save-a", 0).unwrap(), receipt);
    drop(store);
    let reopened = SqliteStore::open(&path, "profile", "server", 300).unwrap();
    assert_eq!(
        reopened
            .service_configuration_revision("saved-a")
            .unwrap()
            .as_service_graph()
            .unwrap()
            .models[0]
            .actual_model_id
            .as_deref(),
        Some("a")
    );
    assert_eq!(
        reopened
            .saved_services(&SavedServiceQuery {
                expected_profile_generation: reopened
                    .service_configuration()
                    .unwrap()
                    .profile_generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation
            })
            .unwrap_err()
            .code,
        "service_directory_unavailable"
    );
}
#[test]
fn selector_clones_version_collision_preserves_draft_and_control_and_filters_activation() {
    let (path, store) = setup();
    let a = save(&store, graph("saved-a", "a"), "save-a", 0).unwrap();
    let first = select(&store, a.saved_resource.clone(), "select-a", 0, 0).unwrap();
    assert_eq!(first.selection_revision, 1);
    assert_eq!(
        store.service_configuration().unwrap().draft.revision_id(),
        "saved-a"
    );
    let q = SavedServiceQuery {
        expected_profile_generation: store.service_configuration().unwrap().profile_generation,
        scope: ServiceScope::Global {},
        purpose: CatalogPurpose::Translation,
    };
    let library = store.saved_services(&q).unwrap();
    assert_eq!(library.rows.len(), 1);
    assert!(matches!(
        library.rows[0].eligibility,
        SavedServiceEligibility::Selectable {}
    ));
    assert!(!library.selection.purpose_enabled);
    let second_saved = save(&store, graph("saved-b", "b"), "save-b", 1).unwrap();
    let second = select(
        &store,
        second_saved.saved_resource,
        "select-b",
        first.applied_head_revision,
        1,
    )
    .unwrap();
    let active = store
        .service_configuration_revision(&second.revision_id)
        .unwrap();
    let g = active.as_service_graph().unwrap();
    assert_eq!(g.models.len(), 2);
    assert_eq!(
        g.models
            .iter()
            .find(|m| m.model_id == "m")
            .unwrap()
            .actual_model_id
            .as_deref(),
        Some("a")
    );
    assert!(
        g.models
            .iter()
            .any(|m| m.model_id != "m" && m.actual_model_id.as_deref() == Some("b"))
    );
    assert_eq!(store.saved_services(&q).unwrap().rows.len(), 2);
    assert_eq!(
        store.service_configuration().unwrap().draft.revision_id(),
        "saved-b"
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    let controls:i64=db.query_row("SELECT count(DISTINCT control_connection_id) FROM activation_resource_materializations",[],|r|r.get(0)).unwrap();
    assert_eq!(controls, 1);
    assert_eq!(
        select(&store, a.saved_resource, "stale", 0, 0)
            .unwrap_err()
            .code,
        "revision_conflict"
    );
}
#[test]
fn duplicate_main_comparison_uses_logical_identity_and_rolls_back() {
    let (path, store) = setup();
    let a = save(&store, graph("saved-a", "a"), "save-a", 0).unwrap();
    let b = save(&store, graph("saved-b", "b"), "save-b", 1).unwrap();
    let err = store
        .select_saved_services(
            &SelectSavedServicesInput {
                operation_id: "duplicates".into(),
                expected_profile_generation: store
                    .service_configuration()
                    .unwrap()
                    .profile_generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: 0,
                expected_applied_head_revision: 0,
                main: a.saved_resource,
                comparisons: vec![b.saved_resource],
            },
            200,
        )
        .unwrap_err();
    assert_eq!(err.code, "duplicate_logical_service");
    assert!(store.service_configuration().unwrap().applied.is_none());
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM config_revisions WHERE revision_role='activation'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn save_source_claim_and_runtime_intent_tamper_are_rejected() {
    let (_, store) = setup();
    let a = save(&store, graph("saved-a", "a"), "save-a", 0).unwrap();
    let mut tampered = graph("tampered", "bad");
    tampered.models[0].legacy_eligibility_origin = Some(HttpConfigRef {
        revision_id: "saved-a".into(),
        model_id: "m".into(),
    });
    assert_eq!(
        save(&store, tampered, "tamper", 1).unwrap_err().code,
        "untrusted_materialization_claim"
    );
    let mut changed = graph("changed", "b");
    changed.bindings.push(PurposeBindingV3 {
        binding_id: "b".into(),
        scope_key: "global".into(),
        game_id: None,
        purpose: CatalogPurpose::Translation,
        binding_mode: "override".into(),
        enabled: true,
        output_mode: Some("text".into()),
        source_language: None,
        target_language: None,
        template_id: None,
        context: Default::default(),
        schedule: Default::default(),
        parameters: Default::default(),
        engines: vec![],
        semantics_version: PurposeSemanticsVersion::TypedV3,
        scope_overrides: Default::default(),
    });
    assert_eq!(
        save(&store, changed, "intent", 1).unwrap_err().code,
        "service_save_changes_runtime_intent"
    );
    assert_eq!(
        store.service_configuration().unwrap().draft.revision_id(),
        a.revision_id
    );
}
struct Ids;
impl app_core::ports::IdentitySource for Ids {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }
    fn now_ms(&self) -> i64 {
        panic!("pure compiler cannot read clock")
    }
}
#[test]
fn actual_directory_compiler_empty_profile_saves_template_and_preset_and_reopens() {
    let (path, store) = setup();
    let directory = builtin_registry().unwrap();
    let state = store.service_configuration().unwrap();
    let input = SaveServiceDraftInput {
        operation_id: "real-compiler-save".into(),
        expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: 0,
        editing: None,
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: "builtin.ollama".into(),
                preset_revision: 1,
            },
        },
        purpose: CatalogPurpose::Translation,
        values: [
            (
                "name".into(),
                CatalogScalar::Text("My local service".into()),
            ),
            (
                "model".into(),
                CatalogScalar::Text("synthetic-model".into()),
            ),
            (
                "template".into(),
                CatalogScalar::Text("Translate {{source}}".into()),
            ),
        ]
        .into_iter()
        .collect(),
        authentication: ServiceAuthenticationSelection::None {},
    };
    let command = app_core::domain::model_config::service_draft::compile_service_draft(
        &state.draft,
        &input,
        &directory,
        &Ids,
        &Default::default(),
    )
    .unwrap();
    let r = store.save_service_configuration(&command, 300).unwrap();
    let document = store
        .service_configuration_revision(&r.revision_id)
        .unwrap();
    let graph = document.as_service_graph().unwrap();
    assert_eq!(graph.templates.len(), 1);
    assert_eq!(graph.templates[0].source_kind, "custom");
    assert!(graph.bindings.is_empty());
    assert_eq!(
        graph.connections[0]
            .preset_source
            .as_ref()
            .unwrap()
            .preset_id,
        "builtin.ollama"
    );
    assert!(store.service_configuration().unwrap().applied.is_none());
    drop(store);
    let reopened = SqliteStore::open(path, "profile", "server", 301).unwrap();
    assert_eq!(
        reopened
            .service_configuration_revision(&r.revision_id)
            .unwrap(),
        document
    );
}
fn legacy_setup() -> (PathBuf, SqliteStore) {
    let dir = std::env::temp_dir().join(format!("service-selection-old-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("profile.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    db.execute_batch(include_str!("fixtures/http-schema2-original.sql"))
        .unwrap();
    drop(db);
    let store = SqliteStore::open(&path, "legacy-http-profile", "service-selector", 300).unwrap();
    store
        .configure_service_directory(Arc::new(builtin_registry().unwrap()))
        .unwrap();
    (path, store)
}
#[test]
fn selecting_new_grammar_keeps_real_legacy_translation_admission_and_inflight_authority() {
    use app_core::ports::{IdentitySource, ProductRepository};
    use app_core::domain::model_config::service_draft::compile_service_draft;
    let (path, store) = legacy_setup();
    let first_unit = command(&store, "continuity-first", Command::ManualInput { text: "最初の文章。".into(), source_language: "ja".into() }, 301).result_id.unwrap();
    let prior = store.prepare_tasks(first_unit, 1, "translation".into(), 302).unwrap();
    assert!(!prior.is_empty());
    let state = store.service_configuration().unwrap();
    let runtime_before = runtime_row(&path, "translation");
    let directory = builtin_registry().unwrap();
    let input = SaveServiceDraftInput {
        operation_id: "continuity-save".into(), expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: state.draft_head_revision, editing: None,
        target: ServiceFormTarget::Http { preset: ProviderPresetKey { preset_id: "builtin.ollama".into(), preset_revision: 1 } },
        purpose: CatalogPurpose::Grammar,
        values: [("model".into(), CatalogScalar::Text("synthetic-grammar".into()))].into_iter().collect(),
        authentication: ServiceAuthenticationSelection::None {},
    };
    struct Clock;
    impl IdentitySource for Clock {
        fn new_id(&self, prefix: &str) -> String { format!("{prefix}-{}", uuid::Uuid::new_v4()) }
        fn now_ms(&self) -> i64 { 303 }
    }
    let compiled = compile_service_draft(&state.draft, &input, &directory, &Clock, &Default::default()).unwrap();
    let saved = store.save_service_configuration(&compiled, 303).unwrap();
    store.select_saved_services(&SelectSavedServicesInput {
        operation_id: "continuity-select".into(), expected_profile_generation: state.profile_generation,
        scope: ServiceScope::Global {}, purpose: CatalogPurpose::Grammar,
        expected_selection_revision: 0, expected_applied_head_revision: state.applied_head_revision,
        main: saved.saved_resource, comparisons: vec![],
    }, 304).unwrap();
    assert_eq!(runtime_row(&path, "translation"), runtime_before);
    for task in &prior { store.validate_task_authority(task.task_id.clone()).unwrap(); }
    let next_unit = command(&store, "continuity-second", Command::ManualInput { text: "次の文章。".into(), source_language: "ja".into() }, 305).result_id.unwrap();
    let next = store.prepare_tasks(next_unit, 1, "translation".into(), 306).unwrap();
    assert!(!next.is_empty());
    assert_eq!(next[0].configuration, prior[0].configuration);
    assert_eq!(next[0].binding, prior[0].binding);
    assert_eq!(next[0].connection, prior[0].connection);
    assert_eq!(next[0].authorization_epoch, prior[0].authorization_epoch);
    for task in next { store.validate_task_authority(task.task_id).unwrap(); }
}
fn runtime_row(path: &PathBuf, purpose: &str) -> (String, String, i64, i64, String, i64) {
    let db = rusqlite::Connection::open(path).unwrap();
    db.query_row("SELECT binding_id,config_revision_id,is_enabled,authorization_epoch,config_semantics_sha256,updated_at_ms FROM purpose_runtime_states WHERE scope_key='global' AND purpose=?1",[purpose],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap()
}
#[test]
fn real_old_source_materialization_is_proved_and_unrelated_runtime_seal_rows_survive() {
    let (path, store) = legacy_setup();
    let state = store.service_configuration().unwrap();
    let old = state.draft.revision_id().to_owned();
    let grammar_before = runtime_row(&path, "grammar");
    let before = store.service_configuration_revision(&old).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    let old_seal: String = db
        .query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&old],
            |r| r.get(0),
        )
        .unwrap();
    drop(db);
    let mut graph =
        app_core::domain::model_config::service_draft::materialize_base(&state.draft).unwrap();
    graph.revision_id = "mixed-saved".into();
    let new = crate::graph("new-fragment", "new");
    graph.connections.extend(new.connections);
    graph.models.extend(new.models);
    graph.templates.extend(new.templates);
    let command = CompiledServiceDraft {
        operation_id: "mixed-save".into(),
        request_hash: content_hash("mixed-save"),
        expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: state.draft_head_revision,
        resource: EngineTarget::Http {
            model_id: "m".into(),
        },
        graph: graph.clone(),
    };
    let saved = store.save_service_configuration(&command, 310).unwrap();
    assert_eq!(
        store
            .service_configuration()
            .unwrap()
            .applied
            .unwrap()
            .revision_id(),
        old
    );
    assert_eq!(runtime_row(&path, "grammar"), grammar_before);
    let selected = store
        .select_saved_services(
            &SelectSavedServicesInput {
                operation_id: "mixed-select".into(),
                expected_profile_generation: state.profile_generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: 0,
                expected_applied_head_revision: state.applied_head_revision,
                main: saved.saved_resource.clone(),
                comparisons: vec![],
            },
            320,
        )
        .unwrap();
    assert_eq!(runtime_row(&path, "grammar"), grammar_before);
    assert_eq!(store.service_configuration_revision(&old).unwrap(), before);
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&old],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        old_seal
    );
    assert!(db.query_row("SELECT count(*) FROM activation_resource_materializations WHERE effective_revision_id=?1 AND reason='legacy_materialization'",[&selected.revision_id],|r|r.get::<_,i64>(0)).unwrap()>0);
    assert_eq!(
        store.service_configuration().unwrap().draft.revision_id(),
        "mixed-saved"
    );
    drop(db);
    let mut tampered = graph;
    tampered.revision_id = "tampered-source".into();
    tampered
        .connections
        .iter_mut()
        .find(|c| c.materialization_source.is_some())
        .unwrap()
        .network_metadata
        .insert("forged".into(), json!(true));
    let mut bad = CompiledServiceDraft {
        operation_id: "bad-source".into(),
        request_hash: content_hash("bad-source"),
        expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: saved.draft_head_revision,
        resource: EngineTarget::Http {
            model_id: "m".into(),
        },
        graph: tampered,
    };
    assert_eq!(
        store
            .save_service_configuration(&bad, 330)
            .unwrap_err()
            .code,
        "materialization_field_mismatch"
    );
    bad.operation_id = "same-op-different-hash".into();
    assert_eq!(
        store
            .service_draft_replay(
                "mixed-save",
                &content_hash("different"),
                state.profile_generation
            )
            .unwrap_err()
            .code,
        "operation_conflict"
    );
}
#[test]
fn selecting_same_saved_versions_is_noop_and_profile_replay_is_fenced() {
    let (path, store) = setup();
    let a = save(&store, graph("saved-a", "a"), "save-a", 0).unwrap();
    let first = select(&store, a.saved_resource.clone(), "select-a", 0, 0).unwrap();
    let row = runtime_row(&path, "translation");
    let same = select(
        &store,
        a.saved_resource,
        "same-selection",
        first.applied_head_revision,
        1,
    )
    .unwrap();
    assert!(!same.semantic_change);
    assert_eq!(same.applied_head_revision, first.applied_head_revision);
    assert!(same.affected_task_ids.is_empty());
    assert_eq!(runtime_row(&path, "translation"), row);
    assert_eq!(
        store
            .service_draft_replay(
                "save-a",
                &content_hash("save-a"),
                std::num::NonZeroU32::new(999).unwrap()
            )
            .unwrap_err()
            .code,
        "profile_generation_conflict"
    );
}
fn command(store: &SqliteStore, op: &str, command: Command, now: i64) -> CommandReceipt {
    use app_core::ports::ProductRepository;
    store
        .dispatch(
            CommandEnvelope {
                operation_id: op.into(),
                expected_revision: None,
                command,
            },
            now,
        )
        .unwrap()
}
fn add_game(store: &SqliteStore) -> String {
    command(
        store,
        "add-game",
        Command::SaveGame(Box::new(GameInput {
            game_id: None,
            row_revision: None,
            display_name: "Synthetic game".into(),
            original_title: None,
            source_language: Some("ja".into()),
            cover_asset_ref: None,
            target: GameTarget {
                environment_kind: "crossover".into(),
                environment_key: "crossover:/private/synthetic-bottle".into(),
                bottle_locator: Some("/private/synthetic-bottle".into()),
                launch_path: Some("C:\\games\\game.exe".into()),
                canonical_exe_path: Some("C:\\games\\game.exe".into()),
                target_path_key: Some("c:/games/game.exe".into()),
                working_directory: None,
                arguments: vec![],
                architecture: Some("x86".into()),
                verification_state: "verified".into(),
            },
        })),
        500,
    )
    .result_id
    .unwrap()
}
#[test]
fn game_override_global_changes_and_restore_inheritance_have_independent_cas_and_epochs() {
    let (path, store) = setup();
    let game = add_game(&store);
    let a = save(&store, graph("a", "a"), "save-a", 0).unwrap();
    let first = select(&store, a.saved_resource, "select-a", 0, 0).unwrap();
    let b = save(&store, graph("b", "b"), "save-b", 1).unwrap();
    let generation = store.service_configuration().unwrap().profile_generation;
    let game_input = SelectSavedServicesInput {
        operation_id: "game-select".into(),
        expected_profile_generation: generation,
        scope: ServiceScope::Game {
            game_id: game.clone(),
        },
        purpose: CatalogPurpose::Translation,
        expected_selection_revision: 0,
        expected_applied_head_revision: first.applied_head_revision,
        main: b.saved_resource.clone(),
        comparisons: vec![],
    };
    let game_receipt = store.select_saved_services(&game_input, 510).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    let game_before:(String,i64,String)=db.query_row("SELECT config_revision_id,authorization_epoch,config_semantics_sha256 FROM purpose_runtime_states WHERE scope_key=?1",[format!("game:{game}")],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    drop(db);
    let c = save(&store, graph("c", "c"), "save-c", 2).unwrap();
    let global = select(
        &store,
        c.saved_resource,
        "select-c",
        game_receipt.applied_head_revision,
        1,
    )
    .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(db.query_row("SELECT config_revision_id,authorization_epoch,config_semantics_sha256 FROM purpose_runtime_states WHERE scope_key=?1",[format!("game:{game}")],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?))).unwrap(),game_before);
    drop(db);
    let global_before = runtime_row(&path, "translation");
    let restore = RestoreServiceInheritanceInput {
        operation_id: "inherit".into(),
        expected_profile_generation: generation,
        game_id: game.clone(),
        purpose: CatalogPurpose::Translation,
        expected_selection_revision: 1,
        expected_applied_head_revision: global.applied_head_revision,
    };
    let restored = store.restore_service_inheritance(&restore, 520).unwrap();
    assert_eq!(restored.selection_revision, 2);
    assert_eq!(runtime_row(&path, "translation"), global_before);
    assert_eq!(
        store.restore_service_inheritance(&restore, 521).unwrap(),
        restored
    );
    let projection = store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: generation,
            scope: ServiceScope::Game { game_id: game },
            purpose: CatalogPurpose::Translation,
        })
        .unwrap();
    assert_eq!(
        projection.selection.effective_scope,
        ServiceScope::Global {}
    );
    assert_eq!(projection.selection.selection_revision, 2);
    assert_eq!(projection.selection.effective_selection_revision, 2);
    assert_eq!(
        projection.selection.main,
        store
            .saved_services(&SavedServiceQuery {
                expected_profile_generation: generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation
            })
            .unwrap()
            .selection
            .main
    );
    assert_eq!(
        store.select_saved_services(&game_input, 522).unwrap(),
        game_receipt
    );
}
#[test]
fn switching_selected_service_invalidates_real_inflight_legacy_tasks_in_transaction() {
    use app_core::ports::ProductRepository;
    let (path, store) = legacy_setup();
    let state = store.service_configuration().unwrap();
    let mut graph =
        app_core::domain::model_config::service_draft::materialize_base(&state.draft).unwrap();
    graph.revision_id = "mixed-for-task".into();
    let new = crate::graph("fragment", "new");
    graph.connections.extend(new.connections);
    graph.models.extend(new.models);
    graph.templates.extend(new.templates);
    let saved = store
        .save_service_configuration(
            &CompiledServiceDraft {
                operation_id: "task-save".into(),
                request_hash: content_hash("task-save"),
                expected_profile_generation: state.profile_generation,
                expected_draft_head_revision: state.draft_head_revision,
                resource: EngineTarget::Http {
                    model_id: "m".into(),
                },
                graph,
            },
            600,
        )
        .unwrap();
    let unit = command(
        &store,
        "manual",
        Command::ManualInput {
            text: "原文".into(),
            source_language: "ja".into(),
        },
        601,
    )
    .result_id
    .unwrap();
    let tasks = store
        .prepare_tasks(unit, 1, "translation".into(), 602)
        .unwrap();
    assert!(!tasks.is_empty());
    assert!(
        store
            .validate_task_authority(tasks[0].task_id.clone())
            .is_ok()
    );
    let selected = store
        .select_saved_services(
            &SelectSavedServicesInput {
                operation_id: "task-switch".into(),
                expected_profile_generation: state.profile_generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: 0,
                expected_applied_head_revision: state.applied_head_revision,
                main: saved.saved_resource,
                comparisons: vec![],
            },
            603,
        )
        .unwrap();
    assert!(selected.affected_task_ids.contains(&tasks[0].task_id));
    assert!(
        store
            .validate_task_authority(tasks[0].task_id.clone())
            .is_err()
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT state FROM request_tasks WHERE task_id=?1",
            [&tasks[0].task_id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "invalidated"
    );
    assert_eq!(
        db.query_row(
            "SELECT state FROM request_attempts WHERE task_id=?1",
            [&tasks[0].task_id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "cancelled"
    );
}
#[test]
fn typed_test_projection_keeps_real_success_and_failure_and_marks_uncovered() {
    use app_core::ports::ProductRepository;
    let (_, store) = setup();
    let saved = save(&store, graph("a", "a"), "save-a", 0).unwrap();
    let record = |id: &str, state: &str, error: Option<&str>, at: i64| {
        app_core::domain::testing::ModelTestRecordInput {
            test_id: id.into(),
            config_revision_id: saved.revision_id.clone(),
            connection_id: Some("c".into()),
            model_id: Some("m".into()),
            binding_id: None,
            purpose: None,
            stage: "model".into(),
            request_semantics_hash: content_hash(id),
            input_scope: "sample".into(),
            unit_id: None,
            text_revision: None,
            input_summary: json!({}),
            response_summary: json!({}),
            output_validation: json!({}),
            state: state.into(),
            started_at_ms: at,
            completed_at_ms: at + 1,
            error_code: error.map(str::to_owned),
            error_stage: None,
            credentials: vec![],
        }
    };
    store
        .record_model_test(record("passed", "passed", None, 400))
        .unwrap();
    let snapshot = store.snapshot().unwrap();
    let test = &snapshot.configuration_tests[0];
    assert_eq!(test.status, "passed");
    assert_eq!(test.object_id, "m");
    assert!(test.error_code.is_none());
    assert!(test.stale);
    assert_eq!(
        test.comparison_coverage,
        Some(ConfigurationTestCoverage::NotCovered)
    );
    let q = SavedServiceQuery {
        expected_profile_generation: store.service_configuration().unwrap().profile_generation,
        scope: ServiceScope::Global {},
        purpose: CatalogPurpose::Translation,
    };
    assert!(matches!(
        store.saved_services(&q).unwrap().rows[0].verification,
        ServiceVerification::Stale { .. }
    ));
    store
        .record_model_test(record("failed", "failed", Some("actual_wire_failure"), 410))
        .unwrap();
    let rows = store.saved_services(&q).unwrap().rows;
    assert!(
        matches!(&rows[0].verification,ServiceVerification::Failed{code,..}if code=="actual_wire_failure")
    );
    assert!(
        matches!(&rows[0].eligibility,SavedServiceEligibility::Blocked{reasons}if reasons.iter().any(|r|r.code=="current_test_failed"))
    );
}
#[test]
fn late_sql_failure_rolls_back_activation_head_seal_runtime_and_receipt() {
    let (path, store) = setup();
    let a = save(&store, graph("a", "a"), "save-a", 0).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("UPDATE profile_meta SET commit_revision=?1", [i64::MAX])
        .unwrap();
    let before: i64 = db
        .query_row("SELECT count(*) FROM config_revisions", [], |r| r.get(0))
        .unwrap();
    drop(db);
    assert!(select(&store, a.saved_resource, "overflow-activation", 0, 0).is_err());
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM config_revisions", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        before
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM config_heads WHERE head_kind='applied'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM purpose_runtime_states", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM operation_receipts WHERE operation_id='overflow-activation'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}
#[test]
fn explicit_purpose_enable_changes_intent_and_runtime_without_moving_draft_or_replaying_current_input()
 {
    let (path, store) = setup();
    let saved = save(&store, graph("a", "a"), "save-a", 0).unwrap();
    let selected = select(&store, saved.saved_resource, "select-a", 0, 0).unwrap();
    let generation = store.service_configuration().unwrap().profile_generation;
    let on = SetServicePurposeEnabledInput {
        operation_id: "purpose-on".into(),
        expected_profile_generation: generation,
        scope: ServiceScope::Global {},
        purpose: CatalogPurpose::Translation,
        expected_selection_revision: 1,
        expected_applied_head_revision: selected.applied_head_revision,
        enabled: true,
    };
    let enabled = store.set_service_purpose_enabled(&on, 700).unwrap();
    let state = store.service_configuration().unwrap();
    assert_eq!(state.draft.revision_id(), "a");
    assert!(
        state
            .applied
            .as_ref()
            .unwrap()
            .as_service_graph()
            .unwrap()
            .bindings[0]
            .enabled
    );
    assert!(state.runtime_purposes[0].runtime_enabled);
    assert_eq!(
        store.set_service_purpose_enabled(&on, 701).unwrap(),
        enabled
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM request_tasks", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    let no_op = store
        .set_service_purpose_enabled(
            &SetServicePurposeEnabledInput {
                operation_id: "same-on".into(),
                expected_selection_revision: enabled.selection_revision,
                expected_applied_head_revision: enabled.applied_head_revision,
                ..on.clone()
            },
            702,
        )
        .unwrap();
    assert!(!no_op.semantic_change);
    assert_eq!(no_op.applied_head_revision, enabled.applied_head_revision);
    let off = store
        .set_service_purpose_enabled(
            &SetServicePurposeEnabledInput {
                operation_id: "purpose-off".into(),
                enabled: false,
                expected_selection_revision: enabled.selection_revision,
                expected_applied_head_revision: enabled.applied_head_revision,
                ..on
            },
            703,
        )
        .unwrap();
    assert!(off.semantic_change);
    assert!(!store.service_configuration().unwrap().runtime_purposes[0].runtime_enabled);
    assert_eq!(
        store.service_configuration().unwrap().draft.revision_id(),
        "a"
    );
}
#[test]
fn saved_incomplete_draft_is_blocked_by_shared_static_preflight() {
    let (_, store) = setup();
    let mut incomplete = graph("incomplete", "unused");
    incomplete.models[0].actual_model_id = None;
    let r = save(&store, incomplete, "save-incomplete", 0).unwrap();
    let rows = store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: store.service_configuration().unwrap().profile_generation,
            scope: ServiceScope::Global {},
            purpose: CatalogPurpose::Translation,
        })
        .unwrap()
        .rows;
    assert!(
        matches!(&rows[0].eligibility,SavedServiceEligibility::Blocked{reasons}if reasons.iter().any(|r|r.code=="missing_model"))
    );
    assert_eq!(
        select(&store, r.saved_resource, "select-incomplete", 0, 0)
            .unwrap_err()
            .code,
        "service_not_selectable"
    );
}
#[test]
fn shared_connection_stop_cannot_be_bypassed_by_another_saved_version_or_enable() {
    let (_, store) = setup();
    let saved = save(&store, graph("a", "a"), "save-a", 0).unwrap();
    let selected = select(&store, saved.saved_resource, "select-a", 0, 0).unwrap();
    command(
        &store,
        "stop",
        Command::SetConnectionSuspended {
            connection_id: "c".into(),
            suspended: true,
            expected_control_revision: 1,
        },
        710,
    );
    let b = save(&store, graph("b", "b"), "save-b", 1).unwrap();
    assert_eq!(
        select(
            &store,
            b.saved_resource,
            "blocked-version",
            selected.applied_head_revision,
            1
        )
        .unwrap_err()
        .code,
        "service_not_selectable"
    );
    assert_eq!(
        store
            .set_service_purpose_enabled(
                &SetServicePurposeEnabledInput {
                    operation_id: "blocked-on".into(),
                    expected_profile_generation: store
                        .service_configuration()
                        .unwrap()
                        .profile_generation,
                    scope: ServiceScope::Global {},
                    purpose: CatalogPurpose::Translation,
                    expected_selection_revision: 1,
                    expected_applied_head_revision: selected.applied_head_revision,
                    enabled: true
                },
                711
            )
            .unwrap_err()
            .code,
        "service_not_selectable"
    );
    assert!(store.service_configuration().unwrap().runtime_connections[0].suspended);
}
#[test]
fn google_service_account_route_and_sql_pool_do_not_fabricate_an_authorization_runtime() {
    use app_core::{
        domain::model_config::credentials::{CredentialMaterialKind, CredentialStorageMetadata},
        ports::ProductRepository,
    };
    let (_, store) = setup();
    store
        .begin_credential_material(
            CredentialReference {
                credential_id: "sa-metadata".into(),
                credential_version_id: "opaque-sa-vault".into(),
                pool_id: "sa-pool".into(),
                pool_revision: 1,
                label: "Synthetic SA metadata".into(),
                masked_hint: "opaque".into(),
            },
            CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::GoogleServiceAccount,
            },
            800,
        )
        .unwrap();
    store
        .activate_credential("sa-metadata".into(), 801)
        .unwrap();
    assert_eq!(
        store.credential_for_pool("sa-pool".into(), 1).unwrap(),
        "opaque-sa-vault"
    );
    let catalog = builtin_registry().unwrap().catalog();
    let preset = catalog
        .presets
        .iter()
        .find(|p| p.key.preset_id == "builtin.google-v3")
        .unwrap();
    let mut g = graph("sa-draft", "unused");
    let c = &mut g.connections[0];
    c.protocol = "google_translate_v3".into();
    c.adapter_binding = Some(preset.binding.clone());
    c.auth_mode = "google_service_account".into();
    c.endpoint = Some("https://translation.googleapis.com".into());
    c.http_options = Some(HttpServiceOptionsV1::GoogleTranslateV3 {
        project_id: "synthetic-project".into(),
        location: "global".into(),
    });
    c.credential_pool_id = Some("sa-pool".into());
    c.credential_pool_revision = std::num::NonZeroU32::new(1);
    g.models[0].actual_model_id = None;
    let r = save(&store, g, "sa-save", 0).unwrap();
    let rows = store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: store.service_configuration().unwrap().profile_generation,
            scope: ServiceScope::Global {},
            purpose: CatalogPurpose::Translation,
        })
        .unwrap()
        .rows;
    assert!(
        matches!(&rows[0].eligibility,SavedServiceEligibility::Blocked{reasons}if reasons.iter().any(|r|r.code=="google_authorization_not_ready"))
    );
    assert_eq!(
        select(&store, r.saved_resource, "sa-select", 0, 0)
            .unwrap_err()
            .code,
        "service_not_selectable"
    );
}
