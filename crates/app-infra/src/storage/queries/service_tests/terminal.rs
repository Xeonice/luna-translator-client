use super::*;
fn summary(p: &PreparedServiceTest, sent: bool, used: bool) -> AppResult<TestSummary> {
    let (game, language, lexical, token) = match &p.reading {
        PreparedTestReading::Sample => (None, "ja".into(), None, None),
        PreparedTestReading::Selected {
            unit,
            lexical_snapshot_id,
            selected_token,
            ..
        } => (
            unit.game_id.clone(),
            unit.source_language.clone(),
            lexical_snapshot_id.clone(),
            selected_token.as_ref().map(canonical_hash).transpose()?,
        ),
    };
    Ok(TestSummary {
        schema: SUMMARY_SCHEMA.into(),
        producer: PRODUCER.into(),
        operation_id: p.input.operation_id.clone(),
        invocation_sha256: invocation_hash(&p.input)?,
        source: p.source.source.clone(),
        logical_resource_id: p.source.logical_resource_id.clone(),
        purpose: p.input.request.purpose,
        output_mode: p.input.request.output_mode.unwrap_or_default(),
        source_semantics_version: p.source_semantics_version.clone(),
        source_semantics_sha256: p.source_semantics_sha256.clone(),
        request_semantics_version: p.request_semantics_version.clone(),
        request_semantics_sha256: p.request_semantics_sha256.clone(),
        input_sha256: p.input_sha256.clone(),
        authority_sha256: p.authority_sha256.clone(),
        profile_generation: p.profile_generation.get(),
        server_instance_id: p.server_instance_id.clone(),
        input_scope: p.input.request.input_scope.clone().unwrap_or_default(),
        authentication: auth_audit(&p.authentication, &p.plan)?,
        evidence_game_id: game,
        evidence_source_language: language,
        lexical_snapshot_id: lexical,
        selected_token_sha256: token,
        fallback_covered: false,
        request_sent: sent,
        credential_used: used,
        authority_blocker: p.authority_blocker.clone(),
        google_lease_observation: None,
    })
}
pub(super) fn disposition(outcome: &ServiceTestTerminalOutcome) -> OperationTerminalDisposition {
    match outcome {
        ServiceTestTerminalOutcome::Cancelled => OperationTerminalDisposition::Cancelled,
        ServiceTestTerminalOutcome::TimedOut => OperationTerminalDisposition::TimedOut,
        _ => OperationTerminalDisposition::Active,
    }
}
fn terminal_identity(input: &FinishServiceTestInput) -> AppResult<String> {
    let observation = match &input.outcome {
        ServiceTestTerminalOutcome::Succeeded { response } => {
            serde_json::json!({"kind":"succeeded","responseSha256":content_hash(response)})
        }
        ServiceTestTerminalOutcome::Failed { error } => {
            serde_json::json!({"kind":"failed","code":error.code,"stage":error.stage})
        }
        ServiceTestTerminalOutcome::Cancelled => serde_json::json!({"kind":"cancelled"}),
        ServiceTestTerminalOutcome::TimedOut => serde_json::json!({"kind":"timed_out"}),
        ServiceTestTerminalOutcome::Unsupported { code } => {
            serde_json::json!({"kind":"unsupported","code":code})
        }
    };
    canonical_hash(&(
        SUMMARY_SCHEMA,
        &input.prepared.test_id,
        &input.prepared.profile_id,
        input.prepared.profile_generation,
        &input.prepared.server_instance_id,
        &input.prepared.request_semantics_sha256,
        observation,
        input.request_sent,
        input.credential_used,
        input.google_lease.as_ref().map(|r| &r.lease_id),
    ))
}
pub(super) fn replay_terminal(
    conn: &Connection,
    input: &FinishServiceTestInput,
) -> AppResult<Option<ServiceTestResult>> {
    let row:Option<(String,String,String)>=conn.query_row("SELECT request_sha256,command_kind,result_refs_json FROM operation_receipts WHERE operation_id=?1",[&input.prepared.input.operation_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?;
    match row {
        None => Ok(None),
        Some((h, k, _)) if h != terminal_identity(input)? || k != "saved_service_test" => {
            Err(fail("operation_conflict"))
        }
        Some((_, _, v)) => {
            let result: ServiceTestResult = parse(v)?;
            let invoked = replay_invocation(conn, &input.prepared.input)?
                .ok_or_else(|| fail("operation_conflict"))?;
            if invoked.test_id != result.test_id {
                return Err(fail("operation_conflict"));
            }
            Ok(Some(result))
        }
    }
}
type StoredTestOutcome = (
    ServiceTestStatus,
    String,
    Option<String>,
    Option<String>,
    serde_json::Value,
    serde_json::Value,
);
fn outcome(
    input: &FinishServiceTestInput,
) -> AppResult<StoredTestOutcome> {
    let validation =
        serde_json::json!({"contract":"saved-service-output-v1","fallbackCovered":false});
    Ok(match &input.outcome {
        ServiceTestTerminalOutcome::Succeeded { response } => {
            if !input.request_sent {
                return Err(fail("test_request_not_sent"));
            }
            if !matches!(
                input.prepared.authentication,
                PreparedTestAuthentication::None
            ) && !input.credential_used
            {
                return Err(fail("test_credential_not_used"));
            }
            if let Some(e) = &input.prepared.execution_blocker {
                return Err(e.clone());
            }
            let valid = if response.trim().is_empty() || response.contains('\0') {
                false
            } else if input.prepared.plan.identity.output_mode == ServiceTestOutputMode::Structured
            {
                let source = match &input.prepared.plan.target {
                    ServiceExecutionTarget::Http { plan } => &plan.request.source_text,
                    ServiceExecutionTarget::Offline { .. } => {
                        return Err(fail("offline_resource_authority_unavailable"));
                    }
                };
                request_plan::validate_study_text(response, source).2 == "structured"
            } else {
                true
            };
            (
                if valid {
                    ServiceTestStatus::Passed
                } else {
                    ServiceTestStatus::Failed
                },
                if valid {
                    "service_request"
                } else {
                    "output_validation"
                }
                .into(),
                (!valid).then(|| "invalid_test_output".into()),
                (!valid).then(|| "output_validation".into()),
                serde_json::json!({"responseSha256":content_hash(response),"byteSize":response.len()}),
                serde_json::json!({"contract":"saved-service-output-v1","valid":valid,"outputMode":input.prepared.plan.identity.output_mode,"fallbackCovered":false}),
            )
        }
        ServiceTestTerminalOutcome::Failed { error } => (
            ServiceTestStatus::Failed,
            error.stage.clone(),
            Some(error.code.clone()),
            Some(error.stage.clone()),
            serde_json::json!({}),
            validation,
        ),
        ServiceTestTerminalOutcome::Cancelled => (
            ServiceTestStatus::Cancelled,
            "cancelled".into(),
            Some("operation_cancelled".into()),
            Some("service_test".into()),
            serde_json::json!({}),
            validation,
        ),
        ServiceTestTerminalOutcome::TimedOut => (
            ServiceTestStatus::TimedOut,
            "timeout".into(),
            Some("operation_timeout".into()),
            Some("service_test".into()),
            serde_json::json!({}),
            validation,
        ),
        ServiceTestTerminalOutcome::Unsupported { code } => (
            ServiceTestStatus::Unsupported,
            "local_resource".into(),
            Some(code.clone()),
            Some("service_test".into()),
            serde_json::json!({}),
            validation,
        ),
    })
}
fn status_name(s: &ServiceTestStatus) -> &'static str {
    match s {
        ServiceTestStatus::Passed => "passed",
        ServiceTestStatus::Failed => "failed",
        ServiceTestStatus::Cancelled => "cancelled",
        ServiceTestStatus::TimedOut => "timed_out",
        ServiceTestStatus::Unsupported => "unsupported",
    }
}
pub(super) fn finish(
    db: &mut Database,
    input: &FinishServiceTestInput,
    completed: i64,
    live: &dyn ServiceTestOperationAuthority,
    google: &dyn GoogleTestOperationAuthority,
) -> AppResult<ServiceTestResult> {
    if let Some(r) = replay_terminal(&db.conn, input)? {
        return Ok(r);
    }
    let p = &input.prepared;
    if completed < p.started_at_ms {
        return Err(fail("invalid_time"));
    }
    let budget = live.operation_budget(p)?;
    if !budget.shares_with(&input.budget) {
        return Err(fail("test_budget_mismatch"));
    }
    let d = disposition(&input.outcome);
    budget.check_terminal(d)?;
    let (status, stage, error_code, error_stage, response, output) = outcome(input)?;
    validate_sql(db, p, status == ServiceTestStatus::Passed)?;
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    if let Some(r) = replay_terminal(&tx, input)? {
        return Ok(r);
    }
    let budget2 = live.operation_budget(p)?;
    if !budget.shares_with(&budget2) {
        return Err(fail("test_budget_mismatch"));
    }
    // The writer actor serializes storage changes; recheck source and frame in this real transaction.
    frame(&tx, p.profile_generation, Some(&p.server_instance_id))?;
    let (_, proof) = saved(&tx, &p.source.source)?;
    if proof.content_sha256 != p.source.content_sha256 || proof.seal_sha256 != p.source.seal_sha256
    {
        return Err(fail("service_source_changed"));
    }
    let mut sum = summary(p, input.request_sent, input.credential_used)?;
    let stopped = matches!(
        status,
        ServiceTestStatus::Cancelled | ServiceTestStatus::TimedOut
    );
    if stopped
        && let Some(lease) = &input.google_lease {
            sum.google_lease_observation =
                Some(observe_stopped_lease(&tx, p, lease, completed, &status)?);
        }

    let (_, revision, resource) = source_parts(&p.source.source);
    let (kind, con, model, unit_id, text_revision, sql_scope) = match &p.plan.target {
        ServiceExecutionTarget::Http { plan } => {
            let (u, r) = match &p.reading {
                PreparedTestReading::Sample => (None, None),
                PreparedTestReading::Selected { unit, .. } => {
                    (Some(unit.unit_id.clone()), Some(unit.text_revision))
                }
            };
            (
                "http",
                Some(plan.connection.connection_id.clone()),
                Some(plan.model.model_id.clone()),
                u,
                r,
                if matches!(p.reading, PreparedTestReading::Sample) {
                    "sample"
                } else {
                    "selected_current"
                },
            )
        }
        ServiceExecutionTarget::Offline { .. } => {
            if status == ServiceTestStatus::Passed {
                return Err(fail("offline_resource_authority_unavailable"));
            }
            let (u, r) = match &p.reading {
                PreparedTestReading::Sample => (None, None),
                PreparedTestReading::Selected { unit, .. } => {
                    (Some(unit.unit_id.clone()), Some(unit.text_revision))
                }
            };
            (
                "offline",
                None,
                None,
                u,
                r,
                if matches!(p.reading, PreparedTestReading::Sample) {
                    "sample"
                } else {
                    "selected_current"
                },
            )
        }
    };
    tx.execute("INSERT INTO test_records(test_id,config_revision_id,connection_id,model_id,binding_id,purpose,stage,request_semantics_hash,input_scope,unit_id,text_revision,input_summary_json,response_summary_json,output_validation_json,state,started_at_ms,completed_at_ms,error_code,error_stage,target_kind) VALUES(?1,?2,?3,?4,NULL,NULL,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",params![p.test_id,revision,con,model,if kind=="http"{"model"}else{"local"},p.request_semantics_sha256,sql_scope,unit_id,text_revision,json(&sum)?,json(&response)?,json(&output)?,status_name(&status),p.started_at_ms,completed,error_code,error_stage,kind]).map_err(err)?;
    if let PreparedTestAuthentication::PoolMember(c) = &p.authentication {
        let s = &c.selection;
        let actual_label: String = tx
            .query_row(
                "SELECT masked_label FROM credential_refs WHERE credential_id=?1",
                [&s.credential_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        let use_outcome = if !input.credential_used {
            "not_attempted"
        } else if status == ServiceTestStatus::Passed {
            "passed"
        } else if matches!(
            status,
            ServiceTestStatus::Cancelled | ServiceTestStatus::TimedOut
        ) {
            "cancelled"
        } else {
            "failed"
        };
        tx.execute("INSERT INTO test_record_credentials(test_id,credential_id,pool_id,pool_revision,member_id,masked_identity_snapshot,outcome) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![p.test_id,s.credential_id,s.pool_id,s.pool_revision,s.member_id,actual_label,use_outcome]).map_err(err)?;
    }
    if let Some(lease) = &input.google_lease {
        if !stopped {
            if !input.credential_used {
                return Err(fail("test_lease_not_committable"));
            }
            google_authorization::insert_terminal_test_lease(&tx, &p.test_id, lease, google)?;
        }
        // OAuth has no pool. Record the actual minted material without inventing one.
        if let app_core::ports::google_authorization::GoogleLeaseOrigin::UserOAuth {
            identity,
            ..
        } = &lease.origin
        {
            let label: String = tx
                .query_row(
                    "SELECT masked_label FROM credential_refs WHERE credential_id=?1",
                    [&identity.credential_version.credential_id],
                    |r| r.get(0),
                )
                .map_err(err)?;
            tx.execute("INSERT INTO test_record_credentials(test_id,credential_id,pool_id,pool_revision,member_id,masked_identity_snapshot,outcome) VALUES(?1,?2,NULL,NULL,NULL,?3,?4)",params![p.test_id,identity.credential_version.credential_id,label,if stopped{"cancelled"}else if status==ServiceTestStatus::Passed{"passed"}else{"failed"}]).map_err(err)?;
        }
    } else if status == ServiceTestStatus::Passed
        && matches!(&p.plan.target,ServiceExecutionTarget::Http{plan}if matches!(plan.connection.auth_mode.as_str(),"google_oauth"|"google_service_account"))
    {
        return Err(fail("google_test_lease_missing"));
    }
    if kind == "offline" {
        tx.execute("INSERT INTO offline_test_targets(test_id,config_revision_id,engine_config_id,phase) VALUES(?1,?2,?3,'probe')",params![p.test_id,revision,resource]).map_err(err)?;
    }
    let result = ServiceTestResult {
        source: p.source.source.clone(),
        purpose: p.input.request.purpose,
        test_id: p.test_id.clone(),
        status: status.clone(),
        stage,
        message: match status {
            ServiceTestStatus::Passed => "已实际完成指定用途请求；仅覆盖记录中的输入和凭据范围",
            ServiceTestStatus::Failed => "指定测试失败；草稿和正在运行的用途保持原值",
            ServiceTestStatus::Cancelled => "测试已取消",
            ServiceTestStatus::TimedOut => "测试已超时",
            ServiceTestStatus::Unsupported => "本机条件或运行前置尚未满足",
        }
        .into(),
        model_ids: model.into_iter().collect(),
        failure: if status == ServiceTestStatus::Cancelled { None } else { error_code.as_deref().map(|code| app_core::domain::service_testing::safe_test_failure(code,error_stage.as_deref())) },
    };
    let rev = bump(&tx, completed)?;
    tx.execute("INSERT INTO operation_receipts(operation_id,request_sha256,command_kind,commit_revision,result_refs_json,committed_at_ms) VALUES(?1,?2,'saved_service_test',?3,?4,?5)",params![p.input.operation_id,terminal_identity(input)?,rev,json(&result)?,completed]).map_err(err)?;
    budget.check_terminal(d)?;
    tx.commit().map_err(err)?;
    Ok(result)
}

fn invocation_hash(input: &PrepareServiceTestInput) -> AppResult<String> {
    canonical_hash(&(
        "saved-service-test-invocation-v1",
        &input.request,
        &input.selected_token,
    ))
}
pub(super) fn replay_invocation(
    conn: &Connection,
    input: &PrepareServiceTestInput,
) -> AppResult<Option<ServiceTestResult>> {
    require_id(&input.operation_id)?;
    frame(conn, input.request.expected_profile_generation, None)?;
    let receipt: Option<(String, String)> = conn
        .query_row(
            "SELECT command_kind,result_refs_json FROM operation_receipts WHERE operation_id=?1",
            [&input.operation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((kind, raw)) = receipt else {
        return Ok(None);
    };
    if kind != "saved_service_test" {
        return Err(fail("operation_conflict"));
    }
    let result: ServiceTestResult = parse(raw)?;
    let (summary,request,revision,model,record_state):(String,String,String,Option<String>,String)=conn.query_row("SELECT input_summary_json,request_semantics_hash,config_revision_id,model_id,state FROM test_records WHERE test_id=?1",[&result.test_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(err)?;
    app_core::domain::model_config::service_codec::validate_unique_service_json(
        summary.as_bytes(),
    )?;
    let s: TestSummary = parse(summary)?;
    let (_, source_revision, source_id) = source_parts(&input.request.source);
    if s.schema != SUMMARY_SCHEMA
        || s.producer != PRODUCER
        || s.operation_id != input.operation_id
        || s.invocation_sha256 != invocation_hash(input)?
        || s.profile_generation != input.request.expected_profile_generation.get()
        || s.source != input.request.source
        || result.source != s.source
        || result.purpose != s.purpose
        || request != s.request_semantics_sha256
        || revision != source_revision
        || record_state != status_name(&result.status)
        || matches!(&s.source, SavedServiceRef::Http { .. }) && model.as_deref() != Some(source_id)
    {
        return Err(fail("operation_conflict"));
    }
    Ok(Some(result))
}

fn observe_stopped_lease(
    conn: &Connection,
    p: &PreparedServiceTest,
    r: &app_core::ports::google_authorization::GoogleExecutionLeaseRecord,
    completed: i64,
    status: &ServiceTestStatus,
) -> AppResult<GoogleLeaseObservation> {
    use app_core::ports::google_authorization::{
        GoogleLeaseOrigin, GoogleLeaseTarget, GoogleNetworkPolicy, GoogleTestInputScope,
        google_scope_hash,
    };
    r.validate()?;
    let ServiceExecutionTarget::Http { plan } = &p.plan.target else {
        return Err(fail("test_lease_observation_mismatch"));
    };
    let GoogleLeaseTarget::TestOperation(op) = &r.request.target else {
        return Err(fail("test_lease_observation_mismatch"));
    };
    let scope = if matches!(p.reading, PreparedTestReading::Sample) {
        GoogleTestInputScope::Sample
    } else {
        GoogleTestInputScope::SelectedCurrent
    };
    if plan.connection.protocol != "google_translate_v3"
        || r.request.config_revision_id != source_parts(&p.source.source).1
        || r.request.connection_id != plan.connection.connection_id
        || r.request.profile_generation != p.profile_generation
        || r.request.server_instance_id != p.server_instance_id
        || r.request.target_policy_sha256 != google_target_policy_sha256(&plan.connection)?
        || r.network_policy
            != GoogleNetworkPolicy::from_connection(
                &plan.connection.proxy_mode,
                &plan.connection.network_metadata,
            )?
        || op.operation_id != p.input.operation_id
        || op.input_scope != scope
        || op.input_sha256 != p.input_sha256
        || op.request_semantics_sha256 != p.request_semantics_sha256
        || r.minted_at_ms > completed
    {
        return Err(fail("test_lease_observation_mismatch"));
    }
    let mut o = GoogleLeaseObservation {
        kind: "locally_observed_minted_lease_not_committed".into(),
        authorization_eligible: false,
        reason: if *status == ServiceTestStatus::Cancelled {
            "operation_cancelled"
        } else {
            "operation_timeout"
        }
        .into(),
        lease_id: r.lease_id.clone(),
        origin: String::new(),
        credential_id: String::new(),
        credential_version_sha256: String::new(),
        lifecycle_revision: 0,
        grant_id: None,
        authorization_epoch: None,
        material_revision: None,
        pool_id: None,
        pool_revision: None,
        member_id: None,
        scopes_sha256: google_scope_hash(),
        client_policy_sha256: None,
        target_policy_sha256: r.request.target_policy_sha256.clone(),
        minted_at_ms: r.minted_at_ms,
        expires_at_ms: r.expires_at_ms,
    };
    match (&p.authentication, &r.origin) {
        (PreparedTestAuthentication::PoolMember(c), GoogleLeaseOrigin::ServiceAccount(i)) => {
            if plan.connection.auth_mode != "google_service_account"
                || c.storage
                    != (CredentialStorageMetadata::EnvelopeV1 {
                        kind: CredentialMaterialKind::GoogleServiceAccount,
                    })
                || i.credential_version.credential_id != c.selection.credential_id
                || i.credential_version.credential_version_id != c.selection.vault_ref
                || i.pool_id != c.selection.pool_id
                || i.pool_revision.get() != c.selection.pool_revision
                || i.member_id != c.selection.member_id
                || i.lifecycle_revision != c.lifecycle_revision
                || i.scopes_hash != google_scope_hash()
            {
                return Err(fail("test_lease_observation_mismatch"));
            }
            o.origin = "service_account".into();
            o.credential_id = i.credential_version.credential_id.clone();
            o.credential_version_sha256 = content_hash(&i.credential_version.credential_version_id);
            o.lifecycle_revision = i.lifecycle_revision;
            o.pool_id = Some(i.pool_id.clone());
            o.pool_revision = Some(i.pool_revision.get());
            o.member_id = Some(i.member_id.clone());
        }
        (
            PreparedTestAuthentication::GoogleOwnedGrant(g),
            GoogleLeaseOrigin::UserOAuth {
                identity: i,
                client,
                lifecycle_revision,
                ..
            },
        ) => {
            if plan.connection.auth_mode != "google_oauth"
                || g.cursor.grant_id != i.grant_id
                || g.cursor.authorization_epoch != Some(i.authorization_epoch)
                || g.client != *client
                || g.scopes_sha256 != i.scopes_hash
                || g.principal_state != i.principal_state
            {
                return Err(fail("test_lease_observation_mismatch"));
            }
            // Exact historical ownership only, without an active/current permission claim.
            let owned:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM oauth_token_material_revisions m JOIN credential_refs c ON c.credential_id=m.credential_id WHERE m.grant_id=?1 AND m.material_revision=?2 AND m.credential_id=?3 AND m.auth_kind='google_oauth_refresh' AND c.vault_ref=?4 AND c.auth_kind='google_oauth_refresh' AND c.envelope_schema=1 AND c.lifecycle_revision>=?5)",params![i.grant_id,i.material_revision.get(),i.credential_version.credential_id,i.credential_version.credential_version_id,lifecycle_revision],|row|row.get(0)).map_err(err)?;
            if !owned {
                return Err(fail("test_lease_observation_mismatch"));
            }
            o.origin = "user_oauth".into();
            o.credential_id = i.credential_version.credential_id.clone();
            o.credential_version_sha256 = content_hash(&i.credential_version.credential_version_id);
            o.lifecycle_revision = *lifecycle_revision;
            o.grant_id = Some(i.grant_id.clone());
            o.authorization_epoch = Some(i.authorization_epoch.get());
            o.material_revision = Some(i.material_revision.get());
            o.client_policy_sha256 = Some(client.policy_sha256.clone());
        }
        _ => return Err(fail("test_lease_observation_mismatch")),
    }
    Ok(o)
}
