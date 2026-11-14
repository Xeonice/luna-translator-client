//! Private worker protocol. It is unrelated to Hook or language-worker wire identities.
use super::*;
use sha2::{Digest, Sha256};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParentFrame {
    Hello {
        protocol: String,
        worker_instance_id: String,
        engine_id: String,
        contract_revision: u32,
        request_id: String,
        identity: Option<ExecutionIdentity>,
        resource: Box<ResourceManifest>,
        operation: WorkerOperation,
    },
    Execute {
        protocol: String,
        worker_instance_id: String,
        identity: ExecutionIdentity,
        source_text: String,
    },
    Cancel {
        protocol: String,
        worker_instance_id: String,
        request_id: String,
    },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOperation {
    Observe,
    Translate,
    Prepare,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ChildFrame {
    Ready {
        protocol: String,
        worker_instance_id: String,
        request_id: String,
        resource_digest: String,
        observation: RuntimeObservation,
    },
    Final {
        protocol: String,
        worker_instance_id: String,
        identity: Box<ExecutionIdentity>,
        resource_identity: ResourceIdentity,
        resource_digest: String,
        route: Vec<RouteHop>,
        text: String,
    },
    Error {
        protocol: String,
        worker_instance_id: String,
        request_id: String,
        error: OfflineFailure,
    },
}
pub fn validate_request(request: &OfflineExecution) -> OfflineResult<OfflineEngine> {
    let engine = OfflineEngine::parse(&request.config.engine.engine_id)?;
    if request.config.request_defaults.purpose != app_core::contracts::CatalogPurpose::Translation {
        return Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedInput,
            "purpose",
        ));
    }
    if request.config.engine.contract_revision.get() != 1 {
        return Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedContract,
            "configuration",
        ));
    }
    let correct_backend = matches!(
        (&request.config.backend, engine),
        (
            app_core::domain::model_config::service_graph::OfflineBackend::PlatformNative,
            OfflineEngine::AppleTranslation
        ) | (
            app_core::domain::model_config::service_graph::OfflineBackend::LocalWorker,
            OfflineEngine::ArgosTranslate
                | OfflineEngine::Jbeijing
                | OfflineEngine::KingsoftFastait
        )
    );
    if !correct_backend {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidRequest,
            "backend",
        ));
    }
    let i = &request.identity;
    for s in [
        &i.request_id,
        &i.task_id,
        &i.attempt_id,
        &i.configuration_revision,
        &i.lease_id,
        &i.unit_id,
    ] {
        if s.is_empty() || s.len() > 256 || s.chars().any(char::is_control) {
            return Err(OfflineFailure::new(
                OfflineErrorCode::InvalidRequest,
                "identity",
            ));
        }
    }
    if i.authorization_epoch == 0
        || i.profile_generation == 0
        || i.text_revision == 0
        || !super::resources::is_digest(&i.source_hash)
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidRequest,
            "identity",
        ));
    }
    if request.payload.source_text.is_empty()
        || request.payload.source_text.len() > MAX_TEXT_BYTES
        || request.payload.source_text.contains('\0')
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidRequest,
            "input",
        ));
    }
    if format!(
        "{:x}",
        Sha256::digest(request.payload.source_text.as_bytes())
    ) != i.source_hash
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidRequest,
            "source_hash",
        ));
    }
    if !request.payload.context.is_empty()
        || !request.payload.glossary.is_empty()
        || request.payload.selected_token.is_some()
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedInput,
            "input_capabilities",
        ));
    }
    if request.payload.source_language != request.config.pair.source
        || request.payload.target_language != request.config.pair.target
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedPair,
            "language_pair",
        ));
    }
    let app_core::domain::model_config::service_graph::OfflineConfigResolution::Resolved {
        resource_snapshot_id,
        route_id,
    } = &request.config.resolution
    else {
        return Err(OfflineFailure::new(
            OfflineErrorCode::NotReady,
            "configuration",
        ));
    };
    if resource_snapshot_id != &request.resource.identity.snapshot_id
        || route_id != &request.resource.identity.route_id
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ResourceChanged,
            "resource_identity",
        ));
    }
    match &request.config.installation {
        Some(installation)
            if installation.installation_id != request.resource.identity.installation_id
                || installation.revision.get()
                    != request.resource.identity.installation_revision =>
        {
            return Err(OfflineFailure::new(
                OfflineErrorCode::ResourceChanged,
                "installation_identity",
            ));
        }
        None if matches!(
            request.config.backend,
            app_core::domain::model_config::service_graph::OfflineBackend::LocalWorker
        ) =>
        {
            return Err(OfflineFailure::new(
                OfflineErrorCode::NotReady,
                "installation_identity",
            ));
        }
        _ => {}
    }
    let route = &request.resource.route;
    if route.is_empty()
        || route.len() > MAX_ROUTE_HOPS
        || route.first().unwrap().pair.source != request.config.pair.source
        || route.last().unwrap().pair.target != request.config.pair.target
        || route
            .windows(2)
            .any(|x| x[0].pair.target != x[1].pair.source)
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidResource,
            "route",
        ));
    }
    if matches!(
        request.config.route_policy,
        app_core::domain::model_config::service_graph::OfflineRoutePolicy::DirectOnly {}
    ) && route.len() != 1
    {
        return Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedPair,
            "direct_route",
        ));
    }
    if engine != OfflineEngine::ArgosTranslate && route.len() != 1 {
        return Err(OfflineFailure::new(
            OfflineErrorCode::UnsupportedPair,
            "route",
        ));
    }
    if request.timeout.is_zero() || request.timeout > Duration::from_secs(600) {
        return Err(OfflineFailure::new(
            OfflineErrorCode::InvalidRequest,
            "deadline",
        ));
    }
    Ok(engine)
}
pub fn encode(frame: &ParentFrame) -> OfflineResult<Vec<u8>> {
    let mut out = serde_json::to_vec(frame)
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::ProtocolViolation, "encode"))?;
    out.push(b'\n');
    if out.len() > MAX_FRAME_BYTES {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ProtocolViolation,
            "frame_budget",
        ));
    }
    Ok(out)
}
pub fn decode(bytes: &[u8]) -> OfflineResult<ChildFrame> {
    if bytes.len() > MAX_FRAME_BYTES || bytes.last() != Some(&b'\n') {
        return Err(OfflineFailure::new(
            OfflineErrorCode::ProtocolViolation,
            "frame_budget",
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|_| OfflineFailure::new(OfflineErrorCode::ProtocolViolation, "decode"))
}
