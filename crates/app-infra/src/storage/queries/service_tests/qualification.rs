use super::*;
fn parse_status(s: &str) -> AppResult<ServiceTestStatus> {
    match s {
        "passed" => Ok(ServiceTestStatus::Passed),
        "failed" => Ok(ServiceTestStatus::Failed),
        "cancelled" => Ok(ServiceTestStatus::Cancelled),
        "timed_out" => Ok(ServiceTestStatus::TimedOut),
        "unsupported" => Ok(ServiceTestStatus::Unsupported),
        _ => Err(fail("invalid_test_status")),
    }
}
fn current_audits(
    conn: &Connection,
    document: &ConfigurationDocument,
    source: &SavedServiceRef,
) -> AppResult<Vec<AuthAudit>> {
    let graph = materialize_base(document)?;
    let SavedServiceRef::Http { source } = source else {
        return Ok(vec![]);
    };
    let m = graph
        .models
        .iter()
        .find(|m| m.model_id == source.model_id)
        .ok_or_else(|| fail("missing_model"))?;
    let c = graph
        .connections
        .iter()
        .find(|c| c.connection_id == m.connection_id)
        .ok_or_else(|| fail("missing_connection"))?;
    if c.auth_mode == "none" {
        return Ok(vec![AuthAudit::None]);
    }
    if c.auth_mode == "google_oauth" {
        let g = google_authorization::prepared_test_grant(conn, c)?;
        let principal = match &g.principal_state {
            PrincipalState::Unknown => canonical_hash(&("unknown", None::<&str>))?,
            PrincipalState::Verified { subject } => canonical_hash(&("verified", Some(subject)))?,
        };
        return Ok(vec![AuthAudit::Google {
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
            target_policy_sha256: google_target_policy_sha256(c)?,
        }]);
    }
    let kind =
        app_core::domain::model_config::catalog::credential_kind_for_auth_mode(&c.auth_mode)?
            .ok_or_else(|| fail("credential_kind_mismatch"))?;
    let pool = c
        .credential_pool_id
        .as_ref()
        .ok_or_else(|| fail("missing_credential"))?;
    let rev = c
        .credential_pool_revision
        .ok_or_else(|| fail("missing_credential"))?
        .get();
    let witness = pool_witness(conn, pool, rev)?;
    let mut q=conn.prepare("SELECT c.credential_id,c.vault_ref,c.auth_kind,c.envelope_schema,c.lifecycle_revision,m.member_id FROM credential_refs c JOIN credential_pool_members m USING(credential_id) WHERE m.pool_id=?1 AND m.pool_revision=?2 AND m.enabled=1 AND c.state='active' ORDER BY m.position,m.member_id").map_err(err)?;
    q.query_map(params![pool, rev], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<u32>>(3)?,
            r.get::<_, u32>(4)?,
            r.get::<_, String>(5)?,
        ))
    })
    .map_err(err)?
    .map(|row| {
        let (id, vault, k, schema, lifecycle, member) = row.map_err(err)?;
        if k != kind.key() {
            return Err(fail("credential_kind_mismatch"));
        }
        CredentialStorageMetadata::from_stored(&k, schema)?;
        Ok(AuthAudit::Pool {
            credential_id: id,
            version_sha256: content_hash(vault),
            pool_id: pool.clone(),
            pool_revision: rev,
            member_id: member,
            pool_policy_sha256: witness.clone(),
            auth_kind: k,
            envelope_schema: schema,
            lifecycle_revision: lifecycle,
        })
    })
    .collect()
}
pub(super) fn qualify(
    db: &Database,
    query: &ServiceTestQualificationQuery,
) -> AppResult<ServiceTestQualification> {
    let directory = db
        .service_directory
        .as_ref()
        .ok_or_else(|| fail("service_directory_unavailable"))?;
    qualification_for(&db.conn, directory.as_ref(), query)
}
pub(in crate::storage::queries) fn qualification_for(
    conn: &Connection,
    directory: &dyn app_core::ports::ServiceDirectory,
    query: &ServiceTestQualificationQuery,
) -> AppResult<ServiceTestQualification> {
    frame(conn, query.expected_profile_generation, None)?;
    let (document, proof) = saved(conn, &query.source)?;
    let catalog = directory.catalog();
    let (kind, _, resource) = source_parts(&query.source);
    let audits = current_audits(conn, &document, &query.source);
    let mut result = ServiceTestQualification {
        current_source: query.source.clone(),
        tested_source: None,
        test_id: None,
        purpose: query.purpose,
        output_mode: query.output_mode,
        status: None,
        comparison: ServiceTestComparison::NotCovered,
        credentials: vec![],
        required_credentials: audits
            .as_ref()
            .map(|a| a.iter().filter(|v| !matches!(v, AuthAudit::None)).count())
            .unwrap_or(0),
        covered_credentials: 0,
        complete_credential_coverage: false,
        input_scope: None,
        tested_at_ms: None,
        reasons: vec![],
    };
    // Only substantive results participate in the authoritative per-scope
    // comparison. Cancellation/other-purpose traffic cannot evict a known
    // result. Keep at most one negative/legacy display fallback, then stream
    // every relevant pass/failure instead of imposing an arbitrary window.
    let purpose = serde_json::to_value(query.purpose)
        .map_err(err)?
        .as_str()
        .ok_or_else(|| fail("invalid_test_purpose"))?
        .to_owned();
    let mode = serde_json::to_value(query.output_mode)
        .map_err(err)?
        .as_str()
        .ok_or_else(|| fail("invalid_test_mode"))?
        .to_owned();
    let base = "SELECT t.test_id,t.state,t.completed_at_ms,t.request_semantics_hash,t.input_summary_json FROM test_records t LEFT JOIN offline_test_targets o USING(test_id) WHERE ((?1='http' AND t.model_id=?2) OR (?1='offline' AND o.engine_config_id=?2))";
    let scope = "json_extract(t.input_summary_json,'$.schema')=?3 AND json_extract(t.input_summary_json,'$.producer')=?4 AND json_extract(t.input_summary_json,'$.purpose')=?5 AND json_extract(t.input_summary_json,'$.outputMode')=?6";
    let row = |r: &rusqlite::Row<'_>| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    };
    let negative=conn.query_row(&format!("{base} AND {scope} AND t.state NOT IN('passed','failed') ORDER BY t.completed_at_ms DESC,t.test_id DESC LIMIT 1"),params![kind,resource,SUMMARY_SCHEMA,PRODUCER,purpose,mode],row).optional().map_err(err)?;
    let legacy=conn.query_row(&format!("{base} AND NOT(coalesce(json_extract(t.input_summary_json,'$.schema'),'')=?3 AND coalesce(json_extract(t.input_summary_json,'$.producer'),'')=?4) AND (t.purpose IS NULL OR t.purpose=?5) ORDER BY t.completed_at_ms DESC,t.test_id DESC LIMIT 1"),params![kind,resource,SUMMARY_SCHEMA,PRODUCER,purpose],row).optional().map_err(err)?;
    let mut stmt=conn.prepare(&format!("{base} AND {scope} AND t.state IN('passed','failed') ORDER BY t.completed_at_ms DESC,t.test_id DESC")).map_err(err)?;
    let rows = stmt
        .query_map(
            params![kind, resource, SUMMARY_SCHEMA, PRODUCER, purpose, mode],
            row,
        )
        .map_err(err)?;
    let mut seen = Vec::<AuthAudit>::new();
    let mut passed = Vec::<AuthAudit>::new();
    let mut any_failed = false;
    let mut any_comparable = false;
    let mut stale = false;
    for row in negative
        .into_iter()
        .chain(legacy)
        .map(Ok::<_, rusqlite::Error>)
        .chain(rows)
    {
        let (test, state, time, request, raw) = row.map_err(err)?;
        let value: serde_json::Value = match parse(raw.clone()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if value.get("schema").and_then(|v| v.as_str()) != Some(SUMMARY_SCHEMA)
            || value.get("producer").and_then(|v| v.as_str()) != Some(PRODUCER)
        {
            if result.test_id.is_none() {
                result.test_id = Some(test);
                result.status = Some(parse_status(&state)?);
                result.tested_at_ms = Some(time);
                result.reasons.push("legacy_test_not_covered".into())
            }
            continue;
        }
        app_core::domain::model_config::service_codec::validate_unique_service_json(
            raw.as_bytes(),
        )?;
        let s: TestSummary = parse(raw)?;
        if s.logical_resource_id != proof.logical_resource_id
            || s.purpose != query.purpose
            || s.output_mode != query.output_mode
        {
            continue;
        }
        if s.request_semantics_sha256 != request || s.fallback_covered {
            return Err(fail("invalid_service_test_summary"));
        }
        let domain = match s.input_scope {
            ServiceTestInputScope::Sample {} => ServiceTestEvidenceDomain::SampleJapaneseV1,
            ServiceTestInputScope::SelectedUnit { .. } => ServiceTestEvidenceDomain::Selected {
                game_id: s.evidence_game_id.clone(),
                source_language: s.evidence_source_language.clone(),
            },
        };
        let fingerprint = service_request_plan::saved_service_test_source_semantics(
            &document,
            &query.source,
            query.purpose,
            query.output_mode,
            &domain,
            &catalog,
        );
        let comparable = fingerprint.as_ref().is_ok_and(|fp| {
            fp.version == s.source_semantics_version && fp.sha256 == s.source_semantics_sha256
        }) && audits.as_ref().is_ok_and(|a| a.contains(&s.authentication));
        if !comparable {
            stale = true;
            if result.test_id.is_none() || result.comparison == ServiceTestComparison::NotCovered {
                result.test_id = Some(test);
                result.tested_source = Some(s.source);
                result.status = Some(parse_status(&state)?);
                result.tested_at_ms = Some(time);
                result.input_scope = Some(s.input_scope);
                result.comparison = ServiceTestComparison::Stale;
            }
            continue;
        }
        any_comparable = true;
        let status = parse_status(&state)?;
        // Cancel/timeout do not erase a previous actual pass/failure for this same scope.
        if !matches!(
            status,
            ServiceTestStatus::Passed | ServiceTestStatus::Failed
        ) {
            if result.test_id.is_none() {
                result.test_id = Some(test);
                result.tested_source = Some(s.source);
                result.status = Some(status);
                result.tested_at_ms = Some(time);
                result.input_scope = Some(s.input_scope)
            }
            continue;
        }
        if seen.contains(&s.authentication) {
            continue;
        }
        seen.push(s.authentication.clone());
        if status == ServiceTestStatus::Passed
            && s.request_sent
            && (matches!(s.authentication, AuthAudit::None) || s.credential_used)
            && s.authority_blocker.is_none()
        {
            passed.push(s.authentication.clone())
        } else if status == ServiceTestStatus::Failed {
            any_failed = true
        }
        if let AuthAudit::Pool {
            credential_id,
            pool_id,
            pool_revision,
            member_id,
            ..
        } = &s.authentication
        {
            result.credentials.push(ServiceTestCredentialCoverage {
                credential_id: credential_id.clone(),
                pool_id: Some(pool_id.clone()),
                pool_revision: Some(*pool_revision),
                member_id: Some(member_id.clone()),
                status: status.clone(),
            });
        }
        if result.comparison != ServiceTestComparison::Compared {
            result.test_id = Some(test);
            result.tested_source = Some(s.source.clone());
            result.status = Some(status);
            result.tested_at_ms = Some(time);
            result.input_scope = Some(s.input_scope.clone());
            result.comparison = ServiceTestComparison::Compared;
        }
    }
    if let Ok(current) = audits {
        result.covered_credentials = current
            .iter()
            .filter(|a| !matches!(a, AuthAudit::None) && passed.contains(a))
            .count();
        result.complete_credential_coverage =
            !current.is_empty() && current.iter().all(|a| passed.contains(a));
    }
    if any_failed {
        result.status = Some(ServiceTestStatus::Failed);
        result.reasons.push("current_test_failed".into())
    } else if result.complete_credential_coverage {
        result.status = Some(ServiceTestStatus::Passed)
    } else if !passed.is_empty() {
        result.status = Some(ServiceTestStatus::Passed);
        result.reasons.push("partial_credential_coverage".into())
    }
    if any_comparable && result.comparison == ServiceTestComparison::NotCovered {
        result.comparison = ServiceTestComparison::Compared;
    } else if stale && !any_comparable {
        result.comparison = ServiceTestComparison::Stale;
        result
            .reasons
            .push("test_semantics_or_authority_changed".into())
    }
    if kind == "offline" {
        result.complete_credential_coverage = false;
        result
            .reasons
            .push("offline_test_authority_unresolved".into());
    }
    Ok(result)
}
