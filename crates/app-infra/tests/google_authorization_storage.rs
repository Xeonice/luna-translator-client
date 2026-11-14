//! Real Store4 transactions. Synthetic clients/material metadata only: no Google
//! account, browser, remote token service or product acceptance is claimed.
use app_core::{
    contracts::{AppError, AppResult},
    domain::model_config::{credentials::*, service_graph::*},
    ports::{ProductRepository, google_authorization::*},
};
use app_infra::storage::SqliteStore;
use rusqlite::Connection;
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    path::PathBuf,
    sync::{Arc, Mutex},
};
fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
#[derive(Default)]
struct Operations {
    admitted: Mutex<BTreeMap<String, GoogleLeaseRequest>>,
    sources: Mutex<BTreeMap<String, GoogleServiceAccountLeaseIdentity>>,
}
impl GoogleTestOperationAuthority for Operations {
    fn check(&self, r: &GoogleLeaseRequest, op: &GoogleTestOperation) -> AppResult<()> {
        if self.admitted.lock().unwrap().get(&op.operation_id) == Some(r) {
            Ok(())
        } else {
            Err(AppError::new(
                "test_operation_not_admitted",
                "controlled_registry",
                "Live operation absent",
            ))
        }
    }
    fn service_account_source(
        &self,
        r: &GoogleLeaseRequest,
        op: &GoogleTestOperation,
    ) -> AppResult<GoogleServiceAccountLeaseIdentity> {
        self.check(r, op)?;
        self.sources
            .lock()
            .unwrap()
            .get(&op.operation_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "test_source_not_admitted",
                    "controlled_registry",
                    "Live source absent",
                )
            })
    }
}
struct Fixture {
    store: SqliteStore,
    path: PathBuf,
    repo: Arc<dyn GoogleAuthorizationRepository>,
    ops: Arc<Operations>,
    generation: NonZeroU32,
    server: String,
    login: GoogleLoginRequest,
    actual_google_handler: Mutex<Option<app_infra::providers::gateway::HttpModelGateway>>,
}
fn client() -> GoogleClientProfile {
    GoogleClientProfile {
        key: GoogleClientKey {
            client_profile_id: "synthetic-client".into(),
            client_profile_revision: nz(1),
        },
        public_client_id: "synthetic-desktop-client-id".into(),
        client_mode: GoogleClientMode::Custom,
        policy_sha256: "a".repeat(64),
        setup_state: GoogleClientSetup::Verified,
    }
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("google-storage-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.sqlite3");
        let server = "synthetic-server".to_string();
        let store = SqliteStore::open(&path, "google-profile", &server, 10).unwrap();
        let generation = nz(Connection::open(&path)
            .unwrap()
            .query_row("SELECT runtime_generation FROM profile_meta", [], |r| {
                r.get(0)
            })
            .unwrap());
        let ops = Arc::new(Operations::default());
        store
            .configure_service_directory(Arc::new(
                app_infra::providers::builtin_registry().unwrap(),
            ))
            .unwrap();
        let repo = store.google_authorization_repository(ops.clone());
        let login = GoogleLoginRequest {
            operation_id: format!("login-{}", uuid::Uuid::new_v4()),
            client: client().key,
            profile_generation: generation,
            server_instance_id: server.clone(),
            network_policy: GoogleNetworkPolicy::Disabled,
        };
        store
            .register_google_client_profile(login.clone(), client(), 11)
            .unwrap();
        Self {
            store,
            path,
            repo,
            ops,
            generation,
            server,
            login,
            actual_google_handler: Mutex::new(None),
        }
    }
    /// Positive runtime-admission cases explicitly install and retain the real
    /// handler. This does not authenticate the synthetic account or read Vault.
    fn setup_actual_google_handler(&self) {
        use app_infra::providers::{gateway::HttpModelGateway, google_authorization::{GoogleAuthorizationService,SystemGoogleClock},google_token::FixedGoogleTokenTransport};
        let vault = Arc::new(MemoryVault::default());
        let owner = Arc::new(GoogleAuthorizationService::new(self.repo.clone(), vault.clone(), Arc::new(FixedGoogleTokenTransport::new().unwrap()), Arc::new(SystemGoogleClock), self.ops.clone()));
        let handler = HttpModelGateway::new(Arc::new(self.store.clone()), vault).with_google_authorization(owner).unwrap();
        let mut installed = self.actual_google_handler.lock().unwrap();
        assert!(installed.is_none(), "positive fixture must not replace a live handler");
        *installed = Some(handler);
    }
    fn db(&self) -> Connection {
        let c = Connection::open(&self.path).unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        c
    }
    fn pending(&self) -> GooglePendingLogin {
        self.repo.prepare_login(&self.login, 12).unwrap()
    }
    fn activate(&self) -> (GoogleOwnedGrant, GooglePendingMaterial) {
        let p = self.pending();
        let m = self
            .repo
            .reserve_material(&GoogleMaterialContext::InitialLogin(p), 13)
            .unwrap();
        let grant = self
            .repo
            .commit_material(&GoogleMaterialCommit {
                pending: m.clone(),
                transition: GoogleMaterialTransition::InitialLogin,
                scopes_sha256: google_scope_hash(),
                committed_at_ms: 14,
            })
            .unwrap();
        (grant, m)
    }
    fn control(&self, g: &GoogleOwnedGrant) -> GoogleGrantControlRequest {
        GoogleGrantControlRequest {
            operation_id: format!("control-{}", uuid::Uuid::new_v4()),
            expected: g.cursor.clone(),
            profile_generation: self.generation,
            server_instance_id: self.server.clone(),
            network_policy: GoogleNetworkPolicy::Disabled,
        }
    }
    fn saved_connection(&self, grant: &GoogleOwnedGrant) -> GoogleLeaseRequest {
        let graph:ConfigurationV3=serde_json::from_value(serde_json::json!({"codecVersion":3,"revisionId":format!("google-config-{}",uuid::Uuid::new_v4()),"connections":[{"connectionId":"google-connection","displayName":"Google Advanced","protocol":"google_translate_v3","deployment":"cloud","authMode":"google_oauth","endpoint":"https://translation.googleapis.com","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":false,"adapterBinding":{"protocol":{"protocolId":"google_translate_v3","contractRevision":1},"profile":{"profileId":"google-v3-text","profileRevision":1}},"httpOptions":{"protocol":"google_translate_v3","projectId":"synthetic-project","location":"global"},"origin":{"kind":"custom"},"googleAuthorization":{"grantId":grant.cursor.grant_id,"authorizationEpoch":grant.cursor.authorization_epoch.unwrap().get()}}],"models":[],"bindings":[],"templates":[],"termSets":[],"offlineEngineConfigs":[]})).unwrap();
        let revision = self
            .store
            .persist_service_graph_revision(graph.clone(), None, 20)
            .unwrap();
        let target = GoogleTestOperation {
            operation_id: format!("test-{}", uuid::Uuid::new_v4()),
            input_scope: GoogleTestInputScope::Sample,
            input_sha256: "b".repeat(64),
            request_semantics_sha256: "c".repeat(64),
        };
        let r = GoogleLeaseRequest {
            target: GoogleLeaseTarget::TestOperation(target.clone()),
            config_revision_id: revision,
            connection_id: "google-connection".into(),
            target_policy_sha256: google_target_policy_sha256(&graph.connections[0]).unwrap(),
            profile_generation: self.generation,
            server_instance_id: self.server.clone(),
        };
        self.ops
            .admitted
            .lock()
            .unwrap()
            .insert(target.operation_id, r.clone());
        r
    }
}
#[test]
fn initial_login_uses_real_pending_grant_without_fabricated_test_or_pool() {
    let f = Fixture::new();
    let pending = f.pending();
    assert_eq!(pending, f.pending());
    assert_eq!(pending.grant.principal_state, PrincipalState::Unknown);
    assert!(pending.grant.material.is_none());
    let m = f
        .repo
        .reserve_material(&GoogleMaterialContext::InitialLogin(pending.clone()), 13)
        .unwrap();
    assert_eq!(m, f.repo.reserve_material(&m.context, 13).unwrap());
    assert_eq!(
        m.material.credential.credential_version_id,
        m.material.vault_ref
    );
    let active = f
        .repo
        .commit_material(&GoogleMaterialCommit {
            pending: m.clone(),
            transition: GoogleMaterialTransition::InitialLogin,
            scopes_sha256: google_scope_hash(),
            committed_at_ms: 14,
        })
        .unwrap();
    assert_eq!(active.state, GoogleGrantState::Active);
    assert_eq!(active.principal_state, PrincipalState::Unknown);
    assert_eq!(active.material.unwrap().lifecycle_revision, 1);
    let c = f.db();
    for table in [
        "credential_pool_members",
        "test_records",
        "execution_authorization_leases",
    ] {
        assert_eq!(
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    assert!(f.repo.check_login(&pending).is_err());
    assert!(!f.repo.release_failed_material(&m, 15).unwrap());
}
#[test]
fn material_transition_scope_and_foreign_reservation_are_rejected_atomically() {
    let f = Fixture::new();
    let pending = f.pending();
    let m = f
        .repo
        .reserve_material(&GoogleMaterialContext::InitialLogin(pending), 13)
        .unwrap();
    for (transition, scope) in [
        (GoogleMaterialTransition::Refresh, google_scope_hash()),
        (GoogleMaterialTransition::InitialLogin, "f".repeat(64)),
    ] {
        assert!(
            f.repo
                .commit_material(&GoogleMaterialCommit {
                    pending: m.clone(),
                    transition,
                    scopes_sha256: scope,
                    committed_at_ms: 14
                })
                .is_err()
        );
    }
    let mut forged = m.clone();
    forged.material.vault_ref = "foreign-vault".into();
    assert!(
        f.repo
            .commit_material(&GoogleMaterialCommit {
                pending: forged,
                transition: GoogleMaterialTransition::InitialLogin,
                scopes_sha256: google_scope_hash(),
                committed_at_ms: 14
            })
            .is_err()
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT state FROM credential_refs WHERE credential_id=?1",
                [&m.material.credential.credential_id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "pending"
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT count(*) FROM oauth_token_material_revisions",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
}
#[test]
fn revoke_before_commit_fences_late_result_and_only_owned_pending_is_cleanable() {
    let f = Fixture::new();
    let pending = f.pending();
    let m = f
        .repo
        .reserve_material(&GoogleMaterialContext::InitialLogin(pending.clone()), 13)
        .unwrap();
    let control = f.control(&pending.grant);
    let invalid = f
        .repo
        .invalidate_grant(&control, GoogleGrantInvalidation::Revoked, 14)
        .unwrap();
    assert_eq!(invalid.state, GoogleGrantState::Revoked);
    assert!(
        f.repo
            .commit_material(&GoogleMaterialCommit {
                pending: m.clone(),
                transition: GoogleMaterialTransition::InitialLogin,
                scopes_sha256: google_scope_hash(),
                committed_at_ms: 15
            })
            .is_err()
    );
    assert!(f.repo.release_failed_material(&m, 16).unwrap());
    let candidates = f
        .store
        .google_material_cleanup_candidates(f.generation, f.server.clone(), 17)
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].credential, m.material.credential);
    f.store
        .confirm_google_material_cleaned(candidates[0].clone(), f.generation, f.server.clone(), 18)
        .unwrap();
    assert!(
        f.store
            .google_material_cleanup_candidates(f.generation, f.server.clone(), 19)
            .unwrap()
            .is_empty()
    );
    assert!(
        f.store
            .confirm_google_material_cleaned(m.material, f.generation, f.server.clone(), 20)
            .is_err()
    );
}
#[test]
fn lost_commit_response_replay_cannot_clean_activated_secret_or_change_client() {
    let f = Fixture::new();
    let (g, m) = f.activate();
    let command = GoogleMaterialCommit {
        pending: m.clone(),
        transition: GoogleMaterialTransition::InitialLogin,
        scopes_sha256: google_scope_hash(),
        committed_at_ms: 15,
    };
    assert_eq!(f.repo.commit_material(&command).unwrap(), g);
    assert!(!f.repo.release_failed_material(&m, 16).unwrap());
    assert!(
        f.store
            .google_material_cleanup_candidates(f.generation, f.server.clone(), 17)
            .unwrap()
            .is_empty()
    );
    let mut profile = client();
    profile.public_client_id = "another-client".into();
    assert!(
        f.store
            .register_google_client_profile(f.login.clone(), profile, 18)
            .is_err()
    );
    let mut wrong = f.login.clone();
    wrong.network_policy = GoogleNetworkPolicy::Inherit;
    assert_eq!(
        f.repo.prepare_login(&wrong, 19).unwrap_err().code,
        "operation_conflict"
    );
}
#[test]
fn profile_change_prevents_reserve_commit_control_and_old_instance_cleanup() {
    let f = Fixture::new();
    let pending = f.pending();
    let m = f
        .repo
        .reserve_material(&GoogleMaterialContext::InitialLogin(pending.clone()), 13)
        .unwrap();
    f.db().execute("UPDATE profile_meta SET runtime_generation=runtime_generation+1,server_instance_id='new-server'",[]).unwrap();
    assert!(f.repo.reserve_material(&m.context, 14).is_err());
    assert!(
        f.repo
            .commit_material(&GoogleMaterialCommit {
                pending: m.clone(),
                transition: GoogleMaterialTransition::InitialLogin,
                scopes_sha256: google_scope_hash(),
                committed_at_ms: 14
            })
            .is_err()
    );
    assert!(
        f.repo
            .invalidate_grant(
                &f.control(&pending.grant),
                GoogleGrantInvalidation::Revoked,
                15
            )
            .is_err()
    );
    assert!(f.repo.release_failed_material(&m, 16).is_err());
    assert!(
        f.store
            .google_material_cleanup_candidates(nz(f.generation.get() + 1), "new-server".into(), 17)
            .is_err()
    );
}
#[test]
fn no_pool_oauth_pending_allocation_is_recovered_after_real_store_restart() {
    let f = Fixture::new();
    let pending = f.pending();
    let m = f
        .repo
        .reserve_material(&GoogleMaterialContext::InitialLogin(pending), 13)
        .unwrap();
    assert!(f.store.pending_credentials().unwrap().is_empty());
    let path = f.path.clone();
    drop(f);
    let store = SqliteStore::open(&path, "google-profile", "restarted", 20).unwrap();
    let generation = nz(Connection::open(&path)
        .unwrap()
        .query_row("SELECT runtime_generation FROM profile_meta", [], |r| {
            r.get(0)
        })
        .unwrap());
    let candidates = store
        .google_material_cleanup_candidates(generation, "restarted".into(), 21)
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].credential, m.material.credential);
    assert_eq!(candidates[0].lifecycle_revision, 1);
    store
        .confirm_google_material_cleaned(candidates[0].clone(), generation, "restarted".into(), 22)
        .unwrap();
    assert!(
        store
            .google_material_cleanup_candidates(generation, "restarted".into(), 23)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn saved_test_requires_live_operation_and_its_exact_frozen_connection() {
    let f = Fixture::new();
    let (g, _) = f.activate();
    let r = f.saved_connection(&g);
    let a = f.repo.lease_authority(&r).unwrap();
    assert_eq!(a.origin.cursor(), Some(g.cursor));
    assert_eq!(
        f.db()
            .query_row("SELECT count(*) FROM connection_runtime_states", [], |r| {
                r.get::<_, u32>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        f.store
            .load_service_graph_revision(r.config_revision_id.clone())
            .unwrap()
            .connections[0]
            .availability_intent,
        false
    );
    let mut wrong = r.clone();
    wrong.target_policy_sha256 = "f".repeat(64);
    assert!(f.repo.lease_authority(&wrong).is_err());
    f.ops.admitted.lock().unwrap().clear();
    assert!(f.repo.lease_authority(&r).is_err());
    assert_eq!(
        f.db()
            .query_row("SELECT count(*) FROM test_records", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
fn lease_record(a: &GoogleAuthoritySnapshot) -> GoogleExecutionLeaseRecord {
    GoogleExecutionLeaseRecord {
        lease_id: format!("lease-{}", uuid::Uuid::new_v4()),
        request: a.request.clone(),
        origin: a.origin.clone(),
        minted_at_ms: 30,
        expires_at_ms: 3600030,
        network_policy: a.network_policy.clone(),
    }
}
#[test]
fn real_refresh_rotation_preserves_old_mint_provenance_but_revocation_kills_it() {
    let f = Fixture::new();
    let (g, _) = f.activate();
    let r = f.saved_connection(&g);
    let a = f.repo.lease_authority(&r).unwrap();
    let old = lease_record(&a);
    f.repo.check_issued_lease(&old).unwrap();
    let c = GoogleMaterialContext::Refresh {
        operation_id: "actual-sql-refresh".into(),
        request: r.clone(),
        expected: g.cursor.clone(),
        mint_material: a.material.clone(),
        network_policy: a.network_policy.clone(),
    };
    let m = f.repo.reserve_material(&c, 31).unwrap();
    let rotated = f
        .repo
        .commit_material(&GoogleMaterialCommit {
            pending: m,
            transition: GoogleMaterialTransition::Refresh,
            scopes_sha256: google_scope_hash(),
            committed_at_ms: 32,
        })
        .unwrap();
    assert_eq!(
        rotated.cursor.authorization_epoch,
        g.cursor.authorization_epoch
    );
    assert_eq!(rotated.cursor.material_revision, Some(nz(2)));
    assert_eq!(old.origin, a.origin);
    assert!(f.repo.check_lease_authority(&a).is_err());
    f.repo.check_issued_lease(&old).unwrap();
    f.store
        .revoke_credential(a.material.credential.credential_id.clone(), 33)
        .unwrap();
    assert!(f.repo.check_issued_lease(&old).is_err());
    let latest = f.repo.lease_authority(&r).unwrap();
    assert_eq!(latest.origin.cursor(), Some(rotated.cursor));
    assert_eq!(
        f.db()
            .query_row(
                "SELECT count(*) FROM oauth_token_material_revisions",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        2
    );
}
#[test]
fn stale_refresh_cannot_reserve_or_commit_and_source_changes_are_rejected() {
    let f = Fixture::new();
    let (g, _) = f.activate();
    let r = f.saved_connection(&g);
    let a = f.repo.lease_authority(&r).unwrap();
    let c = GoogleMaterialContext::Refresh {
        operation_id: "refresh-held".into(),
        request: r.clone(),
        expected: g.cursor.clone(),
        mint_material: a.material.clone(),
        network_policy: a.network_policy.clone(),
    };
    let m = f.repo.reserve_material(&c, 31).unwrap();
    f.repo
        .invalidate_grant(&f.control(&g), GoogleGrantInvalidation::Revoked, 32)
        .unwrap();
    assert!(
        f.repo
            .commit_material(&GoogleMaterialCommit {
                pending: m.clone(),
                transition: GoogleMaterialTransition::Refresh,
                scopes_sha256: google_scope_hash(),
                committed_at_ms: 33
            })
            .is_err()
    );
    assert!(f.repo.reserve_material(&c, 34).is_err());
    assert!(f.repo.check_issued_lease(&lease_record(&a)).is_err());
    assert!(f.repo.release_failed_material(&m, 35).unwrap());
    assert_eq!(
        f.db()
            .query_row(
                "SELECT count(*) FROM oauth_token_material_revisions",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
}
#[test]
fn test_provenance_is_deferred_and_forged_attempt_has_no_authority() {
    let f = Fixture::new();
    let (g, _) = f.activate();
    let r = f.saved_connection(&g);
    let a = f.repo.lease_authority(&r).unwrap();
    let record = lease_record(&a);
    assert_eq!(
        f.repo.record_attempt_lease(&record).unwrap_err().code,
        "google_test_terminal_deferred"
    );
    let mut forged = r;
    forged.target = GoogleLeaseTarget::Attempt {
        attempt_id: "not-a-real-attempt".into(),
        task_id: "not-a-real-task".into(),
    };
    assert!(f.repo.lease_authority(&forged).is_err());
    let mut foreign = record.clone();
    if let GoogleLeaseOrigin::UserOAuth { identity, .. } = &mut foreign.origin {
        identity.material_revision = nz(2);
    }
    assert!(f.repo.check_issued_lease(&foreign).is_err());
    assert_eq!(
        f.db()
            .query_row("SELECT count(*) FROM test_records", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT count(*) FROM execution_authorization_leases",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
}
#[test]
fn explicit_suspension_still_blocks_saved_test_without_test_restoring_control() {
    let f = Fixture::new();
    let (g, _) = f.activate();
    let r = f.saved_connection(&g);
    f.repo.lease_authority(&r).unwrap();
    // Simulates the durable explicit stop left by an earlier applied revision.
    f.db().execute("INSERT INTO connection_runtime_states(connection_id,config_revision_id,control_revision,is_suspended,reason_code,updated_at_ms) VALUES(?1,?2,1,1,'user_suspended',30)",rusqlite::params![r.connection_id,r.config_revision_id]).unwrap();
    assert_eq!(
        f.repo.lease_authority(&r).unwrap_err().code,
        "google_connection_suspended"
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",
                [&r.connection_id],
                |r| r.get::<_, bool>(0)
            )
            .unwrap(),
        true
    );
}
#[test]
fn service_account_test_uses_registry_selected_source_even_with_a_single_member() {
    let f = Fixture::new();
    let (g, _) = f.activate();
    let oauth = f.saved_connection(&g);
    let mut graph = f
        .store
        .load_service_graph_revision(oauth.config_revision_id)
        .unwrap();
    let reference = app_core::contracts::CredentialReference {
        credential_id: "sa-credential".into(),
        credential_version_id: "synthetic-sa-vault".into(),
        pool_id: "sa-pool".into(),
        pool_revision: 1,
        label: "SA".into(),
        masked_hint: "synthetic".into(),
    };
    f.store
        .begin_credential_material(
            reference.clone(),
            CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::GoogleServiceAccount,
            },
            30,
        )
        .unwrap();
    f.store
        .activate_credential(reference.credential_id.clone(), 31)
        .unwrap();
    graph.revision_id = "sa-test-graph".into();
    let c = &mut graph.connections[0];
    c.auth_mode = "google_service_account".into();
    c.google_authorization = None;
    c.credential_pool_id = Some(reference.pool_id.clone());
    c.credential_pool_revision = Some(nz(1));
    let digest = google_target_policy_sha256(c).unwrap();
    let saved = f
        .store
        .persist_service_graph_revision(graph, None, 32)
        .unwrap();
    let op = GoogleTestOperation {
        operation_id: "sa-selected-test".into(),
        input_scope: GoogleTestInputScope::Sample,
        input_sha256: "b".repeat(64),
        request_semantics_sha256: "c".repeat(64),
    };
    let r = GoogleLeaseRequest {
        target: GoogleLeaseTarget::TestOperation(op.clone()),
        config_revision_id: saved,
        connection_id: "google-connection".into(),
        target_policy_sha256: digest,
        profile_generation: f.generation,
        server_instance_id: f.server.clone(),
    };
    f.ops
        .admitted
        .lock()
        .unwrap()
        .insert(op.operation_id.clone(), r.clone());
    assert_eq!(
        f.repo.lease_authority(&r).unwrap_err().code,
        "test_source_not_admitted"
    );
    let member = f
        .db()
        .query_row(
            "SELECT member_id FROM credential_pool_members WHERE pool_id='sa-pool'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let source = GoogleServiceAccountLeaseIdentity {
        pool_id: reference.pool_id,
        pool_revision: nz(1),
        member_id: member,
        credential_version: CredentialVersionRef {
            credential_id: reference.credential_id,
            credential_version_id: reference.credential_version_id,
        },
        lifecycle_revision: 1,
        scopes_hash: google_scope_hash(),
    };
    f.ops
        .sources
        .lock()
        .unwrap()
        .insert(op.operation_id.clone(), source.clone());
    let a = f.repo.lease_authority(&r).unwrap();
    assert_eq!(a.origin, GoogleLeaseOrigin::ServiceAccount(source.clone()));
    let mut bad = source.clone();
    bad.member_id = "not-its-member".into();
    f.ops
        .sources
        .lock()
        .unwrap()
        .insert(op.operation_id.clone(), bad);
    assert!(f.repo.lease_authority(&r).is_err());
    let mut bad = source;
    bad.lifecycle_revision += 1;
    f.ops.sources.lock().unwrap().insert(op.operation_id, bad);
    assert!(f.repo.lease_authority(&r).is_err());
}
#[derive(Default)]
struct MemoryVault(Mutex<BTreeMap<String, Vec<u8>>>);
impl app_core::ports::runtime::CredentialVault for MemoryVault {
    fn put(&self, key: &str, bytes: &[u8]) -> AppResult<()> {
        let mut data = self.0.lock().unwrap();
        if data.contains_key(key) {
            return Err(AppError::new(
                "overwrite_forbidden",
                "fixture",
                "Synthetic Vault refuses replacement",
            ));
        }
        data.insert(key.into(), bytes.to_vec());
        Ok(())
    }
    fn get(&self, key: &str) -> AppResult<Vec<u8>> {
        self.0
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| AppError::new("not_found", "fixture", "Synthetic material absent"))
    }
    fn delete(&self, key: &str) -> AppResult<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}
struct SyntheticGoogleTransport;
impl app_infra::providers::google_token::GoogleTokenTransport for SyntheticGoogleTransport {
    fn exchange<'a>(
        &'a self,
        request: app_infra::providers::google_token::GoogleTokenRequest<'a>,
        _: &'a GoogleNetworkPolicy,
        _: &'a GoogleBudget,
    ) -> app_core::ports::runtime::PortFuture<
        'a,
        app_infra::providers::google_token::GoogleTokenReply,
    > {
        use app_infra::providers::google_token::*;
        Box::pin(async move {
            let value = match request {
                GoogleTokenRequest::AuthorizationCode { .. } => {
                    serde_json::json!({"access_token":"synthetic-login-access-only","token_type":"Bearer","expires_in":3600,"refresh_token":"synthetic-private-refresh-first","scope":GOOGLE_TRANSLATION_SCOPE})
                }
                GoogleTokenRequest::Refresh { refresh_token, .. } => {
                    assert_eq!(refresh_token.expose(), "synthetic-private-refresh-first");
                    serde_json::json!({"access_token":"synthetic-final-access-only","token_type":"Bearer","expires_in":3600,"refresh_token":"synthetic-private-refresh-second","scope":GOOGLE_TRANSLATION_SCOPE})
                }
                GoogleTokenRequest::Revoke { token } => {
                    assert_eq!(token.expose(), "synthetic-private-refresh-second");
                    return Ok(GoogleTokenReply::Revoked);
                }
                _ => {
                    return Err(AppError::new(
                        "unexpected_protocol",
                        "fixture",
                        "No service-account token expected",
                    ));
                }
            };
            decode_google_token_response(200, serde_json::to_vec(&value).unwrap(), false)
        })
    }
}
#[tokio::test]
async fn real_sql_and_provider_pkce_vault_rotation_and_revoke_form_one_private_chain() {
    use app_infra::providers::google_authorization::{
        GoogleAuthorizationService, SystemGoogleClock,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    let f = Fixture::new();
    let vault = Arc::new(MemoryVault::default());
    let service = GoogleAuthorizationService::new(
        f.repo.clone(),
        vault.clone(),
        Arc::new(SyntheticGoogleTransport),
        Arc::new(SystemGoogleClock),
        f.ops.clone(),
    );
    let session = service
        .begin_login(
            f.login.clone(),
            GoogleBudget::new(std::time::Duration::from_secs(3)).unwrap(),
        )
        .await
        .unwrap();
    let uri = reqwest::Url::parse(session.intent().authorization_url()).unwrap();
    let query: BTreeMap<_, _> = uri.query_pairs().into_owned().collect();
    let callback = reqwest::Url::parse(&query["redirect_uri"]).unwrap();
    let browser = async {
        let mut stream = TcpStream::connect(("127.0.0.1", callback.port().unwrap()))
            .await
            .unwrap();
        stream
            .write_all(
                format!(
                    "GET {}?state={}&code=synthetic-code HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
                    callback.path(),
                    query["state"],
                    callback.port().unwrap()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut bytes = vec![];
        stream.read_to_end(&mut bytes).await.unwrap();
        assert!(bytes.starts_with(b"HTTP/1.1 200"));
        assert!(!String::from_utf8_lossy(&bytes).contains("synthetic-code"));
    };
    let (grant, ()) = tokio::join!(session.finish(), browser);
    let grant = grant.unwrap();
    assert_eq!(grant.principal_state, PrincipalState::Unknown);
    let request = f.saved_connection(&grant);
    let lease = service
        .lease(
            request,
            GoogleBudget::new(std::time::Duration::from_secs(3)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        lease.persistence(),
        app_infra::providers::google_authorization::GoogleLeasePersistence::DeferredTerminalTest
    );
    let GoogleLeaseOrigin::UserOAuth { identity, .. } = &lease.record().origin else {
        panic!("not user source");
    };
    assert_eq!(identity.material_revision, nz(1));
    assert!(lease.authorization().is_ok());
    let current = f
        .db()
        .query_row(
            "SELECT material_revision,control_revision FROM oauth_grants WHERE grant_id=?1",
            [&grant.cursor.grant_id],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?)),
        )
        .unwrap();
    assert_eq!(current, (2, 2));
    assert_eq!(vault.0.lock().unwrap().len(), 2);
    let mut control = f.control(&grant);
    control.expected.material_revision = Some(nz(current.0));
    control.expected.control_revision = current.1;
    service
        .revoke(
            control,
            GoogleBudget::new(std::time::Duration::from_secs(3)).unwrap(),
        )
        .await
        .unwrap();
    assert!(lease.authorization().is_err());
    assert_eq!(lease.invalidated().await.code, "google_authority_changed");
    f.store.checkpoint().unwrap();
    let bytes = std::fs::read(&f.path).unwrap();
    for secret in [
        "synthetic-private-refresh-first",
        "synthetic-private-refresh-second",
        "synthetic-final-access-only",
        "synthetic-login-access-only",
    ] {
        assert!(
            !bytes
                .windows(secret.len())
                .any(|part| part == secret.as_bytes())
        );
    }
    assert_eq!(
        f.db()
            .query_row("SELECT count(*) FROM test_records", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[test]
fn bad_times_and_control_replay_policy_conflicts_cannot_commit_partial_state() {
    let f = Fixture::new();
    assert!(f.repo.prepare_login(&f.login, -1).is_err());
    assert!(f.repo.prepare_login(&f.login, i64::MAX).is_err());
    let pending = f.pending();
    let m = f
        .repo
        .reserve_material(&GoogleMaterialContext::InitialLogin(pending.clone()), 20)
        .unwrap();
    assert!(
        f.repo
            .commit_material(&GoogleMaterialCommit {
                pending: m.clone(),
                transition: GoogleMaterialTransition::InitialLogin,
                scopes_sha256: google_scope_hash(),
                committed_at_ms: 19
            })
            .is_err()
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT state,lifecycle_revision FROM credential_refs WHERE credential_id=?1",
                [&m.material.credential.credential_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
            )
            .unwrap(),
        ("pending".into(), 0)
    );
    let control = f.control(&pending.grant);
    f.repo
        .invalidate_grant(&control, GoogleGrantInvalidation::Revoked, 21)
        .unwrap();
    assert_eq!(
        f.repo
            .invalidate_grant(&control, GoogleGrantInvalidation::Revoked, 22)
            .unwrap()
            .state,
        GoogleGrantState::Revoked
    );
    let mut changed = control;
    changed.network_policy = GoogleNetworkPolicy::Inherit;
    assert_eq!(
        f.repo
            .invalidate_grant(&changed, GoogleGrantInvalidation::Revoked, 23)
            .unwrap_err()
            .code,
        "operation_conflict"
    );
}
#[test]
fn new_authority_and_execution_debug_never_print_actual_private_vault_locator() {
    let f = Fixture::new();
    let (g, _) = f.activate();
    let r = f.saved_connection(&g);
    let authority = f.repo.lease_authority(&r).unwrap();
    let record = lease_record(&authority);
    for text in [
        format!("{authority:?}"),
        format!("{record:?}"),
        format!("{:?}", authority.origin),
    ] {
        assert!(!text.contains(&authority.material.vault_ref));
        assert!(!text.contains("synthetic-private-refresh"));
        assert!(text.contains("REDACTED"));
    }
}
struct Ids;
impl app_core::ports::IdentitySource for Ids {
    fn new_id(&self, p: &str) -> String {
        format!("{p}-{}", uuid::Uuid::new_v4())
    }
    fn now_ms(&self) -> i64 {
        panic!("pure draft compilation cannot read clock")
    }
}
fn actual_service_attempt(
    f: &Fixture,
    preset: &str,
    purpose: app_core::contracts::CatalogPurpose,
    values: BTreeMap<String, app_core::contracts::CatalogScalar>,
    authentication: app_core::contracts::ServiceAuthenticationSelection,
    prepared: app_core::domain::model_config::service_draft::PreparedServiceDraftResources,
) -> app_core::domain::service_dispatch::ServiceTaskDispatch {
    use app_core::{
        contracts::*,
        domain::{
            model_config::service_draft::compile_service_draft, service_dispatch::WorkDispatch,
        },
        ports::ServiceConfigurationRepository,
    };
    let operation = uuid::Uuid::new_v4().to_string();
    let state = f.store.service_configuration().unwrap();
    let input = SaveServiceDraftInput {
        operation_id: format!("save-{operation}"),
        expected_profile_generation: state.profile_generation,
        expected_draft_head_revision: state.draft_head_revision,
        editing: None,
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: preset.into(),
                preset_revision: 1,
            },
        },
        purpose,
        values,
        authentication,
    };
    let compiled = compile_service_draft(
        &state.draft,
        &input,
        &app_infra::providers::builtin_registry().unwrap(),
        &Ids,
        &prepared,
    )
    .unwrap();
    let source = f
        .store
        .save_service_configuration(&compiled, 30)
        .unwrap()
        .saved_resource;
    let state = f.store.service_configuration().unwrap();
    let q = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: state.profile_generation,
            scope: ServiceScope::Global {},
            purpose,
        })
        .unwrap();
    assert!(
        q.rows.iter().any(|row| row.source == source
            && matches!(row.eligibility, SavedServiceEligibility::Selectable {})),
        "actual saved eligibility: {q:?}"
    );
    f.store
        .select_saved_services(
            &SelectSavedServicesInput {
                operation_id: format!("select-{operation}"),
                expected_profile_generation: state.profile_generation,
                scope: ServiceScope::Global {},
                purpose,
                expected_selection_revision: q.selection.selection_revision,
                expected_applied_head_revision: state.applied_head_revision,
                main: source,
                comparisons: vec![],
            },
            31,
        )
        .unwrap();
    let state = f.store.service_configuration().unwrap();
    let q = f
        .store
        .saved_services(&SavedServiceQuery {
            expected_profile_generation: state.profile_generation,
            scope: ServiceScope::Global {},
            purpose,
        })
        .unwrap();
    f.store
        .set_service_purpose_enabled(
            &SetServicePurposeEnabledInput {
                operation_id: format!("enable-{operation}"),
                expected_profile_generation: state.profile_generation,
                scope: ServiceScope::Global {},
                purpose,
                expected_selection_revision: q.selection.selection_revision,
                expected_applied_head_revision: state.applied_head_revision,
                enabled: true,
            },
            32,
        )
        .unwrap();
    let receipt = f
        .store
        .dispatch(
            CommandEnvelope {
                operation_id: format!("input-{operation}"),
                expected_revision: None,
                command: Command::ManualInput {
                    text: "放課後、図書館で会った。".into(),
                    source_language: "ja".into(),
                },
            },
            33,
        )
        .unwrap();
    let purpose_name = match purpose {
        CatalogPurpose::Translation => "translation",
        CatalogPurpose::Grammar => "grammar",
        CatalogPurpose::ContextWord => "context_word",
    };
    let mut work = if purpose == CatalogPurpose::Translation {
        f.store
            .prepare_work_tasks_for_operation(
                receipt.result_id.unwrap(),
                1,
                purpose_name.into(),
                format!("prepare-{operation}"),
                34,
            )
            .unwrap()
    } else {
        f.store
            .prepare_work_study_tasks(
                receipt.result_id.unwrap(),
                1,
                purpose_name.into(),
                None,
                format!("prepare-{operation}"),
                34,
            )
            .unwrap()
    };
    assert_eq!(work.len(), 1);
    let WorkDispatch::Service(dispatch) = work.remove(0) else {
        panic!("not actual codec3 admission");
    };
    *dispatch
}
fn actual_google_attempt(
    f: &Fixture,
    g: &GoogleOwnedGrant,
) -> app_core::domain::service_dispatch::ServiceTaskDispatch {
    use app_core::{contracts::*, domain::model_config::service_draft::*};
    let authorization = GoogleAuthorizationRef {
        grant_id: g.cursor.grant_id.clone(),
        authorization_epoch: g.cursor.authorization_epoch.unwrap(),
    };
    let values = [
        (
            "name".into(),
            CatalogScalar::Text("Google actual source".into()),
        ),
        (
            "google_auth_method".into(),
            CatalogScalar::Text("oauth".into()),
        ),
        (
            "oauth_client_mode".into(),
            CatalogScalar::Text("custom".into()),
        ),
        (
            "oauth_client_id".into(),
            CatalogScalar::Text(g.client.public_client_id.clone()),
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
    ]
    .into_iter()
    .collect();
    actual_service_attempt(
        f,
        "builtin.google-v3",
        CatalogPurpose::Translation,
        values,
        ServiceAuthenticationSelection::GoogleOwnedGrant {
            authorization: authorization.clone(),
        },
        PreparedServiceDraftResources {
            google_client: Some(PreparedGoogleClientDraft {
                mode: "custom".into(),
                client_id: Some(g.client.public_client_id.clone()),
                authorization,
            }),
            ..Default::default()
        },
    )
}
#[test]
fn real_admission_attempt_records_exact_old_mint_then_grant_revoke_cancels_its_task() {
    use app_core::domain::service_execution::ServiceExecutionTarget;
    let f = Fixture::new();
    let (g, _) = f.activate();
    f.setup_actual_google_handler();
    let task = actual_google_attempt(&f, &g);
    let ServiceExecutionTarget::Http { plan } = &task.plan.target else {
        panic!("not HTTP");
    };
    assert_eq!(task.credential_vault_ref, None);
    assert_eq!(
        f.db()
            .query_row(
                "SELECT credential_id FROM request_attempts WHERE attempt_id=?1",
                [&task.attempt_id],
                |row| row.get::<_, Option<String>>(0)
            )
            .unwrap(),
        None
    );
    let request = GoogleLeaseRequest {
        target: GoogleLeaseTarget::Attempt {
            attempt_id: task.attempt_id.clone(),
            task_id: task.task_id.clone(),
        },
        config_revision_id: task.plan.identity.configuration_revision.clone(),
        connection_id: plan.connection.connection_id.clone(),
        target_policy_sha256: google_target_policy_sha256(&plan.connection).unwrap(),
        profile_generation: nz(task.profile_generation),
        server_instance_id: task.server_instance_id.clone(),
    };
    let a = f.repo.lease_authority(&request).unwrap();
    let mut record = lease_record(&a);
    record.minted_at_ms = 35;
    record.expires_at_ms = 3600035;
    f.repo.record_attempt_lease(&record).unwrap();
    f.repo.record_attempt_lease(&record).unwrap();
    assert_eq!(
        f.db()
            .query_row(
                "SELECT count(*) FROM execution_authorization_leases WHERE attempt_id=?1",
                [&task.attempt_id],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
    let mut conflict = record.clone();
    conflict.expires_at_ms += 1;
    assert_eq!(
        f.repo.record_attempt_lease(&conflict).unwrap_err().code,
        "google_lease_conflict"
    );
    f.repo
        .invalidate_grant(&f.control(&g), GoogleGrantInvalidation::Revoked, 36)
        .unwrap();
    assert!(f.repo.check_issued_lease(&record).is_err());
    assert_eq!(
        f.db()
            .query_row(
                "SELECT state FROM request_tasks WHERE task_id=?1",
                [&task.task_id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "invalidated"
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT state FROM request_attempts WHERE attempt_id=?1",
                [&task.attempt_id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "cancelled"
    );
}
fn request_for_task(
    task: &app_core::domain::service_dispatch::ServiceTaskDispatch,
) -> GoogleLeaseRequest {
    let app_core::domain::service_execution::ServiceExecutionTarget::Http { plan } =
        &task.plan.target
    else {
        panic!("not HTTP");
    };
    GoogleLeaseRequest {
        target: GoogleLeaseTarget::Attempt {
            attempt_id: task.attempt_id.clone(),
            task_id: task.task_id.clone(),
        },
        config_revision_id: task.plan.identity.configuration_revision.clone(),
        connection_id: plan.connection.connection_id.clone(),
        target_policy_sha256: google_target_policy_sha256(&plan.connection).unwrap(),
        profile_generation: nz(task.profile_generation),
        server_instance_id: task.server_instance_id.clone(),
    }
}
#[test]
fn real_service_account_admission_persists_pool_member_proof_without_creating_a_user_grant() {
    use app_core::contracts::*;
    let f = Fixture::new();
    let reference = CredentialReference {
        credential_id: "sa-actual-credential".into(),
        credential_version_id: "synthetic-sa-actual-vault".into(),
        pool_id: "sa-actual-pool".into(),
        pool_revision: 1,
        label: "SA".into(),
        masked_hint: "synthetic".into(),
    };
    f.store
        .begin_credential_material(
            reference.clone(),
            CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::GoogleServiceAccount,
            },
            12,
        )
        .unwrap();
    f.store
        .activate_credential(reference.credential_id.clone(), 13)
        .unwrap();
    let values = [
        (
            "name".into(),
            CatalogScalar::Text("SA actual source".into()),
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
    ]
    .into_iter()
    .collect();
    f.setup_actual_google_handler();
    let task = actual_service_attempt(
        &f,
        "builtin.google-v3",
        CatalogPurpose::Translation,
        values,
        ServiceAuthenticationSelection::ExistingPool {
            pool_id: reference.pool_id.clone(),
            pool_revision: nz(1),
        },
        Default::default(),
    );
    let request = request_for_task(&task);
    let a = f.repo.lease_authority(&request).unwrap();
    let GoogleLeaseOrigin::ServiceAccount(source) = &a.origin else {
        panic!("SA fabricated a user grant");
    };
    assert_eq!(
        source.credential_version.credential_id,
        reference.credential_id
    );
    assert_eq!(
        Some(a.material.vault_ref.clone()),
        task.credential_vault_ref
    );
    let mut record = lease_record(&a);
    record.minted_at_ms = 35;
    record.expires_at_ms = 3600035;
    f.repo.record_attempt_lease(&record).unwrap();
    assert_eq!(f.db().query_row("SELECT origin_kind,grant_id,member_id FROM execution_authorization_leases WHERE attempt_id=?1",[&task.attempt_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,String>(2)?))).unwrap(),("service_account".into(),None,source.member_id.clone()));
    assert_eq!(
        f.db()
            .query_row("SELECT count(*) FROM oauth_grants", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    f.store
        .revoke_credential(reference.credential_id, 36)
        .unwrap();
    assert!(f.repo.check_issued_lease(&record).is_err());
}
#[test]
fn grant_revoke_leaves_an_unrelated_genuinely_admitted_grammar_task_running() {
    use app_core::contracts::*;
    let f = Fixture::new();
    let (g, _) = f.activate();
    f.setup_actual_google_handler();
    let google = actual_google_attempt(&f, &g);
    let values = [
        (
            "name".into(),
            CatalogScalar::Text("Unrelated grammar".into()),
        ),
        (
            "model".into(),
            CatalogScalar::Text("synthetic-grammar-model".into()),
        ),
        (
            "template".into(),
            CatalogScalar::Text("Explain {{source}}".into()),
        ),
    ]
    .into_iter()
    .collect();
    let grammar = actual_service_attempt(
        &f,
        "builtin.lmstudio",
        CatalogPurpose::Grammar,
        values,
        ServiceAuthenticationSelection::None {},
        Default::default(),
    );
    let a = f.repo.lease_authority(&request_for_task(&google)).unwrap();
    let mut record = lease_record(&a);
    record.minted_at_ms = 35;
    record.expires_at_ms = 3600035;
    f.repo.record_attempt_lease(&record).unwrap();
    f.store.validate_service_dispatch(&grammar).unwrap();
    f.repo
        .invalidate_grant(&f.control(&g), GoogleGrantInvalidation::Revoked, 36)
        .unwrap();
    assert_eq!(
        f.db()
            .query_row(
                "SELECT state FROM request_tasks WHERE task_id=?1",
                [&google.task_id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "invalidated"
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT state FROM request_tasks WHERE task_id=?1",
                [&grammar.task_id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "running"
    );
    f.store.validate_service_dispatch(&grammar).unwrap();
}
#[test]
fn readonly_google_client_draft_uses_real_owned_metadata_and_rejects_stale_epoch_or_frame() {
    use app_core::ports::ServiceConfigurationRepository;
    let f = Fixture::new();
    let (grant, _) = f.activate();
    let reference = GoogleAuthorizationRef {
        grant_id: grant.cursor.grant_id.clone(),
        authorization_epoch: grant.cursor.authorization_epoch.unwrap(),
    };
    let draft = f
        .store
        .google_client_draft(&reference, f.generation)
        .unwrap();
    assert_eq!(draft.mode, "custom");
    assert_eq!(draft.client_id, Some(grant.client.public_client_id.clone()));
    assert_eq!(draft.authorization, reference);
    let mut wrong = reference.clone();
    wrong.authorization_epoch = nz(wrong.authorization_epoch.get() + 1);
    assert!(f.store.google_client_draft(&wrong, f.generation).is_err());
    assert!(
        f.store
            .google_client_draft(&reference, nz(f.generation.get() + 1))
            .is_err()
    );
    f.repo
        .invalidate_grant(&f.control(&grant), GoogleGrantInvalidation::Revoked, 40)
        .unwrap();
    assert!(
        f.store
            .google_client_draft(&reference, f.generation)
            .is_err()
    );
}
struct NoServiceTestOperation;
impl app_core::ports::service_testing::ServiceTestOperationAuthority for NoServiceTestOperation {
    fn operation_budget(
        &self,
        _: &Arc<app_core::ports::service_testing::PreparedServiceTest>,
    ) -> AppResult<app_core::ports::operation_budget::OperationBudget> {
        Err(AppError::new(
            "not_registered",
            "fixture",
            "No execution operation has been registered",
        ))
    }
}
#[test]
fn real_saved_test_preparation_reuses_owned_grant_without_creating_terminal_or_task_rows() {
    use app_core::{contracts::*, ports::service_testing::*};
    let f = Fixture::new();
    let (grant, _) = f.activate();
    f.setup_actual_google_handler();
    let _existing_task = actual_google_attempt(&f, &grant);
    let (revision,model):(String,String)=f.db().query_row("SELECT c.revision_id,m.model_id FROM connections c JOIN config_revisions r ON r.revision_id=c.revision_id JOIN models m ON m.revision_id=c.revision_id AND m.connection_id=c.connection_id WHERE r.revision_role='user' AND c.oauth_grant_id=?1 ORDER BY r.created_at_ms DESC LIMIT 1",[&grant.cursor.grant_id],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
    let source = SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: revision,
            model_id: model,
        },
    };
    let tasks_before = f
        .db()
        .query_row("SELECT count(*) FROM request_tasks", [], |row| {
            row.get::<_, u32>(0)
        })
        .unwrap();
    let tests = f
        .store
        .service_testing_repository(Arc::new(NoServiceTestOperation), f.ops.clone()).unwrap();
    let input = PrepareServiceTestInput {
        operation_id: "g6-real-prepared-google".into(),
        request: SavedServiceTestRequest {
            expected_profile_generation: f.generation,
            source: source.clone(),
            purpose: CatalogPurpose::Translation,
            input_scope: Some(ServiceTestInputScope::Sample {}),
            output_mode: Some(ServiceTestOutputMode::Text),
        },
        selected_token: None,
    };
    let prepared = tests.prepare_service_test(input.clone(), 40).unwrap();
    let PreparedTestAuthentication::GoogleOwnedGrant(actual) = prepared.authentication else {
        panic!("owned grant was converted to a pool");
    };
    assert_eq!(actual, grant);
    assert_eq!(prepared.source.source, source);
    assert_eq!(
        f.db()
            .query_row("SELECT count(*) FROM request_tasks", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        tasks_before
    );
    assert_eq!(
        f.db()
            .query_row("SELECT count(*) FROM test_records", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    f.repo
        .invalidate_grant(&f.control(&grant), GoogleGrantInvalidation::Revoked, 41)
        .unwrap();
    assert!(tests.prepare_service_test(input, 42).is_err());
}
#[derive(Default)]
struct LiveServiceTestOperation {
    entry: Mutex<
        Option<(
            Arc<app_core::ports::service_testing::PreparedServiceTest>,
            app_core::ports::operation_budget::OperationBudget,
        )>,
    >,
}
impl app_core::ports::service_testing::ServiceTestOperationAuthority for LiveServiceTestOperation {
    fn operation_budget(
        &self,
        prepared: &Arc<app_core::ports::service_testing::PreparedServiceTest>,
    ) -> AppResult<app_core::ports::operation_budget::OperationBudget> {
        self.entry
            .lock()
            .unwrap()
            .as_ref()
            .filter(|(actual, _)| Arc::ptr_eq(actual, prepared))
            .map(|(_, budget)| budget.clone())
            .ok_or_else(|| {
                AppError::new(
                    "not_registered",
                    "controlled_test_operation",
                    "Only the exact privately prepared operation is registered",
                )
            })
    }
}
async fn terminal_helper_atomic_scenario(selected: bool) {
    use app_core::{
        application::google_test_operations::LiveGoogleTestOperations,
        contracts::*,
        domain::service_execution::ServiceExecutionTarget,
        ports::{operation_budget::OperationBudget, service_testing::*},
    };
    use app_infra::providers::google_authorization::{
        GoogleAuthorizationService, GoogleClock, SystemGoogleClock,
    };
    use std::time::Duration;
    let f = Fixture::new();
    let (grant, _) = f.activate();
    f.setup_actual_google_handler();
    let existing_task = actual_google_attempt(&f, &grant);
    let (revision,model):(String,String)=f.db().query_row("SELECT c.revision_id,m.model_id FROM connections c JOIN config_revisions r ON r.revision_id=c.revision_id JOIN models m ON m.revision_id=c.revision_id AND m.connection_id=c.connection_id WHERE r.revision_role='user' AND c.oauth_grant_id=?1 ORDER BY r.created_at_ms DESC LIMIT 1",[&grant.cursor.grant_id],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
    let live = Arc::new(LiveServiceTestOperation::default());
    let google = Arc::new(LiveGoogleTestOperations::default());
    let tests = f
        .store
        .service_testing_repository(live.clone(), google.clone()).unwrap();
    let started = SystemGoogleClock.now().unwrap().unix_ms;
    let prepared = Arc::new(
        tests
            .prepare_service_test(
                PrepareServiceTestInput {
                    operation_id: "g6-atomic-terminal".into(),
                    request: SavedServiceTestRequest {
                        expected_profile_generation: f.generation,
                        source: SavedServiceRef::Http {
                            source: HttpConfigRef {
                                revision_id: revision,
                                model_id: model,
                            },
                        },
                        purpose: CatalogPurpose::Translation,
                        input_scope: Some(if selected {
                            ServiceTestInputScope::SelectedUnit {
                                unit_id: existing_task.unit.unit_id.clone(),
                                text_revision: nz(existing_task.unit.text_revision),
                                source_hash: existing_task.unit.source_hash.clone(),
                            }
                        } else {
                            ServiceTestInputScope::Sample {}
                        }),
                        output_mode: Some(ServiceTestOutputMode::Text),
                    },
                    selected_token: None,
                },
                started,
            )
            .unwrap(),
    );
    let budget = OperationBudget::new(Duration::from_secs(10)).unwrap();
    *live.entry.lock().unwrap() = Some((prepared.clone(), budget.clone()));
    let ServiceExecutionTarget::Http { plan } = &prepared.plan.target else {
        panic!("not HTTP");
    };
    let request = GoogleLeaseRequest {
        target: GoogleLeaseTarget::TestOperation(GoogleTestOperation {
            operation_id: prepared.input.operation_id.clone(),
            input_scope: if selected {
                GoogleTestInputScope::SelectedCurrent
            } else {
                GoogleTestInputScope::Sample
            },
            input_sha256: prepared.input_sha256.clone(),
            request_semantics_sha256: prepared.request_semantics_sha256.clone(),
        }),
        config_revision_id: match &prepared.source.source {
            SavedServiceRef::Http { source } => source.revision_id.clone(),
            _ => panic!("not HTTP"),
        },
        connection_id: plan.connection.connection_id.clone(),
        target_policy_sha256: google_target_policy_sha256(&plan.connection).unwrap(),
        profile_generation: f.generation,
        server_instance_id: f.server.clone(),
    };
    let google_budget = GoogleBudget::from_operation_budget(budget.clone());
    let _guard = google
        .register(request.clone(), google_budget.clone(), None)
        .unwrap();
    let vault = Arc::new(MemoryVault::default());
    let material = grant.material.as_ref().unwrap();
    let envelope = encode_vault_credential(&CredentialMaterial::GoogleOauthRefresh {
        refresh_token: SecretText::from_bytes(b"synthetic-private-refresh-first".to_vec()).unwrap(),
    })
    .unwrap();
    vault
        .0
        .lock()
        .unwrap()
        .insert(material.vault_ref.clone(), envelope.expose().to_vec());
    let service = GoogleAuthorizationService::new(
        f.store.google_authorization_repository(google.clone()),
        vault,
        Arc::new(SyntheticGoogleTransport),
        Arc::new(SystemGoogleClock),
        google,
    );
    let lease = service.lease(request, google_budget).await.unwrap();
    let completed = SystemGoogleClock
        .now()
        .unwrap()
        .unix_ms
        .max(lease.record().minted_at_ms);
    let finish = FinishServiceTestInput {
        prepared: prepared.clone(),
        outcome: ServiceTestTerminalOutcome::Failed {
            error: AppError::new(
                "controlled_stop_after_auth",
                "test_request",
                "No model request is claimed",
            ),
        },
        budget: budget.clone(),
        request_sent: false,
        credential_used: true,
        google_lease: Some(lease.record().clone()),
    };
    let mut attacks = vec![];
    let mut changed = lease.record().clone();
    if let GoogleLeaseTarget::TestOperation(op) = &mut changed.request.target {
        op.operation_id = "another-operation".into();
    }
    attacks.push(changed);
    let mut changed = lease.record().clone();
    if let GoogleLeaseTarget::TestOperation(op) = &mut changed.request.target {
        op.input_sha256 = "f".repeat(64);
    }
    attacks.push(changed);
    let mut changed = lease.record().clone();
    if let GoogleLeaseTarget::TestOperation(op) = &mut changed.request.target {
        op.request_semantics_sha256 = "e".repeat(64);
    }
    attacks.push(changed);
    let mut changed = lease.record().clone();
    changed.request.profile_generation = nz(f.generation.get() + 1);
    attacks.push(changed);
    let mut changed = lease.record().clone();
    changed.request.connection_id = "foreign-connection".into();
    attacks.push(changed);
    let mut changed = lease.record().clone();
    if let GoogleLeaseTarget::TestOperation(op) = &mut changed.request.target {
        op.input_scope = if selected {
            GoogleTestInputScope::Sample
        } else {
            GoogleTestInputScope::SelectedCurrent
        };
    }
    attacks.push(changed);
    let mut changed = lease.record().clone();
    changed.minted_at_ms = completed + 1;
    attacks.push(changed);
    for changed in attacks {
        let mut bad = finish.clone();
        bad.google_lease = Some(changed);
        assert!(tests.finish_service_test(bad, completed).is_err());
        assert!(!budget.is_committed());
        budget.check().unwrap();
        for table in [
            "test_records",
            "test_record_credentials",
            "execution_authorization_leases",
        ] {
            assert_eq!(
                f.db()
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                        .get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
    }
    let result = tests
        .finish_service_test(finish.clone(), completed)
        .unwrap();
    assert_eq!(result.status, ServiceTestStatus::Failed);
    assert!(budget.is_committed());
    assert!(lease.authorization().is_err());
    let actual:(String,String,Option<String>,Option<String>)=f.db().query_row("SELECT l.lease_id,l.test_id,l.attempt_id,l.task_id FROM execution_authorization_leases l JOIN test_records t USING(test_id) WHERE t.test_id=?1",[&prepared.test_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
    assert_eq!(
        actual,
        (
            lease.record().lease_id.clone(),
            prepared.test_id.clone(),
            None,
            None
        )
    );
    assert_eq!(
        f.db()
            .query_row(
                "SELECT count(*) FROM test_record_credentials WHERE test_id=?1",
                [&prepared.test_id],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        tests
            .finish_service_test(finish, completed)
            .unwrap()
            .test_id,
        result.test_id
    );
}

#[tokio::test]
async fn terminal_helper_binds_actual_minted_sample_and_rolls_back_mismatched_leases() {
    terminal_helper_atomic_scenario(false).await;
}
#[tokio::test]
async fn terminal_helper_binds_actual_selected_unit_and_rejects_scope_widening() {
    terminal_helper_atomic_scenario(true).await;
}
