use app_core::{contracts::*, domain::model_config::form::project_service_form};
fn text(s: &str) -> CatalogScalar {
    CatalogScalar::Text(s.into())
}
fn field(id: &str, ty: ServiceFieldType) -> ServiceFieldDescriptor {
    ServiceFieldDescriptor {
        id: id.into(),
        label: id.into(),
        field_type: ty,
        classification: ServiceFieldClassification::Advanced,
        group: None,
        condition: ServiceFieldCondition::default(),
        choices: vec![],
        default_value: None,
        default_by: None,
        default_evidence: None,
        inherit_from: None,
        required_when_visible: false,
        masked: false,
        observed_only: false,
        help: None,
        source_refs: vec![],
    }
}
fn req(values: Vec<(&str, CatalogScalar)>) -> ServiceFormRequest {
    ServiceFormRequest {
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: "user.synthetic".into(),
                preset_revision: 1,
            },
        },
        purpose: CatalogPurpose::Translation,
        values: values.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    }
}
fn project(
    fields: &[ServiceFieldDescriptor],
    values: Vec<(&str, CatalogScalar)>,
) -> ServiceFormProjection {
    project_service_form(
        &req(values),
        fields,
        &[],
        &CatalogDecision {
            state: CapabilityState::Unknown,
            reason: Some(CatalogReason::RequiresSavedConfiguration),
        },
    )
    .unwrap()
}
fn row<'a>(p: &'a ServiceFormProjection, id: &str) -> &'a ServiceFormRow {
    p.fields.iter().find(|r| r.descriptor.id == id).unwrap()
}
fn condition(id: &str, value: &str) -> ServiceFieldCondition {
    ServiceFieldCondition {
        all: vec![ServiceConditionClause {
            field: id.into(),
            operator: ServiceConditionOperator::Eq,
            values: vec![text(value)],
        }],
        any: vec![],
    }
}
#[test]
fn defaults_are_dependency_driven_never_first_choice_or_previous_query_input() {
    let mut selector = field("site", ServiceFieldType::Select);
    selector.choices = vec![
        ServiceFieldChoice {
            value: text("a"),
            label: "A".into(),
        },
        ServiceFieldChoice {
            value: text("b"),
            label: "B".into(),
        },
    ];
    let mut endpoint = field("endpoint", ServiceFieldType::Url);
    endpoint.default_by = Some(ServiceFieldDefaultBy {
        field: "site".into(),
        values: vec![ServiceConditionalDefault {
            selector_value: text("b"),
            value: text("https://b.example.invalid"),
        }],
    });
    let p = project(&[endpoint.clone(), selector.clone()], vec![]);
    assert_eq!(row(&p, "site").resolved.value, None);
    assert_eq!(row(&p, "endpoint").resolved.value, None);
    let p = project(
        &[endpoint.clone(), selector.clone()],
        vec![("site", text("b"))],
    );
    assert_eq!(
        row(&p, "endpoint").resolved.value,
        Some(text("https://b.example.invalid"))
    );
    assert_eq!(
        row(&p, "endpoint").resolved.source,
        ServiceFormValueSource::Default
    );
    let p = project(&[endpoint, selector], vec![]);
    assert_eq!(row(&p, "endpoint").resolved.value, None);
}
#[test]
fn hidden_trusted_selector_default_works_but_inactive_caller_cannot_override_it() {
    let mut mode = field("mode", ServiceFieldType::Text);
    mode.default_value = Some(text("preset"));
    let mut auth = field("auth", ServiceFieldType::Text);
    auth.default_value = Some(text("key"));
    auth.condition = condition("mode", "custom");
    let mut key = field("key", ServiceFieldType::Secret);
    key.masked = true;
    key.required_when_visible = true;
    key.condition = condition("auth", "key");
    let p = project(
        &[key.clone(), auth.clone(), mode.clone()],
        vec![("auth", text("none"))],
    );
    assert_eq!(row(&p, "auth").visibility, CapabilityState::Unsupported);
    assert!(
        row(&p, "auth")
            .issues
            .contains(&ServiceFormIssue::InactiveField)
    );
    assert_eq!(row(&p, "auth").input_value, Some(text("none")));
    assert_eq!(row(&p, "auth").resolved.value, Some(text("key")));
    assert_eq!(row(&p, "key").visibility, CapabilityState::Supported);
    assert!(row(&p, "key").required);
    assert_eq!(row(&p, "key").resolved.value, None);
    let p = project(
        &[key, auth, mode],
        vec![("mode", text("custom")), ("auth", text("none"))],
    );
    assert_eq!(
        row(&p, "auth").resolved.source,
        ServiceFormValueSource::User
    );
    assert_eq!(row(&p, "key").visibility, CapabilityState::Unsupported);
}
#[test]
fn readonly_observation_remains_unknown_and_inheritance_has_no_fabricated_preference() {
    let mut observed = field("observed", ServiceFieldType::Status);
    observed.classification = ServiceFieldClassification::Readonly;
    observed.observed_only = true;
    let mut child = field("child", ServiceFieldType::Text);
    child.condition = condition("observed", "yes");
    child.required_when_visible = true;
    let mut language = field("language", ServiceFieldType::Text);
    language.inherit_from = Some(ServiceFieldInheritance::TargetLanguage);
    let p = project(&[observed, child, language], vec![]);
    assert_eq!(row(&p, "child").visibility, CapabilityState::Unknown);
    assert!(!row(&p, "child").required);
    assert!(
        row(&p, "child")
            .issues
            .contains(&ServiceFormIssue::ConditionUnresolved)
    );
    assert_eq!(
        row(&p, "language").resolved.source,
        ServiceFormValueSource::Inherited
    );
    assert_eq!(row(&p, "language").resolved.value, None);
}
#[test]
fn field_issues_retain_bounded_invalid_edits_without_polluting_dependencies() {
    let mut selector = field("selector", ServiceFieldType::Select);
    selector.choices = vec![ServiceFieldChoice {
        value: text("valid"),
        label: "Valid".into(),
    }];
    let mut child = field("child", ServiceFieldType::Text);
    child.condition = condition("selector", "valid");
    let number = field("retries", ServiceFieldType::Number);
    let p = project(
        &[selector, child, number],
        vec![
            ("selector", text("wrong")),
            ("retries", CatalogScalar::Number(4.0)),
        ],
    );
    assert!(
        row(&p, "selector")
            .issues
            .contains(&ServiceFormIssue::InvalidChoice)
    );
    assert_eq!(row(&p, "selector").input_value, Some(text("wrong")));
    assert_eq!(row(&p, "selector").resolved.value, None);
    assert_eq!(row(&p, "child").visibility, CapabilityState::Unknown);
    assert!(
        row(&p, "retries")
            .issues
            .contains(&ServiceFormIssue::InvalidNumber)
    );
}
#[test]
fn protected_unknown_and_unsafe_values_reject_without_echoing_secret() {
    for (ty, class, masked, observed) in [
        (
            ServiceFieldType::Secret,
            ServiceFieldClassification::Required,
            true,
            false,
        ),
        (
            ServiceFieldType::Action,
            ServiceFieldClassification::Advanced,
            false,
            false,
        ),
        (
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            false,
            false,
        ),
        (
            ServiceFieldType::Text,
            ServiceFieldClassification::NotApplicable,
            false,
            false,
        ),
        (
            ServiceFieldType::Text,
            ServiceFieldClassification::Advanced,
            false,
            true,
        ),
        (
            ServiceFieldType::Text,
            ServiceFieldClassification::Readonly,
            false,
            false,
        ),
    ] {
        let mut f = field("protected", ty);
        f.classification = class;
        f.masked = masked;
        f.observed_only = observed;
        let e = project_service_form(
            &req(vec![("protected", text("SECRET-MUST-NOT-RETURN"))]),
            &[f],
            &[],
            &CatalogDecision {
                state: CapabilityState::Unknown,
                reason: None,
            },
        )
        .unwrap_err();
        assert_eq!(e.code, "protected_form_field");
        assert!(!format!("{e:?}").contains("SECRET-MUST-NOT-RETURN"));
    }
    let f = field("v", ServiceFieldType::Text);
    for value in [
        text("x\0y"),
        CatalogScalar::Text("x".repeat(16385)),
        CatalogScalar::Number(f64::NAN),
        CatalogScalar::Number(f64::INFINITY),
    ] {
        assert_eq!(
            project_service_form(
                &req(vec![("v", value)]),
                std::slice::from_ref(&f),
                &[],
                &CatalogDecision {
                    state: CapabilityState::Unknown,
                    reason: None
                }
            )
            .unwrap_err()
            .code,
            "invalid_form_scalar"
        );
    }
    assert_eq!(
        project_service_form(
            &req(vec![("missing", text("x"))]),
            &[f],
            &[],
            &CatalogDecision {
                state: CapabilityState::Unknown,
                reason: None
            }
        )
        .unwrap_err()
        .code,
        "unknown_form_field"
    );
}
#[test]
fn purpose_is_single_typed_authority_and_json_rejects_duplicate_map_keys() {
    let f = field("purpose", ServiceFieldType::Text);
    let p = project(std::slice::from_ref(&f), vec![]);
    assert_eq!(
        row(&p, "purpose").resolved.source,
        ServiceFormValueSource::Purpose
    );
    assert_eq!(row(&p, "purpose").resolved.value, Some(text("translation")));
    assert!(
        project_service_form(
            &req(vec![("purpose", text("translation"))]),
            &[f],
            &[],
            &CatalogDecision {
                state: CapabilityState::Unknown,
                reason: None
            }
        )
        .is_err()
    );
    for json in [
        r#"{"target":{"kind":"http","preset":{"presetId":"user.x","presetRevision":1}},"purpose":"translation","values":{"x":"a","x":"b"}}"#,
        r#"{"target":{"kind":"http","preset":{"presetId":"user.x","presetRevision":1},"script":"no"},"purpose":"translation","values":{}}"#,
    ] {
        assert!(serde_json::from_str::<ServiceFormRequest>(json).is_err());
    }
}
#[test]
fn material_condition_and_implementation_are_independent_and_unobserved_is_not_true() {
    let observed = field("obs", ServiceFieldType::Text);
    let b = ConditionalCatalogBlocker {
        condition: condition("obs", "yes"),
        reason: CatalogReason::ApplicationOauthSetupUnverified,
        message: "Need verified setup".into(),
        source_refs: vec![],
    };
    let p = project_service_form(
        &req(vec![]),
        &[observed],
        &[b],
        &CatalogDecision {
            state: CapabilityState::Unsupported,
            reason: Some(CatalogReason::AdapterNotImplemented),
        },
    )
    .unwrap();
    assert_eq!(
        p.material_blockers[0].condition_state,
        CapabilityState::Unknown
    );
    assert_eq!(p.implementation.state, CapabilityState::Unsupported);
}
#[test]
fn malformed_fake_descriptor_and_dependency_cycle_do_not_become_runtime_support() {
    let mut a = field("a", ServiceFieldType::Text);
    a.default_by = Some(ServiceFieldDefaultBy {
        field: "b".into(),
        values: vec![ServiceConditionalDefault {
            selector_value: text("x"),
            value: text("x"),
        }],
    });
    let mut b = field("b", ServiceFieldType::Text);
    b.default_by = Some(ServiceFieldDefaultBy {
        field: "a".into(),
        values: vec![ServiceConditionalDefault {
            selector_value: text("x"),
            value: text("x"),
        }],
    });
    let p = project(&[a, b], vec![]);
    assert!(p.fields.iter().all(|r| r.resolved.value.is_none()));
    assert_eq!(p.implementation.state, CapabilityState::Unknown);
    let mut broken = field("broken", ServiceFieldType::Text);
    broken.condition = condition("absent", "x");
    assert!(
        project_service_form(
            &req(vec![]),
            &[broken],
            &[],
            &CatalogDecision {
                state: CapabilityState::Unknown,
                reason: None
            }
        )
        .is_err()
    );
}
#[test]
fn valid_zero_and_explicit_false_are_preserved_not_treated_as_missing() {
    let p = project(
        &[
            field("temperature", ServiceFieldType::Number),
            field("cache", ServiceFieldType::Boolean),
        ],
        vec![
            ("temperature", CatalogScalar::Number(0.0)),
            ("cache", CatalogScalar::Boolean(false)),
        ],
    );
    assert!(
        p.fields
            .iter()
            .all(|r| r.issues.is_empty() && r.resolved.source == ServiceFormValueSource::User)
    );
    assert_eq!(
        row(&p, "cache").resolved.value,
        Some(CatalogScalar::Boolean(false))
    );
}

#[test]
fn oscillating_conditional_inputs_are_bounded_and_marked_unknown() {
    let mut a = field("a", ServiceFieldType::Text);
    a.default_value = Some(text("no"));
    a.condition = condition("b", "yes");
    let mut b = field("b", ServiceFieldType::Text);
    b.default_value = Some(text("no"));
    b.condition = condition("a", "no");
    let p = project(&[a, b], vec![("a", text("yes")), ("b", text("yes"))]);
    assert!(
        p.fields
            .iter()
            .all(|r| r.visibility == CapabilityState::Unknown
                && r.resolved.value.is_none()
                && r.issues.contains(&ServiceFormIssue::DependencyCycle))
    );
}
#[test]
fn masked_textarea_cannot_smuggle_catalog_default_and_request_debug_is_redacted() {
    let mut f = field("service_account_json", ServiceFieldType::Textarea);
    f.masked = true;
    f.default_value = Some(text("SECRET"));
    assert_eq!(
        project_service_form(
            &req(vec![]),
            &[f],
            &[],
            &CatalogDecision {
                state: CapabilityState::Unknown,
                reason: None
            }
        )
        .unwrap_err()
        .code,
        "protected_form_descriptor"
    );
    assert!(!format!("{:?}", req(vec![("x", text("SECRET"))])).contains("SECRET"));
}
#[test]
fn numeric_type_boundaries_and_url_errors_preserve_only_nonsecret_draft_values() {
    for (id, value, expected) in [
        (
            "timeout",
            CatalogScalar::Number(0.999),
            ServiceFormIssue::InvalidNumber,
        ),
        (
            "timeout",
            CatalogScalar::Number(300.001),
            ServiceFormIssue::InvalidNumber,
        ),
        (
            "concurrency",
            CatalogScalar::Number(1.5),
            ServiceFormIssue::InvalidNumber,
        ),
        (
            "max_tokens",
            CatalogScalar::Number(-1.0),
            ServiceFormIssue::InvalidNumber,
        ),
        (
            "temperature",
            CatalogScalar::Boolean(false),
            ServiceFormIssue::TypeMismatch,
        ),
    ] {
        let p = project(
            &[field(id, ServiceFieldType::Number)],
            vec![(id, value.clone())],
        );
        assert!(p.fields[0].issues.contains(&expected));
        assert_eq!(p.fields[0].input_value, Some(value));
        assert_eq!(p.fields[0].resolved.value, None);
    }
    for endpoint in [
        "file:///tmp/x",
        "https://key@host.invalid",
        "https://host.invalid?api_key=x",
        "https://",
    ] {
        let p = project(
            &[field("endpoint", ServiceFieldType::Url)],
            vec![("endpoint", text(endpoint))],
        );
        assert!(p.fields[0].issues.contains(&ServiceFormIssue::InvalidUrl));
    }
    let p = project(
        &[field("timeout", ServiceFieldType::Number)],
        vec![("timeout", CatalogScalar::Number(1.001))],
    );
    assert!(p.fields[0].issues.is_empty());
}
#[test]
fn form_wire_and_typescript_exports_preserve_exact_target_and_nested_types() {
    let request = ServiceFormRequest {
        target: ServiceFormTarget::Offline {
            engine_id: "apple_translation".into(),
            contract_revision: 1,
        },
        purpose: CatalogPurpose::Translation,
        values: Default::default(),
    };
    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(
        json["target"],
        serde_json::json!({"kind":"offline","engineId":"apple_translation","contractRevision":1})
    );
    assert_eq!(
        serde_json::from_value::<ServiceFormRequest>(json).unwrap(),
        request
    );
    let declarations = app_core::contracts::typescript_declarations();
    for name in [
        "ServiceFormTarget",
        "ServiceFormRequest",
        "ServiceFormValueSource",
        "ServiceFormResolvedValue",
        "ServiceFormIssue",
        "ServiceFormRow",
        "ServiceFormMaterialBlocker",
        "ServiceFormProjection",
    ] {
        assert!(
            declarations.contains(&format!("type {name} =")),
            "missing nested TS {name}"
        );
    }
}
