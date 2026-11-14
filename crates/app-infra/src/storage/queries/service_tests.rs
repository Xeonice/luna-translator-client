//! Saved-source test preparation and terminal facts. No task/binding is invented.
use super::*;
use crate::storage::SqliteStore;
use app_core::domain::service_testing::{ServiceTestAuthorityCoverage, ServiceTestEvidenceDomain};
use app_core::{
    application::{
        request_plan,
        service_request_plan::{self, SavedServiceTestPlanInput, TestReadingInput},
    },
    domain::{
        model_config::{credentials::*, service_draft::materialize_base, service_graph::*},
        service_execution::ServiceExecutionTarget,
    },
    ports::{
        google_authorization::{GoogleTestOperationAuthority, google_target_policy_sha256},
        operation_budget::OperationTerminalDisposition,
        service_testing::*,
    },
};
use serde::{Deserialize, Serialize};
mod prepare;
mod qualification;
mod terminal;
use prepare::{prepare, validate_sql};
pub(in crate::storage::queries) use qualification::qualification_for;
use qualification::qualify;
use std::{num::NonZeroU32, sync::Arc};
use terminal::{disposition, finish, replay_invocation, replay_terminal};

const SUMMARY_SCHEMA: &str = "saved-service-test-v1";
const PRODUCER: &str = "service_tests/v1";
fn fail(code: &str) -> AppError {
    AppError::new(
        code,
        "service_test",
        "已保存测试的固定对象、来源或授权不可核验",
    )
}
fn source_parts(source: &SavedServiceRef) -> (&str, &str, &str) {
    super::saved_service_source::parts(source)
}
fn frame(conn: &Connection, generation: NonZeroU32, server: Option<&str>) -> AppResult<(String, NonZeroU32, String)> {
    super::saved_service_source::frame(conn, generation, server, fail)
}
fn saved(conn: &Connection, source: &SavedServiceRef) -> AppResult<(ConfigurationDocument, PreparedTestSourceProof)> {
    super::saved_service_source::saved(conn, source, fail)
}
fn check_control(conn: &Connection, proof: &PreparedTestSourceProof) -> AppResult<()> {
    super::saved_service_source::check_control(conn, proof, fail)
}
fn pool_witness(conn: &Connection, pool: &str, revision: u32) -> AppResult<String> {
    super::saved_service_source::pool_witness(conn, pool, revision, fail)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum AuthAudit {
    None,
    Pool {
        credential_id: String,
        version_sha256: String,
        pool_id: String,
        pool_revision: u32,
        member_id: String,
        pool_policy_sha256: String,
        auth_kind: String,
        envelope_schema: Option<u32>,
        lifecycle_revision: u32,
    },
    Google {
        grant_id: String,
        authorization_epoch: u32,
        client_profile_id: String,
        client_profile_revision: u32,
        client_policy_sha256: String,
        scopes_sha256: String,
        principal_sha256: String,
        target_policy_sha256: String,
    },
}
fn auth_audit(
    auth: &PreparedTestAuthentication,
    plan: &app_core::domain::service_testing::SavedServiceTestPlan,
) -> AppResult<AuthAudit> {
    match auth {
        PreparedTestAuthentication::None => Ok(AuthAudit::None),
        PreparedTestAuthentication::PoolMember(c) => {
            let (kind, schema) = match c.storage {
                CredentialStorageMetadata::LegacyRawApiKey => ("api_key", None),
                CredentialStorageMetadata::EnvelopeV1 { kind } => (kind.key(), Some(1)),
            };
            Ok(AuthAudit::Pool {
                credential_id: c.selection.credential_id.clone(),
                version_sha256: content_hash(&c.selection.vault_ref),
                pool_id: c.selection.pool_id.clone(),
                pool_revision: c.selection.pool_revision,
                member_id: c.selection.member_id.clone(),
                pool_policy_sha256: c.pool_policy_sha256.clone(),
                auth_kind: kind.into(),
                envelope_schema: schema,
                lifecycle_revision: c.lifecycle_revision,
            })
        }
        PreparedTestAuthentication::GoogleOwnedGrant(g) => {
            let ServiceExecutionTarget::Http { plan: http } = &plan.target else {
                return Err(fail("credential_identity_mismatch"));
            };
            let principal = match &g.principal_state {
                PrincipalState::Unknown => canonical_hash(&("unknown", None::<&str>))?,
                PrincipalState::Verified { subject } => {
                    canonical_hash(&("verified", Some(subject)))?
                }
            };
            Ok(AuthAudit::Google {
                grant_id: g.cursor.grant_id.clone(),
                authorization_epoch: g
                    .cursor
                    .authorization_epoch
                    .ok_or_else(|| fail("google_grant_missing"))?
                    .get(),
                client_profile_id: g.client.key.client_profile_id.clone(),
                client_profile_revision: g.client.key.client_profile_revision.get(),
                client_policy_sha256: g.client.policy_sha256.clone(),
                scopes_sha256: g.scopes_sha256.clone(),
                principal_sha256: principal,
                target_policy_sha256: google_target_policy_sha256(&http.connection)?,
            })
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TestSummary {
    schema: String,
    producer: String,
    operation_id: String,
    invocation_sha256: String,
    source: SavedServiceRef,
    logical_resource_id: String,
    purpose: CatalogPurpose,
    output_mode: ServiceTestOutputMode,
    source_semantics_version: String,
    source_semantics_sha256: String,
    request_semantics_version: String,
    request_semantics_sha256: String,
    input_sha256: String,
    authority_sha256: String,
    profile_generation: u32,
    server_instance_id: String,
    input_scope: ServiceTestInputScope,
    authentication: AuthAudit,
    evidence_game_id: Option<String>,
    evidence_source_language: String,
    lexical_snapshot_id: Option<String>,
    selected_token_sha256: Option<String>,
    fallback_covered: bool,
    request_sent: bool,
    credential_used: bool,
    authority_blocker: Option<String>,
    google_lease_observation: Option<GoogleLeaseObservation>,
}
struct SqliteServiceTestingRepository {
    store: SqliteStore,
    live: Arc<dyn ServiceTestOperationAuthority>,
    google: Arc<dyn GoogleTestOperationAuthority>,
}
impl SqliteStore {
    /// Backend-only factory. Both lenses refer to the application's same live operation owner.
    pub fn service_testing_repository(
        &self,
        live: Arc<dyn ServiceTestOperationAuthority>,
        google: Arc<dyn GoogleTestOperationAuthority>,
    ) -> AppResult<Arc<dyn ServiceTestingRepository>> {
        let install = live.clone();
        self.call(move |db| {
            if let Some(existing) = &db.service_test_authority {
                if !Arc::ptr_eq(existing, &install) {
                    return Err(fail("service_test_authority_conflict"));
                }
            } else {
                db.service_test_authority = Some(install);
            }
            Ok(())
        })?;
        Ok(Arc::new(SqliteServiceTestingRepository {
            store: self.clone(),
            live,
            google,
        }))
    }
}
impl ServiceTestingRepository for SqliteServiceTestingRepository {
    fn replay_service_test(
        &self,
        input: &PrepareServiceTestInput,
    ) -> AppResult<Option<ServiceTestResult>> {
        let input = input.clone();
        self.store
            .call(move |db| replay_invocation(&db.conn, &input))
    }

    fn prepare_service_test(
        &self,
        input: PrepareServiceTestInput,
        started_at_ms: i64,
    ) -> AppResult<PreparedServiceTest> {
        self.store.call(move |db| prepare(db, input, started_at_ms))
    }
    fn validate_prepared_test(&self, prepared: &Arc<PreparedServiceTest>) -> AppResult<()> {
        let budget = self.live.operation_budget(prepared)?;
        budget.check()?;
        let p = prepared.clone();
        let live = self.live.clone();
        self.store.call(move |db| {
            let current = live.operation_budget(&p)?;
            if !current.shares_with(&budget) {
                return Err(fail("test_budget_mismatch"));
            }
            current.check()?;
            validate_sql(db, &p, true)
        })
    }
    fn finish_service_test(
        &self,
        input: FinishServiceTestInput,
        completed_at_ms: i64,
    ) -> AppResult<ServiceTestResult> {
        let replay = input.clone();
        if let Some(r) = self.store.call(move |db| {
            frame(
                &db.conn,
                replay.prepared.profile_generation,
                Some(&replay.prepared.server_instance_id),
            )?;
            replay_terminal(&db.conn, &replay)
        })? {
            return Ok(r);
        }
        let budget = self.live.operation_budget(&input.prepared)?;
        if !budget.shares_with(&input.budget) {
            return Err(fail("test_budget_mismatch"));
        }
        let live = self.live.clone();
        let google = self.google.clone();
        budget.with_terminal_fence_for(disposition(&input.outcome), || {
            self.store
                .call(move |db| finish(db, &input, completed_at_ms, live.as_ref(), google.as_ref()))
        })
    }
    fn service_test_qualification(
        &self,
        query: ServiceTestQualificationQuery,
    ) -> AppResult<ServiceTestQualification> {
        self.store.call(move |db| qualify(db, &query))
    }
}

/// A local observation is neither an execution lease nor remote authorization success.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GoogleLeaseObservation {
    kind: String,
    authorization_eligible: bool,
    reason: String,
    lease_id: String,
    origin: String,
    credential_id: String,
    credential_version_sha256: String,
    lifecycle_revision: u32,
    grant_id: Option<String>,
    authorization_epoch: Option<u32>,
    material_revision: Option<u32>,
    pool_id: Option<String>,
    pool_revision: Option<u32>,
    member_id: Option<String>,
    scopes_sha256: String,
    client_policy_sha256: Option<String>,
    target_policy_sha256: String,
    minted_at_ms: i64,
    expires_at_ms: i64,
}
