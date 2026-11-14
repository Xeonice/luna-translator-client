use super::*;
fn read_input(
    conn: &Connection,
    input: &PrepareServiceTestInput,
) -> AppResult<PreparedTestReading> {
    match input
        .request
        .input_scope
        .as_ref()
        .unwrap_or(&ServiceTestInputScope::Sample {})
    {
        ServiceTestInputScope::Sample {} => {
            if input.selected_token.is_some() {
                return Err(fail("test_input_scope_mismatch"));
            }
            Ok(PreparedTestReading::Sample)
        }
        ServiceTestInputScope::SelectedUnit {
            unit_id,
            text_revision,
            source_hash,
        } => {
            let u = unit(conn, unit_id, text_revision.get())?;
            if u.source_hash != *source_hash {
                return Err(fail("test_reading_identity_mismatch"));
            }
            if input.request.purpose == CatalogPurpose::ContextWord
                && input.selected_token.is_none()
            {
                return Err(fail("selected_token_required"));
            }
            if input
                .selected_token
                .as_ref()
                .is_some_and(|t| !u.tokens.contains(t))
            {
                return Err(fail("selected_token_mismatch"));
            }
            let lexical:Option<String>=conn.query_row("SELECT lexical_id FROM lexical_snapshots WHERE unit_id=?1 AND text_revision=?2 ORDER BY created_at_ms DESC,lexical_id DESC LIMIT 1",params![unit_id,text_revision.get()],|r|r.get(0)).optional().map_err(err)?;
            if input.selected_token.is_some() && lexical.is_none() {
                return Err(fail("selected_token_mismatch"));
            }
            let history = history(conn, u.game_id.as_deref(), None, 100)?.units;
            Ok(PreparedTestReading::Selected {
                unit: Box::new(u),
                selected_token: input.selected_token.clone(),
                lexical_snapshot_id: lexical,
                history,
            })
        }
    }
}
fn compile(
    document: &ConfigurationDocument,
    input: &PrepareServiceTestInput,
    reading: &PreparedTestReading,
    catalog: &ServiceCatalog,
) -> AppResult<app_core::domain::service_testing::SavedServiceTestPlan> {
    let reading = match reading {
        PreparedTestReading::Sample => TestReadingInput::Sample,
        PreparedTestReading::Selected {
            unit,
            selected_token,
            history,
            ..
        } => TestReadingInput::Selected {
            unit,
            selected_token: selected_token.as_ref(),
            history,
        },
    };
    service_request_plan::compose_saved_service_test_plan(SavedServiceTestPlanInput {
        document,
        request: &input.request,
        reading,
        catalog,
    })
}
fn authentication(
    conn: &Connection,
    plan: &app_core::domain::service_testing::SavedServiceTestPlan,
    cursors: &BTreeMap<(String, u32), u64>,
) -> AppResult<PreparedTestAuthentication> {
    let ServiceExecutionTarget::Http { plan: http } = &plan.target else {
        return Ok(PreparedTestAuthentication::None);
    };
    if http.connection.auth_mode == "google_oauth" {
        return Ok(PreparedTestAuthentication::GoogleOwnedGrant(
            google_authorization::prepared_test_grant(conn, &http.connection)?,
        ));
    }
    let Some(kind) = app_core::domain::model_config::catalog::credential_kind_for_auth_mode(
        &http.connection.auth_mode,
    )?
    else {
        return Ok(PreparedTestAuthentication::None);
    };
    let pool = http
        .connection
        .credential_pool_id
        .as_ref()
        .ok_or_else(|| fail("missing_credential"))?;
    let revision = http
        .connection
        .credential_pool_revision
        .ok_or_else(|| fail("missing_credential"))?
        .get();
    let member=super::super::saved_service_source::pool_member(
        conn,pool,revision,*cursors.get(&(pool.clone(),revision)).unwrap_or(&0),kind,fail,
    )?;
    Ok(PreparedTestAuthentication::PoolMember(PreparedTestCredential {
        selection:member.selection,storage:member.storage,lifecycle_revision:member.lifecycle_revision,pool_policy_sha256:member.pool_policy_sha256,
    }))
}
fn authenticate_existing(conn: &Connection, p: &PreparedServiceTest) -> AppResult<()> {
    match &p.authentication {
        PreparedTestAuthentication::None => {
            if let ServiceExecutionTarget::Http { plan } = &p.plan.target
                && plan.connection.auth_mode != "none" {
                    return Err(fail("credential_identity_mismatch"));
                }
        }
        PreparedTestAuthentication::PoolMember(c) => {
            let s = &c.selection;
            let row:Option<(String,String,Option<u32>,u32)>=conn.query_row("SELECT c.vault_ref,c.auth_kind,c.envelope_schema,c.lifecycle_revision FROM credential_refs c JOIN credential_pool_members m USING(credential_id) WHERE c.credential_id=?1 AND c.state='active' AND m.pool_id=?2 AND m.pool_revision=?3 AND m.member_id=?4 AND m.enabled=1",params![s.credential_id,s.pool_id,s.pool_revision,s.member_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
            let Some((vault, kind, schema, lifecycle)) = row else {
                return Err(fail("credential_unavailable"));
            };
            if vault != s.vault_ref
                || CredentialStorageMetadata::from_stored(&kind, schema)? != c.storage
                || lifecycle != c.lifecycle_revision
                || pool_witness(conn, &s.pool_id, s.pool_revision)? != c.pool_policy_sha256
            {
                return Err(fail("credential_identity_mismatch"));
            }
        }
        PreparedTestAuthentication::GoogleOwnedGrant(g) => {
            let ServiceExecutionTarget::Http { plan } = &p.plan.target else {
                return Err(fail("credential_identity_mismatch"));
            };
            let actual = google_authorization::prepared_test_grant(conn, &plan.connection)?;
            if actual.cursor.grant_id != g.cursor.grant_id
                || actual.cursor.authorization_epoch != g.cursor.authorization_epoch
                || actual.client != g.client
                || actual.scopes_sha256 != g.scopes_sha256
                || actual.principal_state != g.principal_state
            {
                return Err(fail("google_issued_authority_changed"));
            }
        }
    }
    Ok(())
}
fn verify_reading(conn: &Connection, reading: &PreparedTestReading) -> AppResult<()> {
    if let PreparedTestReading::Selected {
        unit: u,
        selected_token,
        lexical_snapshot_id,
        history,
    } = reading
    {
        let actual = unit(conn, &u.unit_id, u.text_revision)?;
        if actual.source_hash != u.source_hash
            || actual.source_text != u.source_text
            || actual.session_id != u.session_id
            || actual.game_id != u.game_id
        {
            return Err(fail("test_reading_identity_mismatch"));
        }
        if let Some(token) = selected_token {
            let lexical = lexical_snapshot_id
                .as_ref()
                .ok_or_else(|| fail("selected_token_mismatch"))?;
            let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM lexical_snapshots s JOIN lexical_tokens t USING(lexical_id) WHERE s.lexical_id=?1 AND s.unit_id=?2 AND s.text_revision=?3 AND s.source_sha256=?4 AND t.byte_start=?5 AND t.byte_end=?6 AND t.surface=?7 AND t.lemma=?8 AND coalesce(t.reading_kana,'')=?9 AND t.pos_major=?10 AND t.is_known=?11)",params![lexical,u.unit_id,u.text_revision,u.source_hash,token.byte_start,token.byte_end,token.surface,token.lemma,token.reading,token.part_of_speech,token.known],|r|r.get(0)).map_err(err)?;
            if !exists {
                return Err(fail("selected_token_mismatch"));
            }
        }
        for old in history {
            let actual = unit(conn, &old.unit_id, old.text_revision)?;
            if actual.source_hash != old.source_hash || actual.game_id != old.game_id {
                return Err(fail("test_history_changed"));
            }
            for t in &old.translations {
                let matches:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM translations tr JOIN request_tasks q USING(task_id) WHERE tr.translation_id=?1 AND q.unit_id=?2 AND q.text_revision=?3 AND tr.translated_text=?4 AND tr.target_language=?5 AND tr.provider_label_snapshot=?6)",params![t.translation_id,old.unit_id,old.text_revision,t.text,t.target_language,t.provider_label],|r|r.get(0)).map_err(err)?;
                if !matches {
                    return Err(fail("test_history_changed"));
                }
                if let Some(c) = &t.correction {
                    let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM corrections WHERE translation_id=?1 AND correction_revision=?2 AND corrected_text=?3)",params![t.translation_id,c.revision,c.text],|r|r.get(0)).map_err(err)?;
                    if !exists {
                        return Err(fail("test_history_changed"));
                    }
                }
            }
        }
    }
    Ok(())
}
type TestFingerprints = (
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
);
fn fingerprints(
    document: &ConfigurationDocument,
    input: &PrepareServiceTestInput,
    reading: &PreparedTestReading,
    auth: &PreparedTestAuthentication,
    plan: &app_core::domain::service_testing::SavedServiceTestPlan,
    catalog: &ServiceCatalog,
) -> AppResult<TestFingerprints> {
    let domain = service_request_plan::saved_service_test_evidence_domain(reading)?;
    let source = service_request_plan::saved_service_test_source_semantics(
        document,
        &input.request.source,
        input.request.purpose,
        input.request.output_mode.unwrap_or_default(),
        &domain,
        catalog,
    )?;
    let request =
        service_request_plan::saved_service_test_request_semantics(plan, &source, reading, auth)?;
    let blocker = match request.authority_coverage {
        ServiceTestAuthorityCoverage::Projected => None,
        ServiceTestAuthorityCoverage::Unresolved { code } => Some(code),
    };
    Ok((
        source.version.into(),
        source.sha256,
        request.version.into(),
        request.sha256,
        request.input_sha256,
        request.authority_sha256,
        blocker,
    ))
}
pub(super) fn prepare(
    db: &mut Database,
    input: PrepareServiceTestInput,
    started: i64,
) -> AppResult<PreparedServiceTest> {
    if db.service_test_authority.is_none() {
        return Err(fail("service_test_authority_unavailable"));
    }
    require_id(&input.operation_id)?;
    if started < 0 {
        return Err(fail("invalid_time"));
    }
    let directory = db
        .service_directory
        .clone()
        .ok_or_else(|| fail("service_directory_unavailable"))?;
    let cursors = db.pool_cursors.clone();
    let tx = db
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)
        .map_err(err)?;
    let (profile, generation, server) =
        frame(&tx, input.request.expected_profile_generation, None)?;
    if replay_invocation(&tx, &input)?.is_some() {
        return Err(fail("service_test_already_completed"));
    }
    let (document, source) = saved(&tx, &input.request.source)?;
    check_control(&tx, &source)?;
    let reading = read_input(&tx, &input)?;
    let catalog = directory.catalog();
    let plan = compile(&document, &input, &reading, &catalog)?;
    let auth = authentication(&tx, &plan, &cursors)?;
    let execution_blocker = match &plan.target {
        ServiceExecutionTarget::Http { plan } => {
            if !crate::providers::adapters::dispatch::installed(&plan.connection.protocol) {
                Some(fail("adapter_not_implemented"))
            } else {
                None
            }
        }
        ServiceExecutionTarget::Offline { .. } => {
            Some(fail("offline_resource_authority_unavailable"))
        }
    };
    let (sv, sh, rv, rh, ih, ah, ab) =
        fingerprints(&document, &input, &reading, &auth, &plan, &catalog)?;
    let p = PreparedServiceTest {
        test_id: id(
            "saved-service-test",
            &format!("{profile}:{}:{}", generation, input.operation_id),
        ),
        input,
        profile_id: profile,
        profile_generation: generation,
        server_instance_id: server,
        started_at_ms: started,
        source,
        reading,
        authentication: auth,
        plan,
        source_semantics_version: sv,
        source_semantics_sha256: sh,
        request_semantics_version: rv,
        request_semantics_sha256: rh,
        input_sha256: ih,
        authority_sha256: ah,
        authority_blocker: ab,
        execution_blocker,
    };
    tx.commit().map_err(err)?;
    Ok(p)
}
pub(super) fn validate_sql(db: &Database, p: &PreparedServiceTest, for_io: bool) -> AppResult<()> {
    let (profile, _, _) = frame(&db.conn, p.profile_generation, Some(&p.server_instance_id))?;
    if profile != p.profile_id {
        return Err(fail("profile_generation_conflict"));
    }
    let (document, source) = saved(&db.conn, &p.input.request.source)?;
    if source.source != p.source.source
        || source.logical_resource_id != p.source.logical_resource_id
        || source.codec_version != p.source.codec_version
        || source.content_sha256 != p.source.content_sha256
        || source.seal_sha256 != p.source.seal_sha256
        || source.canonical_control_id != p.source.canonical_control_id
    {
        return Err(fail("service_source_changed"));
    }
    verify_reading(&db.conn, &p.reading)?;
    let directory = db
        .service_directory
        .as_ref()
        .ok_or_else(|| fail("service_directory_unavailable"))?;
    let catalog = directory.catalog();
    let plan = compile(&document, &p.input, &p.reading, &catalog)?;
    let fp = fingerprints(
        &document,
        &p.input,
        &p.reading,
        &p.authentication,
        &plan,
        &catalog,
    )?;
    if fp
        != (
            p.source_semantics_version.clone(),
            p.source_semantics_sha256.clone(),
            p.request_semantics_version.clone(),
            p.request_semantics_sha256.clone(),
            p.input_sha256.clone(),
            p.authority_sha256.clone(),
            p.authority_blocker.clone(),
        )
    {
        return Err(fail("prepared_test_mismatch"));
    }
    // The actual plan is recomputed too; a caller cannot retain a valid digest but alter a wire field.
    let supplied = fingerprints(
        &document,
        &p.input,
        &p.reading,
        &p.authentication,
        &p.plan,
        &catalog,
    )?;
    if supplied != fp {
        return Err(fail("prepared_test_mismatch"));
    }
    if plan.identity != p.plan.identity
        || canonical_hash(&(
            &plan.target,
            &plan.policy,
            &plan.provenance,
            &plan.context_units,
            &plan.term_sources,
        ))? != canonical_hash(&(
            &p.plan.target,
            &p.plan.policy,
            &p.plan.provenance,
            &p.plan.context_units,
            &p.plan.term_sources,
        ))?
    {
        return Err(fail("prepared_test_mismatch"));
    }
    if for_io {
        if let Some(code) = &p.authority_blocker {
            return Err(fail(code));
        }
        match &plan.target {
            ServiceExecutionTarget::Http { plan } => {
                if !crate::providers::adapters::dispatch::installed(&plan.connection.protocol) {
                    return Err(fail("adapter_not_implemented"));
                }
            }
            ServiceExecutionTarget::Offline { .. } => {
                return Err(fail("offline_resource_authority_unavailable"));
            }
        };
        check_control(&db.conn, &source)?;
        authenticate_existing(&db.conn, p)?;
        if let Some(e) = &p.execution_blocker {
            return Err(e.clone());
        }
    }
    Ok(())
}
