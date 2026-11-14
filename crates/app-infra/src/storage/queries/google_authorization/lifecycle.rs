use super::*;
use serde_json::{Value, json as value};
fn key(c: &GoogleMaterialContext) -> String {
    scoped_operation("google-material", context_operation(c))
}
fn ref_text<'a>(v: &'a Value, name: &str) -> AppResult<&'a str> {
    v.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| failure("google_operation_receipt_invalid"))
}
fn referenced(conn: &Connection, credential: &str) -> AppResult<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM oauth_token_material_revisions WHERE credential_id=?1) OR EXISTS(SELECT 1 FROM credential_pool_members WHERE credential_id=?1) OR EXISTS(SELECT 1 FROM execution_authorization_leases WHERE credential_id=?1) OR EXISTS(SELECT 1 FROM request_attempts WHERE credential_id=?1)",[credential],|r|r.get(0)).map_err(err)
}
fn context_check(
    conn: &Connection,
    handler: &super::GoogleHandlerContext,
    c: &GoogleMaterialContext,
    tests: &dyn GoogleTestOperationAuthority,
) -> AppResult<()> {
    c.validate()?;
    let (generation, server) = context_frame(c);
    frame(conn, generation, server)?;
    match c {
        GoogleMaterialContext::InitialLogin(p) => check_login(conn, p),
        GoogleMaterialContext::Refresh {
            request,
            expected,
            mint_material,
            network_policy,
            ..
        } => {
            let a = authority::load(conn, request, tests, Some(handler))?;
            if a.origin.cursor().as_ref() != Some(expected)
                || a.material != *mint_material
                || a.network_policy != *network_policy
            {
                return Err(failure("google_material_context_stale"));
            }
            Ok(())
        }
    }
}
pub(super) fn register_client(
    db: &mut Database,
    r: &GoogleLoginRequest,
    p: &GoogleClientProfile,
    time: i64,
) -> AppResult<()> {
    r.validate()?;
    now(time)?;
    db_frame(db, r.profile_generation, &r.server_instance_id)?;
    if p.key != r.client {
        return Err(failure("google_client_identity_mismatch"));
    }
    // Verified client format/policy is checked here. Actual Google availability
    // must be established by the trusted setup flow, never inferred from SQL.
    p.validate_verified()?;
    let op = scoped_operation("google-client", &r.operation_id);
    let mode = match p.client_mode {
        GoogleClientMode::Application => "application",
        GoogleClientMode::Custom => "custom",
    };
    let digest = canonical_hash(&(
        "google-client-register-v1",
        login_digest(r)?,
        &p.public_client_id,
        mode,
        &p.policy_sha256,
    ))?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if receipt_json(&tx, &op, &digest, "google_client_register")?.is_some() {
        if authority::client(&tx, &p.key)? != *p {
            return Err(failure("google_client_identity_mismatch"));
        }
        return Ok(());
    }
    let present:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM oauth_client_profiles WHERE client_profile_id=?1 AND client_profile_revision=?2)",params![p.key.client_profile_id,p.key.client_profile_revision.get()],|row|row.get(0)).map_err(err)?;
    if present {
        if authority::client(&tx, &p.key)? != *p {
            return Err(failure("google_client_identity_mismatch"));
        }
    } else {
        tx.execute("INSERT INTO oauth_client_profiles(client_profile_id,client_profile_revision,issuer,public_client_id,client_mode,redirect_policy,policy_sha256,setup_state,created_at_ms) VALUES(?1,?2,'google_accounts',?3,?4,'loopback_pkce',?5,'verified',?6)",params![p.key.client_profile_id,p.key.client_profile_revision.get(),p.public_client_id,mode,p.policy_sha256,time]).map_err(err)?;
    }
    save_receipt(
        &tx,
        &op,
        &digest,
        "google_client_register",
        &value!({"clientProfileId":p.key.client_profile_id,"clientProfileRevision":p.key.client_profile_revision.get()}),
        time,
    )?;
    tx.commit().map_err(err)
}
pub(super) fn prepare(
    db: &mut Database,
    r: &GoogleLoginRequest,
    time: i64,
) -> AppResult<GooglePendingLogin> {
    r.validate()?;
    now(time)?;
    db_frame(db, r.profile_generation, &r.server_instance_id)?;
    let digest = login_digest(r)?;
    let op = scoped_operation("google-login", &r.operation_id);
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    frame(&tx, r.profile_generation, &r.server_instance_id)?;
    let grant_id = if let Some(prior) = receipt_json(&tx, &op, &digest, "google_login_prepare")? {
        ref_text(&prior, "grantId")?.to_owned()
    } else {
        authority::client(&tx, &r.client)?;
        let id = format!("google-grant-{}", uuid::Uuid::new_v4());
        tx.execute("INSERT INTO oauth_grants(grant_id,client_profile_id,client_profile_revision,principal_state,principal_subject,created_at_ms,authorization_epoch,material_revision,control_revision) VALUES(?1,?2,?3,'unknown',NULL,?4,NULL,NULL,0)",params![id,r.client.client_profile_id,r.client.client_profile_revision.get(),time]).map_err(err)?;
        save_receipt(
            &tx,
            &op,
            &digest,
            "google_login_prepare",
            &value!({"grantId":id,"profileGeneration":r.profile_generation.get(),"serverInstanceId":r.server_instance_id}),
            time,
        )?;
        id
    };
    let pending = GooglePendingLogin {
        request: r.clone(),
        grant: authority::grant(&tx, &grant_id)?,
    };
    pending.validate()?;
    check_login(&tx, &pending)?;
    tx.commit().map_err(err)?;
    Ok(pending)
}
pub(super) fn check_login(conn: &Connection, p: &GooglePendingLogin) -> AppResult<()> {
    p.validate()?;
    frame(
        conn,
        p.request.profile_generation,
        &p.request.server_instance_id,
    )?;
    let receipt = receipt_json(
        conn,
        &scoped_operation("google-login", &p.request.operation_id),
        &login_digest(&p.request)?,
        "google_login_prepare",
    )?
    .ok_or_else(|| failure("google_login_not_owned"))?;
    if ref_text(&receipt, "grantId")? != p.grant.cursor.grant_id
        || authority::grant(conn, &p.grant.cursor.grant_id)? != p.grant
    {
        return Err(failure("google_login_not_owned"));
    }
    Ok(())
}
fn pending_from_receipt(
    conn: &Connection,
    c: &GoogleMaterialContext,
    v: &Value,
) -> AppResult<GooglePendingMaterial> {
    let credential = ref_text(v, "credentialId")?;
    let (vault,state,life):(String,String,u32)=conn.query_row("SELECT vault_ref,state,lifecycle_revision FROM credential_refs WHERE credential_id=?1 AND auth_kind='google_oauth_refresh' AND envelope_schema=1",[credential],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
    if state != "pending" || life != 0 || ref_text(v, "vaultRef")? != vault {
        return Err(failure("google_material_reservation_stale"));
    }
    Ok(GooglePendingMaterial {
        allocation_id: ref_text(v, "allocationId")?.into(),
        context: c.clone(),
        material: GoogleVaultMaterial {
            credential: CredentialVersionRef {
                credential_id: credential.into(),
                credential_version_id: vault.clone(),
            },
            vault_ref: vault,
            storage: CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::GoogleOauthRefresh,
            },
            lifecycle_revision: 0,
        },
    })
}
pub(super) fn reserve(
    db: &mut Database,
    c: &GoogleMaterialContext,
    tests: &dyn GoogleTestOperationAuthority,
    time: i64,
) -> AppResult<GooglePendingMaterial> {
    now(time)?;
    let (generation, server) = context_frame(c);
    db_frame(db, generation, server)?;
    let digest = context_digest(c)?;
    let op = key(c);
    let handler = super::handler_context(db);
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    context_check(&tx, &handler, c, tests)?;
    let result = if let Some(prior) = receipt_json(&tx, &op, &digest, "google_material_reserve")? {
        pending_from_receipt(&tx, c, &prior)?
    } else {
        let credential = format!("google-refresh-{}", uuid::Uuid::new_v4());
        let vault = format!("luna-google-{}", uuid::Uuid::new_v4());
        let allocation = format!("google-allocation-{}", uuid::Uuid::new_v4());
        tx.execute("INSERT INTO credential_refs(credential_id,vault_ref,masked_label,auth_kind,state,created_at_ms,lifecycle_revision,envelope_schema) VALUES(?1,?2,'Google OAuth refresh','google_oauth_refresh','pending',?3,0,1)",params![credential,vault,time]).map_err(err)?;
        let result = value!({"credentialId":credential,"vaultRef":vault,"allocationId":allocation,"profileGeneration":generation.get(),"serverInstanceId":server});
        save_receipt(&tx, &op, &digest, "google_material_reserve", &result, time)?;
        pending_from_receipt(&tx, c, &result)?
    };
    tx.commit().map_err(err)?;
    Ok(result)
}
fn reservation(conn: &Connection, p: &GooglePendingMaterial) -> AppResult<Value> {
    let prior = receipt_json(
        conn,
        &key(&p.context),
        &context_digest(&p.context)?,
        "google_material_reserve",
    )?
    .ok_or_else(|| failure("google_material_not_owned"))?;
    if ref_text(&prior, "allocationId")? != p.allocation_id
        || ref_text(&prior, "credentialId")? != p.material.credential.credential_id
        || ref_text(&prior, "vaultRef")? != p.material.vault_ref
        || p.material.credential.credential_version_id != p.material.vault_ref
        || p.material.lifecycle_revision != 0
    {
        return Err(failure("google_material_not_owned"));
    }
    p.material
        .validate_kind(CredentialMaterialKind::GoogleOauthRefresh)?;
    Ok(prior)
}
pub(super) fn commit(
    db: &mut Database,
    c: &GoogleMaterialCommit,
    tests: &dyn GoogleTestOperationAuthority,
) -> AppResult<GoogleOwnedGrant> {
    now(c.committed_at_ms)?;
    let p = &c.pending;
    let (generation, server) = context_frame(&p.context);
    db_frame(db, generation, server)?;
    if c.scopes_sha256 != google_scope_hash()
        || !matches!(
            (&p.context, c.transition),
            (
                GoogleMaterialContext::InitialLogin(_),
                GoogleMaterialTransition::InitialLogin
            ) | (
                GoogleMaterialContext::Refresh { .. },
                GoogleMaterialTransition::Refresh
            )
        )
    {
        return Err(failure("google_material_transition_mismatch"));
    }
    let digest = canonical_hash(&(
        "google-material-commit-v1",
        context_digest(&p.context)?,
        &p.allocation_id,
        &p.material.credential.credential_id,
        &p.material.vault_ref,
        &c.scopes_sha256,
    ))?;
    let op = scoped_operation("google-material-commit", context_operation(&p.context));
    let handler = super::handler_context(db);
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    reservation(&tx, p)?;
    if let Some(prior) = receipt_json(&tx, &op, &digest, "google_material_commit")? {
        let grant = authority::grant(&tx, ref_text(&prior, "grantId")?)?;
        // A replay cannot revive a revoked epoch or substitute a later source.
        let expected_material = prior
            .get("materialRevision")
            .and_then(Value::as_u64)
            .ok_or_else(|| failure("google_operation_receipt_invalid"))?;
        if grant.state != GoogleGrantState::Active
            || grant.cursor.material_revision.map(|n| u64::from(n.get())) != Some(expected_material)
            || grant
                .material
                .as_ref()
                .is_none_or(|m| m.credential != p.material.credential)
        {
            return Err(failure("google_material_commit_stale"));
        }
        return Ok(grant);
    }
    context_check(&tx, &handler, &p.context, tests)?;
    if pending_from_receipt(&tx, &p.context, &reservation(&tx, p)?)? != *p {
        return Err(failure("google_material_not_owned"));
    }
    let expected = p.context.expected();
    let material = next(expected.material_revision.map(NonZeroU32::get).unwrap_or(0))?;
    let control = next(expected.control_revision)?;
    let epoch = match &p.context {
        GoogleMaterialContext::InitialLogin(_) => next(
            expected
                .authorization_epoch
                .map(NonZeroU32::get)
                .unwrap_or(0),
        )?,
        GoogleMaterialContext::Refresh { .. } => expected
            .authorization_epoch
            .ok_or_else(|| failure("google_grant_not_active"))?
            .get(),
    };
    let active=tx.execute("UPDATE credential_refs SET state='active',activated_at_ms=?2,lifecycle_revision=lifecycle_revision+1 WHERE credential_id=?1 AND state='pending' AND lifecycle_revision=0 AND envelope_schema=1 AND auth_kind='google_oauth_refresh'",params![p.material.credential.credential_id,c.committed_at_ms]).map_err(err)?;
    if active != 1 {
        return Err(failure("google_material_reservation_stale"));
    }
    tx.execute("INSERT INTO oauth_token_material_revisions(grant_id,material_revision,credential_id,auth_kind,parent_material_revision,created_at_ms) VALUES(?1,?2,?3,'google_oauth_refresh',?4,?5)",params![expected.grant_id,material,p.material.credential.credential_id,expected.material_revision.map(NonZeroU32::get),c.committed_at_ms]).map_err(err)?;
    if matches!(p.context, GoogleMaterialContext::InitialLogin(_)) {
        tx.execute("INSERT INTO oauth_authorization_snapshots(grant_id,authorization_epoch,state,scope,scopes_sha256,previous_epoch,created_at_ms) VALUES(?1,?2,'active',?3,?4,?5,?6)",params![expected.grant_id,epoch,GOOGLE_TRANSLATION_SCOPE,google_scope_hash(),expected.authorization_epoch.map(NonZeroU32::get),c.committed_at_ms]).map_err(err)?;
    }
    let updated=tx.execute("UPDATE oauth_grants SET authorization_epoch=?2,material_revision=?3,control_revision=?4 WHERE grant_id=?1 AND authorization_epoch IS ?5 AND material_revision IS ?6 AND control_revision=?7",params![expected.grant_id,epoch,material,control,expected.authorization_epoch.map(NonZeroU32::get),expected.material_revision.map(NonZeroU32::get),expected.control_revision]).map_err(err)?;
    if updated != 1 {
        return Err(failure("google_grant_cursor_stale"));
    }
    save_receipt(
        &tx,
        &op,
        &digest,
        "google_material_commit",
        &value!({"grantId":expected.grant_id,"materialRevision":material,"authorizationEpoch":epoch,"credentialId":p.material.credential.credential_id}),
        c.committed_at_ms,
    )?;
    let result = authority::grant(&tx, &expected.grant_id)?;
    tx.commit().map_err(err)?;
    Ok(result)
}
pub(super) fn release(db: &mut Database, p: &GooglePendingMaterial, time: i64) -> AppResult<bool> {
    now(time)?;
    let (generation, server) = context_frame(&p.context);
    db_frame(db, generation, server)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    reservation(&tx, p)?;
    if referenced(&tx, &p.material.credential.credential_id)? {
        return Ok(false);
    }
    // A valid failed allocation may outlive its login/grant context; prove the
    // exact operation receipt + profile fence and never delete an active source.
    let updated=tx.execute("UPDATE credential_refs SET state='cleanup_pending',revoked_at_ms=?2,cleanup_requested_at_ms=?2,lifecycle_revision=lifecycle_revision+1 WHERE credential_id=?1 AND vault_ref=?3 AND state='pending' AND lifecycle_revision=0",params![p.material.credential.credential_id,time,p.material.vault_ref]).map_err(err)?;
    let safe:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM credential_refs WHERE credential_id=?1 AND vault_ref=?2 AND state='cleanup_pending' AND activated_at_ms IS NULL AND cleaned_at_ms IS NULL)",params![p.material.credential.credential_id,p.material.vault_ref],|row|row.get(0)).map_err(err)?;
    if updated > 0 {
        bump(&tx, time)?;
    }
    tx.commit().map_err(err)?;
    Ok(safe)
}
pub(super) fn invalidate(
    db: &mut Database,
    r: &GoogleGrantControlRequest,
    state: GoogleGrantInvalidation,
    time: i64,
) -> AppResult<GoogleOwnedGrant> {
    r.validate()?;
    now(time)?;
    db_frame(db, r.profile_generation, &r.server_instance_id)?;
    let state = match state {
        GoogleGrantInvalidation::Revoked => "revoked",
        GoogleGrantInvalidation::NeedsReauth => "needs_reauth",
    };
    let op = scoped_operation("google-grant-control", &r.operation_id);
    let digest = canonical_hash(&(
        "google-grant-control-v1",
        &r.expected.grant_id,
        r.expected.authorization_epoch.map(NonZeroU32::get),
        r.expected.material_revision.map(NonZeroU32::get),
        r.expected.control_revision,
        r.profile_generation.get(),
        &r.server_instance_id,
        state,
        r.network_policy.cache_identity(),
    ))?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if let Some(prior) = receipt_json(&tx, &op, &digest, "google_grant_invalidate")? {
        let grant = authority::grant(&tx, ref_text(&prior, "grantId")?)?;
        if grant.cursor.authorization_epoch.map(NonZeroU32::get)
            != prior
                .get("authorizationEpoch")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        {
            return Err(failure("google_grant_cursor_stale"));
        }
        return Ok(grant);
    }
    let current = authority::grant(&tx, &r.expected.grant_id)?;
    if current.cursor != r.expected {
        return Err(failure("google_grant_cursor_stale"));
    }
    let epoch = next(
        r.expected
            .authorization_epoch
            .map(NonZeroU32::get)
            .unwrap_or(0),
    )?;
    let control = next(r.expected.control_revision)?;
    tx.execute("INSERT INTO oauth_authorization_snapshots(grant_id,authorization_epoch,state,scope,scopes_sha256,previous_epoch,created_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![r.expected.grant_id,epoch,state,GOOGLE_TRANSLATION_SCOPE,google_scope_hash(),r.expected.authorization_epoch.map(NonZeroU32::get),time]).map_err(err)?;
    let updated=tx.execute("UPDATE oauth_grants SET authorization_epoch=?2,control_revision=?3 WHERE grant_id=?1 AND authorization_epoch IS ?4 AND material_revision IS ?5 AND control_revision=?6",params![r.expected.grant_id,epoch,control,r.expected.authorization_epoch.map(NonZeroU32::get),r.expected.material_revision.map(NonZeroU32::get),r.expected.control_revision]).map_err(err)?;
    if updated != 1 {
        return Err(failure("google_grant_cursor_stale"));
    }
    tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='google_grant_invalidated',finished_at_ms=?2 WHERE state IN('queued','running') AND EXISTS(SELECT 1 FROM task_execution_targets t JOIN connections c ON c.revision_id=t.config_revision_id AND c.connection_id=t.http_connection_id WHERE t.task_id=request_tasks.task_id AND t.target_kind='http' AND c.auth_mode='google_oauth' AND c.oauth_grant_id=?1)",params![r.expected.grant_id,time]).map_err(err)?;
    tx.execute("UPDATE request_attempts SET state='cancelled',error_code='google_grant_invalidated',finished_at_ms=?2 WHERE state='started' AND task_id IN(SELECT task_id FROM request_tasks WHERE state='invalidated' AND invalidation_code='google_grant_invalidated' AND EXISTS(SELECT 1 FROM task_execution_targets t JOIN connections c ON c.revision_id=t.config_revision_id AND c.connection_id=t.http_connection_id WHERE t.task_id=request_tasks.task_id AND c.oauth_grant_id=?1))",params![r.expected.grant_id,time]).map_err(err)?;
    // Durable model-cache authority remains a separate Root integration concern:
    // no fake execution lease is minted to represent a network-free cache hit.
    save_receipt(
        &tx,
        &op,
        &digest,
        "google_grant_invalidate",
        &value!({"grantId":r.expected.grant_id,"authorizationEpoch":epoch}),
        time,
    )?;
    let result = authority::grant(&tx, &r.expected.grant_id)?;
    tx.commit().map_err(err)?;
    Ok(result)
}
pub(super) fn cleanup_candidates(
    db: &mut Database,
    generation: NonZeroU32,
    server: &str,
    time: i64,
) -> AppResult<Vec<GoogleVaultMaterial>> {
    now(time)?;
    db_frame(db, generation, server)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    let mut q=tx.prepare("SELECT c.credential_id,c.vault_ref,c.lifecycle_revision,c.state FROM credential_refs c WHERE c.auth_kind='google_oauth_refresh' AND c.activated_at_ms IS NULL AND c.cleaned_at_ms IS NULL AND (c.state='cleanup_pending' OR (c.state='pending' AND EXISTS(SELECT 1 FROM operation_receipts r WHERE r.command_kind='google_material_reserve' AND json_extract(r.result_refs_json,'$.credentialId')=c.credential_id AND (json_extract(r.result_refs_json,'$.profileGeneration')<>?1 OR json_extract(r.result_refs_json,'$.serverInstanceId')<>?2)))) ORDER BY c.created_at_ms,c.credential_id").map_err(err)?;
    let rows = q
        .query_map(params![generation.get(), server], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    drop(q);
    let mut result = vec![];
    let mut changed = false;
    for (credential, vault, mut life, state) in rows {
        if referenced(&tx, &credential)? {
            continue;
        }
        if state == "pending" {
            tx.execute("UPDATE credential_refs SET state='cleanup_pending',revoked_at_ms=?2,cleanup_requested_at_ms=?2,lifecycle_revision=lifecycle_revision+1 WHERE credential_id=?1 AND state='pending' AND lifecycle_revision=?3",params![credential,time,life]).map_err(err)?;
            life = next(life)?;
            changed = true;
        }
        result.push(GoogleVaultMaterial {
            credential: CredentialVersionRef {
                credential_id: credential,
                credential_version_id: vault.clone(),
            },
            vault_ref: vault,
            storage: CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::GoogleOauthRefresh,
            },
            lifecycle_revision: life,
        });
    }
    if changed {
        bump(&tx, time)?;
    }
    tx.commit().map_err(err)?;
    Ok(result)
}
pub(super) fn confirm_cleaned(
    db: &mut Database,
    m: &GoogleVaultMaterial,
    generation: NonZeroU32,
    server: &str,
    time: i64,
) -> AppResult<()> {
    now(time)?;
    db_frame(db, generation, server)?;
    m.validate_kind(CredentialMaterialKind::GoogleOauthRefresh)?;
    next(m.lifecycle_revision)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if referenced(&tx, &m.credential.credential_id)? {
        return Err(failure("google_cleanup_referenced"));
    }
    if tx.execute("UPDATE credential_refs SET cleaned_at_ms=?3,lifecycle_revision=lifecycle_revision+1 WHERE credential_id=?1 AND vault_ref=?2 AND state='cleanup_pending' AND activated_at_ms IS NULL AND lifecycle_revision=?4 AND cleaned_at_ms IS NULL",params![m.credential.credential_id,m.vault_ref,time,m.lifecycle_revision]).map_err(err)?!=1{return Err(failure("google_cleanup_stale"));}
    bump(&tx, time)?;
    tx.commit().map_err(err)
}
