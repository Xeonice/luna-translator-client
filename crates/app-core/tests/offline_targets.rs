use app_core::{
    contracts::CatalogPurpose,
    domain::model_config::{service_codec::*, service_graph::*},
};
use serde_json::json;
fn graph() -> ConfigurationV3 {
    serde_json::from_value(json!({"revisionId":"offline-r3","connections":[],"models":[],"bindings":[],"templates":[],"termSets":[],"offlineEngineConfigs":[{"id":"o","engine":{"engineId":"argos_translate","contractRevision":1},"backend":"local_worker","displayName":"本機翻訳","pair":{"source":"ja","target":"zh-CN"},"routePolicy":{"kind":"direct_only"},"resolution":{"kind":"draft"},"requestDefaults":{"purpose":"translation","languages":{"kind":"offline_use_config_pair"},"context":{"enabled":false,"maxSentences":0,"mode":"original"},"execution":{"timeoutMs":60000,"concurrency":1,"retries":0},"cache":{"enabled":true},"fallback":[]}}],"codecVersion":3})).unwrap()
}

#[test]
fn unresolved_offline_target_roundtrips_without_fake_http_fields() {
    let graph = graph();
    let bytes = encode_service_graph(&graph).unwrap();
    let decoded = decode_service_graph_import(&bytes).unwrap();
    assert!(decoded.connections.is_empty());
    assert!(decoded.models.is_empty());
    assert!(
        !decoded.offline_engine_configs[0]
            .resolution
            .is_structurally_resolved()
    );
    let text = String::from_utf8(bytes).unwrap();
    for absent in ["apiKey", "endpoint", "modelId", "messages", "nativeLocator"] {
        assert!(!text.contains(absent));
    }
}

#[test]
fn offline_variants_reject_http_secrets_locators_and_duplicate_resource_authority() {
    for (field, value) in [
        ("endpoint", json!("https://example.invalid")),
        ("apiKey", json!("synthetic")),
        ("nativeLocator", json!("/private/path")),
        ("templateId", json!("fake")),
    ] {
        let mut data = serde_json::to_value(graph()).unwrap();
        data["offlineEngineConfigs"][0][field] = value;
        assert!(decode_service_graph_import(&serde_json::to_vec(&data).unwrap()).is_err());
    }
    assert!(
        serde_json::from_value::<EngineTarget>(
            json!({"kind":"offline","offlineConfigId":"o","resourceSnapshotId":"also-authority"})
        )
        .is_err()
    );
    let mut data = serde_json::to_value(graph()).unwrap();
    data["offlineEngineConfigs"][0]["requestDefaults"]["languages"] = json!({"kind":"http","source":{"kind":"inherit_source"},"target":{"kind":"inherit_preference"}});
    assert!(decode_service_graph_import(&serde_json::to_vec(&data).unwrap()).is_err());
}

#[test]
fn offline_engine_can_only_target_translation_and_has_single_resolution_source() {
    let mut graph = graph();
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
            target: EngineTarget::Offline {
                offline_config_id: "o".into(),
            },
            enabled: true,
            http_template_override: None,
            http_parameters: Default::default(),
            policy_overrides: Default::default(),
        }],
        semantics_version: PurposeSemanticsVersion::TypedV3,
        scope_overrides: Default::default(),
    });
    assert!(encode_service_graph(&graph).is_ok());
    graph.bindings[0].purpose = CatalogPurpose::Grammar;
    assert_eq!(
        encode_service_graph(&graph).unwrap_err().code,
        "offline_http_field_forbidden"
    );
    graph.bindings[0].purpose = CatalogPurpose::Translation;
    graph.bindings[0].engines[0].http_template_override = Some("t".into());
    assert_eq!(
        encode_service_graph(&graph).unwrap_err().code,
        "offline_http_field_forbidden"
    );
    graph.bindings[0].engines[0].http_template_override = None;
    graph.offline_engine_configs[0].resolution = OfflineConfigResolution::Resolved {
        resource_snapshot_id: "snapshot".into(),
        route_id: "explicit-route".into(),
    };
    assert!(encode_service_graph(&graph).is_ok());
    assert!(
        graph.offline_engine_configs[0]
            .resolution
            .is_structurally_resolved()
    );
    // Structural resolution is only a claimed reference; no availability or runtime lease is issued.
}
