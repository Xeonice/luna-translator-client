//! Real built-in descriptor coverage of the actual Core compiler. No network/resource
//! probe occurs here; prepared fixtures are non-secret references, not installed status.
use app_core::{
    contracts::*,
    domain::model_config::{service_draft::*, service_graph::*},
    ports::IdentitySource,
};
use app_infra::providers::{builtin_registry, registry::ProviderRegistry};
use std::{
    collections::{BTreeMap, BTreeSet},
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
        panic!("pure compiler has no clock")
    }
}
fn text(s: &str) -> CatalogScalar {
    CatalogScalar::Text(s.into())
}
fn empty() -> ConfigurationDocument {
    ConfigurationDocument::Legacy {
        configuration: Configuration {
            revision_id: "base".into(),
            ..Default::default()
        },
    }
}
fn input(target: ServiceFormTarget, purpose: CatalogPurpose) -> SaveServiceDraftInput {
    SaveServiceDraftInput {
        operation_id: "fixture-save".into(),
        expected_profile_generation: NonZeroU32::new(1).unwrap(),
        expected_draft_head_revision: 0,
        editing: None,
        target,
        purpose,
        values: BTreeMap::new(),
        authentication: ServiceAuthenticationSelection::None {},
    }
}
fn request(input: &SaveServiceDraftInput) -> ServiceFormRequest {
    ServiceFormRequest {
        target: input.target.clone(),
        purpose: input.purpose,
        values: input.values.clone(),
    }
}
fn editable(f: &ServiceFieldDescriptor) -> bool {
    f.id != "purpose"
        && !f.masked
        && !f.observed_only
        && !matches!(
            f.field_type,
            ServiceFieldType::Secret | ServiceFieldType::Action | ServiceFieldType::Status
        )
        && !matches!(
            f.classification,
            ServiceFieldClassification::Readonly | ServiceFieldClassification::NotApplicable
        )
}
fn sample(f: &ServiceFieldDescriptor) -> CatalogScalar {
    if f.id == "source_language" {
        return text("ja");
    }
    if f.id == "target_language" {
        return text("zh-Hans");
    }
    if f.id == "proxy_mode" { return text("disabled"); }
    if f.id == "proxy_url" { return text("http://127.0.0.1:7890"); }
    if f.field_type == ServiceFieldType::Select {
        return f
            .choices
            .last()
            .expect("explicit fixture choice")
            .value
            .clone();
    }
    match f.id.as_str() {
        "name" => text("用户实际填写的名称"),
        "model" => text("actual-fixture-model"),
        "endpoint" => text("http://100.97.190.45:1234/v1"),
        "template" => text("请翻译 {{source}}"),
        "temperature" => CatalogScalar::Number(0.0),
        "max_tokens" => CatalogScalar::Number(1024.0),
        "timeout" => CatalogScalar::Number(1.25),
        "concurrency" => CatalogScalar::Number(2.0),
        "retries" => CatalogScalar::Number(0.0),
        "context_count" => CatalogScalar::Number(0.0),
        "source_language" => text("ja"),
        "target_language" => text("zh-Hans"),
        "fallback" => text(
            r#"[{"kind":"http","source":{"revisionId":"saved-fixture","modelId":"fallback-fixture"}}]"#,
        ),
        "installation_path" => text("/fixture/offline"),
        "language_package" => text("/fixture/language-package"),
        "bottle" => text("FixtureBottle"),
        "cache" => CatalogScalar::Boolean(false),
        "key_pool" => CatalogScalar::Boolean(true),
        _ => match f.field_type {
            ServiceFieldType::Boolean => CatalogScalar::Boolean(false),
            ServiceFieldType::Number => CatalogScalar::Number(1.0),
            _ => text(&format!("fixture-{}", f.id)),
        },
    }
}
/// Supply explicit user fixture choices required to reach a row. This is test input
/// generation, never a production fallback/default or an expected compiler output.
fn activate(
    id: &str,
    fields: &[ServiceFieldDescriptor],
    values: &mut BTreeMap<String, CatalogScalar>,
    visited: &mut BTreeSet<String>,
) {
    if !visited.insert(id.into()) {
        return;
    }
    let Some(f) = fields.iter().find(|f| f.id == id) else {
        return;
    };
    for clause in f.condition.all.iter().chain(f.condition.any.first()) {
        activate(&clause.field, fields, values, visited);
        let dependency = fields.iter().find(|f| f.id == clause.field).unwrap();
        if editable(dependency) {
            values.insert(
                clause.field.clone(),
                clause
                    .values
                    .first()
                    .expect("nonempty fixture condition")
                    .clone(),
            );
        }
    }
}
fn preparation(
    registry: &ProviderRegistry,
    input: &mut SaveServiceDraftInput,
) -> PreparedServiceDraftResources {
    let p = registry.project_form(&request(input)).unwrap();
    let v = |id: &str| {
        p.fields
            .iter()
            .find(|f| f.descriptor.id == id)
            .and_then(|f| f.resolved.value.as_ref())
    };
    let t = |id: &str| match v(id) {
        Some(CatalogScalar::Text(s)) => Some(s.clone()),
        _ => None,
    };
    let mut result = PreparedServiceDraftResources::default();
    match &input.target {
        ServiceFormTarget::Offline {
            engine_id,
            contract_revision,
        } => {
            let prepared_values = ["installation_path", "language_package", "runtime", "bottle"]
                .into_iter()
                .filter_map(|id| v(id).map(|value| (id.into(), value.clone())))
                .collect();
            result.offline = Some(PreparedOfflineDraft {
                engine: OfflineEngineKey {
                    engine_id: engine_id.clone(),
                    contract_revision: NonZeroU32::new(*contract_revision).unwrap(),
                },
                pair: LanguagePair {
                    source: t("source_language").unwrap(),
                    target: t("target_language").unwrap(),
                },
                prepared_values,
                installation: LocalInstallationRef {
                    installation_id: "fixture-installation".into(),
                    revision: NonZeroU32::new(1).unwrap(),
                },
                resolution: OfflineConfigResolution::Draft {
                    observed_resource: None,
                    proposed_route: None,
                },
                pivot_consent_revision: Some(NonZeroU32::new(1).unwrap()),
            });
        }
        ServiceFormTarget::Http { preset } => {
            let catalog = registry.catalog();
            let auth = catalog
                .presets
                .iter()
                .find(|p| &p.key == preset)
                .unwrap()
                .default_auth_kind;
            let oauth = t("google_auth_method").as_deref() == Some("oauth");
            if oauth {
                let authorization = GoogleAuthorizationRef {
                    grant_id: "fixture-owned-grant".into(),
                    authorization_epoch: NonZeroU32::new(1).unwrap(),
                };
                input.authentication = ServiceAuthenticationSelection::GoogleOwnedGrant {
                    authorization: authorization.clone(),
                };
                result.google_client = Some(PreparedGoogleClientDraft {
                    mode: t("oauth_client_mode").unwrap_or_else(|| "application".into()),
                    client_id: t("oauth_client_id"),
                    authorization,
                });
            } else if t("google_auth_method").as_deref() == Some("service_account")
                || matches!(
                    auth,
                    CatalogAuthKind::BaiduAppSecret | CatalogAuthKind::YoudaoAppSecret
                )
                || !matches!(
                    t("auth_mode").as_deref(),
                    None | Some("none") | Some("google_oauth")
                )
            {
                input.authentication = ServiceAuthenticationSelection::ExistingPool {
                    pool_id: "fixture-pool".into(),
                    pool_revision: NonZeroU32::new(1).unwrap(),
                };
                result.credential = Some(PreparedCredentialDraft {
                    pool_id: "fixture-pool".into(),
                    pool_revision: NonZeroU32::new(1).unwrap(),
                    multiple_keys: matches!(v("key_pool"), Some(CatalogScalar::Boolean(true))),
                    public_identifiers: ["app_id", "app_key"]
                        .into_iter()
                        .filter_map(|id| t(id).map(|s| (id.into(), s)))
                        .collect(),
                });
            }
        }
    }
    result
}
fn assert_consumed(id: &str, value: &CatalogScalar, graph: &ConfigurationV3) {
    let raw = serde_json::to_value(graph).unwrap();
    let model = graph.models.first();
    let connection = graph.connections.first();
    let offline = graph.offline_engine_configs.first();
    let defaults = model
        .map(|m| &m.request_defaults[&CatalogPurpose::Translation])
        .or_else(|| offline.map(|o| &o.request_defaults))
        .unwrap();
    match id {
        "name" => assert_eq!(
            model
                .map(|m| m.display_name.as_str())
                .or_else(|| offline.map(|o| o.display_name.as_str())),
            match value {
                CatalogScalar::Text(s) => Some(s.as_str()),
                _ => None,
            }
        ),
        "model" => assert_eq!(
            model.unwrap().actual_model_id.as_ref(),
            match value {
                CatalogScalar::Text(s) => Some(s),
                _ => None,
            }
        ),
        "template" => assert_eq!(
            graph.templates.last().unwrap().body["user"],
            serde_json::to_value(value).unwrap()
        ),
        "timeout" => assert_eq!(defaults.execution.timeout_ms, 1250),
        "cache" => assert!(!defaults.cache.enabled),
        "context_count" => {
            let d = defaults;
            assert_eq!(d.context.max_sentences, 0);
            assert!(!d.context.enabled)
        }
        "concurrency" => assert_eq!(defaults.execution.concurrency, 2),
        "retries" => assert_eq!(defaults.execution.retries, 0),
        "fallback" => assert_eq!(defaults.fallback.len(), 1),
        "temperature" => assert_eq!(
            model.unwrap().parameters["temperature"],
            GenerationParameter::Set {
                value: serde_json::json!(0.0)
            }
        ),
        "max_tokens" => assert_eq!(
            model.unwrap().parameters["max_tokens"],
            GenerationParameter::Set {
                value: serde_json::json!(1024)
            }
        ),
        "installation_path" | "language_package" | "runtime" | "bottle" => {
            assert!(offline.unwrap().installation.is_some());
            if matches!(id, "installation_path" | "language_package") {
                assert!(!raw.to_string().contains("/fixture/"))
            }
        }
        "app_id" | "app_key" | "key_pool" => assert_eq!(
            connection.unwrap().credential_pool_id.as_deref(),
            Some("fixture-pool")
        ),
        "oauth_client_id" | "oauth_client_mode" => assert_eq!(
            connection
                .unwrap()
                .google_authorization
                .as_ref()
                .unwrap()
                .grant_id,
            "fixture-owned-grant"
        ),
        "source_language" | "target_language" if offline.is_some() => {
            let pair = &offline.unwrap().pair;
            assert_eq!(
                if id == "source_language" {
                    &pair.source
                } else {
                    &pair.target
                },
                if id == "source_language" {
                    "ja"
                } else {
                    "zh-Hans"
                }
            );
        }
        "proxy_mode" => assert_eq!(graph.connections[0].proxy_mode.as_str(), match value { CatalogScalar::Text(s) => s.as_str(), _ => panic!("proxy mode must be text") }),
        "proxy_url" => assert_eq!(graph.connections[0].network_metadata.get("proxyUrl"), Some(&serde_json::to_value(value).unwrap())),
        // The remaining fields are closed typed enums/strings. Their selected exact
        // scalar must be present in graph data, never only in the form projection.
        _ => match value {
            CatalogScalar::Text(s) => {
                if id == "temperature_mode" {
                    assert!(matches!(
                        model.unwrap().parameters["temperature"],
                        GenerationParameter::Omit {}
                    ))
                } else {
                    assert!(raw.to_string().contains(s), "missing persisted {id}")
                }
            }
            _ => {}
        },
    }
}
#[test]
fn all_real_services_compile_incomplete_drafts_with_exact_protocol_options_and_no_purpose_activation()
 {
    let registry = builtin_registry().unwrap();
    let catalog = registry.catalog();
    assert_eq!(
        (
            catalog.protocols.len(),
            catalog.presets.len(),
            catalog.offline_engines.len()
        ),
        (13, 21, 4)
    );
    let mut protocols = BTreeSet::new();
    for p in &catalog.presets {
        for purpose in &p.allowed_purposes {
            let req = input(
                ServiceFormTarget::Http {
                    preset: p.key.clone(),
                },
                *purpose,
            );
            let r =
                compile_service_draft(&empty(), &req, &registry, &Ids::new(), &Default::default())
                    .unwrap_or_else(|e| panic!("{} {purpose:?}: {e}", p.key.preset_id));
            let c = &r.graph.connections[0];
            assert_eq!(c.adapter_binding.as_ref(), Some(&p.binding));
            assert_eq!(
                c.http_options.as_ref().unwrap().protocol_id(),
                p.binding.protocol.protocol_id
            );
            assert_eq!(c.preset_source.as_ref(), Some(&p.key));
            assert_eq!(r.graph.models[0].template_purpose, Some(*purpose));
            assert!(r.graph.models[0].actual_model_id.is_none());
            assert!(r.graph.bindings.is_empty());
            assert!(r.graph.templates.is_empty());
            protocols.insert(c.protocol.clone());
        }
    }
    assert_eq!(protocols.len(), 13);
    for e in &catalog.offline_engines {
        let mut req = input(
            ServiceFormTarget::Offline {
                engine_id: e.engine_id.clone(),
                contract_revision: e.contract_revision,
            },
            CatalogPurpose::Translation,
        );
        req.values = BTreeMap::from([
            ("source_language".into(), text("ja")),
            ("target_language".into(), text("zh-Hans")),
        ]);
        let r = compile_service_draft(&empty(), &req, &registry, &Ids::new(), &Default::default())
            .unwrap();
        assert!(r.graph.connections.is_empty());
        assert!(r.graph.models.is_empty());
        assert_eq!(
            r.graph.offline_engine_configs[0].engine.engine_id,
            e.engine_id
        );
        assert!(matches!(
            r.graph.offline_engine_configs[0].resolution,
            OfflineConfigResolution::Draft { .. }
        ));
    }
}
#[test]
fn every_approved_descriptor_field_is_owned_and_every_editable_input_reaches_compilation() {
    let registry = builtin_registry().unwrap();
    let catalog = registry.catalog();
    let mut count = 0;
    let mut unique = BTreeSet::new();
    let mut editable_count = 0;
    let mut protected_count = 0;
    let mut observation_blocked_count = 0;
    let frozen: serde_json::Value = serde_json::from_str(include_str!("../../../docs/architecture/service-configuration-presentation.json")).unwrap();
    let mut delta_count = 0;
    let mut original_unique = BTreeSet::new();
    let services = catalog
        .presets
        .iter()
        .map(|p| {
            (
                ServiceFormTarget::Http {
                    preset: p.key.clone(),
                },
                &p.fields,
            )
        })
        .chain(catalog.offline_engines.iter().map(|e| {
            (
                ServiceFormTarget::Offline {
                    engine_id: e.engine_id.clone(),
                    contract_revision: e.contract_revision,
                },
                &e.fields,
            )
        }));
    for (target, fields) in services {
        let service_id = match &target { ServiceFormTarget::Http { preset } => preset.preset_id.as_str(), ServiceFormTarget::Offline { engine_id, .. } => engine_id.as_str() };
        let expected_service = frozen["services"].as_array().unwrap().iter().find(|row|row["id"]==service_id).expect("new service requires independent review");
        let original_ids = expected_service["fields"].as_array().unwrap().iter().map(|row|row["id"].as_str().unwrap()).collect::<Vec<_>>();
        let delta_ids = if matches!(target, ServiceFormTarget::Http { .. }) { vec!["proxy_mode", "proxy_url"] } else { vec![] };
        let expected_ids = original_ids.iter().copied().chain(delta_ids.iter().copied()).collect::<Vec<_>>();
        assert_eq!(fields.iter().map(|f|f.id.as_str()).collect::<Vec<_>>(), expected_ids, "field set/order changed: {service_id}");
        for f in fields {
            if original_ids.contains(&f.id.as_str()) { original_unique.insert(f.id.clone()); } else { assert!(delta_ids.contains(&f.id.as_str())); delta_count += 1; }
            count += 1;
            unique.insert(f.id.clone());
            assert!(service_field_owner(&f.id).is_some(), "unowned {}", f.id);
            let mut req = input(target.clone(), CatalogPurpose::Translation);
            if matches!(target, ServiceFormTarget::Offline { .. }) {
                req.values.insert("source_language".into(), text("ja"));
                req.values.insert("target_language".into(), text("zh-Hans"));
            }
            let value = sample(f);
            if !editable(f) {
                protected_count += 1;
                req.values.insert(f.id.clone(), value);
                assert_eq!(
                    compile_service_draft(
                        &empty(),
                        &req,
                        &registry,
                        &Ids::new(),
                        &Default::default()
                    )
                    .err()
                    .unwrap()
                    .code,
                    "protected_form_field",
                    "{target:?} {}",
                    f.id
                );
                continue;
            }
            editable_count += 1;
            activate(&f.id, fields, &mut req.values, &mut BTreeSet::new());
            req.values.insert(f.id.clone(), value.clone());
            let projection = registry.project_form(&request(&req)).unwrap();
            let row = projection
                .fields
                .iter()
                .find(|r| r.descriptor.id == f.id)
                .unwrap();
            if row.visibility == CapabilityState::Unknown {
                // The real Argos descriptor needs trusted package_detected observation.
                // A plain immutable directory cannot invent that observation, and the
                // caller cannot supply a protected status through ordinary form values.
                assert_eq!(
                    f.id, "language_package",
                    "unexpected unresolved fixture {target:?}"
                );
                assert!(
                    f.condition
                        .all
                        .iter()
                        .any(|c| c.field == "package_detected")
                );
                let error = compile_service_draft(
                    &empty(),
                    &req,
                    &registry,
                    &Ids::new(),
                    &Default::default(),
                )
                .err()
                .unwrap();
                assert_eq!(error.code, "inactive_service_field");
                observation_blocked_count += 1;
                continue;
            }
            assert_eq!(
                row.visibility,
                CapabilityState::Supported,
                "fixture did not reach {target:?} {}",
                f.id
            );
            let prepared = preparation(&registry, &mut req);
            let compiled = compile_service_draft(&empty(), &req, &registry, &Ids::new(), &prepared)
                .unwrap_or_else(|e| panic!("{target:?} {}: {e}", f.id));
            assert_consumed(&f.id, &value, &compiled.graph);
            assert!(compiled.graph.bindings.is_empty());
        }
    }
    assert_eq!(count - delta_count, 546);
    assert_eq!(original_unique.len(), 55);
    assert_eq!(delta_count, 42);
    assert_eq!(count, 588);
    let expected_unique = original_unique.into_iter().chain(["proxy_mode".into(), "proxy_url".into()]).collect::<BTreeSet<_>>();
    assert_eq!(unique, expected_unique);
    assert_eq!(editable_count + protected_count, count);
    assert_eq!(observation_blocked_count, 1);
    eprintln!(
        "descriptor_fields={count}; unique_fields={}; compiled_editable={}; protected_rejected={protected_count}; trusted_observation_blocked={observation_blocked_count}",
        unique.len(),
        editable_count - observation_blocked_count
    );
}
#[test]
fn conditional_defaults_and_authentication_do_not_become_first_choice_guesses() {
    let registry = builtin_registry().unwrap();
    let c = registry.catalog();
    for id in [
        "builtin.azure-translator",
        "builtin.deepl",
        "builtin.google-v3",
    ] {
        let p = c.presets.iter().find(|p| p.key.preset_id == id).unwrap();
        let req = input(
            ServiceFormTarget::Http {
                preset: p.key.clone(),
            },
            CatalogPurpose::Translation,
        );
        let r = compile_service_draft(&empty(), &req, &registry, &Ids::new(), &Default::default())
            .unwrap();
        match r.graph.connections[0].http_options.as_ref().unwrap() {
            HttpServiceOptionsV1::AzureTranslator { resource_type, .. } => {
                assert_eq!(*resource_type, None)
            }
            HttpServiceOptionsV1::DeeplV2 { plan } => assert_eq!(*plan, None),
            HttpServiceOptionsV1::GoogleTranslateV3 { project_id, .. } => {
                assert!(project_id.is_empty())
            }
            _ => panic!("wrong protocol"),
        }
    }
    let p = c
        .presets
        .iter()
        .find(|p| p.key.preset_id == "builtin.qwen")
        .unwrap();
    let mut req = input(
        ServiceFormTarget::Http {
            preset: p.key.clone(),
        },
        CatalogPurpose::Translation,
    );
    let r =
        compile_service_draft(&empty(), &req, &registry, &Ids::new(), &Default::default()).unwrap();
    assert!(r.graph.connections[0].endpoint.is_none());
    req.values
        .insert("provider_site".into(), text("cn-beijing"));
    let r =
        compile_service_draft(&empty(), &req, &registry, &Ids::new(), &Default::default()).unwrap();
    assert_eq!(
        r.graph.connections[0].endpoint.as_deref(),
        Some("https://dashscope.aliyuncs.com/compatible-mode/v1")
    );
}
