use app_core::contracts::*;
use app_infra::providers::register;
use std::collections::BTreeMap;
fn request(id: &str, revision: u32, offline: bool) -> ServiceFormRequest {
    ServiceFormRequest {
        target: if offline {
            ServiceFormTarget::Offline {
                engine_id: id.into(),
                contract_revision: revision,
            }
        } else {
            ServiceFormTarget::Http {
                preset: ProviderPresetKey {
                    preset_id: id.into(),
                    preset_revision: revision,
                },
            }
        },
        purpose: CatalogPurpose::Translation,
        values: BTreeMap::new(),
    }
}
// Independent extension of the frozen 546-row source. The approved JSON is
// unchanged; this exact two-row suffix is reviewed separately for each HTTP preset.
fn network_delta() -> Vec<ServiceFieldDescriptor> {
    let base = |id: &str, label: &str, field_type, classification| ServiceFieldDescriptor {
        id: id.into(), label: label.into(), field_type, classification,
        group: Some(ServiceFieldGroup::Advanced), condition: Default::default(), choices: vec![],
        default_value: None, default_by: None, default_evidence: None, inherit_from: None,
        required_when_visible: false, masked: false, observed_only: false, help: None, source_refs: vec!["prd".into()],
    };
    let mut mode = base("proxy_mode", "网络连接方式", ServiceFieldType::Select, ServiceFieldClassification::Advanced);
    mode.choices = [("inherit", "继承系统代理"), ("disabled", "直接连接"), ("override", "此服务独立代理")].into_iter().map(|(value,label)|ServiceFieldChoice{value:CatalogScalar::Text(value.into()),label:label.into()}).collect();
    mode.default_value = Some(CatalogScalar::Text("inherit".into()));
    mode.help = Some("仅用于当前服务连接；登录、测试和翻译共用此网络策略。".into());
    let mut url = base("proxy_url", "独立代理地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional);
    url.condition.all.push(ServiceConditionClause { field: "proxy_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("override".into())] });
    url.required_when_visible = true;
    url.help = Some("填写完整HTTP(S)代理地址；不能包含账号、密码、路径、查询参数或片段。".into());
    vec![mode, url]
}
fn frozen_descriptor(value: &serde_json::Value) -> ServiceFieldDescriptor {
    use serde_json::{Value, json};
    let clauses = |name: &str| value["condition"][name].as_array().map(|rows| rows.iter().map(|row| json!({
        "field":row["field"], "operator":row["op"], "values":if row["op"]=="in" { row["value"].clone() } else { json!([row["value"]]) }
    })).collect::<Vec<_>>()).unwrap_or_default();
    let by = value.get("default_by").map(|by|json!({"field":by["field"],"values":by["values"].as_object().unwrap().iter().map(|(key,v)|json!({"selectorValue":key,"value":v})).collect::<Vec<_>>()}));
    serde_json::from_value(json!({
        "id":value["id"],"label":value["label"],"fieldType":value["type"],
        "classification":if value["classification"]=="notapplicable" { json!("not_applicable") } else { value["classification"].clone() },
        "group":value.get("ui_group").unwrap_or(&Value::Null),"condition":{"all":clauses("all"),"any":clauses("any")},
        "choices":value.get("choices").cloned().unwrap_or_else(||json!([])),"defaultValue":value.get("default_value"),"defaultBy":by,
        "defaultEvidence":value.get("default_status"),"inheritFrom":value.get("inherit_from").and_then(Value::as_str).map(|s|s.strip_prefix("purpose.").unwrap_or(s)),
        "requiredWhenVisible":value["required_when_visible"].as_bool().unwrap_or(false),"masked":value["masked"].as_bool().unwrap_or(false),"observedOnly":value["observed_only"].as_bool().unwrap_or(false),
        "help":value.get("help"),"sourceRefs":value["source_refs"]
    })).unwrap()
}
fn normalized(mut value: ServiceFieldDescriptor) -> ServiceFieldDescriptor {
    if let Some(by)=value.default_by.as_mut() { by.values.sort_by_key(|row|serde_json::to_string(&row.selector_value).unwrap()); }
    value
}
#[test]
fn credential_capture_matches_the_same_auth_interpretation_used_by_real_draft_compilation() {
    use app_core::domain::model_config::service_draft::{compile_service_draft, PreparedServiceDraftResources};
    let registry = register::builtin_registry().unwrap();
    for (id, values, capture, auth) in [
        ("builtin.deepseek", vec![], ServiceCredentialCapture::ApiKey, "api_key"),
        ("builtin.baidu", vec![], ServiceCredentialCapture::BaiduAppSecret, "baidu_app_secret"),
        ("builtin.youdao", vec![], ServiceCredentialCapture::YoudaoAppSecret, "youdao_app_secret"),
        ("builtin.ollama", vec![], ServiceCredentialCapture::None, "none"),
        ("builtin.ollama", vec![("connection_mode", "custom"), ("auth_mode", "api_key")], ServiceCredentialCapture::ApiKey, "api_key"),
        ("builtin.google-v3", vec![], ServiceCredentialCapture::Unknown, "none"),
        ("builtin.google-v3", vec![("google_auth_method", "oauth")], ServiceCredentialCapture::GoogleOwnedGrant, "google_oauth"),
        ("builtin.google-v3", vec![("google_auth_method", "service_account")], ServiceCredentialCapture::GoogleServiceAccountFile, "google_service_account"),
    ] {
        let mut query = request(id, 1, false);
        query.values = values.into_iter().map(|(key, value)| (key.into(), CatalogScalar::Text(value.into()))).collect();
        let form = registry.project_form(&query).unwrap();
        assert_eq!(form.credential_capture, capture, "{id}");
        let input = SaveServiceDraftInput {
            operation_id: "capture-consistency".into(), expected_profile_generation: std::num::NonZeroU32::new(1).unwrap(),
            expected_draft_head_revision: 0, editing: None, target: query.target,
            purpose: query.purpose, values: query.values, authentication: ServiceAuthenticationSelection::None {},
        };
        let draft = compile_service_draft(&ConfigurationDocument::Legacy { configuration: Configuration::default() }, &input, &registry, &app_infra::platform::SystemIdentity, &PreparedServiceDraftResources::default()).unwrap();
        assert_eq!(draft.graph.connections[0].auth_mode, auth, "{id}");
    }
    for engine in registry.catalog().offline_engines {
        assert_eq!(registry.project_form(&request(&engine.engine_id, engine.contract_revision, true)).unwrap().credential_capture, ServiceCredentialCapture::None);
    }
}
#[test]
fn all_25_registered_services_project_exactly_the_546_approved_rows_without_mutating_catalog() {
    let registry = register::builtin_registry().unwrap();
    let before = registry.catalog();
    let design: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/architecture/service-configuration-presentation.json"
    ))
    .unwrap();
    let mut rows = 0;
    let mut services = 0;
    let mut delta_rows = 0;
    for service in design["services"].as_array().unwrap() {
        let id = service["id"].as_str().unwrap();
        let offline = service["kind"] != "http";
        let p = registry.project_form(&request(id, 1, offline)).unwrap();
        let actual_fields = if offline {
            &before
                .offline_engines
                .iter()
                .find(|e| e.engine_id == id)
                .unwrap()
                .fields
        } else {
            &before
                .presets
                .iter()
                .find(|e| e.key.preset_id == id)
                .unwrap()
                .fields
        };
        assert_eq!(
            p.fields.iter().map(|r| &r.descriptor).collect::<Vec<_>>(),
            actual_fields.iter().collect::<Vec<_>>()
        );
        let frozen = service["fields"].as_array().unwrap();
        let delta = if offline { vec![] } else { network_delta() };
        assert_eq!(p.fields.len(), frozen.len() + delta.len());
        let (original_rows, extra_rows) = p.fields.split_at(frozen.len());
        assert_eq!(extra_rows.iter().map(|r|r.descriptor.clone()).collect::<Vec<_>>(), delta, "unexpected field suffix for {id}");
        for (actual, expected) in original_rows.iter().zip(frozen) {
            assert_eq!(normalized(actual.descriptor.clone()), normalized(frozen_descriptor(expected)), "frozen descriptor changed: {id} / {}", actual.descriptor.id);
            assert_eq!(actual.descriptor.id, expected["id"]);
            if actual.descriptor.classification == ServiceFieldClassification::NotApplicable {
                assert_eq!(actual.visibility, CapabilityState::Unsupported);
                assert!(!actual.input_allowed);
            }
            if actual.descriptor.masked
                || actual.descriptor.observed_only
                || actual.descriptor.field_type == ServiceFieldType::Secret
            {
                assert!(actual.resolved.value.is_none());
                assert!(!actual.input_allowed);
            }
        }
        rows += original_rows.len();
        delta_rows += extra_rows.len();
        services += 1;
    }
    assert_eq!((services, rows), (25, 546));
    assert_eq!(delta_rows, 42);
    assert_eq!(rows + delta_rows, 588);
    assert_eq!(before, registry.catalog());
    assert_eq!(before.protocols.len(), 13);
    assert_eq!(before.presets.len(), 21);
    assert_eq!(before.offline_engines.len(), 4);
}
#[test]
fn exact_target_revision_and_purpose_are_checked_before_any_projection() {
    let registry = register::builtin_registry().unwrap();
    for r in [
        request("builtin.deepseek", 2, false),
        request("builtin.absent", 1, false),
        request("apple_translation", 2, true),
        request("no_engine", 1, true),
    ] {
        assert!(registry.project_form(&r).is_err());
    }
    for mut r in [
        request("builtin.deepl", 1, false),
        request("apple_translation", 1, true),
    ] {
        r.purpose = CatalogPurpose::Grammar;
        assert_eq!(
            registry.project_form(&r).unwrap_err().code,
            "form_purpose_not_supported"
        );
    }
    let mut r = request("builtin.deepseek", 1, false);
    r.purpose = CatalogPurpose::Grammar;
    assert_eq!(
        registry.project_form(&r).unwrap().purpose,
        CatalogPurpose::Grammar
    );
}
#[test]
fn actual_preset_auth_is_hidden_but_secret_entry_visible_and_old_custom_auth_does_not_override() {
    let registry = register::builtin_registry().unwrap();
    let mut r = request("builtin.deepseek", 1, false);
    r.values
        .insert("auth_mode".into(), CatalogScalar::Text("none".into()));
    let p = registry.project_form(&r).unwrap();
    let auth = p
        .fields
        .iter()
        .find(|f| f.descriptor.id == "auth_mode")
        .unwrap();
    assert_eq!(auth.visibility, CapabilityState::Unsupported);
    assert!(auth.issues.contains(&ServiceFormIssue::InactiveField));
    let key = p
        .fields
        .iter()
        .find(|f| f.descriptor.id == "api_key")
        .unwrap();
    assert_eq!(key.visibility, CapabilityState::Supported);
    assert!(key.required);
    assert_eq!(key.resolved.value, None);
}
#[test]
fn actual_site_plan_defaults_require_explicit_choices_and_material_is_not_adapter_support() {
    let registry = register::builtin_registry().unwrap();
    let mut r = request("builtin.deepl", 1, false);
    let p = registry.project_form(&r).unwrap();
    assert_eq!(
        p.fields
            .iter()
            .find(|f| f.descriptor.id == "endpoint")
            .unwrap()
            .resolved
            .value,
        None
    );
    r.values
        .insert("plan".into(), CatalogScalar::Text("pro".into()));
    let p = registry.project_form(&r).unwrap();
    assert_eq!(
        p.fields
            .iter()
            .find(|f| f.descriptor.id == "endpoint")
            .unwrap()
            .resolved
            .value,
        Some(CatalogScalar::Text("https://api.deepl.com".into()))
    );
    let mut q = request("builtin.qwen", 1, false);
    q.values
        .insert("provider_site".into(), CatalogScalar::Text("other".into()));
    let p = registry.project_form(&q).unwrap();
    assert!(
        p.material_blockers
            .iter()
            .any(|b| b.condition_state == CapabilityState::Supported)
    );
    assert_eq!(p.implementation.state, CapabilityState::Supported);
}
#[test]
fn caller_cannot_supply_observations_readonly_secrets_actions_or_unknown_identity() {
    let registry = register::builtin_registry().unwrap();
    for (id, offline, field) in [
        ("builtin.deepseek", false, "api_key"),
        ("builtin.deepseek", false, "purpose"),
        ("builtin.deepseek", false, "protocol_details"),
        ("builtin.deepl", false, "model"),
        ("apple_translation", true, "system_status"),
        ("apple_translation", true, "prepare_language"),
        ("builtin.deepseek", false, "headers"),
    ] {
        let mut r = request(id, 1, offline);
        r.values
            .insert(field.into(), CatalogScalar::Text("DO-NOT-ECHO".into()));
        let e = registry.project_form(&r).unwrap_err();
        assert!(!format!("{e:?}").contains("DO-NOT-ECHO"));
    }
}
#[test]
fn all_546_field_input_permissions_follow_independent_classification_without_secret_reflection() {
    let registry = register::builtin_registry().unwrap();
    let design: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/architecture/service-configuration-presentation.json"
    ))
    .unwrap();
    let mut checked = 0;
    for service in design["services"].as_array().unwrap() {
        for field in service["fields"].as_array().unwrap() {
            let id = field["id"].as_str().unwrap();
            let mut r = request(
                service["id"].as_str().unwrap(),
                1,
                service["kind"] != "http",
            );
            let forbidden = id == "purpose"
                || field["masked"] == true
                || field["observed_only"] == true
                || matches!(
                    field["classification"].as_str().unwrap(),
                    "readonly" | "notapplicable"
                )
                || matches!(
                    field["type"].as_str().unwrap(),
                    "secret" | "action" | "status"
                );
            let value = if forbidden {
                CatalogScalar::Text("PROTECTED-NEVER-REFLECT".into())
            } else {
                match field["type"].as_str().unwrap() {
                    "boolean" => CatalogScalar::Boolean(false),
                    "number" => CatalogScalar::Number(match id {
                        "timeout" => 30.0,
                        "retries" | "temperature" => 0.0,
                        _ => 1.0,
                    }),
                    "select" => {
                        serde_json::from_value(field["choices"][0]["value"].clone()).unwrap()
                    }
                    "url" => CatalogScalar::Text("https://synthetic.example.invalid".into()),
                    _ => CatalogScalar::Text("synthetic-edit".into()),
                }
            };
            r.values.insert(id.into(), value.clone());
            if forbidden {
                let error = registry.project_form(&r).unwrap_err();
                assert_eq!(
                    error.code, "protected_form_field",
                    "{} / {id}",
                    service["id"]
                );
                assert!(!format!("{error:?}").contains("PROTECTED-NEVER-REFLECT"));
            } else {
                let projection = registry.project_form(&r).unwrap();
                let row = projection
                    .fields
                    .iter()
                    .find(|f| f.descriptor.id == id)
                    .unwrap();
                assert!(row.input_allowed);
                assert_eq!(row.input_value, Some(value));
                let json = serde_json::to_value(&projection).unwrap();
                assert!(json.get("eligible").is_none() && json.get("saved").is_none());
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 546);
}
#[test]
fn every_registered_allowed_purpose_is_projected_from_typed_context_and_never_saved() {
    let registry = register::builtin_registry().unwrap();
    let catalog = registry.catalog();
    let mut count = 0;
    for preset in &catalog.presets {
        for purpose in &preset.allowed_purposes {
            let mut request = request(&preset.key.preset_id, preset.key.preset_revision, false);
            request.purpose = *purpose;
            let projected = registry.project_form(&request).unwrap();
            let field = projected
                .fields
                .iter()
                .find(|r| r.descriptor.id == "purpose")
                .unwrap();
            assert_eq!(field.resolved.source, ServiceFormValueSource::Purpose);
            assert_eq!(
                field.resolved.value,
                Some(CatalogScalar::Text(
                    match purpose {
                        CatalogPurpose::Translation => "translation",
                        CatalogPurpose::Grammar => "grammar",
                        CatalogPurpose::ContextWord => "context_word",
                    }
                    .into()
                ))
            );
            count += 1;
        }
    }
    assert!(count > 21);
    assert_eq!(registry.catalog(), catalog);
}
