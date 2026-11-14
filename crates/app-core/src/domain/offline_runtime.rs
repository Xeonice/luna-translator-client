//! Offline v2 references and metadata claims. No file, SQL, worker or grant is
//! opened here. Canonical/shape validation never creates execution authority.
use crate::{
    contracts::{AppError, AppResult, CatalogPurpose, ServiceTestInputScope},
    domain::{
        canonical_hash,
        model_config::service_graph::{
            LanguagePair, LocalInstallationRef, OfflineBackend, OfflineConfigRef, OfflineEngineKey,
            OfflineRoutePolicy,
        },
        require_id,
        service_execution::ServicePlanIdentity,
    },
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, num::NonZeroU32};

pub const CONTROL_RELATIVE_KEY: &str = ".luna/runtime-bundle.v1.json";
pub const MAX_RELATIVE_KEY_BYTES: usize = 1024;
pub const MAX_SNAPSHOT_FILES: usize = 16_384;
pub const MAX_CONTROL_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_V2_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_STORED_ROUTE_HOPS: usize = 3;
pub const PRIVATE_PROTOCOL_REVISION: u32 = 2;
fn invalid(code: &str) -> AppError {
    AppError::new(
        code,
        "offline_runtime_contract",
        "离线资源、协议或观察的结构与固定身份不一致",
    )
}
fn digest(value: &str) -> AppResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("offline_invalid_digest"));
    }
    Ok(())
}
fn text(value: &str, maximum: usize) -> AppResult<()> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(invalid("offline_invalid_text"));
    }
    Ok(())
}

mod coverage;
mod metadata;
mod references;
mod worker_v2;
pub use coverage::*;
pub use metadata::*;
pub use references::*;
pub use worker_v2::*;
