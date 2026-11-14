//! Explicit saved-source tests share the real target compiler without fabricating
//! active bindings, dispatches, credentials or persisted reading history.
use super::*;
use crate::domain::service_testing::{
    SavedServiceTestIdentity, SavedServiceTestInputIdentity, SavedServiceTestPlan,
};

const SAMPLE_ID: &str = "saved-service-test-sample.ja.v1";
const SAMPLE_TEXT: &str = "雨がやんで、空が明るくなった。";

pub enum TestReadingInput<'a> {
    Sample,
    Selected {
        unit: &'a ReadingUnit,
        selected_token: Option<&'a LexicalToken>,
        history: &'a [ReadingUnit],
    },
}
pub struct SavedServiceTestPlanInput<'a> {
    /// The caller must resolve this exact saved source with the authoritative
    /// Repository loader. A pure Core plan itself cannot grant storage authority.
    pub document: &'a ConfigurationDocument,
    pub request: &'a SavedServiceTestRequest,
    pub reading: TestReadingInput<'a>,
    pub catalog: &'a ServiceCatalog,
}
/// A fixed sample is a private ephemeral adapter for the shared text composer.
/// Its sample namespace is never exposed as a persisted ReadingUnit identity.
fn sample(purpose: CatalogPurpose) -> ReadingUnit {
    let tokens = if purpose == CatalogPurpose::ContextWord {
        let byte_start = SAMPLE_TEXT.find('空').expect("registered sample selection") as u32;
        let byte_end = byte_start + '空'.len_utf8() as u32;
        let (utf16_start, utf16_end) =
            utf16_span(SAMPLE_TEXT, byte_start, byte_end).expect("registered sample UTF16");
        vec![LexicalToken {
            surface: "空".into(),
            lemma: "空".into(),
            reading: String::new(),
            part_of_speech: String::new(),
            byte_start,
            byte_end,
            utf16_start,
            utf16_end,
            known: false,
        }]
    } else {
        vec![]
    };
    ReadingUnit {
        source_summary: None,
        unit_id: SAMPLE_ID.into(),
        session_id: SAMPLE_ID.into(),
        game_id: None,
        text_revision: 1,
        source_text: SAMPLE_TEXT.into(),
        source_hash: content_hash(SAMPLE_TEXT),
        source_language: "ja".into(),
        created_at_ms: 0.,
        translations: vec![],
        tokens,
    }
}
/// Compile only the requested saved service, regardless of selection/enabled
/// intent. Runtime still rejects explicit suspension, stale sources, missing
/// capability/resource authority and revoked credentials before any side effect.
pub fn compose_saved_service_test_plan(
    input: SavedServiceTestPlanInput<'_>,
) -> AppResult<SavedServiceTestPlan> {
    let request = input.request;
    let graph = crate::domain::model_config::service_draft::materialize_base(input.document)?;
    let (source_revision, target) = match &request.source {
        SavedServiceRef::Http { source } => (
            &source.revision_id,
            EngineTarget::Http {
                model_id: source.model_id.clone(),
            },
        ),
        SavedServiceRef::Offline { source } => (
            &source.revision_id,
            EngineTarget::Offline {
                offline_config_id: source.config_id.clone(),
            },
        ),
    };
    if source_revision != &graph.revision_id {
        return Err(error(
            "service_revision_mismatch",
            "测试来源与实际保存版本不一致",
        ));
    }
    require_id(source_revision)?;
    let output_mode = request.output_mode.unwrap_or_default();
    if request.purpose == CatalogPurpose::Translation
        && output_mode == ServiceTestOutputMode::Structured
    {
        return Err(error(
            "structured_output_not_applicable",
            "翻译测试只支持文本输出，不能授予结构化学习能力",
        ));
    }
    service_config_preflight(&graph, &target, request.purpose, input.catalog)?;
    let fixed;
    let (unit, selected_token, history, identity) =
        match (request.input_scope.as_ref(), input.reading) {
            (None | Some(ServiceTestInputScope::Sample {}), TestReadingInput::Sample) => {
                fixed = sample(request.purpose);
                let token = fixed.tokens.first();
                (
                    &fixed,
                    token,
                    &[][..],
                    SavedServiceTestInputIdentity::Sample {
                        sample_id: SAMPLE_ID.into(),
                        source_hash: fixed.source_hash.clone(),
                    },
                )
            }
            (
                Some(ServiceTestInputScope::SelectedUnit {
                    unit_id,
                    text_revision,
                    source_hash,
                }),
                TestReadingInput::Selected {
                    unit,
                    selected_token,
                    history,
                },
            ) => {
                if unit.unit_id != *unit_id
                    || unit.text_revision != text_revision.get()
                    || unit.source_hash != *source_hash
                {
                    return Err(error(
                        "test_reading_identity_mismatch",
                        "已选择原文的版本或指纹已变化",
                    ));
                }
                if selected_token
                    .is_some_and(|token| !unit.tokens.iter().any(|actual| actual == token))
                {
                    return Err(error(
                        "selected_token_mismatch",
                        "测试词语不属于实际已核分词对象",
                    ));
                }
                for old in history {
                    validate_reading_input(old, None, &old.source_language, "zh-Hans")?;
                }
                (
                    unit,
                    selected_token,
                    history,
                    SavedServiceTestInputIdentity::SelectedUnit {
                        unit_id: unit.unit_id.clone(),
                        text_revision: unit.text_revision,
                        source_hash: unit.source_hash.clone(),
                        session_id: unit.session_id.clone(),
                        game_id: unit.game_id.clone(),
                    },
                )
            }
            _ => {
                return Err(error(
                    "test_input_scope_mismatch",
                    "测试范围与实际固定样本或已选原文不一致",
                ));
            }
        };
    if request.purpose == CatalogPurpose::ContextWord && selected_token.is_none() {
        return Err(error(
            "selected_token_required",
            "语境查词测试需要实际固定的词语",
        ));
    }
    let target_language = match &target {
        EngineTarget::Offline { offline_config_id } => graph
            .offline_engine_configs
            .iter()
            .find(|config| config.id == *offline_config_id)
            .ok_or_else(|| error("missing_offline_configuration", "离线配置不存在"))?
            .pair
            .target
            .as_str(),
        EngineTarget::Http { .. } => "zh-Hans",
    };
    validate_reading_input(unit, selected_token, &unit.source_language, target_language)?;
    let mut provenance = BTreeMap::new();
    let sample_input = matches!(identity, SavedServiceTestInputIdentity::Sample { .. });
    for field in [
        "sourceText",
        "sourceHash",
        "unitId",
        "textRevision",
        "sessionId",
        "gameId",
    ] {
        provenance.insert(
            field.into(),
            if sample_input {
                source(PlanSourceKind::Builtin, None, Some(SAMPLE_ID), field)
            } else {
                source(
                    PlanSourceKind::ReadingUnit,
                    None,
                    Some(&unit.unit_id),
                    field,
                )
            },
        );
    }
    provenance.insert(
        "purpose".into(),
        source(
            PlanSourceKind::TestRequest,
            None,
            None,
            "savedServiceTest.purpose",
        ),
    );
    let mut compiled = compile_target(
        RequestContext {
            graph: &graph,
            unit,
            selected_token,
            history,
            source_language: &unit.source_language,
            target_language,
            catalog: input.catalog,
        },
        &target,
        RequestScope::ServiceOnly {
            purpose: request.purpose,
            output_mode,
            output_mode_explicit: request.output_mode.is_some(),
        },
        provenance,
    )?;
    if let ServiceExecutionTarget::Http { plan } = &compiled.target
        && sample_input && plan.request.source_language != unit.source_language {
            return Err(error(
                "test_sample_language_mismatch",
                "固定日语样本不匹配此服务的明确源语言",
            ));
        }
    // A test's resolved fallback target language is a product default, never an
    // assertion that another active purpose or a saved preference was consulted.
    for (field, builtin) in [
        ("sourceLanguage", "savedServiceTest.inputLanguage"),
        ("targetLanguage", "defaultTargetLanguageV1"),
    ] {
        if let Some(current) = compiled.provenance.get_mut(field)
            && current.kind == PlanSourceKind::ReadingPreference
        {
            *current = if field == "sourceLanguage" && !sample_input {
                source(
                    PlanSourceKind::ReadingUnit,
                    None,
                    Some(&unit.unit_id),
                    "sourceLanguage",
                )
            } else {
                source(PlanSourceKind::Builtin, None, None, builtin)
            };
        }
    }
    if sample_input && compiled.provenance.contains_key("selectedToken") {
        compiled.provenance.insert(
            "selectedToken".into(),
            source(
                PlanSourceKind::Builtin,
                None,
                Some(SAMPLE_ID),
                "fixedSample.selection",
            ),
        );
    }
    Ok(SavedServiceTestPlan {
        identity: SavedServiceTestIdentity {
            source: request.source.clone(),
            purpose: request.purpose,
            output_mode,
            input: identity,
        },
        selected_token: selected_token.cloned(),
        policy: compiled.policy,
        provenance: compiled.provenance,
        context_units: compiled.context_units,
        term_sources: compiled.term_sources,
        target: compiled.target,
    })
}

use crate::{
    domain::{
        canonical_hash,
        model_config::credentials::{
            CredentialMaterialKind, CredentialStorageMetadata, PrincipalState,
        },
        service_testing::{
            ServiceTestAuthorityCoverage, ServiceTestEvidenceDomain, ServiceTestRequestSemantics,
            ServiceTestSemanticFingerprint,
        },
    },
    ports::{
        google_authorization::{
            GoogleClientMode, GoogleGrantState, GoogleNetworkPolicy, google_scope_hash,
        },
        service_testing::{PreparedTestAuthentication, PreparedTestReading},
    },
};
const SOURCE_SEMANTICS: &str = "saved-service-test-source-primary-v1";
const REQUEST_SEMANTICS: &str = "saved-service-test-request-primary-v1";

pub fn saved_service_test_evidence_domain(
    reading: &PreparedTestReading,
) -> AppResult<ServiceTestEvidenceDomain> {
    match reading {
        PreparedTestReading::Sample => Ok(ServiceTestEvidenceDomain::SampleJapaneseV1),
        PreparedTestReading::Selected { unit, .. } => {
            require_id(&unit.source_language)?;
            if let Some(game) = &unit.game_id {
                require_id(game)?;
            }
            Ok(ServiceTestEvidenceDomain::Selected {
                game_id: unit.game_id.clone(),
                source_language: unit.source_language.clone(),
            })
        }
    }
}
fn evidence_scope(domain: &ServiceTestEvidenceDomain) -> AppResult<(String, &str)> {
    match domain {
        ServiceTestEvidenceDomain::SampleJapaneseV1 => Ok(("global".into(), "ja")),
        ServiceTestEvidenceDomain::Selected {
            game_id,
            source_language,
        } => {
            require_id(source_language)?;
            let scope = match game_id {
                Some(game) => {
                    require_id(game)?;
                    format!("game:{game}")
                }
                None => "global".into(),
            };
            Ok((scope, source_language))
        }
    }
}
/// Explicit transport behavior only. A label, active intent, provenance alias or
/// overall configuration revision is not a request behavior.
fn test_connection_semantics(connection: &ConnectionV3) -> Value {
    json!({"protocol":connection.protocol,"deployment":connection.deployment,"endpoint":connection.endpoint,
        "authMode":connection.auth_mode,"credentialPoolId":connection.credential_pool_id,"credentialPoolRevision":connection.credential_pool_revision,
        "googleAuthorization":connection.google_authorization,"proxyMode":connection.proxy_mode,"networkMetadata":connection.network_metadata,
        "adapterBinding":connection.adapter_binding,"httpOptions":connection.http_options,"origin":connection.origin,"presetSource":connection.preset_source})
}
fn purpose_capabilities(
    model: &ModelV3,
    purpose: CatalogPurpose,
    mode: ServiceTestOutputMode,
) -> BTreeMap<String, Value> {
    let mut keys = vec!["streaming", purpose_name(purpose)];
    if mode == ServiceTestOutputMode::Structured {
        keys.push("structured");
    }
    keys.into_iter()
        .filter_map(|key| {
            model
                .capabilities
                .get(key)
                .map(|value| (key.into(), value.clone()))
        })
        .collect()
}
/// Canonical source behavior of this primary-only test. This does not inspect a
/// source seal or authorize a credential: the Repository must load real facts.
pub fn saved_service_test_source_semantics(
    document: &ConfigurationDocument,
    reference: &SavedServiceRef,
    purpose: CatalogPurpose,
    output_mode: ServiceTestOutputMode,
    domain: &ServiceTestEvidenceDomain,
    catalog: &ServiceCatalog,
) -> AppResult<ServiceTestSemanticFingerprint> {
    let graph = crate::domain::model_config::service_draft::materialize_base(document)?;
    let (revision, target) = match reference {
        SavedServiceRef::Http { source } => (
            &source.revision_id,
            EngineTarget::Http {
                model_id: source.model_id.clone(),
            },
        ),
        SavedServiceRef::Offline { source } => (
            &source.revision_id,
            EngineTarget::Offline {
                offline_config_id: source.config_id.clone(),
            },
        ),
    };
    if revision != &graph.revision_id {
        return Err(error(
            "service_revision_mismatch",
            "测试来源与实际保存版本不一致",
        ));
    }
    if purpose == CatalogPurpose::Translation && output_mode == ServiceTestOutputMode::Structured {
        return Err(error(
            "structured_output_not_applicable",
            "翻译测试不授结构化解释能力",
        ));
    }
    service_config_preflight(&graph, &target, purpose, catalog)?;
    let (term_scope, input_source) = evidence_scope(domain)?;
    let scope = RequestScope::ServiceOnly {
        purpose,
        output_mode,
        output_mode_explicit: true,
    };
    let terms = semantic_scope_terms(&graph, &term_scope);
    let projection = match &target {
        EngineTarget::Http { model_id } => {
            let model = graph
                .models
                .iter()
                .find(|model| &model.model_id == model_id)
                .ok_or_else(|| error("missing_model_configuration", "模型配置不存在"))?;
            let connection = graph
                .connections
                .iter()
                .find(|connection| connection.connection_id == model.connection_id)
                .ok_or_else(|| error("missing_connection", "连接不存在"))?;
            let defaults = model
                .request_defaults
                .get(&purpose)
                .ok_or_else(|| error("missing_request_defaults", "该用途缺少服务默认值"))?;
            let mut language_sources = BTreeMap::new();
            let (source_language, target_language) = resolve_http_languages(
                &graph,
                &scope,
                defaults,
                &model.model_id,
                input_source,
                "zh-Hans",
                &mut language_sources,
            )?;
            if matches!(domain, ServiceTestEvidenceDomain::SampleJapaneseV1)
                && source_language != "ja"
            {
                return Err(error(
                    "test_sample_language_mismatch",
                    "固定日语样本不匹配此服务的明确源语言",
                ));
            }
            let (template, _) = scope.template(&graph, model)?;
            if let Some(template) = &template {
                known_direction(template, Some(&source_language), Some(&target_language))?;
            }
            let builtin_system = if template
                .as_ref()
                .and_then(|template| template.body.get("system"))
                .and_then(Value::as_str)
                .is_none()
            {
                Some(purpose_system_prompt(
                    purpose_name(purpose),
                    &target_language,
                )?)
            } else {
                None
            };
            let mut parameter_sources = BTreeMap::new();
            let parameters = scope.parameters(&graph, model, &mut parameter_sources)?;
            let protocol = catalog
                .protocols
                .iter()
                .find(|descriptor| {
                    connection
                        .adapter_binding
                        .as_ref()
                        .is_some_and(|binding| binding.protocol == descriptor.key)
                })
                .ok_or_else(|| error("unknown_protocol_version", "协议版本未注册"))?;
            let capability = protocol
                .capabilities
                .iter()
                .find(|capability| capability.purpose == purpose)
                .map(|capability| &capability.support);
            json!({"kind":"http","connection":test_connection_semantics(connection),"actualModelId":model.actual_model_id,"templatePurpose":model.template_purpose,
                "capabilities":purpose_capabilities(model,purpose,output_mode),"protocolPurposeCapability":capability,
                "languages":{"defaults":defaults.languages,"source":source_language,"target":target_language},
                "policy":scope.policy(defaults),"parameters":parameters,"parameterSources":semantics_sources(parameter_sources),
                "template":template.as_ref().map(semantic_template),"builtinSystem":builtin_system,"safetySuffix":request_system_safety_suffix(),
                "structuredSuffix":if output_mode==ServiceTestOutputMode::Structured {Some(request_system_structured_suffix())} else {None}})
        }
        EngineTarget::Offline { offline_config_id } => {
            let config = graph
                .offline_engine_configs
                .iter()
                .find(|config| &config.id == offline_config_id)
                .ok_or_else(|| error("missing_offline_configuration", "离线配置不存在"))?;
            if input_source != config.pair.source {
                return Err(error(
                    "offline_language_pair_mismatch",
                    "离线配置与实际输入方向不一致",
                ));
            }
            json!({"kind":"offline","engine":config.engine,"backend":config.backend,"pair":config.pair,"routePolicy":config.route_policy,
                "installation":config.installation,"resolution":config.resolution,"policy":scope.policy(&config.request_defaults)})
        }
    };
    Ok(ServiceTestSemanticFingerprint {
        version: SOURCE_SEMANTICS,
        sha256: canonical_hash(&(
            SOURCE_SEMANTICS,
            purpose,
            output_mode,
            domain,
            projection,
            terms,
        ))?,
        purpose,
        output_mode,
        evidence_domain: domain.clone(),
    })
}
fn checked_hash(hash: &str) -> AppResult<()> {
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(error("invalid_test_fingerprint", "测试指纹无效"));
    }
    Ok(())
}
fn target_material(plan: &SavedServiceTestPlan) -> (&str, &[Value], &[Value]) {
    match &plan.target {
        ServiceExecutionTarget::Http { plan } => (
            &plan.request.source_text,
            &plan.reference_material.context,
            &plan.reference_material.glossary,
        ),
        ServiceExecutionTarget::Offline { payload, .. } => {
            (&payload.source_text, &payload.context, &payload.glossary)
        }
    }
}
fn frozen_input_projection(
    plan: &SavedServiceTestPlan,
    reading: &PreparedTestReading,
) -> AppResult<Value> {
    let fixed;
    let (unit, token, lexical, history, scope) = match reading {
        PreparedTestReading::Sample => {
            fixed = sample(plan.identity.purpose);
            if !matches!(&plan.identity.input,SavedServiceTestInputIdentity::Sample {sample_id,source_hash} if sample_id==SAMPLE_ID && source_hash==&fixed.source_hash)
            {
                return Err(error(
                    "test_reading_identity_mismatch",
                    "固定样本与计划不一致",
                ));
            }
            (
                &fixed,
                fixed.tokens.first(),
                None,
                &[][..],
                json!({"kind":"sample","sampleId":SAMPLE_ID,"sourceHash":fixed.source_hash}),
            )
        }
        PreparedTestReading::Selected {
            unit,
            selected_token,
            lexical_snapshot_id,
            history,
        } => {
            let actual = SavedServiceTestInputIdentity::SelectedUnit {
                unit_id: unit.unit_id.clone(),
                text_revision: unit.text_revision,
                source_hash: unit.source_hash.clone(),
                session_id: unit.session_id.clone(),
                game_id: unit.game_id.clone(),
            };
            if plan.identity.input != actual {
                return Err(error(
                    "test_reading_identity_mismatch",
                    "固定原文与计划不一致",
                ));
            }
            if let Some(lexical) = lexical_snapshot_id {
                require_id(lexical)?;
            }
            if selected_token.is_some() && lexical_snapshot_id.is_none() {
                return Err(error(
                    "test_lexical_proof_missing",
                    "真实选词缺少已核词法快照",
                ));
            }
            if selected_token
                .as_ref()
                .is_some_and(|token| !unit.tokens.iter().any(|actual| actual == token))
            {
                return Err(error("selected_token_mismatch", "选词不属于真实词法对象"));
            }
            (
                unit.as_ref(),
                selected_token.as_ref(),
                if selected_token.is_some() {
                    lexical_snapshot_id.as_deref()
                } else {
                    None
                },
                history.as_slice(),
                json!({"kind":"selected_unit","unitId":unit.unit_id,"textRevision":unit.text_revision,"sourceHash":unit.source_hash,"sessionId":unit.session_id,"gameId":unit.game_id,"sourceLanguage":unit.source_language,"createdAt":unit.created_at_ms}),
            )
        }
    };
    if plan.selected_token.as_ref() != token {
        return Err(error("selected_token_mismatch", "固定选择与测试计划不一致"));
    }
    let (source_text, context, _) = target_material(plan);
    if source_text != unit.source_text {
        return Err(error(
            "test_reading_identity_mismatch",
            "计划正文与固定输入不一致",
        ));
    }
    validate_reading_input(unit, token, &unit.source_language, "zh-Hans")?;
    let material = compose_reference_material(ReferenceMaterialInput {
        unit,
        context_enabled: plan.policy.context.enabled,
        context_count: plan.policy.context.max_sentences as usize,
        context_mode: mode_name(&plan.policy.context.mode),
        term_sets: &[],
        history,
    })?;
    if material.context_units != plan.context_units || material.context != context {
        return Err(error(
            "test_context_proof_mismatch",
            "已同意上下文与实际固定历史不一致",
        ));
    }
    let mut references = Vec::new();
    for (id, revision) in &plan.context_units {
        let candidates: Vec<_> = history
            .iter()
            .filter(|unit| unit.unit_id == *id && unit.text_revision == *revision)
            .collect();
        if candidates.len() != 1 {
            return Err(error(
                "test_context_proof_mismatch",
                "上下文身份不唯一或缺失",
            ));
        }
        let old = candidates[0];
        validate_reading_input(old, None, &old.source_language, "zh-Hans")?;
        let translations = if plan.policy.context.mode == HistoryInputMode::Original {
            None
        } else {
            Some(old.translations.iter().map(|translation|json!({"translationId":translation.translation_id,"taskId":translation.task_id,"targetLanguage":translation.target_language,"textHash":content_hash(&translation.text),"correction":translation.correction.as_ref().map(|correction|json!({"revision":correction.revision,"textHash":content_hash(&correction.text)}))})).collect::<Vec<_>>())
        };
        references.push(json!({"unitId":old.unit_id,"textRevision":old.text_revision,"sourceHash":old.source_hash,"sessionId":old.session_id,"gameId":old.game_id,"translations":translations}));
    }
    Ok(
        json!({"scope":scope,"selectedToken":token,"lexicalSnapshotId":lexical,"contextReferences":references,"termReferences":plan.term_sources}),
    )
}
fn storage_projection(storage: &CredentialStorageMetadata) -> Value {
    match storage {
        CredentialStorageMetadata::LegacyRawApiKey => json!({"kind":"api_key","schema":null}),
        CredentialStorageMetadata::EnvelopeV1 { kind } => json!({"kind":kind.key(),"schema":1}),
    }
}
fn auth_projection(
    plan: &SavedServiceTestPlan,
    auth: &PreparedTestAuthentication,
) -> AppResult<(Value, ServiceTestAuthorityCoverage)> {
    let ServiceExecutionTarget::Http { plan: http } = &plan.target else {
        if !matches!(auth, PreparedTestAuthentication::None) {
            return Err(error(
                "test_authentication_target_mismatch",
                "离线请求不接受HTTP凭据",
            ));
        }
        let ServiceExecutionTarget::Offline { config, .. } = &plan.target else {
            unreachable!()
        };
        return Ok((
            json!({"kind":"offline_unresolved","engine":config.engine,"installation":config.installation,"resolution":config.resolution}),
            ServiceTestAuthorityCoverage::Unresolved {
                code: "offline_test_authority_unresolved".into(),
            },
        ));
    };
    let connection = &http.connection;
    let projection = match auth {
        PreparedTestAuthentication::None if connection.auth_mode == "none" => {
            json!({"kind":"none"})
        }
        PreparedTestAuthentication::PoolMember(credential) => {
            let expected = match connection.auth_mode.as_str() {
                "api_key" => CredentialMaterialKind::ApiKey,
                "baidu_app_secret" => CredentialMaterialKind::BaiduAppSecret,
                "youdao_app_secret" => CredentialMaterialKind::YoudaoAppSecret,
                "google_service_account" => CredentialMaterialKind::GoogleServiceAccount,
                _ => {
                    return Err(error(
                        "test_authentication_target_mismatch",
                        "凭据分支与实际目标不一致",
                    ));
                }
            };
            let selection = &credential.selection;
            let actual_kind = match credential.storage {
                CredentialStorageMetadata::LegacyRawApiKey => CredentialMaterialKind::ApiKey,
                CredentialStorageMetadata::EnvelopeV1 { kind } => kind,
            };
            if actual_kind != expected
                || connection.credential_pool_id.as_deref() != Some(&selection.pool_id)
                || connection
                    .credential_pool_revision
                    .map(|revision| revision.get())
                    != Some(selection.pool_revision)
                || selection.pool_revision == 0
                || credential.lifecycle_revision == 0
            {
                return Err(error(
                    "test_authentication_target_mismatch",
                    "实际凭据种类、池或生命周期不一致",
                ));
            }
            for id in [
                &selection.credential_id,
                &selection.pool_id,
                &selection.member_id,
                &selection.vault_ref,
            ] {
                require_id(id)?;
            }
            checked_hash(&credential.pool_policy_sha256)?;
            json!({"kind":"pool_member","poolId":selection.pool_id,"poolRevision":selection.pool_revision,"poolPolicyHash":credential.pool_policy_sha256,"memberId":selection.member_id,"credentialId":selection.credential_id,"privateVersionRefHash":content_hash(&selection.vault_ref),"storage":storage_projection(&credential.storage),"lifecycleRevision":credential.lifecycle_revision,"scopesHash":if expected==CredentialMaterialKind::GoogleServiceAccount {Some(google_scope_hash())} else {None}})
        }
        PreparedTestAuthentication::GoogleOwnedGrant(grant) => {
            grant.validate()?;
            let epoch = grant
                .cursor
                .authorization_epoch
                .ok_or_else(|| error("google_grant_not_active", "授权尚未激活"))?;
            if connection.auth_mode != "google_oauth"
                || grant.state != GoogleGrantState::Active
                || connection
                    .google_authorization
                    .as_ref()
                    .is_none_or(|reference| {
                        reference.grant_id != grant.cursor.grant_id
                            || reference.authorization_epoch != epoch
                    })
            {
                return Err(error(
                    "test_authentication_target_mismatch",
                    "当前Google授权不属于实际测试目标",
                ));
            }
            let network = GoogleNetworkPolicy::from_connection(
                &connection.proxy_mode,
                &connection.network_metadata,
            )?;
            let network = match network {
                GoogleNetworkPolicy::Inherit => json!({"mode":"inherit"}),
                GoogleNetworkPolicy::Disabled => json!({"mode":"disabled"}),
                GoogleNetworkPolicy::Override { proxy_url } => {
                    json!({"mode":"override","proxyUrlHash":content_hash(proxy_url)})
                }
            };
            let principal = match &grant.principal_state {
                PrincipalState::Unknown => json!({"state":"unknown"}),
                PrincipalState::Verified { subject } => {
                    json!({"state":"verified","subject":subject})
                }
            };
            json!({"kind":"google_oauth","grantId":grant.cursor.grant_id,"authorizationEpoch":epoch,"client":{"profileId":grant.client.key.client_profile_id,"profileRevision":grant.client.key.client_profile_revision,"publicClientId":grant.client.public_client_id,"mode":match grant.client.client_mode {GoogleClientMode::Application=>"application",GoogleClientMode::Custom=>"custom"},"policyHash":grant.client.policy_sha256},"scopesHash":grant.scopes_sha256,"principal":principal,"target":{"endpoint":connection.endpoint,"adapterBinding":connection.adapter_binding,"httpOptions":connection.http_options,"network":network}})
        }
        _ => {
            return Err(error(
                "test_authentication_target_mismatch",
                "认证覆盖与测试目标不一致",
            ));
        }
    };
    Ok((projection, ServiceTestAuthorityCoverage::Projected))
}
/// Input/authority projections are derived from the actual prepared facts. The
/// Repository still recomputes the source and validates dynamic authority; no
/// supplied fingerprint is accepted as an authorization capability.
pub fn saved_service_test_request_semantics(
    plan: &SavedServiceTestPlan,
    source: &ServiceTestSemanticFingerprint,
    reading: &PreparedTestReading,
    auth: &PreparedTestAuthentication,
) -> AppResult<ServiceTestRequestSemantics> {
    if source.version != SOURCE_SEMANTICS {
        return Err(error(
            "test_semantics_version_mismatch",
            "测试语义版本不一致",
        ));
    }
    checked_hash(&source.sha256)?;
    if source.purpose != plan.identity.purpose
        || source.output_mode != plan.identity.output_mode
        || source.evidence_domain != saved_service_test_evidence_domain(reading)?
    {
        return Err(error(
            "test_source_semantics_scope_mismatch",
            "来源指纹不属于本次用途、模式或固定输入范围",
        ));
    }
    let input = frozen_input_projection(plan, reading)?;
    let input_sha256 = canonical_hash(&("saved-service-test-input-v1", input))?;
    let (authority, authority_coverage) = auth_projection(plan, auth)?;
    let authority_sha256 = canonical_hash(&("saved-service-test-authority-v1", authority))?;
    let request = match &plan.target {
        ServiceExecutionTarget::Http { plan } => {
            json!({"kind":"http","request":plan.request,"referenceMaterial":plan.reference_material})
        }
        ServiceExecutionTarget::Offline { payload, .. } => {
            json!({"kind":"offline","payload":payload})
        }
    };
    let sha256 = canonical_hash(&(
        REQUEST_SEMANTICS,
        &source.sha256,
        plan.identity.purpose,
        plan.identity.output_mode,
        request,
        &input_sha256,
        &authority_sha256,
    ))?;
    Ok(ServiceTestRequestSemantics {
        version: REQUEST_SEMANTICS,
        sha256,
        input_sha256,
        authority_sha256,
        authority_coverage,
    })
}
