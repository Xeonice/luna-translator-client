//! TP-16: mutate real persisted cache lookup inputs, never task invalidation.
#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{
    contracts::*,
    domain::{TaskCompletion, TaskDispatch, capture::*, content_hash},
    ports::ProductRepository,
};
use app_infra::storage::SqliteStore;
use rusqlite::{Connection, OpenFlags};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const SOURCE: &str = "図書館で待つ。";
fn config() -> Configuration {
    serde_json::from_value(json!({"revisionId":"","connections":[{"connectionId":"c","displayName":"Connection","protocol":"openai_compatible","deployment":"local","authMode":"api_key","endpoint":"http://127.0.0.1:1/v1","credentialPoolId":"pool-a","credentialPoolRevision":1,"proxyMode":"disabled","networkMetadata":{},"availabilityIntent":true}],"models":[{"modelId":"m","connectionId":"c","actualModelId":"controlled-model","displayName":"Model","selectionSource":"manual","capabilities":{},"parameters":{}}],"bindings":[{"bindingId":"b","scopeKey":"global","purpose":"translation","bindingMode":"override","enabled":true,"outputMode":"text","sourceLanguage":"ja","targetLanguage":"zh-Hans","context":{"enabled":false},"schedule":{"cacheTtlMs":100000},"parameters":{},"engines":[{"engineId":"e","modelId":"m","enabled":true,"parameters":{}}]}],"templates":[],"termSets":[]})).unwrap()
}
struct Fixture {
    store: SqliteStore,
    path: PathBuf,
    now: i64,
    sequence: u64,
    configuration: Configuration,
}
impl Fixture {
    fn tick(&mut self) -> i64 {
        self.now += 10;
        self.now
    }
    fn new(path: &Path, context: bool) -> Self {
        let profile = format!(
            "cache-profile-{}",
            path.file_stem().unwrap().to_str().unwrap()
        );
        let store = SqliteStore::open(path, &profile, "cache-server", 100).unwrap();
        for (id, pool, vault) in [("a", "pool-a", "vault-a"), ("b", "pool-b", "vault-b")] {
            store
                .begin_credential(
                    CredentialReference {
                        credential_id: id.into(),
                        credential_version_id: vault.into(),
                        pool_id: pool.into(),
                        pool_revision: 1,
                        label: "controlled reference, no service secret".into(),
                        masked_hint: "••••".into(),
                    },
                    101,
                )
                .unwrap();
            store.activate_credential(id.into(), 102).unwrap();
        }
        let mut f = Self {
            store,
            path: path.to_owned(),
            now: 110,
            sequence: 0,
            configuration: config(),
        };
        f.configuration.bindings[0].context = BTreeMap::from([
            ("enabled".into(), json!(context)),
            ("maxSentences".into(), json!(1)),
            ("historyMode".into(), json!("original")),
        ]);
        f.apply();
        let now = f.tick();
        f.store
            .open_hook_session(
                HookSessionInput {
                    session_id: "cache-hook".into(),
                    game_id: None,
                    environment_key: "crossover:/private/cache-fixture".into(),
                    target_exe_path: "C:\\cache.exe".into(),
                    windows_pid: 100,
                    process_created_u64: "100".into(),
                    architecture: "x86".into(),
                    broker_instance_id: "cache-broker".into(),
                    resource: resource("core", "v1", "core-a"),
                    target_identity_sha256: content_hash("cache-target"),
                },
                now,
            )
            .unwrap();
        f.capture("candidate");
        f.command(Command::SelectSource {
            source_id: "cache-source".into(),
        });
        f
    }
    fn command(&mut self, command: Command) -> CommandReceipt {
        let now = self.tick();
        self.store
            .dispatch(
                CommandEnvelope {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    expected_revision: None,
                    command,
                },
                now,
            )
            .unwrap()
    }
    fn apply(&mut self) {
        let state = self.store.snapshot().unwrap().configuration;
        let receipt = self.command(Command::SaveConfiguration {
            configuration: self.configuration.clone(),
            expected_head_revision: state.draft_head_revision,
        });
        self.command(Command::ApplyConfiguration {
            revision_id: receipt.result_id.unwrap(),
            expected_head_revision: state.applied_head_revision,
        });
        self.configuration = self
            .store
            .snapshot()
            .unwrap()
            .configuration
            .applied
            .unwrap()
            .as_legacy().expect("legacy fixture").clone();
    }
    fn capture(&mut self, text: &str) -> Option<CaptureVersion> {
        self.sequence += 1;
        let now = self.tick();
        self.store
            .ingest_hook_text(
                HookTextInput {
                    session_id: "cache-hook".into(),
                    event_id: format!("cache-event-{}", self.sequence),
                    sequence_u64: self.sequence.to_string(),
                    source: HookSourceInput {
                        source_id: "cache-source".into(),
                        host_instance_id: "cache-host".into(),
                        hook_address_u64: "100".into(),
                        context_u64: "0".into(),
                        subcontext_u64: "0".into(),
                        hook_code: "HW-cache".into(),
                        hook_name: "cache-input".into(),
                        encoding: "utf-16le".into(),
                        stream_identity_sha256: content_hash("cache-stream"),
                        identity_extensions_json: "{}".into(),
                    },
                    text: text.into(),
                    source_language: "ja".into(),
                },
                now,
            )
            .unwrap()
            .visible_version
    }
    fn line(&mut self) -> CaptureVersion {
        self.capture(SOURCE).unwrap()
    }
    fn manual(&mut self) -> CaptureVersion {
        let id = self
            .command(Command::ManualInput {
                text: SOURCE.into(),
                source_language: "ja".into(),
            })
            .result_id
            .unwrap();
        CaptureVersion {
            unit_id: id,
            text_revision: 1,
        }
    }
    fn tokens(&mut self, unit: &CaptureVersion, version: &str, manifest: &str, analysis: &str) {
        let now = self.tick();
        self.store
            .set_tokens(
                TokenSnapshotInput {
                    unit_id: unit.unit_id.clone(),
                    text_revision: unit.text_revision,
                    analysis_revision: analysis.into(),
                    tokens: tokens(),
                    resource: resource("language", version, manifest),
                },
                now,
            )
            .unwrap();
    }
    fn prepare(
        &mut self,
        unit: &CaptureVersion,
        purpose: &str,
        selected: Option<u32>,
    ) -> TaskDispatch {
        let now = self.tick();
        let mut tasks = if purpose == "context_word" {
            self.store
                .prepare_study_tasks(
                    unit.unit_id.clone(),
                    unit.text_revision,
                    purpose.into(),
                    selected,
                    uuid::Uuid::new_v4().to_string(),
                    now,
                )
                .unwrap()
        } else {
            self.store
                .prepare_tasks_for_operation(
                    unit.unit_id.clone(),
                    unit.text_revision,
                    purpose.into(),
                    uuid::Uuid::new_v4().to_string(),
                    now,
                )
                .unwrap()
        };
        assert_eq!(tasks.len(), 1);
        let task = tasks.remove(0);
        let connection =
            Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let kind: String = connection
            .query_row(
                "SELECT dispatch_kind FROM request_attempts WHERE task_id=?1",
                [&task.task_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            kind,
            if task.cached_result.is_some() {
                "cache"
            } else {
                "initial"
            }
        );
        task
    }
    fn complete(&mut self, task: &TaskDispatch) {
        let now = self.tick();
        self.store
            .complete_task(
                TaskCompletion {
                    task_id: task.task_id.clone(),
                    attempt_id: task.attempt_id.clone(),
                    text: task
                        .cached_result
                        .as_ref()
                        .map(|r| r.text.clone())
                        .unwrap_or_else(|| "缓存语义矩阵的服务结果".into()),
                    output_mode: if task.binding.purpose == "translation" {
                        "text"
                    } else {
                        "plain_text"
                    }
                    .into(),
                    structured: json!({}),
                    target_language: task.binding.target_language.clone().unwrap(),
                    provider_label: "controlled-cache-provider".into(),
                },
                now,
            )
            .unwrap();
    }
}
fn tokens() -> Vec<LexicalToken> {
    vec![
        LexicalToken {
            surface: "図書館".into(),
            lemma: "図書館".into(),
            reading: "としょかん".into(),
            part_of_speech: "名詞".into(),
            byte_start: 0,
            byte_end: 9,
            utf16_start: 0,
            utf16_end: 3,
            known: true,
        },
        LexicalToken {
            surface: "待つ".into(),
            lemma: "待つ".into(),
            reading: "まつ".into(),
            part_of_speech: "動詞".into(),
            byte_start: 12,
            byte_end: 18,
            utf16_start: 4,
            utf16_end: 6,
            known: true,
        },
    ]
}
fn resource(kind: &str, version: &str, manifest: &str) -> ResourceSnapshotInput {
    ResourceSnapshotInput {
        snapshot_id: format!("cache-{kind}-{version}-{manifest}"),
        kind: kind.into(),
        version: version.into(),
        manifest_sha256: content_hash(manifest),
        contract_version: "1".into(),
        metadata_json: "{}".into(),
    }
}

#[test]
fn real_cache_lookup_metamorphic_matrix_preserves_semantics_and_corrections() {
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.build/storage-cache-matrix")
        .join(uuid::Uuid::new_v4().to_string());
    let output = acceptance_evidence::directory("cache-matrix", &fallback);
    let mut rows = vec![];
    for dimension in [
        "equivalent",
        "display_only",
        "source_language",
        "target_language",
        "text_revision",
        "processing_identity",
        "context",
        "glossary",
        "template",
        "model",
        "credential_version",
        "credential_identity",
        "resource_version",
        "resource_manifest",
        "analysis_version",
        "purpose",
        "selected_word",
        "profile",
    ] {
        let path = output.join(format!("{dimension}.sqlite3"));
        let mut f = Fixture::new(&path, dimension == "context");
        if dimension == "credential_identity" {
            f.command(Command::SaveCredentialPool {
                pool: CredentialPoolInput {
                    pool_id: Some("pool-a".into()),
                    expected_revision: 1,
                    policy: "round_robin".into(),
                    metadata: BTreeMap::new(),
                    members: vec![
                        CredentialPoolMemberInput {
                            member_id: None,
                            credential_id: "a".into(),
                            enabled: true,
                            weight: 1,
                        },
                        CredentialPoolMemberInput {
                            member_id: None,
                            credential_id: "b".into(),
                            enabled: true,
                            weight: 1,
                        },
                    ],
                },
            });
            f.configuration.connections[0].credential_pool_revision = Some(2);
            f.apply();
        }
        let purpose = if dimension == "selected_word" {
            "context_word"
        } else {
            "translation"
        };
        if matches!(dimension, "selected_word" | "purpose") {
            let mut study = f.configuration.bindings[0].clone();
            study.binding_id = "study".into();
            study.purpose = "context_word".into();
            study.output_mode = Some("plain_text".into());
            f.configuration.bindings.push(study);
            f.apply();
        }
        if dimension == "context" {
            f.capture("朝の話。");
        }
        let baseline = if dimension == "processing_identity" {
            f.manual()
        } else {
            f.line()
        };
        f.tokens(&baseline, "v1", "language-a", "analysis-a");
        let first = f.prepare(&baseline, purpose, Some(0));
        assert!(first.cached_result.is_none());
        f.complete(&first);
        let before = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(
            before
                .query_row("SELECT count(*) FROM cache_entries", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            1
        );
        drop(before);
        match dimension {
            "display_only" => {
                f.configuration.connections[0].display_name = "Connection renamed".into();
                f.configuration.models[0].display_name = "Model renamed".into();
                f.apply();
                let preferences = Preferences {
                    theme: "light".into(),
                    ..Preferences::default()
                };
                f.command(Command::SavePreferences { preferences });
            }
            "source_language" => {
                f.configuration.bindings[0].source_language = Some("ko".into());
                f.apply();
            }
            "target_language" => {
                f.configuration.bindings[0].target_language = Some("zh-TW".into());
                f.apply();
            }
            "glossary" => {
                f.configuration.term_sets.push(TermSet {
                    term_set_id: "terms".into(),
                    scope_key: "global".into(),
                    game_id: None,
                    display_name: "Terms".into(),
                    enabled: true,
                    terms: vec![Term {
                        term_id: "term".into(),
                        source_text: Some("図書館".into()),
                        target_text: Some("書庫".into()),
                        source_language: Some("ja".into()),
                        target_language: Some("zh-Hans".into()),
                        enabled: true,
                    }],
                });
                f.apply();
            }
            "template" => {
                f.configuration.templates.push(Template {
                    template_id: "specific".into(),
                    purpose: "translation".into(),
                    display_name: "Request template".into(),
                    source_kind: "custom".into(),
                    source_version: None,
                    body: BTreeMap::from([
                        (
                            "system".into(),
                            json!("Translate exactly with a conservative style."),
                        ),
                        ("user".into(), json!("{{source}}")),
                    ]),
                    variables: vec!["source".into()],
                    compatibility: BTreeMap::new(),
                });
                f.configuration.bindings[0].template_id = Some("specific".into());
                f.apply();
            }
            "model" => {
                f.configuration.models[0].actual_model_id = Some("controlled-next-model".into());
                f.apply();
            }
            "credential_version" => {
                f.command(Command::SaveCredentialPool {
                    pool: CredentialPoolInput {
                        pool_id: Some("pool-a".into()),
                        expected_revision: 1,
                        policy: "round_robin".into(),
                        metadata: BTreeMap::new(),
                        members: vec![CredentialPoolMemberInput {
                            member_id: None,
                            credential_id: "a".into(),
                            enabled: true,
                            weight: 1,
                        }],
                    },
                });
                f.configuration.connections[0].credential_pool_revision = Some(2);
                f.apply();
            }
            "context" => {
                f.capture("夜の話。");
            }
            _ => {}
        }
        let revision = if dimension == "text_revision" {
            f.capture("図書館で");
            f.capture("待つ。").unwrap()
        } else {
            f.line()
        };
        assert_eq!(
            f.store
                .unit(revision.unit_id.clone(), revision.text_revision)
                .unwrap()
                .source_text,
            SOURCE
        );
        if dimension == "text_revision" {
            assert_eq!(revision.text_revision, 2);
        }
        f.tokens(
            &revision,
            if dimension == "resource_version" {
                "v2"
            } else {
                "v1"
            },
            if dimension == "resource_manifest" || dimension == "resource_version" {
                "language-b"
            } else {
                "language-a"
            },
            if dimension == "analysis_version" {
                "analysis-b"
            } else {
                "analysis-a"
            },
        );
        let next_purpose = if dimension == "purpose" {
            "context_word"
        } else {
            purpose
        };
        let candidate = if dimension == "profile" {
            let mut other = Fixture::new(&output.join("other-profile.sqlite3"), false);
            let unit = other.line();
            other.tokens(&unit, "v1", "language-a", "analysis-a");
            let task = other.prepare(&unit, "translation", None);
            assert!(task.cached_result.is_none());
            task
        } else {
            f.prepare(
                &revision,
                next_purpose,
                Some(if dimension == "selected_word" { 1 } else { 0 }),
            )
        };
        let expected_hit = matches!(dimension, "equivalent" | "display_only");
        rows.push(json!({"dimension":dimension,"expected_hit":expected_hit,"actual_hit":candidate.cached_result.is_some(),"baseline_request_sha256":first.request_semantics_hash,"candidate_request_sha256":candidate.request_semantics_hash,"baseline_task":first.task_id,"candidate_task":candidate.task_id,"baseline_revision":baseline.text_revision,"candidate_revision":revision.text_revision}));
        fs::write(
            output.join("matrix.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .unwrap();
        assert_eq!(
            candidate.cached_result.is_some(),
            expected_hit,
            "real cache lookup dimension {dimension}"
        );
        if expected_hit {
            assert_eq!(
                first.request_semantics_hash, candidate.request_semantics_hash,
                "equivalent semantics must retain their actual lookup key: {dimension}"
            );
        } else {
            assert_ne!(
                first.request_semantics_hash, candidate.request_semantics_hash,
                "changed fixed input must have a separate actual lookup key: {dimension}"
            );
        }
        if expected_hit {
            assert_eq!(
                candidate.cached_result.as_ref().unwrap().text,
                "缓存语义矩阵的服务结果"
            );
        }
    }
    acceptance_evidence::record(
        "cache-semantic-metamorphic-matrix",
        &[("STORAGE-13", rows.len() as u64)],
        json!({"matrix":output.join("matrix.json"),"dimensions":rows.len(),"source":"actual Repository cache lookup cached_result plus persisted request_attempt dispatch_kind"}),
    );
}
