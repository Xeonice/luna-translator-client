//! Actual built-in directory -> draft -> editor -> saved edit, with no network,
//! Vault or installation execution. Opaque fixture facts cannot prove live readiness.
use app_core::{
    contracts::*,
    domain::model_config::{service_draft::*, service_editor::*, service_graph::*},
    ports::{IdentitySource, service_configuration::ServiceDirectory},
};
use app_infra::providers::{builtin_registry, registry::ProviderRegistry};
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
        panic!("pure projection has no clock")
    }
}
fn text(s: &str) -> CatalogScalar {
    CatalogScalar::Text(s.into())
}
fn empty() -> ConfigurationDocument {
    ConfigurationDocument::Legacy {
        configuration: Configuration {
            revision_id: "empty".into(),
            ..Default::default()
        },
    }
}
fn input(target: ServiceFormTarget) -> SaveServiceDraftInput {
    SaveServiceDraftInput {
        operation_id: "create".into(),
        expected_profile_generation: NonZeroU32::new(2).unwrap(),
        expected_draft_head_revision: 0,
        editing: None,
        target,
        purpose: CatalogPurpose::Translation,
        values: BTreeMap::new(),
        authentication: ServiceAuthenticationSelection::None {},
    }
}
fn snapshot(g: ConfigurationV3) -> ServiceConfigurationSnapshot {
    ServiceConfigurationSnapshot {
        profile_generation: NonZeroU32::new(2).unwrap(),
        draft_head_revision: 4,
        applied_head_revision: 3,
        draft: ConfigurationDocument::ServiceGraph { configuration: g },
        applied: None,
        runtime_purposes: vec![],
        runtime_connections: vec![],
    }
}
fn saved_ref(g: &ConfigurationV3) -> SavedServiceRef {
    if let Some(m) = g.models.first() {
        SavedServiceRef::Http {
            source: HttpConfigRef {
                revision_id: g.revision_id.clone(),
                model_id: m.model_id.clone(),
            },
        }
    } else {
        SavedServiceRef::Offline {
            source: OfflineConfigRef {
                revision_id: g.revision_id.clone(),
                config_id: g.offline_engine_configs[0].id.clone(),
            },
        }
    }
}
fn editor(
    g: &ConfigurationV3,
    registry: &ProviderRegistry,
    facts: &ServiceEditorFacts,
) -> ServiceDraftEditorProjection {
    let current = snapshot(g.clone());
    project_service_editor(
        &ServiceDraftQuery {
            expected_profile_generation: current.profile_generation,
            source: saved_ref(g),
            purpose: CatalogPurpose::Translation,
        },
        &current.draft,
        &current,
        registry,
        facts,
    )
    .unwrap()
}
fn edit(p: ServiceDraftEditorProjection) -> SaveServiceDraftInput {
    assert!(p.readonly_reason.is_none(), "{:?}", p.readonly_reason);
    let mut values = p.values;
    assert!(
        !values.contains_key("fallback"),
        "UI must receive typed fallback, not an editable JSON text field"
    );
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
fn assert_resource_equal(old: &ConfigurationV3, new: &ConfigurationV3) {
    assert_eq!(old.models, new.models);
    assert_eq!(old.connections, new.connections);
    assert_eq!(old.templates, new.templates);
    assert_eq!(old.offline_engine_configs, new.offline_engine_configs);
    assert_eq!(old.bindings, new.bindings);
    assert_eq!(old.term_sets, new.term_sets);
    assert_ne!(old.revision_id, new.revision_id);
}
fn representative(registry: &ProviderRegistry, target: ServiceFormTarget) -> SaveServiceDraftInput {
    let mut req = input(target);
    let form = registry
        .form(&ServiceFormRequest {
            target: req.target.clone(),
            purpose: req.purpose,
            values: BTreeMap::new(),
        })
        .unwrap();
    let candidates = BTreeMap::from([
        ("name", text("实际保存的服务")),
        ("source_language", text("ja")),
        ("target_language", text("zh-Hans")),
        ("context_count", CatalogScalar::Number(0.0)),
        ("timeout", CatalogScalar::Number(1.25)),
        ("concurrency", CatalogScalar::Number(2.0)),
        ("retries", CatalogScalar::Number(0.0)),
        ("cache", CatalogScalar::Boolean(false)),
        ("temperature_mode", text("omit")),
        ("max_tokens", CatalogScalar::Number(1024.0)),
        ("template", text("待完成模板 {{source}}")),
    ]);
    for (id, value) in candidates {
        if form
            .fields
            .iter()
            .any(|r| r.descriptor.id == id && r.input_allowed)
        {
            req.values.insert(id.into(), value);
        }
    }
    req
}
#[test]
fn all_21_presets_and_4_offline_services_roundtrip_exact_resource_semantics() {
    let registry = builtin_registry().unwrap();
    let catalog = registry.catalog();
    let targets = catalog
        .presets
        .iter()
        .map(|p| ServiceFormTarget::Http {
            preset: p.key.clone(),
        })
        .chain(
            catalog
                .offline_engines
                .iter()
                .map(|e| ServiceFormTarget::Offline {
                    engine_id: e.engine_id.clone(),
                    contract_revision: e.contract_revision,
                }),
        );
    let mut count = 0;
    for target in targets {
        let req = representative(&registry, target.clone());
        let mut g =
            compile_service_draft(&empty(), &req, &registry, &Ids::at(0), &Default::default())
                .unwrap_or_else(|e| panic!("create {target:?}: {e}"))
                .graph;
        if let Some(model) = g.models.first_mut() {
            let d = model
                .request_defaults
                .get_mut(&CatalogPurpose::Translation)
                .unwrap();
            d.context.mode = HistoryInputMode::Both;
            model
                .capabilities
                .insert("streaming".into(), serde_json::json!(false));
            model.selection_source = "saved-source".into();
            model
                .parameters
                .insert("max_tokens".into(), GenerationParameter::Omit {});
            for t in &mut g.templates {
                t.body.insert(
                    "system".into(),
                    serde_json::json!("保留隐藏system {{history}}"),
                );
                t.compatibility
                    .insert("preserve".into(), serde_json::json!(false));
                t.variables.push("history".into());
            }
        } else {
            let o = &mut g.offline_engine_configs[0];
            o.request_defaults.context = ServiceContextConsent {
                enabled: false,
                max_sentences: 9,
                mode: HistoryInputMode::Translation,
            };
        }
        let p = editor(&g, &registry, &Default::default());
        assert_eq!(p.target, Some(target.clone()));
        assert_eq!(p.editing, Some(saved_ref(&g)));
        let projected = registry
            .form(&ServiceFormRequest {
                target: target.clone(),
                purpose: p.purpose,
                values: p.values.clone(),
            })
            .unwrap();
        for id in p.values.keys() {
            let row = projected
                .fields
                .iter()
                .find(|r| &r.descriptor.id == id)
                .unwrap();
            assert!(row.input_allowed);
            assert_eq!(row.visibility, CapabilityState::Supported);
            assert!(!matches!(
                row.descriptor.field_type,
                ServiceFieldType::Secret | ServiceFieldType::Action | ServiceFieldType::Status
            ));
        }
        let snap = snapshot(g.clone());
        let updated = compile_service_draft(
            &snap.draft,
            &edit(p),
            &registry,
            &Ids::at(100),
            &Default::default(),
        )
        .unwrap_or_else(|e| panic!("edit {target:?}: {e}"))
        .graph;
        assert_resource_equal(&g, &updated);
        count += 1;
    }
    assert_eq!(count, 25);
}
#[test]
fn custom_site_options_real_model_temperature_zero_and_typed_fallback_survive() {
    let registry = builtin_registry().unwrap();
    let catalog = registry.catalog();
    let preset = catalog
        .presets
        .iter()
        .find(|p| p.key.preset_id == "builtin.qwen")
        .unwrap();
    let mut req = representative(
        &registry,
        ServiceFormTarget::Http {
            preset: preset.key.clone(),
        },
    );
    req.values.extend(BTreeMap::from([("connection_mode".into(),text("custom")),("provider_site".into(),text("cn-beijing")),("endpoint".into(),text("http://100.97.190.45:1234/v1")),("model".into(),text("actual-model")),("temperature_mode".into(),text("set")),("temperature".into(),CatalogScalar::Number(0.0)),("fallback".into(),text(r#"[{"kind":"offline","source":{"revisionId":"external-offline","configId":"offline-fallback"}}]"#))]));
    let g = compile_service_draft(&empty(), &req, &registry, &Ids::at(0), &Default::default())
        .unwrap()
        .graph;
    let p = editor(&g, &registry, &Default::default());
    assert_eq!(p.values["temperature"], CatalogScalar::Number(0.0));
    assert_eq!(p.values["provider_site"], text("cn-beijing"));
    assert!(!p.values.contains_key("fallback"));
    assert_eq!(p.fallback.len(), 1);
    let snap = snapshot(g.clone());
    let result = compile_service_draft(
        &snap.draft,
        &edit(p),
        &registry,
        &Ids::at(100),
        &Default::default(),
    )
    .unwrap()
    .graph;
    assert_resource_equal(&g, &result);
}
#[test]
fn opaque_credentials_are_retained_and_public_facts_follow_rust_capture() {
    let registry = builtin_registry().unwrap();
    let catalog = registry.catalog();
    for preset_id in [
        "builtin.openai-chat",
        "builtin.baidu",
        "builtin.youdao",
        "builtin.google-v3",
    ] {
        let preset = catalog
            .presets
            .iter()
            .find(|p| p.key.preset_id == preset_id)
            .unwrap();
        let mut req = representative(
            &registry,
            ServiceFormTarget::Http {
                preset: preset.key.clone(),
            },
        );
        if preset_id == "builtin.google-v3" {
            req.values
                .insert("google_auth_method".into(), text("service_account"));
        }
        req.authentication = ServiceAuthenticationSelection::ExistingPool {
            pool_id: "owned-pool".into(),
            pool_revision: NonZeroU32::new(3).unwrap(),
        };
        let mut facts = ServiceEditorFacts {
            credential: Some(PreparedCredentialDraft {
                pool_id: "owned-pool".into(),
                pool_revision: NonZeroU32::new(3).unwrap(),
                multiple_keys: false,
                public_identifiers: BTreeMap::from([
                    ("app_id".into(), "public-app-id".into()),
                    ("app_key".into(), "public-app-key".into()),
                    ("api_key".into(), "must-never-be-exposed".into()),
                ]),
            }),
            ..Default::default()
        };
        let g = compile_service_draft(&empty(), &req, &registry, &Ids::at(0), &facts)
            .unwrap()
            .graph;
        let missing = editor(&g, &registry, &Default::default());
        assert!(missing.readonly_reason.as_ref().unwrap().contains("凭据组"));
        assert_eq!(missing.authentication, req.authentication);
        assert!(missing.editing.is_none());
        let p = editor(&g, &registry, &facts);
        assert!(
            p.readonly_reason.is_none(),
            "{preset_id}: {:?}",
            p.readonly_reason
        );
        assert!(
            !serde_json::to_string(&p)
                .unwrap()
                .contains("must-never-be-exposed")
        );
        let form = registry
            .form(&ServiceFormRequest {
                target: p.target.clone().unwrap(),
                purpose: p.purpose,
                values: p.values.clone(),
            })
            .unwrap();
        assert!(matches!(
            form.credential_capture,
            ServiceCredentialCapture::ApiKey
                | ServiceCredentialCapture::BaiduAppSecret
                | ServiceCredentialCapture::YoudaoAppSecret
                | ServiceCredentialCapture::GoogleServiceAccountFile
        ));
        let snap = snapshot(g.clone());
        let r = compile_service_draft(&snap.draft, &edit(p), &registry, &Ids::at(100), &facts)
            .unwrap()
            .graph;
        assert_resource_equal(&g, &r);
        facts.credential.as_mut().unwrap().pool_revision = NonZeroU32::new(4).unwrap();
        assert_eq!(
            project_service_editor(
                &ServiceDraftQuery {
                    expected_profile_generation: NonZeroU32::new(2).unwrap(),
                    source: saved_ref(&g),
                    purpose: CatalogPurpose::Translation
                },
                &snap.draft,
                &snap,
                &registry,
                &facts
            )
            .unwrap_err()
            .code,
            "service_editor_fact_mismatch"
        );
    }
}
#[test]
fn google_owned_client_and_offline_resources_require_exact_nonsecret_facts() {
    let registry = builtin_registry().unwrap();
    let catalog = registry.catalog();
    let preset = catalog
        .presets
        .iter()
        .find(|p| p.key.preset_id == "builtin.google-v3")
        .unwrap();
    let mut req = representative(
        &registry,
        ServiceFormTarget::Http {
            preset: preset.key.clone(),
        },
    );
    req.values.extend(BTreeMap::from([
        ("google_auth_method".into(), text("oauth")),
        ("oauth_client_mode".into(), text("custom")),
        ("oauth_client_id".into(), text("public-client")),
    ]));
    let authorization = GoogleAuthorizationRef {
        grant_id: "owned-grant".into(),
        authorization_epoch: NonZeroU32::new(2).unwrap(),
    };
    req.authentication = ServiceAuthenticationSelection::GoogleOwnedGrant {
        authorization: authorization.clone(),
    };
    let facts = ServiceEditorFacts {
        google_client: Some(PreparedGoogleClientDraft {
            mode: "custom".into(),
            client_id: Some("public-client".into()),
            authorization,
        }),
        ..Default::default()
    };
    let g = compile_service_draft(&empty(), &req, &registry, &Ids::at(0), &facts)
        .unwrap()
        .graph;
    let p = editor(&g, &registry, &facts);
    assert_eq!(p.values["oauth_client_mode"], text("custom"));
    assert_eq!(p.values["oauth_client_id"], text("public-client"));
    let snap = snapshot(g.clone());
    let r = compile_service_draft(&snap.draft, &edit(p), &registry, &Ids::at(100), &facts)
        .unwrap()
        .graph;
    assert_resource_equal(&g, &r);
    assert!(
        editor(&g, &registry, &Default::default())
            .readonly_reason
            .unwrap()
            .contains("Google")
    );
    let mut req = representative(
        &registry,
        ServiceFormTarget::Offline {
            engine_id: "jbeijing".into(),
            contract_revision: 1,
        },
    );
    req.values.extend(BTreeMap::from([
        ("installation_path".into(), text("/fixture/jbeijing")),
        ("runtime".into(), text("wine")),
        ("bottle".into(), text("FixtureBottle")),
    ]));
    let facts = ServiceEditorFacts {
        offline: Some(PreparedOfflineDraft {
            engine: OfflineEngineKey {
                engine_id: "jbeijing".into(),
                contract_revision: NonZeroU32::new(1).unwrap(),
            },
            pair: LanguagePair {
                source: "ja".into(),
                target: "zh-Hans".into(),
            },
            prepared_values: BTreeMap::from([
                ("installation_path".into(), text("/fixture/jbeijing")),
                ("runtime".into(), text("wine")),
                ("bottle".into(), text("FixtureBottle")),
            ]),
            installation: LocalInstallationRef {
                installation_id: "existing-installation".into(),
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
    let g = compile_service_draft(&empty(), &req, &registry, &Ids::at(0), &facts)
        .unwrap()
        .graph;
    let p = editor(&g, &registry, &facts);
    assert_eq!(p.values["installation_path"], text("/fixture/jbeijing"));
    let snap = snapshot(g.clone());
    let r = compile_service_draft(&snap.draft, &edit(p), &registry, &Ids::at(100), &facts)
        .unwrap()
        .graph;
    assert_resource_equal(&g, &r);
    assert!(
        editor(&g, &registry, &Default::default())
            .readonly_reason
            .unwrap()
            .contains("安装")
    );
}
#[test]
fn explicit_empty_drafts_and_numeric_storage_representation_are_not_defaulted_away() {
    let registry = builtin_registry().unwrap();
    let catalog = registry.catalog();
    let preset = catalog
        .presets
        .iter()
        .find(|p| p.key.preset_id == "builtin.openai-chat")
        .unwrap();
    let mut req = representative(
        &registry,
        ServiceFormTarget::Http {
            preset: preset.key.clone(),
        },
    );
    req.values.extend(BTreeMap::from([
        ("connection_mode".into(), text("custom")),
        ("endpoint".into(), text("")),
        ("temperature_mode".into(), text("set")),
        ("temperature".into(), CatalogScalar::Number(0.0)),
    ]));
    let mut g = compile_service_draft(&empty(), &req, &registry, &Ids::at(0), &Default::default())
        .unwrap()
        .graph;
    assert!(g.connections[0].endpoint.is_none());
    g.models[0].parameters.insert(
        "temperature".into(),
        GenerationParameter::Set {
            value: serde_json::json!(0),
        },
    );
    g.models[0].parameters.insert(
        "max_tokens".into(),
        GenerationParameter::Set {
            value: serde_json::json!(1024.0),
        },
    );
    g.templates[0]
        .body
        .insert("user".into(), serde_json::json!(""));
    let p = editor(&g, &registry, &Default::default());
    assert_eq!(p.values["endpoint"], text(""));
    assert_eq!(p.values["template"], text(""));
    let snap = snapshot(g.clone());
    let result = compile_service_draft(
        &snap.draft,
        &edit(p),
        &registry,
        &Ids::at(100),
        &Default::default(),
    )
    .unwrap()
    .graph;
    assert_resource_equal(&g, &result);
    let preset = catalog
        .presets
        .iter()
        .find(|p| p.key.preset_id == "builtin.google-v3")
        .unwrap();
    let mut req = representative(
        &registry,
        ServiceFormTarget::Http {
            preset: preset.key.clone(),
        },
    );
    req.values.insert("location".into(), text(""));
    req.values.insert("project_id".into(), text(""));
    let g = compile_service_draft(&empty(), &req, &registry, &Ids::at(0), &Default::default())
        .unwrap()
        .graph;
    let p = editor(&g, &registry, &Default::default());
    assert_eq!(p.values["location"], text(""));
    let snap = snapshot(g.clone());
    let result = compile_service_draft(
        &snap.draft,
        &edit(p),
        &registry,
        &Ids::at(100),
        &Default::default(),
    )
    .unwrap()
    .graph;
    assert_resource_equal(&g, &result);
}

#[test]
fn reviewed_http_network_delta_roundtrips_all_21_services_and_removes_inactive_proxy() {
    let registry = builtin_registry().unwrap();
    let presets = registry.catalog().presets;
    assert_eq!(presets.len(), 21);
    for preset in presets {
        let mut request = input(ServiceFormTarget::Http { preset: preset.key.clone() });
        request.values.insert("proxy_mode".into(), text("override"));
        request.values.insert("proxy_url".into(), text("http://127.0.0.1:7890"));
        let mut created = compile_service_draft(&empty(), &request, &registry, &Ids::at(0), &Default::default()).unwrap().graph;
        assert_eq!(created.connections[0].proxy_mode, "override");
        assert_eq!(created.connections[0].network_metadata["proxyUrl"], "http://127.0.0.1:7890");
        created.connections[0].network_metadata.insert("timeoutMs".into(), serde_json::json!(5000));
        let projection = editor(&created, &registry, &Default::default());
        assert_eq!(projection.values["proxy_mode"], text("override"));
        assert_eq!(projection.values["proxy_url"], text("http://127.0.0.1:7890"));
        let mut update = edit(projection);
        let base = ConfigurationDocument::ServiceGraph { configuration: created.clone() };
        let same = compile_service_draft(&base, &update, &registry, &Ids::at(100), &Default::default()).unwrap();
        assert_resource_equal(&created, &same.graph);
        update.values.insert("proxy_mode".into(), text("disabled"));
        update.values.remove("proxy_url");
        let direct = compile_service_draft(&base, &update, &registry, &Ids::at(200), &Default::default()).unwrap();
        assert_eq!(direct.graph.connections[0].proxy_mode, "disabled");
        assert!(!direct.graph.connections[0].network_metadata.contains_key("proxyUrl"));
        assert_eq!(direct.graph.connections[0].network_metadata["timeoutMs"], 5000);
        let projected_direct = editor(&direct.graph, &registry, &Default::default());
        assert_eq!(projected_direct.values["proxy_mode"], text("disabled"));
        assert!(!projected_direct.values.contains_key("proxy_url"));
    }
}

#[test]
fn network_delta_rejects_credentials_parameters_paths_and_unrecognized_modes() {
    let registry = builtin_registry().unwrap();
    let preset = registry.catalog().presets.into_iter().find(|p|p.key.preset_id=="builtin.deepseek").unwrap();
    let mut request = input(ServiceFormTarget::Http { preset: preset.key });
    request.values.insert("proxy_mode".into(), text("override"));
    for invalid in ["http://user:fixture@127.0.0.1:7890", "http://127.0.0.1:7890/?token=fixture", "http://127.0.0.1:7890/#fragment", "socks5://127.0.0.1:7890", "http://127.0.0.1:7890/path", "http://127.0.0.1:7890/ space"] {
        request.values.insert("proxy_url".into(), text(invalid));
        let error = compile_service_draft(&empty(), &request, &registry, &Ids::at(0), &Default::default()).err().expect("invalid proxy must never persist");
        assert!(matches!(error.code.as_str(), "invalid_google_proxy" | "invalid_service_field"), "unexpected failure {}", error.code);
    }
    request.values.insert("proxy_mode".into(), text("unreviewed"));
    assert_eq!(compile_service_draft(&empty(), &request, &registry, &Ids::at(0), &Default::default()).err().unwrap().code, "invalid_service_field");
    request.values.insert("proxy_mode".into(), text("inherit"));
    request.values.insert("proxy_url".into(), text("http://127.0.0.1:7890"));
    assert_eq!(compile_service_draft(&empty(), &request, &registry, &Ids::at(0), &Default::default()).err().unwrap().code, "inactive_service_field");
}

#[test]
fn independent_all_21_http_services_roundtrip_each_of_three_proxy_modes_and_all_transitions(){
 let registry=builtin_registry().unwrap();let presets=registry.catalog().presets;assert_eq!(presets.len(),21);
 for preset in presets {for from in ["inherit","disabled","override"]{let mut req=input(ServiceFormTarget::Http{preset:preset.key.clone()});req.values.insert("proxy_mode".into(),text(from));if from=="override"{req.values.insert("proxy_url".into(),text("https://127.0.0.1:7891"));}let mut graph=compile_service_draft(&empty(),&req,&registry,&Ids::at(0),&Default::default()).unwrap().graph;graph.connections[0].network_metadata.insert("timeoutMs".into(),serde_json::json!(9876));
  let projected=editor(&graph,&registry,&Default::default());assert_eq!(projected.values["proxy_mode"],text(from));assert_eq!(projected.values.contains_key("proxy_url"),from=="override");
  for to in ["inherit","disabled","override"]{let mut update=edit(projected.clone());update.values.insert("proxy_mode".into(),text(to));update.values.remove("proxy_url");if to=="override"{update.values.insert("proxy_url".into(),text("http://127.0.0.1:7892"));}let base=ConfigurationDocument::ServiceGraph{configuration:graph.clone()};let changed=compile_service_draft(&base,&update,&registry,&Ids::at(100),&Default::default()).unwrap().graph;assert_eq!(changed.connections[0].proxy_mode,to);assert_eq!(changed.connections[0].connection_id,graph.connections[0].connection_id);assert_eq!(changed.connections[0].network_metadata["timeoutMs"],9876);assert_eq!(changed.connections[0].network_metadata.contains_key("proxyUrl"),to=="override");let again=editor(&changed,&registry,&Default::default());assert_eq!(again.values["proxy_mode"],text(to));assert_eq!(again.values.contains_key("proxy_url"),to=="override");}
 }}
}
#[test]
fn independent_invalid_proxy_credentials_never_reach_safe_errors_or_proxy_debug(){
 let registry=builtin_registry().unwrap();let marker="INDEPENDENT_SECRET_DO_NOT_LOG";
 for preset in registry.catalog().presets {for url in [format!("http://user:{marker}@127.0.0.1:7890"),format!("http://127.0.0.1:7890/?token={marker}"),format!("http://127.0.0.1:7890/#{marker}")]{let mut req=input(ServiceFormTarget::Http{preset:preset.key.clone()});req.values.insert("proxy_mode".into(),text("override"));req.values.insert("proxy_url".into(),text(&url));let error=compile_service_draft(&empty(),&req,&registry,&Ids::at(0),&Default::default()).err().expect("credentialed proxy must reject");assert!(!format!("{error:?}").contains(marker));let proxy=GoogleInteractiveProxy{mode:"override".into(),proxy_url:Some(url)};assert!(!format!("{proxy:?}").contains(marker));}}
}
