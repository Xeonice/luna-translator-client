//! Complete profile-field evidence from explicit production Repository scenarios.
//! Owned DTO fields cover Some/None/Unicode/parameter states; internal fields
//! are asserted against typed read-only values, exact producer inputs, hashes,
//! frozen authority/lifecycle relations and controlled maintenance outcomes.
//! The frozen field inventory is checked only by the independent collector; it
//! never creates a passing assertion or substitutes for a business producer.
use super::evidence as acceptance_evidence;
use app_core::{contracts::*, ports::ProductRepository};
use app_infra::storage::SqliteStore;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn save(store: &SqliteStore, operation: &str, command: Command, now: i64) -> CommandReceipt {
    store
        .dispatch(
            CommandEnvelope {
                operation_id: operation.into(),
                expected_revision: None,
                command,
            },
            now,
        )
        .unwrap()
}
struct Proofs {
    rows: BTreeMap<String, Value>,
}
impl Proofs {
    fn fields(
        &mut self,
        table: &str,
        mapping: &[(&str, &str)],
        actual: &Value,
        expected: &Value,
        case: &str,
        source: (&str, &str),
    ) {
        let (model, codec) = source;
        for (column, pointer) in mapping {
            let actual = actual
                .pointer(pointer)
                .unwrap_or_else(|| panic!("actual {model} has no {pointer}"));
            let expected = expected
                .pointer(pointer)
                .unwrap_or_else(|| panic!("expected {model} has no {pointer}"));
            assert_eq!(
                actual, expected,
                "actual production {table}.{column} DTO mapping in {case}"
            );
            let id = format!("profile.{table}.{column}");
            let row=self.rows.entry(id.clone()).or_insert_with(||json!({"id":id,"status":"partial_runtime_evidence","model":model,"codec":codec,"assertions":0,"cases":[],"null_seen":false,"nonnull_seen":false}));
            row["assertions"] = json!(row["assertions"].as_u64().unwrap() + 1);
            row["cases"].as_array_mut().unwrap().push(json!(case));
            row[if actual.is_null() {
                "null_seen"
            } else {
                "nonnull_seen"
            }] = json!(true);
        }
    }
}
fn game_input(full: bool) -> GameInput {
    GameInput {
        game_id: None,
        row_revision: None,
        display_name: "型の往復𠮷🦀".into(),
        original_title: full.then(|| "原題・が".into()),
        source_language: full.then(|| "ja".into()),
        cover_asset_ref: full.then(|| "local-cover/𠮷.png".into()),
        target: if full {
            GameTarget {
                environment_kind: "crossover".into(),
                environment_key: "crossover:/private/test-bottle".into(),
                bottle_locator: Some("/private/test-bottle".into()),
                launch_path: Some("C:\\Games\\launcher.exe".into()),
                canonical_exe_path: Some("C:\\Games\\実行.exe".into()),
                target_path_key: Some("c:\\games\\実行.exe".into()),
                working_directory: Some("C:\\Games".into()),
                arguments: vec!["--章=𠮷".into(), "two words".into()],
                architecture: Some("x64".into()),
                verification_state: "verified".into(),
            }
        } else {
            GameTarget {
                environment_kind: "unresolved".into(),
                environment_key: "unresolved:draft".into(),
                bottle_locator: None,
                launch_path: None,
                canonical_exe_path: None,
                target_path_key: None,
                working_directory: None,
                arguments: vec![],
                architecture: None,
                verification_state: "unresolved".into(),
            }
        },
    }
}
fn parameters() -> BTreeMap<String, Parameter> {
    BTreeMap::from([
        (
            "temperature".into(),
            Parameter {
                mode: ParameterMode::Set,
                value: Some(json!(0)),
            },
        ),
        (
            "top_p".into(),
            Parameter {
                mode: ParameterMode::Omit,
                value: None,
            },
        ),
        (
            "max_tokens".into(),
            Parameter {
                mode: ParameterMode::Inherit,
                value: None,
            },
        ),
    ])
}
fn configuration(full: bool) -> Configuration {
    Configuration {
        codec_version: None,
        revision_id: String::new(),
        connections: vec![Connection {
            connection_id: "con-typed".into(),
            display_name: "内網・𠮷".into(),
            protocol: "openai_compatible".into(),
            deployment: "local".into(),
            auth_mode: if full { "api_key" } else { "none" }.into(),
            endpoint: full.then(|| "http://127.0.0.1:1234/v1".into()),
            credential_pool_id: full.then(|| "pool-typed".into()),
            credential_pool_revision: full.then_some(u32::MAX),
            proxy_mode: if full { "override" } else { "disabled" }.into(),
            network_metadata: if full {
                BTreeMap::from([
                    ("proxyUrl".into(), json!("http://127.0.0.1:8118")),
                    ("headers".into(), json!({"x-test":"型"})),
                ])
            } else {
                BTreeMap::new()
            },
            availability_intent: full,
            adapter_binding: None,
            preset_source: None,
        }],
        models: vec![Model {
            default_templates: Default::default(),
            model_id: "model-typed".into(),
            connection_id: "con-typed".into(),
            actual_model_id: full.then(|| "actual/模型𠮷".into()),
            display_name: "モデル型".into(),
            selection_source: if full { "manual" } else { "discovered" }.into(),
            capabilities: if full {
                BTreeMap::from([
                    ("streaming".into(), json!(true)),
                    ("nested".into(), json!({"zero":0,"list":["𠮷",null]})),
                ])
            } else {
                BTreeMap::new()
            },
            parameters: parameters(),
        }],
        templates: vec![Template {
            template_id: "template-typed".into(),
            purpose: "translation".into(),
            display_name: "テンプレート".into(),
            source_kind: "custom".into(),
            source_version: full.then(|| "custom-型-v1".into()),
            body: BTreeMap::from([
                ("system".into(), json!("翻訳者")),
                ("user".into(), json!("{source}")),
            ]),
            variables: vec!["source".into()],
            compatibility: BTreeMap::from([("protocols".into(), json!(["openai_compatible"]))]),
        }],
        bindings: vec![PurposeBinding {
            binding_id: "binding-typed".into(),
            scope_key: "global".into(),
            game_id: None,
            purpose: "translation".into(),
            binding_mode: "override".into(),
            enabled: full,
            output_mode: full.then(|| "text".into()),
            source_language: full.then(|| "ja".into()),
            target_language: full.then(|| "zh-Hans".into()),
            template_id: full.then(|| "template-typed".into()),
            context: BTreeMap::from([
                ("recentSentences".into(), json!(0)),
                ("mode".into(), json!("both")),
            ]),
            schedule: BTreeMap::from([
                ("retryCount".into(), json!(0)),
                ("concurrency".into(), json!(1)),
            ]),
            parameters: parameters(),
            engines: vec![Engine {
                engine_id: "engine-typed".into(),
                model_id: "model-typed".into(),
                enabled: full,
                template_id: full.then(|| "template-typed".into()),
                parameters: parameters(),
            }],
        }],
        term_sets: vec![TermSet {
            term_set_id: "terms-typed".into(),
            scope_key: "global".into(),
            game_id: None,
            display_name: "用語𠮷".into(),
            enabled: full,
            terms: vec![Term {
                term_id: "term-typed".into(),
                source_text: full.then(|| "図書館".into()),
                target_text: full.then(|| "图书馆".into()),
                source_language: full.then(|| "ja".into()),
                target_language: full.then(|| "zh-Hans".into()),
                enabled: full,
            }],
        }],
    }
}

fn verify_owned_fields() -> Vec<Value> {
    let root = root();
    let fallback = root
        .join(".build/acceptance-closure-20261019/storage/field-work/owned")
        .join(format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    let out = acceptance_evidence::directory("typed-fields", &fallback);
    let path = out.join("profile.sqlite3");
    let store = SqliteStore::open(&path, "typed-profile", "typed-server", 100).unwrap();
    let mut proofs = Proofs {
        rows: BTreeMap::new(),
    };
    let game_fields = [
        ("game_id", "/gameId"),
        ("display_name", "/displayName"),
        ("original_title", "/originalTitle"),
        ("source_language", "/sourceLanguage"),
        ("cover_asset_ref", "/coverAssetRef"),
        ("row_revision", "/rowRevision"),
    ];
    let target_fields = [
        ("game_id", "/gameId"),
        ("environment_kind", "/target/environmentKind"),
        ("environment_key", "/target/environmentKey"),
        ("bottle_locator", "/target/bottleLocator"),
        ("launch_path", "/target/launchPath"),
        ("canonical_exe_path", "/target/canonicalExePath"),
        ("target_path_key", "/target/targetPathKey"),
        ("working_directory", "/target/workingDirectory"),
        ("arguments_json", "/target/arguments"),
        ("architecture", "/target/architecture"),
        ("verification_state", "/target/verificationState"),
    ];
    let mut game_id = None;
    for (index, full) in [true, false].into_iter().enumerate() {
        let mut input = game_input(full);
        input.game_id = game_id.clone();
        input.row_revision = game_id.as_ref().map(|_| 1);
        let receipt = save(
            &store,
            &format!("game-map-{index}"),
            Command::SaveGame(Box::new(input.clone())),
            101 + index as i64,
        );
        let id = receipt.result_id.unwrap();
        game_id = Some(id.clone());
        let expected = Game {
            game_id: id,
            display_name: input.display_name,
            original_title: input.original_title,
            source_language: input.source_language,
            cover_asset_ref: input.cover_asset_ref,
            row_revision: 1 + index as u32,
            target: input.target,
        };
        let actual = store.snapshot().unwrap().games.into_iter().next().unwrap();
        assert_eq!(actual, expected);
        let case = if full {
            "GameInput optional Some/Unicode/argv"
        } else {
            "GameInput optional None/empty argv/CAS edit"
        };
        let actual = serde_json::to_value(actual).unwrap();
        let expected = serde_json::to_value(expected).unwrap();
        proofs.fields(
            "games",
            &game_fields,
            &actual,
            &expected,
            case,
            ("GameInput->Game", "storage/queries/mod.rs::dispatch/games"),
        );
        proofs.fields(
            "game_targets",
            &target_fields,
            &actual,
            &expected,
            case,
            ("GameTarget", "storage/queries/mod.rs::dispatch/games"),
        );
    }
    for (index, prefs) in [
        Preferences {
            scope_key: "global".into(),
            theme: "dark".into(),
            transparency_mode: "reduced".into(),
            show_ruby: false,
            show_pos: true,
            segmentation_enabled: true,
            reading_display_mode: ReadingDisplayMode::Both,
            hud_background_opacity: 0.70,
            source_font_pt: 12.5,
            translation_font_pt: 22.25,
            hud_width_pt: 580.5,
            shortcuts: BTreeMap::from([("toggleHud".into(), "Command+Shift+R".into())]),
            overrides: BTreeMap::from([("showRuby".into(), false)]),
        },
        Preferences::default(),
    ]
    .into_iter()
    .enumerate()
    {
        save(
            &store,
            &format!("preference-map-{index}"),
            Command::SavePreferences {
                preferences: prefs.clone(),
            },
            110 + index as i64,
        );
        let actual = store.snapshot().unwrap().preferences;
        assert_eq!(actual, prefs);
        proofs.fields(
            "preference_revisions",
            &[
                ("scope_key", "/scopeKey"),
                ("theme", "/theme"),
                ("transparency_mode", "/transparencyMode"),
                ("show_ruby", "/showRuby"),
                ("show_pos", "/showPos"),
                ("source_font_pt", "/sourceFontPt"),
                ("translation_font_pt", "/translationFontPt"),
                ("hud_width_pt", "/hudWidthPt"),
                ("shortcuts_json", "/shortcuts"),
                ("overrides_json", "/overrides"),
            ],
            &serde_json::to_value(actual).unwrap(),
            &serde_json::to_value(prefs).unwrap(),
            "Preferences actual finite f64/bool/map round trip",
            (
                "Preferences",
                "storage/queries/mod.rs::save_preferences/global_preferences",
            ),
        );
    }
    let before_invalid = store.snapshot().unwrap().commit_revision;
    for (index, value) in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0]
        .into_iter()
        .enumerate()
    {
        let mut prefs = store.snapshot().unwrap().preferences;
        prefs.source_font_pt = value;
        let err = store
            .dispatch(
                CommandEnvelope {
                    operation_id: format!("invalid-float-{index}"),
                    expected_revision: None,
                    command: Command::SavePreferences { preferences: prefs },
                },
                120 + index as i64,
            )
            .unwrap_err();
        assert_eq!(err.code, "invalid_preferences");
        assert_eq!(store.snapshot().unwrap().commit_revision, before_invalid);
    }
    store
        .begin_credential(
            CredentialReference {
                credential_id: "credential-typed".into(),
                credential_version_id: "vault-typed".into(),
                pool_id: "pool-typed".into(),
                pool_revision: u32::MAX,
                label: "typed fake reference".into(),
                masked_hint: "••••0000".into(),
            },
            130,
        )
        .unwrap();
    store
        .activate_credential("credential-typed".into(), 131)
        .unwrap();
    for (index, full) in [true, false].into_iter().enumerate() {
        let mut expected = configuration(full);
        let head = store.snapshot().unwrap().configuration.draft_head_revision;
        let receipt = save(
            &store,
            &format!("configuration-map-{index}"),
            Command::SaveConfiguration {
                configuration: expected.clone(),
                expected_head_revision: head,
            },
            140 + index as i64,
        );
        expected.revision_id = receipt.result_id.unwrap();
        let actual = store.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone();
        assert_eq!(
            actual, expected,
            "production configuration load must decode its complete original typed values"
        );
        let actual = serde_json::to_value(actual).unwrap();
        let expected = serde_json::to_value(expected).unwrap();
        let case = if full {
            "Configuration optional Some/u32MAX/zero nested JSON"
        } else {
            "Configuration optional None/bool false/three parameter states"
        };
        for (table, prefix, fields) in [
            (
                "connections",
                "/connections/0",
                vec![
                    ("connection_id", "connectionId"),
                    ("display_name", "displayName"),
                    ("protocol", "protocol"),
                    ("deployment", "deployment"),
                    ("auth_mode", "authMode"),
                    ("endpoint", "endpoint"),
                    ("credential_pool_id", "credentialPoolId"),
                    ("credential_pool_revision", "credentialPoolRevision"),
                    ("proxy_mode", "proxyMode"),
                    ("network_metadata_json", "networkMetadata"),
                    ("availability_intent", "availabilityIntent"),
                ],
            ),
            (
                "models",
                "/models/0",
                vec![
                    ("model_id", "modelId"),
                    ("connection_id", "connectionId"),
                    ("actual_model_id", "actualModelId"),
                    ("display_name", "displayName"),
                    ("selection_source", "selectionSource"),
                    ("capability_json", "capabilities"),
                ],
            ),
            (
                "templates",
                "/templates/0",
                vec![
                    ("template_id", "templateId"),
                    ("purpose", "purpose"),
                    ("display_name", "displayName"),
                    ("source_kind", "sourceKind"),
                    ("source_version", "sourceVersion"),
                    ("template_body_json", "body"),
                    ("template_variables_json", "variables"),
                    ("compatibility_json", "compatibility"),
                ],
            ),
            (
                "purpose_bindings",
                "/bindings/0",
                vec![
                    ("binding_id", "bindingId"),
                    ("scope_key", "scopeKey"),
                    ("game_id", "gameId"),
                    ("purpose", "purpose"),
                    ("binding_mode", "bindingMode"),
                    ("enabled", "enabled"),
                    ("output_mode", "outputMode"),
                    ("source_language", "sourceLanguage"),
                    ("target_language", "targetLanguage"),
                    ("template_id", "templateId"),
                    ("context_json", "context"),
                    ("schedule_json", "schedule"),
                ],
            ),
            (
                "binding_engines",
                "/bindings/0/engines/0",
                vec![
                    ("engine_id", "engineId"),
                    ("model_id", "modelId"),
                    ("enabled", "enabled"),
                    ("template_id", "templateId"),
                ],
            ),
            (
                "term_sets",
                "/termSets/0",
                vec![
                    ("term_set_id", "termSetId"),
                    ("scope_key", "scopeKey"),
                    ("game_id", "gameId"),
                    ("display_name", "displayName"),
                    ("enabled", "enabled"),
                ],
            ),
            (
                "terms",
                "/termSets/0/terms/0",
                vec![
                    ("term_id", "termId"),
                    ("source_text", "sourceText"),
                    ("target_text", "targetText"),
                    ("source_language", "sourceLanguage"),
                    ("target_language", "targetLanguage"),
                    ("enabled", "enabled"),
                ],
            ),
        ] {
            for (column, property) in fields {
                let pointer = format!("{prefix}/{property}");
                proofs.fields(
                    table,
                    &[(column, &pointer)],
                    &actual,
                    &expected,
                    case,
                    (
                        "Configuration typed child DTO",
                        "storage/queries/configuration.rs::save/load",
                    ),
                );
            }
        }
        for (table, prefix) in [
            ("model_parameters", "/models/0/parameters"),
            ("binding_parameters", "/bindings/0/parameters"),
            ("engine_parameters", "/bindings/0/engines/0/parameters"),
        ] {
            for key in ["temperature", "top_p", "max_tokens"] {
                for (column, property) in [("mode", "mode"), ("value_json", "value")] {
                    let pointer = format!("{prefix}/{key}/{property}");
                    proofs.fields(
                        table,
                        &[(column, &pointer)],
                        &actual,
                        &expected,
                        &format!("{case}: {key}"),
                        (
                            "ParameterMode/Option<JSON>",
                            "storage/queries/configuration.rs::save/parameters",
                        ),
                    );
                }
            }
        }
    }
    proofs.rows.into_values().map(|mut row| { row["id"]=json!(format!("RUST-MAP:{}",row["id"].as_str().unwrap())); row["status"]=json!("passed"); row["skipped"]=json!(0); row["source"]=json!("explicit production DTO optional Some/None/Unicode/parameter tri-state write/load assertions"); row }).collect()
}

use app_core::domain::{
    capture::*,
    retention::{ProtectedObjects, RetentionPolicy},
    testing::ModelTestRecordInput,
    *,
};
use rusqlite::{Connection as SqlConnection, OpenFlags, types::ValueRef};
/// Explicit expected business/derived values determine the Rust decoder. No DDL
/// or field registry is consulted to discover values or produce passing rows.
fn row(
    conn: &SqlConnection,
    proofs: &mut BTreeMap<String, Value>,
    table: &str,
    selector: &str,
    fields: &[(&str, Value)],
    source: &str,
) {
    let sql = format!(
        "SELECT {} FROM {table} WHERE {selector}",
        fields.iter().map(|(c, _)| *c).collect::<Vec<_>>().join(",")
    );
    let mut statement = conn.prepare(&sql).unwrap();
    let mut rows = statement.query([]).unwrap();
    let record = rows
        .next()
        .unwrap()
        .unwrap_or_else(|| panic!("missing actual producer row {table} {selector}"));
    for (index, (column, expected)) in fields.iter().enumerate() {
        let actual = if column.ends_with("_json") && !expected.is_null() {
            serde_json::from_str::<Value>(&record.get::<_, String>(index).unwrap()).unwrap()
        } else {
            match expected {
                Value::Null => {
                    assert!(matches!(record.get_ref(index).unwrap(), ValueRef::Null));
                    Value::Null
                }
                Value::Bool(_) => json!(record.get::<_, bool>(index).unwrap()),
                Value::Number(n) => {
                    if n.is_f64() {
                        json!(record.get::<_, f64>(index).unwrap())
                    } else {
                        json!(record.get::<_, i64>(index).unwrap())
                    }
                }
                Value::String(_) => json!(record.get::<_, String>(index).unwrap()),
                Value::Array(_) | Value::Object(_) => {
                    serde_json::from_str::<Value>(&record.get::<_, String>(index).unwrap()).unwrap()
                }
            }
        };
        assert_eq!(
            &actual, expected,
            "actual producer typed mapping {table}.{column}: {source}"
        );
        let id = format!("RUST-MAP:profile.{table}.{column}");
        let proof=proofs.entry(id.clone()).or_insert_with(||json!({"id":id,"status":"passed","assertions":0,"skipped":0,"source":source,"cases":[]}));
        proof["assertions"] = json!(proof["assertions"].as_u64().unwrap() + 1);
        proof["cases"].as_array_mut().unwrap().push(json!(source));
    }
    assert!(
        rows.next().unwrap().is_none(),
        "field witness must identify one actual producer row"
    );
}
fn key(column: &str, value: &str) -> String {
    format!("{column}='{}'", value.replace('\'', "''"))
}
fn rid(prefix: &str, value: &str) -> String {
    format!("{prefix}-{}", content_hash(value))
}
fn inspect(path: &Path) -> SqlConnection {
    let c = SqlConnection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    c.execute_batch("PRAGMA query_only=ON").unwrap();
    c
}
fn scalar<T: rusqlite::types::FromSql>(c: &SqlConnection, sql: &str) -> T {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn resource(kind: &str) -> ResourceSnapshotInput {
    ResourceSnapshotInput {
        snapshot_id: format!("resource-{kind}"),
        kind: kind.into(),
        version: "actual-fixture-v1".into(),
        manifest_sha256: content_hash(kind),
        contract_version: "1".into(),
        metadata_json: "{\"source\":\"storage-acceptance-controlled\"}".into(),
    }
}
fn event(sequence: &str, text: &str) -> HookTextInput {
    HookTextInput {
        session_id: "hook-fields".into(),
        event_id: format!("event-{sequence}"),
        sequence_u64: sequence.into(),
        source: HookSourceInput {
            source_id: "source-fields".into(),
            host_instance_id: "host-fields".into(),
            hook_address_u64: u64::MAX.to_string(),
            context_u64: "0".into(),
            subcontext_u64: (1u64 << 63).to_string(),
            hook_code: "HW-identity".into(),
            hook_name: "controlled-source".into(),
            encoding: "utf-16le".into(),
            stream_identity_sha256: content_hash("stream-fields"),
            identity_extensions_json: "{\"v\":1}".into(),
        },
        text: text.into(),
        source_language: "ja".into(),
    }
}
fn check_configuration(
    c: &SqlConnection,
    p: &mut BTreeMap<String, Value>,
    configuration: &Configuration,
    parent: Option<&str>,
    now: i64,
) {
    let rev = &configuration.revision_id;
    let selector = key("revision_id", rev);
    let hash = canonical_hash(configuration).unwrap();
    row(
        c,
        p,
        "config_revisions",
        &selector,
        &[
            ("revision_id", json!(rev)),
            ("parent_revision_id", json!(parent)),
            ("content_hash", json!(hash)),
            ("created_at_ms", json!(now)),
            ("save_reason", json!("edit")),
            ("local_validation_json", json!({})),
        ],
        "SaveConfiguration full typed DTO and immutable content-hash lineage",
    );
    row(
        c,
        p,
        "config_revision_seals",
        &selector,
        &[
            ("revision_id", json!(rev)),
            ("content_hash", json!(hash)),
            ("sealed_at_ms", json!(now)),
        ],
        "SaveConfiguration atomically seals exact canonical configuration",
    );
    for con in &configuration.connections {
        let sel = format!(
            "{selector} AND {}",
            key("connection_id", &con.connection_id)
        );
        row(
            c,
            p,
            "connections",
            &sel,
            &[
                ("revision_id", json!(rev)),
                ("connection_id", json!(con.connection_id)),
                ("display_name", json!(con.display_name)),
                ("protocol", json!(con.protocol)),
                ("deployment", json!(con.deployment)),
                ("auth_mode", json!(con.auth_mode)),
                ("endpoint", json!(con.endpoint)),
                ("credential_pool_id", json!(con.credential_pool_id)),
                (
                    "credential_pool_revision",
                    json!(con.credential_pool_revision),
                ),
                ("proxy_mode", json!(con.proxy_mode)),
                ("network_metadata_json", json!(con.network_metadata)),
                ("availability_intent", json!(con.availability_intent)),
            ],
            "SaveConfiguration Connection DTO + version relationship",
        );
    }
    for m in &configuration.models {
        let sel = format!("{selector} AND {}", key("model_id", &m.model_id));
        row(
            c,
            p,
            "models",
            &sel,
            &[
                ("revision_id", json!(rev)),
                ("model_id", json!(m.model_id)),
                ("connection_id", json!(m.connection_id)),
                ("actual_model_id", json!(m.actual_model_id)),
                ("display_name", json!(m.display_name)),
                ("selection_source", json!(m.selection_source)),
                ("capability_json", json!(m.capabilities)),
            ],
            "SaveConfiguration Model DTO exact connection and revision",
        );
        for (purpose, template) in &m.default_templates {
            row(
                c,
                p,
                "model_default_templates",
                &format!("{sel} AND {}", key("purpose", purpose)),
                &[
                    ("revision_id", json!(rev)),
                    ("model_id", json!(m.model_id)),
                    ("purpose", json!(purpose)),
                    ("template_id", json!(template)),
                ],
                "real model default typed SaveConfiguration + reopen DTO + compose request template relationship",
            );
        }
        for (name, param) in &m.parameters {
            row(
                c,
                p,
                "model_parameters",
                &format!("{sel} AND {}", key("parameter_key", name)),
                &[
                    ("revision_id", json!(rev)),
                    ("model_id", json!(m.model_id)),
                    ("parameter_key", json!(name)),
                    (
                        "mode",
                        json!(match param.mode {
                            ParameterMode::Set => "set",
                            ParameterMode::Inherit => "inherit",
                            ParameterMode::Omit => "omit",
                        }),
                    ),
                    ("value_json", param.value.clone().unwrap_or(Value::Null)),
                ],
                "actual typed model parameter set/inherit/omit value and composite identity",
            );
        }
    }
    for t in &configuration.templates {
        row(
            c,
            p,
            "templates",
            &format!("{selector} AND {}", key("template_id", &t.template_id)),
            &[
                ("revision_id", json!(rev)),
                ("template_id", json!(t.template_id)),
                ("purpose", json!(t.purpose)),
                ("display_name", json!(t.display_name)),
                ("source_kind", json!(t.source_kind)),
                ("source_version", json!(t.source_version)),
                ("template_body_json", json!(t.body)),
                ("template_variables_json", json!(t.variables)),
                ("compatibility_json", json!(t.compatibility)),
                (
                    "content_hash",
                    json!(
                        canonical_hash(&(&t.purpose, &t.body, &t.variables, &t.compatibility))
                            .unwrap()
                    ),
                ),
            ],
            "real Template DTO canonical body/variable/compatibility hash",
        );
    }
    for b in &configuration.bindings {
        let sel = format!("{selector} AND {}", key("binding_id", &b.binding_id));
        row(
            c,
            p,
            "purpose_bindings",
            &sel,
            &[
                ("revision_id", json!(rev)),
                ("binding_id", json!(b.binding_id)),
                ("scope_key", json!(b.scope_key)),
                ("game_id", json!(b.game_id)),
                ("purpose", json!(b.purpose)),
                ("binding_mode", json!(b.binding_mode)),
                ("enabled", json!(b.enabled)),
                ("output_mode", json!(b.output_mode)),
                ("source_language", json!(b.source_language)),
                ("target_language", json!(b.target_language)),
                ("template_id", json!(b.template_id)),
                ("context_json", json!(b.context)),
                ("schedule_json", json!(b.schedule)),
            ],
            "real purpose binding DTO with typed JSON and scope relationship",
        );
        for (name, param) in &b.parameters {
            row(
                c,
                p,
                "binding_parameters",
                &format!("{sel} AND {}", key("parameter_key", name)),
                &[
                    ("revision_id", json!(rev)),
                    ("binding_id", json!(b.binding_id)),
                    ("parameter_key", json!(name)),
                    (
                        "mode",
                        json!(match param.mode {
                            ParameterMode::Set => "set",
                            ParameterMode::Inherit => "inherit",
                            ParameterMode::Omit => "omit",
                        }),
                    ),
                    ("value_json", param.value.clone().unwrap_or(Value::Null)),
                ],
                "typed binding parameter tri-state and version relationship",
            );
        }
        for (position, e) in b.engines.iter().enumerate() {
            let esel = format!("{sel} AND {}", key("engine_id", &e.engine_id));
            row(
                c,
                p,
                "binding_engines",
                &esel,
                &[
                    ("revision_id", json!(rev)),
                    ("binding_id", json!(b.binding_id)),
                    ("engine_id", json!(e.engine_id)),
                    ("purpose", json!(b.purpose)),
                    ("model_id", json!(e.model_id)),
                    ("position", json!(position)),
                    ("enabled", json!(e.enabled)),
                    ("template_id", json!(e.template_id)),
                ],
                "typed binding engine DTO + exact order/purpose/model relationship",
            );
            for (name, param) in &e.parameters {
                row(
                    c,
                    p,
                    "engine_parameters",
                    &format!("{esel} AND {}", key("parameter_key", name)),
                    &[
                        ("revision_id", json!(rev)),
                        ("binding_id", json!(b.binding_id)),
                        ("engine_id", json!(e.engine_id)),
                        ("parameter_key", json!(name)),
                        (
                            "mode",
                            json!(match param.mode {
                                ParameterMode::Set => "set",
                                ParameterMode::Inherit => "inherit",
                                ParameterMode::Omit => "omit",
                            }),
                        ),
                        ("value_json", param.value.clone().unwrap_or(Value::Null)),
                    ],
                    "typed engine parameter tri-state exact composite identity",
                );
            }
        }
    }
    for ts in &configuration.term_sets {
        let sel = format!("{selector} AND {}", key("term_set_id", &ts.term_set_id));
        row(
            c,
            p,
            "term_sets",
            &sel,
            &[
                ("revision_id", json!(rev)),
                ("term_set_id", json!(ts.term_set_id)),
                ("scope_key", json!(ts.scope_key)),
                ("game_id", json!(ts.game_id)),
                ("display_name", json!(ts.display_name)),
                ("enabled", json!(ts.enabled)),
                ("match_rules_json", json!({})),
                ("content_hash", json!(canonical_hash(&ts.terms).unwrap())),
            ],
            "typed glossary scope and canonical terms content hash",
        );
        for (position, t) in ts.terms.iter().enumerate() {
            row(
                c,
                p,
                "terms",
                &format!("{sel} AND {}", key("term_id", &t.term_id)),
                &[
                    ("revision_id", json!(rev)),
                    ("term_set_id", json!(ts.term_set_id)),
                    ("term_id", json!(t.term_id)),
                    ("source_text", json!(t.source_text)),
                    ("target_text", json!(t.target_text)),
                    ("source_language", json!(t.source_language)),
                    ("target_language", json!(t.target_language)),
                    ("position", json!(position)),
                    ("enabled", json!(t.enabled)),
                    ("match_metadata_json", json!({})),
                ],
                "typed glossary term null/Some exact position and owner relationship",
            );
        }
    }
}
fn check_task(
    c: &SqlConnection,
    p: &mut BTreeMap<String, Value>,
    t: &TaskDispatch,
    operation: &str,
    now: i64,
    finish: Option<i64>,
    state: &str,
) {
    let plan = app_core::application::request_plan::compose(t, &t.frozen_history).unwrap();
    let r = resource("language");
    let cache_input = json!({"schema":"fixed-cache-input-v2","profileId":"field-profile","sourceHash":t.unit.source_hash,"textRevision":t.unit.text_revision,"sourceLanguage":"ja","processingRevision":"hook-bounded-quoted-growth-v3","processingSnapshot":{"policy":"bounded-quoted-growth-v3","quotedGrowth":"unique-open-quote-prefix-growth-v1","prefixBytes":null,"idleMs":CAPTURE_IDLE_MS,"idleFinalizesIncomplete":false,"completion":"balanced-terminal-v2","maxAgeMs":CAPTURE_MAX_AGE_MS,"maxBytes":CAPTURE_MAX_BYTES,"maxRevisions":CAPTURE_MAX_REVISIONS,"maxRefs":CAPTURE_MAX_REFS,"fold":"NewUnit","join":"exact-ref-concatenation","unitProjection":"latest"},"lexical":{"analysisRevision":"field-align-v1","tokenSchema":"language-v1","resource":{"kind":"language","version":r.version,"manifestSha256":r.manifest_sha256,"contractVersion":r.contract_version,"metadata":{"source":"storage-acceptance-controlled"}}}});
    assert_eq!(
        t.request_semantics_hash,
        canonical_hash(&(
            "model-request-semantics-v2",
            &plan,
            &cache_input,
            &t.connection.credential_pool_id,
            &t.connection.credential_pool_revision,
            Some("credential-typed")
        ))
        .unwrap(),
        "actual task semantic identity includes fixed revision/processing/resource inputs, composed plan and selected immutable credential"
    );
    let mut snapshot = json!({"requestPlan":plan,"cacheInput":cache_input,"cacheReadPolicy":"use","sourceHash":t.unit.source_hash,"operationId":operation,"selection":t.selected_token,"contextRefs":t.frozen_history.iter().map(|u|json!({"unitId":u.unit_id,"textRevision":u.text_revision,"sourceHash":u.source_hash})).collect::<Vec<_>>()});
    if t.binding.purpose == "translation" {
        // This fixture admits its first translation request for this fixed unit.
        // Keep every original exact field assertion and independently expect the
        // typed empty causal frontier, rather than copying the stored metadata.
        snapshot["translationStatusLineage"] = serde_json::to_value(
            app_core::domain::translation_status::TranslationStatusLineage::new(vec![]).unwrap(),
        )
        .unwrap();
    }
    row(
        c,
        p,
        "request_tasks",
        &key("task_id", &t.task_id),
        &[
            ("task_id", json!(t.task_id)),
            ("unit_id", json!(t.unit.unit_id)),
            ("text_revision", json!(t.unit.text_revision)),
            ("session_id", json!(t.unit.session_id)),
            ("scope_key", json!(t.binding.scope_key)),
            ("purpose", json!(t.binding.purpose)),
            ("config_revision_id", json!(t.configuration.revision_id)),
            ("binding_id", json!(t.binding.binding_id)),
            ("engine_id", json!(t.engine.engine_id)),
            ("profile_generation", json!(t.profile_generation)),
            ("server_instance_id", json!(t.server_instance_id)),
            ("authorization_epoch", json!(t.authorization_epoch)),
            ("request_semantics_sha256", json!(t.request_semantics_hash)),
            ("input_snapshot_json", snapshot),
            ("state", json!(state)),
            ("invalidation_code", Value::Null),
            ("created_at_ms", json!(now)),
            ("finished_at_ms", json!(finish)),
        ],
        "real prepare_tasks TaskDispatch/request-plan exact immutable snapshot and authority; no network request is claimed",
    );
    row(
        c,
        p,
        "request_attempts",
        &key("attempt_id", &t.attempt_id),
        &[
            ("attempt_id", json!(t.attempt_id)),
            ("task_id", json!(t.task_id)),
            ("attempt_number", json!(1)),
            ("config_revision_id", json!(t.configuration.revision_id)),
            ("binding_id", json!(t.binding.binding_id)),
            ("purpose", json!(t.binding.purpose)),
            ("actual_engine_id", json!(t.engine.engine_id)),
            ("credential_id", json!("credential-typed")),
            (
                "dispatch_kind",
                json!(if t.cached_result.is_some() {
                    "cache"
                } else {
                    "initial"
                }),
            ),
            (
                "state",
                json!(if finish.is_some() {
                    "completed"
                } else {
                    "started"
                }),
            ),
            ("error_code", Value::Null),
            (
                "finish_reason",
                if finish.is_some() {
                    json!("stop")
                } else {
                    Value::Null
                },
            ),
            ("usage_json", json!({})),
            ("started_at_ms", json!(now)),
            ("finished_at_ms", json!(finish)),
        ],
        "real prepare/complete attempt typed credential, lifecycle, finish-reason, no invented provider usage",
    );
}
pub fn verify_profile_fields() -> Vec<Value> {
    let mut proofs: BTreeMap<String, Value> = verify_owned_fields()
        .into_iter()
        .map(|v| (v["id"].as_str().unwrap().to_owned(), v))
        .collect();
    let out = std::env::var_os("FP_STORAGE_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join(".build/acceptance-closure-20261019/storage/field-work"))
        .join(format!(
            "profile-fields-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&out).unwrap();
    let path = out.join("profile.sqlite3");
    let s = SqliteStore::open(&path, "field-profile", "field-server", 10).unwrap();
    let c = inspect(&path);
    row(
        &c,
        &mut proofs,
        "schema_migrations",
        "version=1",
        &[
            ("version", json!(1)),
            (
                "migration_sha256",
                json!(app_infra::storage::profile_migration_sha256()),
            ),
            ("applied_at_ms", json!(10)),
            ("app_build", json!(env!("CARGO_PKG_VERSION"))),
            ("previous_version", json!(0)),
        ],
        "actual SqliteStore fresh production migration ledger/build/version",
    );
    row(
        &c,
        &mut proofs,
        "profile_meta",
        "singleton=1",
        &[
            ("singleton", json!(1)),
            ("profile_id", json!("field-profile")),
            ("commit_revision", json!(0)),
            ("runtime_generation", json!(1)),
            ("server_instance_id", json!("field-server")),
            ("runtime_state", json!("active")),
            ("created_at_ms", json!(10)),
            ("updated_at_ms", json!(10)),
        ],
        "fresh actual profile authority metadata",
    );
    let input = game_input(true);
    let envelope = CommandEnvelope {
        operation_id: "field-add-game".into(),
        expected_revision: Some(0),
        command: Command::SaveGame(Box::new(input.clone())),
    };
    let receipt = s.dispatch(envelope.clone(), 20).unwrap();
    let gid = receipt.result_id.clone().unwrap();
    let g = s.snapshot().unwrap().games.remove(0);
    assert_eq!(g.display_name, input.display_name);
    assert!(s.dispatch(envelope.clone(), 21).unwrap().replayed);
    row(
        &c,
        &mut proofs,
        "operation_receipts",
        &key("operation_id", "field-add-game"),
        &[
            ("operation_id", json!(envelope.operation_id)),
            (
                "request_sha256",
                json!(canonical_hash(&envelope.command).unwrap()),
            ),
            ("command_kind", json!("save_game")),
            ("commit_revision", json!(receipt.revision)),
            ("result_refs_json", serde_json::to_value(&receipt).unwrap()),
            ("committed_at_ms", json!(20)),
        ],
        "real SaveGame command/request canonical hash, committed receipt/replay exact result",
    );
    row(
        &c,
        &mut proofs,
        "games",
        &key("game_id", &gid),
        &[
            ("game_id", json!(gid)),
            ("display_name", json!(g.display_name)),
            ("original_title", json!(g.original_title)),
            ("source_language", json!(g.source_language)),
            ("cover_asset_ref", json!(g.cover_asset_ref)),
            ("row_revision", json!(1)),
            ("created_at_ms", json!(20)),
            ("updated_at_ms", json!(20)),
            ("archived_at_ms", Value::Null),
        ],
        "real game DTO + created/updated/archive lifecycle metadata",
    );
    let targetid = rid("target", &gid);
    row(
        &c,
        &mut proofs,
        "game_targets",
        &key("target_id", &targetid),
        &[
            ("target_id", json!(targetid)),
            ("game_id", json!(gid)),
            ("environment_kind", json!(g.target.environment_kind)),
            ("environment_key", json!(g.target.environment_key)),
            ("bottle_locator", json!(g.target.bottle_locator)),
            ("launch_path", json!(g.target.launch_path)),
            ("canonical_exe_path", json!(g.target.canonical_exe_path)),
            ("target_path_key", json!(g.target.target_path_key)),
            ("working_directory", json!(g.target.working_directory)),
            ("arguments_json", json!(g.target.arguments)),
            ("architecture", json!(g.target.architecture)),
            ("verification_state", json!(g.target.verification_state)),
            ("row_revision", json!(1)),
            ("updated_at_ms", json!(20)),
            ("archived_at_ms", Value::Null),
        ],
        "actual normalized game target DTO, stable SHA target identity, exact lifecycle metadata",
    );
    let prefs = Preferences::default();
    let pref = save(
        &s,
        "field-pref",
        Command::SavePreferences {
            preferences: prefs.clone(),
        },
        30,
    )
    .result_id
    .unwrap();
    row(
        &c,
        &mut proofs,
        "preference_revisions",
        &key("preference_revision_id", &pref),
        &[
            ("preference_revision_id", json!(pref)),
            ("scope_key", json!(prefs.scope_key)),
            ("game_id", Value::Null),
            ("parent_revision_id", Value::Null),
            ("theme", json!(prefs.theme)),
            ("transparency_mode", json!(prefs.transparency_mode)),
            ("show_ruby", json!(prefs.show_ruby)),
            ("show_pos", json!(prefs.show_pos)),
            ("source_font_pt", json!(prefs.source_font_pt)),
            ("translation_font_pt", json!(prefs.translation_font_pt)),
            ("hud_width_pt", json!(prefs.hud_width_pt)),
            ("shortcuts_json", json!(prefs.shortcuts)),
            ("overrides_json", json!(prefs.overrides)),
            ("created_at_ms", json!(30)),
        ],
        "actual typed SavePreferences initial immutable revision",
    );
    row(
        &c,
        &mut proofs,
        "preference_heads",
        "scope_key='global'",
        &[
            ("scope_key", json!("global")),
            ("preference_revision_id", json!(pref)),
            ("head_revision", json!(1)),
            ("updated_at_ms", json!(30)),
        ],
        "SavePreferences atomic pointer and CAS head",
    );
    let second_pref = save(
        &s,
        "field-pref-next",
        Command::SavePreferences {
            preferences: prefs.clone(),
        },
        31,
    )
    .result_id
    .unwrap();
    row(
        &c,
        &mut proofs,
        "preference_revisions",
        &key("preference_revision_id", &second_pref),
        &[
            ("parent_revision_id", json!(pref)),
            ("created_at_ms", json!(31)),
        ],
        "actual preference edit records immutable parent Some plus atomic current-head advancement",
    );
    row(
        &c,
        &mut proofs,
        "preference_heads",
        "scope_key='global'",
        &[
            ("preference_revision_id", json!(second_pref)),
            ("head_revision", json!(2)),
            ("updated_at_ms", json!(31)),
        ],
        "actual second preference revision head CAS",
    );
    let credential = CredentialReference {
        credential_id: "credential-typed".into(),
        credential_version_id: "vault-typed".into(),
        pool_id: "pool-typed".into(),
        pool_revision: u32::MAX,
        label: "fake-reference".into(),
        masked_hint: "••••0000".into(),
    };
    s.begin_credential(credential.clone(), 40).unwrap();
    s.activate_credential(credential.credential_id.clone(), 41)
        .unwrap();
    let poolhash = canonical_hash(&(
        &credential.pool_id,
        credential.pool_revision,
        &credential.credential_id,
    ))
    .unwrap();
    let member = rid("member", &credential.credential_id);
    row(
        &c,
        &mut proofs,
        "credential_pool_revisions",
        "pool_id='pool-typed'",
        &[
            ("pool_id", json!(credential.pool_id)),
            ("pool_revision", json!(u32::MAX)),
            ("policy", json!("selected")),
            ("policy_metadata_json", json!({})),
            ("created_at_ms", json!(40)),
            ("content_hash", json!(poolhash)),
        ],
        "actual begin_credential durable immutable pool, u32 MAX typed revision and canonical source hash",
    );
    row(
        &c,
        &mut proofs,
        "credential_pool_members",
        "pool_id='pool-typed'",
        &[
            ("pool_id", json!(credential.pool_id)),
            ("pool_revision", json!(u32::MAX)),
            ("member_id", json!(member)),
            ("credential_id", json!(credential.credential_id)),
            ("position", json!(0)),
            ("enabled", json!(true)),
            ("weight", json!(1)),
        ],
        "actual credential registration derives stable member identity/order/weight",
    );
    row(
        &c,
        &mut proofs,
        "credential_pool_seals",
        "pool_id='pool-typed'",
        &[
            ("pool_id", json!(credential.pool_id)),
            ("pool_revision", json!(u32::MAX)),
            ("content_hash", json!(poolhash)),
            ("sealed_at_ms", json!(41)),
        ],
        "actual activation seals exact credential pool content",
    );
    let mut configuration = configuration(true);
    configuration.connections[0].network_metadata = BTreeMap::new();
    configuration.connections[0].proxy_mode = "disabled".into();
    configuration.templates[0]
        .body
        .insert("system".into(), json!("FIELD-MODEL-DEFAULT"));
    configuration.templates[0]
        .body
        .insert("user".into(), json!("{{source}}"));
    configuration.bindings[0].template_id = None;
    configuration.bindings[0].engines[0].template_id = None;
    configuration.models[0]
        .default_templates
        .insert("translation".into(), "template-typed".into());
    configuration.bindings[0]
        .schedule
        .insert("cacheTtlMs".into(), json!(10000));
    for purpose in ["grammar", "context_word"] {
        let mut b = configuration.bindings[0].clone();
        b.binding_id = format!("binding-{purpose}");
        b.purpose = purpose.into();
        b.output_mode = Some(
            if purpose == "grammar" {
                "plain_text"
            } else {
                "structured"
            }
            .into(),
        );
        b.engines[0].engine_id = format!("engine-{purpose}");
        b.schedule.remove("cacheTtlMs");
        configuration.bindings.push(b);
    }
    configuration
        .bindings
        .sort_by(|a, b| a.binding_id.cmp(&b.binding_id));
    let configrev = save(
        &s,
        "field-config",
        Command::SaveConfiguration {
            configuration: configuration.clone(),
            expected_head_revision: 0,
        },
        50,
    )
    .result_id
    .unwrap();
    configuration.revision_id = configrev.clone();
    assert_eq!(s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone(), configuration);
    check_configuration(&c, &mut proofs, &configuration, None, 50);
    save(
        &s,
        "field-apply",
        Command::ApplyConfiguration {
            revision_id: configrev.clone(),
            expected_head_revision: 0,
        },
        51,
    );
    for kind in ["draft", "applied"] {
        row(
            &c,
            &mut proofs,
            "config_heads",
            &key("head_kind", kind),
            &[
                ("head_kind", json!(kind)),
                ("revision_id", json!(configrev)),
                ("head_revision", json!(1)),
                (
                    "updated_at_ms",
                    json!(if kind == "draft" { 50 } else { 51 }),
                ),
            ],
            "real draft save and explicit applied-head promotion",
        );
    }
    row(
        &c,
        &mut proofs,
        "connection_runtime_states",
        "connection_id='con-typed'",
        &[
            ("connection_id", json!("con-typed")),
            ("config_revision_id", json!(configrev)),
            ("is_suspended", json!(false)),
            ("control_revision", json!(1)),
            ("reason_code", Value::Null),
            ("updated_at_ms", json!(51)),
        ],
        "explicit ApplyConfiguration initializes usable connection controls",
    );
    for b in &configuration.bindings {
        let h = binding_semantics(&configuration, b);
        row(
            &c,
            &mut proofs,
            "purpose_runtime_states",
            &format!("scope_key='global' AND {}", key("purpose", &b.purpose)),
            &[
                ("scope_key", json!("global")),
                ("purpose", json!(b.purpose)),
                ("binding_id", json!(b.binding_id)),
                ("config_revision_id", json!(configrev)),
                ("is_enabled", json!(true)),
                ("authorization_epoch", json!(1)),
                ("config_semantics_sha256", json!(h)),
                ("updated_at_ms", json!(51)),
            ],
            "ApplyConfiguration exact resolved authority semantics for real global purpose binding",
        );
    }
    s.open_hook_session(
        HookSessionInput {
            session_id: "hook-fields".into(),
            game_id: Some(gid.clone()),
            environment_key: "crossover:/private/test-bottle".into(),
            target_exe_path: "C:\\Games\\実行.exe".into(),
            windows_pid: u32::MAX,
            process_created_u64: u64::MAX.to_string(),
            architecture: "x64".into(),
            broker_instance_id: "broker-fields".into(),
            resource: resource("core"),
            target_identity_sha256: content_hash("actual-controlled-target"),
        },
        60,
    )
    .unwrap();
    let e = event("18446744073709551614", "候補");
    assert!(s.receive_hook_text(e.clone(), 61).unwrap().is_none());
    save(
        &s,
        "field-select",
        Command::SelectSource {
            source_id: "source-fields".into(),
        },
        62,
    );
    let text = "図書館😀";
    let e2 = event("18446744073709551615", text);
    let version = s
        .ingest_hook_text(e2.clone(), 63)
        .unwrap()
        .visible_version
        .unwrap();
    assert_eq!(
        s.unit(version.unit_id.clone(), 1).unwrap().source_text,
        text
    );
    s.flush_hook_capture(
        CaptureFlush {
            session_id: None,
            reason: CaptureFlushReason::Shutdown,
        },
        64,
    )
    .unwrap();
    row(
        &c,
        &mut proofs,
        "sessions",
        "session_id='hook-fields'",
        &[
            ("session_id", json!("hook-fields")),
            ("game_id", json!(gid)),
            ("source_kind", json!("hook")),
            ("state", json!("active")),
            ("environment_key", json!("crossover:/private/test-bottle")),
            ("target_exe_path", json!("C:\\Games\\実行.exe")),
            ("windows_pid", json!(u32::MAX)),
            ("process_created_u64", json!(u64::MAX.to_string())),
            ("architecture", json!("x64")),
            ("broker_instance_id", json!("broker-fields")),
            ("core_resource_snapshot_id", json!("resource-core")),
            (
                "target_identity_sha256",
                json!(content_hash("actual-controlled-target")),
            ),
            ("started_at_ms", json!(60)),
            ("ended_at_ms", Value::Null),
        ],
        "production Hook session exact composite process identity with u32/u64 MAX, real game/core relationships",
    );
    row(
        &c,
        &mut proofs,
        "capture_sources",
        "source_id='source-fields'",
        &[
            ("source_id", json!(e.source.source_id)),
            ("session_id", json!(e.session_id)),
            ("host_instance_id", json!(e.source.host_instance_id)),
            ("hook_address_u64", json!(e.source.hook_address_u64)),
            ("context_u64", json!(e.source.context_u64)),
            ("subcontext_u64", json!(e.source.subcontext_u64)),
            ("hook_code", json!(e.source.hook_code)),
            ("hook_name", json!(e.source.hook_name)),
            ("encoding", json!(e.source.encoding)),
            (
                "stream_identity_sha256",
                json!(e.source.stream_identity_sha256),
            ),
            ("identity_extensions_json", json!({"v":1})),
            ("discovered_at_ms", json!(61)),
        ],
        "actual hook event producer retains complete thread identity without integer truncation",
    );
    row(
        &c,
        &mut proofs,
        "source_selections",
        "session_id='hook-fields' AND selection_revision=1",
        &[
            ("session_id", json!("hook-fields")),
            ("selection_revision", json!(1)),
            ("source_id", json!("source-fields")),
            ("persistence_kind", json!("temporary")),
            ("match_snapshot_json", json!({})),
            ("selected_at_ms", json!(62)),
        ],
        "explicit SelectSource command immutable selected source/session revision",
    );
    row(
        &c,
        &mut proofs,
        "raw_events",
        &key("event_id", &e2.event_id),
        &[
            ("event_id", json!(e2.event_id)),
            ("session_id", json!(e2.session_id)),
            ("source_id", json!(e2.source.source_id)),
            ("wire_sequence_u64", json!(e2.sequence_u64)),
            ("ingest_sequence", json!(2)),
            ("event_kind", json!("text")),
            ("payload_byte_length", json!(text.len())),
            ("payload_sha256", json!(content_hash(text))),
            ("observed_at_ms", json!(63)),
            ("integrity_state", json!("intact")),
        ],
        "real hook raw-event UTF8 bytes/hash/order/full-u64 sequence retained exactly",
    );
    let blob: Vec<u8> = scalar(
        &c,
        "SELECT payload_bytes FROM raw_event_payloads WHERE event_id='event-18446744073709551615'",
    );
    assert_eq!(blob, text.as_bytes());
    proofs.insert("RUST-MAP:profile.raw_event_payloads.payload_bytes".into(),json!({"id":"RUST-MAP:profile.raw_event_payloads.payload_bytes","status":"passed","assertions":1,"skipped":0,"source":"typed Vec<u8> read equals actual HookTextInput UTF8 bytes including emoji"}));
    row(
        &c,
        &mut proofs,
        "raw_event_payloads",
        &key("event_id", &e2.event_id),
        &[
            ("event_id", json!(e2.event_id)),
            ("retained_at_ms", json!(63)),
        ],
        "actual raw BLOB producer metadata",
    );
    row(
        &c,
        &mut proofs,
        "reading_units",
        &key("unit_id", &version.unit_id),
        &[
            ("unit_id", json!(version.unit_id)),
            ("session_id", json!("hook-fields")),
            ("unit_ordinal", json!(1)),
            ("state", json!("finalized")),
            ("created_at_ms", json!(63)),
        ],
        "actual selected text aggregation and explicit bounded flush finalization",
    );
    row(
        &c,
        &mut proofs,
        "reading_unit_revisions",
        &key("unit_id", &version.unit_id),
        &[
            ("unit_id", json!(version.unit_id)),
            ("text_revision", json!(1)),
            ("session_id", json!("hook-fields")),
            ("source_text", json!(text)),
            ("source_byte_length", json!(text.len())),
            ("source_sha256", json!(content_hash(text))),
            ("source_language", json!("ja")),
            (
                "processing_revision",
                json!("hook-bounded-quoted-growth-v3"),
            ),
            (
                "processing_snapshot_json",
                json!({"policy":"bounded-quoted-growth-v3","quotedGrowth":"unique-open-quote-prefix-growth-v1","prefixBytes":null,"idleMs":CAPTURE_IDLE_MS,"idleFinalizesIncomplete":false,"completion":"balanced-terminal-v2","maxAgeMs":CAPTURE_MAX_AGE_MS,"maxBytes":CAPTURE_MAX_BYTES,"maxRevisions":CAPTURE_MAX_REVISIONS,"maxRefs":CAPTURE_MAX_REFS,"fold":"NewUnit","join":"exact-ref-concatenation","unitProjection":"latest"}),
            ),
            ("selection_revision", json!(1)),
            ("created_at_ms", json!(63)),
        ],
        "actual immutable reading revision exact UTF8 hash/length and processing/selection snapshot",
    );
    row(
        &c,
        &mut proofs,
        "source_refs",
        &key("unit_id", &version.unit_id),
        &[
            ("unit_id", json!(version.unit_id)),
            ("text_revision", json!(1)),
            ("session_id", json!("hook-fields")),
            ("ref_ordinal", json!(0)),
            ("event_id", json!(e2.event_id)),
            ("byte_start", json!(0)),
            ("byte_end", json!(text.len())),
        ],
        "source provenance exact raw span reconstructs selected original",
    );
    let token = LexicalToken {
        surface: "図書館".into(),
        lemma: "図書館".into(),
        reading: "としょかん".into(),
        part_of_speech: "名詞".into(),
        byte_start: 0,
        byte_end: 9,
        utf16_start: 0,
        utf16_end: 3,
        known: true,
    };
    s.set_tokens(
        TokenSnapshotInput {
            unit_id: version.unit_id.clone(),
            text_revision: 1,
            analysis_revision: "field-align-v1".into(),
            tokens: vec![token.clone()],
            resource: resource("language"),
        },
        65,
    )
    .unwrap();
    assert_eq!(
        s.unit(version.unit_id.clone(), 1).unwrap().tokens,
        vec![token.clone()]
    );
    let lid = rid(
        "lexical",
        &format!(
            "{}:1:{}:language-v1:field-align-v1",
            version.unit_id,
            content_hash("language")
        ),
    );
    row(
        &c,
        &mut proofs,
        "lexical_snapshots",
        &key("lexical_id", &lid),
        &[
            ("lexical_id", json!(lid)),
            ("unit_id", json!(version.unit_id)),
            ("text_revision", json!(1)),
            ("source_sha256", json!(content_hash(text))),
            ("language_resource_snapshot_id", json!("resource-language")),
            ("token_schema_version", json!("language-v1")),
            ("analysis_revision", json!("field-align-v1")),
            ("token_count", json!(1)),
            ("created_at_ms", json!(65)),
        ],
        "real set_tokens alignment validation and immutable lexical identity tied to source and language manifest",
    );
    row(
        &c,
        &mut proofs,
        "lexical_tokens",
        &key("lexical_id", &lid),
        &[
            ("lexical_id", json!(lid)),
            ("token_index", json!(0)),
            ("byte_start", json!(0)),
            ("byte_end", json!(9)),
            ("surface", json!(token.surface)),
            ("lemma", json!(token.lemma)),
            ("reading_kana", json!(token.reading)),
            ("pos_major", json!(token.part_of_speech)),
            ("features_json", json!({})),
            ("token_kind", json!("word")),
            ("is_known", json!(true)),
        ],
        "actual lexical DTO UTF8/UTF16 alignment and producer token kind without fake parser claim",
    );
    for (kind, now) in [("core", 60), ("language", 65)] {
        let r = resource(kind);
        row(
            &c,
            &mut proofs,
            "resource_snapshots",
            &key("resource_snapshot_id", &r.snapshot_id),
            &[
                ("resource_snapshot_id", json!(r.snapshot_id)),
                ("kind", json!(r.kind)),
                ("version_label", json!(r.version)),
                ("manifest_sha256", json!(r.manifest_sha256)),
                ("contract_version", json!(r.contract_version)),
                (
                    "metadata_json",
                    json!({"source":"storage-acceptance-controlled"}),
                ),
                ("recorded_at_ms", json!(now)),
            ],
            "actual Hook/token producers persist their supplied immutable resource identity",
        );
    }
    let task = s
        .prepare_tasks_for_operation(
            version.unit_id.clone(),
            1,
            "translation".into(),
            "field-translation".into(),
            70,
        )
        .unwrap()
        .remove(0);
    assert!(
        app_core::application::request_plan::compose(&task, &task.frozen_history)
            .unwrap()
            .messages[0]["content"]
            .as_str()
            .unwrap()
            .starts_with("FIELD-MODEL-DEFAULT")
    );
    assert_eq!(task.credential_vault_ref.as_deref(), Some("vault-typed"));
    check_task(
        &c,
        &mut proofs,
        &task,
        "field-translation",
        70,
        None,
        "running",
    );
    let complete = TaskCompletion {
        task_id: task.task_id.clone(),
        attempt_id: task.attempt_id.clone(),
        text: "图书馆😀".into(),
        output_mode: "text".into(),
        structured: json!({}),
        target_language: "zh-Hans".into(),
        provider_label: "controlled-local-provider".into(),
    };
    s.complete_task(complete.clone(), 71).unwrap();
    check_task(
        &c,
        &mut proofs,
        &task,
        "field-translation",
        70,
        Some(71),
        "completed",
    );
    let translation = rid("translation", &task.task_id);
    row(
        &c,
        &mut proofs,
        "translations",
        &key("translation_id", &translation),
        &[
            ("translation_id", json!(translation)),
            ("task_id", json!(task.task_id)),
            ("attempt_id", json!(task.attempt_id)),
            ("translated_text", json!(complete.text)),
            ("target_language", json!(complete.target_language)),
            ("provider_label_snapshot", json!(complete.provider_label)),
            ("created_at_ms", json!(71)),
        ],
        "actual complete_task output preserved against fixed original/attempt authority",
    );
    let correction = save(
        &s,
        "field-correction",
        Command::SaveCorrection {
            translation_id: translation.clone(),
            base_revision: 0,
            text: "人工修正🦀".into(),
        },
        72,
    )
    .result_id
    .unwrap();
    row(
        &c,
        &mut proofs,
        "corrections",
        &key("correction_id", &correction),
        &[
            ("correction_id", json!(correction)),
            ("translation_id", json!(translation)),
            ("correction_revision", json!(1)),
            ("corrected_text", json!("人工修正🦀")),
            ("base_revision", json!(0)),
            ("created_at_ms", json!(72)),
        ],
        "real SaveCorrection CAS appends independent original-preserving edit",
    );
    let cached = CachedTaskResult {
        text: complete.text.clone(),
        output_mode: complete.output_mode.clone(),
        structured: complete.structured.clone(),
        target_language: complete.target_language.clone(),
        provider_label: complete.provider_label.clone(),
    };
    let cachejson = serde_json::to_string(&cached).unwrap();
    let cachekey = canonical_hash(&(
        "request-cache-v1",
        "translation",
        &task.request_semantics_hash,
    ))
    .unwrap();
    let hit = s
        .prepare_tasks_for_operation(
            version.unit_id.clone(),
            1,
            "translation".into(),
            "field-cache-hit".into(),
            73,
        )
        .unwrap()
        .remove(0);
    assert_eq!(hit.cached_result.as_ref().unwrap().text, complete.text);
    row(
        &c,
        &mut proofs,
        "cache_entries",
        &key("cache_key", &cachekey),
        &[
            ("cache_key", json!(cachekey)),
            ("purpose", json!("translation")),
            (
                "request_semantics_sha256",
                json!(task.request_semantics_hash),
            ),
            (
                "resource_semantics_sha256",
                json!(binding_semantics(&configuration, &task.binding)),
            ),
            ("result_json", serde_json::to_value(&cached).unwrap()),
            ("result_sha256", json!(content_hash(&cachejson))),
            ("byte_size", json!(cachejson.len())),
            ("created_at_ms", json!(71)),
            ("expires_at_ms", json!(10071)),
            ("last_accessed_at_ms", json!(73)),
        ],
        "real completed result cache typed payload/hash/byte count/TTL and actual cache-hit access timestamp",
    );
    s.complete_task(
        TaskCompletion {
            task_id: hit.task_id,
            attempt_id: hit.attempt_id,
            ..complete.clone()
        },
        74,
    )
    .unwrap();
    for (index, purpose) in ["grammar", "context_word"].into_iter().enumerate() {
        let now = 80 + index as i64 * 3;
        let operation = format!("field-{purpose}");
        let study = s
            .prepare_study_tasks(
                version.unit_id.clone(),
                1,
                purpose.into(),
                (purpose == "context_word").then_some(0),
                operation.clone(),
                now,
            )
            .unwrap()
            .remove(0);
        let output = TaskCompletion {
            task_id: study.task_id.clone(),
            attempt_id: study.attempt_id.clone(),
            text: format!("受控{purpose}结果"),
            output_mode: if purpose == "grammar" {
                "plain_text"
            } else {
                "structured"
            }
            .into(),
            structured: json!({"references":[]}),
            target_language: "zh-Hans".into(),
            provider_label: "controlled-study".into(),
        };
        s.complete_task(output.clone(), now + 1).unwrap();
        check_task(
            &c,
            &mut proofs,
            &study,
            &operation,
            now,
            Some(now + 1),
            "completed",
        );
        row(
            &c,
            &mut proofs,
            "study_results",
            &key("task_id", &study.task_id),
            &[
                ("study_result_id", json!(rid("study", &study.task_id))),
                ("task_id", json!(study.task_id)),
                ("attempt_id", json!(study.attempt_id)),
                ("output_mode", json!(output.output_mode)),
                ("explanation_text", json!(output.text)),
                ("structured_json", json!(output.structured)),
                (
                    "output_schema_version",
                    if purpose == "grammar" {
                        Value::Null
                    } else {
                        json!("study-v1")
                    },
                ),
                (
                    "selection_byte_start",
                    if purpose == "grammar" {
                        Value::Null
                    } else {
                        json!(0)
                    },
                ),
                (
                    "selection_byte_end",
                    if purpose == "grammar" {
                        Value::Null
                    } else {
                        json!(9)
                    },
                ),
                ("ambiguity_json", json!([])),
                ("created_at_ms", json!(now + 1)),
            ],
            "production prepare_study_tasks/complete_task plain/structured results and immutable selected-token UTF8 span",
        );
    }
    let testcred = s
        .credential_test_reference(
            "pool-typed".into(),
            u32::MAX,
            "vault-typed".into(),
            "failed".into(),
        )
        .unwrap();
    let input = ModelTestRecordInput {
        test_id: "field-test".into(),
        config_revision_id: configrev.clone(),
        connection_id: Some("con-typed".into()),
        model_id: Some("model-typed".into()),
        binding_id: Some("binding-typed".into()),
        purpose: Some("translation".into()),
        stage: "purpose".into(),
        request_semantics_hash: content_hash("controlled-test-request"),
        input_scope: "selected_current".into(),
        unit_id: Some(version.unit_id.clone()),
        text_revision: Some(1),
        input_summary: json!({"sourceHash":content_hash(text)}),
        response_summary: json!({"bytes":0}),
        output_validation: json!({"valid":false}),
        state: "failed".into(),
        started_at_ms: 90,
        completed_at_ms: 91,
        error_code: Some("connection_failed".into()),
        error_stage: Some("network".into()),
        credentials: vec![testcred.clone()],
    };
    s.record_model_test(input.clone()).unwrap();
    assert_eq!(
        s.snapshot()
            .unwrap()
            .configuration_tests
            .iter()
            .find(|t| t.test_id == input.test_id)
            .unwrap()
            .status,
        "failed"
    );
    row(
        &c,
        &mut proofs,
        "test_records",
        "test_id='field-test'",
        &[
            ("test_id", json!(input.test_id)),
            ("config_revision_id", json!(input.config_revision_id)),
            ("connection_id", json!(input.connection_id)),
            ("model_id", json!(input.model_id)),
            ("binding_id", json!(input.binding_id)),
            ("purpose", json!(input.purpose)),
            ("stage", json!(input.stage)),
            (
                "request_semantics_hash",
                json!(input.request_semantics_hash),
            ),
            ("input_scope", json!(input.input_scope)),
            ("unit_id", json!(input.unit_id)),
            ("text_revision", json!(input.text_revision)),
            ("input_summary_json", input.input_summary),
            ("response_summary_json", input.response_summary),
            ("output_validation_json", input.output_validation),
            ("state", json!(input.state)),
            ("started_at_ms", json!(90)),
            ("completed_at_ms", json!(91)),
            ("error_code", json!(input.error_code)),
            ("error_stage", json!(input.error_stage)),
        ],
        "real record_model_test typed failed-test provenance, request identity and fixed reading object; no live model is claimed",
    );
    row(
        &c,
        &mut proofs,
        "test_record_credentials",
        "test_id='field-test'",
        &[
            ("test_id", json!("field-test")),
            ("credential_id", json!(testcred.credential_id)),
            ("pool_id", json!(testcred.pool_id)),
            ("pool_revision", json!(testcred.pool_revision)),
            ("member_id", json!(testcred.member_id)),
            ("masked_identity_snapshot", json!(testcred.masked_identity)),
            ("outcome", json!(testcred.outcome)),
        ],
        "actual credential_test_reference resolves versioned pool/member identity; recorded model test preserves masked provenance only",
    );
    let pending = s
        .prepare_tasks_for_operation(
            version.unit_id.clone(),
            1,
            "translation".into(),
            "field-invalidation".into(),
            91,
        )
        .unwrap()
        .remove(0);
    save(
        &s,
        "field-suspend",
        Command::SetConnectionSuspended {
            connection_id: "con-typed".into(),
            suspended: true,
            expected_control_revision: 1,
        },
        92,
    );
    row(
        &c,
        &mut proofs,
        "connection_runtime_states",
        "connection_id='con-typed'",
        &[
            ("connection_id", json!("con-typed")),
            ("config_revision_id", json!(configrev)),
            ("is_suspended", json!(true)),
            ("control_revision", json!(2)),
            ("reason_code", json!("user_suspended")),
            ("updated_at_ms", json!(92)),
        ],
        "explicit connection safety suspension version and reason state transition",
    );
    row(
        &c,
        &mut proofs,
        "request_tasks",
        &key("task_id", &pending.task_id),
        &[
            ("state", json!("invalidated")),
            ("invalidation_code", json!("connection_suspended")),
            ("finished_at_ms", json!(92)),
        ],
        "actual active request invalidation after explicit connection safety suspension; nullable reason/timestamp Some state",
    );
    row(
        &c,
        &mut proofs,
        "request_attempts",
        &key("attempt_id", &pending.attempt_id),
        &[
            ("state", json!("cancelled")),
            ("error_code", json!("connection_suspended")),
            ("finished_at_ms", json!(92)),
            ("finish_reason", Value::Null),
        ],
        "actual suspended active attempt cancellation and stable reason; no fabricated completion",
    );
    save(
        &s,
        "field-stop-purpose",
        Command::SetPurposeRuntimeEnabled {
            scope_key: "global".into(),
            purpose: "grammar".into(),
            enabled: false,
            expected_authorization_epoch: 1,
        },
        93,
    );
    row(
        &c,
        &mut proofs,
        "purpose_runtime_states",
        "scope_key='global' AND purpose='grammar'",
        &[
            ("scope_key", json!("global")),
            ("purpose", json!("grammar")),
            ("binding_id", json!("binding-grammar")),
            ("config_revision_id", json!(configrev)),
            ("is_enabled", json!(false)),
            ("authorization_epoch", json!(2)),
            (
                "config_semantics_sha256",
                json!(binding_semantics(
                    &configuration,
                    &configuration.bindings[1]
                )),
            ),
            ("updated_at_ms", json!(93)),
        ],
        "explicit purpose disable invalidates old authorization generation",
    );
    s.revoke_credential("credential-typed".into(), 94).unwrap();
    s.mark_credential_cleaned("credential-typed".into(), 95)
        .unwrap();
    row(
        &c,
        &mut proofs,
        "credential_refs",
        "credential_id='credential-typed'",
        &[
            ("credential_id", json!("credential-typed")),
            ("vault_ref", json!("vault-typed")),
            ("masked_label", json!("fake-reference ••••0000")),
            ("auth_kind", json!("api_key")),
            ("state", json!("cleanup_pending")),
            ("created_at_ms", json!(40)),
            ("activated_at_ms", json!(41)),
            ("revoked_at_ms", json!(94)),
            ("cleanup_requested_at_ms", json!(95)),
            ("cleaned_at_ms", json!(95)),
            ("lifecycle_revision", json!(3)),
        ],
        "actual begin/activate/revoke/mark-cleaned public lifecycle timestamps and revision; reference-only never real secret",
    );
    save(&s, "field-disconnect", Command::Disconnect, 96);
    row(
        &c,
        &mut proofs,
        "sessions",
        "session_id='hook-fields'",
        &[("state", json!("stopped")), ("ended_at_ms", json!(96))],
        "explicit Disconnect records normal Hook terminal state",
    );
    save(
        &s,
        "field-archive",
        Command::ArchiveGame {
            game_id: gid.clone(),
            row_revision: 2,
        },
        97,
    );
    row(
        &c,
        &mut proofs,
        "games",
        &key("game_id", &gid),
        &[
            ("row_revision", json!(3)),
            ("updated_at_ms", json!(97)),
            ("archived_at_ms", json!(97)),
        ],
        "real game archive preserves history and increments actual CAS",
    );
    row(
        &c,
        &mut proofs,
        "game_targets",
        &key("game_id", &gid),
        &[
            ("row_revision", json!(2)),
            ("updated_at_ms", json!(60)),
            ("archived_at_ms", json!(97)),
        ],
        "real Hook-confirmed target revision + archive target marker",
    );
    // A dedicated unreferenced configuration exercises controlled child collection
    // without mutating the exact configuration referenced by historical tasks.
    let gc = save(
        &s,
        "field-gc-config",
        Command::SaveConfiguration {
            configuration: configuration.clone(),
            expected_head_revision: 1,
        },
        100,
    )
    .result_id
    .unwrap();
    let mut gc_configuration = configuration.clone();
    gc_configuration.revision_id = gc.clone();
    check_configuration(&c, &mut proofs, &gc_configuration, Some(&configrev), 100);
    let _next = save(
        &s,
        "field-next-config",
        Command::SaveConfiguration {
            configuration: configuration.clone(),
            expected_head_revision: 2,
        },
        101,
    )
    .result_id
    .unwrap();
    let retention = s
        .maintain_storage(
            RetentionPolicy {
                max_rows_per_batch: 100,
                config_revision_ids: vec![gc.clone()],
                ..Default::default()
            },
            ProtectedObjects::default(),
            "field-config-gc".into(),
            102,
        )
        .unwrap();
    assert_eq!(retention.collected_config_snapshots, 1);
    row(
        &c,
        &mut proofs,
        "config_gc_marks",
        &key("revision_id", &gc),
        &[
            ("revision_id", json!(gc)),
            ("operation_id", json!("field-config-gc")),
            ("requested_at_ms", json!(102)),
        ],
        "real public maintain_storage controlled sealed-config collection leaves auditable tombstone",
    );
    assert_eq!(
        scalar::<u32>(
            &c,
            &format!(
                "SELECT count(*) FROM models WHERE {}",
                key("revision_id", &gc)
            )
        ),
        0
    );
    verify_pool_gc(&s, &c, &mut proofs);
    // Writer authority and restart are observable in public DTOs and actual typed
    // metadata; old immutable model default mappings remain exactly recoverable.
    let revision = s.snapshot().unwrap().commit_revision;
    row(
        &c,
        &mut proofs,
        "profile_meta",
        "singleton=1",
        &[("commit_revision", json!(revision))],
        "public snapshot commit revision equals durable writer authority",
    );
    s.checkpoint().unwrap();
    drop(c);
    drop(s);
    let s = SqliteStore::open(&path, "field-profile", "field-restarted", 200).unwrap();
    assert_eq!(s.snapshot().unwrap().profile_generation, 2);
    assert_eq!(
        s.snapshot()
            .unwrap()
            .configuration
            .applied
            .as_ref()
            .unwrap().as_legacy().expect("legacy storage fixture remains codec1/2")
            .models[0]
            .default_templates,
        configuration.models[0].default_templates
    );
    let c = inspect(&path);
    row(
        &c,
        &mut proofs,
        "profile_meta",
        "singleton=1",
        &[
            ("runtime_generation", json!(2)),
            ("server_instance_id", json!("field-restarted")),
            ("runtime_state", json!("active")),
            ("updated_at_ms", json!(200)),
        ],
        "real same-version reopen recovers authority without dropping immutable model defaults",
    );
    assert_eq!(scalar::<String>(&c, "PRAGMA integrity_check"), "ok");
    assert_eq!(
        scalar::<u32>(&c, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    drop(c);
    drop(s);
    verify_relationships(&out);
    let checks = proofs.into_values().collect::<Vec<_>>();
    fs::write(
        out.join("field-proofs.json"),
        serde_json::to_vec_pretty(&checks).unwrap(),
    )
    .unwrap();
    checks
}
fn binding_semantics(c: &Configuration, b: &PurposeBinding) -> String {
    assert_eq!(b.scope_key, "global");
    let models = b
        .engines
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| c.models.iter().find(|m| m.model_id == e.model_id))
        .map(|m| {
            (
                &m.model_id,
                &m.connection_id,
                &m.actual_model_id,
                &m.parameters,
            )
        })
        .collect::<Vec<_>>();
    let cons = models
        .iter()
        .filter_map(|m| c.connections.iter().find(|con| con.connection_id == *m.1))
        .map(|con| {
            (
                &con.protocol,
                &con.endpoint,
                &con.auth_mode,
                &con.credential_pool_id,
                &con.credential_pool_revision,
                &con.proxy_mode,
                &con.network_metadata,
                &con.availability_intent,
            )
        })
        .collect::<Vec<_>>();
    let selected = b
        .engines
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| {
            let model = c.models.iter().find(|m| m.model_id == e.model_id).unwrap();
            app_core::domain::model_config::templates::resolve_template(c, model, b, e)
                .unwrap()
                .map(|t| t.template_id.clone())
        })
        .collect::<std::collections::BTreeSet<_>>();
    let templates = c
        .templates
        .iter()
        .filter(|t| selected.contains(&t.template_id))
        .map(|t| {
            (
                &t.template_id,
                &t.purpose,
                &t.body,
                &t.variables,
                &t.compatibility,
            )
        })
        .collect::<Vec<_>>();
    let terms = c
        .term_sets
        .iter()
        .filter(|t| t.enabled && t.scope_key == "global")
        .map(|t| (&t.scope_key, &t.terms))
        .collect::<Vec<_>>();
    canonical_hash(&(b, Option::<Value>::None, models, cons, templates, terms)).unwrap()
}
fn verify_pool_gc(s: &SqliteStore, c: &SqlConnection, p: &mut BTreeMap<String, Value>) {
    let credential = CredentialReference {
        credential_id: "collectable-credential".into(),
        credential_version_id: "collectable-vault-reference".into(),
        pool_id: "collectable-pool".into(),
        pool_revision: 1,
        label: "controlled".into(),
        masked_hint: "••••2222".into(),
    };
    s.begin_credential(credential.clone(), 110).unwrap();
    s.activate_credential(credential.credential_id.clone(), 111)
        .unwrap();
    let before = scalar::<String>(
        c,
        "SELECT content_hash FROM credential_pool_seals WHERE pool_id='collectable-pool'",
    );
    let policy = RetentionPolicy {
        max_rows_per_batch: 100,
        credential_pool_versions: vec![("collectable-pool".into(), 1)],
        ..Default::default()
    };
    let protected = ProtectedObjects {
        credential_pool_versions: policy.credential_pool_versions.clone(),
        ..Default::default()
    };
    let error = s
        .maintain_storage(
            policy.clone(),
            protected,
            "field-pool-protected".into(),
            112,
        )
        .unwrap_err();
    assert_eq!(error.code, "credential_pool_in_use");
    assert_eq!(
        scalar::<u32>(
            c,
            "SELECT count(*) FROM credential_pool_members WHERE pool_id='collectable-pool'"
        ),
        1
    );
    assert_eq!(
        scalar::<u32>(
            c,
            "SELECT count(*) FROM credential_pool_gc_marks WHERE pool_id='collectable-pool'"
        ),
        0
    );
    let result = s
        .maintain_storage(
            policy.clone(),
            ProtectedObjects::default(),
            "field-pool-gc".into(),
            113,
        )
        .unwrap();
    assert_eq!(result.collected_pool_snapshots, 1);
    assert!(!result.replayed);
    row(
        c,
        p,
        "credential_pool_gc_marks",
        "pool_id='collectable-pool'",
        &[
            ("pool_id", json!("collectable-pool")),
            ("pool_revision", json!(1)),
            ("operation_id", json!("field-pool-gc")),
            ("requested_at_ms", json!(113)),
        ],
        "real public maintain_storage sealed pool GC: protected pool rejected without mutation, approved unreferenced pool members removed with immutable tombstone",
    );
    assert_eq!(
        scalar::<u32>(
            c,
            "SELECT count(*) FROM credential_pool_members WHERE pool_id='collectable-pool'"
        ),
        0
    );
    assert_eq!(
        scalar::<String>(
            c,
            "SELECT content_hash FROM credential_pool_seals WHERE pool_id='collectable-pool'"
        ),
        before
    );
    assert_eq!(
        scalar::<String>(
            c,
            "SELECT content_hash FROM credential_pool_revisions WHERE pool_id='collectable-pool'"
        ),
        before
    );
    assert!(s.select_credential("collectable-pool".into(), 1).is_err());
    let replay = s
        .maintain_storage(
            policy,
            ProtectedObjects::default(),
            "field-pool-gc".into(),
            114,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.commit_revision, result.commit_revision);
    // This is one component of STORAGE-22; the target combines it with actual
    // raw/cache/history/config maintenance checks, never treats it as all of 22.
    super::evidence::record(
        "sealed-pool-gc",
        &[("STORAGE-22", 12)],
        json!({"producer":"ProductRepository::begin_credential/activate_credential/maintain_storage","protected_rejection":"credential_pool_in_use","sealed_metadata_preserved":true,"gc_members_removed":1,"replay_verified":true}),
    );
}
fn verify_relationships(output: &Path) {
    let path = output.join("relationship-profile.sqlite3");
    let s = SqliteStore::open(&path, "relation-profile", "relation-server", 1000).unwrap();
    let game = save(
        &s,
        "relation-game",
        Command::SaveGame(Box::new(game_input(true))),
        1001,
    )
    .result_id
    .unwrap();
    // SQLite NULL uniqueness semantics must also be reachable through the actual
    // library command, instead of fabricated SQL fixture inserts.
    for n in 0..2 {
        let mut g = game_input(false);
        g.display_name = format!("Unresolved {n}");
        save(
            &s,
            &format!("relation-null-{n}"),
            Command::SaveGame(Box::new(g)),
            1002 + n,
        );
    }
    assert_eq!(s.snapshot().unwrap().games.len(), 3);
    let duplicate = s
        .dispatch(
            CommandEnvelope {
                operation_id: "relation-duplicate-target".into(),
                expected_revision: None,
                command: Command::SaveGame(Box::new(game_input(true))),
            },
            1004,
        )
        .unwrap_err();
    assert!(
        duplicate.code == "duplicate_record",
        "actual duplicate error {}",
        duplicate.code
    );
    assert_eq!(s.snapshot().unwrap().games.len(), 3);
    let mut cfg = configuration(true);
    cfg.connections[0].auth_mode = "none".into();
    cfg.connections[0].credential_pool_id = None;
    cfg.connections[0].credential_pool_revision = None;
    cfg.connections[0].proxy_mode = "disabled".into();
    cfg.connections[0].network_metadata.clear();
    cfg.templates[0]
        .body
        .insert("user".into(), json!("{{source}}"));
    let rev = save(
        &s,
        "relation-config",
        Command::SaveConfiguration {
            configuration: cfg.clone(),
            expected_head_revision: 0,
        },
        1005,
    )
    .result_id
    .unwrap();
    save(
        &s,
        "relation-apply",
        Command::ApplyConfiguration {
            revision_id: rev.clone(),
            expected_head_revision: 0,
        },
        1006,
    );
    let other = save(
        &s,
        "relation-other-config",
        Command::SaveConfiguration {
            configuration: cfg,
            expected_head_revision: 1,
        },
        1007,
    )
    .result_id
    .unwrap();
    let a = save(
        &s,
        "relation-manual-a",
        Command::ManualInput {
            text: "図書館".into(),
            source_language: "ja".into(),
        },
        1010,
    )
    .result_id
    .unwrap();
    let b = save(
        &s,
        "relation-manual-b",
        Command::ManualInput {
            text: "駅".into(),
            source_language: "ja".into(),
        },
        1011,
    )
    .result_id
    .unwrap();
    let au = s.unit(a.clone(), 1).unwrap();
    let bu = s.unit(b.clone(), 1).unwrap();
    let task = s
        .prepare_tasks_for_operation(
            a.clone(),
            1,
            "translation".into(),
            "relation-request".into(),
            1012,
        )
        .unwrap()
        .remove(0);
    s.open_hook_session(
        HookSessionInput {
            session_id: "relation-hook".into(),
            game_id: Some(game.clone()),
            environment_key: "crossover:/private/test-bottle".into(),
            target_exe_path: "C:\\Games\\実行.exe".into(),
            windows_pid: 100,
            process_created_u64: "100".into(),
            architecture: "x64".into(),
            broker_instance_id: "relation-broker".into(),
            resource: resource("core"),
            target_identity_sha256: content_hash("relation-target"),
        },
        1013,
    )
    .unwrap();
    let rw = SqlConnection::open(&path).unwrap();
    rw.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    assert_eq!(scalar::<u32>(&rw, "PRAGMA foreign_keys"), 1);
    fn reject(c: &SqlConnection, sql: &str, fragment: &str) {
        c.execute_batch("SAVEPOINT invalid_relation").unwrap();
        let error = c.execute(sql, []).unwrap_err();
        assert!(
            error.to_string().contains(fragment),
            "expected specific rejection {fragment}; actual {error}; SQL {sql}"
        );
        c.execute_batch("ROLLBACK TO invalid_relation;RELEASE invalid_relation")
            .unwrap();
        assert_eq!(scalar::<u32>(c, "PRAGMA foreign_keys"), 1);
    }
    reject(
        &rw,
        &format!(
            "INSERT INTO request_tasks SELECT 'cross-session-task',unit_id,text_revision,'{}',scope_key,purpose,config_revision_id,binding_id,engine_id,profile_generation,server_instance_id,authorization_epoch,request_semantics_sha256,input_snapshot_json,'queued',invalidation_code,created_at_ms,finished_at_ms FROM request_tasks WHERE {}",
            bu.session_id,
            key("task_id", &task.task_id)
        ),
        "FOREIGN KEY constraint failed",
    );
    reject(
        &rw,
        &format!(
            "INSERT INTO request_attempts SELECT 'cross-config-attempt',task_id,2,'{other}',binding_id,purpose,actual_engine_id,credential_id,dispatch_kind,'started',NULL,NULL,usage_json,started_at_ms,NULL FROM request_attempts WHERE {}",
            key("attempt_id", &task.attempt_id)
        ),
        "FOREIGN KEY constraint failed",
    );
    reject(
        &rw,
        &format!("DELETE FROM games WHERE {}", key("game_id", &game)),
        "FOREIGN KEY constraint failed",
    );
    reject(
        &rw,
        &format!(
            "UPDATE reading_unit_revisions SET source_text='mutated' WHERE {}",
            key("unit_id", &a)
        ),
        "immutable_reading",
    );
    reject(
        &rw,
        &format!("DELETE FROM models WHERE {}", key("revision_id", &rev)),
        "sealed_config_snapshot",
    );
    // A live, produced token snapshot and a produced test record share one manual
    // reading object. Test-only rollback verifies actual CASCADE and SET NULL.
    let token = LexicalToken {
        surface: "駅".into(),
        lemma: "駅".into(),
        reading: "えき".into(),
        part_of_speech: "名詞".into(),
        byte_start: 0,
        byte_end: 3,
        utf16_start: 0,
        utf16_end: 1,
        known: true,
    };
    s.set_tokens(
        TokenSnapshotInput {
            unit_id: b.clone(),
            text_revision: 1,
            analysis_revision: "relation-v1".into(),
            tokens: vec![token],
            resource: resource("language"),
        },
        1014,
    )
    .unwrap();
    s.record_model_test(ModelTestRecordInput {
        test_id: "relation-test".into(),
        config_revision_id: rev,
        connection_id: Some("con-typed".into()),
        model_id: None,
        binding_id: None,
        purpose: None,
        stage: "connection".into(),
        request_semantics_hash: content_hash("relation-test"),
        input_scope: "selected_current".into(),
        unit_id: Some(b.clone()),
        text_revision: Some(1),
        input_summary: json!({"original":"fixed"}),
        response_summary: json!({}),
        output_validation: json!({}),
        state: "passed".into(),
        started_at_ms: 1015,
        completed_at_ms: 1016,
        error_code: None,
        error_stage: None,
        credentials: vec![],
    })
    .unwrap();
    rw.execute_batch("SAVEPOINT cascade_relationship").unwrap();
    assert_eq!(
        rw.execute("DELETE FROM reading_units WHERE unit_id=?1", [&b])
            .unwrap(),
        1
    );
    assert_eq!(
        scalar::<u32>(
            &rw,
            &format!(
                "SELECT count(*) FROM reading_unit_revisions WHERE {}",
                key("unit_id", &b)
            )
        ),
        0
    );
    assert_eq!(
        scalar::<u32>(
            &rw,
            &format!(
                "SELECT count(*) FROM lexical_snapshots WHERE {}",
                key("unit_id", &b)
            )
        ),
        0
    );
    assert_eq!(scalar::<u32>(&rw, "SELECT count(*) FROM lexical_tokens"), 0);
    let test:(Option<String>,Option<u32>,String,String)=rw.query_row("SELECT unit_id,text_revision,state,input_summary_json FROM test_records WHERE test_id='relation-test'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(
        test,
        (
            None,
            None,
            "passed".into(),
            "{\"original\":\"fixed\"}".into()
        )
    );
    rw.execute_batch("ROLLBACK TO cascade_relationship;RELEASE cascade_relationship")
        .unwrap();
    assert_eq!(s.unit(b, 1).unwrap().tokens.len(), 1);
    assert_eq!(s.unit(a, 1).unwrap().session_id, au.session_id);
    assert_eq!(
        scalar::<u32>(&rw, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    super::evidence::record(
        "typed-production-relationships",
        &[("STORAGE-18", 15)],
        json!({"producer":"actual SaveGame/SaveConfiguration/ManualInput/prepare_tasks/open_hook_session/set_tokens/record_model_test","specific_negatives":["cross-session reading task composite FK","cross-config attempt composite FK","game referenced by Hook session DELETE RESTRICT","immutable original UPDATE","sealed config child DELETE","duplicate non-null target through SaveGame"],"specific_positive_boundaries":["two real unresolved NULL-target games admitted","reading revision and lexical snapshot/token CASCADE","test record fixed reading composite reference SET NULL retains conclusion"],"foreign_keys_enabled":true,"scope":"these concrete negative/data cases; full schema definition is independently compared by schema contract"}),
    );
}
