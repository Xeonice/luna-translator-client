use super::*;
use app_core::domain::{
    model_config::service_draft::materialize_base, service_execution::ServiceExecutionTarget,
};

pub(super) fn client(conn: &Connection, key: &GoogleClientKey) -> AppResult<GoogleClientProfile> {
    let (issuer,public,mode,redirect,policy,setup):(String,String,String,String,String,String)=conn.query_row("SELECT issuer,public_client_id,client_mode,redirect_policy,policy_sha256,setup_state FROM oauth_client_profiles WHERE client_profile_id=?1 AND client_profile_revision=?2",params![key.client_profile_id,key.client_profile_revision.get()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(err)?.ok_or_else(||failure("google_client_not_found"))?;
    if issuer != "google_accounts" || redirect != "loopback_pkce" {
        return Err(failure("google_client_policy_invalid"));
    }
    let result = GoogleClientProfile {
        key: key.clone(),
        public_client_id: public,
        client_mode: match mode.as_str() {
            "application" => GoogleClientMode::Application,
            "custom" => GoogleClientMode::Custom,
            _ => return Err(failure("google_client_policy_invalid")),
        },
        policy_sha256: policy,
        setup_state: match setup.as_str() {
            "unknown" => GoogleClientSetup::Unknown,
            "verified" => GoogleClientSetup::Verified,
            "unsupported" => GoogleClientSetup::Unsupported,
            _ => return Err(failure("google_client_policy_invalid")),
        },
    };
    result.validate_verified()?;
    Ok(result)
}
fn material_row(
    conn: &Connection,
    credential: &str,
    kind: CredentialMaterialKind,
) -> AppResult<(GoogleVaultMaterial, String)> {
    let (vault,actual,state,lifecycle,schema):(String,String,String,u32,Option<u32>)=conn.query_row("SELECT vault_ref,auth_kind,state,lifecycle_revision,envelope_schema FROM credential_refs WHERE credential_id=?1",[credential],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(err)?.ok_or_else(||failure("google_material_not_found"))?;
    if actual != kind.key() || schema != Some(1) {
        return Err(failure("google_material_kind_mismatch"));
    }
    let result = GoogleVaultMaterial {
        credential: CredentialVersionRef {
            credential_id: credential.into(),
            credential_version_id: vault.clone(),
        },
        vault_ref: vault,
        storage: CredentialStorageMetadata::EnvelopeV1 { kind },
        lifecycle_revision: lifecycle,
    };
    result.validate_kind(kind)?;
    Ok((result, state))
}
pub(super) fn active_material(
    conn: &Connection,
    credential: &str,
    kind: CredentialMaterialKind,
) -> AppResult<GoogleVaultMaterial> {
    let (m, state) = material_row(conn, credential, kind)?;
    if state != "active" {
        return Err(failure("google_material_inactive"));
    }
    Ok(m)
}
pub(super) fn grant(conn: &Connection, id: &str) -> AppResult<GoogleOwnedGrant> {
    let (client_id,client_revision,principal,subject,epoch,material,control):(String,u32,String,Option<String>,Option<u32>,Option<u32>,u32)=conn.query_row("SELECT client_profile_id,client_profile_revision,principal_state,principal_subject,authorization_epoch,material_revision,control_revision FROM oauth_grants WHERE grant_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional().map_err(err)?.ok_or_else(||failure("google_grant_not_found"))?;
    let client = client(
        conn,
        &GoogleClientKey {
            client_profile_id: client_id,
            client_profile_revision: nz(client_revision)?,
        },
    )?;
    let (state, scope) = if let Some(epoch) = epoch {
        let (state,scope,hash):(String,String,String)=conn.query_row("SELECT state,scope,scopes_sha256 FROM oauth_authorization_snapshots WHERE grant_id=?1 AND authorization_epoch=?2",params![id,epoch],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
        if scope != GOOGLE_TRANSLATION_SCOPE {
            return Err(failure("google_scope_mismatch"));
        }
        (
            match state.as_str() {
                "pending" => GoogleGrantState::Pending,
                "active" => GoogleGrantState::Active,
                "revoked" => GoogleGrantState::Revoked,
                "needs_reauth" => GoogleGrantState::NeedsReauth,
                _ => return Err(failure("google_grant_state_invalid")),
            },
            hash,
        )
    } else {
        (GoogleGrantState::Pending, google_scope_hash())
    };
    let secret = if let Some(revision) = material {
        let credential:String=conn.query_row("SELECT credential_id FROM oauth_token_material_revisions WHERE grant_id=?1 AND material_revision=?2 AND auth_kind='google_oauth_refresh'",params![id,revision],|r|r.get(0)).map_err(err)?;
        Some(
            material_row(
                conn,
                &credential,
                CredentialMaterialKind::GoogleOauthRefresh,
            )?
            .0,
        )
    } else {
        None
    };
    let result = GoogleOwnedGrant {
        cursor: GoogleGrantCursor {
            grant_id: id.into(),
            authorization_epoch: epoch.map(nz).transpose()?,
            material_revision: material.map(nz).transpose()?,
            control_revision: control,
        },
        client,
        state,
        scopes_sha256: scope,
        principal_state: match (principal.as_str(), subject) {
            ("unknown", None) => PrincipalState::Unknown,
            ("verified", Some(subject)) => PrincipalState::Verified { subject },
            _ => return Err(failure("google_principal_invalid")),
        },
        material: secret,
    };
    result.validate()?;
    Ok(result)
}
fn endpoint(connection: &ConnectionV3) -> AppResult<()> {
    let value = connection
        .endpoint
        .as_ref()
        .ok_or_else(|| failure("google_target_endpoint_missing"))?;
    let uri = reqwest::Url::parse(value).map_err(|_| failure("google_target_endpoint_invalid"))?;
    if uri.scheme() != "https"
        || uri.host_str().is_none()
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.query().is_some()
        || uri.fragment().is_some()
        || value
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return Err(failure("google_target_endpoint_invalid"));
    }
    Ok(())
}
pub(super) fn target(
    conn: &Connection,
    r: &GoogleLeaseRequest,
    tests: &dyn GoogleTestOperationAuthority,
    handler: Option<&super::GoogleHandlerContext>,
) -> AppResult<ConnectionV3> {
    r.validate()?;
    frame(conn, r.profile_generation, &r.server_instance_id)?;
    let connection = match &r.target {
        GoogleLeaseTarget::Attempt {
            attempt_id,
            task_id,
        } => {
            let plan =
                super::super::service_tasks::validate_google_prelease(conn, handler.ok_or_else(|| failure("google_authorization_not_ready"))?, task_id, attempt_id)?;
            let ServiceExecutionTarget::Http { plan: http } = &plan.target else {
                return Err(failure("google_target_mismatch"));
            };
            let frame_matches:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM request_tasks WHERE task_id=?1 AND config_revision_id=?2 AND profile_generation=?3 AND server_instance_id=?4)",params![task_id,r.config_revision_id,r.profile_generation.get(),r.server_instance_id],|row|row.get(0)).map_err(err)?;
            if !frame_matches
                || plan.identity.configuration_revision != r.config_revision_id
                || http.connection.connection_id != r.connection_id
            {
                return Err(failure("google_target_mismatch"));
            }
            http.connection.clone()
        }
        GoogleLeaseTarget::TestOperation(op) => {
            tests.check(r, op)?;
            let graph = materialize_base(&super::super::configuration::load_document(
                conn,
                &r.config_revision_id,
            )?)?;
            graph
                .connections
                .into_iter()
                .find(|c| c.connection_id == r.connection_id)
                .ok_or_else(|| failure("google_target_mismatch"))?
        }
    };
    if connection.protocol != "google_translate_v3"
        || !matches!(
            connection.auth_mode.as_str(),
            "google_service_account" | "google_oauth"
        )
        || (matches!(r.target, GoogleLeaseTarget::Attempt { .. })
            && !connection.availability_intent)
        || google_target_policy_sha256(&connection)? != r.target_policy_sha256
    {
        return Err(failure("google_target_mismatch"));
    }
    endpoint(&connection)?;
    let gate = super::super::service_selection::connection_control(
        conn,
        &r.config_revision_id,
        &r.connection_id,
    )?;
    let suspended: Option<bool> = conn
        .query_row(
            "SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",
            [gate],
            |row| row.get(0),
        )
        .optional()
        .map_err(err)?;
    // Explicit saved tests do not apply a service, enable a purpose or create a
    // runtime control. Existing suspension remains authoritative for both paths.
    if suspended == Some(true)
        || (matches!(r.target, GoogleLeaseTarget::Attempt { .. }) && suspended.is_none())
    {
        return Err(failure("google_connection_suspended"));
    }
    let policy =
        GoogleNetworkPolicy::from_connection(&connection.proxy_mode, &connection.network_metadata)?;
    crate::providers::google_token::validate_google_network_policy(&policy)?;
    Ok(connection)
}
fn sa_member(
    conn: &Connection,
    r: &GoogleLeaseRequest,
    c: &ConnectionV3,
    tests: &dyn GoogleTestOperationAuthority,
) -> AppResult<(String, String)> {
    let pool = c
        .credential_pool_id
        .as_ref()
        .ok_or_else(|| failure("google_pool_missing"))?;
    let revision = c
        .credential_pool_revision
        .ok_or_else(|| failure("google_pool_missing"))?;
    let (credential,member)=match &r.target{
        GoogleLeaseTarget::Attempt{attempt_id,task_id}=>conn.query_row("SELECT a.credential_id,json_extract(t.input_snapshot_json,'$.credentialAuthority.memberId') FROM request_attempts a JOIN request_tasks t ON t.task_id=a.task_id WHERE a.attempt_id=?1 AND a.task_id=?2 AND a.state='started'",params![attempt_id,task_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(|_|failure("google_source_mismatch"))?,
        GoogleLeaseTarget::TestOperation(operation)=>{
            let source=tests.service_account_source(r,operation)?;source.validate()?;
            if source.pool_id!=*pool||source.pool_revision!=revision||source.scopes_hash!=google_scope_hash(){return Err(failure("google_source_mismatch"));}
            let actual=active_material(conn,&source.credential_version.credential_id,CredentialMaterialKind::GoogleServiceAccount)?;
            if actual.credential!=source.credential_version||actual.lifecycle_revision!=source.lifecycle_revision{return Err(failure("google_source_mismatch"));}
            (source.credential_version.credential_id,source.member_id)
        }
    };
    let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM credential_pool_members m JOIN credential_pool_revisions p USING(pool_id,pool_revision) JOIN credential_pool_seals s ON s.pool_id=p.pool_id AND s.pool_revision=p.pool_revision AND s.content_hash=p.content_hash WHERE m.pool_id=?1 AND m.pool_revision=?2 AND m.member_id=?3 AND m.credential_id=?4 AND m.enabled=1 AND NOT EXISTS(SELECT 1 FROM credential_pool_gc_marks g WHERE g.pool_id=p.pool_id AND g.pool_revision=p.pool_revision))",params![pool,revision.get(),member,credential],|row|row.get(0)).map_err(err)?;
    if !valid {
        return Err(failure("google_source_mismatch"));
    }
    Ok((credential, member))
}
pub(super) fn load(
    conn: &Connection,
    r: &GoogleLeaseRequest,
    tests: &dyn GoogleTestOperationAuthority,
    handler: Option<&super::GoogleHandlerContext>,
) -> AppResult<GoogleAuthoritySnapshot> {
    let c = target(conn, r, tests, handler)?;
    let network_policy = GoogleNetworkPolicy::from_connection(&c.proxy_mode, &c.network_metadata)?;
    let (origin, material) = if c.auth_mode == "google_service_account" {
        if c.google_authorization.is_some() {
            return Err(failure("google_source_mismatch"));
        }
        let (credential, member) = sa_member(conn, r, &c, tests)?;
        let m = active_material(
            conn,
            &credential,
            CredentialMaterialKind::GoogleServiceAccount,
        )?;
        (
            GoogleLeaseOrigin::ServiceAccount(GoogleServiceAccountLeaseIdentity {
                pool_id: c
                    .credential_pool_id
                    .ok_or_else(|| failure("google_pool_missing"))?,
                pool_revision: c
                    .credential_pool_revision
                    .ok_or_else(|| failure("google_pool_missing"))?,
                member_id: member,
                credential_version: m.credential.clone(),
                lifecycle_revision: m.lifecycle_revision,
                scopes_hash: google_scope_hash(),
            }),
            m,
        )
    } else {
        if c.credential_pool_id.is_some() || c.credential_pool_revision.is_some() {
            return Err(failure("google_source_mismatch"));
        }
        let reference = c
            .google_authorization
            .ok_or_else(|| failure("google_grant_missing"))?;
        let g = grant(conn, &reference.grant_id)?;
        if g.state != GoogleGrantState::Active
            || g.cursor.authorization_epoch != Some(reference.authorization_epoch)
        {
            return Err(failure("google_grant_inactive"));
        }
        let m = g
            .material
            .ok_or_else(|| failure("google_material_not_found"))?;
        if active_material(
            conn,
            &m.credential.credential_id,
            CredentialMaterialKind::GoogleOauthRefresh,
        )? != m
        {
            return Err(failure("google_material_changed"));
        }
        let identity = GoogleAuthorizationLeaseIdentity {
            grant_id: reference.grant_id,
            authorization_epoch: reference.authorization_epoch,
            material_revision: g
                .cursor
                .material_revision
                .ok_or_else(|| failure("google_material_not_found"))?,
            credential_version: m.credential.clone(),
            principal_state: g.principal_state,
            scopes_hash: g.scopes_sha256,
        };
        (
            GoogleLeaseOrigin::UserOAuth {
                identity,
                client: g.client,
                control_revision: g.cursor.control_revision,
                lifecycle_revision: m.lifecycle_revision,
            },
            m,
        )
    };
    let a = GoogleAuthoritySnapshot {
        request: r.clone(),
        origin,
        material,
        network_policy,
    };
    a.validate()?;
    Ok(a)
}
pub(super) fn issued(
    conn: &Connection,
    r: &GoogleExecutionLeaseRecord,
    tests: &dyn GoogleTestOperationAuthority,
    handler: Option<&super::GoogleHandlerContext>,
) -> AppResult<()> {
    r.validate()?;
    let c = target(conn, &r.request, tests, handler)?;
    if r.network_policy != GoogleNetworkPolicy::from_connection(&c.proxy_mode, &c.network_metadata)?
    {
        return Err(failure("google_target_mismatch"));
    }
    match &r.origin {
        GoogleLeaseOrigin::ServiceAccount(identity) => {
            if c.auth_mode != "google_service_account"
                || c.credential_pool_id.as_deref() != Some(&identity.pool_id)
                || c.credential_pool_revision != Some(identity.pool_revision)
                || c.google_authorization.is_some()
            {
                return Err(failure("google_source_mismatch"));
            }
            let (credential, member) = sa_member(conn, &r.request, &c, tests)?;
            let material = active_material(
                conn,
                &credential,
                CredentialMaterialKind::GoogleServiceAccount,
            )?;
            if member != identity.member_id
                || material.credential != identity.credential_version
                || material.lifecycle_revision != identity.lifecycle_revision
            {
                return Err(failure("google_source_mismatch"));
            }
        }
        GoogleLeaseOrigin::UserOAuth {
            identity,
            client,
            control_revision,
            lifecycle_revision,
        } => {
            let expected = c
                .google_authorization
                .ok_or_else(|| failure("google_grant_missing"))?;
            let current = grant(conn, &identity.grant_id)?;
            if c.auth_mode != "google_oauth"
                || expected.grant_id != identity.grant_id
                || expected.authorization_epoch != identity.authorization_epoch
                || current.state != GoogleGrantState::Active
                || current.cursor.authorization_epoch != Some(identity.authorization_epoch)
                || current.cursor.control_revision < *control_revision
                || current.client != *client
                || current.principal_state != identity.principal_state
                || current.scopes_sha256 != identity.scopes_hash
                || current
                    .cursor
                    .material_revision
                    .is_none_or(|n| n < identity.material_revision)
            {
                return Err(failure("google_issued_authority_changed"));
            }
            let owned:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM oauth_token_material_revisions WHERE grant_id=?1 AND material_revision=?2 AND credential_id=?3 AND auth_kind='google_oauth_refresh')",params![identity.grant_id,identity.material_revision.get(),identity.credential_version.credential_id],|row|row.get(0)).map_err(err)?;
            let material = active_material(
                conn,
                &identity.credential_version.credential_id,
                CredentialMaterialKind::GoogleOauthRefresh,
            )?;
            if !owned
                || material.credential != identity.credential_version
                || material.lifecycle_revision != *lifecycle_revision
            {
                return Err(failure("google_source_mismatch"));
            }
        }
    }
    Ok(())
}
const LEASE_COLUMNS: &str = "lease_id,attempt_id,task_id,test_id,origin_kind,grant_id,authorization_epoch,material_revision,pool_id,pool_revision,member_id,credential_id,auth_kind,lifecycle_revision,scopes_sha256,client_policy_sha256,target_policy_sha256,minted_at_ms,expires_at_ms";
enum LeaseRowTarget<'a> {
    Attempt(&'a str, &'a str),
    Test(&'a str),
}
fn insert_row(
    conn: &Connection,
    r: &GoogleExecutionLeaseRecord,
    target: LeaseRowTarget<'_>,
) -> AppResult<bool> {
    use rusqlite::types::Value as V;
    let s = |v: &str| V::Text(v.into());
    let n = |v: u32| V::Integer(i64::from(v));
    let mut values = match target {
        LeaseRowTarget::Attempt(attempt, task) => {
            vec![s(&r.lease_id), s(attempt), s(task), V::Null]
        }
        LeaseRowTarget::Test(test) => vec![s(&r.lease_id), V::Null, V::Null, s(test)],
    };
    match &r.origin {
        GoogleLeaseOrigin::ServiceAccount(i) => values.extend([
            s("service_account"),
            V::Null,
            V::Null,
            V::Null,
            s(&i.pool_id),
            n(i.pool_revision.get()),
            s(&i.member_id),
            s(&i.credential_version.credential_id),
            s("google_service_account"),
            n(i.lifecycle_revision),
            s(&i.scopes_hash),
            V::Null,
        ]),
        GoogleLeaseOrigin::UserOAuth {
            identity: i,
            client,
            lifecycle_revision,
            ..
        } => values.extend([
            s("user_oauth"),
            s(&i.grant_id),
            n(i.authorization_epoch.get()),
            n(i.material_revision.get()),
            V::Null,
            V::Null,
            V::Null,
            s(&i.credential_version.credential_id),
            s("google_oauth_refresh"),
            n(*lifecycle_revision),
            s(&i.scopes_hash),
            s(&client.policy_sha256),
        ]),
    }
    values.extend([
        s(&r.request.target_policy_sha256),
        V::Integer(r.minted_at_ms),
        V::Integer(r.expires_at_ms),
    ]);
    let existing = conn
        .query_row(
            &format!(
                "SELECT {LEASE_COLUMNS} FROM execution_authorization_leases WHERE lease_id=?1"
            ),
            [&r.lease_id],
            |row| {
                (0..19)
                    .map(|i| row.get::<_, V>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            },
        )
        .optional()
        .map_err(err)?;
    if let Some(old) = existing {
        if old != values {
            return Err(failure("google_lease_conflict"));
        }
        return Ok(false);
    }
    conn.execute(
        &format!(
            "INSERT INTO execution_authorization_leases({LEASE_COLUMNS}) VALUES({})",
            (1..=19)
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        rusqlite::params_from_iter(values),
    )
    .map_err(err)?;
    Ok(true)
}
pub(super) fn record(
    db: &mut Database,
    r: &GoogleExecutionLeaseRecord,
    tests: &dyn GoogleTestOperationAuthority,
) -> AppResult<()> {
    let GoogleLeaseTarget::Attempt {
        attempt_id,
        task_id,
    } = &r.request.target
    else {
        return Err(failure("google_test_terminal_deferred"));
    };
    let handler = super::handler_context(db);
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    issued(&tx, r, tests, Some(&handler))?;
    if insert_row(&tx, r, LeaseRowTarget::Attempt(attempt_id, task_id))? {
        bump(&tx, r.minted_at_ms)?;
    }
    tx.commit().map_err(err)
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn summary_text<'a>(value: &'a serde_json::Value, key: &str) -> AppResult<&'a str> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| failure("google_terminal_test_mismatch"))
}
type TerminalTestRow = (
    String, Option<String>, Option<String>, Option<String>, Option<String>,
    String, String, String, Option<String>, Option<u32>, String, String, i64, i64, String,
);
fn terminal_row(conn: &Connection, test: &str, r: &GoogleExecutionLeaseRecord) -> AppResult<()> {
    let GoogleLeaseTarget::TestOperation(op) = &r.request.target else {
        return Err(failure("google_test_operation_required"));
    };
    require_id(test)?;
    let (revision,connection,model,binding,purpose,stage,semantics,scope,unit,text_revision,summary,state,started,completed,target):TerminalTestRow=conn.query_row("SELECT config_revision_id,connection_id,model_id,binding_id,purpose,stage,request_semantics_hash,input_scope,unit_id,text_revision,input_summary_json,state,started_at_ms,completed_at_ms,target_kind FROM test_records WHERE test_id=?1",[test],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?,row.get(13)?,row.get(14)?))).optional().map_err(err)?.ok_or_else(||failure("google_terminal_test_missing"))?;
    if revision != r.request.config_revision_id
        || connection.as_deref() != Some(&r.request.connection_id)
        || model.is_none()
        || binding.is_some()
        || purpose.is_some()
        || stage != "model"
        || target != "http"
        || semantics != op.request_semantics_sha256
        || !matches!(
            state.as_str(),
            "passed" | "failed" | "cancelled" | "timed_out" | "unsupported"
        )
        || started < 0
        || completed < started
        || r.minted_at_ms > completed
    {
        return Err(failure("google_terminal_test_mismatch"));
    }
    app_core::domain::model_config::service_codec::validate_unique_service_json(
        summary.as_bytes(),
    )?;
    let value: serde_json::Value = parse(summary)?;
    if summary_text(&value, "schema")? != "saved-service-test-v1"
        || summary_text(&value, "producer")? != "service_tests/v1"
        || summary_text(&value, "operationId")? != op.operation_id
        || summary_text(&value, "requestSemanticsSha256")? != op.request_semantics_sha256
        || summary_text(&value, "inputSha256")? != op.input_sha256
        || summary_text(&value, "serverInstanceId")? != r.request.server_instance_id
        || value
            .get("profileGeneration")
            .and_then(serde_json::Value::as_u64)
            != Some(u64::from(r.request.profile_generation.get()))
        || summary_text(&value, "purpose")? != "translation"
        || summary_text(&value, "outputMode")? != "text"
        || !valid_hash(summary_text(&value, "sourceSemanticsSha256")?)
    {
        return Err(failure("google_terminal_test_mismatch"));
    }
    require_id(summary_text(&value, "logicalResourceId")?)?;
    let source: SavedServiceRef = serde_json::from_value(
        value
            .get("source")
            .cloned()
            .ok_or_else(|| failure("google_terminal_test_mismatch"))?,
    )
    .map_err(|_| failure("google_terminal_test_mismatch"))?;
    let SavedServiceRef::Http { source } = &source else {
        return Err(failure("google_terminal_test_mismatch"));
    };
    if source.revision_id != revision || Some(&source.model_id) != model.as_ref() {
        return Err(failure("google_terminal_test_mismatch"));
    }
    let user_source:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM config_revisions WHERE revision_id=?1 AND revision_role='user')",[&revision],|row|row.get(0)).map_err(err)?;
    if !user_source {
        return Err(failure("google_terminal_test_mismatch"));
    }
    let input: ServiceTestInputScope = serde_json::from_value(
        value
            .get("inputScope")
            .cloned()
            .ok_or_else(|| failure("google_terminal_test_mismatch"))?,
    )
    .map_err(|_| failure("google_terminal_test_mismatch"))?;
    match (&op.input_scope, input) {
        (GoogleTestInputScope::Sample, ServiceTestInputScope::Sample {})
            if scope == "sample" && unit.is_none() && text_revision.is_none() => {}
        (
            GoogleTestInputScope::SelectedCurrent,
            ServiceTestInputScope::SelectedUnit {
                unit_id,
                text_revision: version,
                source_hash,
            },
        ) if scope == "selected_current"
            && unit.as_ref() == Some(&unit_id)
            && text_revision == Some(version.get()) =>
        {
            let current:Option<String>=conn.query_row("SELECT source_sha256 FROM reading_unit_revisions WHERE unit_id=?1 AND text_revision=?2",params![unit_id,version.get()],|row|row.get(0)).optional().map_err(err)?;
            if current.as_deref() != Some(&source_hash) {
                return Err(failure("google_terminal_test_mismatch"));
            }
        }
        _ => return Err(failure("google_terminal_test_mismatch")),
    }
    Ok(())
}
/// Caller owns the already-open terminal transaction and the shared terminal
/// cancellation fence. This never opens a transaction or creates a test row.
pub(super) fn insert_terminal_test_lease(
    tx: &Transaction,
    test_id: &str,
    r: &GoogleExecutionLeaseRecord,
    registry: &dyn GoogleTestOperationAuthority,
) -> AppResult<()> {
    terminal_row(tx, test_id, r)?;
    super::validate_live_test_lease(tx, r, registry)?;
    insert_row(tx, r, LeaseRowTarget::Test(test_id))?;
    Ok(())
}
