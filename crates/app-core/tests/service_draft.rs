use app_core::{
    contracts::*,
    domain::model_config::{form::project_service_form, service_draft::*, service_graph::*},
    ports::{IdentitySource, service_configuration::ServiceDirectory},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Ids(AtomicUsize);
impl Ids {
    fn new() -> Self {
        Self(AtomicUsize::new(0))
    }
}
impl IdentitySource for Ids {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-{}", self.0.fetch_add(1, Ordering::SeqCst))
    }
    fn now_ms(&self) -> i64 {
        panic!("draft compilation must not consult time")
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
fn select(id: &str, choices: &[&str], default: Option<&str>) -> ServiceFieldDescriptor {
    let mut f = field(id, ServiceFieldType::Select);
    f.choices = choices
        .iter()
        .map(|s| ServiceFieldChoice {
            value: text(s),
            label: (*s).into(),
        })
        .collect();
    f.default_value = default.map(text);
    f
}
#[derive(Clone)]
struct Directory(ServiceCatalog);
impl ServiceDirectory for Directory {
    fn catalog(&self) -> ServiceCatalog {
        self.0.clone()
    }
    fn form(&self, r: &ServiceFormRequest) -> AppResult<ServiceFormProjection> {
        let fields = match &r.target {
            ServiceFormTarget::Http { preset } => {
                &self
                    .0
                    .presets
                    .iter()
                    .find(|p| &p.key == preset)
                    .unwrap()
                    .fields
            }
            ServiceFormTarget::Offline { .. } => &self.0.offline_engines[0].fields,
        };
        project_service_form(
            r,
            fields,
            &[],
            &CatalogDecision {
                state: CapabilityState::Unknown,
                reason: Some(CatalogReason::RequiresSavedConfiguration),
            },
        )
    }
}
fn directory() -> Directory {
    let binding = AdapterBinding {
        protocol: ProtocolKey {
            protocol_id: "openai_compatible".into(),
            contract_revision: 1,
        },
        profile: CompatibilityProfileKey {
            profile_id: "openai-chat-text".into(),
            profile_revision: 1,
        },
    };
    let mut fields = vec![
        field("name", ServiceFieldType::Text),
        field("model", ServiceFieldType::Text),
        field("endpoint", ServiceFieldType::Url),
        field("template", ServiceFieldType::Textarea),
        select("connection_mode", &["preset", "custom"], Some("preset")),
        select("auth_mode", &["api_key", "none"], Some("api_key")),
        select(
            "temperature_mode",
            &["inherit", "set", "omit"],
            Some("inherit"),
        ),
    ];
    for id in [
        "temperature",
        "max_tokens",
        "timeout",
        "concurrency",
        "retries",
        "context_count",
    ] {
        fields.push(field(id, ServiceFieldType::Number));
    }
    for id in [
        "source_language",
        "target_language",
        "fallback",
        "organization_id",
        "openai_project_id",
    ] {
        fields.push(field(id, ServiceFieldType::Text));
    }
    fields.push(field("cache", ServiceFieldType::Boolean));
    let mut secret = field("api_key", ServiceFieldType::Secret);
    secret.masked = true;
    fields.push(secret);
    Directory(ServiceCatalog {
        schema_version: 1,
        brands: vec![],
        protocols: vec![],
        profiles: vec![],
        sources: vec![],
        presets: vec![ProviderPresetDescriptor {
            key: ProviderPresetKey {
                preset_id: "user.fixture".into(),
                preset_revision: 1,
            },
            namespace: CatalogNamespace::User,
            provider_id: "fixture".into(),
            brand_id: "fixture".into(),
            brand_label: "Fixture".into(),
            label: "Fixture".into(),
            binding,
            allowed_purposes: vec![CatalogPurpose::Translation, CatalogPurpose::Grammar],
            interface_label: "HTTP".into(),
            interface_description: "fixture".into(),
            interface_choice_required: false,
            default_auth_kind: CatalogAuthKind::BearerApiKey,
            default_endpoint: None,
            fields,
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
        offline_engines: vec![OfflineEngineDescriptor {
            engine_id: "argos_translate".into(),
            contract_revision: 1,
            label: "Argos".into(),
            brand_id: "argos".into(),
            brand_label: "Argos".into(),
            backend: OfflineExecutionBackend::LocalWorker,
            allowed_purposes: vec![CatalogPurpose::Translation],
            model_identity: CatalogModelIdentity::NotApplicable,
            fields: vec![
                field("name", ServiceFieldType::Text),
                field("source_language", ServiceFieldType::Text),
                field("target_language", ServiceFieldType::Text),
                field("installation_path", ServiceFieldType::Path),
                select(
                    "route_policy",
                    &["direct_only", "pivot"],
                    Some("direct_only"),
                ),
            ],
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
    })
}
fn empty() -> ConfigurationDocument {
    ConfigurationDocument::Legacy {
        configuration: Configuration {
            revision_id: "empty".into(),
            ..Default::default()
        },
    }
}
#[test]
fn retry_fingerprint_is_stable_across_new_graph_ids_but_changes_for_input_or_resources() {
    let request = input(vec![]);
    let ids = Ids::new();
    let prepared = PreparedServiceDraftResources::default();
    let first = compile_service_draft(&empty(), &request, &directory(), &ids, &prepared).unwrap();
    let retry = compile_service_draft(&empty(), &request, &directory(), &ids, &prepared).unwrap();
    assert_ne!(first.graph.revision_id, retry.graph.revision_id);
    assert_eq!(first.request_hash, retry.request_hash);
    let mut changed = request.clone();
    changed.values.insert("name".into(), text("different service"));
    assert_ne!(first.request_hash, service_draft_request_hash(&changed, &prepared).unwrap());
    changed = request.clone();
    changed.expected_draft_head_revision += 1;
    assert_ne!(first.request_hash, service_draft_request_hash(&changed, &prepared).unwrap());
    let changed_resources = PreparedServiceDraftResources {
        credential: Some(PreparedCredentialDraft {
            pool_id: "pool-v2".into(),
            pool_revision: NonZeroU32::new(2).unwrap(),
            multiple_keys: false,
            public_identifiers: BTreeMap::new(),
        }),
        ..Default::default()
    };
    assert_ne!(first.request_hash, service_draft_request_hash(&request, &changed_resources).unwrap());
}
#[test]
fn first_save_compiles_a_truly_unsaved_profile_without_inventing_a_source_revision() {
    let base = ConfigurationDocument::Legacy { configuration: Configuration::default() };
    let compiled = compile_service_draft(&base, &input(vec![]), &directory(), &Ids::new(), &Default::default()).unwrap();
    assert!(!compiled.graph.revision_id.is_empty());
    assert!(compiled.graph.connections.iter().all(|connection| connection.materialization_source.is_none()));
    assert!(compiled.graph.models.iter().all(|model| model.legacy_eligibility_origin.is_none()));
    assert!(compiled.graph.bindings.is_empty());
    let mut invalid = Configuration::default();
    invalid.models.push(serde_json::from_value(json!({"modelId":"orphan","connectionId":"missing","actualModelId":null,"displayName":"orphan","selectionSource":"manual","capabilities":{},"parameters":{},"defaultTemplates":{}})).unwrap());
    assert!(compile_service_draft(&ConfigurationDocument::Legacy { configuration: invalid }, &input(vec![]), &directory(), &Ids::new(), &Default::default()).is_err());
}
fn input(values: Vec<(&str, CatalogScalar)>) -> SaveServiceDraftInput {
    SaveServiceDraftInput {
        operation_id: "op".into(),
        expected_profile_generation: NonZeroU32::new(4).unwrap(),
        expected_draft_head_revision: 7,
        editing: None,
        target: ServiceFormTarget::Http {
            preset: ProviderPresetKey {
                preset_id: "user.fixture".into(),
                preset_revision: 1,
            },
        },
        purpose: CatalogPurpose::Translation,
        values: values.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        authentication: ServiceAuthenticationSelection::None {},
    }
}
fn compile(
    base: &ConfigurationDocument,
    input: &SaveServiceDraftInput,
) -> AppResult<app_core::ports::service_configuration::CompiledServiceDraft> {
    compile_service_draft(base, input, &directory(), &Ids::new(), &Default::default())
}

#[test]
fn incomplete_save_is_pure_and_does_not_create_bindings_or_credentials() {
    let base = empty();
    let original = base.clone();
    let r = compile(&base, &input(vec![])).unwrap();
    assert_eq!(base, original);
    assert!(r.graph.bindings.is_empty());
    assert!(r.graph.templates.is_empty());
    assert!(r.graph.offline_engine_configs.is_empty());
    assert_eq!(r.graph.models[0].actual_model_id, None);
    assert_eq!(
        r.graph.models[0].template_purpose,
        Some(CatalogPurpose::Translation)
    );
    assert_eq!(r.graph.connections[0].credential_pool_id, None);
    assert_eq!(r.expected_profile_generation.get(), 4);
    assert_eq!(r.expected_draft_head_revision, 7);
    assert_eq!(r.operation_id, "op");
    let d = &r.graph.models[0].request_defaults[&CatalogPurpose::Translation];
    assert_eq!(d.execution.timeout_ms, 60000);
    assert!(!d.context.enabled);
    assert!(matches!(
        d.languages,
        ServiceLanguageDefaults::Http {
            source: SourceLanguageChoice::InheritSource {},
            target: TargetLanguageChoice::InheritPreference {}
        }
    ));
}
#[test]
fn defaults_parameters_and_incomplete_template_have_separate_owners() {
    let r = compile(
        &empty(),
        &input(vec![
            ("temperature_mode", text("set")),
            ("temperature", CatalogScalar::Number(0.0)),
            ("max_tokens", CatalogScalar::Number(2048.0)),
            ("timeout", CatalogScalar::Number(1.25)),
            ("concurrency", CatalogScalar::Number(3.0)),
            ("retries", CatalogScalar::Number(0.0)),
            ("context_count", CatalogScalar::Number(0.0)),
            ("cache", CatalogScalar::Boolean(false)),
            ("source_language", text("ja")),
            ("target_language", text("zh-Hans")),
            ("template", text("{{unfinished")),
            ("organization_id", text("org-fixture")),
            ("openai_project_id", text("project-fixture")),
        ]),
    )
    .unwrap();
    let m = &r.graph.models[0];
    assert_eq!(r.graph.templates[0].source_kind, "custom");
    let d = &m.request_defaults[&CatalogPurpose::Translation];
    assert_eq!(
        d.execution,
        ServiceExecutionDefaults {
            timeout_ms: 1250,
            concurrency: 3,
            retries: 0
        }
    );
    assert!(!d.cache.enabled);
    assert_eq!(d.context.max_sentences, 0);
    assert_eq!(
        m.parameters["temperature"],
        GenerationParameter::Set { value: json!(0.0) }
    );
    assert_eq!(
        m.parameters["max_tokens"],
        GenerationParameter::Set { value: json!(2048) }
    );
    assert_eq!(r.graph.templates[0].body["user"], json!("{{unfinished"));
    assert!(r.graph.connections[0].network_metadata.is_empty());
    assert_eq!(
        r.graph.connections[0].http_options,
        Some(HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: Some("org-fixture".into()),
            project_id: Some("project-fixture".into())
        })
    );
    let omitted = compile(&empty(), &input(vec![("temperature_mode", text("omit"))])).unwrap();
    assert_eq!(
        omitted.graph.models[0].parameters["temperature"],
        GenerationParameter::Omit {}
    );
}
#[test]
fn malformed_fields_and_protected_inputs_are_not_silently_defaulted() {
    for (id, v) in [
        ("timeout", CatalogScalar::Number(0.5)),
        ("context_count", CatalogScalar::Number(51.0)),
        ("cache", text("false")),
        ("temperature_mode", text("unknown")),
        ("endpoint", text("https://user:secret@example.invalid")),
    ] {
        let e = compile(&empty(), &input(vec![(id, v)])).err().unwrap();
        assert_eq!(e.code, "invalid_service_field");
        assert!(e.message.contains(id));
        assert!(!e.message.contains("secret"));
    }
    assert_eq!(
        compile(&empty(), &input(vec![("api_key", text("must-not-leak"))]))
            .err()
            .unwrap()
            .code,
        "protected_form_field"
    );
    assert_eq!(
        compile(&empty(), &input(vec![("unknown", text("x"))]))
            .err()
            .unwrap()
            .code,
        "unknown_form_field"
    );
    assert_eq!(
        compile(&empty(), &input(vec![("temperature_mode", text("set"))]))
            .err()
            .unwrap()
            .code,
        "generation_value_required"
    );
}
#[test]
fn newly_registered_fields_fail_closed_and_misplaced_options_are_not_discarded() {
    let mut d = directory();
    d.0.presets[0]
        .fields
        .push(field("future_option", ServiceFieldType::Text));
    let e = compile_service_draft(
        &empty(),
        &input(vec![]),
        &d,
        &Ids::new(),
        &Default::default(),
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "unmapped_service_field");
    let mut d = directory();
    d.0.presets[0]
        .fields
        .push(field("workspace_id", ServiceFieldType::Text));
    let e = compile_service_draft(
        &empty(),
        &input(vec![("workspace_id", text("anthropic-option"))]),
        &d,
        &Ids::new(),
        &Default::default(),
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "inapplicable_service_field");
    assert!(e.message.contains("workspace_id"));
    let e = compile(
        &empty(),
        &input(vec![("source_language", text("ja\ninvalid"))]),
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "invalid_service_field");
    assert!(e.message.contains("source_language"));
}
#[test]
fn credential_pool_ui_intent_requires_matching_nonsecret_preparation() {
    let mut d = directory();
    d.0.presets[0]
        .fields
        .push(field("key_pool", ServiceFieldType::Boolean));
    let mut req = input(vec![("key_pool", CatalogScalar::Boolean(false))]);
    req.authentication = ServiceAuthenticationSelection::ExistingPool {
        pool_id: "owned-pool".into(),
        pool_revision: NonZeroU32::new(2).unwrap(),
    };
    assert_eq!(
        compile_service_draft(&empty(), &req, &d, &Ids::new(), &Default::default())
            .err()
            .unwrap()
            .code,
        "credential_preparation_required"
    );
    let mut prepared = PreparedServiceDraftResources {
        credential: Some(PreparedCredentialDraft {
            pool_id: "wrong-pool".into(),
            pool_revision: NonZeroU32::new(2).unwrap(),
            multiple_keys: false,
            public_identifiers: BTreeMap::new(),
        }),
        ..Default::default()
    };
    assert_eq!(
        compile_service_draft(&empty(), &req, &d, &Ids::new(), &prepared)
            .err()
            .unwrap()
            .code,
        "credential_preparation_mismatch"
    );
    prepared.credential.as_mut().unwrap().pool_id = "owned-pool".into();
    let r = compile_service_draft(&empty(), &req, &d, &Ids::new(), &prepared).unwrap();
    assert_eq!(
        r.graph.connections[0].credential_pool_id.as_deref(),
        Some("owned-pool")
    );
    assert_eq!(
        r.graph.connections[0]
            .credential_pool_revision
            .unwrap()
            .get(),
        2
    );
    assert!(
        !serde_json::to_string(&r.graph)
            .unwrap()
            .contains("key_pool")
    );
}
#[test]
fn fallback_is_typed_ordered_and_arbitrary_strings_or_duplicate_keys_fail() {
    let refs = r#"[{"kind":"offline","source":{"revisionId":"old-offline","configId":"offline-a"}},{"kind":"http","source":{"revisionId":"old-http","modelId":"http-b"}}]"#;
    let r = compile(&empty(), &input(vec![("fallback", text(refs))])).unwrap();
    let f = &r.graph.models[0].request_defaults[&CatalogPurpose::Translation].fallback;
    assert!(matches!(&f[0],SavedServiceRef::Offline{source} if source.config_id=="offline-a"));
    assert!(matches!(&f[1],SavedServiceRef::Http{source} if source.model_id=="http-b"));
    for bad in [
        "some-provider",
        r#"[{"kind":"http","source":{"revisionId":"a","revisionId":"b","modelId":"c"}}]"#,
    ] {
        assert_eq!(
            compile(&empty(), &input(vec![("fallback", text(bad))]))
                .err()
                .unwrap()
                .code,
            "invalid_fallback_sources"
        );
    }
}
#[test]
fn edit_requires_exact_source_and_keeps_unexposed_values_and_other_models() {
    let first = compile(&empty(), &input(vec![])).unwrap();
    let mut graph = first.graph;
    let original = graph.models[0].model_id.clone();
    graph.models[0].parameters.insert(
        "top_p".into(),
        GenerationParameter::Set {
            value: json!(false),
        },
    );
    graph.models[0]
        .capabilities
        .insert("streaming".into(), json!(false));
    graph.connections[0].availability_intent = false;
    graph.connections[0].proxy_mode = "disabled".into();
    graph.connections[0]
        .network_metadata
        .insert("timeoutMs".into(), json!(8000));
    let mut req = input(vec![("name", text("changed"))]);
    req.editing = Some(SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: "stale".into(),
            model_id: original.clone(),
        },
    });
    let base = ConfigurationDocument::ServiceGraph {
        configuration: graph.clone(),
    };
    assert_eq!(
        compile(&base, &req).err().unwrap().code,
        "stale_service_edit"
    );
    req.editing = Some(SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: graph.revision_id.clone(),
            model_id: original.clone(),
        },
    });
    // A distinct IdentitySource sequence prevents collision with the source revision.
    let result = compile_service_draft(
        &base,
        &req,
        &directory(),
        &Ids(AtomicUsize::new(100)),
        &Default::default(),
    )
    .unwrap();
    assert_eq!(result.graph.models[0].model_id, original);
    assert_eq!(
        result.graph.models[0].parameters["top_p"],
        GenerationParameter::Set {
            value: json!(false)
        }
    );
    assert_eq!(
        result.graph.models[0].capabilities["streaming"],
        json!(false)
    );
    assert!(!result.graph.connections[0].availability_intent);
    assert_eq!(result.graph.connections[0].proxy_mode, "disabled");
    assert_eq!(
        result.graph.connections[0].network_metadata,
        graph.connections[0].network_metadata
    );
    assert_eq!(result.graph.bindings, graph.bindings);
    assert_eq!(
        base,
        ConfigurationDocument::ServiceGraph {
            configuration: graph
        }
    );
}
#[test]
fn shared_connection_changes_are_explicitly_rejected() {
    let first = compile(&empty(), &input(vec![])).unwrap();
    let mut g = first.graph;
    let mut sibling = g.models[0].clone();
    sibling.model_id = "sibling".into();
    g.models.push(sibling);
    let mut req = input(vec![("endpoint", text("https://changed.example.invalid"))]);
    req.editing = Some(SavedServiceRef::Http {
        source: HttpConfigRef {
            revision_id: g.revision_id.clone(),
            model_id: g.models[0].model_id.clone(),
        },
    });
    assert_eq!(
        compile(
            &ConfigurationDocument::ServiceGraph { configuration: g },
            &req
        )
        .err()
        .unwrap()
        .code,
        "shared_connection_review_required"
    );
}
#[test]
fn offline_saves_no_fake_http_objects_and_paths_require_preparation() {
    let mut req = input(vec![
        ("source_language", text("ja")),
        ("target_language", text("zh")),
    ]);
    req.target = ServiceFormTarget::Offline {
        engine_id: "argos_translate".into(),
        contract_revision: 1,
    };
    let draft = compile(&empty(), &req).unwrap();
    assert!(draft.graph.connections.is_empty());
    assert!(draft.graph.models.is_empty());
    assert!(draft.graph.templates.is_empty());
    assert_eq!(draft.graph.offline_engine_configs.len(), 1);
    assert!(matches!(
        draft.graph.offline_engine_configs[0].resolution,
        OfflineConfigResolution::Draft { .. }
    ));
    req.values
        .insert("installation_path".into(), text("/fixture/argos"));
    assert_eq!(
        compile(&empty(), &req).err().unwrap().code,
        "offline_preparation_required"
    );
    let prepared = PreparedServiceDraftResources {
        offline: Some(PreparedOfflineDraft {
            engine: OfflineEngineKey {
                engine_id: "argos_translate".into(),
                contract_revision: NonZeroU32::new(1).unwrap(),
            },
            pair: LanguagePair {
                source: "ja".into(),
                target: "zh".into(),
            },
            prepared_values: BTreeMap::from([("installation_path".into(), text("/fixture/argos"))]),
            installation: LocalInstallationRef {
                installation_id: "fixture-install".into(),
                revision: NonZeroU32::new(1).unwrap(),
            },
            resolution: OfflineConfigResolution::Draft {
                observed_resource: None,
                proposed_route: None,
            },
            pivot_consent_revision: None,
        }),
        ..Default::default()
    };
    let r = compile_service_draft(&empty(), &req, &directory(), &Ids::new(), &prepared).unwrap();
    assert_eq!(
        r.graph.offline_engine_configs[0]
            .installation
            .as_ref()
            .unwrap()
            .installation_id,
        "fixture-install"
    );
    assert!(
        !serde_json::to_string(&r.graph)
            .unwrap()
            .contains("/fixture/argos")
    );
    req.values.remove("source_language");
    assert_eq!(
        compile(&empty(), &req).err().unwrap().code,
        "offline_language_pair_required"
    );
}
#[test]
fn legacy_materialization_preserves_policy_sequence_templates_and_unsupported_objects() {
    let c:Configuration=serde_json::from_value(json!({"revisionId":"legacy-real-source","codecVersion":2,"connections":[{"connectionId":"c","displayName":"unknown preserved","protocol":"unsupported","deployment":"cloud","authMode":"none","endpoint":null,"credentialPoolId":null,"credentialPoolRevision":null,"proxyMode":"inherit","networkMetadata":{},"availabilityIntent":false}],"models":[{"modelId":"m","connectionId":"c","actualModelId":null,"displayName":"old model","selectionSource":"manual","capabilities":{"streaming":false},"parameters":{"top_p":{"mode":"set","value":0}},"defaultTemplates":{"translation":"t"}}],"bindings":[{"bindingId":"b","scopeKey":"global","gameId":null,"purpose":"translation","bindingMode":"override","enabled":false,"outputMode":"comparison","sourceLanguage":"ja","targetLanguage":"zh","templateId":"t","context":{"enabled":false,"maxSentences":0},"schedule":{"retries":0,"timeoutMs":4500},"parameters":{"sample":{"mode":"set","value":false}},"engines":[{"engineId":"z","modelId":"m","enabled":false,"templateId":"t","parameters":{}},{"engineId":"a","modelId":"m","enabled":true,"templateId":null,"parameters":{}}]}],"templates":[{"templateId":"t","purpose":"translation","displayName":"old template","sourceKind":"user","sourceVersion":null,"body":{"user":"{{source}}"},"variables":["source"],"compatibility":{}}],"termSets":[{"termSetId":"terms","scopeKey":"global","gameId":null,"displayName":"terms","enabled":false,"terms":[{"termId":"z","sourceText":"first","targetText":"一","sourceLanguage":"ja","targetLanguage":"zh","enabled":true},{"termId":"a","sourceText":"second","targetText":"二","sourceLanguage":"ja","targetLanguage":"zh","enabled":false}]}]})).unwrap();
    let base = ConfigurationDocument::Legacy {
        configuration: c.clone(),
    };
    let g = materialize_base(&base).unwrap();
    assert_eq!(
        g.bindings[0].semantics_version,
        PurposeSemanticsVersion::LegacyV2
    );
    assert_eq!(g.bindings[0].context, c.bindings[0].context);
    assert_eq!(g.bindings[0].schedule, c.bindings[0].schedule);
    assert_eq!(
        g.bindings[0]
            .engines
            .iter()
            .map(|e| e.engine_id.as_str())
            .collect::<Vec<_>>(),
        vec!["z", "a"]
    );
    assert_eq!(
        g.term_sets[0]
            .terms
            .iter()
            .map(|t| t.term_id.as_str())
            .collect::<Vec<_>>(),
        vec!["z", "a"]
    );
    assert_eq!(g.templates[0].body, c.templates[0].body);
    assert_eq!(g.connections[0].protocol, "unsupported");
    assert_eq!(
        g.connections[0]
            .materialization_source
            .as_ref()
            .unwrap()
            .revision_id,
        "legacy-real-source"
    );
    assert!(g.connections[0].http_options.is_none());
    assert!(g.models[0].template_purpose.is_none());
    assert!(g.models[0].request_defaults.is_empty());
    let r = compile(&base, &input(vec![])).unwrap();
    assert_eq!(r.graph.bindings, g.bindings);
    assert_eq!(r.graph.connections[0], g.connections[0]);
    assert_eq!(r.graph.models[0], g.models[0]);
    assert_eq!(r.graph.term_sets, g.term_sets);
    assert_eq!(base, ConfigurationDocument::Legacy { configuration: c });
}
