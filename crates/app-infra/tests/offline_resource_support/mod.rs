//! Controlled material fixtures through real O2 I/O and real production Store4.
//! The bytes are not vendor binaries and never establish a usable loader/runtime.
#![allow(dead_code)]
use app_core::{
    domain::{content_hash, model_config::service_graph::*, offline_runtime::*},
    ports::operation_budget::OperationBudget,
};
use app_infra::{
    offline_translation::{
        OfflineEngine,
        closure::{StoredRuntimeFileRow, StoredRuntimeMaterialSkeleton},
        resolver::{OfflineRuntimeMaterialResolver, ResolvedOfflineMaterial},
        resources::ResourceInspector,
        runtime_policy::registered_policy,
        windows,
    },
    storage::*,
};
use rusqlite::Connection;
use std::{num::NonZeroU32, path::PathBuf, sync::Arc, time::Duration};
pub fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}
pub fn budget() -> OperationBudget {
    OperationBudget::new(Duration::from_secs(20)).unwrap()
}
pub struct Fixture {
    pub directory: PathBuf,
    pub root: PathBuf,
    pub database: PathBuf,
    pub store: SqliteStore,
    pub owner: Arc<LiveOfflinePreparations>,
    pub repo: SqliteOfflineResources,
    pub skeleton: StoredRuntimeMaterialSkeleton,
}
impl Fixture {
    pub fn new() -> Self {
        let base = std::env::var_os("FP_OFFLINE_SQL_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let directory = base.join(format!("offline-store-o3a-{}", uuid::Uuid::new_v4()));
        let root = directory.join("resources");
        std::fs::create_dir_all(root.join(".luna")).unwrap();
        let root = root.canonicalize().unwrap();
        let policy = registered_policy(
            OfflineEngine::Jbeijing,
            RuntimePlatform::Crossover,
            RuntimeArchitecture::X86,
        )
        .unwrap();
        let processing = policy
            .processing_for_declared_runtime(windows::JBEIJING_ABI)
            .unwrap();
        let expected = ExpectedRuntimeObservation {
            engine: policy.profile().engine.clone(),
            runtime_version: windows::JBEIJING_ABI.into(),
            platform: RuntimePlatform::Windows,
            architecture: RuntimeArchitecture::X86,
            resource_version: None,
            physical_cancel_supported: false,
            processing_sha256: processing.canonical_sha256().unwrap(),
        };
        let files = [
            ("worker.exe", RuntimeFileRole::Worker),
            ("engine.dll", RuntimeFileRole::Engine),
            ("dictionary.bin", RuntimeFileRole::Dictionary),
        ]
        .into_iter()
        .map(|(name, role)| {
            let bytes = format!("isolated, non-executable fixture bytes: {name}");
            std::fs::write(root.join(name), &bytes).unwrap();
            RuntimeFileClaim {
                relative_key: name.into(),
                byte_length: bytes.len() as u64,
                sha256: content_hash(&bytes),
                role,
            }
        })
        .collect::<Vec<_>>();
        let content =
            runtime_content_sha256(policy.profile(), &processing, &expected, &files).unwrap();
        let bundle = RuntimeBundleManifest {
            schema: BundleSchema::V1,
            profile: policy.profile().clone(),
            processing,
            expected,
            payload_files: files,
            content_sha256: content,
        };
        let bytes = encode_bundle_metadata(&bundle).unwrap();
        std::fs::write(root.join(CONTROL_RELATIVE_KEY), &bytes).unwrap();
        let control = ControlArtifactRef {
            relative_key: CONTROL_RELATIVE_KEY.into(),
            byte_length: bytes.len() as u64,
            sha256: content_hash(&bytes),
        };
        let manifest = snapshot_manifest_sha256(&bundle, &control).unwrap();
        let pair = LanguagePair {
            source: "ja".into(),
            target: "zh-Hans".into(),
        };
        let pairs = vec![OfflineResourcePair {
            pair: pair.clone(),
            pipeline_profile_id: "controlled-vendor-pipeline".into(),
        }];
        let resource_hash = offline_snapshot_content_hash(
            policy.profile(),
            &OfflineSnapshotIdentity::FileManifest {
                manifest_sha256: manifest.clone(),
            },
            &pairs,
        )
        .unwrap();
        let mut rows = bundle
            .payload_files
            .iter()
            .map(|f| StoredRuntimeFileRow {
                relative_key: f.relative_key.clone(),
                byte_length: f.byte_length,
                sha256: f.sha256.clone(),
                role: f.role.stored_role(),
            })
            .collect::<Vec<_>>();
        rows.push(StoredRuntimeFileRow {
            relative_key: control.relative_key,
            byte_length: control.byte_length,
            sha256: control.sha256,
            role: StoredFileRole::Runtime,
        });
        let skeleton = StoredRuntimeMaterialSkeleton {
            snapshot_id: "snapshot".into(),
            route_id: "route".into(),
            observation: ObservationRef {
                observation_id: "observation-1".into(),
                epoch: nz(1),
                subject_key: "installation:one".into(),
                head_revision: nz(1),
            },
            profile: policy.profile().clone(),
            installation: LocalInstallationRef {
                installation_id: "installation".into(),
                revision: nz(1),
            },
            environment: RuntimeEnvironmentRef {
                environment_id: "environment".into(),
                revision: nz(1),
                machine_fingerprint: content_hash("controlled-machine"),
            },
            root: GrantedRootRef {
                grant_id: "root-grant".into(),
                authorization_epoch: nz(1),
                machine_fingerprint: content_hash("controlled-machine"),
            },
            resource_content_sha256: resource_hash,
            pair: pair.clone(),
            route: vec![OfflineRouteHop {
                snapshot_id: "snapshot".into(),
                pair,
                pipeline_profile_id: "controlled-vendor-pipeline".into(),
                package_relative_key: None,
            }],
            route_policy: OfflineRoutePolicy::DirectOnly {},
            files: rows,
            manifest_sha256: manifest,
        };
        let database = directory.join("profile.sqlite3");
        let store = SqliteStore::open(&database, "profile", "server", 10).unwrap();
        store
            .configure_service_directory(Arc::new(
                app_infra::providers::builtin_registry().unwrap(),
            ))
            .unwrap();
        let owner = LiveOfflinePreparations::new();
        let repo = store.offline_resources_repository(owner.clone()).unwrap();
        Self {
            directory,
            root,
            database,
            store,
            owner,
            repo,
            skeleton,
        }
    }
    pub fn sql(&self) -> Connection {
        let c = Connection::open(&self.database).unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        c
    }
    pub fn metadata(&self, op: &str, epoch: u32) -> OfflineUnknownMetadata {
        OfflineUnknownMetadata {
            operation_id: op.into(),
            frame: self.repo.frame().clone(),
            abi_profile_id: Some(windows::JBEIJING_ABI.into()),
            local: Some(OfflineLocalLocation {
                opaque_locator: "controlled-owned-root".into(),
                canonical_root: self.root.clone(),
                environment: OfflineEnvironmentMetadata {
                    os_build: "controlled-host-build".into(),
                    platform: RuntimePlatform::Crossover,
                    architecture: RuntimeArchitecture::X86,
                    bottle_identity: Some("ControlledBottle".into()),
                    local_prefix_path: Some(
                        self.directory
                            .join("controlled-prefix")
                            .to_str()
                            .unwrap()
                            .into(),
                    ),
                    wine_build: Some("controlled-wine-build".into()),
                },
            }),
            pairs: vec![OfflineResourcePair {
                pair: self.skeleton.pair.clone(),
                pipeline_profile_id: "controlled-vendor-pipeline".into(),
            }],
            route: self.skeleton.route.clone(),
            route_policy: self.skeleton.route_policy.clone(),
            expected_installation_head: (epoch > 1).then(|| nz(1)),
            expected_observation_head: (epoch > 1).then(|| nz(epoch - 1)),
        }
    }
    pub async fn resolved(&self, epoch: u32) -> Arc<ResolvedOfflineMaterial> {
        let mut input = self.skeleton.clone();
        input.observation.observation_id = format!("observation-{epoch}");
        input.observation.epoch = nz(epoch);
        input.observation.head_revision = nz(epoch);
        Arc::new(
            OfflineRuntimeMaterialResolver::new(ResourceInspector::new())
                .inspect_stored(input, self.root.clone(), budget())
                .await
                .unwrap(),
        )
    }
    pub async fn prepare(&self, op: &str, epoch: u32) -> Arc<OfflineUnknownPreparation> {
        OfflineUnknownPreparation::from_resolved(
            self.metadata(op, epoch),
            self.resolved(epoch).await,
        )
        .unwrap()
    }
    pub async fn commit(&self, op: &str, epoch: u32) -> OfflineStoredPreparationReceipt {
        let p = self.prepare(op, epoch).await;
        let b = budget();
        let _guard = self.owner.begin(p.clone(), b.clone()).unwrap();
        self.repo
            .commit_unknown(p, b, 20 + i64::from(epoch))
            .unwrap()
    }
    pub fn counts(&self) -> Vec<(String, i64)> {
        let c = self.sql();
        [
            "offline_runtime_profiles",
            "offline_execution_environments",
            "offline_local_grants",
            "offline_installation_versions",
            "offline_installation_heads",
            "offline_resource_snapshots",
            "offline_resource_snapshot_files",
            "offline_resource_pairs",
            "offline_resource_seals",
            "offline_translation_routes",
            "offline_translation_route_hops",
            "offline_route_seals",
            "offline_resource_observations",
            "offline_observation_heads",
            "operation_receipts",
        ]
        .into_iter()
        .map(|t| {
            (
                t.into(),
                c.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))
                    .unwrap(),
            )
        })
        .collect()
    }
}
impl Fixture {
    pub fn apple_preparation(&self, op: &str, epoch: u32) -> Arc<OfflineUnknownPreparation> {
        let policy = registered_policy(
            OfflineEngine::AppleTranslation,
            RuntimePlatform::Macos,
            RuntimeArchitecture::Arm64,
        )
        .unwrap();
        let pair = LanguagePair {
            source: "ja".into(),
            target: "zh-Hans".into(),
        };
        let pairs = vec![OfflineResourcePair {
            pair: pair.clone(),
            pipeline_profile_id: "controlled-apple-pair".into(),
        }];
        let identity = OfflineSnapshotIdentity::AppleObservation {
            os_build: "controlled-apple-build".into(),
            bridge_build_sha256: content_hash("controlled bridge bytes"),
            api_profile: app_infra::offline_translation::runtime_policy::APPLE_API_PROFILE.into(),
            strategy: AppleStrategy::TraditionalLowLatency,
            pair: pair.clone(),
            observation_epoch: nz(epoch),
        };
        let expected = policy
            .expected_apple_system_declaration(
                "controlled-apple-build",
                app_infra::offline_translation::runtime_policy::APPLE_API_PROFILE,
                &AppleStrategy::TraditionalLowLatency,
            )
            .unwrap();
        let snapshot_id = format!("apple-snapshot-{epoch}");
        let route_id = format!("apple-route-{epoch}");
        let reference = OfflineResourceReference {
            snapshot_id: snapshot_id.clone(),
            route_id,
            observation: ObservationRef {
                observation_id: format!("apple-observation-{epoch}"),
                epoch: nz(epoch),
                subject_key: "apple:ja:zh-Hans".into(),
                head_revision: nz(epoch),
            },
            profile: policy.profile().clone(),
            backend: ResourceBackendIdentity::AppleSystem {
                os_build: "controlled-apple-build".into(),
                bridge_build_sha256: content_hash("controlled bridge bytes"),
                api_profile: app_infra::offline_translation::runtime_policy::APPLE_API_PROFILE
                    .into(),
                strategy: AppleStrategy::TraditionalLowLatency,
            },
            resource_content_sha256: offline_snapshot_content_hash(
                policy.profile(),
                &identity,
                &pairs,
            )
            .unwrap(),
            processing_sha256: expected.processing_sha256.clone(),
            expected_observation_sha256: expected.canonical_sha256().unwrap(),
            control: None,
            pair: pair.clone(),
        };
        let material = Arc::new(
            OfflineRuntimeMaterialResolver::new(ResourceInspector::new())
                .reconstruct_apple(reference, &budget())
                .unwrap(),
        );
        OfflineUnknownPreparation::from_resolved(
            OfflineUnknownMetadata {
                operation_id: op.into(),
                frame: self.repo.frame().clone(),
                abi_profile_id: None,
                local: None,
                pairs,
                route: vec![OfflineRouteHop {
                    snapshot_id,
                    pair,
                    pipeline_profile_id: "controlled-apple-pair".into(),
                    package_relative_key: None,
                }],
                route_policy: OfflineRoutePolicy::DirectOnly {},
                expected_installation_head: None,
                expected_observation_head: (epoch > 1).then(|| nz(epoch - 1)),
            },
            material,
        )
        .unwrap()
    }
    pub fn save_offline_config(
        &self,
        receipt: &OfflineStoredPreparationReceipt,
    ) -> OfflineConfigRef {
        let r = &receipt.reference;
        let config = OfflineEngineConfigV1 {
            id: "local-configuration".into(),
            engine: r.profile.engine.clone(),
            backend: r.profile.backend.clone(),
            display_name: "Unknown controlled resource".into(),
            pair: r.pair.clone(),
            route_policy: OfflineRoutePolicy::DirectOnly {},
            installation: match &r.backend {
                ResourceBackendIdentity::LocalInstallation { installation, .. } => {
                    Some(installation.clone())
                }
                _ => None,
            },
            resolution: OfflineConfigResolution::Resolved {
                resource_snapshot_id: r.snapshot_id.clone(),
                route_id: r.route_id.clone(),
            },
            request_defaults: ServiceRequestDefaults {
                purpose: app_core::contracts::CatalogPurpose::Translation,
                languages: ServiceLanguageDefaults::OfflineUseConfigPair {},
                context: ServiceContextConsent {
                    enabled: false,
                    max_sentences: 0,
                    mode: HistoryInputMode::Original,
                },
                execution: ServiceExecutionDefaults {
                    timeout_ms: 1000,
                    concurrency: 1,
                    retries: 0,
                },
                cache: ServiceCachePolicy { enabled: false },
                fallback: vec![],
            },
        };
        let graph = ConfigurationV3 {
            revision_id: "offline-config".into(),
            connections: vec![],
            models: vec![],
            bindings: vec![],
            templates: vec![],
            term_sets: vec![],
            offline_engine_configs: vec![config],
            codec_version: 3,
        };
        let request_hash = content_hash(serde_json::to_vec(&graph).unwrap());
        let command = app_core::ports::service_configuration::CompiledServiceDraft {
            operation_id: "controlled-compiled-offline-save".into(),
            request_hash,
            expected_profile_generation: self.repo.frame().generation,
            expected_draft_head_revision: 0,
            graph,
            resource: EngineTarget::Offline {
                offline_config_id: "local-configuration".into(),
            },
        };
        let receipt = app_core::ports::ServiceConfigurationRepository::save_service_configuration(
            &self.store,
            &command,
            100,
        )
        .unwrap();
        OfflineConfigRef {
            revision_id: receipt.revision_id,
            config_id: "local-configuration".into(),
        }
    }
}
