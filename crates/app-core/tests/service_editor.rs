use app_core::{
    contracts::*,
    domain::model_config::{
        form::project_service_form, service_draft::*, service_editor::*, service_graph::*,
    },
    ports::{IdentitySource, service_configuration::ServiceDirectory},
};
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Ids(AtomicUsize);
impl Ids {
    fn at(n: usize) -> Self {
        Self(AtomicUsize::new(n))
    }
}
impl IdentitySource for Ids {
    fn new_id(&self, p: &str) -> String {
        format!("{p}-{}", self.0.fetch_add(1, Ordering::SeqCst))
    }
    fn now_ms(&self) -> i64 {
        panic!("pure editor/compile must not use time")
    }
}
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
        condition: Default::default(),
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
#[derive(Clone)]
struct Directory {
    catalog: ServiceCatalog,
}
impl ServiceDirectory for Directory {
    fn catalog(&self) -> ServiceCatalog {
        self.catalog.clone()
    }
    fn form(&self, r: &ServiceFormRequest) -> AppResult<ServiceFormProjection> {
        let p = &self.catalog.presets[0];
        project_service_form(r, &p.fields, &[], &p.implementation)
    }
}
fn directory() -> Directory {
    Directory {
        catalog: ServiceCatalog {
            schema_version: 1,
            brands: vec![],
            protocols: vec![],
            profiles: vec![],
            offline_engines: vec![],
            sources: vec![],
            presets: vec![ProviderPresetDescriptor {
                key: ProviderPresetKey {
                    preset_id: "user.editor-fixture".into(),
                    preset_revision: 1,
                },
                namespace: CatalogNamespace::User,
                provider_id: "fixture".into(),
                brand_id: "fixture".into(),
                brand_label: "Fixture".into(),
                label: "Fixture".into(),
                binding: AdapterBinding {
                    protocol: ProtocolKey {
                        protocol_id: "openai_compatible".into(),
                        contract_revision: 1,
                    },
                    profile: CompatibilityProfileKey {
                        profile_id: "openai-chat-text".into(),
                        profile_revision: 1,
                    },
                },
                allowed_purposes: vec![CatalogPurpose::Translation, CatalogPurpose::Grammar],
                interface_label: "HTTP".into(),
                interface_description: "fixture".into(),
                interface_choice_required: false,
                default_auth_kind: CatalogAuthKind::None,
                default_endpoint: None,
                fields: vec![
                    field("name", ServiceFieldType::Text),
                    field("context_count", ServiceFieldType::Number),
                    field("timeout", ServiceFieldType::Number),
                    field("model", ServiceFieldType::Text),
                    field("template", ServiceFieldType::Textarea),
                ],
                material_blockers: vec![],
                source_refs: vec![],
                implementation: CatalogDecision {
                    state: CapabilityState::Unknown,
                    reason: None,
                },
                eligibility: CatalogDecision {
                    state: CapabilityState::Unknown,
                    reason: None,
                },
            }],
        },
    }
}
fn input() -> SaveServiceDraftInput {
    SaveServiceDraftInput {
        operation_id: "save".into(),
        expected_profile_generation: NonZeroU32::new(1).unwrap(),
        expected_draft_head_revision: 1,
        editing: None,
        target: ServiceFormTarget::Http {
            preset: directory().catalog.presets[0].key.clone(),
        },
        purpose: CatalogPurpose::Translation,
        values: BTreeMap::new(),
        authentication: ServiceAuthenticationSelection::None {},
    }
}
fn initial() -> ConfigurationV3 {
    compile_service_draft(
        &ConfigurationDocument::Legacy {
            configuration: Configuration {
                revision_id: "empty".into(),
                ..Default::default()
            },
        },
        &input(),
        &directory(),
        &Ids::at(0),
        &Default::default(),
    )
    .unwrap()
    .graph
}
#[test]
fn editing_preserves_unexposed_context_mode_and_unchanged_consent() {
    let mut graph = initial();
    let d = graph.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap();
    d.context = ServiceContextConsent {
        enabled: false,
        max_sentences: 3,
        mode: HistoryInputMode::Both,
    };
    let mut req = input();
    req.editing = Some(SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: graph.revision_id.clone(),
            model_id: graph.models[0].model_id.clone(),
        },
    });
    req.values
        .insert("context_count".into(), CatalogScalar::Number(3.0));
    let r = compile_service_draft(
        &ConfigurationDocument::ServiceGraph {
            configuration: graph,
        },
        &req,
        &directory(),
        &Ids::at(100),
        &Default::default(),
    )
    .unwrap();
    assert_eq!(
        r.graph.models[0].request_defaults[&CatalogPurpose::Translation].context,
        ServiceContextConsent {
            enabled: false,
            max_sentences: 3,
            mode: HistoryInputMode::Both
        }
    );
}
fn source(g: &ConfigurationV3) -> SavedServiceRef {
    SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: g.revision_id.clone(),
            model_id: g.models[0].model_id.clone(),
        },
    }
}
fn snapshot(g: ConfigurationV3) -> ServiceConfigurationSnapshot {
    ServiceConfigurationSnapshot {
        profile_generation: NonZeroU32::new(1).unwrap(),
        draft_head_revision: 8,
        applied_head_revision: 3,
        draft: ConfigurationDocument::ServiceGraph { configuration: g },
        applied: None,
        runtime_purposes: vec![],
        runtime_connections: vec![],
    }
}
fn query(g: &ConfigurationV3) -> ServiceDraftQuery {
    ServiceDraftQuery {
        expected_profile_generation: NonZeroU32::new(1).unwrap(),
        source: source(g),
        purpose: CatalogPurpose::Translation,
    }
}
fn save_projection(p: ServiceDraftEditorProjection) -> SaveServiceDraftInput {
    let mut values = p.values;
    if !p.fallback.is_empty() {
        values.insert(
            "fallback".into(),
            text(&serde_json::to_string(&p.fallback).unwrap()),
        );
    }
    SaveServiceDraftInput {
        operation_id: "edit".into(),
        expected_profile_generation: p.profile_generation,
        expected_draft_head_revision: p.draft_head_revision,
        editing: p.editing,
        target: p.target.unwrap(),
        purpose: p.purpose,
        values,
        authentication: p.authentication,
    }
}
#[test]
fn editor_roundtrip_preserves_templates_unexposed_parameters_and_source_metadata() {
    let d = directory();
    let mut g = initial();
    let m = &mut g.models[0];
    m.parameters
        .insert("max_tokens".into(), GenerationParameter::Omit {});
    m.parameters.insert(
        "temperature".into(),
        GenerationParameter::Set {
            value: serde_json::json!(0),
        },
    );
    m.selection_source = "catalog-cache".into();
    m.request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .context
        .mode = HistoryInputMode::Translation;
    m.default_templates
        .insert(CatalogPurpose::Translation, "custom-template".into());
    g.templates.push(TemplateV3 {
        template_id: "custom-template".into(),
        purpose: CatalogPurpose::Translation,
        display_name: "原模板".into(),
        source_kind: "custom".into(),
        source_version: Some("v3".into()),
        body: BTreeMap::from([
            (
                "system".into(),
                serde_json::json!("hidden system {{history}}"),
            ),
            ("user".into(), serde_json::json!("source {{source}}")),
        ]),
        variables: vec!["source".into(), "history".into()],
        compatibility: BTreeMap::from([("exact".into(), serde_json::json!(false))]),
    });
    g.connections[0].display_name = "原连接名称".into();
    g.connections[0].deployment = "remote".into();
    g.connections[0].availability_intent = false;
    let snap = snapshot(g.clone());
    let p =
        project_service_editor(&query(&g), &snap.draft, &snap, &d, &Default::default()).unwrap();
    assert!(p.readonly_reason.is_none(), "{:?}", p.readonly_reason);
    assert_eq!(p.editing, Some(source(&g)));
    assert!(p.values.contains_key("template"));
    assert!(!p.values.contains_key("temperature"));
    let r = compile_service_draft(
        &snap.draft,
        &save_projection(p),
        &d,
        &Ids::at(100),
        &Default::default(),
    )
    .unwrap();
    assert_eq!(r.graph.models, g.models);
    assert_eq!(r.graph.connections, g.connections);
    assert_eq!(r.graph.templates, g.templates);
    assert_eq!(r.graph.bindings, g.bindings);
}
#[test]
fn historical_edit_requires_whole_template_closure_equality() {
    let d = directory();
    let mut g = initial();
    g.models[0]
        .default_templates
        .insert(CatalogPurpose::Translation, "t".into());
    g.templates.push(TemplateV3 {
        template_id: "t".into(),
        purpose: CatalogPurpose::Translation,
        display_name: "template".into(),
        source_kind: "custom".into(),
        source_version: None,
        body: BTreeMap::from([
            ("user".into(), serde_json::json!("{{source}}")),
            ("system".into(), serde_json::json!("original")),
        ]),
        variables: vec!["source".into()],
        compatibility: BTreeMap::new(),
    });
    let old = ConfigurationDocument::ServiceGraph {
        configuration: g.clone(),
    };
    let mut current = g.clone();
    current.revision_id = "new-head".into();
    let snap = snapshot(current.clone());
    let p = project_service_editor(&query(&g), &old, &snap, &d, &Default::default()).unwrap();
    assert_eq!(p.editing, Some(source(&current)));
    assert_eq!(p.source, source(&g));
    current.templates[0]
        .body
        .insert("system".into(), serde_json::json!("changed hidden system"));
    let p = project_service_editor(
        &query(&g),
        &old,
        &snapshot(current),
        &d,
        &Default::default(),
    )
    .unwrap();
    assert!(p.editing.is_none());
    assert!(p.target.is_some());
    assert!(p.readonly_reason.unwrap().contains("相关模板"));
}
#[test]
fn wrong_source_resource_purpose_or_profile_is_rejected() {
    let g = initial();
    let snap = snapshot(g.clone());
    let d = directory();
    let mut q = query(&g);
    q.expected_profile_generation = NonZeroU32::new(2).unwrap();
    assert_eq!(
        project_service_editor(&q, &snap.draft, &snap, &d, &Default::default())
            .unwrap_err()
            .code,
        "profile_generation_changed"
    );
    q = query(&g);
    if let SavedServiceRef::Http { source } = &mut q.source {
        source.revision_id = "wrong".into();
    }
    assert_eq!(
        project_service_editor(&q, &snap.draft, &snap, &d, &Default::default())
            .unwrap_err()
            .code,
        "service_editor_source_mismatch"
    );
    q = query(&g);
    q.purpose = CatalogPurpose::Grammar;
    assert_eq!(
        project_service_editor(&q, &snap.draft, &snap, &d, &Default::default())
            .unwrap_err()
            .code,
        "service_editor_purpose_mismatch"
    );
    q = query(&g);
    if let SavedServiceRef::Http { source } = &mut q.source {
        source.model_id = "missing".into();
    }
    assert_eq!(
        project_service_editor(&q, &snap.draft, &snap, &d, &Default::default())
            .unwrap_err()
            .code,
        "service_editor_resource_missing"
    );
}
#[test]
fn legacy_and_unknown_presets_are_readonly_without_guessing_target() {
    let mut g = initial();
    g.connections[0].preset_source = None;
    g.connections[0].origin = Some(ConnectionOrigin::Custom {
        copied_from: None,
        site: None,
    });
    let snap = snapshot(g.clone());
    let p = project_service_editor(
        &query(&g),
        &snap.draft,
        &snap,
        &directory(),
        &Default::default(),
    )
    .unwrap();
    assert!(p.target.is_none());
    assert!(p.editing.is_none());
    assert!(p.values.is_empty());
    assert!(p.readonly_reason.is_some());
    g.models[0].template_purpose = None;
    g.models[0].legacy_eligibility_origin = Some(HttpConfigRef {
        revision_id: "legacy-origin".into(),
        model_id: g.models[0].model_id.clone(),
    });
    g.models[0].request_defaults.clear();
    g.models[0]
        .default_templates
        .insert(CatalogPurpose::Translation, "legacy-template".into());
    g.templates.push(TemplateV3 {
        template_id: "legacy-template".into(),
        purpose: CatalogPurpose::Translation,
        display_name: "legacy".into(),
        source_kind: "custom".into(),
        source_version: None,
        body: BTreeMap::from([("user".into(), serde_json::json!("{{source}}"))]),
        variables: vec!["source".into()],
        compatibility: BTreeMap::new(),
    });
    let snap = snapshot(g.clone());
    let p = project_service_editor(
        &query(&g),
        &snap.draft,
        &snap,
        &directory(),
        &Default::default(),
    )
    .unwrap();
    assert!(p.target.is_none());
    assert!(p.editing.is_none());
}
#[test]
fn shared_models_are_reported_without_turning_service_rename_into_connection_edit() {
    let mut d = directory();
    d.catalog.presets[0]
        .fields
        .push(field("endpoint", ServiceFieldType::Url));
    let mut g = initial();
    let mut sibling = g.models[0].clone();
    sibling.model_id = "sibling".into();
    g.models.push(sibling);
    let snap = snapshot(g.clone());
    let p =
        project_service_editor(&query(&g), &snap.draft, &snap, &d, &Default::default()).unwrap();
    assert_eq!(p.affected_shared_model_ids, vec!["sibling"]);
    let mut req = save_projection(p);
    req.values.insert("name".into(), text("只更改服务名称"));
    let r =
        compile_service_draft(&snap.draft, &req, &d, &Ids::at(100), &Default::default()).unwrap();
    assert_eq!(r.graph.connections, g.connections);
    assert_eq!(r.graph.models[1], g.models[1]);
    req.values
        .insert("endpoint".into(), text("https://changed.example.invalid"));
    assert_eq!(
        compile_service_draft(&snap.draft, &req, &d, &Ids::at(100), &Default::default())
            .err()
            .unwrap()
            .code,
        "shared_connection_review_required"
    );
}
#[test]
fn hidden_fields_keep_previous_values_while_visible_omissions_are_desired_unset() {
    let mut d = directory();
    for row in &mut d.catalog.presets[0].fields {
        if matches!(
            row.id.as_str(),
            "context_count" | "timeout" | "model" | "template"
        ) {
            row.classification = ServiceFieldClassification::Readonly;
        }
    }
    let mut g = initial();
    g.models[0].actual_model_id = Some("hidden-model".into());
    let defaults = g.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap();
    defaults.execution.timeout_ms = 12345;
    defaults.context = ServiceContextConsent {
        enabled: true,
        max_sentences: 9,
        mode: HistoryInputMode::Both,
    };
    let snap = snapshot(g.clone());
    let p =
        project_service_editor(&query(&g), &snap.draft, &snap, &d, &Default::default()).unwrap();
    assert!(!p.values.contains_key("model"));
    assert!(!p.values.contains_key("context_count"));
    let r = compile_service_draft(
        &snap.draft,
        &save_projection(p),
        &d,
        &Ids::at(100),
        &Default::default(),
    )
    .unwrap();
    assert_eq!(r.graph.models, g.models);
    let d = directory();
    let mut req = input();
    req.editing = Some(source(&g));
    let r =
        compile_service_draft(&snap.draft, &req, &d, &Ids::at(100), &Default::default()).unwrap();
    assert!(r.graph.models[0].actual_model_id.is_none());
    assert_eq!(
        r.graph.models[0].request_defaults[&CatalogPurpose::Translation]
            .execution
            .timeout_ms,
        60000
    );
}
