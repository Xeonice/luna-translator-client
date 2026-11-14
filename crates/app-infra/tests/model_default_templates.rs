//! Production configuration Repository + actual RequestPlan compiler; no live network request.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{
    application::request_plan::compose,
    contracts::*,
    domain::{TaskCompletion, TaskDispatch},
    ports::ProductRepository,
};
use app_infra::storage::SqliteStore;
use serde_json::json;
use std::collections::BTreeMap;
fn store() -> (SqliteStore, std::path::PathBuf) {
    let p = std::env::temp_dir().join(format!(
        "luna-model-templates-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    (
        SqliteStore::open(&p, "model-templates", "template-server", 1).unwrap(),
        p,
    )
}
fn config() -> Configuration {
    serde_json::from_value(json!({"revisionId":"","connections":[{"connectionId":"c","displayName":"controlled local","protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:1/v1","proxyMode":"disabled","networkMetadata":{},"availabilityIntent":true}],"models":[{"modelId":"m","connectionId":"c","actualModelId":"controlled","displayName":"Source model","selectionSource":"manual","capabilities":{},"parameters":{}}],"bindings":[{"bindingId":"b","scopeKey":"global","purpose":"translation","bindingMode":"override","enabled":true,"outputMode":"text","sourceLanguage":"ja","targetLanguage":"zh-CN","context":{},"schedule":{},"parameters":{},"engines":[{"engineId":"e","modelId":"m","enabled":true,"parameters":{}}]}],"templates":[],"termSets":[]})).unwrap()
}
fn template(id: &str, purpose: &str, owner: Option<&str>) -> Template {
    Template {
        template_id: id.into(),
        purpose: purpose.into(),
        display_name: id.into(),
        source_kind: "custom".into(),
        source_version: None,
        body: BTreeMap::from([
            ("system".into(), json!(format!("MARKER:{id}"))),
            ("user".into(), json!("{{source}}")),
        ]),
        variables: vec!["source".into()],
        compatibility: owner
            .map(|o| BTreeMap::from([("modelConfigurationId".into(), json!(o))]))
            .unwrap_or_default(),
    }
}
fn envelope(command: Command) -> CommandEnvelope {
    CommandEnvelope {
        operation_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: None,
        command,
    }
}
fn command(s: &SqliteStore, c: Command) -> CommandReceipt {
    s.dispatch(envelope(c), 10).unwrap()
}
fn save(s: &SqliteStore, c: Configuration) -> String {
    command(
        s,
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: s.snapshot().unwrap().configuration.draft_head_revision,
        },
    )
    .result_id
    .unwrap()
}
fn apply(s: &SqliteStore, id: String) {
    command(
        s,
        Command::ApplyConfiguration {
            revision_id: id,
            expected_head_revision: s.snapshot().unwrap().configuration.applied_head_revision,
        },
    );
}
fn manual(s: &SqliteStore) -> String {
    command(
        s,
        Command::ManualInput {
            text: "雨がやんだ。".into(),
            source_language: "ja".into(),
        },
    )
    .result_id
    .unwrap()
}
fn task(s: &SqliteStore) -> TaskDispatch {
    s.prepare_tasks(manual(s), 1, "translation".into(), 12)
        .unwrap()
        .remove(0)
}
fn system(task: &TaskDispatch) -> String {
    compose(task, &[]).unwrap().messages[0]["content"]
        .as_str()
        .unwrap()
        .into()
}
#[test]
fn model_defaults_round_trip_real_four_column_relation_and_drive_actual_requests_after_restart() {
    let (s, path) = store();
    let mut c = config();
    for p in ["translation", "grammar", "context_word"] {
        let id = format!("model-{p}");
        c.templates.push(template(&id, p, Some("m")));
        c.models[0].default_templates.insert(p.into(), id);
    }
    let revision = save(&s, c.clone());
    assert!(s.snapshot().unwrap().configuration.applied.is_none());
    apply(&s, revision);
    let read =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let rows:Vec<(String,String,String,String)>=read.prepare("SELECT revision_id,model_id,purpose,template_id FROM model_default_templates ORDER BY purpose").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(rows.len(), 3);
    let applied = s
        .snapshot()
        .unwrap()
        .configuration
        .applied
        .unwrap()
        .as_legacy()
        .expect("legacy fixture")
        .clone();
    for (rev, model, purpose, id) in rows {
        assert_eq!(rev, applied.revision_id);
        assert_eq!(model, "m");
        assert_eq!(Some(&id), c.models[0].default_templates.get(&purpose));
    }
    assert!(system(&task(&s)).starts_with("MARKER:model-translation"));
    drop(read);
    drop(s);
    let s = SqliteStore::open(path, "model-templates", "restarted", 30).unwrap();
    assert_eq!(
        s.snapshot()
            .unwrap()
            .configuration
            .applied
            .unwrap()
            .as_legacy()
            .expect("legacy fixture")
            .clone()
            .models[0]
            .default_templates,
        c.models[0].default_templates
    );
    assert!(system(&task(&s)).starts_with("MARKER:model-translation"));
    acceptance_evidence::record(
        "model-default-templates-reopen",
        &[("STORAGE-19", 1)],
        serde_json::json!({"actual_test":"model_defaults_round_trip_real_four_column_relation_and_drive_actual_requests_after_restart"}),
    );
}
#[test]
fn real_request_compiler_obeys_engine_purpose_game_global_model_then_builtin() {
    let (s, _) = store();
    let revision = save(&s, config());
    apply(&s, revision);
    let mut t = task(&s);
    assert!(system(&t).starts_with("Translate the current source"));
    for id in [
        "model-default",
        "global-purpose",
        "global-engine",
        "game-purpose",
        "game-engine",
    ] {
        t.configuration
            .templates
            .push(template(id, "translation", None));
    }
    t.model
        .default_templates
        .insert("translation".into(), "model-default".into());
    t.configuration.models[0] = t.model.clone();
    assert!(system(&t).starts_with("MARKER:model-default"));
    t.binding.template_id = Some("global-purpose".into());
    t.configuration.bindings[0] = t.binding.clone();
    assert!(system(&t).starts_with("MARKER:global-purpose"));
    t.engine.template_id = Some("global-engine".into());
    t.binding.engines[0] = t.engine.clone();
    t.configuration.bindings[0] = t.binding.clone();
    assert!(system(&t).starts_with("MARKER:global-engine"));
    t.binding.binding_id = "game-binding".into();
    t.binding.scope_key = "game:compiler-input".into();
    t.binding.game_id = Some("compiler-input".into());
    t.binding.template_id = None;
    t.engine.template_id = None;
    t.binding.engines[0] = t.engine.clone();
    t.configuration.bindings.push(t.binding.clone());
    assert!(system(&t).starts_with("MARKER:global-engine"));
    t.configuration.bindings[0].engines[0].template_id = None;
    assert!(system(&t).starts_with("MARKER:global-purpose"));
    t.binding.template_id = Some("game-purpose".into());
    assert!(system(&t).starts_with("MARKER:game-purpose"));
    t.engine.template_id = Some("game-engine".into());
    assert!(system(&t).starts_with("MARKER:game-engine"));
    t.engine.template_id = None;
    t.binding.template_id = None;
    t.configuration.bindings[0].template_id = None;
    assert!(system(&t).starts_with("MARKER:model-default"));
}
#[test]
fn repository_rejects_cross_model_purpose_duplicate_name_and_explicit_compatibility_mismatch() {
    for fault in ["owner", "purpose", "name", "protocol", "actual_model"] {
        let (s, _) = store();
        let mut c = config();
        c.templates.push(template("one", "translation", Some("m")));
        c.models[0]
            .default_templates
            .insert("translation".into(), "one".into());
        let expected = match fault {
            "owner" => {
                let mut other = c.models[0].clone();
                other.model_id = "other".into();
                other.display_name = "Other".into();
                other.default_templates.clear();
                c.models.push(other);
                c.templates[0]
                    .compatibility
                    .insert("modelConfigurationId".into(), json!("other"));
                "template_model_scope"
            }
            "purpose" => {
                c.templates[0].purpose = "grammar".into();
                "template_purpose"
            }
            "name" => {
                let mut duplicate = c.templates[0].clone();
                duplicate.template_id = "two".into();
                c.templates.push(duplicate);
                "duplicate_template_name"
            }
            "protocol" => {
                c.templates[0]
                    .compatibility
                    .insert("protocols".into(), json!(["anthropic"]));
                "template_incompatible"
            }
            _ => {
                c.templates[0]
                    .compatibility
                    .insert("actualModelIds".into(), json!(["different-exact-id"]));
                "template_incompatible"
            }
        };
        assert_eq!(
            s.dispatch(
                envelope(Command::SaveConfiguration {
                    configuration: c,
                    expected_head_revision: 0
                }),
                2
            )
            .unwrap_err()
            .code,
            expected
        );
        assert_eq!(s.snapshot().unwrap().configuration.draft_head_revision, 0);
    }
}
#[test]
fn actual_resolved_language_not_model_name_controls_template_compatibility() {
    let (s, _) = store();
    let mut c = config();
    c.bindings[0].source_language = None;
    c.templates
        .push(template("restricted", "translation", None));
    c.templates[0]
        .compatibility
        .insert("sourceLanguages".into(), json!(["ja"]));
    c.models[0]
        .default_templates
        .insert("translation".into(), "restricted".into());
    let revision = save(&s, c);
    apply(&s, revision);
    let mut t = task(&s);
    assert!(system(&t).starts_with("MARKER:restricted"));
    t.unit.source_language = "en".into();
    assert_eq!(compose(&t, &[]).unwrap_err().code, "template_incompatible");
}
#[test]
fn copy_is_cas_idempotent_independent_scoped_templates_and_preserves_shared_references() {
    let (s, _) = store();
    let mut c = config();
    c.templates = vec![
        template("owned", "translation", Some("m")),
        template("shared", "grammar", None),
    ];
    c.models[0].default_templates = BTreeMap::from([
        ("translation".into(), "owned".into()),
        ("grammar".into(), "shared".into()),
    ]);
    c.models[0].parameters.insert(
        "temperature".into(),
        Parameter {
            mode: ParameterMode::Set,
            value: Some(json!(0.3)),
        },
    );
    let revision = save(&s, c);
    apply(&s, revision);
    let original_applied = s
        .snapshot()
        .unwrap()
        .configuration
        .applied
        .unwrap()
        .as_legacy()
        .expect("legacy fixture")
        .clone();
    let old_task = task(&s);
    let head = s.snapshot().unwrap().configuration.draft_head_revision;
    let request = CommandEnvelope {
        operation_id: "stable-copy-operation".into(),
        expected_revision: None,
        command: Command::CopyModelConfiguration {
            model_id: "m".into(),
            new_display_name: "独立副本".into(),
            expected_head_revision: head,
        },
    };
    assert_eq!(
        s.dispatch(
            envelope(Command::CopyModelConfiguration {
                model_id: "m".into(),
                new_display_name: "旧窗口".into(),
                expected_head_revision: head - 1
            }),
            20
        )
        .unwrap_err()
        .code,
        "revision_conflict"
    );
    let receipt = s.dispatch(request.clone(), 21).unwrap();
    assert!(s.dispatch(request, 22).unwrap().replayed);
    let copied_id = receipt.result_id.unwrap();
    let state = s.snapshot().unwrap().configuration;
    assert_eq!(
        state
            .applied
            .as_ref()
            .unwrap()
            .as_legacy()
            .expect("legacy fixture")
            .clone()
            .revision_id,
        original_applied.revision_id
    );
    let copied = state
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .models
        .iter()
        .find(|m| m.model_id == copied_id)
        .unwrap();
    let original = state
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .models
        .iter()
        .find(|m| m.model_id == "m")
        .unwrap();
    assert_eq!(copied.connection_id, original.connection_id);
    assert_eq!(copied.parameters, original.parameters);
    assert_eq!(copied.default_templates["grammar"], "shared");
    assert_ne!(copied.default_templates["translation"], "owned");
    let owned = state
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .templates
        .iter()
        .find(|t| t.template_id == copied.default_templates["translation"])
        .unwrap();
    assert_eq!(
        owned.compatibility["modelConfigurationId"],
        json!(copied_id)
    );
    assert_eq!(owned.source_kind, "copied");
    let revision = state
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .clone()
        .revision_id
        .clone();
    apply(&s, revision);
    s.validate_task_authority(old_task.task_id.clone()).unwrap();
    s.complete_task(
        TaskCompletion {
            task_id: old_task.task_id,
            attempt_id: old_task.attempt_id,
            text: "原模型任务不受未引用副本影响".into(),
            output_mode: "text".into(),
            structured: json!({}),
            target_language: "zh-CN".into(),
            provider_label: "controlled".into(),
        },
        23,
    )
    .unwrap();
    let mut changed = s
        .snapshot()
        .unwrap()
        .configuration
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .clone();
    let copied = changed
        .models
        .iter_mut()
        .find(|m| m.model_id == copied_id)
        .unwrap();
    copied.parameters.get_mut("temperature").unwrap().value = Some(json!(0.8));
    let id = copied.default_templates["translation"].clone();
    changed
        .templates
        .iter_mut()
        .find(|t| t.template_id == id)
        .unwrap()
        .body
        .insert("system".into(), json!("COPIED EDIT"));
    save(&s, changed);
    let draft = s
        .snapshot()
        .unwrap()
        .configuration
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .clone();
    assert_eq!(
        draft
            .models
            .iter()
            .find(|m| m.model_id == "m")
            .unwrap()
            .parameters["temperature"]
            .value,
        Some(json!(0.3))
    );
    assert_eq!(
        draft
            .templates
            .iter()
            .find(|t| t.template_id == "owned")
            .unwrap()
            .body["system"],
        json!("MARKER:owned")
    );
    assert_eq!(
        s.dispatch(
            envelope(Command::CopyModelConfiguration {
                model_id: "m".into(),
                new_display_name: "独立副本".into(),
                expected_head_revision: s.snapshot().unwrap().configuration.draft_head_revision
            }),
            25
        )
        .unwrap_err()
        .code,
        "duplicate_model_name"
    );
}
#[test]
fn old_model_json_defaults_empty_and_changing_model_default_invalidates_old_request() {
    let model:Model=serde_json::from_value(json!({"modelId":"m","connectionId":"c","actualModelId":"x","displayName":"legacy","selectionSource":"manual","capabilities":{},"parameters":{}})).unwrap();
    assert!(model.default_templates.is_empty());
    let (s, _) = store();
    let mut c = config();
    c.templates.push(template("old", "translation", Some("m")));
    c.models[0]
        .default_templates
        .insert("translation".into(), "old".into());
    let revision = save(&s, c);
    apply(&s, revision);
    let old = task(&s);
    let mut c = s
        .snapshot()
        .unwrap()
        .configuration
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .clone();
    c.templates.push(template("new", "translation", Some("m")));
    c.models[0]
        .default_templates
        .insert("translation".into(), "new".into());
    let revision = save(&s, c);
    apply(&s, revision);
    assert!(s.validate_task_authority(old.task_id).is_err());
    assert!(system(&task(&s)).starts_with("MARKER:new"));
}

#[test]
fn changing_another_purpose_default_preserves_current_translation_authority() {
    let (s, _) = store();
    let mut c = config();
    c.templates = vec![
        template("translation-default", "translation", Some("m")),
        template("grammar-one", "grammar", Some("m")),
        template("grammar-two", "grammar", Some("m")),
    ];
    c.models[0].default_templates = BTreeMap::from([
        ("translation".into(), "translation-default".into()),
        ("grammar".into(), "grammar-one".into()),
    ]);
    let revision = save(&s, c);
    apply(&s, revision);
    let translation = task(&s);
    let mut c = s
        .snapshot()
        .unwrap()
        .configuration
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .clone();
    c.models[0]
        .default_templates
        .insert("grammar".into(), "grammar-two".into());
    let revision = save(&s, c);
    apply(&s, revision);
    s.validate_task_authority(translation.task_id).unwrap();
    assert!(system(&task(&s)).starts_with("MARKER:translation-default"));
}

#[test]
fn purpose_test_history_becomes_stale_for_selected_default_id_or_body_but_model_probe_does_not() {
    use app_core::domain::model_config::testing::{ModelTestRecordInput, scope_semantics_hash};
    let (s, _) = store();
    let mut c = config();
    c.templates = vec![
        template("one", "translation", Some("m")),
        template("two", "translation", Some("m")),
    ];
    c.models[0]
        .default_templates
        .insert("translation".into(), "one".into());
    save(&s, c);
    // Seed controlled test metadata; this exercises production record staleness, not live-model acceptance.
    let record = |name: &str, kind: &str, c: &Configuration| {
        let hash =
            scope_semantics_hash(c, kind, if kind == "purpose" { "b" } else { "m" }).unwrap();
        s.record_model_test(ModelTestRecordInput {
            test_id: name.into(),
            config_revision_id: c.revision_id.clone(),
            connection_id: Some("c".into()),
            model_id: Some("m".into()),
            binding_id: (kind == "purpose").then(|| "b".into()),
            purpose: (kind == "purpose").then(|| "translation".into()),
            stage: kind.into(),
            request_semantics_hash: hash.clone(),
            input_scope: "sample".into(),
            unit_id: None,
            text_revision: None,
            input_summary: json!({"scopeSemanticsHash":hash}),
            response_summary: json!({}),
            output_validation: json!({}),
            state: "passed".into(),
            started_at_ms: 10,
            completed_at_ms: 11,
            error_code: None,
            error_stage: None,
            credentials: vec![],
        })
        .unwrap();
    };
    let c = s
        .snapshot()
        .unwrap()
        .configuration
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .clone();
    record("purpose-before-id", "purpose", &c);
    record("fixed-model-probe", "model", &c);
    let mut c = c.clone();
    c.models[0]
        .default_templates
        .insert("translation".into(), "two".into());
    save(&s, c);
    let state = s.snapshot().unwrap();
    assert!(
        state
            .configuration_tests
            .iter()
            .find(|v| v.test_id == "purpose-before-id")
            .unwrap()
            .stale
    );
    assert!(
        !state
            .configuration_tests
            .iter()
            .find(|v| v.test_id == "fixed-model-probe")
            .unwrap()
            .stale
    );
    record(
        "purpose-before-body",
        "purpose",
        &state
            .configuration
            .draft
            .as_legacy()
            .expect("legacy fixture")
            .clone(),
    );
    let mut c = state
        .configuration
        .draft
        .as_legacy()
        .expect("legacy fixture")
        .clone();
    c.templates
        .iter_mut()
        .find(|t| t.template_id == "two")
        .unwrap()
        .body
        .insert(
            "system".into(),
            json!("changed actual selected default body"),
        );
    save(&s, c);
    let state = s.snapshot().unwrap();
    assert!(
        state
            .configuration_tests
            .iter()
            .find(|v| v.test_id == "purpose-before-body")
            .unwrap()
            .stale
    );
    assert!(
        !state
            .configuration_tests
            .iter()
            .find(|v| v.test_id == "fixed-model-probe")
            .unwrap()
            .stale
    );
}

#[test]
fn traditional_translator_cannot_silently_accept_ignored_model_prompt_templates() {
    let (s, _) = store();
    let mut c = config();
    c.connections[0].protocol = "azure_translator".into();
    c.models[0].actual_model_id = None;
    c.templates.push(template("prompt", "translation", None));
    c.models[0]
        .default_templates
        .insert("translation".into(), "prompt".into());
    assert_eq!(
        s.dispatch(
            envelope(Command::SaveConfiguration {
                configuration: c.clone(),
                expected_head_revision: 0
            }),
            10
        )
        .unwrap_err()
        .code,
        "unsupported_template_protocol"
    );
    c.models[0].default_templates.clear();
    let revision = save(&s, c);
    apply(&s, revision);
    assert!(s.snapshot().unwrap().configuration.applied.is_some());
}
