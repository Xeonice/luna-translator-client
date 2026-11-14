//! Actual Gateway/Broker/SQLite composition facts, not remote account acceptance.
use app_core::{
    contracts::*,
    domain::model_config::{
        credentials::*, service_draft::compile_service_draft, service_graph::*,
    },
    ports::{
        ProductRepository, ServiceConfigurationRepository, google_authorization::*,
        runtime::CredentialVault,
    },
};
use app_infra::{
    platform::SystemIdentity,
    providers::{
        builtin_registry,
        gateway::HttpModelGateway,
        google_authorization::{GoogleAuthorizationService, SystemGoogleClock},
        google_token::FixedGoogleTokenTransport,
    },
    storage::SqliteStore,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
struct NoTests;
impl GoogleTestOperationAuthority for NoTests {
    fn check(&self, _: &GoogleLeaseRequest, _: &GoogleTestOperation) -> AppResult<()> {
        Err(AppError::new("no_test", "test", "no operation"))
    }
}
struct NoVault;
impl CredentialVault for NoVault {
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        panic!("runtime registration must not read Vault")
    }
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        panic!("runtime registration must not write Vault")
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        panic!("runtime registration must not delete Vault")
    }
}
struct Fixture {
    store: Arc<SqliteStore>,
    path: PathBuf,
    source: SavedServiceRef,
}
fn setup(profile: &str, server: &str) -> Fixture {
    let path =
        std::env::temp_dir().join(format!("google-handler-{}.sqlite3", uuid::Uuid::new_v4()));
    let store = Arc::new(SqliteStore::open(&path, profile, server, 10).unwrap());
    let directory = Arc::new(builtin_registry().unwrap());
    store
        .configure_service_directory(directory.clone())
        .unwrap();
    store
        .begin_credential_material(
            CredentialReference {
                credential_id: "metadata-only-sa".into(),
                credential_version_id: "opaque-not-a-real-sa-key".into(),
                pool_id: "sa-pool".into(),
                pool_revision: 1,
                label: "Synthetic SA metadata".into(),
                masked_hint: "metadata".into(),
            },
            CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::GoogleServiceAccount,
            },
            11,
        )
        .unwrap();
    store
        .activate_credential("metadata-only-sa".into(), 12)
        .unwrap();
    let state = store.service_configuration().unwrap();
    let input = SaveServiceDraftInput {
        operation_id: "save-source".into(),
        expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: state.draft_head_revision,
        editing: None,
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: "builtin.google-v3".into(),
                preset_revision: 1,
            },
        },
        purpose: CatalogPurpose::Translation,
        values: BTreeMap::from([
            (
                "name".into(),
                CatalogScalar::Text("Google SA metadata".into()),
            ),
            (
                "google_auth_method".into(),
                CatalogScalar::Text("service_account".into()),
            ),
            (
                "project_id".into(),
                CatalogScalar::Text("synthetic-project".into()),
            ),
            ("location".into(), CatalogScalar::Text("global".into())),
            (
                "model".into(),
                CatalogScalar::Text(
                    "projects/synthetic-project/locations/global/models/general/nmt".into(),
                ),
            ),
        ]),
        authentication: ServiceAuthenticationSelection::ExistingPool {
            pool_id: "sa-pool".into(),
            pool_revision: std::num::NonZeroU32::new(1).unwrap(),
        },
    };
    let compiled = compile_service_draft(
        &state.draft,
        &input,
        directory.as_ref(),
        &SystemIdentity,
        &Default::default(),
    )
    .unwrap();
    let source = store
        .save_service_configuration(&compiled, 20)
        .unwrap()
        .saved_resource;
    Fixture {
        store,
        path,
        source,
    }
}
fn owner(f: &Fixture) -> Arc<GoogleAuthorizationService> {
    let tests = Arc::new(NoTests);
    Arc::new(GoogleAuthorizationService::new(
        f.store.google_authorization_repository(tests.clone()),
        Arc::new(NoVault),
        Arc::new(FixedGoogleTokenTransport::new().unwrap()),
        Arc::new(SystemGoogleClock),
        tests,
    ))
}
fn install(f: &Fixture, owner: Arc<GoogleAuthorizationService>) -> AppResult<HttpModelGateway> {
    HttpModelGateway::new(f.store.clone(), Arc::new(NoVault)).with_google_authorization(owner)
}
fn row(f: &Fixture) -> SavedServiceRow {
    f.store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: f
                .store
                .service_configuration()
                .unwrap()
                .profile_generation,
            scope: ServiceScope::Global {},
            purpose: CatalogPurpose::Translation,
        })
        .unwrap()
        .rows
        .into_iter()
        .find(|r| r.source == f.source)
        .unwrap()
}
fn assert_not_ready(f: &Fixture) {
    assert!(
        matches!(row(f).eligibility,SavedServiceEligibility::Blocked{reasons} if reasons.iter().any(|r|r.code=="google_authorization_not_ready"))
    )
}
fn assert_ready(f: &Fixture) {
    assert!(matches!(
        row(f).eligibility,
        SavedServiceEligibility::Selectable {}
    ))
}
#[test]
fn metadata_or_constructed_broker_never_stands_in_for_an_installed_handler() {
    let f = setup("only-metadata", "server");
    assert_not_ready(&f);
    let broker = owner(&f);
    assert_not_ready(&f);
    let gateway = install(&f, broker.clone()).unwrap();
    assert_ready(&f);
    drop(gateway);
    assert_not_ready(&f);
    assert!(
        Arc::strong_count(&broker) >= 1,
        "the Broker is intentionally still alive"
    );
}
#[test]
fn gateway_clones_and_same_owner_compositions_share_one_handler_registration() {
    let f = setup("clone-profile", "server");
    let broker = owner(&f);
    let one = install(&f, broker.clone()).unwrap();
    let clone = one.clone();
    let another = install(&f, broker.clone()).unwrap();
    assert_ready(&f);
    drop(one);
    drop(clone);
    assert_ready(&f);
    drop(another);
    assert_not_ready(&f);
}
#[test]
fn a_distinct_live_owner_cannot_replace_the_installed_handler() {
    let f = setup("live-owner", "server");
    let first = install(&f, owner(&f)).unwrap();
    let second = owner(&f);
    assert_eq!(
        install(&f, second.clone()).err().unwrap().code,
        "google_handler_already_installed"
    );
    assert_ready(&f);
    drop(first);
    let next = install(&f, second).unwrap();
    assert_ready(&f);
    drop(next);
    assert_not_ready(&f);
}
#[test]
fn foreign_profile_even_with_the_same_server_and_generation_is_rejected() {
    let a = setup("profile-a", "same-server");
    let b = setup("profile-b", "same-server");
    let broker = owner(&a);
    assert_eq!(
        install(&b, broker.clone()).err().unwrap().code,
        "google_handler_frame_mismatch"
    );
    assert_not_ready(&a);
    assert_not_ready(&b);
    let installed = install(&a, broker).unwrap();
    assert_ready(&a);
    assert_not_ready(&b);
    drop(installed);
}
#[test]
fn stale_generation_or_server_registration_cannot_be_reused() {
    for column in [
        "runtime_generation=runtime_generation+1",
        "server_instance_id='next-server'",
    ] {
        let f = setup("epoch-profile", "initial-server");
        let broker = owner(&f);
        let gateway = install(&f, broker.clone()).unwrap();
        assert_ready(&f);
        rusqlite::Connection::open(&f.path)
            .unwrap()
            .execute(
                &format!("UPDATE profile_meta SET {column} WHERE singleton=1"),
                [],
            )
            .unwrap();
        assert_not_ready(&f);
        assert_eq!(
            install(&f, broker).err().unwrap().code,
            if column.starts_with("server_instance_id") {
                "google_handler_frame_mismatch"
            } else {
                "google_handler_already_installed"
            }
        );
        drop(gateway);
    }
}
#[test]
fn closed_profile_refuses_new_handler_composition() {
    let f = setup("closed-profile", "server");
    let broker = owner(&f);
    let gateway = install(&f, broker.clone()).unwrap();
    rusqlite::Connection::open(&f.path)
        .unwrap()
        .execute(
            "UPDATE profile_meta SET runtime_state='inactive' WHERE singleton=1",
            [],
        )
        .unwrap();
    assert!(install(&f, broker).is_err());
    drop(gateway);
}
#[test]
fn raw_foreign_registration_inputs_do_not_install_a_handler() {
    let f = setup("raw-frame", "server");
    let repo = f.store.google_authorization_repository(Arc::new(NoTests));
    let actual = f.store.google_runtime_frame().unwrap();
    for part in ["profile", "generation", "server"] {
        let mut wrong = actual.clone();
        match part {
            "profile" => wrong.profile_id = "foreign".into(),
            "generation" => {
                wrong.profile_generation =
                    std::num::NonZeroU32::new(actual.profile_generation.get() + 1).unwrap()
            }
            _ => wrong.server_instance_id = "foreign".into(),
        };
        let temporary = Arc::new(());
        assert_eq!(
            repo.register_google_handler(wrong, Arc::downgrade(&temporary))
                .err()
                .unwrap()
                .code,
            "google_handler_frame_mismatch"
        );
        assert_not_ready(&f);
    }
}

#[test]
fn matching_labels_do_not_substitute_for_the_same_actual_writer() {
    let a = setup("same-profile", "same-server");
    let b = setup("same-profile", "same-server");
    let af = a.store.google_runtime_frame().unwrap();
    let bf = b.store.google_runtime_frame().unwrap();
    assert_eq!(af.profile_id, bf.profile_id);
    assert_eq!(af.profile_generation, bf.profile_generation);
    assert_eq!(af.server_instance_id, bf.server_instance_id);
    assert_ne!(
        af, bf,
        "distinct Database actors have distinct opaque identities"
    );
    assert_eq!(
        install(&a, owner(&b)).err().unwrap().code,
        "google_handler_frame_mismatch"
    );
    assert_not_ready(&a);
    assert_not_ready(&b);
    let clone = (*a.store).clone();
    assert_eq!(af, clone.google_runtime_frame().unwrap());
    let broker = Arc::new(GoogleAuthorizationService::new(
        clone.google_authorization_repository(Arc::new(NoTests)),
        Arc::new(NoVault),
        Arc::new(FixedGoogleTokenTransport::new().unwrap()),
        Arc::new(SystemGoogleClock),
        Arc::new(NoTests),
    ));
    let gateway = install(&a, broker).unwrap();
    assert_ready(&a);
    assert_not_ready(&b);
    drop(gateway);
    assert_not_ready(&a);
}

fn enable(f: &Fixture) {
    let state = f.store.service_configuration().unwrap();
    let query = SavedServiceQuery {
        expected_profile_generation: state.profile_generation,
        scope: ServiceScope::Global {},
        purpose: CatalogPurpose::Translation,
    };
    let selection = f.store.saved_services(&query).unwrap().selection;
    f.store
        .select_saved_services(
            &SelectSavedServicesInput {
                operation_id: "handler-select".into(),
                expected_profile_generation: state.profile_generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: selection.selection_revision,
                expected_applied_head_revision: state.applied_head_revision,
                main: f.source.clone(),
                comparisons: vec![],
            },
            30,
        )
        .unwrap();
    let state = f.store.service_configuration().unwrap();
    let selection = f.store.saved_services(&query).unwrap().selection;
    f.store
        .set_service_purpose_enabled(
            &SetServicePurposeEnabledInput {
                operation_id: "handler-enable".into(),
                expected_profile_generation: state.profile_generation,
                scope: ServiceScope::Global {},
                purpose: CatalogPurpose::Translation,
                expected_selection_revision: selection.selection_revision,
                expected_applied_head_revision: state.applied_head_revision,
                enabled: true,
            },
            31,
        )
        .unwrap();
}
fn input(f: &Fixture, operation: &str, text: &str) -> String {
    f.store
        .dispatch(
            CommandEnvelope {
                operation_id: operation.into(),
                expected_revision: None,
                command: Command::ManualInput {
                    text: text.into(),
                    source_language: "ja".into(),
                },
            },
            32,
        )
        .unwrap()
        .result_id
        .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn last_handler_drop_blocks_real_existing_and_new_task_retry_commit_and_mint_authority() {
    use app_core::domain::{
        TaskCompletion, service_dispatch::WorkDispatch, service_execution::ServiceExecutionTarget,
    };
    let f = setup("task-handler", "server");
    let broker = owner(&f);
    let gateway = install(&f, broker.clone()).unwrap();
    enable(&f);
    let unit = input(&f, "first-unit", "雨がやんだ。");
    let mut work = f
        .store
        .prepare_work_tasks_for_operation(unit, 1, "translation".into(), "first-task".into(), 33)
        .unwrap();
    let WorkDispatch::Service(task) = work.remove(0) else {
        panic!("actual typed task required")
    };
    f.store.validate_service_dispatch(&task).unwrap();
    f.store
        .validate_task_authority(task.task_id.clone())
        .unwrap();
    let ServiceExecutionTarget::Http { plan } = &task.plan.target else {
        panic!()
    };
    let request = GoogleLeaseRequest {
        target: GoogleLeaseTarget::Attempt {
            attempt_id: task.attempt_id.clone(),
            task_id: task.task_id.clone(),
        },
        config_revision_id: task.plan.identity.configuration_revision.clone(),
        connection_id: plan.connection.connection_id.clone(),
        target_policy_sha256: google_target_policy_sha256(&plan.connection).unwrap(),
        profile_generation: std::num::NonZeroU32::new(task.profile_generation).unwrap(),
        server_instance_id: task.server_instance_id.clone(),
    };
    let authorization = f.store.google_authorization_repository(Arc::new(NoTests));
    authorization.lease_authority(&request).unwrap();
    drop(gateway);
    assert_not_ready(&f);
    let old_dispatch = f.store.validate_service_dispatch(&task);
    assert!(old_dispatch.is_err());
    assert_eq!(
        old_dispatch.err().unwrap().code,
        "service_runtime_unavailable"
    );
    let old_task = f.store.validate_task_authority(task.task_id.clone());
    assert!(old_task.is_err());
    assert_eq!(old_task.err().unwrap().code, "service_runtime_unavailable");
    let old_mint = authorization.lease_authority(&request);
    assert!(old_mint.is_err());
    assert_eq!(old_mint.err().unwrap().code, "service_runtime_unavailable");
    assert!(
        broker
            .lease(
                request,
                GoogleBudget::new(std::time::Duration::from_secs(5)).unwrap()
            )
            .await
            .is_err()
    );
    assert!(
        f.store
            .retry_attempt(
                task.task_id.clone(),
                task.attempt_id.clone(),
                "network_error".into(),
                34
            )
            .is_err()
    );
    assert!(
        f.store
            .complete_task(
                TaskCompletion {
                    task_id: task.task_id.clone(),
                    attempt_id: task.attempt_id,
                    text: "late controlled result".into(),
                    output_mode: "text".into(),
                    structured: serde_json::json!({}),
                    target_language: "zh-Hans".into(),
                    provider_label: "not sent".into()
                },
                35
            )
            .is_err()
    );
    let unit = input(&f, "second-unit", "空が明るい。");
    assert!(
        f.store
            .prepare_work_tasks_for_operation(
                unit,
                1,
                "translation".into(),
                "second-task".into(),
                36
            )
            .is_err()
    );
    let db = rusqlite::Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM request_tasks", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM translations", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM execution_authorization_leases",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}
#[derive(Default)]
struct CountingVault(std::sync::atomic::AtomicUsize);
impl CredentialVault for CountingVault {
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(AppError::new("not_found", "test", "no material supplied"))
    }
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        panic!("must not write")
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        panic!("must not delete")
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_gateway_cannot_use_new_profile_test_frame_to_read_vault() {
    use app_core::ports::{
        operation_budget::OperationBudget, runtime::ModelGateway,
        service_test_execution::ServiceTestExecutionObservation, service_testing::*,
    };
    let f = setup("stale-test", "server");
    let live = Arc::new(
        app_core::application::google_test_operations::LiveServiceTestOperations::default(),
    );
    let vault = Arc::new(CountingVault::default());
    let broker = Arc::new(GoogleAuthorizationService::new(
        f.store.google_authorization_repository(live.clone()),
        vault.clone(),
        Arc::new(FixedGoogleTokenTransport::new().unwrap()),
        Arc::new(SystemGoogleClock),
        live.clone(),
    ));
    let gateway = HttpModelGateway::new(f.store.clone(), vault.clone())
        .with_google_authorization(broker)
        .unwrap();
    rusqlite::Connection::open(&f.path)
        .unwrap()
        .execute(
            "UPDATE profile_meta SET runtime_generation=runtime_generation+1 WHERE singleton=1",
            [],
        )
        .unwrap();
    assert_not_ready(&f);
    let tests = f
        .store
        .service_testing_repository(live.clone(), live.clone())
        .unwrap();
    let prepared = tests
        .prepare_service_test(
            PrepareServiceTestInput {
                operation_id: "new-generation-test".into(),
                request: SavedServiceTestRequest {
                    expected_profile_generation: f
                        .store
                        .service_configuration()
                        .unwrap()
                        .profile_generation,
                    source: f.source.clone(),
                    purpose: CatalogPurpose::Translation,
                    input_scope: None,
                    output_mode: None,
                },
                selected_token: None,
            },
            30,
        )
        .unwrap();
    let budget = OperationBudget::new(std::time::Duration::from_secs(5)).unwrap();
    let guard = live.register_service(prepared, budget.clone()).unwrap();
    let observed = Arc::new(ServiceTestExecutionObservation::default());
    assert!(
        gateway
            .execute_saved_service_test(guard.prepared(), tests, budget, observed.clone())
            .await
            .is_err()
    );
    observed.wait_for_physical_drain().await.unwrap();
    assert_eq!(vault.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!observed.snapshot().unwrap().credential_used);
}
#[test]
fn observer_registration_arc_does_not_prolong_or_revive_a_handler_lifetime() {
    // Core lifetime invariant only; this handle is not registered as a product runtime.
    let f = setup("lifetime-primitive", "server");
    let owner = Arc::new(());
    let handle = GoogleHandlerHandle::new(
        f.store.google_runtime_frame().unwrap(),
        Arc::downgrade(&owner),
    )
    .unwrap();
    let observer = handle.registration().clone();
    let actual_clone = handle.clone();
    drop(handle);
    assert!(observer.is_live());
    drop(actual_clone);
    assert!(!observer.is_live());
    assert_eq!(
        observer.acquire().err().unwrap().code,
        "google_handler_closed"
    );
    assert!(Arc::strong_count(&owner) > 0);
}
