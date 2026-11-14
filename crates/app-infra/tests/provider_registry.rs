//! Registry contracts exercise real Rust composition and rejection paths without network or secrets.
use app_core::{contracts::*, domain::model_config::catalog};
use app_infra::providers::{presets, profiles, register, registry::ProviderRegistryBuilder};
use std::collections::BTreeMap;

fn user() -> UserProviderPresetInput {
    UserProviderPresetInput {
        schema_version: 1,
        key: ProviderPresetKey {
            preset_id: "user.my-server".into(),
            preset_revision: 1,
        },
        provider_id: "user.my-provider".into(),
        provider_label: "私有服务提供者".into(),
        display_name: "私有日文服务".into(),
        binding: AdapterBinding {
            protocol: ProtocolKey {
                protocol_id: "openai_compatible".into(),
                contract_revision: 1,
            },
            profile: CompatibilityProfileKey {
                profile_id: "deepseek-text".into(),
                profile_revision: 1,
            },
        },
        endpoint: Some("http://127.0.0.1:8181/v1".into()),
        auth_kind: CatalogAuthKind::None,
        options: BTreeMap::from([("temperature".into(), CatalogScalar::Number(0.0))]),
    }
}
fn builder() -> ProviderRegistryBuilder {
    let mut b = ProviderRegistryBuilder::new();
    for s in register::builtin_sources() {
        b.register_source(s).unwrap();
    }
    for p in register::builtin_protocols() {
        b.register_protocol(p).unwrap();
    }
    for p in profiles::builtin_profiles() {
        b.register_profile(p).unwrap();
    }
    for p in presets::builtin_presets() {
        b.register_builtin_preset(p).unwrap();
    }
    for p in presets::builtin_offline_engines() {
        b.register_offline_engine(p).unwrap();
    }
    b
}
#[test]
fn complete_catalog_matches_independent_approved_ids_and_every_field_shape() {
    let registry = register::builtin_registry().unwrap();
    let catalog = registry.catalog();
    let design: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/architecture/model-service-registry-design.json"
    ))
    .unwrap();
    let presentation: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/architecture/service-configuration-presentation.json"
    ))
    .unwrap();
    assert_eq!(catalog.protocols.len(), 13);
    assert_eq!(catalog.profiles.len(), 21);
    assert_eq!(catalog.presets.len(), 21);
    assert_eq!(catalog.offline_engines.len(), 4);
    for p in design["protocols"].as_array().unwrap() {
        let actual = catalog
            .protocols
            .iter()
            .find(|x| x.key.protocol_id == p["id"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            actual.key.contract_revision,
            p["contract_revision"].as_u64().unwrap() as u32
        );
    }
    for s in presentation["services"].as_array().unwrap() {
        let id = s["id"].as_str().unwrap();
        let fields = if s["kind"] == "http" {
            &catalog
                .presets
                .iter()
                .find(|x| x.key.preset_id == id)
                .unwrap()
                .fields
        } else {
            &catalog
                .offline_engines
                .iter()
                .find(|x| x.engine_id == id)
                .unwrap()
                .fields
        };
        let original_len = s["fields"].as_array().unwrap().len();
        let (original_fields, delta) = fields.split_at(original_len);
        assert_eq!(
            original_fields
                .iter()
                .map(|f| f.id.as_str())
                .collect::<Vec<_>>(),
            s["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f["id"].as_str().unwrap())
                .collect::<Vec<_>>()
        );
        // Preserve the complete original inventory and its field assertions.
        // Only the separately reviewed two-field HTTP suffix is additional;
        // arbitrary extra fields remain an error and offline gets no suffix.
        let expected_delta: &[&str] = if s["kind"] == "http" {
            &["proxy_mode", "proxy_url"]
        } else {
            &[]
        };
        assert_eq!(
            delta
                .iter()
                .map(|field| field.id.as_str())
                .collect::<Vec<_>>(),
            expected_delta,
            "{id}"
        );
        for expected in s["fields"].as_array().unwrap() {
            let actual = fields
                .iter()
                .find(|f| f.id == expected["id"].as_str().unwrap())
                .unwrap();
            assert_eq!(actual.label, expected["label"].as_str().unwrap());
            assert_eq!(
                serde_json::to_value(actual.field_type).unwrap(),
                expected["type"]
            );
            let class = serde_json::to_value(actual.classification).unwrap();
            assert_eq!(
                class
                    .as_str()
                    .unwrap()
                    .replace("not_applicable", "notapplicable"),
                expected["classification"].as_str().unwrap()
            );
        }
    }
    assert!(
        catalog
            .protocols
            .iter()
            .all(|p| !p.key.protocol_id.contains("tmt"))
    );
}
#[test]
fn catalog_separates_source_implementation_from_saved_service_eligibility() {
    let c = register::builtin_registry().unwrap().catalog();
    assert_eq!(
        c.protocols
            .iter()
            .filter(|p| p.implementation.state == CapabilityState::Supported)
            .count(),
        3
    );
    assert!(c.presets.iter().all(|p| p.eligibility
        == CatalogDecision {
            state: CapabilityState::Unknown,
            reason: Some(CatalogReason::RequiresSavedConfiguration)
        }));
    for p in c
        .protocols
        .iter()
        .filter(|p| p.implementation.state != CapabilityState::Supported)
    {
        assert!(
            p.capabilities
                .iter()
                .all(|cap| cap.support.state != CapabilityState::Supported)
        );
        assert_eq!(
            p.implementation.reason,
            Some(CatalogReason::AdapterNotImplemented)
        );
    }
    assert!(
        c.offline_engines
            .iter()
            .all(|p| p.implementation.state == CapabilityState::Unsupported
                && p.eligibility.state == CapabilityState::Unknown)
    );
}
#[test]
fn exact_versions_coexist_and_never_fall_back_across_profile_or_protocol() {
    let mut b = builder();
    let mut protocol = register::builtin_protocols().remove(0);
    protocol.key.contract_revision = 2;
    b.register_protocol(protocol.clone()).unwrap();
    let mut profile = profiles::builtin_profiles()
        .into_iter()
        .find(|p| p.protocol.protocol_id == protocol.key.protocol_id)
        .unwrap();
    let old = profile.key.clone();
    profile.key.profile_revision = 2;
    profile.protocol = protocol.key.clone();
    b.register_profile(profile.clone()).unwrap();
    let r = b.freeze().unwrap();
    let matching = AdapterBinding {
        protocol: protocol.key.clone(),
        profile: profile.key,
    };
    assert_eq!(
        r.resolve(&matching).unwrap().protocol.key.contract_revision,
        2
    );
    assert_eq!(
        r.resolve(&AdapterBinding {
            protocol: protocol.key,
            profile: old
        })
        .unwrap_err()
        .code,
        "profile_protocol_mismatch"
    );
    let mut missing = matching;
    missing.protocol.contract_revision = 77;
    assert_eq!(
        r.resolve(&missing).unwrap_err().code,
        "unknown_protocol_version"
    );
}
#[test]
fn duplicates_reject_without_replacing_first_registered_objects() {
    let mut b = builder();
    let mut p = register::builtin_protocols().remove(0);
    let original = p.label.clone();
    p.label = "覆盖的名字".into();
    assert_eq!(
        b.register_protocol(p.clone()).unwrap_err().code,
        "duplicate_protocol_version"
    );
    let r = b.freeze().unwrap();
    assert_eq!(
        r.catalog()
            .protocols
            .iter()
            .find(|x| x.key == p.key)
            .unwrap()
            .label,
        original
    );
    let mut b = builder();
    assert_eq!(
        b.register_builtin_preset(presets::builtin_presets().remove(0))
            .unwrap_err()
            .code,
        "duplicate_preset_version"
    );
}
#[test]
fn profiles_cannot_elevate_traditional_purposes_or_authentication() {
    let mut b = builder();
    let mut p = profiles::builtin_profiles()
        .into_iter()
        .find(|p| p.protocol.protocol_id == "azure_translator")
        .unwrap();
    p.key.profile_revision = 2;
    p.allowed_purposes.push(CatalogPurpose::Grammar);
    assert_eq!(
        b.register_profile(p).unwrap_err().code,
        "profile_protocol_mismatch"
    );
    let mut p = profiles::builtin_profiles()
        .into_iter()
        .find(|p| p.protocol.protocol_id == "anthropic")
        .unwrap();
    p.key.profile_revision = 2;
    p.auth_kinds.push(CatalogAuthKind::GoogleOauth);
    assert_eq!(
        b.register_profile(p).unwrap_err().code,
        "profile_protocol_mismatch"
    );
}
#[test]
fn user_preset_import_is_strict_versioned_and_cannot_overwrite_builtin_or_transfer_authority() {
    let r = register::builtin_registry().unwrap();
    let input = user();
    let r2 = r.with_user_preset(input.clone()).unwrap();
    assert_eq!(r.catalog().presets.len(), 21);
    assert_eq!(r2.catalog().presets.len(), 22);
    let p = r2.preset(&input.key).unwrap();
    assert_eq!(p.namespace, CatalogNamespace::User);
    assert_eq!(p.default_endpoint, input.endpoint);
    assert_eq!(p.eligibility.state, CapabilityState::Unknown);
    assert_eq!(
        r2.with_user_preset(input.clone()).unwrap_err().code,
        "duplicate_preset_version"
    );
    let mut next = input.clone();
    next.key.preset_revision = 2;
    assert_eq!(
        r2.with_user_preset(next).unwrap().catalog().presets.len(),
        23
    );
    let mut bad = input.clone();
    bad.key.preset_id = "builtin.deepseek".into();
    assert_eq!(
        r.with_user_preset(bad).unwrap_err().code,
        "invalid_preset_namespace"
    );
    let mut bad = input;
    bad.binding.profile.profile_revision = 99;
    assert_eq!(
        r.with_user_preset(bad).unwrap_err().code,
        "unknown_profile_version"
    );
}
#[test]
fn imports_reject_unknown_fields_scripts_headers_and_secret_values() {
    let value = serde_json::to_value(user()).unwrap();
    for key in [
        "secret",
        "headers",
        "script",
        "vaultRef",
        "credentialPoolId",
    ] {
        let mut bad = value.clone();
        bad.as_object_mut()
            .unwrap()
            .insert(key.into(), serde_json::json!("sensitive-material"));
        assert!(serde_json::from_value::<UserProviderPresetInput>(bad).is_err());
    }
    let mut bad = value;
    bad["binding"]["protocol"]["headers"] = serde_json::json!({"Authorization":"secret"});
    assert!(serde_json::from_value::<UserProviderPresetInput>(bad).is_err());
    let r = register::builtin_registry().unwrap();
    for key in ["api_key", "headers", "unknown", "script", "credential_ref"] {
        let mut input = user();
        input
            .options
            .insert(key.into(), CatalogScalar::Text("not-allowed".into()));
        assert!(r.with_user_preset(input).is_err());
    }
}
#[test]
fn imported_endpoints_and_option_types_do_not_bypass_profile_schema() {
    let r = register::builtin_registry().unwrap();
    for url in [
        "https://user:password@example.com/v1",
        "https://example.com/v1?api_key=secret",
        "https://example.com/v1#secret",
        "file:///tmp/server",
        "https://example.com/\nsecret",
    ] {
        let mut input = user();
        input.endpoint = Some(url.into());
        assert!(r.with_user_preset(input).is_err());
    }
    let mut input = user();
    input
        .options
        .insert("temperature".into(), CatalogScalar::Text("0".into()));
    assert_eq!(
        r.with_user_preset(input).unwrap_err().code,
        "invalid_preset_option_type"
    );
    let mut input = user();
    input
        .options
        .insert("temperature".into(), CatalogScalar::Number(f64::NAN));
    assert_eq!(
        r.with_user_preset(input).unwrap_err().code,
        "protected_preset_option"
    );
}
#[test]
fn unknown_conditions_and_unselected_site_defaults_are_not_false_or_first_choice() {
    let c = register::builtin_registry().unwrap().catalog();
    let qwen = c
        .presets
        .iter()
        .find(|p| p.key.preset_id == "builtin.qwen")
        .unwrap();
    let endpoint = qwen.fields.iter().find(|f| f.id == "endpoint").unwrap();
    let mut values = BTreeMap::new();
    assert_eq!(
        catalog::evaluate_condition(&endpoint.condition, &values),
        CapabilityState::Unknown
    );
    assert!(catalog::resolve_field_default(endpoint, &values).is_none());
    values.insert(
        "connection_mode".into(),
        CatalogScalar::Text("custom".into()),
    );
    assert_eq!(
        catalog::evaluate_condition(&endpoint.condition, &values),
        CapabilityState::Supported
    );
    values.insert("provider_site".into(), CatalogScalar::Text("other".into()));
    assert!(catalog::resolve_field_default(endpoint, &values).is_none());
    values.insert(
        "provider_site".into(),
        CatalogScalar::Text("cn-beijing".into()),
    );
    assert_eq!(
        catalog::resolve_field_default(endpoint, &values),
        Some(&CatalogScalar::Text(
            "https://dashscope.aliyuncs.com/compatible-mode/v1".into()
        ))
    );
    assert_eq!(
        qwen.material_blockers[0].reason,
        CatalogReason::PresetSiteMaterialIncomplete
    );
}
#[test]
fn offline_unknown_package_observation_does_not_turn_into_missing_package_false() {
    let c = register::builtin_registry().unwrap().catalog();
    let argos = c
        .offline_engines
        .iter()
        .find(|e| e.engine_id == "argos_translate")
        .unwrap();
    let package = argos
        .fields
        .iter()
        .find(|f| f.id == "language_package")
        .unwrap();
    let mut values = BTreeMap::new();
    assert_eq!(
        catalog::evaluate_condition(&package.condition, &values),
        CapabilityState::Unknown
    );
    values.insert("package_detected".into(), CatalogScalar::Boolean(false));
    assert_eq!(
        catalog::evaluate_condition(&package.condition, &values),
        CapabilityState::Supported
    );
    values.insert("package_detected".into(), CatalogScalar::Boolean(true));
    assert_eq!(
        catalog::evaluate_condition(&package.condition, &values),
        CapabilityState::Unsupported
    );
}
#[test]
fn freezing_rejects_unresolved_sources_and_does_not_publish_live_mutable_state() {
    let mut b = ProviderRegistryBuilder::new();
    b.register_protocol(register::builtin_protocols().remove(0))
        .unwrap();
    assert_eq!(b.freeze().unwrap_err().code, "unknown_catalog_source");
    let r = register::builtin_registry().unwrap();
    let mut projection = r.catalog();
    projection.presets[0].label = "窜改".into();
    projection.presets.clear();
    assert_eq!(r.catalog().presets.len(), 21);
}
#[test]
fn metadata_rejects_zero_versions_control_ids_secret_defaults_and_unimplemented_support() {
    let mut b = ProviderRegistryBuilder::new();
    let mut p = register::builtin_protocols().remove(0);
    p.key.contract_revision = 0;
    assert_eq!(
        b.register_protocol(p).unwrap_err().code,
        "invalid_catalog_revision"
    );
    let mut p = register::builtin_protocols().remove(0);
    p.key.protocol_id = "bad\0id".into();
    assert_eq!(
        b.register_protocol(p).unwrap_err().code,
        "invalid_catalog_id"
    );
    let mut p = register::builtin_protocols()
        .into_iter()
        .find(|p| p.implementation.state == CapabilityState::Unsupported)
        .unwrap();
    p.capabilities[0].support.state = CapabilityState::Supported;
    assert_eq!(
        b.register_protocol(p).unwrap_err().code,
        "unimplemented_protocol_capability"
    );
    let mut field = presets::builtin_presets()[0]
        .fields
        .iter()
        .find(|f| f.field_type == ServiceFieldType::Secret)
        .unwrap()
        .clone();
    field.default_value = Some(CatalogScalar::Text("must-never-project".into()));
    assert_eq!(
        catalog::validate_fields(&[field]).unwrap_err().code,
        "secret_catalog_default"
    );
}

#[test]
fn brand_projection_is_rust_owned_and_conflicting_labels_are_rejected() {
    let r = register::builtin_registry().unwrap();
    let c = r.catalog();
    assert_eq!(c.brands.len(), 19);
    assert_eq!(
        c.brands
            .iter()
            .find(|b| b.brand_id == "gemini")
            .unwrap()
            .label,
        "Gemini"
    );
    let input = user();
    let r = r.with_user_preset(input.clone()).unwrap();
    let mut changed = input;
    changed.key.preset_revision = 2;
    changed.provider_label = "冲突的品牌名".into();
    assert_eq!(
        r.with_user_preset(changed).unwrap_err().code,
        "catalog_brand_conflict"
    );
}
#[test]
fn custom_import_never_inherits_an_auth_or_endpoint_default_from_builtin() {
    let r = register::builtin_registry().unwrap();
    let mut input = user();
    input.endpoint = None;
    let r = r.with_user_preset(input.clone()).unwrap();
    let p = r.preset(&input.key).unwrap();
    assert!(p.default_endpoint.is_none());
    assert!(
        p.fields
            .iter()
            .find(|f| f.id == "endpoint")
            .unwrap()
            .default_value
            .is_none()
    );
    assert_eq!(
        p.fields
            .iter()
            .find(|f| f.id == "auth_mode")
            .unwrap()
            .default_value,
        Some(CatalogScalar::Text("none".into()))
    );
    assert_eq!(
        p.fields
            .iter()
            .find(|f| f.id == "connection_mode")
            .unwrap()
            .default_value,
        Some(CatalogScalar::Text("custom".into()))
    );
}
