//! Actual business DTO -> production Repository -> production DTO round trips.
//! Only asserted fields receive mapping evidence; the full frozen inventory is
//! reported separately as pending, never satisfied by generic column reads.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{contracts::*, ports::ProductRepository};
use app_infra::storage::SqliteStore;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
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

#[test]
fn real_product_dtos_preserve_owned_fields_nulls_tristates_and_unlimited_integer_identity() {
    let root = root();
    let fallback = root.join(".build/storage-typed-mappings").join(format!(
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
    let registry: Value = serde_json::from_slice(
        &fs::read(root.join("docs/architecture/database/field-boundaries.json")).unwrap(),
    )
    .unwrap();
    let required = registry["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    let tested = proofs.rows.keys().cloned().collect::<BTreeSet<_>>();
    assert!(tested.is_subset(&required));
    // These samples prove real bidirectional business mappings. Completeness,
    // field-specific null/range coverage and ownership classifications remain
    // explicit; no single sample falsely satisfies all 431 RUST-MAP checks.
    let pending = required.difference(&tested).cloned().collect::<Vec<_>>();
    let report = json!({"schema_version":1,"kind":"actual_production_dto_mapping_components","status":"partial","full_storage_acceptance":"not_issued","actual_business_fields_tested":tested.len(),"required_field_count":required.len(),"field_proofs":proofs.rows.values().collect::<Vec<_>>(),"uncovered_field_ids":pending,"all_431_production_mapping_completed":false});
    fs::write(
        out.join("receipt.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!(
        "PRODUCTION_TYPED_MAPPING_RECEIPT {}",
        out.join("receipt.json").display()
    );
}
