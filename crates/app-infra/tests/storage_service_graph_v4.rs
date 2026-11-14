//! Actual Store4 and relational codec3. The independent design is the expected oracle, never production DDL.
use app_core::{
    contracts::{CatalogPurpose, Command, CommandEnvelope, Configuration},
    domain::{
        content_hash,
        model_config::{service_codec::*, service_graph::*},
    },
    ports::ProductRepository,
};
use app_infra::storage::{SqliteStore, service_graph_migration_sha256};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command as Process,
};
fn dir() -> PathBuf {
    let p = std::env::var_os("FP_STORAGE_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("service-graph-v4-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn defaults() -> ServiceRequestDefaults {
    ServiceRequestDefaults {
        purpose: CatalogPurpose::Translation,
        languages: ServiceLanguageDefaults::Http {
            source: SourceLanguageChoice::Explicit {
                language: "ja".into(),
            },
            target: TargetLanguageChoice::InheritPreference {},
        },
        context: ServiceContextConsent {
            enabled: false,
            max_sentences: 0,
            mode: HistoryInputMode::Original,
        },
        execution: ServiceExecutionDefaults {
            timeout_ms: 1001,
            concurrency: 2,
            retries: 0,
        },
        cache: ServiceCachePolicy { enabled: false },
        fallback: vec![],
    }
}
fn graph(id: &str) -> ConfigurationV3 {
    let mut document = json!({"revisionId":id,"connections":[{"connectionId":"c","displayName":"测试连接","protocol":"openai_compatible","deployment":"remote","authMode":"none","endpoint":"http://127.0.0.1:8181/v1","proxyMode":"inherit","networkMetadata":{},"availabilityIntent":false,"adapterBinding":{"protocol":{"protocolId":"openai_compatible","contractRevision":1},"profile":{"profileId":"openai-chat-text","profileRevision":1}},"httpOptions":{"protocol":"openai_compatible"},"origin":{"kind":"custom"}}],"models":[],"bindings":[],"templates":[],"termSets":[],"offlineEngineConfigs":[],"codecVersion":3});
    document["models"] = json!([{"defaultTemplates":{"translation":"t"},"modelId":"m","connectionId":"c","actualModelId":"synthetic-model","displayName":"测试模型","selectionSource":"manual","capabilities":{"translation":"unknown"},"parameters":{"temperature":{"mode":"set","value":0},"maxTokens":{"mode":"omit"}},"templatePurpose":"translation","requestDefaults":{"translation":defaults()}}]);
    document["templates"] = json!([{"templateId":"t","purpose":"translation","displayName":"翻译","sourceKind":"custom","body":{"system":"Translate {{source}}"},"variables":["source"],"compatibility":{}}]);
    document["bindings"] = json!([{"bindingId":"b","scopeKey":"global","purpose":"translation","bindingMode":"override","enabled":false,"outputMode":"text","context":{},"schedule":{},"parameters":{"topP":{"mode":"inherit"}},"engines":[{"engineId":"e","target":{"kind":"http","modelId":"m"},"enabled":true,"httpParameters":{"maxTokens":{"mode":"set","value":128}},"policyOverrides":PolicyOverrides::default()}],"semanticsVersion":"typed_v3","scopeOverrides":PolicyOverrides {execution:ExecutionPolicyOverrides {retries:PolicyOverride::Set {value:0},..Default::default()},cache:PolicyOverride::Set {value:ServiceCachePolicy {enabled:false}},..Default::default()}}]);
    document["termSets"] = json!([{"termSetId":"terms","scopeKey":"global","displayName":"术语","enabled":true,"terms":[{"termId":"z","sourceText":"春","targetText":"春天","sourceLanguage":"ja","targetLanguage":"zh-Hans","enabled":true},{"termId":"a","sourceText":"空","enabled":false}]}]);
    decode_service_graph_import(&serde_json::to_vec(&document).unwrap()).unwrap()
}
fn old_reader() -> (PathBuf, Value) {
    let path = PathBuf::from(
        std::env::var_os("FP_STORAGE_OLD_STORE3_READER_MANIFEST")
            .expect("real frozen Store3 reader prerequisite missing; blocked, never skip"),
    );
    let manifest: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(manifest["schema_version"], 3);
    let binary = PathBuf::from(manifest["binary_path"].as_str().unwrap());
    assert_eq!(
        content_hash(std::fs::read(&binary).unwrap()),
        manifest["binary_sha256"]
    );
    (binary, manifest)
}
fn old_open(path: &Path, profile: &str, mode: &str) -> Value {
    let (binary, _) = old_reader();
    let output = Process::new(binary)
        .arg(path)
        .arg(profile)
        .arg(mode)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn fresh_store_retains_734_old_fields_and_adds_the_fourteen_schema_five_fields() {
    let p = dir().join("fresh.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    store.checkpoint().unwrap();
    let c = Connection::open(&p).unwrap();
    let oracle: Value = serde_json::from_str(include_str!(
        "../../../docs/architecture/database/v5/schema-contract.json"
    ))
    .unwrap();
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        oracle["migration_version"].as_u64().unwrap() as u32
    );
    assert_eq!(
        service_graph_migration_sha256(),
        "09eab3339ed56f535f72cc95d6d12cab069c73d6cd47a07197dd69f561351d55"
    );
    let tables = oracle["databases"]["profile"]["tables"]
        .as_object()
        .unwrap();
    let prior: Value = serde_json::from_str(include_str!("../../../docs/architecture/database/v4/schema-contract.json")).unwrap();
    let prior_tables = prior["databases"]["profile"]["tables"].as_object().unwrap();
    assert_eq!(prior_tables.len(), 81);
    assert_eq!(prior_tables.values().map(|table| table["columns"].as_array().unwrap().len()).sum::<usize>(), 734);
    for (name, original) in prior_tables { assert_eq!(&tables[name], original, "whole old table shape remains exact: {name}"); }
    assert_eq!(tables.len(), 83);
    let mut fields = 0;
    for (name, expected) in tables {
        let mut q = c
            .prepare(&format!("PRAGMA table_xinfo(\"{name}\")"))
            .unwrap();
        let actual=q.query_map([],|r|Ok(json!({"cid":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"type":r.get::<_,String>(2)?,"notnull":r.get::<_,i64>(3)?,"dflt_value":r.get::<_,Option<String>>(4)?.map(|literal|vec![literal]),"pk":r.get::<_,i64>(5)?,"hidden":r.get::<_,i64>(6)?}))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
        assert_eq!(
            actual,
            expected["columns"].as_array().unwrap().clone(),
            "independent fields {name}"
        );
        fields += actual.len();
    }
    assert_eq!(fields, 748);
    assert_eq!(
        c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}
#[test]
fn typed_http_offline_defaults_templates_terms_and_overrides_roundtrip_after_reopen_without_moving_heads()
 {
    let p = dir().join("graph.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    let before_draft = store.snapshot().unwrap().configuration.draft;
    let mut g = graph("typed-v3");
    let mut offdefaults = defaults();
    offdefaults.languages = ServiceLanguageDefaults::OfflineUseConfigPair {};
    g.offline_engine_configs.push(OfflineEngineConfigV1 {
        id: "off".into(),
        engine: OfflineEngineKey {
            engine_id: "argos_translate".into(),
            contract_revision: 1.try_into().unwrap(),
        },
        backend: OfflineBackend::LocalWorker,
        display_name: "本机草稿".into(),
        pair: LanguagePair {
            source: "ja".into(),
            target: "zh-Hans".into(),
        },
        route_policy: OfflineRoutePolicy::DirectOnly {},
        installation: None,
        resolution: OfflineConfigResolution::Draft {
            observed_resource: None,
            proposed_route: None,
        },
        request_defaults: offdefaults,
    });
    g.bindings[0].engines.push(EngineV3 {
        engine_id: "offline-slot".into(),
        target: EngineTarget::Offline {
            offline_config_id: "off".into(),
        },
        enabled: false,
        http_template_override: None,
        http_parameters: Default::default(),
        policy_overrides: Default::default(),
    });
    let bytes = encode_service_graph(&g).unwrap();
    store
        .persist_service_graph_revision(g.clone(), None, 20)
        .unwrap();
    assert_eq!(
        encode_service_graph(
            &store
                .load_service_graph_revision(g.revision_id.clone())
                .unwrap()
        )
        .unwrap(),
        bytes
    );
    assert_eq!(store.snapshot().unwrap().configuration.draft, before_draft);
    drop(store);
    let store = SqliteStore::open(&p, "profile", "reopened", 30).unwrap();
    assert_eq!(
        encode_service_graph(
            &store
                .load_service_graph_revision(g.revision_id.clone())
                .unwrap()
        )
        .unwrap(),
        bytes
    );
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id='typed-v3'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        content_hash(&bytes)
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM offline_engine_configs", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM connections", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM config_heads", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT http_options_json FROM connections", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        r#"{"protocol":"openai_compatible"}"#
    );
}
#[test]
fn draft_cas_conflicts_rollback_and_public_legacy_claims_cannot_create_authority() {
    let p = dir().join("cas.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    let first = graph("first");
    store
        .save_service_graph_draft(first.clone(), 0, 20)
        .unwrap();
    let second = graph("second");
    assert_eq!(
        store
            .save_service_graph_draft(second.clone(), 0, 21)
            .unwrap_err()
            .code,
        "stale_draft_head"
    );
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM config_revisions WHERE revision_id='second'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    drop(c);
    store
        .save_service_graph_draft(second.clone(), 1, 22)
        .unwrap();
    let mut broken = graph("broken");
    broken.models[0].legacy_eligibility_origin = Some(HttpConfigRef {
        revision_id: "first".into(),
        model_id: "m".into(),
    });
    assert!(
        store
            .persist_service_graph_revision(broken, None, 23)
            .is_err()
    );
    let mut changed = second;
    changed.models[0].display_name = "changed".into();
    assert_eq!(
        store
            .persist_service_graph_revision(changed, Some("first".into()), 24)
            .unwrap_err()
            .code,
        "configuration_revision_conflict"
    );
}
#[test]
fn actual_original_writer_two_and_frozen_host_three_migrate_without_rewriting_prior_seals() {
    let d = dir();
    let p = d.join("old.sqlite3");
    let c = Connection::open(&p).unwrap();
    c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    c.execute_batch(include_str!("fixtures/http-schema2-original.sql"))
        .unwrap();
    let seal: String = c
        .query_row("SELECT content_hash FROM config_revision_seals", [], |r| {
            r.get(0)
        })
        .unwrap();
    drop(c);
    assert!(
        old_open(&p, "legacy-http-profile", "accept")["opened"]
            .as_bool()
            .unwrap()
    );
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        3
    );
    drop(c);
    let before = std::fs::read(&p).unwrap();
    let store = SqliteStore::open(&p, "legacy-http-profile", "four", 301).unwrap();
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        5
    );
    assert_eq!(
        c.query_row("SELECT content_hash FROM config_revision_seals", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        seal
    );
    assert!(store.snapshot().unwrap().configuration.applied.is_some());
    let backup = p.with_file_name("old.sqlite3.pre-v5-301.sqlite3");
    assert!(backup.is_file());
    let b = Connection::open(&backup).unwrap();
    assert_eq!(
        b.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        3
    );
    drop(b);
    drop(c);
    drop(store);
    assert!(!before.is_empty());
    let before = content_hash(std::fs::read(&p).unwrap());
    let rejected = old_open(&p, "legacy-http-profile", "reject");
    assert_eq!(rejected["error_code"], "unsupported_schema");
    assert_eq!(content_hash(std::fs::read(&p).unwrap()), before);
}
#[test]
fn legacy_codec_writers_still_save_after_added_columns_without_rehashing_old_grammar() {
    let p = dir().join("legacy-save.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    for (index, codec) in [None, Some(2)].into_iter().enumerate() {
        let mut document = serde_json::to_value(graph(&format!("legacy-{index}"))).unwrap();
        document["bindings"][0]["engines"][0]["modelId"] = json!("m");
        document["bindings"][0]["engines"][0]["parameters"] =
            document["bindings"][0]["engines"][0]["httpParameters"].clone();
        if let Some(codec) = codec {
            document["codecVersion"] = json!(codec);
        } else {
            document.as_object_mut().unwrap().remove("codecVersion");
            for connection in document["connections"].as_array_mut().unwrap() {
                connection.as_object_mut().unwrap().remove("adapterBinding");
                connection.as_object_mut().unwrap().remove("presetSource");
            }
        }
        let config: Configuration = serde_json::from_value(document).unwrap();
        let receipt = store
            .dispatch(
                CommandEnvelope {
                    operation_id: format!("legacy-save-{index}"),
                    expected_revision: None,
                    command: Command::SaveConfiguration {
                        configuration: config.clone(),
                        expected_head_revision: index as u32,
                    },
                },
                20 + index as i64,
            )
            .unwrap();
        let mut expected_config = config.clone();
        expected_config.revision_id = receipt.result_id.unwrap();
        let expected =
            app_core::domain::model_config::codec::encode_configuration(&expected_config).unwrap();
        let c = Connection::open(&p).unwrap();
        let seal: String = c
            .query_row(
                "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
                [&expected_config.revision_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(seal, content_hash(expected));
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.configuration.draft.as_legacy().expect("legacy fixture").codec_version, codec);
    }
}
#[test]
fn direct_historical_one_and_original_writer_two_upgrade_with_versioned_closed_backup() {
    for version in [1, 2] {
        let p = dir().join(format!("old-{version}.sqlite3"));
        let c = Connection::open(&p).unwrap();
        c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        let profile = if version == 2 {
            c.execute_batch(include_str!("fixtures/http-schema2-original.sql"))
                .unwrap();
            "legacy-http-profile"
        } else {
            c.execute_batch(include_str!("../src/storage/migrations/001_profile.sql"))
                .unwrap();
            c.execute_batch(include_str!("../src/storage/migrations/001_config.sql"))
                .unwrap();
            c.execute(
                "INSERT INTO schema_migrations VALUES(1,?1,0,'canonical-v1-synthetic',0)",
                [app_infra::storage::profile_migration_sha256()],
            )
            .unwrap();
            c.execute(
                "INSERT INTO profile_meta VALUES(1,'historical-one',0,1,'historical','active',0,0)",
                [],
            )
            .unwrap();
            "historical-one"
        };
        let old_seals: Vec<String> = c
            .prepare("SELECT content_hash FROM config_revision_seals ORDER BY revision_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        drop(c);
        let store = SqliteStore::open(&p, profile, "four", 200).unwrap();
        let c = Connection::open(&p).unwrap();
        assert_eq!(
            c.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            5
        );
        let new_seals: Vec<String> = c
            .prepare("SELECT content_hash FROM config_revision_seals ORDER BY revision_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(new_seals, old_seals);
        let backup = p.with_file_name(format!("old-{version}.sqlite3.pre-v5-200.sqlite3"));
        let b = Connection::open(&backup).unwrap();
        assert_eq!(
            b.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            version
        );
        drop(b);
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{suffix}", backup.display()));
            assert!(!sidecar.exists() || sidecar.metadata().unwrap().len() == 0);
        }
        drop(store);
    }
}
#[test]
fn all_13_exact_typed_http_option_variants_use_shared_json_without_shadow_mapping() {
    let p = dir().join("options.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    let mut g = graph("all-options");
    g.models.clear();
    g.bindings.clear();
    g.templates.clear();
    g.term_sets.clear();
    let base = g.connections.remove(0);
    let variants = vec![
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: Some("org".into()),
            project_id: Some("project".into()),
        },
        HttpServiceOptionsV1::Anthropic {
            workspace_id: Some("workspace".into()),
        },
        HttpServiceOptionsV1::AzureTranslator {
            resource_type: Some(AzureResourceType::Regional),
            region: Some("japaneast".into()),
        },
        HttpServiceOptionsV1::OpenaiResponses {
            organization_id: None,
            project_id: Some("project".into()),
        },
        HttpServiceOptionsV1::GeminiGenerateContent {},
        HttpServiceOptionsV1::OllamaNative {},
        HttpServiceOptionsV1::LmstudioNativeV1 {},
        HttpServiceOptionsV1::DeeplV2 {
            plan: Some(DeeplPlan::Free),
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
    for (i, options) in variants.into_iter().enumerate() {
        let mut c = base.clone();
        c.connection_id = format!("c-{i}");
        c.protocol = options.protocol_id().into();
        c.adapter_binding.as_mut().unwrap().protocol.protocol_id = c.protocol.clone();
        c.http_options = Some(options);
        g.connections.push(c);
    }
    let bytes = encode_service_graph(&g).unwrap();
    store
        .persist_service_graph_revision(g.clone(), None, 20)
        .unwrap();
    assert_eq!(
        encode_service_graph(
            &store
                .load_service_graph_revision(g.revision_id.clone())
                .unwrap()
        )
        .unwrap(),
        bytes
    );
    let c = Connection::open(&p).unwrap();
    for connection in &g.connections {
        let payload: String = c
            .query_row(
                "SELECT http_options_json FROM connections WHERE connection_id=?1",
                [&connection.connection_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            payload,
            serde_json::to_string(connection.http_options.as_ref().unwrap()).unwrap()
        );
    }
    assert_eq!(
        c.query_row("SELECT count(*) FROM connections", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        13
    );
}
#[test]
fn custom_endpoint_provider_site_survives_real_relational_save_and_reopen() {
    let path = dir().join("custom-site.sqlite3");
    let store = SqliteStore::open(&path, "profile", "four", 10).unwrap();
    let mut configuration = graph("custom-site");
    configuration.connections[0].origin = Some(ConnectionOrigin::Custom {
        copied_from: None,
        site: Some("cn-beijing".into()),
    });
    store.persist_service_graph_revision(configuration.clone(), None, 20).unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let reopened = SqliteStore::open(&path, "profile", "four", 30).unwrap();
    assert_eq!(reopened.load_service_graph_revision("custom-site".into()).unwrap(), configuration);
}
#[test]
fn codec3_metadata_seal_and_canonical_graph_hash_must_all_agree() {
    for mode in ["metadata", "seal", "both"] {
        let p = dir().join(format!("hash-tamper-{mode}.sqlite3"));
        let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
        store
            .persist_service_graph_revision(graph("sealed"), None, 20)
            .unwrap();
        store.checkpoint().unwrap();
        drop(store);
        let c = Connection::open(&p).unwrap();
        for table in ["config_revisions", "config_revision_seals"] {
            if (mode == "metadata" && table != "config_revisions")
                || (mode == "seal" && table != "config_revision_seals")
            {
                continue;
            }
            let trigger = format!("{table}_no_update");
            let ddl: String = c
                .query_row(
                    "SELECT sql FROM sqlite_schema WHERE name=?1",
                    [&trigger],
                    |r| r.get(0),
                )
                .unwrap();
            c.execute_batch(&format!("DROP TRIGGER {trigger}")).unwrap();
            c.execute(
                &format!("UPDATE {table} SET content_hash=?1 WHERE revision_id='sealed'"),
                ["0".repeat(64)],
            )
            .unwrap();
            c.execute_batch(&ddl).unwrap();
            let restored: String = c
                .query_row(
                    "SELECT sql FROM sqlite_schema WHERE name=?1",
                    [&trigger],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(restored, ddl, "restore exact production trigger");
        }
        assert_eq!(
            c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, u32>(
                0
            ))
            .unwrap(),
            0
        );
        drop(c);
        let reopened = SqliteStore::open(&p, "profile", "four", 30).unwrap();
        assert_eq!(
            reopened
                .load_service_graph_revision("sealed".into())
                .unwrap_err()
                .code,
            "configuration_seal_mismatch",
            "codec3 must reject {mode} corruption even when schema, foreign keys and SQL integrity remain valid"
        );
    }
}
#[test]
fn sealed_sql_payload_tamper_unknown_duplicate_keys_and_wrong_trusted_codec_are_rejected() {
    for mode in ["unknown", "duplicate", "codec"] {
        let p = dir().join(format!("tamper-{mode}.sqlite3"));
        let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
        let g = graph("sealed");
        store.persist_service_graph_revision(g, None, 20).unwrap();
        let c = Connection::open(&p).unwrap();
        if mode == "codec" {
            c.execute_batch("DROP TRIGGER config_revisions_no_update; UPDATE config_revisions SET codec_version=2 WHERE revision_id='sealed'").unwrap();
            assert_eq!(
                store
                    .load_service_graph_revision("sealed".into())
                    .unwrap_err()
                    .code,
                "unsupported_configuration_codec"
            );
        } else {
            c.execute_batch("DROP TRIGGER connections_no_update")
                .unwrap();
            let payload = if mode == "unknown" {
                r#"{"protocol":"openai_compatible","headers":{"Authorization":"must-not-execute"}}"#
            } else {
                r#"{"protocol":"openai_compatible","organizationId":"a","organizationId":"b"}"#
            };
            c.execute(
                "UPDATE connections SET http_options_json=?1 WHERE revision_id='sealed'",
                [payload],
            )
            .unwrap();
            assert!(store.load_service_graph_revision("sealed".into()).is_err());
        }
    }
}
#[test]
fn resolved_native_and_worker_configs_roundtrip_real_relations_without_faking_http_or_runtime_readiness()
 {
    // Controlled SQL metadata fixtures exercise persistence only. Both runtime profiles remain UNKNOWN;
    // no files, OS grant, translation, installation probe or executable eligibility is claimed.
    let p = dir().join("resolved.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    let base = graph("source-fallback");
    store
        .persist_service_graph_revision(base, None, 11)
        .unwrap();
    let c = Connection::open(&p).unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    for (runtime, engine, backend) in [
        ("native", "apple_translation", "platform_native"),
        ("worker", "argos_translate", "local_worker"),
    ] {
        c.execute("INSERT INTO offline_runtime_profiles VALUES(?1,1,?2,1,?3,'macos','arm64',?4,?5,'unknown',?6,0)",rusqlite::params![runtime,engine,backend,if backend=="local_worker"{Some(1)}else{None},if backend=="local_worker"{Some("synthetic-cli-contract")}else{None},"a".repeat(64)]).unwrap();
    }
    c.execute("INSERT INTO offline_execution_environments VALUES('env',1,'macos','arm64',?1,'synthetic-os',NULL,NULL,NULL,0)",["b".repeat(64)]).unwrap();
    c.execute("INSERT INTO offline_local_grants VALUES('grant',?1,'synthetic-opaque-locator',1,'unknown',0)",["b".repeat(64)]).unwrap();
    c.execute("INSERT INTO offline_installation_versions VALUES('installation',1,'argos_translate',1,'env',1,'/synthetic/argos','grant','worker',1,0)",[]).unwrap();
    c.execute("INSERT INTO offline_resource_snapshots VALUES('native-resource','apple_translation',1,'platform_native','native',1,'apple_observation',NULL,'synthetic-os',?1,'synthetic-api','traditional_low_latency','ja','zh-Hans',1,?2,0)",rusqlite::params!["c".repeat(64),"d".repeat(64)]).unwrap();
    c.execute("INSERT INTO offline_resource_snapshots VALUES('worker-resource','argos_translate',1,'local_worker','worker',1,'file_manifest',?1,NULL,NULL,NULL,NULL,NULL,NULL,NULL,?2,0)",rusqlite::params!["e".repeat(64),"f".repeat(64)]).unwrap();
    c.execute("INSERT INTO offline_resource_snapshot_files VALUES('worker-resource','synthetic/package.argosmodel',0,?1,'language_package')",["a".repeat(64)]).unwrap();
    for (snapshot, source, target) in [
        ("native-resource", "ja", "zh-Hans"),
        ("worker-resource", "ja", "en"),
        ("worker-resource", "en", "zh-Hans"),
    ] {
        c.execute(
            "INSERT INTO offline_resource_pairs VALUES(?1,?2,?3,'synthetic-pipeline')",
            rusqlite::params![snapshot, source, target],
        )
        .unwrap();
    }
    for (snapshot, digest) in [("native-resource", "d"), ("worker-resource", "f")] {
        c.execute(
            "INSERT INTO offline_resource_seals VALUES(?1,?2,0)",
            rusqlite::params![snapshot, digest.repeat(64)],
        )
        .unwrap();
    }
    c.execute("INSERT INTO offline_translation_routes VALUES('native-route','apple_translation',1,'platform_native','direct','ja','zh-Hans',NULL,?1,0)",["1".repeat(64)]).unwrap();
    c.execute("INSERT INTO offline_translation_routes VALUES('worker-route','argos_translate',1,'local_worker','explicit_pivot','ja','zh-Hans',1,?1,0)",["2".repeat(64)]).unwrap();
    for (route, ordinal, snapshot, engine, backend, source, target) in [
        (
            "native-route",
            0,
            "native-resource",
            "apple_translation",
            "platform_native",
            "ja",
            "zh-Hans",
        ),
        (
            "worker-route",
            0,
            "worker-resource",
            "argos_translate",
            "local_worker",
            "ja",
            "en",
        ),
        (
            "worker-route",
            1,
            "worker-resource",
            "argos_translate",
            "local_worker",
            "en",
            "zh-Hans",
        ),
    ] {
        c.execute("INSERT INTO offline_translation_route_hops VALUES(?1,?2,?3,?4,1,?5,?6,?7,'synthetic-pipeline')",rusqlite::params![route,ordinal,snapshot,engine,backend,source,target]).unwrap();
    }
    for (route, digest) in [("native-route", "1"), ("worker-route", "2")] {
        c.execute(
            "INSERT INTO offline_route_seals VALUES(?1,?2,0)",
            rusqlite::params![route, digest.repeat(64)],
        )
        .unwrap();
    }
    let mut document = graph("resolved-graph");
    document.connections.clear();
    document.models.clear();
    document.templates.clear();
    document.term_sets.clear();
    document.bindings[0].engines.clear();
    for (id, engine, backend, resource, route, installation, policy) in [
        (
            "native",
            "apple_translation",
            OfflineBackend::PlatformNative,
            "native-resource",
            "native-route",
            None,
            OfflineRoutePolicy::DirectOnly {},
        ),
        (
            "worker",
            "argos_translate",
            OfflineBackend::LocalWorker,
            "worker-resource",
            "worker-route",
            Some(LocalInstallationRef {
                installation_id: "installation".into(),
                revision: 1.try_into().unwrap(),
            }),
            OfflineRoutePolicy::ExplicitPivot {
                consent_revision: 1.try_into().unwrap(),
            },
        ),
    ] {
        let mut d = defaults();
        d.languages = ServiceLanguageDefaults::OfflineUseConfigPair {};
        d.fallback = vec![SavedServiceRef::Http {
            source: HttpConfigRef {
                revision_id: "source-fallback".into(),
                model_id: "m".into(),
            },
        }];
        document.offline_engine_configs.push(OfflineEngineConfigV1 {
            id: id.into(),
            engine: OfflineEngineKey {
                engine_id: engine.into(),
                contract_revision: 1.try_into().unwrap(),
            },
            backend,
            display_name: id.into(),
            pair: LanguagePair {
                source: "ja".into(),
                target: "zh-Hans".into(),
            },
            route_policy: policy,
            installation,
            resolution: OfflineConfigResolution::Resolved {
                resource_snapshot_id: resource.into(),
                route_id: route.into(),
            },
            request_defaults: d,
        });
        document.bindings[0].engines.push(EngineV3 {
            engine_id: format!("slot-{id}"),
            target: EngineTarget::Offline {
                offline_config_id: id.into(),
            },
            enabled: false,
            http_template_override: None,
            http_parameters: Default::default(),
            policy_overrides: Default::default(),
        });
    }
    let bytes = encode_service_graph(&document).unwrap();
    store
        .persist_service_graph_revision(document.clone(), Some("source-fallback".into()), 20)
        .unwrap();
    assert_eq!(
        encode_service_graph(
            &store
                .load_service_graph_revision(document.revision_id.clone())
                .unwrap()
        )
        .unwrap(),
        bytes
    );
    drop(c);
    drop(store);
    let store = SqliteStore::open(&p, "profile", "reopened", 30).unwrap();
    assert_eq!(
        encode_service_graph(
            &store
                .load_service_graph_revision(document.revision_id)
                .unwrap()
        )
        .unwrap(),
        bytes
    );
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM offline_runtime_profiles WHERE verification_state='unknown'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM connections WHERE revision_id='resolved-graph'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM models WHERE revision_id='resolved-graph'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn fallback_reference_must_be_already_saved_not_just_an_unsealed_fk_parent() {
    let p = dir().join("unsaved-fallback.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    let c = Connection::open(&p).unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    c.execute("INSERT INTO config_revisions(revision_id,content_hash,created_at_ms,save_reason) VALUES('unsealed',?1,0,'edit')",["a".repeat(64)]).unwrap();
    c.execute("INSERT INTO connections(revision_id,connection_id,display_name,protocol,deployment,auth_mode) VALUES('unsealed','c','draft','openai_compatible','local','none')",[]).unwrap();
    c.execute("INSERT INTO models(revision_id,model_id,connection_id,display_name) VALUES('unsealed','m','c','draft')",[]).unwrap();
    let mut g = graph("must-rollback");
    g.models[0]
        .request_defaults
        .get_mut(&CatalogPurpose::Translation)
        .unwrap()
        .fallback
        .push(SavedServiceRef::Http {
            source: HttpConfigRef {
                revision_id: "unsealed".into(),
                model_id: "m".into(),
            },
        });
    assert_eq!(
        store
            .persist_service_graph_revision(g, None, 20)
            .unwrap_err()
            .code,
        "fallback_source_not_saved"
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM config_revisions WHERE revision_id='must-rollback'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn preset_and_custom_copy_origins_roundtrip_exact_shared_identity_and_missing_version_rolls_back() {
    let p = dir().join("origins.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    let mut g = graph("preset-origin");
    let key = app_core::contracts::ProviderPresetKey {
        preset_id: "user.controlled-origin".into(),
        preset_revision: 1,
    };
    let record = app_core::contracts::StoredProviderPreset {
        key: key.clone(),
        namespace: app_core::contracts::CatalogNamespace::User,
        provider_id: "user.controlled-origin".into(),
        provider_label: "Controlled".into(),
        display_name: "Controlled source".into(),
        binding: g.connections[0].adapter_binding.clone().unwrap(),
        endpoint: g.connections[0].endpoint.clone(),
        auth_kind: "none".into(),
        options: Default::default(),
        source_refs: vec!["controlled-storage-fixture".into()],
    };
    store.store_provider_preset(record, 11).unwrap();
    g.connections[0].preset_source = Some(key.clone());
    g.connections[0].origin = Some(ConnectionOrigin::Preset {
        key: key.clone(),
        site: Some("controlled".into()),
    });
    let bytes = encode_service_graph(&g).unwrap();
    store
        .persist_service_graph_revision(g.clone(), None, 20)
        .unwrap();
    assert_eq!(
        encode_service_graph(&store.load_service_graph_revision(g.revision_id).unwrap()).unwrap(),
        bytes
    );
    let mut copied = graph("custom-copy");
    copied.connections[0].preset_source = Some(key.clone());
    copied.connections[0].origin = Some(ConnectionOrigin::Custom {
        copied_from: Some(key.clone()),
        site: None,
    });
    let bytes = encode_service_graph(&copied).unwrap();
    store
        .persist_service_graph_revision(copied.clone(), None, 21)
        .unwrap();
    assert_eq!(
        encode_service_graph(
            &store
                .load_service_graph_revision(copied.revision_id)
                .unwrap()
        )
        .unwrap(),
        bytes
    );
    let mut missing = graph("unknown-preset-version");
    let mut bad = key;
    bad.preset_revision = 2;
    missing.connections[0].preset_source = Some(bad.clone());
    missing.connections[0].origin = Some(ConnectionOrigin::Preset {
        key: bad,
        site: None,
    });
    assert!(
        store
            .persist_service_graph_revision(missing, None, 22)
            .is_err()
    );
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM config_revisions WHERE revision_id='unknown-preset-version'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn missing_deepl_azure_choices_are_saved_drafts_without_first_choice_defaults_or_bad_types() {
    let p = dir().join("missing-choices.sqlite3");
    let store = SqliteStore::open(&p, "profile", "four", 10).unwrap();
    let mut g = graph("missing-choices");
    g.models[0].actual_model_id = None;
    g.connections[0].auth_mode = "api_key".into();
    assert!(g.connections[0].credential_pool_id.is_none());
    let base = g.connections[0].clone();
    for (id, options, auth) in [
        (
            "deepl-missing",
            HttpServiceOptionsV1::DeeplV2 { plan: None },
            "api_key",
        ),
        (
            "azure-missing",
            HttpServiceOptionsV1::AzureTranslator {
                resource_type: None,
                region: None,
            },
            "api_key",
        ),
        (
            "google-route-missing",
            HttpServiceOptionsV1::GoogleTranslateV3 {
                project_id: String::new(),
                location: String::new(),
            },
            "google_service_account",
        ),
    ] {
        let mut connection = base.clone();
        connection.connection_id = id.into();
        connection.protocol = options.protocol_id().into();
        connection
            .adapter_binding
            .as_mut()
            .unwrap()
            .protocol
            .protocol_id = connection.protocol.clone();
        connection.auth_mode = auth.into();
        connection.http_options = Some(options);
        g.connections.push(connection);
    }
    let bytes = encode_service_graph(&g).unwrap();
    store
        .persist_service_graph_revision(g.clone(), None, 20)
        .unwrap();
    drop(store);
    let store = SqliteStore::open(&p, "profile", "reopened", 30).unwrap();
    assert_eq!(
        encode_service_graph(
            &store
                .load_service_graph_revision(g.revision_id.clone())
                .unwrap()
        )
        .unwrap(),
        bytes
    );
    let c = Connection::open(&p).unwrap();
    for (id, literal) in [
        ("deepl-missing", r#"{"protocol":"deepl_v2"}"#),
        ("azure-missing", r#"{"protocol":"azure_translator"}"#),
    ] {
        assert_eq!(
            c.query_row(
                "SELECT http_options_json FROM connections WHERE connection_id=?1",
                [id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            literal
        );
    }
    for (index, field) in [(1, "plan"), (2, "resourceType")] {
        let mut invalid = serde_json::to_value(&g).unwrap();
        invalid["connections"][index]["httpOptions"][field] = json!(false);
        assert!(decode_service_graph_import(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    assert_eq!(
        c.query_row("SELECT count(*) FROM credential_pool_members", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM config_heads", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn r2_real_parent_sql_seal_matrix_preserves_legacy_duplicates_but_rejects_typed_direct_alias_and_disabled_duplicates()
 {
    // SQL/storage-contract probe, not the not-yet-implemented private activation writer.
    // The source is real original writer2 data migrated by the frozen Store3 binary and current Store4.
    let p = dir().join("r2-parent-matrix.sqlite3");
    let c = Connection::open(&p).unwrap();
    c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    c.execute_batch(include_str!("fixtures/http-schema2-original.sql"))
        .unwrap();
    drop(c);
    old_open(&p, "legacy-http-profile", "accept");
    let store = SqliteStore::open(&p, "legacy-http-profile", "four", 301).unwrap();
    let original = store.snapshot().unwrap().configuration.applied.unwrap().as_legacy().expect("legacy fixture").clone();
    let source_model = original
        .models
        .iter()
        .find(|m| m.model_id == "z-model")
        .unwrap();
    let source_connection = original
        .connections
        .iter()
        .find(|c| c.connection_id == source_model.connection_id)
        .unwrap();
    assert!(source_model.default_templates.is_empty());
    assert!(original.codec_version.is_none());
    let source_seal: String = Connection::open(&p)
        .unwrap()
        .query_row(
            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
            [&original.revision_id],
            |r| r.get(0),
        )
        .unwrap();
    let heads_before: Vec<(String, String, u32)> = Connection::open(&p)
        .unwrap()
        .prepare("SELECT head_kind,revision_id,head_revision FROM config_heads ORDER BY head_kind")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let convert_parameters = |parameters: &std::collections::BTreeMap<
        String,
        app_core::contracts::Parameter,
    >| {
        parameters
            .iter()
            .map(|(key, p)| {
                (
                    key.clone(),
                    match p.mode {
                        app_core::contracts::ParameterMode::Inherit => {
                            GenerationParameter::Inherit {}
                        }
                        app_core::contracts::ParameterMode::Omit => GenerationParameter::Omit {},
                        app_core::contracts::ParameterMode::Set => GenerationParameter::Set {
                            value: p.value.clone().expect("actual saved set value"),
                        },
                    },
                )
            })
            .collect()
    };
    let mut tested = 0;
    for semantics in [
        PurposeSemanticsVersion::LegacyV2,
        PurposeSemanticsVersion::TypedV3,
    ] {
        for alias in [false, true] {
            for disabled in [false, true] {
                let case = format!(
                    "matrix-{}-{alias}-{disabled}",
                    if semantics == PurposeSemanticsVersion::LegacyV2 {
                        "legacy"
                    } else {
                        "typed"
                    }
                );
                let mut connection: ConnectionV3 =
                    serde_json::from_value(serde_json::to_value(source_connection).unwrap())
                        .unwrap();
                connection.connection_id = "derived-connection".into();
                connection.materialization_source = Some(ConnectionSourceRef {
                    revision_id: original.revision_id.clone(),
                    connection_id: source_connection.connection_id.clone(),
                });
                let mut graph = ConfigurationV3 {
                    revision_id: case.clone(),
                    connections: vec![connection],
                    models: vec![],
                    bindings: vec![],
                    templates: vec![],
                    term_sets: vec![],
                    offline_engine_configs: vec![],
                    codec_version: 3,
                };
                for number in 0..if alias { 2 } else { 1 } {
                    graph.models.push(ModelV3 {
                        default_templates: Default::default(),
                        model_id: format!("model-{number}"),
                        connection_id: "derived-connection".into(),
                        actual_model_id: source_model.actual_model_id.clone(),
                        display_name: source_model.display_name.clone(),
                        selection_source: source_model.selection_source.clone(),
                        capabilities: source_model.capabilities.clone(),
                        parameters: convert_parameters(&source_model.parameters),
                        template_purpose: Some(CatalogPurpose::Translation),
                        legacy_eligibility_origin: Some(HttpConfigRef {
                            revision_id: original.revision_id.clone(),
                            model_id: source_model.model_id.clone(),
                        }),
                        request_defaults: [(CatalogPurpose::Translation, defaults())].into(),
                    });
                }
                graph.bindings.push(PurposeBindingV3 {
                    binding_id: "b".into(),
                    scope_key: "global".into(),
                    game_id: None,
                    purpose: CatalogPurpose::Translation,
                    binding_mode: "override".into(),
                    enabled: false,
                    output_mode: Some("text".into()),
                    source_language: None,
                    target_language: None,
                    template_id: None,
                    context: Default::default(),
                    schedule: Default::default(),
                    parameters: Default::default(),
                    engines: (0..2)
                        .map(|number| EngineV3 {
                            engine_id: format!("e-{number}"),
                            target: EngineTarget::Http {
                                model_id: format!("model-{}", if alias { number } else { 0 }),
                            },
                            enabled: number == 0 || !disabled,
                            http_template_override: None,
                            http_parameters: Default::default(),
                            policy_overrides: Default::default(),
                        })
                        .collect(),
                    semantics_version: semantics,
                    scope_overrides: Default::default(),
                });
                let bytes = encode_service_graph(&graph).unwrap();
                let hash = content_hash(&bytes);
                let mut database = Connection::open(&p).unwrap();
                database.execute_batch("PRAGMA foreign_keys=ON").unwrap();
                let tx = database
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .unwrap();
                // Compare the actual source seal/codec and copied wire fields, rather than trusting a DTO origin claim.
                assert!(tx.query_row("SELECT EXISTS(SELECT 1 FROM config_revisions c JOIN config_revision_seals s ON s.revision_id=c.revision_id WHERE c.revision_id=?1 AND c.codec_version<3 AND s.content_hash=?2)",rusqlite::params![original.revision_id,source_seal],|r|r.get::<_,bool>(0)).unwrap());
                tx.execute("INSERT INTO config_revisions(revision_id,parent_revision_id,content_hash,created_at_ms,save_reason,codec_version,revision_role) VALUES(?1,?2,?3,302,'edit',3,'activation')",rusqlite::params![case,original.revision_id,hash]).unwrap();
                tx.execute("INSERT INTO connections(revision_id,connection_id,display_name,protocol,deployment,auth_mode,endpoint,credential_pool_id,credential_pool_revision,proxy_mode,network_metadata_json,availability_intent,adapter_binding_schema,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision,provider_preset_id,provider_preset_revision) SELECT ?1,'derived-connection',display_name,protocol,deployment,auth_mode,endpoint,credential_pool_id,credential_pool_revision,proxy_mode,network_metadata_json,availability_intent,adapter_binding_schema,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision,provider_preset_id,provider_preset_revision FROM connections WHERE revision_id=?2 AND connection_id=?3",rusqlite::params![case,original.revision_id,source_connection.connection_id]).unwrap();
                let projection = |revision: &str, id: &str| {
                    tx.query_row("SELECT quote(display_name),quote(protocol),quote(deployment),quote(auth_mode),quote(endpoint),quote(credential_pool_id),quote(credential_pool_revision),quote(proxy_mode),quote(network_metadata_json),quote(availability_intent) FROM connections WHERE revision_id=?1 AND connection_id=?2",rusqlite::params![revision,id],|r|(0..10).map(|index|r.get::<_,String>(index)).collect::<rusqlite::Result<Vec<_>>>()).unwrap()
                };
                assert_eq!(
                    projection(&case, "derived-connection"),
                    projection(&original.revision_id, &source_connection.connection_id)
                );
                for model in &graph.models {
                    tx.execute("INSERT INTO models(revision_id,model_id,connection_id,actual_model_id,display_name,selection_source,capability_json,template_purpose) SELECT ?1,?2,'derived-connection',actual_model_id,display_name,selection_source,capability_json,'translation' FROM models WHERE revision_id=?3 AND model_id=?4",rusqlite::params![case,model.model_id,original.revision_id,source_model.model_id]).unwrap();
                    tx.execute("INSERT INTO model_parameters SELECT ?1,?2,parameter_key,mode,value_json FROM model_parameters WHERE revision_id=?3 AND model_id=?4",rusqlite::params![case,model.model_id,original.revision_id,source_model.model_id]).unwrap();
                    let model_projection = |revision: &str, id: &str| {
                        tx.query_row("SELECT quote(actual_model_id),quote(display_name),quote(selection_source),quote(capability_json) FROM models WHERE revision_id=?1 AND model_id=?2",rusqlite::params![revision,id],|r|(0..4).map(|index|r.get::<_,String>(index)).collect::<rusqlite::Result<Vec<_>>>()).unwrap()
                    };
                    assert_eq!(
                        model_projection(&case, &model.model_id),
                        model_projection(&original.revision_id, &source_model.model_id)
                    );
                    tx.execute("INSERT INTO activation_resource_materializations(materialization_id,effective_revision_id,target_kind,effective_model_id,source_revision_id,source_model_id,reason,effective_connection_id,source_connection_id,control_connection_id) VALUES(?1,?2,'http',?3,?4,?5,'legacy_materialization','derived-connection',?6,?6)",rusqlite::params![format!("{case}-{}",model.model_id),case,model.model_id,original.revision_id,source_model.model_id,source_connection.connection_id]).unwrap();
                    tx.execute("INSERT INTO service_request_defaults VALUES(?1,?2,'http',?3,NULL,'translation','explicit','ja','inherit_preference',NULL,0,0,'original',1001,2,0,0)",rusqlite::params![case,format!("defaults-{}",model.model_id),model.model_id]).unwrap();
                }
                let policy = serde_json::to_string(&PolicyOverrides::default()).unwrap();
                tx.execute("INSERT INTO purpose_bindings(revision_id,binding_id,scope_key,purpose,binding_mode,enabled,output_mode,context_json,schedule_json,semantics_version,policy_overrides_json) VALUES(?1,'b','global','translation','override',0,'text','{}','{}',?2,?3)",rusqlite::params![case,if semantics==PurposeSemanticsVersion::TypedV3{3}else{2},policy]).unwrap();
                for (position, engine) in graph.bindings[0].engines.iter().enumerate() {
                    let EngineTarget::Http { model_id } = &engine.target else {
                        unreachable!()
                    };
                    tx.execute("INSERT INTO binding_engines(revision_id,binding_id,engine_id,purpose,model_id,position,enabled,target_kind,policy_overrides_json) VALUES(?1,'b',?2,'translation',?3,?4,?5,'http',?6)",rusqlite::params![case,engine.engine_id,model_id,position as i64,engine.enabled,policy]).unwrap();
                }
                let result = tx.execute(
                    "INSERT INTO config_revision_seals VALUES(?1,?2,302)",
                    rusqlite::params![case, hash],
                );
                if semantics == PurposeSemanticsVersion::TypedV3 {
                    let error = result.expect_err(
                        "typed graph must reject duplicate logical service even disabled/aliased",
                    );
                    assert!(
                        error.to_string().contains("duplicate_logical_service"),
                        "{case}: {error}"
                    );
                    tx.rollback().unwrap();
                    assert_eq!(
                        database
                            .query_row(
                                "SELECT count(*) FROM config_revisions WHERE revision_id=?1",
                                [&case],
                                |r| r.get::<_, u32>(0)
                            )
                            .unwrap(),
                        0
                    );
                } else {
                    result.unwrap();
                    tx.commit().unwrap();
                    assert_eq!(
                        encode_service_graph(
                            &store.load_service_graph_revision(case.clone()).unwrap()
                        )
                        .unwrap(),
                        bytes
                    );
                }
                assert_eq!(
                    database
                        .query_row(
                            "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
                            [&original.revision_id],
                            |r| r.get::<_, String>(0)
                        )
                        .unwrap(),
                    source_seal
                );
                tested += 1;
            }
        }
    }
    assert_eq!(tested, 8);
    let c = Connection::open(&p).unwrap();
    let heads_after: Vec<(String, String, u32)> = c
        .prepare("SELECT head_kind,revision_id,head_revision FROM config_heads ORDER BY head_kind")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(heads_after, heads_before);
}
