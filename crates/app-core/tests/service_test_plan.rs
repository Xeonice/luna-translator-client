//! Core-only test contracts. Synthetic descriptors are not provider/resource
//! authorization; no test result here claims a real service or SQL record.
use app_core::{
    application::{request_plan::RequestPlan, service_request_plan::*},
    contracts::*,
    domain::{
        content_hash, model_config::service_graph::*, service_execution::*, service_testing::*,
    },
};
use serde_json::json;
use std::num::NonZeroU32;
fn decision() -> CatalogDecision {
    CatalogDecision {
        state: CapabilityState::Unknown,
        reason: Some(CatalogReason::AdapterNotImplemented),
    }
}
fn defaults(purpose: CatalogPurpose) -> ServiceRequestDefaults {
    ServiceRequestDefaults {
        purpose,
        languages: ServiceLanguageDefaults::Http {
            source: SourceLanguageChoice::InheritSource {},
            target: TargetLanguageChoice::InheritPreference {},
        },
        context: ServiceContextConsent {
            enabled: true,
            max_sentences: 2,
            mode: HistoryInputMode::Original,
        },
        execution: ServiceExecutionDefaults {
            timeout_ms: 12345,
            concurrency: 2,
            retries: 1,
        },
        cache: ServiceCachePolicy { enabled: true },
        fallback: vec![],
    }
}
fn fixture(
    options: HttpServiceOptionsV1,
    purpose: CatalogPurpose,
) -> (ConfigurationV3, ServiceCatalog) {
    let protocol = options.protocol_id();
    let traditional = matches!(
        protocol,
        "azure_translator"
            | "deepl_v2"
            | "google_translate_v2"
            | "google_translate_v3"
            | "baidu_vip"
            | "youdao_v3"
            | "tencent_tokenhub_translation"
    );
    let (mode, auth) = match protocol {
        "anthropic" => ("api_key", CatalogAuthKind::AnthropicApiKey),
        "gemini_generate_content" | "google_translate_v2" => {
            ("api_key", CatalogAuthKind::GoogleApiKey)
        }
        "azure_translator" => ("api_key", CatalogAuthKind::AzureSubscriptionKey),
        "deepl_v2" => ("api_key", CatalogAuthKind::DeeplAuthKey),
        "google_translate_v3" => (
            "google_service_account",
            CatalogAuthKind::GoogleServiceAccount,
        ),
        "baidu_vip" => ("baidu_app_secret", CatalogAuthKind::BaiduAppSecret),
        "youdao_v3" => ("youdao_app_secret", CatalogAuthKind::YoudaoAppSecret),
        "tencent_tokenhub_translation" => ("api_key", CatalogAuthKind::BearerApiKey),
        _ => ("none", CatalogAuthKind::None),
    };
    let model_identity = if matches!(protocol, "google_translate_v3") {
        CatalogModelIdentity::Optional
    } else if traditional && protocol != "tencent_tokenhub_translation" {
        CatalogModelIdentity::NotApplicable
    } else {
        CatalogModelIdentity::Required
    };
    let actual_model = if model_identity == CatalogModelIdentity::Required {
        Some("synthetic-model")
    } else {
        None
    };
    let binding = AdapterBinding {
        protocol: ProtocolKey {
            protocol_id: protocol.into(),
            contract_revision: 1,
        },
        profile: CompatibilityProfileKey {
            profile_id: "controlled-profile".into(),
            profile_revision: 1,
        },
    };
    let mut graph:ConfigurationV3=serde_json::from_value(json!({"revisionId":"saved-source","codecVersion":3,
        "connections":[{"connectionId":"c","displayName":"Saved resource","protocol":protocol,"deployment":"local","authMode":mode,"endpoint":"http://127.0.0.1:1/v1","credentialPoolId":if mode=="none" {None} else {Some("synthetic-pool")},"credentialPoolRevision":if mode=="none" {None} else {Some(1)},"proxyMode":"disabled","networkMetadata":{},"availabilityIntent":false,"adapterBinding":binding,"httpOptions":options,"origin":{"kind":"custom"}}],
        "models":[{"modelId":"m","connectionId":"c","actualModelId":actual_model,"displayName":"Saved model","selectionSource":"manual","capabilities":{"streaming":false},"parameters":{},"defaultTemplates":{},"templatePurpose":purpose,"requestDefaults":{match purpose {CatalogPurpose::Translation=>"translation",CatalogPurpose::Grammar=>"grammar",CatalogPurpose::ContextWord=>"context_word"}:defaults(purpose)}}],
        "bindings":[],"templates":[],"termSets":[],"offlineEngineConfigs":[]})).unwrap();
    if protocol == "google_translate_v3" {
        graph.connections[0].endpoint = Some("https://translate.googleapis.com/v3".into());
    }
    let purposes = if traditional {
        vec![CatalogPurpose::Translation]
    } else {
        vec![
            CatalogPurpose::Translation,
            CatalogPurpose::Grammar,
            CatalogPurpose::ContextWord,
        ]
    };
    let catalog = ServiceCatalog {
        brands: vec![],
        schema_version: 1,
        protocols: vec![ProtocolDescriptor {
            key: binding.protocol.clone(),
            label: "controlled".into(),
            family: if traditional {
                CatalogFamily::Translation
            } else {
                CatalogFamily::Llm
            },
            auth_kinds: vec![auth],
            model_identity,
            wire_formats: vec![CatalogWireFormat::Json],
            allowed_purposes: purposes.clone(),
            capabilities: vec![],
            implementation: decision(),
            source_refs: vec![],
        }],
        profiles: vec![CompatibilityProfileDescriptor {
            key: binding.profile,
            protocol: binding.protocol,
            allowed_purposes: purposes,
            auth_kinds: vec![auth],
            option_field_ids: vec![],
            source_refs: vec![],
        }],
        presets: vec![],
        offline_engines: vec![],
        sources: vec![],
    };
    (graph, catalog)
}
fn request(graph: &ConfigurationV3, purpose: CatalogPurpose) -> SavedServiceTestRequest {
    SavedServiceTestRequest {
        expected_profile_generation: NonZeroU32::new(1).unwrap(),
        source: SavedServiceRef::Http {
            source: HttpConfigRef {
                revision_id: graph.revision_id.clone(),
                model_id: "m".into(),
            },
        },
        purpose,
        input_scope: None,
        output_mode: None,
    }
}
fn compile(
    graph: &ConfigurationV3,
    req: &SavedServiceTestRequest,
    reading: TestReadingInput<'_>,
    catalog: &ServiceCatalog,
) -> AppResult<SavedServiceTestPlan> {
    compose_saved_service_test_plan(SavedServiceTestPlanInput {
        document: &ConfigurationDocument::ServiceGraph {
            configuration: graph.clone(),
        },
        request: req,
        reading,
        catalog,
    })
}
fn http(plan: &SavedServiceTestPlan) -> &FrozenHttpServicePlan<RequestPlan> {
    match &plan.target {
        ServiceExecutionTarget::Http { plan } => plan,
        _ => panic!("HTTP target expected"),
    }
}
fn unit(id: &str, text: &str, time: f64) -> ReadingUnit {
    ReadingUnit {
        source_summary: None,
        unit_id: id.into(),
        session_id: "actual-session".into(),
        game_id: Some("actual-game".into()),
        text_revision: 3,
        source_text: text.into(),
        source_hash: content_hash(text),
        source_language: "ja".into(),
        created_at_ms: time,
        translations: vec![],
        tokens: vec![],
    }
}
fn selected(unit: &ReadingUnit) -> ServiceTestInputScope {
    ServiceTestInputScope::SelectedUnit {
        unit_id: unit.unit_id.clone(),
        text_revision: NonZeroU32::new(unit.text_revision).unwrap(),
        source_hash: unit.source_hash.clone(),
    }
}
fn options() -> Vec<HttpServiceOptionsV1> {
    vec![
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        HttpServiceOptionsV1::OpenaiResponses {
            organization_id: None,
            project_id: None,
        },
        HttpServiceOptionsV1::Anthropic { workspace_id: None },
        HttpServiceOptionsV1::GeminiGenerateContent {},
        HttpServiceOptionsV1::OllamaNative {},
        HttpServiceOptionsV1::LmstudioNativeV1 {},
        HttpServiceOptionsV1::AzureTranslator {
            resource_type: Some(AzureResourceType::Global),
            region: None,
        },
        HttpServiceOptionsV1::DeeplV2 {
            plan: Some(DeeplPlan::Free),
        },
        HttpServiceOptionsV1::GoogleTranslateV2 {},
        HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id: "p".into(),
            location: "global".into(),
        },
        HttpServiceOptionsV1::BaiduVip {},
        HttpServiceOptionsV1::YoudaoV3 {},
        HttpServiceOptionsV1::TencentTokenhubTranslation {},
    ]
}
#[test]
fn all_thirteen_protocols_compile_exact_saved_targets_without_enabling_or_fake_slots() {
    for option in options() {
        let (graph, catalog) = fixture(option, CatalogPurpose::Translation);
        let before = serde_json::to_vec(&graph).unwrap();
        let plan = compile(
            &graph,
            &request(&graph, CatalogPurpose::Translation),
            TestReadingInput::Sample,
            &catalog,
        )
        .unwrap();
        assert_eq!(http(&plan).request.protocol, graph.connections[0].protocol);
        assert_eq!(http(&plan).connection, graph.connections[0]);
        assert_eq!(http(&plan).model, graph.models[0]);
        assert!(!http(&plan).connection.availability_intent);
        assert!(graph.bindings.is_empty());
        assert!(
            !plan.provenance.contains_key("bindingId") && !plan.provenance.contains_key("engineId")
        );
        assert!(matches!(
            plan.identity.input,
            SavedServiceTestInputIdentity::Sample { .. }
        ));
        assert!(plan.context_units.is_empty());
        assert_eq!(plan.policy.execution.timeout_ms, 12345);
        assert_eq!(serde_json::to_vec(&graph).unwrap(), before);
    }
}
#[test]
fn output_is_explicit_and_unrelated_active_overrides_do_not_leak_into_testing() {
    let (mut graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Grammar,
    );
    graph.bindings.push(serde_json::from_value(json!({"bindingId":"unrelated","scopeKey":"global","purpose":"grammar","bindingMode":"override","enabled":false,"outputMode":"structured","sourceLanguage":"en","targetLanguage":"de","context":{},"schedule":{},"parameters":{"temperature":{"mode":"set","value":0.9}},"engines":[],"semanticsVersion":"typed_v3","scopeOverrides":PolicyOverrides::default()})).unwrap());
    let req = request(&graph, CatalogPurpose::Grammar);
    let text = compile(&graph, &req, TestReadingInput::Sample, &catalog).unwrap();
    assert!(!http(&text).request.structured);
    assert_eq!(text.provenance["structured"].kind, PlanSourceKind::Builtin);
    assert_eq!(
        text.provenance["structured"].field,
        "savedServiceTest.defaultTextV1"
    );
    assert_eq!(text.provenance["purpose"].kind, PlanSourceKind::TestRequest);
    assert_eq!(http(&text).request.source_language, "ja");
    assert_eq!(http(&text).request.target_language, "zh-Hans");
    assert!(http(&text).request.parameters.is_empty());
    let mut structured = req;
    structured.output_mode = Some(ServiceTestOutputMode::Structured);
    let plan = compile(&graph, &structured, TestReadingInput::Sample, &catalog).unwrap();
    assert!(http(&plan).request.structured);
    assert_eq!(
        plan.provenance["structured"].kind,
        PlanSourceKind::TestRequest
    );
    assert_ne!(text.identity, plan.identity);
    assert_eq!(
        plan.provenance["structured"].field,
        "savedServiceTest.outputMode"
    );
    assert_eq!(
        plan.provenance["targetLanguage"].kind,
        PlanSourceKind::Builtin
    );
    let (graph, catalog) = fixture(
        HttpServiceOptionsV1::DeeplV2 {
            plan: Some(DeeplPlan::Pro),
        },
        CatalogPurpose::Translation,
    );
    let mut req = request(&graph, CatalogPurpose::Translation);
    req.output_mode = Some(ServiceTestOutputMode::Structured);
    assert_eq!(
        compile(&graph, &req, TestReadingInput::Sample, &catalog)
            .unwrap_err()
            .code,
        "structured_output_not_applicable"
    );
}
#[test]
fn exact_selected_input_and_real_token_identity_are_required() {
    let (graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::ContextWord,
    );
    let mut unit = unit("real-unit", "青い空。", 100.);
    let (start, end) = app_core::domain::utf16_span(&unit.source_text, 6, 9).unwrap();
    let token = LexicalToken {
        surface: "空".into(),
        lemma: "空".into(),
        reading: "ソラ".into(),
        part_of_speech: "名詞".into(),
        byte_start: 6,
        byte_end: 9,
        utf16_start: start,
        utf16_end: end,
        known: true,
    };
    unit.tokens.push(token.clone());
    let mut req = request(&graph, CatalogPurpose::ContextWord);
    req.input_scope = Some(selected(&unit));
    let read = || TestReadingInput::Selected {
        unit: &unit,
        selected_token: Some(&token),
        history: &[],
    };
    assert!(compile(&graph, &req, read(), &catalog).is_ok());
    assert_eq!(
        compile(
            &graph,
            &req,
            TestReadingInput::Selected {
                unit: &unit,
                selected_token: None,
                history: &[]
            },
            &catalog
        )
        .unwrap_err()
        .code,
        "selected_token_required"
    );
    let mut fake = token.clone();
    fake.lemma = "別の単語".into();
    assert_eq!(
        compile(
            &graph,
            &req,
            TestReadingInput::Selected {
                unit: &unit,
                selected_token: Some(&fake),
                history: &[]
            },
            &catalog
        )
        .unwrap_err()
        .code,
        "selected_token_mismatch"
    );
    req.input_scope = Some(ServiceTestInputScope::SelectedUnit {
        unit_id: unit.unit_id.clone(),
        text_revision: NonZeroU32::new(2).unwrap(),
        source_hash: unit.source_hash.clone(),
    });
    assert_eq!(
        compile(&graph, &req, read(), &catalog).unwrap_err().code,
        "test_reading_identity_mismatch"
    );
    req.input_scope = None;
    assert_eq!(
        compile(&graph, &req, read(), &catalog).unwrap_err().code,
        "test_input_scope_mismatch"
    );
    let sample = compile(&graph, &req, TestReadingInput::Sample, &catalog).unwrap();
    assert_eq!(
        sample.provenance["selectedToken"].kind,
        PlanSourceKind::Builtin
    );
    assert!(sample.context_units.is_empty());
}
#[test]
fn revision_target_and_purpose_mismatch_fail_without_borrowing_another_binding() {
    let (graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    let mut req = request(&graph, CatalogPurpose::Translation);
    let SavedServiceRef::Http { source } = &mut req.source else {
        unreachable!()
    };
    source.revision_id = "different".into();
    assert_eq!(
        compile(&graph, &req, TestReadingInput::Sample, &catalog)
            .unwrap_err()
            .code,
        "service_revision_mismatch"
    );
    let SavedServiceRef::Http { source } = &mut req.source else {
        unreachable!()
    };
    source.revision_id = graph.revision_id.clone();
    source.model_id = "missing".into();
    assert_eq!(
        compile(&graph, &req, TestReadingInput::Sample, &catalog)
            .unwrap_err()
            .code,
        "missing_model_configuration"
    );
    let SavedServiceRef::Http { source } = &mut req.source else {
        unreachable!()
    };
    source.model_id = "m".into();
    req.purpose = CatalogPurpose::Grammar;
    assert_eq!(
        compile(&graph, &req, TestReadingInput::Sample, &catalog)
            .unwrap_err()
            .code,
        "missing_request_defaults"
    );
}
#[test]
fn selected_consent_uses_real_history_and_terms_but_samples_never_read_game_history() {
    let (mut graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    let current = unit("current", "青い空。", 100.);
    let history = [unit("prior", "雨がやんだ。", 20.)];
    graph.term_sets.push(TermSetV3 {
        term_set_id: "game-terms".into(),
        scope_key: "game:actual-game".into(),
        game_id: Some("actual-game".into()),
        display_name: "terms".into(),
        enabled: true,
        terms: vec![TermV3 {
            term_id: "sky".into(),
            source_text: Some("空".into()),
            target_text: Some("天空".into()),
            source_language: Some("ja".into()),
            target_language: Some("zh-Hans".into()),
            enabled: true,
        }],
    });
    let mut req = request(&graph, CatalogPurpose::Translation);
    req.input_scope = Some(selected(&current));
    let selected = compile(
        &graph,
        &req,
        TestReadingInput::Selected {
            unit: &current,
            selected_token: None,
            history: &history,
        },
        &catalog,
    )
    .unwrap();
    assert_eq!(selected.context_units, vec![("prior".into(), 3)]);
    assert_eq!(
        selected.provenance["sourceLanguage"].kind,
        PlanSourceKind::ReadingUnit
    );
    assert_eq!(
        selected.provenance["sourceLanguage"].entity_id.as_deref(),
        Some("current")
    );
    assert_eq!(
        http(&selected).reference_material.context[0]["source"],
        "雨がやんだ。"
    );
    assert_eq!(
        selected.term_sources,
        vec![("game-terms".into(), "sky".into())]
    );
    req.input_scope = None;
    let sample = compile(&graph, &req, TestReadingInput::Sample, &catalog).unwrap();
    assert!(sample.context_units.is_empty() && sample.term_sources.is_empty());
    assert!(
        http(&sample).reference_material.context.is_empty()
            && http(&sample).reference_material.glossary.is_empty()
    );
    assert_eq!(sample.provenance["gameId"].kind, PlanSourceKind::Builtin);
}

#[test]
fn active_and_test_share_exact_target_template_parameters_and_consented_material() {
    let (mut graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: Some("org".into()),
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    graph.connections[0].availability_intent = true;
    graph.models[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0) },
    );
    graph.templates.push(TemplateV3 {
        template_id: "service-template".into(),
        purpose: CatalogPurpose::Translation,
        display_name: "saved".into(),
        source_kind: "custom".into(),
        source_version: None,
        body: [
            (
                "system".into(),
                json!("Translate {{source}} using {{history}}"),
            ),
            ("user".into(), json!("{{source}}")),
        ]
        .into(),
        variables: vec!["source".into(), "history".into()],
        compatibility: Default::default(),
    });
    graph.models[0]
        .default_templates
        .insert(CatalogPurpose::Translation, "service-template".into());
    graph.bindings.push(serde_json::from_value(json!({"bindingId":"actual-binding","scopeKey":"global","purpose":"translation","bindingMode":"override","enabled":true,"outputMode":"text","context":{},"schedule":{},"parameters":{},"engines":[{"engineId":"actual-engine","target":{"kind":"http","modelId":"m"},"enabled":true,"httpParameters":{},"policyOverrides":PolicyOverrides::default()}],"semanticsVersion":"typed_v3","scopeOverrides":PolicyOverrides::default()})).unwrap());
    let unit = unit("actual-unit", "空が明るくなった。", 100.);
    let history = [crate::unit("actual-earlier", "雨がやんだ。", 20.)];
    let active = compose_service_plan(ServicePlanInput {
        graph: &graph,
        expected_revision_id: &graph.revision_id,
        binding_id: "actual-binding",
        engine_id: "actual-engine",
        unit: &unit,
        selected_token: None,
        history: &history,
        source_language: "ja",
        target_language: "zh-Hans",
        catalog: &catalog,
    })
    .unwrap();
    let mut req = request(&graph, CatalogPurpose::Translation);
    req.input_scope = Some(selected(&unit));
    let test = compile(
        &graph,
        &req,
        TestReadingInput::Selected {
            unit: &unit,
            selected_token: None,
            history: &history,
        },
        &catalog,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&active.target).unwrap(),
        serde_json::to_value(&test.target).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&active.policy).unwrap(),
        serde_json::to_value(&test.policy).unwrap()
    );
    assert_eq!(active.context_units, test.context_units);
    assert_eq!(active.term_sources, test.term_sources);
    assert_eq!(http(&test).request.parameters["temperature"], 0);
    assert!(http(&test).template.is_some());
    let before = http(&test).request.messages.clone();
    graph.templates[0]
        .body
        .insert("system".into(), json!("Changed {{source}}"));
    let changed = compile(
        &graph,
        &req,
        TestReadingInput::Selected {
            unit: &unit,
            selected_token: None,
            history: &history,
        },
        &catalog,
    )
    .unwrap();
    assert_ne!(http(&changed).request.messages, before);
}
#[test]
fn fallback_is_configuration_only_and_tests_never_switch_the_requested_source() {
    let (mut graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    let other = SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "unavailable-fallback-version".into(),
            model_id: "another-resource".into(),
        },
    };
    graph.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .fallback = vec![other.clone()];
    let req = request(&graph, CatalogPurpose::Translation);
    let plan = compile(&graph, &req, TestReadingInput::Sample, &catalog).unwrap();
    assert_eq!(plan.identity.source, req.source);
    assert_ne!(plan.identity.source, other);
    assert_eq!(http(&plan).model.model_id, "m");
    assert_eq!(
        http(&plan).connection.endpoint,
        graph.connections[0].endpoint
    );
    // The inner saved model retains its real defaults. SavedServiceTestPlan has
    // no executable fallback slot list and compilation never resolves the edge.
    assert_eq!(
        http(&plan).model.request_defaults[&CatalogPurpose::Translation].fallback,
        vec![other]
    );
}
#[test]
fn all_four_offline_targets_remain_offline_and_require_exact_input_direction() {
    for (engine, backend) in [
        ("apple_translation", OfflineBackend::PlatformNative),
        ("argos_translate", OfflineBackend::LocalWorker),
        ("jbeijing", OfflineBackend::LocalWorker),
        ("kingsoft_fastait", OfflineBackend::LocalWorker),
    ] {
        let (mut graph, mut catalog) = fixture(
            HttpServiceOptionsV1::OpenaiCompatible {
                organization_id: None,
                project_id: None,
            },
            CatalogPurpose::Translation,
        );
        graph.connections.clear();
        graph.models.clear();
        let mut d = defaults(CatalogPurpose::Translation);
        d.languages = ServiceLanguageDefaults::OfflineUseConfigPair {};
        graph.offline_engine_configs.push(OfflineEngineConfigV1 {
            id: "offline".into(),
            engine: OfflineEngineKey {
                engine_id: engine.into(),
                contract_revision: NonZeroU32::new(1).unwrap(),
            },
            backend: backend.clone(),
            display_name: "offline".into(),
            pair: LanguagePair {
                source: "ja".into(),
                target: "zh-Hans".into(),
            },
            route_policy: OfflineRoutePolicy::DirectOnly {},
            installation: if backend == OfflineBackend::LocalWorker {
                Some(LocalInstallationRef {
                    installation_id: "installation".into(),
                    revision: NonZeroU32::new(1).unwrap(),
                })
            } else {
                None
            },
            resolution: OfflineConfigResolution::Resolved {
                resource_snapshot_id: "resource-snapshot".into(),
                route_id: "route".into(),
            },
            request_defaults: d,
        });
        catalog.protocols.clear();
        catalog.profiles.clear();
        catalog.offline_engines.push(OfflineEngineDescriptor {
            engine_id: engine.into(),
            contract_revision: 1,
            label: "controlled offline".into(),
            brand_id: "controlled".into(),
            brand_label: "controlled".into(),
            backend: if backend == OfflineBackend::LocalWorker {
                OfflineExecutionBackend::LocalWorker
            } else {
                OfflineExecutionBackend::PlatformNative
            },
            allowed_purposes: vec![CatalogPurpose::Translation],
            model_identity: CatalogModelIdentity::NotApplicable,
            fields: vec![],
            source_refs: vec![],
            implementation: decision(),
            eligibility: decision(),
        });
        let mut req = request(&graph, CatalogPurpose::Translation);
        req.source = SavedServiceRef::Offline {
            source: OfflineConfigRef {
                revision_id: graph.revision_id.clone(),
                config_id: "offline".into(),
            },
        };
        let plan = compile(&graph, &req, TestReadingInput::Sample, &catalog).unwrap();
        let ServiceExecutionTarget::Offline { config, payload } = &plan.target else {
            panic!("true offline union")
        };
        assert_eq!(config.engine.engine_id, engine);
        assert_eq!(payload.source_language, "ja");
        assert_eq!(payload.target_language, "zh-Hans");
        let source_semantics = saved_service_test_source_semantics(
            &ConfigurationDocument::ServiceGraph {
                configuration: graph.clone(),
            },
            &req.source,
            req.purpose,
            ServiceTestOutputMode::Text,
            &ServiceTestEvidenceDomain::SampleJapaneseV1,
            &catalog,
        )
        .unwrap();
        let semantics = saved_service_test_request_semantics(
            &plan,
            &source_semantics,
            &PreparedTestReading::Sample,
            &PreparedTestAuthentication::None,
        )
        .unwrap();
        assert_eq!(
            semantics.authority_coverage,
            ServiceTestAuthorityCoverage::Unresolved {
                code: "offline_test_authority_unresolved".into()
            }
        );
        let encoded = serde_json::to_value(&plan.target).unwrap().to_string();
        assert!(
            !encoded.contains("endpoint")
                && !encoded.contains("modelId")
                && !encoded.contains("credential")
        );
        let mut wrong = unit("wrong-language", "English text", 20.);
        wrong.source_language = "en".into();
        req.input_scope = Some(selected(&wrong));
        assert_eq!(
            compile(
                &graph,
                &req,
                TestReadingInput::Selected {
                    unit: &wrong,
                    selected_token: None,
                    history: &[]
                },
                &catalog
            )
            .unwrap_err()
            .code,
            "offline_language_pair_mismatch"
        );
        req.input_scope = None;
        req.purpose = CatalogPurpose::Grammar;
        assert!(compile(&graph, &req, TestReadingInput::Sample, &catalog).is_err());
    }
}

#[test]
fn google_short_token_target_requires_explicit_https_but_local_non_google_http_remains_valid() {
    let (mut graph, catalog) = fixture(
        HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id: "p".into(),
            location: "global".into(),
        },
        CatalogPurpose::Translation,
    );
    let req = request(&graph, CatalogPurpose::Translation);
    assert!(compile(&graph, &req, TestReadingInput::Sample, &catalog).is_ok());
    graph.connections[0].endpoint = Some("http://127.0.0.1:1/v3".into());
    assert_eq!(
        compile(&graph, &req, TestReadingInput::Sample, &catalog)
            .unwrap_err()
            .code,
        "google_endpoint_requires_https"
    );
    for endpoint in [
        "https://user:password@example.invalid/v3",
        "https://example.invalid/v3?api_key=private",
        "https://example.invalid/v3#fragment",
    ] {
        graph.connections[0].endpoint = Some(endpoint.into());
        assert_eq!(
            compile(&graph, &req, TestReadingInput::Sample, &catalog)
                .unwrap_err()
                .code,
            "invalid_endpoint"
        );
    }
    graph.connections[0].endpoint =
        Some("https://explicit-user-configured.example.invalid/v3".into());
    assert!(compile(&graph, &req, TestReadingInput::Sample, &catalog).is_ok());
    let (local, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    assert!(
        compile(
            &local,
            &request(&local, CatalogPurpose::Translation),
            TestReadingInput::Sample,
            &catalog
        )
        .is_ok()
    );
}

use app_core::{
    domain::{
        credentials::CredentialSelection,
        model_config::credentials::{
            CredentialMaterialKind, CredentialStorageMetadata, CredentialVersionRef, PrincipalState,
        },
    },
    ports::{
        google_authorization::{
            GoogleClientKey, GoogleClientMode, GoogleClientProfile, GoogleClientSetup,
            GoogleGrantCursor, GoogleGrantState, GoogleOwnedGrant, GoogleVaultMaterial,
            google_scope_hash,
        },
        service_testing::{
            PreparedTestAuthentication, PreparedTestCredential, PreparedTestReading,
        },
    },
};
fn fingerprint(
    graph: &ConfigurationV3,
    catalog: &ServiceCatalog,
    domain: &ServiceTestEvidenceDomain,
) -> ServiceTestSemanticFingerprint {
    saved_service_test_source_semantics(
        &ConfigurationDocument::ServiceGraph {
            configuration: graph.clone(),
        },
        &request(graph, CatalogPurpose::Translation).source,
        CatalogPurpose::Translation,
        ServiceTestOutputMode::Text,
        domain,
        catalog,
    )
    .unwrap()
}
fn game_term(game: &str, word: &str, target: &str) -> TermSetV3 {
    TermSetV3 {
        term_set_id: format!("terms-{game}"),
        scope_key: format!("game:{game}"),
        game_id: Some(game.into()),
        display_name: "label".into(),
        enabled: true,
        terms: vec![TermV3 {
            term_id: "term".into(),
            source_text: Some(word.into()),
            target_text: Some(target.into()),
            source_language: Some("ja".into()),
            target_language: Some("zh-Hans".into()),
            enabled: true,
        }],
    }
}
#[test]
fn source_fingerprint_ignores_labels_overall_revision_unused_closures_and_inactive_controls() {
    let (mut graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    let domain = ServiceTestEvidenceDomain::SampleJapaneseV1;
    let original = fingerprint(&graph, &catalog, &domain);
    graph.revision_id = "renamed-version".into();
    graph.connections[0].display_name = "renamed".into();
    graph.models[0].display_name = "renamed model".into();
    graph.connections[0].availability_intent = true;
    graph.models[0]
        .capabilities
        .insert("grammar".into(), json!({"state":"supported"}));
    graph.models[0]
        .capabilities
        .insert("unrelated_observation".into(), json!({"state":"new"}));
    let mut unused_connection = graph.connections[0].clone();
    unused_connection.connection_id = "unused-connection".into();
    unused_connection.endpoint = Some("http://127.0.0.1:9999/unused".into());
    graph.connections.push(unused_connection);
    let mut unused_model = graph.models[0].clone();
    unused_model.model_id = "unused-model".into();
    unused_model.connection_id = "unused-connection".into();
    unused_model.actual_model_id = Some("unselected-model".into());
    graph.models.push(unused_model);
    graph
        .term_sets
        .push(game_term("another-game", "空", "unused"));
    graph.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .fallback = vec![SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "another-source".into(),
            model_id: "other-model".into(),
        },
    }];
    assert_eq!(fingerprint(&graph, &catalog, &domain), original);
}
#[test]
fn source_fingerprint_tracks_effective_target_policy_template_parameter_and_relevant_terms() {
    let (base, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    let domain = ServiceTestEvidenceDomain::SampleJapaneseV1;
    let original = fingerprint(&base, &catalog, &domain);
    for variant in 0..8 {
        let mut graph = base.clone();
        match variant {
            0 => graph.connections[0].endpoint = Some("http://127.0.0.1:2/v1".into()),
            1 => {
                graph.connections[0].http_options = Some(HttpServiceOptionsV1::OpenaiCompatible {
                    organization_id: Some("org".into()),
                    project_id: None,
                })
            }
            2 => graph.models[0].actual_model_id = Some("new-model".into()),
            3 => {
                graph.models[0]
                    .capabilities
                    .insert("streaming".into(), json!(true));
            }
            4 => {
                graph.models[0]
                    .capabilities
                    .insert("translation".into(), json!("supported"));
            }
            5 => {
                graph.models[0].parameters.insert(
                    "temperature".into(),
                    GenerationParameter::Set { value: json!(0) },
                );
            }
            6 => {
                graph.models[0]
                    .request_defaults
                    .get_mut(&CatalogPurpose::Translation)
                    .unwrap()
                    .execution
                    .timeout_ms = 20000
            }
            7 => {
                graph.models[0]
                    .request_defaults
                    .get_mut(&CatalogPurpose::Translation)
                    .unwrap()
                    .languages = ServiceLanguageDefaults::Http {
                    source: SourceLanguageChoice::InheritSource {},
                    target: TargetLanguageChoice::Explicit {
                        language: "en".into(),
                    },
                }
            }
            _ => unreachable!(),
        }
        assert_ne!(
            fingerprint(&graph, &catalog, &domain),
            original,
            "variant {variant}"
        );
    }
    let mut inherited = base.clone();
    inherited.models[0]
        .parameters
        .insert("temperature".into(), GenerationParameter::Inherit {});
    assert_eq!(fingerprint(&inherited, &catalog, &domain), original);
    inherited.models[0]
        .parameters
        .insert("temperature".into(), GenerationParameter::Omit {});
    assert_ne!(fingerprint(&inherited, &catalog, &domain), original);
    let mut templated = base.clone();
    templated.templates.push(TemplateV3 {
        template_id: "t".into(),
        purpose: CatalogPurpose::Translation,
        display_name: "name".into(),
        source_kind: "custom".into(),
        source_version: Some("1".into()),
        body: [("system".into(), json!("My system {{source}}"))].into(),
        variables: vec!["source".into()],
        compatibility: Default::default(),
    });
    templated.models[0]
        .default_templates
        .insert(CatalogPurpose::Translation, "t".into());
    let before = fingerprint(&templated, &catalog, &domain);
    templated.templates[0].display_name = "rename only".into();
    assert_eq!(fingerprint(&templated, &catalog, &domain), before);
    templated.templates[0]
        .body
        .insert("system".into(), json!("Another {{source}}"));
    assert_ne!(fingerprint(&templated, &catalog, &domain), before);
    let mut terms = base;
    terms.term_sets = vec![game_term("a", "空", "天空"), game_term("b", "空", "蓝天")];
    let a = ServiceTestEvidenceDomain::Selected {
        game_id: Some("a".into()),
        source_language: "ja".into(),
    };
    let b = ServiceTestEvidenceDomain::Selected {
        game_id: Some("b".into()),
        source_language: "ja".into(),
    };
    let fa = fingerprint(&terms, &catalog, &a);
    let fb = fingerprint(&terms, &catalog, &b);
    let fs = fingerprint(&terms, &catalog, &domain);
    terms.term_sets[0].terms[0].target_text = Some("changed".into());
    assert_ne!(fingerprint(&terms, &catalog, &a), fa);
    assert_eq!(fingerprint(&terms, &catalog, &b), fb);
    assert_eq!(fingerprint(&terms, &catalog, &domain), fs);
}
fn prepared_plan(
    graph: &ConfigurationV3,
    catalog: &ServiceCatalog,
    reading: &PreparedTestReading,
) -> SavedServiceTestPlan {
    let mut req = request(graph, CatalogPurpose::Translation);
    let input = match reading {
        PreparedTestReading::Sample => TestReadingInput::Sample,
        PreparedTestReading::Selected {
            unit,
            selected_token,
            history,
            ..
        } => {
            req.input_scope = Some(selected(unit));
            TestReadingInput::Selected {
                unit,
                selected_token: selected_token.as_ref(),
                history,
            }
        }
    };
    compile(graph, &req, input, catalog).unwrap()
}
#[test]
fn request_fingerprint_binds_frozen_input_corrections_and_only_consented_history() {
    let (mut graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    graph.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .context
        .mode = HistoryInputMode::Both;
    let current = unit("current", "青い空。", 100.);
    let mut old = unit("prior", "雨がやんだ。", 20.);
    old.translations.push(Translation {
        translation_id: "translation".into(),
        task_id: "actual-history-task".into(),
        text: "Original translation".into(),
        provider_label: "display only".into(),
        target_language: "zh-Hans".into(),
        correction: Some(Correction {
            revision: 1,
            text: "Corrected translation".into(),
        }),
    });
    let reading = PreparedTestReading::Selected {
        unit: Box::new(current.clone()),
        selected_token: None,
        lexical_snapshot_id: None,
        history: vec![old.clone()],
    };
    let domain = saved_service_test_evidence_domain(&reading).unwrap();
    let source = fingerprint(&graph, &catalog, &domain);
    let plan = prepared_plan(&graph, &catalog, &reading);
    let original = saved_service_test_request_semantics(
        &plan,
        &source,
        &reading,
        &PreparedTestAuthentication::None,
    )
    .unwrap();
    let wrong_domain_source = fingerprint(
        &graph,
        &catalog,
        &ServiceTestEvidenceDomain::SampleJapaneseV1,
    );
    assert_eq!(
        saved_service_test_request_semantics(
            &plan,
            &wrong_domain_source,
            &reading,
            &PreparedTestAuthentication::None
        )
        .unwrap_err()
        .code,
        "test_source_semantics_scope_mismatch"
    );

    let mut later = reading.clone();
    let PreparedTestReading::Selected {
        history,
        lexical_snapshot_id,
        ..
    } = &mut later
    else {
        unreachable!()
    };
    history.push(unit("later", "これは次の台詞。", 200.));
    *lexical_snapshot_id = Some("unused-lexical-refresh".into());
    assert_eq!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &later,
            &PreparedTestAuthentication::None
        )
        .unwrap(),
        original
    );
    let mut corrected = reading.clone();
    let PreparedTestReading::Selected { history, .. } = &mut corrected else {
        unreachable!()
    };
    history[0].translations[0]
        .correction
        .as_mut()
        .unwrap()
        .revision = 2;
    let changed = saved_service_test_request_semantics(
        &plan,
        &source,
        &corrected,
        &PreparedTestAuthentication::None,
    )
    .unwrap();
    assert_ne!(changed.input_sha256, original.input_sha256);
    assert_ne!(changed.sha256, original.sha256);
    assert_eq!(changed.authority_sha256, original.authority_sha256);
    let mut changed_unit = current;
    changed_unit.text_revision += 1;
    changed_unit.source_text = "夕焼けの空。".into();
    changed_unit.source_hash = content_hash(&changed_unit.source_text);
    let changed_reading = PreparedTestReading::Selected {
        unit: Box::new(changed_unit),
        selected_token: None,
        lexical_snapshot_id: None,
        history: vec![old],
    };
    assert_eq!(
        saved_service_test_evidence_domain(&changed_reading).unwrap(),
        domain
    );
    assert!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &changed_reading,
            &PreparedTestAuthentication::None
        )
        .is_err()
    );
    let new_plan = prepared_plan(&graph, &catalog, &changed_reading);
    assert_ne!(
        saved_service_test_request_semantics(
            &new_plan,
            &source,
            &changed_reading,
            &PreparedTestAuthentication::None
        )
        .unwrap()
        .sha256,
        original.sha256
    );
}
fn key_auth() -> PreparedTestAuthentication {
    PreparedTestAuthentication::PoolMember(PreparedTestCredential {
        selection: CredentialSelection {
            credential_id: "credential".into(),
            vault_ref: "private-immutable-version-1".into(),
            pool_id: "pool".into(),
            pool_revision: 2,
            member_id: "member".into(),
            masked_identity: "label".into(),
        },
        storage: CredentialStorageMetadata::EnvelopeV1 {
            kind: CredentialMaterialKind::ApiKey,
        },
        lifecycle_revision: 1,
        pool_policy_sha256: "a".repeat(64),
    })
}
#[test]
fn request_authority_coverage_is_exact_and_never_hashes_a_masked_label_as_version() {
    let (mut graph, mut catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    graph.connections[0].auth_mode = "api_key".into();
    graph.connections[0].credential_pool_id = Some("pool".into());
    graph.connections[0].credential_pool_revision = NonZeroU32::new(2);
    catalog.protocols[0].auth_kinds = vec![CatalogAuthKind::BearerApiKey];
    catalog.profiles[0].auth_kinds = catalog.protocols[0].auth_kinds.clone();
    let reading = PreparedTestReading::Sample;
    let plan = prepared_plan(&graph, &catalog, &reading);
    let source = fingerprint(
        &graph,
        &catalog,
        &ServiceTestEvidenceDomain::SampleJapaneseV1,
    );
    let auth = key_auth();
    let original = saved_service_test_request_semantics(&plan, &source, &reading, &auth).unwrap();
    assert_eq!(
        original.authority_coverage,
        ServiceTestAuthorityCoverage::Projected
    );
    let mut renamed = auth.clone();
    let PreparedTestAuthentication::PoolMember(credential) = &mut renamed else {
        unreachable!()
    };
    credential.selection.masked_identity = "renamed".into();
    assert_eq!(
        saved_service_test_request_semantics(&plan, &source, &reading, &renamed).unwrap(),
        original
    );
    for variant in 0..5 {
        let mut changed = auth.clone();
        let PreparedTestAuthentication::PoolMember(credential) = &mut changed else {
            unreachable!()
        };
        match variant {
            0 => credential.selection.member_id = "another-member".into(),
            1 => credential.selection.vault_ref = "private-immutable-version-2".into(),
            2 => credential.lifecycle_revision = 2,
            3 => credential.pool_policy_sha256 = "b".repeat(64),
            4 => credential.selection.credential_id = "another-credential".into(),
            _ => unreachable!(),
        }
        let changed =
            saved_service_test_request_semantics(&plan, &source, &reading, &changed).unwrap();
        assert_ne!(
            changed.authority_sha256, original.authority_sha256,
            "variant {variant}"
        );
        assert_eq!(changed.input_sha256, original.input_sha256);
    }
    assert!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &reading,
            &PreparedTestAuthentication::None
        )
        .is_err()
    );
    let mut wrong = auth;
    let PreparedTestAuthentication::PoolMember(credential) = &mut wrong else {
        unreachable!()
    };
    credential.storage = CredentialStorageMetadata::EnvelopeV1 {
        kind: CredentialMaterialKind::GoogleServiceAccount,
    };
    assert!(saved_service_test_request_semantics(&plan, &source, &reading, &wrong).is_err());
}
fn oauth_grant() -> GoogleOwnedGrant {
    GoogleOwnedGrant {
        cursor: GoogleGrantCursor {
            grant_id: "grant".into(),
            authorization_epoch: NonZeroU32::new(3),
            material_revision: NonZeroU32::new(1),
            control_revision: 1,
        },
        client: GoogleClientProfile {
            key: GoogleClientKey {
                client_profile_id: "client".into(),
                client_profile_revision: NonZeroU32::new(1).unwrap(),
            },
            public_client_id: "controlled.apps.googleusercontent.com".into(),
            client_mode: GoogleClientMode::Custom,
            policy_sha256: "a".repeat(64),
            setup_state: GoogleClientSetup::Verified,
        },
        state: GoogleGrantState::Active,
        scopes_sha256: google_scope_hash(),
        principal_state: PrincipalState::Unknown,
        material: Some(GoogleVaultMaterial {
            credential: CredentialVersionRef {
                credential_id: "refresh-1".into(),
                credential_version_id: "version-1".into(),
            },
            vault_ref: "private-refresh-1".into(),
            storage: CredentialStorageMetadata::EnvelopeV1 {
                kind: CredentialMaterialKind::GoogleOauthRefresh,
            },
            lifecycle_revision: 1,
        }),
    }
}
#[test]
fn oauth_refresh_cursor_changes_do_not_masquerade_as_authorization_scope_changes() {
    let (mut graph, mut catalog) = fixture(
        HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id: "p".into(),
            location: "global".into(),
        },
        CatalogPurpose::Translation,
    );
    graph.connections[0].auth_mode = "google_oauth".into();
    graph.connections[0].credential_pool_id = None;
    graph.connections[0].credential_pool_revision = None;
    graph.connections[0].google_authorization = Some(GoogleAuthorizationRef {
        grant_id: "grant".into(),
        authorization_epoch: NonZeroU32::new(3).unwrap(),
    });
    catalog.protocols[0].auth_kinds = vec![CatalogAuthKind::GoogleOauth];
    catalog.profiles[0].auth_kinds = catalog.protocols[0].auth_kinds.clone();
    let reading = PreparedTestReading::Sample;
    let plan = prepared_plan(&graph, &catalog, &reading);
    let source = fingerprint(
        &graph,
        &catalog,
        &ServiceTestEvidenceDomain::SampleJapaneseV1,
    );
    let grant = oauth_grant();
    let original = saved_service_test_request_semantics(
        &plan,
        &source,
        &reading,
        &PreparedTestAuthentication::GoogleOwnedGrant(grant.clone()),
    )
    .unwrap();
    let mut refreshed = grant.clone();
    refreshed.cursor.material_revision = NonZeroU32::new(2);
    refreshed.cursor.control_revision = 3;
    let material = refreshed.material.as_mut().unwrap();
    material.credential.credential_id = "refresh-2".into();
    material.credential.credential_version_id = "version-2".into();
    material.vault_ref = "private-refresh-2".into();
    material.lifecycle_revision = 2;
    assert_eq!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &reading,
            &PreparedTestAuthentication::GoogleOwnedGrant(refreshed)
        )
        .unwrap(),
        original
    );
    let mut changed = grant.clone();
    changed.client.policy_sha256 = "b".repeat(64);
    assert_ne!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &reading,
            &PreparedTestAuthentication::GoogleOwnedGrant(changed)
        )
        .unwrap()
        .authority_sha256,
        original.authority_sha256
    );
    let mut changed = grant.clone();
    changed.principal_state = PrincipalState::Verified {
        subject: "verified-subject".into(),
    };
    assert_ne!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &reading,
            &PreparedTestAuthentication::GoogleOwnedGrant(changed)
        )
        .unwrap()
        .authority_sha256,
        original.authority_sha256
    );
    let mut wrong = grant;
    wrong.cursor.authorization_epoch = NonZeroU32::new(4);
    assert!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &reading,
            &PreparedTestAuthentication::GoogleOwnedGrant(wrong)
        )
        .is_err()
    );
}

#[test]
fn word_request_fingerprint_binds_real_lexical_snapshot_and_fixed_selection() {
    let (graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::ContextWord,
    );
    let mut unit = unit("actual-word-unit", "青い空。", 100.);
    let token = LexicalToken {
        surface: "空".into(),
        lemma: "空".into(),
        reading: "ソラ".into(),
        part_of_speech: "名詞".into(),
        byte_start: 6,
        byte_end: 9,
        utf16_start: 2,
        utf16_end: 3,
        known: true,
    };
    unit.tokens.push(token.clone());
    let mut req = request(&graph, CatalogPurpose::ContextWord);
    req.input_scope = Some(selected(&unit));
    let plan = compile(
        &graph,
        &req,
        TestReadingInput::Selected {
            unit: &unit,
            selected_token: Some(&token),
            history: &[],
        },
        &catalog,
    )
    .unwrap();
    let reading = PreparedTestReading::Selected {
        unit: Box::new(unit.clone()),
        selected_token: Some(token.clone()),
        lexical_snapshot_id: Some("actual-lexical-1".into()),
        history: vec![],
    };
    let domain = saved_service_test_evidence_domain(&reading).unwrap();
    let source = saved_service_test_source_semantics(
        &ConfigurationDocument::ServiceGraph {
            configuration: graph.clone(),
        },
        &req.source,
        req.purpose,
        ServiceTestOutputMode::Text,
        &domain,
        &catalog,
    )
    .unwrap();
    let original = saved_service_test_request_semantics(
        &plan,
        &source,
        &reading,
        &PreparedTestAuthentication::None,
    )
    .unwrap();
    let mut missing = reading.clone();
    let PreparedTestReading::Selected {
        lexical_snapshot_id,
        ..
    } = &mut missing
    else {
        unreachable!()
    };
    *lexical_snapshot_id = None;
    assert_eq!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &missing,
            &PreparedTestAuthentication::None
        )
        .unwrap_err()
        .code,
        "test_lexical_proof_missing"
    );
    let mut changed = reading.clone();
    let PreparedTestReading::Selected {
        lexical_snapshot_id,
        ..
    } = &mut changed
    else {
        unreachable!()
    };
    *lexical_snapshot_id = Some("actual-lexical-2".into());
    assert_ne!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &changed,
            &PreparedTestAuthentication::None
        )
        .unwrap()
        .input_sha256,
        original.input_sha256
    );
    let mut unrelated = reading.clone();
    let PreparedTestReading::Selected { unit, .. } = &mut unrelated else {
        unreachable!()
    };
    unit.tokens.push(LexicalToken {
        surface: "青い".into(),
        lemma: "青い".into(),
        reading: "アオイ".into(),
        part_of_speech: "形容詞".into(),
        byte_start: 0,
        byte_end: 6,
        utf16_start: 0,
        utf16_end: 2,
        known: true,
    });
    assert_eq!(
        saved_service_test_request_semantics(
            &plan,
            &source,
            &unrelated,
            &PreparedTestAuthentication::None
        )
        .unwrap(),
        original
    );
}
#[test]
fn primary_only_request_hash_does_not_claim_or_depend_on_unexecuted_fallbacks() {
    let (mut graph, catalog) = fixture(
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: None,
            project_id: None,
        },
        CatalogPurpose::Translation,
    );
    let source = fingerprint(
        &graph,
        &catalog,
        &ServiceTestEvidenceDomain::SampleJapaneseV1,
    );
    let plan = prepared_plan(&graph, &catalog, &PreparedTestReading::Sample);
    let original = saved_service_test_request_semantics(
        &plan,
        &source,
        &PreparedTestReading::Sample,
        &PreparedTestAuthentication::None,
    )
    .unwrap();
    graph.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .fallback = vec![SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "unavailable-fallback".into(),
            model_id: "other".into(),
        },
    }];
    let changed_source = fingerprint(
        &graph,
        &catalog,
        &ServiceTestEvidenceDomain::SampleJapaneseV1,
    );
    assert_eq!(changed_source, source);
    let plan = prepared_plan(&graph, &catalog, &PreparedTestReading::Sample);
    assert_eq!(
        saved_service_test_request_semantics(
            &plan,
            &changed_source,
            &PreparedTestReading::Sample,
            &PreparedTestAuthentication::None
        )
        .unwrap(),
        original
    );
}
