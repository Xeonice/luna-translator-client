//! Google authority uses the existing bounded SQLite writer. No method handles
//! access/refresh token bytes; Vault and network I/O run outside these transactions.
use super::super::database::SqliteStore;
use super::*;
use app_core::{
    domain::model_config::{credentials::*, service_graph::*},
    ports::google_authorization::*,
};
use std::{num::NonZeroU32, sync::Arc};
mod authority;
mod lifecycle;
mod interactive;
/// Pure actor-local read. It never invokes a Broker or another store.call.
pub(in crate::storage) fn runtime_frame(
    conn: &Connection,
    writer_identity: std::sync::Weak<()>,
) -> AppResult<GoogleRuntimeFrame> {
    let (profile_id, generation, server_instance_id): (String, u32, String) = conn.query_row(
        "SELECT profile_id,runtime_generation,server_instance_id FROM profile_meta WHERE singleton=1 AND runtime_state='active'",
        [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(err)?.ok_or_else(|| failure("google_handler_frame_inactive"))?;
    GoogleRuntimeFrame::new(
        profile_id,
        NonZeroU32::new(generation).ok_or_else(|| failure("profile_generation_conflict"))?,
        server_instance_id,
        writer_identity,
    )
}
pub(in crate::storage) fn bound_runtime_frame(db: &Database) -> AppResult<GoogleRuntimeFrame> {
    let frame = runtime_frame(&db.conn, Arc::downgrade(&db.google_writer_identity))?;
    if frame.profile_id != db.profile_id || frame.server_instance_id != db.server_id {
        return Err(failure("google_handler_frame_mismatch"));
    }
    Ok(frame)
}
/// Read-only actor context. The writer identity comes from Database itself,
/// never from configuration labels or a serialized frame.
pub(in crate::storage) struct GoogleHandlerContext {
    writer_identity: std::sync::Weak<()>,
    registration: Option<Arc<GoogleHandlerRegistration>>,
}
pub(in crate::storage) fn handler_context(db: &Database) -> GoogleHandlerContext {
    GoogleHandlerContext {
        writer_identity: Arc::downgrade(&db.google_writer_identity),
        registration: db
            .google_handler
            .as_ref()
            .and_then(std::sync::Weak::upgrade),
    }
}
pub(in crate::storage) fn handler_ready(
    conn: &Connection,
    context: &GoogleHandlerContext,
) -> AppResult<bool> {
    let Some(registration) = &context.registration else {
        return Ok(false);
    };
    if !registration.is_live() {
        return Ok(false);
    }
    let current = match runtime_frame(conn, context.writer_identity.clone()) {
        Ok(current) => current,
        Err(error) if error.code == "google_handler_frame_inactive" => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(registration.frame() == &current)
}

/// Metadata-only readiness shared by eligibility, admission and current control
/// checks. No Vault/token/network and no fictitious OAuth credential pool. The
/// caller remains responsible for proving the actual saved connection source.
pub(in crate::storage) fn connection_authorized(
    conn: &Connection,
    c: &ConnectionV3,
) -> AppResult<bool> {
    if c.protocol != "google_translate_v3"
        || c.auth_mode != "google_oauth"
        || c.credential_pool_id.is_some()
        || c.credential_pool_revision.is_some()
    {
        return Ok(false);
    }
    let Some(reference) = &c.google_authorization else {
        return Ok(false);
    };
    let g = match authority::grant(conn, &reference.grant_id) {
        Ok(g) => g,
        Err(e)
            if matches!(
                e.code.as_str(),
                "google_grant_not_found"
                    | "google_client_not_found"
                    | "google_client_setup_unverified"
                    | "google_material_not_found"
            ) =>
        {
            return Ok(false);
        }
        Err(e) => return Err(e),
    };
    if g.state != GoogleGrantState::Active
        || g.cursor.authorization_epoch != Some(reference.authorization_epoch)
        || g.scopes_sha256 != google_scope_hash()
    {
        return Ok(false);
    }
    let Some(material) = g.material else {
        return Ok(false);
    };
    match authority::active_material(
        conn,
        &material.credential.credential_id,
        CredentialMaterialKind::GoogleOauthRefresh,
    ) {
        Ok(current) => Ok(current == material),
        Err(e) if e.code == "google_material_inactive" => Ok(false),
        Err(e) => Err(e),
    }
}
/// A read-only owned grant projection for an already proved saved-test source.
/// The caller still verifies the operation's profile, input and source proof.
pub(in crate::storage) fn prepared_test_grant(
    conn: &Connection,
    connection: &ConnectionV3,
) -> AppResult<GoogleOwnedGrant> {
    if !connection_authorized(conn, connection)? {
        return Err(failure("google_grant_not_ready"));
    }
    let reference = connection
        .google_authorization
        .as_ref()
        .ok_or_else(|| failure("google_grant_missing"))?;
    authority::grant(conn, &reference.grant_id)
}

/// Only a real SQLite actor supplies `expected_server`; no UI server assertion
/// or nested Store.call can substitute for this generation/runtime fence.
pub(in crate::storage) fn prepared_google_client_draft(
    conn: &Connection,
    authorization: &GoogleAuthorizationRef,
    expected_generation: NonZeroU32,
    expected_server: &str,
) -> AppResult<app_core::domain::model_config::service_draft::PreparedGoogleClientDraft> {
    frame(conn, expected_generation, expected_server)?;
    require_id(&authorization.grant_id)?;
    let grant = authority::grant(conn, &authorization.grant_id)?;
    if grant.state != GoogleGrantState::Active
        || grant.cursor.authorization_epoch != Some(authorization.authorization_epoch)
    {
        return Err(failure("google_grant_not_ready"));
    }
    let material = grant
        .material
        .as_ref()
        .ok_or_else(|| failure("google_material_not_found"))?;
    if authority::active_material(
        conn,
        &material.credential.credential_id,
        CredentialMaterialKind::GoogleOauthRefresh,
    )? != *material
    {
        return Err(failure("google_material_changed"));
    }
    Ok(
        app_core::domain::model_config::service_draft::PreparedGoogleClientDraft {
            mode: match grant.client.client_mode {
                GoogleClientMode::Application => "application",
                GoogleClientMode::Custom => "custom",
            }
            .into(),
            client_id: Some(grant.client.public_client_id),
            authorization: authorization.clone(),
        },
    )
}

pub(in crate::storage) fn validate_live_test_lease(
    conn: &Connection,
    record: &GoogleExecutionLeaseRecord,
    registry: &dyn GoogleTestOperationAuthority,
) -> AppResult<()> {
    if !matches!(record.request.target, GoogleLeaseTarget::TestOperation(_)) {
        return Err(failure("google_test_operation_required"));
    }
    authority::issued(conn, record, registry, None)
}

/// The real terminal test is inserted by the caller in this same transaction.
/// Its shared OperationBudget terminal fence stays with the owning controller;
/// this helper neither reacquires that fence nor opens a nested transaction.
pub(in crate::storage) fn insert_terminal_test_lease(
    tx: &Transaction,
    test_id: &str,
    record: &GoogleExecutionLeaseRecord,
    registry: &dyn GoogleTestOperationAuthority,
) -> AppResult<()> {
    authority::insert_terminal_test_lease(tx, test_id, record, registry)
}
fn failure(code: &str) -> AppError {
    AppError::new(
        code,
        "google_authority_storage",
        "Google授权来源、操作或当前档身份不符",
    )
}
fn frame(conn: &Connection, generation: NonZeroU32, server: &str) -> AppResult<()> {
    require_id(server)?;
    let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM profile_meta WHERE singleton=1 AND runtime_state='active' AND runtime_generation=?1 AND server_instance_id=?2)",params![generation.get(),server],|r|r.get(0)).map_err(err)?;
    if !valid {
        return Err(failure("google_profile_stale"));
    }
    Ok(())
}
fn db_frame(db: &Database, generation: NonZeroU32, server: &str) -> AppResult<()> {
    if db.server_id != server {
        return Err(failure("google_profile_stale"));
    }
    frame(&db.conn, generation, server)
}
fn now(value: i64) -> AppResult<()> {
    if !(0..=253_402_300_799_000).contains(&value) {
        return Err(failure("authorization_clock_invalid"));
    }
    Ok(())
}
fn next(value: u32) -> AppResult<u32> {
    value
        .checked_add(1)
        .ok_or_else(|| failure("google_revision_exhausted"))
}
fn nz(value: u32) -> AppResult<NonZeroU32> {
    NonZeroU32::new(value).ok_or_else(|| failure("google_revision_invalid"))
}
fn scoped_operation(kind: &str, operation: &str) -> String {
    id(kind, operation)
}
fn receipt_json(
    conn: &Connection,
    operation: &str,
    digest: &str,
    kind: &str,
) -> AppResult<Option<serde_json::Value>> {
    let saved:Option<(String,String,String)>=conn.query_row("SELECT request_sha256,command_kind,result_refs_json FROM operation_receipts WHERE operation_id=?1",[operation],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?;
    match saved {
        Some((hash, tag, _)) if hash != digest || tag != kind => Err(failure("operation_conflict")),
        Some((_, _, text)) => Ok(Some(parse(text)?)),
        None => Ok(None),
    }
}
fn save_receipt(
    tx: &Transaction,
    operation: &str,
    digest: &str,
    kind: &str,
    result: &serde_json::Value,
    time: i64,
) -> AppResult<()> {
    let revision = bump(tx, time)?;
    tx.execute("INSERT INTO operation_receipts(operation_id,request_sha256,command_kind,commit_revision,result_refs_json,committed_at_ms) VALUES(?1,?2,?3,?4,?5,?6)",params![operation,digest,kind,revision,json(result)?,time]).map_err(err)?;
    Ok(())
}
fn login_digest(r: &GoogleLoginRequest) -> AppResult<String> {
    canonical_hash(&(
        "google-login-v1",
        &r.operation_id,
        &r.client.client_profile_id,
        r.client.client_profile_revision.get(),
        r.profile_generation.get(),
        &r.server_instance_id,
        r.network_policy.cache_identity(),
    ))
}
fn lease_digest(r: &GoogleLeaseRequest) -> AppResult<String> {
    let target = match &r.target {
        GoogleLeaseTarget::Attempt {
            attempt_id,
            task_id,
        } => serde_json::json!(["attempt", attempt_id, task_id]),
        GoogleLeaseTarget::TestOperation(t) => serde_json::json!([
            "test",
            t.operation_id,
            match t.input_scope {
                GoogleTestInputScope::Sample => "sample",
                GoogleTestInputScope::SelectedCurrent => "selected_current",
            },
            t.input_sha256,
            t.request_semantics_sha256
        ]),
    };
    canonical_hash(&(
        "google-lease-request-v1",
        target,
        &r.config_revision_id,
        &r.connection_id,
        &r.target_policy_sha256,
        r.profile_generation.get(),
        &r.server_instance_id,
    ))
}
fn context_operation(c: &GoogleMaterialContext) -> &str {
    match c {
        GoogleMaterialContext::InitialLogin(p) => &p.request.operation_id,
        GoogleMaterialContext::Refresh { operation_id, .. } => operation_id,
    }
}
fn context_frame(c: &GoogleMaterialContext) -> (NonZeroU32, &str) {
    match c {
        GoogleMaterialContext::InitialLogin(p) => {
            (p.request.profile_generation, &p.request.server_instance_id)
        }
        GoogleMaterialContext::Refresh { request, .. } => {
            (request.profile_generation, &request.server_instance_id)
        }
    }
}
fn context_digest(c: &GoogleMaterialContext) -> AppResult<String> {
    match c {
        GoogleMaterialContext::InitialLogin(p) => canonical_hash(&(
            "google-initial-material-v1",
            login_digest(&p.request)?,
            &p.grant.cursor.grant_id,
            p.grant.cursor.control_revision,
        )),
        GoogleMaterialContext::Refresh {
            operation_id,
            request,
            expected,
            mint_material,
            network_policy,
        } => canonical_hash(&(
            "google-refresh-material-v1",
            operation_id,
            lease_digest(request)?,
            &expected.grant_id,
            expected.authorization_epoch.map(NonZeroU32::get),
            expected.material_revision.map(NonZeroU32::get),
            expected.control_revision,
            &mint_material.credential.credential_id,
            &mint_material.credential.credential_version_id,
            mint_material.lifecycle_revision,
            network_policy.cache_identity(),
        )),
    }
}
struct SqliteGoogleAuthorizationRepository {
    store: SqliteStore,
    tests: Arc<dyn GoogleTestOperationAuthority>,
}
impl SqliteStore {
    /// Trusted backend factory; the live in-memory test registry is mandatory.
    /// Merely having a database/configuration row never authorizes a test input.
    pub fn google_authorization_repository(
        &self,
        tests: Arc<dyn GoogleTestOperationAuthority>,
    ) -> Arc<dyn GoogleAuthorizationRepository> {
        Arc::new(SqliteGoogleAuthorizationRepository {
            store: self.clone(),
            tests,
        })
    }
    /// Backend-only immutable registration. "Verified" must be determined by the
    /// trusted client-configuration flow, not an unvalidated browser field. This
    /// stores no client secret and does not prove real Google client availability.
    pub fn register_google_client_profile(
        &self,
        request: GoogleLoginRequest,
        profile: GoogleClientProfile,
        time: i64,
    ) -> AppResult<()> {
        self.call(move |db| lifecycle::register_client(db, &request, &profile, time))
    }
    /// OAuth materials intentionally have no synthetic credential pool. Recover
    /// unreferenced failed/stale allocations through this dedicated private API.
    pub fn google_material_cleanup_candidates(
        &self,
        generation: NonZeroU32,
        server: String,
        time: i64,
    ) -> AppResult<Vec<GoogleVaultMaterial>> {
        self.call(move |db| lifecycle::cleanup_candidates(db, generation, &server, time))
    }
    pub fn confirm_google_material_cleaned(
        &self,
        material: GoogleVaultMaterial,
        generation: NonZeroU32,
        server: String,
        time: i64,
    ) -> AppResult<()> {
        self.call(move |db| lifecycle::confirm_cleaned(db, &material, generation, &server, time))
    }
}
impl GoogleAuthorizationRepository for SqliteGoogleAuthorizationRepository {
    fn register_google_handler(&self, destination: GoogleRuntimeFrame, owner: std::sync::Weak<()>) -> AppResult<GoogleHandlerHandle> {
        self.store.call(move |db| {
            let actual = bound_runtime_frame(db)?;
            if actual != destination { return Err(failure("google_handler_frame_mismatch")); }
            if owner.upgrade().is_none() { return Err(failure("google_handler_owner_closed")); }
            if let Some(current) = db.google_handler.as_ref().and_then(std::sync::Weak::upgrade)
                && current.is_live() {
                    if current.frame() != &actual || !current.same_owner(&owner) { return Err(failure("google_handler_already_installed")); }
                    match current.acquire() {
                        Ok(handle) => return Ok(handle),
                        Err(error) if error.code == "google_handler_closed" => {},
                        Err(error) => return Err(error),
                    }
                }
            let handle = GoogleHandlerHandle::new(actual, owner)?;
            db.google_handler = Some(Arc::downgrade(handle.registration()));
            Ok(handle)
        })
    }

    fn lease_authority(&self, r: &GoogleLeaseRequest) -> AppResult<GoogleAuthoritySnapshot> {
        let r = r.clone();
        let tests = self.tests.clone();
        self.store.call(move |db| {
            db_frame(db, r.profile_generation, &r.server_instance_id)?;
            authority::load(&db.conn, &r, tests.as_ref(), Some(&handler_context(db)))
        })
    }
    fn check_lease_authority(&self, a: &GoogleAuthoritySnapshot) -> AppResult<()> {
        let a = a.clone();
        let tests = self.tests.clone();
        self.store.call(move |db| {
            db_frame(
                db,
                a.request.profile_generation,
                &a.request.server_instance_id,
            )?;
            a.validate()?;
            if authority::load(&db.conn, &a.request, tests.as_ref(), Some(&handler_context(db)))? != a {
                return Err(failure("google_authority_changed"));
            }
            Ok(())
        })
    }
    fn check_issued_lease(&self, r: &GoogleExecutionLeaseRecord) -> AppResult<()> {
        let r = r.clone();
        let tests = self.tests.clone();
        self.store.call(move |db| {
            db_frame(
                db,
                r.request.profile_generation,
                &r.request.server_instance_id,
            )?;
            authority::issued(&db.conn, &r, tests.as_ref(), Some(&handler_context(db)))
        })
    }
    fn record_attempt_lease(&self, r: &GoogleExecutionLeaseRecord) -> AppResult<()> {
        let r = r.clone();
        let tests = self.tests.clone();
        self.store.call(move |db| {
            db_frame(
                db,
                r.request.profile_generation,
                &r.request.server_instance_id,
            )?;
            authority::record(db, &r, tests.as_ref())
        })
    }
    fn prepare_login(&self, r: &GoogleLoginRequest, time: i64) -> AppResult<GooglePendingLogin> {
        let r = r.clone();
        self.store.call(move |db| lifecycle::prepare(db, &r, time))
    }
    fn check_login(&self, p: &GooglePendingLogin) -> AppResult<()> {
        let p = p.clone();
        self.store
            .call(move |db| lifecycle::check_login(&db.conn, &p))
    }
    fn reserve_material(
        &self,
        c: &GoogleMaterialContext,
        time: i64,
    ) -> AppResult<GooglePendingMaterial> {
        let c = c.clone();
        let tests = self.tests.clone();
        self.store
            .call(move |db| lifecycle::reserve(db, &c, tests.as_ref(), time))
    }
    fn commit_material(&self, c: &GoogleMaterialCommit) -> AppResult<GoogleOwnedGrant> {
        let c = c.clone();
        let tests = self.tests.clone();
        self.store
            .call(move |db| lifecycle::commit(db, &c, tests.as_ref()))
    }
    fn release_failed_material(&self, p: &GooglePendingMaterial, time: i64) -> AppResult<bool> {
        let p = p.clone();
        self.store.call(move |db| lifecycle::release(db, &p, time))
    }
    fn owned_grant(&self, r: &GoogleGrantControlRequest) -> AppResult<GoogleOwnedGrant> {
        let r = r.clone();
        self.store.call(move |db| {
            r.validate()?;
            db_frame(db, r.profile_generation, &r.server_instance_id)?;
            let grant = authority::grant(&db.conn, &r.expected.grant_id)?;
            if grant.cursor != r.expected {
                return Err(failure("google_grant_cursor_stale"));
            }
            Ok(grant)
        })
    }
    fn invalidate_grant(
        &self,
        r: &GoogleGrantControlRequest,
        state: GoogleGrantInvalidation,
        time: i64,
    ) -> AppResult<GoogleOwnedGrant> {
        let r = r.clone();
        self.store
            .call(move |db| lifecycle::invalidate(db, &r, state, time))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readiness_distinguishes_missing_ownership_from_a_broken_schema() {
        let conn = Connection::open_in_memory().unwrap();
        let mut c:ConnectionV3=serde_json::from_value(serde_json::json!({"connectionId":"c","displayName":"Google","protocol":"google_translate_v3","deployment":"cloud","authMode":"google_oauth","endpoint":"https://translation.googleapis.com","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":false})).unwrap();
        assert!(!connection_authorized(&conn, &c).unwrap());
        c.google_authorization = Some(GoogleAuthorizationRef {
            grant_id: "grant".into(),
            authorization_epoch: nz(1).unwrap(),
        });
        assert!(connection_authorized(&conn, &c).is_err());
        conn.execute_batch("CREATE TABLE oauth_grants(grant_id TEXT,client_profile_id TEXT,client_profile_revision INTEGER,principal_state TEXT,principal_subject TEXT,authorization_epoch INTEGER,material_revision INTEGER,control_revision INTEGER);").unwrap();
        assert!(!connection_authorized(&conn, &c).unwrap());
        conn.execute("INSERT INTO oauth_grants VALUES('grant','client','not-an-integer','unknown',NULL,NULL,NULL,0)",[]).unwrap();
        assert!(connection_authorized(&conn, &c).is_err());
    }
}
