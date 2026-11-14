//! Strict private wire facts only. No loader, installed state, lease or I/O authority.
use super::*;
use crate::domain::content_hash;
use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use std::fmt;

pub const MAX_V2_SOURCE_BYTES: usize = 65_536;
pub const MAX_V2_OUTPUT_BYTES: usize = 262_144;
pub const MAX_V2_EVIDENCE_CHUNK: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineWorkerHelloV2 {
    pub reference: OfflineWorkerReferenceV2,
    pub expected: ExpectedRuntimeObservation,
    pub route: Vec<OfflineRouteHop>,
    pub route_policy: OfflineRoutePolicy,
}
impl OfflineWorkerHelloV2 {
    pub fn validate(&self) -> AppResult<()> {
        self.reference.validate()?;
        self.expected.validate()?;
        let r = &self.reference.resource;
        if self.expected.engine != r.profile.engine
            || self.expected.platform != r.profile.expected_worker_platform()
            || self.expected.architecture != r.profile.architecture
            || self.expected.processing_sha256 != r.processing_sha256
            || self.expected.canonical_sha256()? != r.expected_observation_sha256
        {
            return Err(invalid("offline_v2_expected_binding"));
        }
        validate_route(&self.route, &r.pair, &self.route_policy)?;
        if self.route[0].snapshot_id != r.snapshot_id {
            return Err(invalid("offline_v2_route_binding"));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParentFrameV2 {
    Hello {
        hello: Box<OfflineWorkerHelloV2>,
    },
    Execute {
        reference: Box<OfflineWorkerReferenceV2>,
        source_text: String,
    },
    Cancel {
        reference: Box<OfflineWorkerReferenceV2>,
    },
}
impl fmt::Debug for ParentFrameV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Hello { .. } => "HelloV2(..)",
            Self::Execute { .. } => "ExecuteV2(<redacted>)",
            Self::Cancel { .. } => "CancelV2(..)",
        })
    }
}
impl ParentFrameV2 {
    pub fn reference(&self) -> &OfflineWorkerReferenceV2 {
        match self {
            Self::Hello { hello } => &hello.reference,
            Self::Execute { reference, .. } | Self::Cancel { reference } => reference,
        }
    }
    pub fn validate(&self) -> AppResult<()> {
        self.reference().validate()?;
        match self {
            Self::Hello { hello } => hello.validate(),
            Self::Execute {
                reference,
                source_text,
            } => {
                payload(source_text, MAX_V2_SOURCE_BYTES)?;
                let expected = match &reference.operation {
                    OfflineOperationTarget::Attempt { identity, .. } => &identity.source_hash,
                    OfflineOperationTarget::ExplicitTest { input_sha256, .. } => input_sha256,
                };
                if content_hash(source_text) != *expected {
                    return Err(invalid("offline_v2_source_mismatch"));
                }
                Ok(())
            }
            Self::Cancel { .. } => Ok(()),
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerEvidencePhaseV2 {
    Initialization,
    Execution,
}
/// Untrusted records from a child, never RuntimeCoverageReport or a grant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkerEvidenceRecordV2 {
    File {
        claim: RuntimeFileClaim,
    },
    SystemApi {
        os_build: String,
        bridge_build_sha256: String,
        api_profile: String,
        strategy: AppleStrategy,
        observation_epoch: NonZeroU32,
    },
}
impl WorkerEvidenceRecordV2 {
    fn validate(&self, reference: &OfflineWorkerReferenceV2) -> AppResult<()> {
        match (self, &reference.resource.backend) {
            (Self::File { claim }, ResourceBackendIdentity::LocalInstallation { .. }) => {
                validate_relative_key(&claim.relative_key)?;
                if claim.relative_key == CONTROL_RELATIVE_KEY || claim.byte_length > i64::MAX as u64
                {
                    return Err(invalid("offline_v2_evidence_file"));
                }
                digest(&claim.sha256)
            }
            (
                Self::SystemApi {
                    os_build,
                    bridge_build_sha256,
                    api_profile,
                    strategy,
                    observation_epoch,
                },
                ResourceBackendIdentity::AppleSystem {
                    os_build: eb,
                    bridge_build_sha256: eh,
                    api_profile: ep,
                    strategy: es,
                },
            ) => {
                if os_build != eb
                    || bridge_build_sha256 != eh
                    || api_profile != ep
                    || strategy != es
                    || *observation_epoch != reference.resource.observation.epoch
                {
                    return Err(invalid("offline_v2_system_binding"));
                }
                Ok(())
            }
            _ => Err(invalid("offline_v2_evidence_backend")),
        }
    }
    fn key(&self) -> &str {
        match self {
            Self::File { claim } => &claim.relative_key,
            Self::SystemApi { .. } => "@system-api",
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ChildFrameV2 {
    Evidence {
        reference: Box<OfflineWorkerReferenceV2>,
        phase: WorkerEvidencePhaseV2,
        sequence: NonZeroU32,
        records: Vec<WorkerEvidenceRecordV2>,
    },
    Ready {
        reference: Box<OfflineWorkerReferenceV2>,
        observed: ExpectedRuntimeObservation,
        availability: ResourceAvailability,
        evidence_count: u32,
        evidence_sha256: String,
    },
    Final {
        reference: Box<OfflineWorkerReferenceV2>,
        route: Vec<OfflineRouteHop>,
        text: String,
        evidence_count: u32,
        evidence_sha256: String,
    },
    Error {
        reference: Box<OfflineWorkerReferenceV2>,
        reason_code: String,
    },
}
impl fmt::Debug for ChildFrameV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Evidence { .. } => "EvidenceV2(..)",
            Self::Ready { .. } => "ReadyV2(..)",
            Self::Final { .. } => "FinalV2(<redacted>)",
            Self::Error { .. } => "ErrorV2(..)",
        })
    }
}
impl ChildFrameV2 {
    pub fn reference(&self) -> &OfflineWorkerReferenceV2 {
        match self {
            Self::Evidence { reference, .. }
            | Self::Ready { reference, .. }
            | Self::Final { reference, .. }
            | Self::Error { reference, .. } => reference,
        }
    }
    pub fn validate(&self) -> AppResult<()> {
        self.reference().validate()?;
        match self {
            Self::Evidence {
                reference, records, ..
            } => {
                if records.is_empty() || records.len() > MAX_V2_EVIDENCE_CHUNK {
                    return Err(invalid("offline_v2_evidence_chunk"));
                }
                let mut keys = BTreeSet::new();
                for record in records {
                    record.validate(reference)?;
                    if !keys.insert(record.key()) {
                        return Err(invalid("offline_v2_duplicate_evidence"));
                    }
                }
            }
            Self::Ready {
                observed,
                evidence_count,
                evidence_sha256,
                ..
            } => {
                observed.validate()?;
                evidence_summary(*evidence_count, evidence_sha256)?;
            }
            Self::Final {
                route,
                text,
                evidence_count,
                evidence_sha256,
                ..
            } => {
                payload(text, MAX_V2_OUTPUT_BYTES)?;
                if route.is_empty() || route.len() > MAX_STORED_ROUTE_HOPS {
                    return Err(invalid("offline_invalid_route_length"));
                }
                for hop in route {
                    require_id(&hop.snapshot_id)?;
                    super::text(&hop.pair.source, 32)?;
                    super::text(&hop.pair.target, 32)?;
                    if hop.pair.source == hop.pair.target {
                        return Err(invalid("offline_invalid_language_pair"));
                    }
                    super::text(&hop.pipeline_profile_id, 160)?;
                    if let Some(key) = &hop.package_relative_key {
                        validate_relative_key(key)?;
                    }
                }
                evidence_summary(*evidence_count, evidence_sha256)?;
            }
            Self::Error { reason_code, .. } => {
                if reason_code.is_empty()
                    || reason_code.len() > 160
                    || !reason_code
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                {
                    return Err(invalid("offline_v2_error_code"));
                }
            }
        }
        Ok(())
    }
}
fn payload(value: &str, max: usize) -> AppResult<()> {
    if value.is_empty() || value.len() > max || value.contains('\0') {
        return Err(invalid("offline_v2_payload_size"));
    }
    Ok(())
}
fn evidence_summary(count: u32, sha: &str) -> AppResult<()> {
    if count as usize > MAX_SNAPSHOT_FILES {
        return Err(invalid("offline_v2_evidence_limit"));
    }
    digest(sha)
}
/// Canonical typed wire-record digest, not proof of actual file I/O or authority.
pub fn evidence_records_sha256_v2(
    phase: WorkerEvidencePhaseV2,
    records: &[WorkerEvidenceRecordV2],
) -> AppResult<String> {
    if records.len() > MAX_SNAPSHOT_FILES {
        return Err(invalid("offline_v2_evidence_limit"));
    }
    canonical_hash(&("offline-worker-evidence-v2", phase, records))
}
struct FrameBytes {
    bytes: Vec<u8>,
    overflow: bool,
}
impl std::io::Write for FrameBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MAX_V2_FRAME_BYTES - 1 {
            self.overflow = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "frame limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode<T: Serialize>(value: &T) -> AppResult<Vec<u8>> {
    let mut output = FrameBytes {
        bytes: Vec::new(),
        overflow: false,
    };
    if serde_json::to_writer(&mut output, value).is_err() {
        return Err(invalid(if output.overflow {
            "offline_v2_frame_too_large"
        } else {
            "offline_v2_invalid_json"
        }));
    }
    output.bytes.push(b'\n');
    Ok(output.bytes)
}

pub fn encode_parent_v2(value: &ParentFrameV2) -> AppResult<Vec<u8>> {
    value.validate()?;
    encode(value)
}
pub fn encode_child_v2(value: &ChildFrameV2) -> AppResult<Vec<u8>> {
    value.validate()?;
    encode(value)
}
pub fn decode_parent_v2(bytes: &[u8]) -> AppResult<ParentFrameV2> {
    let v: ParentFrameV2 = decode(bytes)?;
    v.validate()?;
    Ok(v)
}
pub fn decode_child_v2(bytes: &[u8]) -> AppResult<ChildFrameV2> {
    let v: ChildFrameV2 = decode(bytes)?;
    v.validate()?;
    Ok(v)
}
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> AppResult<T> {
    if bytes.len() > MAX_V2_FRAME_BYTES
        || bytes.last() != Some(&b'\n')
        || bytes[..bytes.len().saturating_sub(1)].contains(&b'\n')
    {
        return Err(invalid("offline_invalid_v2_frame"));
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    UniqueJson::deserialize(&mut d)
        .and_then(|_| d.end())
        .map_err(|_| invalid("offline_v2_invalid_json"))?;
    serde_json::from_slice(bytes).map_err(|_| invalid("offline_v2_invalid_json"))
}
struct UniqueJson;
impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueJson;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded JSON without duplicate keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut keys = BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key) {
                        return Err(de::Error::custom("duplicate key"));
                    }
                    map.next_value::<UniqueJson>()?;
                }
                Ok(UniqueJson)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                while seq.next_element::<UniqueJson>()?.is_some() {}
                Ok(UniqueJson)
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueJson)
            }
        }
        d.deserialize_any(UniqueVisitor)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerConversationStateV2 {
    Created,
    AwaitingReady,
    ReadyObserved,
    AwaitingFinal,
    Terminated,
}
/// This pure grammar can model Execute. It never sends bytes or authorizes I/O.
pub struct WorkerConversationV2 {
    hello: OfflineWorkerHelloV2,
    state: WorkerConversationStateV2,
    records: Vec<WorkerEvidenceRecordV2>,
    next_sequence: u32,
}
impl WorkerConversationV2 {
    pub fn new(hello: OfflineWorkerHelloV2) -> AppResult<Self> {
        hello.validate()?;
        Ok(Self {
            hello,
            state: WorkerConversationStateV2::Created,
            records: Vec::new(),
            next_sequence: 1,
        })
    }
    pub fn state(&self) -> WorkerConversationStateV2 {
        self.state
    }
    pub fn sent(&mut self, frame: &ParentFrameV2) -> AppResult<()> {
        frame.validate()?;
        if frame.reference() != &self.hello.reference {
            return Err(invalid("offline_v2_reference_mismatch"));
        }
        match (self.state, frame) {
            (WorkerConversationStateV2::Created, ParentFrameV2::Hello { hello })
                if **hello == self.hello =>
            {
                self.state = WorkerConversationStateV2::AwaitingReady
            }
            (WorkerConversationStateV2::ReadyObserved, ParentFrameV2::Execute { .. }) => {
                self.state = WorkerConversationStateV2::AwaitingFinal;
                self.records.clear();
                self.next_sequence = 1;
            }
            (
                WorkerConversationStateV2::AwaitingReady
                | WorkerConversationStateV2::ReadyObserved
                | WorkerConversationStateV2::AwaitingFinal,
                ParentFrameV2::Cancel { .. },
            ) => self.state = WorkerConversationStateV2::Terminated,
            _ => return Err(invalid("offline_v2_state_violation")),
        }
        Ok(())
    }
    pub fn received(&mut self, frame: &ChildFrameV2) -> AppResult<()> {
        frame.validate()?;
        if frame.reference() != &self.hello.reference {
            return Err(invalid("offline_v2_reference_mismatch"));
        }
        match (self.state, frame) {
            (
                WorkerConversationStateV2::AwaitingReady | WorkerConversationStateV2::AwaitingFinal,
                ChildFrameV2::Evidence {
                    phase,
                    sequence,
                    records,
                    ..
                },
            ) => {
                let phase_expected = if self.state == WorkerConversationStateV2::AwaitingReady {
                    WorkerEvidencePhaseV2::Initialization
                } else {
                    WorkerEvidencePhaseV2::Execution
                };
                if *phase != phase_expected || sequence.get() != self.next_sequence {
                    return Err(invalid("offline_v2_evidence_sequence"));
                }
                if self.records.len().saturating_add(records.len()) > MAX_SNAPSHOT_FILES {
                    return Err(invalid("offline_v2_evidence_limit"));
                }
                let mut keys: BTreeSet<_> = self
                    .records
                    .iter()
                    .map(WorkerEvidenceRecordV2::key)
                    .collect();
                for record in records {
                    if !keys.insert(record.key()) {
                        return Err(invalid("offline_v2_duplicate_evidence"));
                    }
                }
                self.records.extend(records.iter().cloned());
                self.next_sequence += 1;
            }
            (
                WorkerConversationStateV2::AwaitingReady,
                ChildFrameV2::Ready {
                    observed,
                    evidence_count,
                    evidence_sha256,
                    ..
                },
            ) => {
                if *observed != self.hello.expected {
                    return Err(invalid("offline_v2_observation_mismatch"));
                }
                self.check_summary(
                    WorkerEvidencePhaseV2::Initialization,
                    *evidence_count,
                    evidence_sha256,
                )?;
                self.state = WorkerConversationStateV2::ReadyObserved;
            }
            (
                WorkerConversationStateV2::AwaitingFinal,
                ChildFrameV2::Final {
                    route,
                    evidence_count,
                    evidence_sha256,
                    ..
                },
            ) => {
                if *route != self.hello.route {
                    return Err(invalid("offline_v2_route_binding"));
                }
                self.check_summary(
                    WorkerEvidencePhaseV2::Execution,
                    *evidence_count,
                    evidence_sha256,
                )?;
                self.state = WorkerConversationStateV2::Terminated;
            }
            (
                WorkerConversationStateV2::AwaitingReady
                | WorkerConversationStateV2::ReadyObserved
                | WorkerConversationStateV2::AwaitingFinal,
                ChildFrameV2::Error { .. },
            ) => self.state = WorkerConversationStateV2::Terminated,
            _ => return Err(invalid("offline_v2_state_violation")),
        }
        Ok(())
    }
    fn check_summary(&self, phase: WorkerEvidencePhaseV2, count: u32, sha: &str) -> AppResult<()> {
        if count as usize != self.records.len()
            || evidence_records_sha256_v2(phase, &self.records)? != sha
        {
            return Err(invalid("offline_v2_evidence_summary"));
        }
        Ok(())
    }
    pub fn eof(&self) -> AppResult<()> {
        if self.state != WorkerConversationStateV2::Terminated {
            return Err(invalid("offline_v2_unexpected_eof"));
        }
        Ok(())
    }
}
