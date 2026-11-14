//! Read-only saved resource projection and eligibility.
use super::*;
pub(super) fn logical(conn: &Connection, source: &SavedServiceRef) -> AppResult<String> {
    let (kind, revision, id) = match source {
        SavedServiceRef::Http { source } => ("http", &source.revision_id, &source.model_id),
        SavedServiceRef::Offline { source } => ("offline", &source.revision_id, &source.config_id),
    };
    let original:Option<String>=conn.query_row("SELECT coalesce(source_model_id,source_offline_config_id) FROM activation_resource_materializations WHERE effective_revision_id=?1 AND (effective_model_id=?2 OR effective_offline_config_id=?2)",params![revision,id],|r|r.get(0)).optional().map_err(err)?;
    Ok(format!("{kind}:{}", original.as_deref().unwrap_or(id)))
}
pub(super) fn source_for(
    conn: &Connection,
    graph: &ConfigurationV3,
    target: &EngineTarget,
) -> AppResult<SavedServiceRef> {
    let current = resource_ref(&graph.revision_id, target);
    let ms = aliases(conn, &graph.revision_id)?;
    Ok(ms
        .into_iter()
        .find(|a| match target {
            EngineTarget::Http { model_id } => a.effective_model.as_ref() == Some(model_id),
            EngineTarget::Offline { offline_config_id } => {
                a.effective_offline.as_ref() == Some(offline_config_id)
            }
        })
        .map(|a| a.source)
        .unwrap_or(current))
}
pub(super) fn binding<'a>(
    g: &'a ConfigurationV3,
    scope: &str,
    purpose: CatalogPurpose,
) -> Option<&'a PurposeBindingV3> {
    g.bindings
        .iter()
        .find(|b| b.scope_key == scope && b.purpose == purpose)
}
pub(super) fn effective<'a>(
    g: &'a ConfigurationV3,
    scope: &str,
    p: CatalogPurpose,
) -> Option<&'a PurposeBindingV3> {
    binding(g, scope, p)
        .filter(|b| b.binding_mode == "override")
        .or_else(|| binding(g, "global", p).filter(|b| b.binding_mode == "override"))
}
pub(super) fn projection(
    conn: &Connection,
    scope: &ServiceScope,
    p: CatalogPurpose,
) -> AppResult<EffectiveServiceSelection> {
    let scope_key = scope.scope_key()?;
    let Some((revision, _)) = head(conn, "applied")? else {
        return Ok(EffectiveServiceSelection {
            requested_scope: scope.clone(),
            effective_scope: ServiceScope::Global {},
            selection_revision: 0,
            effective_selection_revision: 0,
            main: None,
            comparisons: vec![],
            purpose_enabled: false,
        });
    };
    let g = materialize_base(&configuration::load_document(conn, &revision)?)?;
    let revisions = selections(conn, &revision)?;
    let requested = binding(&g, &scope_key, p);
    let selected = effective(&g, &scope_key, p);
    let mut refs = Vec::new();
    if let Some(b) = selected {
        for e in &b.engines {
            refs.push(source_for(conn, &g, &e.target)?);
        }
    }
    let runtime_enabled: Option<bool> = conn
        .query_row(
            "SELECT is_enabled FROM purpose_runtime_states WHERE scope_key=?1 AND purpose=?2",
            params![
                selected.map(|b| b.scope_key.as_str()).unwrap_or("global"),
                purpose(p)
            ],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    Ok(EffectiveServiceSelection {
        requested_scope: scope.clone(),
        effective_scope: if selected.is_some_and(|b| b.scope_key != "global") {
            scope.clone()
        } else {
            ServiceScope::Global {}
        },
        selection_revision: requested
            .and_then(|b| revisions.get(&b.binding_id))
            .copied()
            .unwrap_or(0),
        effective_selection_revision: selected
            .and_then(|b| revisions.get(&b.binding_id))
            .copied()
            .unwrap_or(0),
        main: refs.first().cloned(),
        comparisons: refs.into_iter().skip(1).collect(),
        purpose_enabled: selected.is_some_and(|b| b.enabled) && runtime_enabled.unwrap_or(false),
    })
}
pub(super) fn verification(
    conn: &Connection,
    directory: &dyn ServiceDirectory,
    source: &SavedServiceRef,
    p: CatalogPurpose,
) -> AppResult<ServiceVerification> {
    use app_core::ports::service_testing::{ServiceTestComparison, ServiceTestQualificationQuery};
    let qualified = super::super::service_tests::qualification_for(
        conn,
        directory,
        &ServiceTestQualificationQuery {
            expected_profile_generation: generation(conn)?,
            source: source.clone(),
            purpose: p,
            output_mode: ServiceTestOutputMode::Text,
        },
    )?;
    let Some(test_id) = qualified.test_id else {
        return Ok(ServiceVerification::Untested {});
    };
    let Some(tested_at_ms) = qualified.tested_at_ms else {
        return Ok(ServiceVerification::Untested {});
    };
    Ok(match qualified.comparison {
        ServiceTestComparison::Stale => ServiceVerification::Stale { test_id },
        ServiceTestComparison::NotCovered => {
            // Unknown semantics cannot qualify a purpose, but an actual model
            // request failure on this exact saved object must retain its fact.
            // A record for another revision is historical, never current failure.
            let exact_failure = if qualified.status == Some(ServiceTestStatus::Failed) {
                match source {
                    SavedServiceRef::Http { source } => conn.query_row(
                        "SELECT error_code FROM test_records WHERE test_id=?1 AND config_revision_id=?2 AND model_id=?3 AND state='failed' AND (purpose IS NULL OR purpose=?4)",
                        params![test_id, source.revision_id, source.model_id, purpose(p)],
                        |row| row.get::<_, Option<String>>(0),
                    ).optional().map_err(err)?,
                    _ => None,
                }
            } else { None };
            match exact_failure {
                Some(code) => ServiceVerification::Failed { test_id, code: code.unwrap_or_else(|| "test_failed".into()), tested_at_ms },
                None => ServiceVerification::Stale { test_id },
            }
        },
        ServiceTestComparison::Compared => match qualified.status {
            Some(ServiceTestStatus::Failed) => ServiceVerification::Failed {
                test_id,
                code: "current_test_failed".into(),
                tested_at_ms,
            },
            Some(ServiceTestStatus::Passed) if qualified.complete_credential_coverage => {
                ServiceVerification::Passed {
                    test_id,
                    tested_at_ms,
                }
            }
            Some(ServiceTestStatus::Passed) => ServiceVerification::PartiallyTested {
                test_id,
                passed_credential_count: u32::try_from(qualified.covered_credentials)
                    .map_err(|_| failure("test_coverage_invalid"))?,
                total_credential_count: u32::try_from(qualified.required_credentials)
                    .map_err(|_| failure("test_coverage_invalid"))?,
                tested_at_ms,
            },
            _ => ServiceVerification::Untested {},
        },
    })
}
pub(super) fn eligibility(
    conn: &Connection,
    directory: &dyn ServiceDirectory,
    google_handler: &super::super::google_authorization::GoogleHandlerContext,
    g: &ConfigurationV3,
    source: &SavedServiceRef,
    p: CatalogPurpose,
) -> AppResult<SavedServiceEligibility> {
    use app_core::{
        application::service_request_plan::service_config_preflight,
        domain::service_execution::ServicePreflightRequirement,
    };
    let mut reasons = Vec::new();
    let mut block = |code: &str, message: &str| {
        reasons.push(ServiceEligibilityIssue {
            code: code.into(),
            message: message.into(),
            field_id: None,
        })
    };
    let target = match source {
        SavedServiceRef::Http { source } => EngineTarget::Http {
            model_id: source.model_id.clone(),
        },
        SavedServiceRef::Offline { source } => EngineTarget::Offline {
            offline_config_id: source.config_id.clone(),
        },
    };
    let legacy = match &target {
        EngineTarget::Http { model_id } => g
            .models
            .iter()
            .find(|m| &m.model_id == model_id)
            .is_some_and(|m| m.legacy_eligibility_origin.is_some()),
        _ => false,
    };
    if legacy {
        block(
            "legacy_requires_guided_configuration",
            "旧配置继续由原用途运行；请新建引导配置后用于统一选择器",
        )
    } else {
        match service_config_preflight(g, &target, p, &directory.catalog()) {
            Err(error) => block(&error.code, &error.message),
            Ok(preflight) => {
                for obligation in preflight.runtime_requirements {
                    if let ServicePreflightRequirement::MaterialCondition { reason } = obligation {
                        // The pure catalog cannot read an owned OAuth client's
                        // setup. Discharge only that exact requirement against
                        // real current grant/client/material rows; unrelated
                        // missing choices/material remain unknown.
                        let proved_google_client =
                            if reason == CatalogReason::ApplicationOauthSetupUnverified {
                                if let EngineTarget::Http { model_id } = &target {
                                    let connection =
                                        g.models.iter().find(|m| &m.model_id == model_id).and_then(
                                            |m| {
                                                g.connections
                                                    .iter()
                                                    .find(|c| c.connection_id == m.connection_id)
                                            },
                                        );
                                    if let Some(connection) = connection {
                                        super::super::google_authorization::connection_authorized(
                                            conn, connection,
                                        )?
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            };
                        if proved_google_client {
                            continue;
                        }
                        block(
                            "service_material_unknown",
                            "服务资料条件尚无可核验事实，暂不能选用",
                        )
                    }
                }
            }
        }
    }
    match &target {
        EngineTarget::Http { model_id } => {
            let m = g
                .models
                .iter()
                .find(|m| &m.model_id == model_id)
                .ok_or_else(|| failure("missing_model"))?;
            let c = g
                .connections
                .iter()
                .find(|c| c.connection_id == m.connection_id)
                .ok_or_else(|| failure("missing_connection"))?;
            if let Some(defaults) = m.request_defaults.get(&p)
                && let Err(error) =
                    super::super::service_tasks::validate_primary_route(&defaults.fallback)
                {
                    block(&error.code, &error.message);
                }
            if !crate::providers::adapters::dispatch::installed(&c.protocol) {
                block("adapter_not_implemented", "实际协议适配器尚未安装")
            }
            if !c.availability_intent {
                block("connection_disabled", "所存连接处于停用状态")
            }
            let control = connection_control(conn, &g.revision_id, &c.connection_id)?;
            let suspended: Option<bool> = conn
                .query_row(
                    "SELECT is_suspended FROM connection_runtime_states WHERE connection_id=?1",
                    [&control],
                    |r| r.get(0),
                )
                .optional()
                .map_err(err)?;
            if suspended == Some(true) {
                block("connection_suspended", "共享连接已被即时停用")
            }
            if !crate::providers::gateway::authentication_implemented(&c.auth_mode)
                || c.protocol == "google_translate_v3" && !super::super::google_authorization::handler_ready(conn, google_handler)? {

                block(
                    "google_authorization_not_ready",
                    "当前授权层尚未完成真实准入",
                )
            } else if !configuration_lifecycle::typed_connection_authorized(conn, c)? {
                block(
                    "credential_unavailable",
                    "所选确切凭据池版本没有可用的同类型材料",
                )
            }
        }
        EngineTarget::Offline { offline_config_id } => {
            let config = g
                .offline_engine_configs
                .iter()
                .find(|o| &o.id == offline_config_id)
                .ok_or_else(|| failure("missing_offline_config"))?;
            let catalog = directory.catalog();
            if catalog
                .offline_engines
                .iter()
                .find(|e| {
                    e.engine_id == config.engine.engine_id
                        && e.contract_revision == config.engine.contract_revision.get()
                })
                .is_none_or(|e| e.implementation.state != CapabilityState::Supported)
            {
                block(
                    "offline_runtime_not_implemented",
                    "实际本机引擎运行器尚未完成安装核验",
                )
            }
            // Resolution is a frozen source snapshot, never proof of current health/lease.
            block(
                "offline_authority_not_ready",
                "本机资源/安装/当前观察和短期授权尚待实际运行器核验",
            );
        }
    }
    if matches!(
        verification(conn, directory, source, p)?,
        ServiceVerification::Failed { .. }
    ) {
        block("current_test_failed", "当前确切版本最近一次实际测试失败")
    }
    Ok(if reasons.is_empty() {
        SavedServiceEligibility::Selectable {}
    } else {
        SavedServiceEligibility::Blocked { reasons }
    })
}
pub(super) fn query_saved(
    db: &Database,
    query: &SavedServiceQuery,
) -> AppResult<SavedServiceProjection> {
    check_generation(&db.conn, query.expected_profile_generation)?;
    let directory = directory(db)?;
    let google_handler = super::super::google_authorization::handler_context(db);
    let mut q=db.conn.prepare("SELECT c.revision_id FROM config_revisions c JOIN config_revision_seals s ON s.revision_id=c.revision_id WHERE c.revision_role='user' AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=c.revision_id) ORDER BY c.created_at_ms DESC,c.revision_id").map_err(err)?;
    let ids = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let mut rows = Vec::new();
    for revision in ids {
        let g = materialize_base(&configuration::load_document(&db.conn, &revision)?)?;
        for m in &g.models {
            if m.template_purpose.is_some_and(|p| p != query.purpose) {
                continue;
            }
            let source = SavedServiceRef::Http {
                source: HttpConfigRef {
                    revision_id: revision.clone(),
                    model_id: m.model_id.clone(),
                },
            };
            let c = g
                .connections
                .iter()
                .find(|c| c.connection_id == m.connection_id)
                .ok_or_else(|| failure("missing_connection"))?;
            rows.push(SavedServiceRow {
                logical_resource_id: logical(&db.conn, &source)?,
                display_name: m.display_name.clone(),
                backend: if matches!(
                    c.protocol.as_str(),
                    "azure_translator"
                        | "deepl_v2"
                        | "google_translate_v2"
                        | "google_translate_v3"
                        | "baidu_vip"
                        | "youdao_v3"
                        | "tencent_tokenhub_translation"
                ) {
                    ServiceBackendKind::HttpTranslation
                } else {
                    ServiceBackendKind::HttpLlm
                },
                purpose: query.purpose,
                eligibility: eligibility(&db.conn, directory.as_ref(), &google_handler, &g, &source, query.purpose)?,
                verification: verification(&db.conn, directory.as_ref(), &source, query.purpose)?,
                source,
            });
        }
        for o in &g.offline_engine_configs {
            if query.purpose != CatalogPurpose::Translation {
                continue;
            }
            let source = SavedServiceRef::Offline {
                source: OfflineConfigRef {
                    revision_id: revision.clone(),
                    config_id: o.id.clone(),
                },
            };
            rows.push(SavedServiceRow {
                logical_resource_id: logical(&db.conn, &source)?,
                display_name: o.display_name.clone(),
                backend: if o.backend == OfflineBackend::PlatformNative {
                    ServiceBackendKind::PlatformNative
                } else {
                    ServiceBackendKind::LocalWorker
                },
                purpose: query.purpose,
                eligibility: eligibility(&db.conn, directory.as_ref(), &google_handler, &g, &source, query.purpose)?,
                verification: verification(&db.conn, directory.as_ref(), &source, query.purpose)?,
                source,
            });
        }
    }
    Ok(SavedServiceProjection {
        profile_generation: generation(&db.conn)?,
        applied_head_revision: head(&db.conn, "applied")?.map(|v| v.1).unwrap_or(0),
        selection: projection(&db.conn, &query.scope, query.purpose)?,
        rows,
    })
}
