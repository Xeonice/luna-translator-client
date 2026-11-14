//! Actual codec3 repository -> task/attempt target facts -> authority/commit/cache.
//! No real-vendor, GUI, packaged or installed-offline qualification is claimed.
use app_core::{
    contracts::*,
    domain::{
        TaskCompletion, content_hash,
        model_config::{service_draft::compile_service_draft, service_graph::*},
        service_dispatch::{ServiceTaskDispatch, WorkDispatch},
        service_execution::ServiceExecutionTarget,
    },
    ports::{IdentitySource, ProductRepository, ServiceConfigurationRepository},
};
use app_infra::{providers::builtin_registry, storage::SqliteStore};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
struct Ids;
impl IdentitySource for Ids {
    fn new_id(&self, p: &str) -> String {
        format!("{p}-{}", uuid::Uuid::new_v4())
    }
    fn now_ms(&self) -> i64 {
        panic!("pure compiler cannot read time")
    }
}
struct Fixture {
    path: PathBuf,
    store: SqliteStore,
}
fn fixture() -> Fixture {
    let dir = std::env::temp_dir().join(format!("typed-task-admission-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("profile.sqlite3");
    let store = SqliteStore::open(&path, "profile", "server", 10).unwrap();
    store
        .configure_service_directory(Arc::new(builtin_registry().unwrap()))
        .unwrap();
    Fixture { path, store }
}
fn command(f: &Fixture, op: &str, command: Command, now: i64) -> CommandReceipt {
    f.store
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
fn save(f: &Fixture, p: CatalogPurpose, op: &str, cache: bool, retries: u32) -> SavedServiceRef {
    save_options(
        f,
        p,
        op,
        cache,
        retries,
        "builtin.lmstudio",
        ServiceAuthenticationSelection::None {},
        vec![],
    )
}
fn save_options(
    f: &Fixture,
    p: CatalogPurpose,
    op: &str,
    cache: bool,
    retries: u32,
    preset: &str,
    authentication: ServiceAuthenticationSelection,
    extra: Vec<(String, CatalogScalar)>,
) -> SavedServiceRef {
    let state = f.store.service_configuration().unwrap();
    let input = SaveServiceDraftInput {
        operation_id: op.into(),
        expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: state.draft_head_revision,
        editing: None,
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: preset.into(),
                preset_revision: 1,
            },
        },
        purpose: p,
        values: [
            (
                "name".into(),
                CatalogScalar::Text(format!("{op} saved service")),
            ),
            (
                "model".into(),
                CatalogScalar::Text("synthetic-model".into()),
            ),
            (
                "template".into(),
                CatalogScalar::Text("Translate {{source}}".into()),
            ),
            ("cache".into(), CatalogScalar::Boolean(cache)),
            ("retries".into(), CatalogScalar::Number(retries as f64)),
        ]
        .into_iter()
        .chain(extra)
        .collect(),
        authentication,
    };
    let compiled = compile_service_draft(
        &state.draft,
        &input,
        &builtin_registry().unwrap(),
        &Ids,
        &Default::default(),
    )
    .unwrap();
    f.store
        .save_service_configuration(&compiled, 20)
        .unwrap()
        .saved_resource
}
fn select(
    f: &Fixture,
    source: SavedServiceRef,
    scope: ServiceScope,
    p: CatalogPurpose,
    op: &str,
) -> ServiceSelectionReceipt {
    let state = f.store.service_configuration().unwrap();
    let q = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: state.profile_generation,
            scope: scope.clone(),
            purpose: p,
        })
        .unwrap();
    f.store
        .select_saved_services(
            &SelectSavedServicesInput {
                operation_id: op.into(),
                expected_profile_generation: state.profile_generation,
                scope,
                purpose: p,
                expected_selection_revision: q.selection.selection_revision,
                expected_applied_head_revision: state.applied_head_revision,
                main: source,
                comparisons: vec![],
            },
            30,
        )
        .unwrap()
}
fn enable(
    f: &Fixture,
    scope: ServiceScope,
    p: CatalogPurpose,
    enabled: bool,
    op: &str,
) -> ServiceSelectionReceipt {
    let state = f.store.service_configuration().unwrap();
    let q = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: state.profile_generation,
            scope: scope.clone(),
            purpose: p,
        })
        .unwrap();
    f.store
        .set_service_purpose_enabled(
            &SetServicePurposeEnabledInput {
                operation_id: op.into(),
                expected_profile_generation: state.profile_generation,
                scope,
                purpose: p,
                expected_selection_revision: q.selection.selection_revision,
                expected_applied_head_revision: state.applied_head_revision,
                enabled,
            },
            40,
        )
        .unwrap()
}
fn configured(cache: bool, retries: u32) -> Fixture {
    let f = fixture();
    let r = save(&f, CatalogPurpose::Translation, "save", cache, retries);
    select(
        &f,
        r,
        ServiceScope::Global {},
        CatalogPurpose::Translation,
        "select",
    );
    enable(
        &f,
        ServiceScope::Global {},
        CatalogPurpose::Translation,
        true,
        "enable",
    );
    f
}
fn manual(f: &Fixture, op: &str, text: &str) -> String {
    command(
        f,
        op,
        Command::ManualInput {
            text: text.into(),
            source_language: "ja".into(),
        },
        50,
    )
    .result_id
    .unwrap()
}
fn prepared(f: &Fixture, unit: &str, op: &str, now: i64) -> ServiceTaskDispatch {
    let mut result = f
        .store
        .prepare_work_tasks_for_operation(unit.into(), 1, "translation".into(), op.into(), now)
        .unwrap();
    assert_eq!(result.len(), 1);
    match result.remove(0) {
        WorkDispatch::Service(value) => *value,
        WorkDispatch::Legacy(_) => panic!("typed graph was projected into legacy grammar"),
    }
}
fn completion(task: &ServiceTaskDispatch, text: &str) -> TaskCompletion {
    let ServiceExecutionTarget::Http { plan } = &task.plan.target else {
        panic!("HTTP fixture")
    };
    TaskCompletion {
        task_id: task.task_id.clone(),
        attempt_id: task.attempt_id.clone(),
        text: text.into(),
        output_mode: if task.plan.identity.purpose == CatalogPurpose::Translation {
            "text"
        } else {
            "plain_text"
        }
        .into(),
        structured: json!({}),
        target_language: plan.request.target_language.clone(),
        provider_label: plan.connection.display_name.clone(),
    }
}
#[test]
fn actual_http_admission_freezes_typed_targets_default_origin_and_real_cache_producer() {
    let f = configured(true, 1);
    let unit = manual(&f, "manual", "雨が降る。");
    let task = prepared(&f, &unit, "run", 60);
    f.store.validate_service_dispatch(&task).unwrap();
    let db = Connection::open(&f.path).unwrap();
    let target:(String,String,String)=db.query_row("SELECT target_kind,http_model_id,http_connection_id FROM task_execution_targets WHERE task_id=?1",[&task.task_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    let ServiceExecutionTarget::Http { plan: http } = &task.plan.target else {
        panic!()
    };
    assert_eq!(
        target,
        (
            "http".into(),
            http.model.model_id.clone(),
            http.connection.connection_id.clone()
        )
    );
    let snapshot: Value = serde_json::from_str(
        &db.query_row(
            "SELECT input_snapshot_json FROM request_tasks WHERE task_id=?1",
            [&task.task_id],
            |r| r.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        snapshot["servicePlan"]["provenance"]["targetLanguage"]["kind"],
        "builtin"
    );
    assert_eq!(
        snapshot["servicePlan"]["provenance"]["targetLanguage"]["field"],
        "defaultTargetLanguageV1"
    );
    assert_eq!(snapshot["cachePolicyVersion"], "typed-cache-v1");
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM attempt_execution_targets WHERE attempt_id=?1",
            [&task.attempt_id],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    let mut forged = task.clone();
    if let ServiceExecutionTarget::Http { plan } = &mut forged.plan.target {
        plan.request.endpoint = "http://127.0.0.1:1/other".into()
    }
    assert!(f.store.validate_service_dispatch(&forged).is_err());
    f.store
        .complete_task(completion(&task, "下雨了。"), 70)
        .unwrap();
    let row: (String, i64, i64) = db
        .query_row(
            "SELECT origin_attempt_id,created_at_ms,expires_at_ms FROM cache_entries",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(row, (task.attempt_id.clone(), 70, 70 + 86_400_000));
    assert!(f.store.validate_service_dispatch(&task).is_err());
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
}
#[test]
fn typed_retry_keeps_exact_target_and_old_attempt_and_stop_late_result_are_rejected() {
    let f = configured(false, 1);
    let unit = manual(&f, "manual", "原文");
    let mut task = prepared(&f, &unit, "run", 60);
    let old = task.clone();
    let retry = f
        .store
        .retry_attempt(
            task.task_id.clone(),
            task.attempt_id.clone(),
            "timeout".into(),
            70,
        )
        .unwrap();
    task.attempt_id = retry.clone();
    f.store.validate_service_dispatch(&task).unwrap();
    assert!(f.store.validate_service_dispatch(&old).is_err());
    assert!(f.store.complete_task(completion(&old, "late"), 71).is_err());
    let db = Connection::open(&f.path).unwrap();
    let targets: u32 = db
        .query_row(
            "SELECT count(*) FROM attempt_execution_targets WHERE task_id=?1",
            [&task.task_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(targets, 2);
    assert_eq!(
        f.store
            .retry_attempt(task.task_id.clone(), retry, "timeout".into(), 72)
            .unwrap_err()
            .code,
        "retry_limit"
    );
    enable(
        &f,
        ServiceScope::Global {},
        CatalogPurpose::Translation,
        false,
        "off",
    );
    assert!(
        f.store
            .complete_task(completion(&task, "late-after-stop"), 80)
            .is_err()
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM translations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM cache_entries", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[test]
fn typed_cache_reuses_valid_producer_and_explicit_retranslation_bypasses_once() {
    let f = configured(true, 0);
    let uid = manual(&f, "input", "同じ文");
    let first = prepared(&f, &uid, "initial", 60);
    f.store
        .complete_task(completion(&first, "cached result"), 70)
        .unwrap();
    let second = prepared(&f, &uid, "cached", 80);
    assert!(
        second
            .cached_result
            .as_ref()
            .is_some_and(|r| r.text == "cached result")
    );
    f.store
        .complete_task(completion(&second, "cached result"), 81)
        .unwrap();
    let refreshed = f
        .store
        .prepare_work_uncached_translation_for_operation(uid.clone(), 1, "fresh".into(), 90)
        .unwrap();
    assert_eq!(refreshed.dispatches.len(), 1);
    let WorkDispatch::Service(task) = &refreshed.dispatches[0] else {
        panic!()
    };
    assert!(task.cached_result.is_none());
    f.store
        .complete_task(completion(task, "new result"), 91)
        .unwrap();
    assert!(
        f.store
            .prepare_work_uncached_translation_for_operation(uid, 1, "fresh".into(), 92)
            .unwrap()
            .dispatches
            .is_empty()
    );
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row("SELECT origin_attempt_id FROM cache_entries", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        task.attempt_id
    );
}
fn game(f: &Fixture, name: &str) -> String {
    command(
        f,
        &format!("game-{name}"),
        Command::SaveGame(Box::new(GameInput {
            game_id: None,
            row_revision: None,
            display_name: name.into(),
            original_title: None,
            source_language: Some("ja".into()),
            cover_asset_ref: None,
            target: GameTarget {
                environment_kind: "crossover".into(),
                environment_key: "crossover:/private/bottle".into(),
                bottle_locator: Some("/private/bottle".into()),
                launch_path: Some(format!("C:\\games\\{name}.exe")),
                canonical_exe_path: Some(format!("C:\\games\\{name}.exe")),
                target_path_key: Some(format!("c:/games/{name}.exe")),
                working_directory: None,
                arguments: vec![],
                architecture: Some("x86".into()),
                verification_state: "verified".into(),
            },
        })),
        100,
    )
    .result_id
    .unwrap()
}
fn game_unit(f: &Fixture, game: &str, suffix: &str) -> String {
    use app_core::domain::capture::*;
    let target = f
        .store
        .snapshot()
        .unwrap()
        .games
        .into_iter()
        .find(|g| g.game_id == game)
        .unwrap()
        .target;
    let session = format!("hook-{suffix}");
    let source_id = format!("source-{suffix}");
    f.store
        .open_hook_session(
            HookSessionInput {
                session_id: session.clone(),
                game_id: Some(game.into()),
                environment_key: target.environment_key,
                target_exe_path: target.canonical_exe_path.unwrap(),
                windows_pid: 100,
                process_created_u64: "1".into(),
                architecture: "x86".into(),
                broker_instance_id: format!("broker-{suffix}"),
                resource: ResourceSnapshotInput {
                    snapshot_id: "fixture-core".into(),
                    kind: "core".into(),
                    version: "test-v1".into(),
                    manifest_sha256: content_hash("test-core"),
                    contract_version: "1".into(),
                    metadata_json: "{}".into(),
                },
                target_identity_sha256: content_hash(game),
            },
            200,
        )
        .unwrap();
    let event = |seq: &str, text: &str| HookTextInput {
        session_id: session.clone(),
        event_id: format!("{session}-{seq}"),
        sequence_u64: seq.into(),
        source: HookSourceInput {
            source_id: source_id.clone(),
            host_instance_id: format!("host-{suffix}"),
            hook_address_u64: "100".into(),
            context_u64: "0".into(),
            subcontext_u64: "0".into(),
            hook_code: "HW...".into(),
            hook_name: "TextOutW".into(),
            encoding: "utf-16le".into(),
            stream_identity_sha256: content_hash(suffix),
            identity_extensions_json: "{}".into(),
        },
        text: text.into(),
        source_language: "ja".into(),
    };
    f.store
        .receive_hook_text(event("1", "candidate"), 201)
        .unwrap();
    command(
        f,
        &format!("source-{suffix}"),
        Command::SelectSource {
            source_id: source_id.clone(),
        },
        202,
    );
    let unit = f
        .store
        .ingest_hook_text(event("2", &format!("{suffix}の文章。")), 203)
        .unwrap()
        .visible_version
        .unwrap();
    f.store
        .flush_hook_capture(
            CaptureFlush {
                session_id: Some(session),
                reason: CaptureFlushReason::Due,
            },
            2000,
        )
        .unwrap();
    unit.unit_id
}
#[test]
fn two_real_game_scopes_keep_inherited_tasks_isolated_through_override_and_restore() {
    let f = configured(false, 0);
    let game_a = game(&f, "first");
    let game_b = game(&f, "second");
    let unit_a = game_unit(&f, &game_a, "first");
    let unit_b = game_unit(&f, &game_b, "second");
    let a = prepared(&f, &unit_a, "inherited-a", 3000);
    let b = prepared(&f, &unit_b, "inherited-b", 3000);
    let db = Connection::open(&f.path).unwrap();
    let snapshot: Value = serde_json::from_str(
        &db.query_row(
            "SELECT input_snapshot_json FROM request_tasks WHERE task_id=?1",
            [&a.task_id],
            |r| r.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        snapshot["selectionAuthority"]["requestedScope"],
        format!("game:{game_a}")
    );
    assert_eq!(snapshot["selectionAuthority"]["requestedRevision"], 0);
    assert_eq!(
        db.query_row(
            "SELECT selection_scope FROM task_execution_targets WHERE task_id=?1",
            [&a.task_id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "global"
    );
    let local = save(&f, CatalogPurpose::Translation, "local", false, 0);
    select(
        &f,
        local,
        ServiceScope::Game {
            game_id: game_a.clone(),
        },
        CatalogPurpose::Translation,
        "override-a",
    );
    assert!(f.store.validate_service_dispatch(&a).is_err());
    assert!(
        f.store
            .complete_task(completion(&a, "late-a"), 3010)
            .is_err()
    );
    f.store.validate_service_dispatch(&b).unwrap();
    let local_task = prepared(&f, &unit_a, "local-a", 3020);
    f.store.validate_service_dispatch(&local_task).unwrap();
    let state = f.store.service_configuration().unwrap();
    let before_global:(String,u32,String)=db.query_row("SELECT config_revision_id,authorization_epoch,config_semantics_sha256 FROM purpose_runtime_states WHERE scope_key='global' AND purpose='translation'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    let projection = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: state.profile_generation,
            scope: ServiceScope::Game {
                game_id: game_a.clone(),
            },
            purpose: CatalogPurpose::Translation,
        })
        .unwrap();
    f.store
        .restore_service_inheritance(
            &RestoreServiceInheritanceInput {
                operation_id: "restore-a".into(),
                expected_profile_generation: state.profile_generation,
                game_id: game_a.clone(),
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: projection.selection.selection_revision,
                expected_applied_head_revision: state.applied_head_revision,
            },
            3030,
        )
        .unwrap();
    assert!(f.store.validate_service_dispatch(&local_task).is_err());
    assert!(
        f.store
            .complete_task(completion(&local_task, "late-local"), 3031)
            .is_err()
    );
    f.store.validate_service_dispatch(&b).unwrap();
    assert_eq!(db.query_row("SELECT config_revision_id,authorization_epoch,config_semantics_sha256 FROM purpose_runtime_states WHERE scope_key='global' AND purpose='translation'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?))).unwrap(),before_global);
    let restored_state = f.store.service_configuration().unwrap();
    let restored_projection = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: restored_state.profile_generation,
            scope: ServiceScope::Game {
                game_id: game_a.clone(),
            },
            purpose: CatalogPurpose::Translation,
        })
        .unwrap();
    let again = f
        .store
        .restore_service_inheritance(
            &RestoreServiceInheritanceInput {
                operation_id: "restore-a-again".into(),
                expected_profile_generation: restored_state.profile_generation,
                game_id: game_a.clone(),
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: restored_projection.selection.selection_revision,
                expected_applied_head_revision: restored_state.applied_head_revision,
            },
            3035,
        )
        .unwrap();
    assert!(!again.semantic_change);
    assert_eq!(
        again.applied_head_revision,
        restored_state.applied_head_revision
    );
    assert_eq!(
        again.selection_revision,
        restored_projection.selection.selection_revision
    );
    assert!(
        !f.store
            .snapshot()
            .unwrap()
            .configuration
            .runtime_purposes
            .iter()
            .any(|p| p.scope_key == format!("game:{game_a}"))
    );
    let inherited = prepared(&f, &unit_a, "inherited-again", 3040);
    assert_eq!(
        inherited.plan.identity.binding_id,
        b.plan.identity.binding_id
    );
    f.store.validate_service_dispatch(&inherited).unwrap();
    f.store
        .complete_task(completion(&b, "second unaffected"), 3050)
        .unwrap();
    f.store
        .complete_task(completion(&inherited, "first restored"), 3051)
        .unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM translations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        2
    );
    let again_source = save(
        &f,
        CatalogPurpose::Translation,
        "local-after-restore",
        false,
        0,
    );
    select(
        &f,
        again_source,
        ServiceScope::Game {
            game_id: game_a.clone(),
        },
        CatalogPurpose::Translation,
        "override-a-again",
    );
    let again_task = prepared(&f, &unit_a, "local-after-restore-run", 3060);
    f.store.validate_service_dispatch(&again_task).unwrap();
}

#[test]
fn real_credential_version_and_lifecycle_are_checked_before_dispatch_and_commit() {
    use app_core::domain::model_config::credentials::{
        CredentialMaterialKind, CredentialStorageMetadata,
    };
    let f = fixture();
    f.store
        .begin_credential_material(
            CredentialReference {
                credential_id: "test-credential".into(),
                credential_version_id: "opaque-test-vault-version".into(),
                pool_id: "test-pool".into(),
                pool_revision: 1,
                label: "synthetic".into(),
                masked_hint: "••".into(),
            },
            CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::ApiKey,
            },
            12,
        )
        .unwrap();
    f.store
        .activate_credential("test-credential".into(), 13)
        .unwrap();
    let source = save_options(
        &f,
        CatalogPurpose::Translation,
        "authenticated",
        false,
        1,
        "builtin.openai-chat",
        ServiceAuthenticationSelection::ExistingPool {
            pool_id: "test-pool".into(),
            pool_revision: std::num::NonZeroU32::new(1).unwrap(),
        },
        vec![],
    );
    select(
        &f,
        source,
        ServiceScope::Global {},
        CatalogPurpose::Translation,
        "select-auth",
    );
    enable(
        &f,
        ServiceScope::Global {},
        CatalogPurpose::Translation,
        true,
        "enable-auth",
    );
    let uid = manual(&f, "auth-input", "認証の文章。");
    let task = prepared(&f, &uid, "auth-run", 60);
    assert_eq!(
        task.credential_vault_ref.as_deref(),
        Some("opaque-test-vault-version")
    );
    f.store.validate_service_dispatch(&task).unwrap();
    let db = Connection::open(&f.path).unwrap();
    let stored: String = db
        .query_row(
            "SELECT input_snapshot_json FROM request_tasks WHERE task_id=?1",
            [&task.task_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!stored.contains("opaque-test-vault-version"));
    let fact: Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(
        fact["credentialAuthority"]["credentialId"],
        "test-credential"
    );
    assert_eq!(fact["credentialAuthority"]["envelopeSchema"], 1);
    assert_eq!(
        db.query_row(
            "SELECT credential_id FROM request_attempts WHERE attempt_id=?1",
            [&task.attempt_id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "test-credential"
    );
    for index in 0..8 {
        let mut wrong = task.clone();
        match index {
            0 => wrong.credential_vault_ref = Some("different-version".into()),
            1 => wrong.profile_generation += 1,
            2 => wrong.authorization_epoch += 1,
            3 => wrong.server_instance_id = "other-server".into(),
            4 => wrong.request_semantics_hash = content_hash("other-request"),
            5 => wrong.plan.identity.engine_id = "other-engine".into(),
            6 => wrong.unit.source_text = "forged text with unchanged hash".into(),
            _ => wrong.unit.source_hash = content_hash("other-source"),
        }
        assert!(
            f.store.validate_service_dispatch(&wrong).is_err(),
            "identity component {index}"
        );
    }
    f.store
        .revoke_credential("test-credential".into(), 70)
        .unwrap();
    assert!(f.store.validate_service_dispatch(&task).is_err());
    assert!(
        f.store
            .retry_attempt(
                task.task_id.clone(),
                task.attempt_id.clone(),
                "timeout".into(),
                71
            )
            .is_err()
    );
    assert!(
        f.store
            .complete_task(completion(&task, "late revoked result"), 72)
            .is_err()
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM translations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM execution_authorization_leases",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn fallback_is_saved_but_not_advertised_as_executable_without_a_real_router() {
    let f = fixture();
    let fallback = save(&f, CatalogPurpose::Translation, "fallback", false, 0);
    let primary = save_options(
        &f,
        CatalogPurpose::Translation,
        "primary",
        false,
        0,
        "builtin.lmstudio",
        ServiceAuthenticationSelection::None {},
        vec![(
            "fallback".into(),
            CatalogScalar::Text(serde_json::to_string(&vec![fallback]).unwrap()),
        )],
    );
    let state = f.store.service_configuration().unwrap();
    let catalog = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: state.profile_generation,
            scope: ServiceScope::Global {},
            purpose: CatalogPurpose::Translation,
        })
        .unwrap();
    let entry = catalog.rows.iter().find(|s| s.source == primary).unwrap();
    assert!(format!("{:?}", entry.eligibility).contains("service_fallback_not_ready"));
    assert_eq!(
        Connection::open(&f.path)
            .unwrap()
            .query_row("SELECT count(*) FROM request_tasks", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn fixed_cache_is_semantic_across_units_rejects_corruption_and_obeys_policy_identity() {
    let f = configured(true, 0);
    let first_uid = manual(&f, "first-input", "同じ文章。");
    let first = prepared(&f, &first_uid, "first-run", 60);
    f.store
        .complete_task(completion(&first, "derived value"), 70)
        .unwrap();
    assert_eq!(
        f.store
            .prepare_work_uncached_translation_for_operation(
                first_uid.clone(),
                1,
                "first-run".into(),
                71
            )
            .err()
            .expect("operation cache policy mismatch")
            .code,
        "operation_conflict"
    );
    let second_uid = manual(&f, "second-input", "同じ文章。");
    assert_ne!(first_uid, second_uid);
    let second = prepared(&f, &second_uid, "second-run", 80);
    assert_eq!(second.request_semantics_hash, first.request_semantics_hash);
    assert_eq!(second.cached_result.as_ref().unwrap().text, "derived value");
    f.store
        .complete_task(completion(&second, "derived value"), 81)
        .unwrap();
    let db = Connection::open(&f.path).unwrap();
    db.execute("UPDATE cache_entries SET result_json='{}'", [])
        .unwrap();
    let third = prepared(&f, &second_uid, "third-run", 90);
    assert!(third.cached_result.is_none());
    f.store
        .complete_task(completion(&third, "new checked value"), 91)
        .unwrap();
    let expired = prepared(&f, &second_uid, "expired-run", 91 + 86_400_000);
    assert!(expired.cached_result.is_none());
}
#[test]
fn capture_and_study_entrypoints_use_real_target_admission_and_reject_invalid_scope() {
    use app_core::domain::capture::CaptureVersion;
    let f = configured(false, 0);
    let manual_id = manual(&f, "manual-only", "手入力。");
    assert!(
        f.store
            .prepare_work_capture_tasks(
                CaptureVersion {
                    unit_id: manual_id.clone(),
                    text_revision: 1
                },
                "not-hook".into(),
                2000
            )
            .unwrap()
            .is_empty()
    );
    let g = game(&f, "hook-game");
    let hook_id = game_unit(&f, &g, "captured");
    let capture = f
        .store
        .prepare_work_capture_tasks(
            CaptureVersion {
                unit_id: hook_id.clone(),
                text_revision: 1,
            },
            "capture".into(),
            3000,
        )
        .unwrap();
    assert_eq!(capture.len(), 1);
    let WorkDispatch::Service(task) = &capture[0] else {
        panic!("typed capture")
    };
    f.store.validate_service_dispatch(task).unwrap();
    assert_eq!(task.unit.game_id.as_deref(), Some(g.as_str()));
    let grammar = save(&f, CatalogPurpose::Grammar, "grammar", false, 0);
    select(
        &f,
        grammar,
        ServiceScope::Global {},
        CatalogPurpose::Grammar,
        "select-grammar",
    );
    enable(
        &f,
        ServiceScope::Global {},
        CatalogPurpose::Grammar,
        true,
        "enable-grammar",
    );
    let studies = f
        .store
        .prepare_work_study_tasks(
            manual_id.clone(),
            1,
            "grammar".into(),
            None,
            "study".into(),
            3001,
        )
        .unwrap();
    assert_eq!(studies.len(), 1);
    let WorkDispatch::Service(study) = &studies[0] else {
        panic!("typed grammar")
    };
    assert_eq!(study.plan.identity.purpose, CatalogPurpose::Grammar);
    f.store.validate_service_dispatch(study).unwrap();
    assert!(
        f.store
            .prepare_work_study_tasks(
                manual_id.clone(),
                0,
                "grammar".into(),
                None,
                "bad-version".into(),
                3002
            )
            .is_err()
    );
    assert!(
        f.store
            .prepare_work_study_tasks(
                manual_id,
                1,
                "translation".into(),
                None,
                "bad-purpose".into(),
                3002
            )
            .is_err()
    );
    // An unrelated grammar selection keeps the already admitted capture valid.
    f.store.validate_service_dispatch(task).unwrap();
}
#[test]
fn deleting_a_completed_producer_collects_its_derived_cache_before_parent_history() {
    use app_core::domain::retention::{ProtectedObjects, RetentionPolicy};
    let f = configured(true, 0);
    let uid = manual(&f, "retention-input", "保持された文章。");
    let task = prepared(&f, &uid, "retention-run", 60);
    f.store
        .complete_task(completion(&task, "retained derived value"), 70)
        .unwrap();
    let other_uid = manual(&f, "other-retention-input", "別の保持された文章。");
    let other = prepared(&f, &other_uid, "other-retention-run", 71);
    f.store
        .complete_task(completion(&other, "unrelated derived value"), 72)
        .unwrap();
    let policy = RetentionPolicy {
        max_rows_per_batch: 100,
        history_unit_ids: vec![uid.clone()],
        ..Default::default()
    };
    let db = Connection::open(&f.path).unwrap();
    assert!(
        f.store
            .maintain_storage(
                policy.clone(),
                ProtectedObjects {
                    unit_ids: vec![uid.clone()],
                    ..Default::default()
                },
                "protect-explicit-history".into(),
                73
            )
            .is_err()
    );
    let running = prepared(&f, &uid, "running-protected", 74);
    assert!(
        f.store
            .maintain_storage(
                policy.clone(),
                Default::default(),
                "reject-active-history".into(),
                75
            )
            .is_err()
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM cache_entries", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        2
    );
    f.store
        .complete_task(completion(&running, "retained derived value"), 76)
        .unwrap();
    let result = f
        .store
        .maintain_storage(
            policy,
            Default::default(),
            "delete-explicit-history".into(),
            80,
        )
        .unwrap();
    assert_eq!(result.removed_history_units, 1);
    assert_eq!(result.removed_cache_entries, 1);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM reading_units WHERE unit_id=?1",
            [uid],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT origin_attempt_id FROM cache_entries", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        other.attempt_id
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
}
#[test]
fn legacy_real_cache_producer_is_collected_without_changing_legacy_cache_policy() {
    let f = fixture();
    let config:Configuration=serde_json::from_value(json!({"revisionId":"","connections":[{"connectionId":"legacy-c","displayName":"Legacy Connection","protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:1/v1","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":true}],"models":[{"modelId":"legacy-m","connectionId":"legacy-c","actualModelId":"controlled-model","displayName":"Legacy Model","selectionSource":"manual","capabilities":{},"parameters":{}}],"bindings":[{"bindingId":"legacy-b","scopeKey":"global","purpose":"translation","bindingMode":"override","enabled":true,"outputMode":"text","sourceLanguage":"ja","targetLanguage":"zh-Hans","context":{"enabled":false},"schedule":{"cacheTtlMs":100000},"parameters":{},"engines":[{"engineId":"legacy-e","modelId":"legacy-m","enabled":true,"parameters":{}}]}],"templates":[],"termSets":[]})).unwrap();
    let saved = command(
        &f,
        "legacy-save",
        Command::SaveConfiguration {
            configuration: config,
            expected_head_revision: 0,
        },
        20,
    );
    command(
        &f,
        "legacy-apply",
        Command::ApplyConfiguration {
            revision_id: saved.result_id.unwrap(),
            expected_head_revision: 0,
        },
        21,
    );
    let uid = manual(&f, "legacy-retention-input", "古い実行契約。");
    let work = f
        .store
        .prepare_work_tasks_for_operation(
            uid.clone(),
            1,
            "translation".into(),
            "legacy-retention-run".into(),
            60,
        )
        .unwrap();
    let WorkDispatch::Legacy(task) = &work[0] else {
        panic!("actual legacy binding")
    };
    f.store
        .complete_task(
            TaskCompletion {
                task_id: task.task_id.clone(),
                attempt_id: task.attempt_id.clone(),
                text: "legacy derived".into(),
                output_mode: "text".into(),
                structured: json!({}),
                target_language: "zh-Hans".into(),
                provider_label: "Legacy Connection".into(),
            },
            70,
        )
        .unwrap();
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT origin_attempt_id,expires_at_ms-created_at_ms FROM cache_entries",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        )
        .unwrap(),
        (task.attempt_id.clone(), 100000)
    );
    let result = f
        .store
        .maintain_storage(
            app_core::domain::retention::RetentionPolicy {
                max_rows_per_batch: 100,
                history_unit_ids: vec![uid],
                ..Default::default()
            },
            Default::default(),
            "delete-legacy-history".into(),
            80,
        )
        .unwrap();
    assert_eq!(result.removed_cache_entries, 1);
    assert_eq!(result.removed_history_units, 1);
    assert_eq!(
        db.query_row("SELECT count(*) FROM cache_entries", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
}
