use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantedRootRef {
    pub grant_id: String,
    pub authorization_epoch: NonZeroU32,
    pub machine_fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeEnvironmentRef {
    pub environment_id: String,
    pub revision: NonZeroU32,
    pub machine_fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationRef {
    pub observation_id: String,
    pub epoch: NonZeroU32,
    pub subject_key: String,
    pub head_revision: NonZeroU32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppleStrategy {
    TraditionalLowLatency,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResourceBackendIdentity {
    LocalInstallation {
        installation: LocalInstallationRef,
        environment: RuntimeEnvironmentRef,
        root: GrantedRootRef,
        runtime_content_sha256: String,
    },
    AppleSystem {
        os_build: String,
        bridge_build_sha256: String,
        api_profile: String,
        strategy: AppleStrategy,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineResourceReference {
    pub snapshot_id: String,
    pub route_id: String,
    pub observation: ObservationRef,
    pub profile: RuntimeProfileRef,
    pub backend: ResourceBackendIdentity,
    /// For Apple this hashes the declared system metadata, never package bytes.
    pub resource_content_sha256: String,
    pub processing_sha256: String,
    pub expected_observation_sha256: String,
    pub control: Option<ControlArtifactRef>,
    pub pair: LanguagePair,
}
impl OfflineResourceReference {
    pub fn validate(&self) -> AppResult<()> {
        for id in [
            &self.snapshot_id,
            &self.route_id,
            &self.observation.observation_id,
            &self.observation.subject_key,
        ] {
            require_id(id)?
        }
        if self.observation.subject_key.len() > 512 {
            return Err(invalid("offline_invalid_subject_key"));
        }
        self.profile.validate()?;
        digest(&self.resource_content_sha256)?;
        digest(&self.processing_sha256)?;
        digest(&self.expected_observation_sha256)?;
        pair(&self.pair)?;
        match &self.backend {
            ResourceBackendIdentity::LocalInstallation {
                installation,
                environment,
                root,
                runtime_content_sha256,
            } => {
                digest(runtime_content_sha256)?;
                if self.profile.backend != OfflineBackend::LocalWorker
                    || environment.machine_fingerprint != root.machine_fingerprint
                {
                    return Err(invalid("offline_backend_identity_mismatch"));
                }
                for id in [
                    &installation.installation_id,
                    &environment.environment_id,
                    &root.grant_id,
                ] {
                    require_id(id)?
                }
                digest(&root.machine_fingerprint)?;
                self.control
                    .as_ref()
                    .ok_or_else(|| invalid("offline_control_ref_missing"))?
                    .validate()?;
            }
            ResourceBackendIdentity::AppleSystem {
                os_build,
                bridge_build_sha256,
                api_profile,
                ..
            } => {
                if self.profile.backend != OfflineBackend::PlatformNative
                    || self.profile.platform != RuntimePlatform::Macos
                    || self.profile.engine.engine_id != "apple_translation"
                    || self.control.is_some()
                {
                    return Err(invalid("offline_backend_identity_mismatch"));
                }
                text(os_build, 512)?;
                text(api_profile, 512)?;
                digest(bridge_build_sha256)?;
            }
        }
        Ok(())
    }
}
fn pair(value: &LanguagePair) -> AppResult<()> {
    text(&value.source, 32)?;
    text(&value.target, 32)?;
    if value.source == value.target {
        return Err(invalid("offline_invalid_language_pair"));
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineRouteHop {
    pub snapshot_id: String,
    pub pair: LanguagePair,
    pub pipeline_profile_id: String,
    pub package_relative_key: Option<String>,
}
pub fn validate_route(
    route: &[OfflineRouteHop],
    requested: &LanguagePair,
    policy: &OfflineRoutePolicy,
) -> AppResult<()> {
    pair(requested)?;
    if route.is_empty() || route.len() > MAX_STORED_ROUTE_HOPS {
        return Err(invalid("offline_invalid_route_length"));
    }
    if matches!(policy, OfflineRoutePolicy::DirectOnly {}) && route.len() != 1 {
        return Err(invalid("offline_pivot_not_authorized"));
    }
    if matches!(policy, OfflineRoutePolicy::ExplicitPivot { .. }) && route.len() < 2 {
        return Err(invalid("offline_invalid_route_length"));
    }
    for hop in route {
        require_id(&hop.snapshot_id)?;
        pair(&hop.pair)?;
        text(&hop.pipeline_profile_id, 160)?;
        if let Some(key) = &hop.package_relative_key {
            validate_relative_key(key)?
        }
    }
    if route[0].pair.source != requested.source
        || route.last().unwrap().pair.target != requested.target
        || route
            .windows(2)
            .any(|h| h[0].pair.target != h[1].pair.source)
    {
        return Err(invalid("offline_route_discontinuous"));
    }
    Ok(())
}
/// Non-authoritative correlation data. Repository/live-operation checks remain
/// mandatory; parsing a worker reference never creates a Task or test operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OfflineOperationTarget {
    Attempt {
        task_id: String,
        attempt_id: String,
        #[serde(deserialize_with = "strict_plan_identity")]
        identity: ServicePlanIdentity,
    },
    ExplicitTest {
        operation_id: String,
        test_id: String,
        input_scope: ServiceTestInputScope,
        input_sha256: String,
        request_semantics_sha256: String,
    },
}
fn strict_plan_identity<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<ServicePlanIdentity, D::Error> {
    // Reuse the real domain value while keeping the additive v2 decoder strict;
    // the original legacy ServicePlanIdentity grammar remains unchanged.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct StrictIdentity {
        configuration_revision: String,
        binding_id: String,
        engine_id: String,
        purpose: CatalogPurpose,
        unit_id: String,
        text_revision: u32,
        source_hash: String,
        session_id: String,
        game_id: Option<String>,
    }
    let value = StrictIdentity::deserialize(deserializer)?;
    Ok(ServicePlanIdentity {
        configuration_revision: value.configuration_revision,
        binding_id: value.binding_id,
        engine_id: value.engine_id,
        purpose: value.purpose,
        unit_id: value.unit_id,
        text_revision: value.text_revision,
        source_hash: value.source_hash,
        session_id: value.session_id,
        game_id: value.game_id,
    })
}
impl OfflineOperationTarget {
    pub fn validate(&self) -> AppResult<()> {
        match self {
            Self::Attempt {
                task_id,
                attempt_id,
                identity,
            } => {
                for id in [
                    task_id,
                    attempt_id,
                    &identity.configuration_revision,
                    &identity.binding_id,
                    &identity.engine_id,
                    &identity.unit_id,
                    &identity.session_id,
                ] {
                    require_id(id)?
                }
                if identity.purpose != CatalogPurpose::Translation || identity.text_revision == 0 {
                    return Err(invalid("offline_invalid_operation"));
                }
                digest(&identity.source_hash)?;
                if let Some(game) = &identity.game_id {
                    require_id(game)?
                }
            }
            Self::ExplicitTest {
                operation_id,
                test_id,
                input_scope,
                input_sha256,
                request_semantics_sha256,
            } => {
                require_id(operation_id)?;
                require_id(test_id)?;
                digest(input_sha256)?;
                digest(request_semantics_sha256)?;
                if let ServiceTestInputScope::SelectedUnit {
                    unit_id,
                    source_hash,
                    ..
                } = input_scope
                {
                    require_id(unit_id)?;
                    digest(source_hash)?
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkerProtocolV2 {
    #[serde(rename = "translation-worker-v2")]
    V2,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineWorkerReferenceV2 {
    pub protocol: WorkerProtocolV2,
    pub worker_instance_id: String,
    pub profile_generation: NonZeroU32,
    pub server_instance_id: String,
    pub source: OfflineConfigRef,
    pub operation: OfflineOperationTarget,
    pub resource: OfflineResourceReference,
}
impl OfflineWorkerReferenceV2 {
    pub fn validate(&self) -> AppResult<()> {
        for id in [
            &self.worker_instance_id,
            &self.server_instance_id,
            &self.source.revision_id,
            &self.source.config_id,
        ] {
            require_id(id)?
        }
        self.operation.validate()?;
        self.resource.validate()?;
        if let OfflineOperationTarget::Attempt { identity, .. } = &self.operation
            && identity.configuration_revision != self.source.revision_id {
                return Err(invalid("offline_operation_source_mismatch"));
            }
        Ok(())
    }
}
pub fn encode_v2_reference(value: &OfflineWorkerReferenceV2) -> AppResult<Vec<u8>> {
    value.validate()?;
    let mut bytes = serde_json::to_vec(value).map_err(|_| invalid("offline_invalid_v2_json"))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_V2_FRAME_BYTES {
        return Err(invalid("offline_v2_frame_too_large"));
    }
    Ok(bytes)
}
pub fn decode_v2_reference(bytes: &[u8]) -> AppResult<OfflineWorkerReferenceV2> {
    if bytes.len() > MAX_V2_FRAME_BYTES
        || bytes.last() != Some(&b'\n')
        || bytes[..bytes.len().saturating_sub(1)].contains(&b'\n')
    {
        return Err(invalid("offline_invalid_v2_frame"));
    }
    let value: OfflineWorkerReferenceV2 =
        serde_json::from_slice(bytes).map_err(|_| invalid("offline_invalid_v2_json"))?;
    value.validate()?;
    Ok(value)
}
