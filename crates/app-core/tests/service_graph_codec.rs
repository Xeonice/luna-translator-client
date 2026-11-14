use app_core::{
    contracts::{CatalogPurpose, Configuration},
    domain::{
        content_hash,
        model_config::{codec::encode_legacy_configuration, service_codec::*, service_graph::*},
    },
};
use serde_json::{Value, json};

const EMPTY: &str = r#"{"revisionId":"r3","connections":[],"models":[],"bindings":[],"templates":[],"termSets":[],"offlineEngineConfigs":[],"codecVersion":3}"#;
const CONNECTION_GOLDEN: &str = r#"{"revisionId":"r3","connections":[{"connectionId":"c","displayName":"测试","protocol":"openai_compatible","deployment":"cloud","authMode":"none","proxyMode":"inherit","networkMetadata":{},"availabilityIntent":false,"adapterBinding":{"protocol":{"protocolId":"openai_compatible","contractRevision":1},"profile":{"profileId":"openai-chat-text","profileRevision":1}},"httpOptions":{"protocol":"openai_compatible"},"origin":{"kind":"custom"}}],"models":[],"bindings":[],"templates":[],"termSets":[],"offlineEngineConfigs":[],"codecVersion":3}"#;
fn empty() -> ConfigurationV3 {
    decode_service_graph_import(EMPTY.as_bytes()).unwrap()
}
#[test]
fn custom_endpoint_keeps_explicit_provider_site_and_absent_site_keeps_original_bytes() {
    let original = decode_service_graph_import(CONNECTION_GOLDEN.as_bytes()).unwrap();
    assert_eq!(encode_service_graph(&original).unwrap(), CONNECTION_GOLDEN.as_bytes());
    let mut graph = original;
    graph.connections[0].origin = Some(ConnectionOrigin::Custom {
        copied_from: None,
        site: Some("cn-beijing".into()),
    });
    let bytes = encode_service_graph(&graph).unwrap();
    let encoded: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(encoded["connections"][0]["origin"]["site"], "cn-beijing");
    assert_eq!(decode_service_graph_import(&bytes).unwrap(), graph);
    graph.connections[0].origin = Some(ConnectionOrigin::Custom {
        copied_from: None,
        site: Some("invalid\0site".into()),
    });
    assert!(encode_service_graph(&graph).is_err());
}
fn policy() -> PolicyOverrides {
    PolicyOverrides::default()
}
fn defaults() -> ServiceRequestDefaults {
    ServiceRequestDefaults {
        purpose: CatalogPurpose::Translation,
        languages: ServiceLanguageDefaults::Http {
            source: SourceLanguageChoice::InheritSource {},
            target: TargetLanguageChoice::InheritPreference {},
        },
        context: ServiceContextConsent {
            enabled: false,
            max_sentences: 0,
            mode: HistoryInputMode::Original,
        },
        execution: ServiceExecutionDefaults {
            timeout_ms: 60000,
            concurrency: 1,
            retries: 2,
        },
        cache: ServiceCachePolicy { enabled: true },
        fallback: vec![],
    }
}
fn http_graph() -> ConfigurationV3 {
    let mut graph = empty();
    graph.connections.push(serde_json::from_value(json!({"connectionId":"c","displayName":"测试","protocol":"openai_compatible","deployment":"cloud","authMode":"none","proxyMode":"inherit","networkMetadata":{},"availabilityIntent":false,"adapterBinding":{"protocol":{"protocolId":"openai_compatible","contractRevision":1},"profile":{"profileId":"openai-chat-text","profileRevision":1}},"httpOptions":{"protocol":"openai_compatible"},"origin":{"kind":"custom"}})).unwrap());
    graph.models.push(ModelV3 {
        default_templates: Default::default(),
        model_id: "m".into(),
        connection_id: "c".into(),
        actual_model_id: None,
        display_name: "测试模型".into(),
        selection_source: "manual".into(),
        capabilities: Default::default(),
        parameters: Default::default(),
        template_purpose: Some(CatalogPurpose::Translation),
        legacy_eligibility_origin: None,
        request_defaults: [(CatalogPurpose::Translation, defaults())].into(),
    });
    graph.bindings.push(PurposeBindingV3 {
        binding_id: "b".into(),
        scope_key: "global".into(),
        game_id: None,
        purpose: CatalogPurpose::Translation,
        binding_mode: "override".into(),
        enabled: false,
        output_mode: None,
        source_language: None,
        target_language: None,
        template_id: None,
        context: Default::default(),
        schedule: Default::default(),
        parameters: Default::default(),
        engines: vec![EngineV3 {
            engine_id: "e".into(),
            target: EngineTarget::Http {
                model_id: "m".into(),
            },
            enabled: true,
            http_template_override: None,
            http_parameters: Default::default(),
            policy_overrides: policy(),
        }],
        semantics_version: PurposeSemanticsVersion::TypedV3,
        scope_overrides: policy(),
    });
    graph
}

#[test]
fn literal_v3_golden_and_frozen_legacy_empty_grammar_are_distinct() {
    assert_eq!(encode_service_graph(&empty()).unwrap(), EMPTY.as_bytes());
    assert_eq!(
        content_hash(EMPTY),
        "9af0bc03c21c33139ed607c93c9b504b1d8bf64948a6aed18c751b837c24a1bf"
    );
    // Literal pre-existing codec1 field order; not a refreshed serializer oracle.
    let legacy = Configuration {
        revision_id: "r3".into(),
        ..Default::default()
    };
    assert_eq!(encode_legacy_configuration(&legacy).unwrap(),br#"{"revisionId":"r3","connections":[],"models":[],"bindings":[],"templates":[],"termSets":[]}"#);
    assert!(decode_service_graph_import(&encode_legacy_configuration(&legacy).unwrap()).is_err());
    let connection = decode_service_graph_import(CONNECTION_GOLDEN.as_bytes()).unwrap();
    assert_eq!(
        encode_service_graph(&connection).unwrap(),
        CONNECTION_GOLDEN.as_bytes()
    );
}

#[test]
fn canonical_entity_order_does_not_reorder_engines_fallbacks_or_terms() {
    let mut graph = http_graph();
    let mut another = graph.models[0].clone();
    another.model_id = "other".into();
    graph.models.push(another);
    let expected = encode_service_graph(&graph).unwrap();
    graph.models.reverse();
    assert_eq!(encode_service_graph(&graph).unwrap(), expected);
    let mut engine = graph.bindings[0].engines[0].clone();
    engine.engine_id = "other-engine".into();
    engine.target = EngineTarget::Http {
        model_id: "other".into(),
    };
    graph.bindings[0].engines.push(engine);
    let before = encode_service_graph(&graph).unwrap();
    graph.bindings[0].engines.reverse();
    assert_ne!(encode_service_graph(&graph).unwrap(), before);
    let a = SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "old-a".into(),
            model_id: "m".into(),
        },
    };
    let b = SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "old-b".into(),
            model_id: "m".into(),
        },
    };
    graph.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .fallback = vec![a, b];
    let before = encode_service_graph(&graph).unwrap();
    graph.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .fallback
        .reverse();
    assert_ne!(encode_service_graph(&graph).unwrap(), before);
    graph.term_sets.push(serde_json::from_value(json!({"termSetId":"ts","scopeKey":"global","displayName":"用語","enabled":true,"terms":[{"termId":"a","sourceText":"甲","enabled":true},{"termId":"b","sourceText":"乙","enabled":true}]})).unwrap());
    let before = encode_service_graph(&graph).unwrap();
    graph.term_sets[0].terms.reverse();
    assert_ne!(encode_service_graph(&graph).unwrap(), before);
}

#[test]
fn duplicate_json_keys_entities_scopes_and_wrong_codecs_are_rejected() {
    for input in [
        EMPTY.replace("\"codecVersion\":3", "\"codecVersion\":2"),
        EMPTY.replace("\"codecVersion\":3", "\"codecVersion\":4"),
        EMPTY.replacen('{', "{\"revisionId\":\"other\",", 1),
        EMPTY.replace("\"models\":[]", "\"models\":[],\"mystery\":true"),
    ] {
        assert!(decode_service_graph_internal(input.as_bytes()).is_err());
    }
    let mut graph = http_graph();
    graph.models.push(graph.models[0].clone());
    assert_eq!(
        encode_service_graph(&graph).unwrap_err().code,
        "duplicate_service_identity"
    );
    let mut graph = http_graph();
    let mut b = graph.bindings[0].clone();
    b.binding_id = "b2".into();
    graph.bindings.push(b);
    assert_eq!(
        encode_service_graph(&graph).unwrap_err().code,
        "duplicate_service_scope"
    );
    let mut graph = http_graph();
    graph.connections[0]
        .network_metadata
        .insert("x".into(), json!({"key":1}));
    let wire = String::from_utf8(encode_service_graph(&graph).unwrap())
        .unwrap()
        .replace("\"key\":1", "\"key\":1,\"key\":2");
    assert!(decode_service_graph_internal(wire.as_bytes()).is_err());
    let huge = vec![b' '; MAX_SERVICE_GRAPH_BYTES + 1];
    assert_eq!(
        decode_service_graph_internal(&huge).unwrap_err().code,
        "service_graph_too_large"
    );
}

#[test]
fn local_targets_cannot_accept_cross_revision_or_opposite_variant_fields() {
    for value in [
        json!({"kind":"http","source":{"revisionId":"old","modelId":"m"}}),
        json!({"kind":"http","modelId":"m","revisionId":"old"}),
        json!({"kind":"offline","offlineConfigId":"o","modelId":"m"}),
        json!({"kind":"script","script":"none"}),
    ] {
        assert!(serde_json::from_value::<EngineTarget>(value).is_err());
    }
    let mut graph = http_graph();
    graph.bindings[0].engines[0].target = EngineTarget::Http {
        model_id: "missing".into(),
    };
    assert_eq!(
        encode_service_graph(&graph).unwrap_err().code,
        "orphan_http_target"
    );
    let mut value = serde_json::to_value(http_graph()).unwrap();
    value["models"][0]["parameters"] = json!({"temperature":{"mode":"set","value":0,"headers":{}}});
    assert!(decode_service_graph_internal(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn original_opaque_ids_survive_and_import_cannot_self_issue_internal_origin() {
    let mut graph = http_graph();
    let old = "旧 对象 ".repeat(80);
    graph.revision_id = old.clone();
    graph.connections[0].connection_id = old.clone();
    graph.models[0].connection_id = old.clone();
    graph.models[0].model_id = old.clone();
    graph.bindings[0].engines[0].target = EngineTarget::Http {
        model_id: old.clone(),
    };
    assert_eq!(
        decode_service_graph_import(&encode_service_graph(&graph).unwrap())
            .unwrap()
            .models[0]
            .model_id,
        old
    );
    graph.models[0].template_purpose = None;
    graph.models[0].legacy_eligibility_origin = Some(HttpConfigRef {
        revision_id: "真实旧版".into(),
        model_id: old,
    });
    graph.bindings[0].semantics_version = PurposeSemanticsVersion::LegacyV2;
    let bytes = encode_service_graph(&graph).unwrap();
    assert!(decode_service_graph_internal(&bytes).is_ok());
    assert_eq!(
        decode_service_graph_import(&bytes).unwrap_err().code,
        "internal_service_origin_not_importable"
    );
    let mut graph = http_graph();
    graph.models[0].model_id = "bad\0id".into();
    assert!(encode_service_graph(&graph).is_err());
}

#[test]
fn inherit_set_false_zero_and_precedence_have_distinct_intent_and_provenance() {
    let d = defaults();
    let mut scope = policy();
    scope.cache = PolicyOverride::Set {
        value: ServiceCachePolicy { enabled: false },
    };
    scope.execution.retries = PolicyOverride::Set { value: 0 };
    let mut engine = policy();
    engine.execution.timeout_ms = PolicyOverride::Set { value: 1000 };
    let effective = effective_policy(&d, &scope, &engine);
    assert!(!effective.cache.enabled);
    assert_eq!(effective.execution.retries, 0);
    assert_eq!(effective.execution.timeout_ms, 1000);
    assert_eq!(effective.sources["cache"], PolicySource::Scope);
    assert_eq!(effective.sources["timeoutMs"], PolicySource::Engine);
    assert_eq!(effective.sources["concurrency"], PolicySource::Service);
    assert_ne!(
        serde_json::to_vec(&scope).unwrap(),
        serde_json::to_vec(&policy()).unwrap()
    );
    engine.cache = PolicyOverride::Set {
        value: ServiceCachePolicy { enabled: true },
    };
    assert!(effective_policy(&d, &scope, &engine).cache.enabled);
    assert!(
        serde_json::from_value::<PolicyOverride<bool>>(json!({"mode":"inherit","value":false}))
            .is_err()
    );
    let mut graph = http_graph();
    graph.bindings[0].engines[0].policy_overrides = engine;
    let bytes = encode_service_graph(&graph).unwrap();
    assert_eq!(
        encode_service_graph(&decode_service_graph_internal(&bytes).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn http_auth_options_and_template_purpose_are_independent_strict_contracts() {
    let mut graph = http_graph();
    graph.connections[0].http_options = Some(HttpServiceOptionsV1::GoogleTranslateV2 {});
    assert_eq!(
        encode_service_graph(&graph).unwrap_err().code,
        "http_options_protocol_mismatch"
    );
    let mut graph = http_graph();
    graph.connections[0].credential_pool_id = Some("p".into());
    assert_eq!(
        encode_service_graph(&graph).unwrap_err().code,
        "incomplete_pool_reference"
    );
    let mut graph = http_graph();
    graph.models[0].template_purpose = Some(CatalogPurpose::Grammar);
    assert!(encode_service_graph(&graph).is_err());
    for options in [
        json!({"protocol":"openai_compatible","headers":{}}),
        json!({"protocol":"google_translate_v3","projectId":"p","location":"l","tokenUri":"https://example.invalid"}),
    ] {
        assert!(serde_json::from_value::<HttpServiceOptionsV1>(options).is_err());
    }
    let value: Value = serde_json::from_str(EMPTY).unwrap();
    assert!(value.get("offlineEngineConfigs").is_some());
}

#[test]
fn empty_tagged_variants_never_discard_unknown_payloads() {
    assert!(
        serde_json::from_value::<GenerationParameter>(
            json!({"mode":"omit","value":"must-not-ignore"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<GenerationParameter>(json!({"mode":"inherit","value":false}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<SourceLanguageChoice>(
            json!({"kind":"inherit_source","language":"ja"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<TargetLanguageChoice>(
            json!({"kind":"inherit_preference","language":"zh-CN"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<ServiceLanguageDefaults>(
            json!({"kind":"offline_use_config_pair","source":"ja"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<OfflineRoutePolicy>(
            json!({"kind":"direct_only","consentRevision":1})
        )
        .is_err()
    );
}

#[test]
fn typed_policy_rejects_legacy_opt_out_shadow_and_legacy_maps_remain_lossless() {
    let mut graph = http_graph();
    graph.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .context = ServiceContextConsent {
        enabled: true,
        max_sentences: 10,
        mode: HistoryInputMode::Original,
    };
    graph.bindings[0].context =
        serde_json::from_value(json!({"enabled":false,"maxSentences":0,"historyMode":"original"}))
            .unwrap();
    graph.bindings[0].schedule =
        serde_json::from_value(json!({"timeoutMs":1000,"retries":0})).unwrap();
    let input = serde_json::to_vec(&graph).unwrap();
    assert_eq!(
        decode_service_graph_import(&input).unwrap_err().code,
        "legacy_policy_shadow_forbidden"
    );
    let original_context = graph.bindings[0].context.clone();
    let original_schedule = graph.bindings[0].schedule.clone();
    graph.bindings[0].semantics_version = PurposeSemanticsVersion::LegacyV2;
    let bytes = encode_service_graph(&graph).unwrap();
    let decoded = decode_service_graph_internal(&bytes).unwrap();
    assert_eq!(decoded.bindings[0].context, original_context);
    assert_eq!(decoded.bindings[0].schedule, original_schedule);
    assert_eq!(encode_service_graph(&decoded).unwrap(), bytes);
    assert_eq!(
        decode_service_graph_import(&bytes).unwrap_err().code,
        "internal_service_origin_not_importable"
    );
    for (wire, expected) in [
        ("original", HistoryInputMode::Original),
        ("translation", HistoryInputMode::Translation),
        ("both", HistoryInputMode::Both),
    ] {
        assert_eq!(
            serde_json::from_str::<HistoryInputMode>(&format!("\"{wire}\"")).unwrap(),
            expected
        );
        assert_eq!(
            serde_json::to_string(&expected).unwrap(),
            format!("\"{wire}\"")
        );
    }
}

#[test]
fn term_scope_label_and_text_integrity_are_checked_before_import() {
    let mut graph = http_graph();
    graph.term_sets.push(serde_json::from_value(json!({"termSetId":"ts","scopeKey":"global","displayName":"用語","enabled":true,"terms":[{"termId":"a","sourceText":"言葉","enabled":true}]})).unwrap());
    let mut wrong = graph.clone();
    wrong.term_sets[0].game_id = Some("game".into());
    assert_eq!(
        encode_service_graph(&wrong).unwrap_err().code,
        "invalid_service_scope"
    );
    let mut wrong = graph.clone();
    wrong.term_sets[0].display_name = "bad\0label".into();
    assert!(encode_service_graph(&wrong).is_err());
    let mut wrong = graph.clone();
    wrong.term_sets[0].terms[0].source_text = Some("bad\0text".into());
    assert_eq!(
        encode_service_graph(&wrong).unwrap_err().code,
        "invalid_service_text"
    );
    graph.term_sets[0].game_id = Some("旧 ゲーム".into());
    graph.term_sets[0].scope_key = "game:旧 ゲーム".into();
    assert!(encode_service_graph(&graph).is_ok());
}

#[test]
fn deployment_domain_preserves_frozen_sql_local_remote_cloud() {
    for deployment in ["local", "remote", "cloud"] {
        let mut graph = http_graph();
        graph.connections[0].deployment = deployment.into();
        let bytes = encode_service_graph(&graph).unwrap();
        assert_eq!(
            decode_service_graph_import(&bytes).unwrap().connections[0].deployment,
            deployment
        );
    }
    let mut graph = http_graph();
    graph.connections[0].deployment = "lan".into();
    assert_eq!(
        encode_service_graph(&graph).unwrap_err().code,
        "invalid_connection_policy"
    );
}

#[test]
fn schema3_source_shapes_remain_representable_without_granting_import_or_execution() {
    for protocol in ["unsupported", "baidu_vip", "openai_responses"] {
        let mut graph = http_graph();
        let connection = &mut graph.connections[0];
        connection.protocol = protocol.into();
        connection.origin = None;
        connection.http_options = None;
        connection.materialization_source = Some(ConnectionSourceRef {
            revision_id: "old-schema3".into(),
            connection_id: "old-c".into(),
        });
        if protocol == "unsupported" {
            connection.adapter_binding = None;
        } else {
            connection
                .adapter_binding
                .as_mut()
                .unwrap()
                .protocol
                .protocol_id = protocol.into();
        }
        let bytes = encode_service_graph(&graph).unwrap();
        assert!(decode_service_graph_internal(&bytes).is_ok());
        assert_eq!(
            decode_service_graph_import(&bytes).unwrap_err().code,
            "internal_service_origin_not_importable"
        );
    }
}

#[test]
fn missing_required_service_choices_are_saved_as_drafts_not_guessed_from_first_option() {
    for (protocol, profile, options) in [
        (
            "azure_translator",
            "azure-translator-v3",
            json!({"protocol":"azure_translator"}),
        ),
        ("deepl_v2", "deepl-v2", json!({"protocol":"deepl_v2"})),
    ] {
        let mut graph = http_graph();
        let c = &mut graph.connections[0];
        c.protocol = protocol.into();
        let binding = c.adapter_binding.as_mut().unwrap();
        binding.protocol.protocol_id = protocol.into();
        binding.profile.profile_id = profile.into();
        c.http_options = Some(serde_json::from_value(options.clone()).unwrap());
        let bytes = encode_service_graph(&graph).unwrap();
        let decoded = decode_service_graph_import(&bytes).unwrap();
        assert_eq!(
            serde_json::to_value(decoded.connections[0].http_options.as_ref().unwrap()).unwrap(),
            options
        );
        // This only records incomplete user intent. No credential/eligibility/execution lease is produced.
    }
    assert!(
        serde_json::from_value::<HttpServiceOptionsV1>(
            json!({"protocol":"deepl_v2","plan":"unknown"})
        )
        .is_err()
    );
}
