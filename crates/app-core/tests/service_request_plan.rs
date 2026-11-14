//! Pure contract tests: no descriptor/plan is proof of a live provider or resource lease.
use app_core::{
    application::{request_plan::RequestPlan, service_request_plan::*},
    contracts::*,
    domain::{content_hash, model_config::service_graph::*, service_execution::*},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn unit(id: &str, text: &str, time: f64) -> ReadingUnit {
    ReadingUnit {
        source_summary: None,
        unit_id: id.into(),
        session_id: "session".into(),
        game_id: Some("game".into()),
        text_revision: 1,
        source_text: text.into(),
        source_hash: content_hash(text),
        source_language: "ja".into(),
        created_at_ms: time,
        translations: vec![],
        tokens: vec![],
    }
}
fn defaults() -> ServiceRequestDefaults {
    ServiceRequestDefaults {
        purpose: CatalogPurpose::Translation,
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
            timeout_ms: 12000,
            concurrency: 2,
            retries: 2,
        },
        cache: ServiceCachePolicy { enabled: true },
        fallback: vec![],
    }
}
fn graph() -> ConfigurationV3 {
    serde_json::from_value(json!({"revisionId":"r3","codecVersion":3,
      "connections":[{"connectionId":"c","displayName":"contract test","protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:1/v1","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":true,"adapterBinding":{"protocol":{"protocolId":"openai_compatible","contractRevision":1},"profile":{"profileId":"contract-profile","profileRevision":1}},"httpOptions":{"protocol":"openai_compatible"},"origin":{"kind":"custom","site":"explicit-site"}}],
      "models":[{"modelId":"m","connectionId":"c","actualModelId":"actual-model","displayName":"contract model","selectionSource":"manual","capabilities":{"streaming":false},"parameters":{},"defaultTemplates":{},"templatePurpose":"translation","requestDefaults":{"translation":defaults()}}],
      "bindings":[{"bindingId":"b","scopeKey":"global","purpose":"translation","bindingMode":"override","enabled":true,"context":{},"schedule":{},"parameters":{},"engines":[{"engineId":"e","target":{"kind":"http","modelId":"m"},"enabled":true,"httpParameters":{},"policyOverrides":{"context":{"mode":"inherit"},"execution":{"timeoutMs":{"mode":"inherit"},"concurrency":{"mode":"inherit"},"retries":{"mode":"inherit"}},"cache":{"mode":"inherit"}}}],"semanticsVersion":"typed_v3","scopeOverrides":{"context":{"mode":"inherit"},"execution":{"timeoutMs":{"mode":"inherit"},"concurrency":{"mode":"inherit"},"retries":{"mode":"inherit"}},"cache":{"mode":"inherit"}}}],
      "templates":[],"termSets":[],"offlineEngineConfigs":[]})).unwrap()
}
/// Explicitly controlled contract directory. Actual production registration is an infrastructure test.
fn directory(g: &ConfigurationV3, model_identity: CatalogModelIdentity) -> ServiceCatalog {
    let binding = g.connections[0].adapter_binding.clone().unwrap();
    let auth = match g.connections[0].auth_mode.as_str() {
        "google_oauth" => CatalogAuthKind::GoogleOauth,
        "google_service_account" => CatalogAuthKind::GoogleServiceAccount,
        "baidu_app_secret" => CatalogAuthKind::BaiduAppSecret,
        "youdao_app_secret" => CatalogAuthKind::YoudaoAppSecret,
        "api_key" => match g.connections[0].protocol.as_str() {
            "anthropic" => CatalogAuthKind::AnthropicApiKey,
            "gemini_generate_content" | "google_translate_v2" => CatalogAuthKind::GoogleApiKey,
            "azure_translator" => CatalogAuthKind::AzureSubscriptionKey,
            "deepl_v2" => CatalogAuthKind::DeeplAuthKey,
            _ => CatalogAuthKind::BearerApiKey,
        },
        _ => CatalogAuthKind::None,
    };
    ServiceCatalog {
        brands: vec![],
        schema_version: 1,
        protocols: vec![ProtocolDescriptor {
            key: binding.protocol.clone(),
            label: "controlled protocol".into(),
            family: CatalogFamily::Llm,
            auth_kinds: vec![auth],
            model_identity,
            wire_formats: vec![CatalogWireFormat::Json],
            allowed_purposes: vec![CatalogPurpose::Translation],
            capabilities: vec![],
            implementation: CatalogDecision {
                state: CapabilityState::Unknown,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec![],
        }],
        profiles: vec![CompatibilityProfileDescriptor {
            key: binding.profile,
            protocol: binding.protocol,
            allowed_purposes: vec![CatalogPurpose::Translation],
            auth_kinds: vec![auth],
            option_field_ids: vec![],
            source_refs: vec![],
        }],
        presets: vec![],
        offline_engines: vec![],
        sources: vec![],
    }
}
fn plan(
    g: &ConfigurationV3,
    u: &ReadingUnit,
    h: &[ReadingUnit],
    d: &ServiceCatalog,
) -> AppResult<ComposedServicePlan> {
    compose_service_plan(ServicePlanInput {
        graph: g,
        expected_revision_id: &g.revision_id,
        binding_id: "b",
        engine_id: "e",
        unit: u,
        selected_token: None,
        history: h,
        source_language: "ja",
        target_language: "zh-CN",
        catalog: d,
    })
}
fn request(p: &ComposedServicePlan) -> &RequestPlan {
    match &p.target {
        ServiceExecutionTarget::Http { plan } => &plan.request,
        _ => panic!("expected real HTTP variant"),
    }
}
fn envelope(p: &ComposedServicePlan) -> Value {
    serde_json::from_str(request(p).messages[1]["content"].as_str().unwrap()).unwrap()
}

#[test]
fn inherited_defaults_use_actual_preferences_and_false_zero_omit_keep_provenance() {
    let mut g = graph();
    let u = unit("u", "雨がやんだ。", 100.);
    let h = [unit("before", "秘密", 10.)];
    g.models[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0.7) },
    );
    g.models[0].parameters.insert(
        "top_p".into(),
        GenerationParameter::Set { value: json!(0.8) },
    );
    g.bindings[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0) },
    );
    g.bindings[0].engines[0]
        .http_parameters
        .insert("temperature".into(), GenerationParameter::Inherit {});
    g.bindings[0].engines[0]
        .http_parameters
        .insert("top_p".into(), GenerationParameter::Omit {});
    g.bindings[0].scope_overrides.execution.retries = PolicyOverride::Set { value: 0 };
    g.bindings[0].scope_overrides.cache = PolicyOverride::Set {
        value: ServiceCachePolicy { enabled: false },
    };
    g.bindings[0].engines[0].policy_overrides.context = PolicyOverride::Set {
        value: ServiceContextConsent {
            enabled: false,
            max_sentences: 0,
            mode: HistoryInputMode::Both,
        },
    };
    let p = plan(&g, &u, &h, &directory(&g, CatalogModelIdentity::Required)).unwrap();
    assert_eq!(p.policy.execution.timeout_ms, 12000);
    assert_eq!(p.policy.execution.retries, 0);
    assert!(!p.policy.cache.enabled);
    assert_eq!(
        p.provenance["execution.retries"].kind,
        PlanSourceKind::Scope
    );
    assert_eq!(p.provenance["context.enabled"].kind, PlanSourceKind::Engine);
    assert_eq!(request(&p).parameters["temperature"], 0);
    assert!(!request(&p).parameters.contains_key("top_p"));
    assert_eq!(
        p.provenance["parameters.top_p"].field,
        "parameters.top_p.omit"
    );
    assert!(envelope(&p)["context"].as_array().unwrap().is_empty());
    assert_eq!(request(&p).target_language, "zh-CN");
    assert_eq!(
        p.provenance["targetLanguage"].kind,
        PlanSourceKind::ReadingPreference
    );
    assert!(!request(&p).stream);
}

#[test]
fn all_context_modes_filter_exact_session_game_time_and_keep_corrected_translations() {
    let mut g = graph();
    let current = unit("u", "雨。", 100.);
    let mut prior = unit("prior", "原文", 10.);
    prior.translations.push(Translation {
        translation_id: "tr".into(),
        task_id: "t".into(),
        text: "旧译".into(),
        provider_label: "test".into(),
        target_language: "zh-CN".into(),
        correction: Some(Correction {
            revision: 1,
            text: "人工修正".into(),
        }),
    });
    let mut other_session = prior.clone();
    other_session.session_id = "other".into();
    let mut other_game = prior.clone();
    other_game.game_id = Some("other".into());
    let future = unit("future", "不允许未来", 101.);
    let h = [other_session, prior, other_game, future, current.clone()];
    for mode in [
        HistoryInputMode::Original,
        HistoryInputMode::Translation,
        HistoryInputMode::Both,
    ] {
        g.models[0]
            .request_defaults
            .get_mut(&CatalogPurpose::Translation)
            .unwrap()
            .context
            .mode = mode.clone();
        let p = plan(
            &g,
            &current,
            &h,
            &directory(&g, CatalogModelIdentity::Required),
        )
        .unwrap();
        let e = envelope(&p);
        let c = &e["context"][0];
        assert_eq!(p.context_units, vec![("prior".into(), 1)]);
        assert_eq!(e["context"].as_array().unwrap().len(), 1);
        assert_eq!(
            c.get("source").is_some(),
            mode != HistoryInputMode::Translation
        );
        assert_eq!(
            c.get("translations").is_some(),
            mode != HistoryInputMode::Original
        );
        if mode != HistoryInputMode::Original {
            assert_eq!(c["translations"][0]["text"], "人工修正");
        }
    }
    g.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .context
        .max_sentences = 0;
    assert!(
        envelope(
            &plan(
                &g,
                &current,
                &h,
                &directory(&g, CatalogModelIdentity::Required)
            )
            .unwrap()
        )["context"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn template_and_terms_are_shared_material_with_exact_source_identity_and_scope_precedence() {
    let mut g = graph();
    let u = unit("u", "彼女と雨。", 100.);
    g.templates.push(TemplateV3 {
        template_id: "template".into(),
        purpose: CatalogPurpose::Translation,
        display_name: "custom".into(),
        source_kind: "custom".into(),
        source_version: Some("1".into()),
        body: BTreeMap::from([
            ("system".into(), json!("参考 {{source}} {{history}}")),
            ("user".into(), json!("{{source}} {{terms}}")),
        ]),
        variables: vec!["source".into(), "history".into(), "terms".into()],
        compatibility: BTreeMap::new(),
    });
    g.models[0]
        .default_templates
        .insert(CatalogPurpose::Translation, "template".into());
    g.term_sets.push(TermSetV3 {
        term_set_id: "ts".into(),
        scope_key: "game:game".into(),
        game_id: Some("game".into()),
        display_name: "terms".into(),
        enabled: true,
        terms: vec![TermV3 {
            term_id: "term".into(),
            source_text: Some("彼女".into()),
            target_text: Some("她".into()),
            source_language: None,
            target_language: None,
            enabled: true,
        }],
    });
    let p = plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Required)).unwrap();
    assert_eq!(
        envelope(&p)["glossary"][0],
        json!({"source":"彼女","target":"她"})
    );
    assert!(
        request(&p).messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("[untrusted input.source data]")
    );
    assert_eq!(p.provenance["template"].entity_id.as_deref(), Some("m"));
    assert_eq!(
        p.provenance["template.body"].entity_id.as_deref(),
        Some("template")
    );
    assert_eq!(p.term_sources, vec![("ts".into(), "term".into())]);
    if let ServiceExecutionTarget::Http { plan } = &p.target {
        assert_eq!(
            plan.template.as_ref().unwrap().source_version.as_deref(),
            Some("1")
        );
    }
    g.templates[0]
        .compatibility
        .insert("sourceLanguages".into(), json!(["en"]));
    assert_eq!(
        plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Required))
            .unwrap_err()
            .code,
        "template_incompatible"
    );
}

#[test]
fn required_fields_and_exact_versions_fail_before_any_vault_authority() {
    let u = unit("u", "雨。", 100.);
    for (mut g, expected) in [
        (graph(), "missing_endpoint"),
        (graph(), "missing_model"),
        (graph(), "unknown_profile_version"),
    ] {
        let d = directory(&g, CatalogModelIdentity::Required);
        match expected {
            "missing_endpoint" => g.connections[0].endpoint = None,
            "missing_model" => g.models[0].actual_model_id = None,
            _ => {
                g.connections[0]
                    .adapter_binding
                    .as_mut()
                    .unwrap()
                    .profile
                    .profile_revision = 2
            }
        }
        assert_eq!(plan(&g, &u, &[], &d).unwrap_err().code, expected);
    }
    let mut g = graph();
    g.connections[0].auth_mode = "api_key".into();
    assert_eq!(
        plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Required))
            .unwrap_err()
            .code,
        "missing_credential_reference"
    );
    let mut d = directory(&g, CatalogModelIdentity::Required);
    g.connections[0].credential_pool_id = Some("opaque-pool".into());
    g.connections[0].credential_pool_revision = std::num::NonZeroU32::new(2);
    // A reference can be frozen, but this pure result cannot attest its actual repository existence.
    assert!(plan(&g, &u, &[], &d).is_ok());
    d.protocols[0].key.contract_revision = 2;
    assert_eq!(
        plan(&g, &u, &[], &d).unwrap_err().code,
        "unknown_protocol_version"
    );
    let g = graph();
    let d = directory(&g, CatalogModelIdentity::Required);
    assert_eq!(
        compose_service_plan(ServicePlanInput {
            graph: &g,
            expected_revision_id: "wrong",
            binding_id: "b",
            engine_id: "e",
            unit: &u,
            selected_token: None,
            history: &[],
            source_language: "ja",
            target_language: "zh-CN",
            catalog: &d
        })
        .unwrap_err()
        .code,
        "service_revision_mismatch"
    );
}

#[test]
fn exact_http_thirteen_option_variants_survive_without_model_or_credential_invention() {
    let options = vec![
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: Some("org".into()),
            project_id: Some("project".into()),
        },
        HttpServiceOptionsV1::Anthropic {
            workspace_id: Some("workspace".into()),
        },
        HttpServiceOptionsV1::AzureTranslator {
            resource_type: Some(AzureResourceType::Regional),
            region: Some("region".into()),
        },
        HttpServiceOptionsV1::OpenaiResponses {
            organization_id: None,
            project_id: Some("project".into()),
        },
        HttpServiceOptionsV1::GeminiGenerateContent {},
        HttpServiceOptionsV1::OllamaNative {},
        HttpServiceOptionsV1::LmstudioNativeV1 {},
        HttpServiceOptionsV1::DeeplV2 {
            plan: Some(DeeplPlan::Pro),
        },
        HttpServiceOptionsV1::GoogleTranslateV2 {},
        HttpServiceOptionsV1::GoogleTranslateV3 {
            project_id: "project".into(),
            location: "global".into(),
        },
        HttpServiceOptionsV1::BaiduVip {},
        HttpServiceOptionsV1::YoudaoV3 {},
        HttpServiceOptionsV1::TencentTokenhubTranslation {},
    ];
    assert_eq!(options.len(), 13);
    for option in options {
        let mut g = graph();
        let protocol = option.protocol_id().to_string();
        g.connections[0].protocol = protocol.clone();
        if protocol == "google_translate_v3" {
            g.connections[0].endpoint = Some("https://translate.googleapis.com/v3".into());
        }
        g.connections[0]
            .adapter_binding
            .as_mut()
            .unwrap()
            .protocol
            .protocol_id = protocol.clone();
        g.connections[0].http_options = Some(option.clone());
        // Optional is a controlled directory contract, not a claim about the registered provider.
        g.models[0].actual_model_id = None;
        let d = directory(&g, CatalogModelIdentity::Optional);
        let p = plan(&g, &unit("u", "雨。", 100.), &[], &d).unwrap();
        let ServiceExecutionTarget::Http { plan } = &p.target else {
            panic!("HTTP variant required")
        };
        assert_eq!(plan.http_options, option);
        assert_eq!(plan.request.model, None);
        assert_eq!(plan.connection.protocol, protocol);
        assert_eq!(plan.origin, g.connections[0].origin.clone().unwrap());
        assert_eq!(
            plan.request.adapter_binding,
            g.connections[0].adapter_binding
        );
    }
}

#[test]
fn azure_deepl_and_google_unselected_route_fields_are_not_first_option_defaults() {
    let u = unit("u", "雨。", 100.);
    for (option, expected) in [
        (
            HttpServiceOptionsV1::AzureTranslator {
                resource_type: None,
                region: Some("region".into()),
            },
            "missing_azure_resource_type",
        ),
        (
            HttpServiceOptionsV1::AzureTranslator {
                resource_type: Some(AzureResourceType::MultiService),
                region: None,
            },
            "missing_azure_region",
        ),
        (
            HttpServiceOptionsV1::DeeplV2 { plan: None },
            "missing_deepl_plan",
        ),
        (
            HttpServiceOptionsV1::GoogleTranslateV3 {
                project_id: String::new(),
                location: "global".into(),
            },
            "missing_google_route",
        ),
    ] {
        let mut g = graph();
        g.connections[0].protocol = option.protocol_id().into();
        if option.protocol_id() == "google_translate_v3" {
            g.connections[0].endpoint = Some("https://translate.googleapis.com/v3".into());
        }
        g.connections[0]
            .adapter_binding
            .as_mut()
            .unwrap()
            .protocol
            .protocol_id = option.protocol_id().into();
        g.connections[0].http_options = Some(option);
        assert_eq!(
            plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Optional))
                .unwrap_err()
                .code,
            expected
        );
    }
}
fn offline_graph() -> ConfigurationV3 {
    let mut g = graph();
    g.connections.clear();
    g.models.clear();
    g.bindings[0].engines[0].target = EngineTarget::Offline {
        offline_config_id: "offline".into(),
    };
    let mut defaults = defaults();
    defaults.languages = ServiceLanguageDefaults::OfflineUseConfigPair {};
    g.offline_engine_configs.push(OfflineEngineConfigV1 {
        id: "offline".into(),
        engine: OfflineEngineKey {
            engine_id: "argos_translate".into(),
            contract_revision: std::num::NonZeroU32::new(1).unwrap(),
        },
        backend: OfflineBackend::LocalWorker,
        display_name: "offline".into(),
        pair: LanguagePair {
            source: "ja".into(),
            target: "zh-CN".into(),
        },
        route_policy: OfflineRoutePolicy::DirectOnly {},
        installation: Some(LocalInstallationRef {
            installation_id: "installation".into(),
            revision: std::num::NonZeroU32::new(2).unwrap(),
        }),
        resolution: OfflineConfigResolution::Resolved {
            resource_snapshot_id: "snapshot".into(),
            route_id: "route".into(),
        },
        request_defaults: defaults,
    });
    g
}
fn offline_directory() -> ServiceCatalog {
    ServiceCatalog {
        brands: vec![],
        schema_version: 1,
        protocols: vec![],
        profiles: vec![],
        presets: vec![],
        sources: vec![],
        offline_engines: vec![OfflineEngineDescriptor {
            engine_id: "argos_translate".into(),
            contract_revision: 1,
            label: "controlled offline contract".into(),
            brand_id: "contract".into(),
            brand_label: "contract".into(),
            backend: OfflineExecutionBackend::LocalWorker,
            allowed_purposes: vec![CatalogPurpose::Translation],
            model_identity: CatalogModelIdentity::NotApplicable,
            fields: vec![],
            source_refs: vec![],
            implementation: CatalogDecision {
                state: CapabilityState::Unknown,
                reason: Some(CatalogReason::OfflineRuntimeNotImplemented),
            },
            eligibility: CatalogDecision {
                state: CapabilityState::Unknown,
                reason: Some(CatalogReason::RequiresSavedConfiguration),
            },
        }],
    }
}
#[test]
fn offline_payload_has_real_pair_route_and_policy_without_fake_http() {
    let g = offline_graph();
    let p = plan(
        &g,
        &unit("u", "雨。", 100.),
        &[unit("before", "前句", 10.)],
        &offline_directory(),
    )
    .unwrap();
    let ServiceExecutionTarget::Offline { config, payload } = &p.target else {
        panic!("offline variant required")
    };
    assert_eq!(config.id, "offline");
    assert_eq!(payload.source_language, "ja");
    assert_eq!(payload.target_language, "zh-CN");
    assert_eq!(payload.context[0]["source"], "前句");
    assert_eq!(p.policy.execution.timeout_ms, 12000);
    assert_eq!(p.provenance["pair"].entity_id.as_deref(), Some("offline"));
    let wire = serde_json::to_value(&p).unwrap();
    assert!(wire["target"].get("request").is_none());
    let text = serde_json::to_string(&wire).unwrap();
    for field in [
        "credentialPoolId",
        "actualModelId",
        "endpoint",
        "messages",
        "httpOptions",
        "httpParameters",
    ] {
        assert!(!text.contains(field), "unexpected offline field {field}");
    }
}
#[test]
fn offline_rejects_unresolved_wrong_pair_http_shadow_and_unknown_contract() {
    let u = unit("u", "雨。", 100.);
    let d = offline_directory();
    let mut g = offline_graph();
    g.offline_engine_configs[0].resolution = OfflineConfigResolution::Draft {
        observed_resource: None,
        proposed_route: None,
    };
    assert_eq!(
        plan(&g, &u, &[], &d).unwrap_err().code,
        "offline_route_unresolved"
    );
    let mut g = offline_graph();
    g.offline_engine_configs[0].pair.target = "en".into();
    assert_eq!(
        plan(&g, &u, &[], &d).unwrap_err().code,
        "offline_language_pair_mismatch"
    );
    let mut g = offline_graph();
    g.offline_engine_configs[0].engine.contract_revision = std::num::NonZeroU32::new(2).unwrap();
    assert_eq!(
        plan(&g, &u, &[], &d).unwrap_err().code,
        "unknown_offline_contract"
    );
    let mut g = offline_graph();
    g.bindings[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0) },
    );
    assert_eq!(
        plan(&g, &u, &[], &d).unwrap_err().code,
        "offline_http_field_forbidden"
    );
    let mut g = offline_graph();
    g.bindings[0].engines[0].http_parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0) },
    );
    assert_eq!(
        plan(&g, &u, &[], &d).unwrap_err().code,
        "offline_http_field_forbidden"
    );
}
#[test]
fn legacy_v2_is_explicitly_delegated_and_fixed_reading_selection_is_verified() {
    let mut g = graph();
    g.bindings[0].semantics_version = PurposeSemanticsVersion::LegacyV2;
    assert_eq!(
        plan(
            &g,
            &unit("u", "雨。", 100.),
            &[],
            &directory(&g, CatalogModelIdentity::Required)
        )
        .unwrap_err()
        .code,
        "legacy_dispatch_required"
    );
    let g = graph();
    let mut u = unit("u", "雨。", 100.);
    u.source_hash = content_hash("other");
    assert_eq!(
        plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Required))
            .unwrap_err()
            .code,
        "invalid_reading_identity"
    );
    let u = unit("u", "彼女と雨。", 100.);
    let token = LexicalToken {
        surface: "雨".into(),
        lemma: "雨".into(),
        reading: "あめ".into(),
        part_of_speech: "名詞".into(),
        byte_start: 0,
        byte_end: 3,
        utf16_start: 0,
        utf16_end: 1,
        known: true,
    };
    let d = directory(&g, CatalogModelIdentity::Required);
    assert_eq!(
        compose_service_plan(ServicePlanInput {
            graph: &g,
            expected_revision_id: "r3",
            binding_id: "b",
            engine_id: "e",
            unit: &u,
            selected_token: Some(&token),
            history: &[],
            source_language: "ja",
            target_language: "zh-CN",
            catalog: &d
        })
        .unwrap_err()
        .code,
        "selected_token_mismatch"
    );
}

#[test]
fn language_defaults_and_complete_engine_policy_override_do_not_use_legacy_shadow_maps() {
    let mut g = graph();
    let defaults = g.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap();
    defaults.languages = ServiceLanguageDefaults::Http {
        source: SourceLanguageChoice::Explicit {
            language: "en".into(),
        },
        target: TargetLanguageChoice::Explicit {
            language: "fr".into(),
        },
    };
    g.bindings[0].scope_overrides.execution.timeout_ms = PolicyOverride::Set { value: 45000 };
    g.bindings[0].scope_overrides.execution.concurrency = PolicyOverride::Set { value: 4 };
    g.bindings[0].engines[0]
        .policy_overrides
        .execution
        .timeout_ms = PolicyOverride::Set { value: 23000 };
    g.bindings[0].engines[0]
        .policy_overrides
        .execution
        .concurrency = PolicyOverride::Set { value: 1 };
    g.bindings[0].engines[0].policy_overrides.execution.retries = PolicyOverride::Set { value: 0 };
    g.bindings[0].engines[0].policy_overrides.cache = PolicyOverride::Set {
        value: ServiceCachePolicy { enabled: false },
    };
    let p = plan(
        &g,
        &unit("u", "Source", 100.),
        &[],
        &directory(&g, CatalogModelIdentity::Required),
    )
    .unwrap();
    assert_eq!(
        (
            request(&p).source_language.as_str(),
            request(&p).target_language.as_str()
        ),
        ("en", "fr")
    );
    assert_eq!(
        (
            p.policy.execution.timeout_ms,
            p.policy.execution.concurrency,
            p.policy.execution.retries
        ),
        (23000, 1, 0)
    );
    assert!(!p.policy.cache.enabled);
    assert_eq!(p.provenance["sourceLanguage"].kind, PlanSourceKind::Service);
    assert_eq!(
        p.provenance["execution.timeoutMs"].kind,
        PlanSourceKind::Engine
    );
    g.bindings[0].context.insert("enabled".into(), json!(false));
    assert_eq!(
        plan(
            &g,
            &unit("u", "Source", 100.),
            &[],
            &directory(&g, CatalogModelIdentity::Required)
        )
        .unwrap_err()
        .code,
        "legacy_policy_shadow_forbidden"
    );
}

#[test]
fn graph_change_cannot_mutate_frozen_http_options_or_fallback_revisions() {
    let mut g = graph();
    g.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .fallback = vec![SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "fallback-r1".into(),
            model_id: "other-model".into(),
        },
    }];
    let p = plan(
        &g,
        &unit("u", "雨。", 100.),
        &[],
        &directory(&g, CatalogModelIdentity::Required),
    )
    .unwrap();
    g.connections[0].http_options = Some(HttpServiceOptionsV1::OpenaiCompatible {
        organization_id: Some("changed".into()),
        project_id: None,
    });
    g.models[0].actual_model_id = Some("changed-model".into());
    assert_eq!(request(&p).model.as_deref(), Some("actual-model"));
    assert_eq!(
        p.fallback,
        vec![SavedServiceRef::Http {
            source: HttpConfigRef {
                revision_id: "fallback-r1".into(),
                model_id: "other-model".into()
            }
        }]
    );
    if let ServiceExecutionTarget::Http { plan } = &p.target {
        assert_eq!(
            plan.http_options,
            HttpServiceOptionsV1::OpenaiCompatible {
                organization_id: None,
                project_id: None
            }
        );
    }
}

#[test]
fn selected_token_freezes_the_exact_utf8_and_utf16_selection() {
    let g = graph();
    let u = unit("u", "彼女😀と雨。", 100.);
    let d = directory(&g, CatalogModelIdentity::Required);
    let token = LexicalToken {
        surface: "雨".into(),
        lemma: "雨".into(),
        reading: "あめ".into(),
        part_of_speech: "名詞".into(),
        byte_start: 13,
        byte_end: 16,
        utf16_start: 5,
        utf16_end: 6,
        known: true,
    };
    let p = compose_service_plan(ServicePlanInput {
        graph: &g,
        expected_revision_id: "r3",
        binding_id: "b",
        engine_id: "e",
        unit: &u,
        selected_token: Some(&token),
        history: &[],
        source_language: "ja",
        target_language: "zh-CN",
        catalog: &d,
    })
    .unwrap();
    assert_eq!(envelope(&p)["selectedToken"]["surface"], "雨");
    assert_eq!(envelope(&p)["selectedToken"]["byteStart"], 13);
    assert_eq!(
        p.provenance["selectedToken"].entity_id.as_deref(),
        Some("u")
    );
}

#[test]
fn explicit_scope_languages_override_service_defaults_and_none_returns_to_real_preferences() {
    let mut g = graph();
    let u = unit("u", "Source", 100.);
    let defaults = g.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap();
    defaults.languages = ServiceLanguageDefaults::Http {
        source: SourceLanguageChoice::Explicit {
            language: "en".into(),
        },
        target: TargetLanguageChoice::Explicit {
            language: "fr".into(),
        },
    };
    g.bindings[0].source_language = Some("de".into());
    g.bindings[0].target_language = Some("es".into());
    let p = plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Required)).unwrap();
    assert_eq!(
        (
            request(&p).source_language.as_str(),
            request(&p).target_language.as_str()
        ),
        ("de", "es")
    );
    assert_eq!(p.provenance["sourceLanguage"].kind, PlanSourceKind::Scope);
    assert_eq!(
        p.provenance["targetLanguage"].entity_id.as_deref(),
        Some("b")
    );
    g.bindings[0].source_language = None;
    g.bindings[0].target_language = None;
    let p = plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Required)).unwrap();
    assert_eq!(request(&p).source_language, "en");
    assert_eq!(request(&p).target_language, "fr");
    g.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .languages = ServiceLanguageDefaults::Http {
        source: SourceLanguageChoice::InheritSource {},
        target: TargetLanguageChoice::InheritPreference {},
    };
    let p = plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Required)).unwrap();
    assert_eq!(request(&p).source_language, "ja");
    assert_eq!(request(&p).target_language, "zh-CN");
    g.bindings[0].target_language = Some(" ".into());
    assert_eq!(
        plan(&g, &u, &[], &directory(&g, CatalogModelIdentity::Required))
            .unwrap_err()
            .code,
        "invalid_scope_language"
    );
}

#[test]
fn offline_scope_languages_must_match_the_prepared_pair_without_mutating_the_route() {
    let mut g = offline_graph();
    let u = unit("u", "雨。", 100.);
    let d = offline_directory();
    g.bindings[0].source_language = Some("ja".into());
    g.bindings[0].target_language = Some("zh-CN".into());
    let p = plan(&g, &u, &[], &d).unwrap();
    assert_eq!(p.provenance["targetLanguage"].kind, PlanSourceKind::Scope);
    if let ServiceExecutionTarget::Offline { config, .. } = p.target {
        assert_eq!(
            config.resolution,
            OfflineConfigResolution::Resolved {
                resource_snapshot_id: "snapshot".into(),
                route_id: "route".into()
            }
        );
    }
    g.bindings[0].target_language = Some("en".into());
    assert_eq!(
        plan(&g, &u, &[], &d).unwrap_err().code,
        "offline_language_pair_mismatch"
    );
    g.bindings[0].target_language = Some(String::new());
    assert_eq!(
        plan(&g, &u, &[], &d).unwrap_err().code,
        "invalid_scope_language"
    );
}

fn preflight(g: &ConfigurationV3, d: &ServiceCatalog) -> AppResult<ServiceConfigPreflight> {
    service_config_preflight(
        g,
        &g.bindings[0].engines[0].target,
        CatalogPurpose::Translation,
        d,
    )
}
fn attach_template(g: &mut ConfigurationV3) {
    g.templates.push(TemplateV3 {
        template_id: "template".into(),
        purpose: CatalogPurpose::Translation,
        display_name: "custom".into(),
        source_kind: "custom".into(),
        source_version: Some("1".into()),
        body: BTreeMap::from([
            ("system".into(), json!("Translate {{source}}")),
            ("user".into(), json!("{{source}} {{terms}}")),
        ]),
        variables: vec!["source".into(), "terms".into()],
        compatibility: BTreeMap::new(),
    });
    g.models[0]
        .default_templates
        .insert(CatalogPurpose::Translation, "template".into());
}
fn term_set(scope: &str, game: Option<&str>, id: &str) -> TermSetV3 {
    TermSetV3 {
        term_set_id: id.into(),
        scope_key: scope.into(),
        game_id: game.map(str::to_owned),
        display_name: id.into(),
        enabled: true,
        terms: vec![TermV3 {
            term_id: format!("{id}-term"),
            source_text: Some("雨".into()),
            target_text: Some("雨天".into()),
            source_language: None,
            target_language: None,
            enabled: true,
        }],
    }
}
fn with_option(option: HttpServiceOptionsV1) -> ConfigurationV3 {
    let mut g = graph();
    g.connections[0].protocol = option.protocol_id().into();
    if option.protocol_id() == "google_translate_v3" {
        g.connections[0].endpoint = Some("https://translate.googleapis.com/v3".into());
    }
    g.connections[0]
        .adapter_binding
        .as_mut()
        .unwrap()
        .protocol
        .protocol_id = option.protocol_id().into();
    g.connections[0].http_options = Some(option);
    g.models[0].actual_model_id = None;
    g
}

#[test]
fn static_preflight_exposes_runtime_authorities_without_reading_or_claiming_them() {
    let mut g = graph();
    g.connections[0].auth_mode = "api_key".into();
    g.connections[0].credential_pool_id = Some("reference-not-a-lease".into());
    g.connections[0].credential_pool_revision = std::num::NonZeroU32::new(3);
    let mut d = directory(&g, CatalogModelIdentity::Required);
    for state in [
        CapabilityState::Unknown,
        CapabilityState::Unsupported,
        CapabilityState::Supported,
    ] {
        d.protocols[0].implementation.state = state;
        let result = preflight(&g, &d).unwrap();
        assert_eq!(result.target, g.bindings[0].engines[0].target);
        assert_eq!(
            result.runtime_requirements,
            vec![
                ServicePreflightRequirement::InstalledAdapter,
                ServicePreflightRequirement::EffectiveRequestLanguages,
                ServicePreflightRequirement::CredentialAuthority,
                ServicePreflightRequirement::ModelCapabilities,
                ServicePreflightRequirement::NetworkAvailability
            ]
        );
    }
    g.connections[0].endpoint = None;
    assert_eq!(preflight(&g, &d).unwrap_err().code, "missing_endpoint");
    g.connections[0].endpoint = Some("http://127.0.0.1:1/v1".into());
    g.models[0].actual_model_id = None;
    assert_eq!(preflight(&g, &d).unwrap_err().code, "missing_model");
    g.models[0].actual_model_id = Some("actual".into());
    g.connections[0]
        .adapter_binding
        .as_mut()
        .unwrap()
        .profile
        .profile_revision = 2;
    assert_eq!(
        preflight(&g, &d).unwrap_err().code,
        "unknown_profile_version"
    );
}

#[test]
fn static_http_options_require_exact_choices_routes_and_google_model_resources() {
    for (option, expected) in [
        (
            HttpServiceOptionsV1::AzureTranslator {
                resource_type: None,
                region: None,
            },
            "missing_azure_resource_type",
        ),
        (
            HttpServiceOptionsV1::AzureTranslator {
                resource_type: Some(AzureResourceType::Regional),
                region: None,
            },
            "missing_azure_region",
        ),
        (
            HttpServiceOptionsV1::DeeplV2 { plan: None },
            "missing_deepl_plan",
        ),
        (
            HttpServiceOptionsV1::OpenaiCompatible {
                organization_id: Some(" ".into()),
                project_id: None,
            },
            "invalid_http_options",
        ),
        (
            HttpServiceOptionsV1::GoogleTranslateV3 {
                project_id: "..".into(),
                location: "global".into(),
            },
            "invalid_google_route",
        ),
        (
            HttpServiceOptionsV1::GoogleTranslateV3 {
                project_id: "p".into(),
                location: "a:b".into(),
            },
            "invalid_google_route",
        ),
    ] {
        let g = with_option(option);
        assert_eq!(
            preflight(&g, &directory(&g, CatalogModelIdentity::Optional))
                .unwrap_err()
                .code,
            expected
        );
    }
    let mut g = with_option(HttpServiceOptionsV1::GoogleTranslateV3 {
        project_id: "p".into(),
        location: "global".into(),
    });
    let d = directory(&g, CatalogModelIdentity::Optional);
    assert!(preflight(&g, &d).is_ok()); // Optional means no model is invented.
    for model in [
        "projects/p/locations/global/models/custom",
        "projects/p/locations/global/models/general/nmt",
        "projects/p/locations/global/models/general/translation-llm",
        "projects/p/locations/global/models/translation-llm-custom/custom",
    ] {
        g.models[0].actual_model_id = Some(model.into());
        assert!(preflight(&g, &d).is_ok(), "{model}");
    }
    for model in [
        "custom",
        "projects/other/locations/global/models/custom",
        "projects/p/locations/global/models/general",
        "projects/p/locations/global/models/translation-llm-custom/..",
        "projects/p/locations/global/models/general/nmt/extra",
    ] {
        g.models[0].actual_model_id = Some(model.into());
        assert_eq!(
            preflight(&g, &d).unwrap_err().code,
            "invalid_google_model",
            "{model}"
        );
    }
}

#[test]
fn static_template_and_wire_checks_are_shared_with_effective_scope_composition() {
    let mut g = graph();
    attach_template(&mut g);
    let d = directory(&g, CatalogModelIdentity::Required);
    g.models[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0) },
    );
    assert!(preflight(&g, &d).is_ok());
    g.bindings[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set {
            value: json!(false),
        },
    );
    assert!(preflight(&g, &d).is_ok()); // A target preflight does not claim to validate overrides.
    assert_eq!(
        plan(&g, &unit("u", "雨", 100.), &[], &d).unwrap_err().code,
        "invalid_parameter"
    );
    g.bindings[0].parameters.clear();
    g.models[0].parameters.insert(
        "arbitrary".into(),
        GenerationParameter::Set { value: json!(true) },
    );
    assert_eq!(preflight(&g, &d).unwrap_err().code, "unsupported_parameter");
    g.models[0].parameters.clear();
    g.templates[0]
        .body
        .insert("user".into(), json!("{{unlisted}}"));
    assert!(preflight(&g, &d).is_err());
    g.templates[0]
        .body
        .insert("user".into(), json!("{{source}}"));
    g.templates[0]
        .compatibility
        .insert("sourceLanguages".into(), json!(["en"]));
    assert!(preflight(&g, &d).is_ok()); // Inherited actual language remains unresolved.
    assert_eq!(
        plan(&g, &unit("u", "雨", 100.), &[], &d).unwrap_err().code,
        "template_incompatible"
    );
    g.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .languages = ServiceLanguageDefaults::Http {
        source: SourceLanguageChoice::Explicit {
            language: "ja".into(),
        },
        target: TargetLanguageChoice::Explicit {
            language: "zh-CN".into(),
        },
    };
    assert_eq!(preflight(&g, &d).unwrap_err().code, "template_incompatible");
    let mut traditional = with_option(HttpServiceOptionsV1::DeeplV2 {
        plan: Some(DeeplPlan::Free),
    });
    attach_template(&mut traditional);
    assert_eq!(
        preflight(
            &traditional,
            &directory(&traditional, CatalogModelIdentity::NotApplicable)
        )
        .unwrap_err()
        .code,
        "unsupported_template_protocol"
    );
}

#[test]
fn static_preset_choices_and_material_conditions_never_default_or_assert_unknown_state() {
    let mut g = graph();
    let mut d = directory(&g, CatalogModelIdentity::Required);
    let key = ProviderPresetKey {
        preset_id: "builtin.controlled-preset".into(),
        preset_revision: 1,
    };
    g.connections[0].origin = Some(ConnectionOrigin::Preset {
        key: key.clone(),
        site: None,
    });
    g.connections[0].preset_source = Some(key.clone());
    let mut p: ProviderPresetDescriptor = serde_json::from_value(json!({
        "key": key, "namespace":"builtin", "providerId":"controlled", "brandId":"controlled", "brandLabel":"controlled", "label":"controlled",
        "binding":g.connections[0].adapter_binding, "allowedPurposes":["translation"], "interfaceLabel":"wire", "interfaceDescription":"contract test",
        "interfaceChoiceRequired":false, "defaultAuthKind":"none", "defaultEndpoint":null, "fields":[], "materialBlockers":[], "sourceRefs":[],
        "implementation":{"state":"unknown","reason":"adapter_not_implemented"}, "eligibility":{"state":"unknown","reason":"requires_saved_configuration"}
    })).unwrap();
    p.fields.push(serde_json::from_value(json!({"id":"provider_site","label":"site","fieldType":"select","classification":"required","group":null,
        "condition":{"all":[],"any":[]}, "choices":[{"value":"reviewed","label":"reviewed"}], "defaultValue":"reviewed", "defaultBy":null,
        "defaultEvidence":null,"inheritFrom":null,"requiredWhenVisible":true,"masked":false,"observedOnly":false,"help":null,"sourceRefs":[]})).unwrap());
    d.presets.push(p);
    assert_eq!(
        preflight(&g, &d).unwrap_err().code,
        "required_service_choice_missing"
    );
    d.presets[0].fields[0].condition = ServiceFieldCondition {
        all: vec![ServiceConditionClause {
            field: "unresolved-source".into(),
            operator: ServiceConditionOperator::Eq,
            values: vec![CatalogScalar::Text("dynamic".into())],
        }],
        any: vec![],
    };
    assert!(preflight(&g, &d).unwrap().runtime_requirements.contains(
        &ServicePreflightRequirement::MaterialCondition {
            reason: CatalogReason::ConditionUnresolved
        }
    ));
    d.presets[0].fields[0].condition = ServiceFieldCondition::default();
    if let Some(ConnectionOrigin::Preset { site, .. }) = &mut g.connections[0].origin {
        *site = Some("not-reviewed".into());
    }
    assert_eq!(
        preflight(&g, &d).unwrap_err().code,
        "invalid_service_choice"
    );
    if let Some(ConnectionOrigin::Preset { site, .. }) = &mut g.connections[0].origin {
        *site = Some("reviewed".into());
    }
    d.presets[0]
        .material_blockers
        .push(ConditionalCatalogBlocker {
            condition: ServiceFieldCondition {
                all: vec![ServiceConditionClause {
                    field: "application_oauth_client_mode".into(),
                    operator: ServiceConditionOperator::Eq,
                    values: vec![CatalogScalar::Text("managed".into())],
                }],
                any: vec![],
            },
            reason: CatalogReason::ApplicationOauthSetupUnverified,
            message: "real client authority absent from graph".into(),
            source_refs: vec![],
        });
    assert!(preflight(&g, &d).unwrap().runtime_requirements.contains(
        &ServicePreflightRequirement::MaterialCondition {
            reason: CatalogReason::ApplicationOauthSetupUnverified
        }
    ));
    d.presets[0].material_blockers[0].condition = ServiceFieldCondition::default();
    assert_eq!(
        preflight(&g, &d).unwrap_err().code,
        "application_oauth_setup_unverified"
    );
    g.connections[0].origin = Some(ConnectionOrigin::Custom {
        site: None,
        copied_from: Some(key),
    });
    assert!(preflight(&g, &d).is_ok()); // A copied origin is custom; fixed preset material isn't silently retained.
}

#[test]
fn offline_preflight_preserves_resource_authority_and_never_introduces_http_fields() {
    let mut g = offline_graph();
    let mut d = offline_directory();
    for state in [CapabilityState::Unknown, CapabilityState::Supported] {
        d.offline_engines[0].eligibility.state = state;
        let result = preflight(&g, &d).unwrap();
        assert_eq!(
            result.runtime_requirements,
            vec![
                ServicePreflightRequirement::InstalledAdapter,
                ServicePreflightRequirement::OfflineResourceAuthority
            ]
        );
        let encoded = serde_json::to_value(result).unwrap().to_string();
        assert!(
            !encoded.contains("endpoint")
                && !encoded.contains("modelId")
                && !encoded.contains("credential")
        );
    }
    g.offline_engine_configs[0].installation = None;
    assert_eq!(
        preflight(&g, &d).unwrap_err().code,
        "missing_offline_installation"
    );
}

#[test]
fn purpose_hash_excludes_revision_labels_and_unselected_resource_and_purpose_closures() {
    let mut g = graph();
    attach_template(&mut g);
    let original = purpose_service_semantics_hash(&g, "b").unwrap();
    g.revision_id = "new-storage-revision".into();
    g.connections[0].display_name = "renamed".into();
    g.models[0].display_name = "renamed model".into();
    g.templates[0].display_name = "renamed template".into();
    let mut unused_connection = g.connections[0].clone();
    unused_connection.connection_id = "unused-c".into();
    unused_connection.endpoint = Some("http://127.0.0.1:2/v1".into());
    g.connections.push(unused_connection);
    let mut unused_model = g.models[0].clone();
    unused_model.model_id = "unused-m".into();
    unused_model.connection_id = "unused-c".into();
    unused_model.template_purpose = Some(CatalogPurpose::Grammar);
    unused_model.default_templates.clear();
    let mut grammar = defaults();
    grammar.purpose = CatalogPurpose::Grammar;
    unused_model.request_defaults = BTreeMap::from([(CatalogPurpose::Grammar, grammar)]);
    g.models.push(unused_model);
    let mut unused_template = g.templates[0].clone();
    unused_template.template_id = "unused-t".into();
    unused_template
        .body
        .insert("user".into(), json!("unrelated {{source}}"));
    g.templates.push(unused_template);
    g.models[0]
        .capabilities
        .insert("unrelated_observation".into(), json!({"state":"unknown"}));
    assert_eq!(purpose_service_semantics_hash(&g, "b").unwrap(), original);
    g.connections[0].endpoint = Some("http://127.0.0.1:3/v1".into());
    assert_ne!(purpose_service_semantics_hash(&g, "b").unwrap(), original);
}

#[test]
fn purpose_hash_tracks_effective_templates_parameters_policy_and_field_sources() {
    let mut base = graph();
    attach_template(&mut base);
    base.models[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0.4) },
    );
    let hash = purpose_service_semantics_hash(&base, "b").unwrap();
    let mut changed = base.clone();
    changed.templates[0]
        .body
        .insert("system".into(), json!("Changed {{source}}"));
    assert_ne!(purpose_service_semantics_hash(&changed, "b").unwrap(), hash);
    changed = base.clone();
    changed.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .execution
        .retries = 0;
    assert_ne!(purpose_service_semantics_hash(&changed, "b").unwrap(), hash);
    changed = base.clone();
    changed.bindings[0].scope_overrides.execution.retries = PolicyOverride::Set { value: 2 };
    assert_ne!(purpose_service_semantics_hash(&changed, "b").unwrap(), hash); // Same numeric value, different source.
    changed = base.clone();
    changed.bindings[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0.4) },
    );
    let scope_hash = purpose_service_semantics_hash(&changed, "b").unwrap();
    assert_ne!(scope_hash, hash); // Source identity matters, even when wire value is equal.
    changed.models[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set { value: json!(0.9) },
    );
    assert_eq!(
        purpose_service_semantics_hash(&changed, "b").unwrap(),
        scope_hash
    ); // Shadowed defaults are irrelevant.
    changed.bindings[0].engines[0].http_template_override = Some("template".into());
    assert_ne!(
        purpose_service_semantics_hash(&changed, "b").unwrap(),
        scope_hash
    );
    changed = base.clone();
    changed.models[0].capabilities.remove("streaming");
    assert_ne!(purpose_service_semantics_hash(&changed, "b").unwrap(), hash);
}

#[test]
fn purpose_hash_scope_terms_are_global_plus_only_the_actual_game() {
    let mut g = graph();
    g.term_sets = vec![
        term_set("global", None, "global"),
        term_set("game:a", Some("a"), "a"),
        term_set("game:b", Some("b"), "b"),
    ];
    let global = purpose_service_semantics_hash(&g, "b").unwrap();
    let a = purpose_service_semantics_hash_for_scope(&g, "b", "game:a").unwrap();
    let b = purpose_service_semantics_hash_for_scope(&g, "b", "game:b").unwrap();
    g.term_sets[1].terms[0].target_text = Some("当前游戏术语修改".into());
    assert_eq!(purpose_service_semantics_hash(&g, "b").unwrap(), global);
    assert_ne!(
        purpose_service_semantics_hash_for_scope(&g, "b", "game:a").unwrap(),
        a
    );
    assert_eq!(
        purpose_service_semantics_hash_for_scope(&g, "b", "game:b").unwrap(),
        b
    );
    g.term_sets[1].enabled = false;
    let a_disabled = purpose_service_semantics_hash_for_scope(&g, "b", "game:a").unwrap();
    g.term_sets[1].terms[0].target_text = Some("disabled change".into());
    assert_eq!(
        purpose_service_semantics_hash_for_scope(&g, "b", "game:a").unwrap(),
        a_disabled
    );
    g.term_sets[0].terms[0].target_text = Some("全局术语修改".into());
    assert_ne!(purpose_service_semantics_hash(&g, "b").unwrap(), global);
    assert_ne!(
        purpose_service_semantics_hash_for_scope(&g, "b", "game:b").unwrap(),
        b
    );
    g.bindings[0].scope_key = "game:a".into();
    g.bindings[0].game_id = Some("a".into());
    assert_eq!(
        purpose_service_semantics_hash(&g, "b").unwrap(),
        purpose_service_semantics_hash_for_scope(&g, "b", "game:a").unwrap()
    );
    assert_eq!(
        purpose_service_semantics_hash_for_scope(&g, "b", "game:b")
            .unwrap_err()
            .code,
        "purpose_scope_mismatch"
    );
}

#[test]
fn purpose_hash_offline_pair_route_versions_and_legacy_boundary_are_explicit() {
    let mut g = offline_graph();
    let hash = purpose_service_semantics_hash(&g, "b").unwrap();
    g.revision_id = "new-revision".into();
    g.offline_engine_configs[0].display_name = "renamed".into();
    assert_eq!(purpose_service_semantics_hash(&g, "b").unwrap(), hash);
    let mut changed = g.clone();
    changed.offline_engine_configs[0].pair.target = "en".into();
    assert_ne!(purpose_service_semantics_hash(&changed, "b").unwrap(), hash);
    changed = g.clone();
    changed.offline_engine_configs[0]
        .installation
        .as_mut()
        .unwrap()
        .revision = std::num::NonZeroU32::new(4).unwrap();
    assert_ne!(purpose_service_semantics_hash(&changed, "b").unwrap(), hash);
    changed = g.clone();
    changed.offline_engine_configs[0].resolution = OfflineConfigResolution::Resolved {
        resource_snapshot_id: "new-snapshot".into(),
        route_id: "new-route".into(),
    };
    assert_ne!(purpose_service_semantics_hash(&changed, "b").unwrap(), hash);
    g.bindings[0].semantics_version = PurposeSemanticsVersion::LegacyV2;
    assert_eq!(
        purpose_service_semantics_hash(&g, "b").unwrap_err().code,
        "legacy_dispatch_required"
    );
}

#[test]
fn actual_builtin_target_origin_is_not_claimed_as_a_saved_reading_preference() {
    let mut graph = graph();
    let unit = unit("origin-unit", "雨がやんだ。", 100.);
    let catalog = directory(&graph, CatalogModelIdentity::Required);
    let compose = |graph: &ConfigurationV3, target: &str| {
        compose_service_plan_with_target_origin(
            ServicePlanInput {
                graph,
                expected_revision_id: &graph.revision_id,
                binding_id: "b",
                engine_id: "e",
                unit: &unit,
                selected_token: None,
                history: &[],
                source_language: "ja",
                target_language: target,
                catalog: &catalog,
            },
            TargetLanguageOrigin::BuiltinDefault,
        )
    };
    assert_eq!(
        compose(&graph, "zh-CN").unwrap_err().code,
        "target_language_origin_mismatch"
    );
    let planned = compose(&graph, "zh-Hans").unwrap();
    assert_eq!(request(&planned).target_language, "zh-Hans");
    let source = &planned.provenance["targetLanguage"];
    assert_eq!(source.kind, PlanSourceKind::Builtin);
    assert_eq!(source.field, "defaultTargetLanguageV1");
    assert_eq!(
        (source.entity_id.as_deref(), source.revision_id.as_deref()),
        (None, None)
    );
    graph.bindings[0].target_language = Some("en".into());
    let explicit = compose(&graph, "zh-Hans").unwrap();
    assert_eq!(request(&explicit).target_language, "en");
    assert_eq!(
        explicit.provenance["targetLanguage"].kind,
        PlanSourceKind::Scope
    );
}

#[test]
fn dedicated_translation_materials_are_the_exact_consented_envelope_fields() {
    let graph = graph();
    let current = unit("material-current", "雨がやんだ。", 100.);
    let history = [unit("material-old", "空が明るい。", 20.)];
    let planned = plan(
        &graph,
        &current,
        &history,
        &directory(&graph, CatalogModelIdentity::Required),
    )
    .unwrap();
    let expected = envelope(&planned);
    let ServiceExecutionTarget::Http { plan } = planned.target else {
        panic!("genuine HTTP plan")
    };
    assert_eq!(
        serde_json::to_value(&plan.reference_material.context).unwrap(),
        expected["context"]
    );
    assert_eq!(
        serde_json::to_value(&plan.reference_material.glossary).unwrap(),
        expected["glossary"]
    );
    assert_eq!(plan.reference_material.context.len(), 1);
    assert_eq!(plan.reference_material.context[0]["source"], "空が明るい。");
}
