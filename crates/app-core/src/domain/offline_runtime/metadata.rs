use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePlatform {
    Macos,
    Windows,
    Crossover,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeArchitecture {
    Arm64,
    X86,
    X64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProfileRef {
    pub profile_id: String,
    pub revision: NonZeroU32,
    pub engine: OfflineEngineKey,
    pub backend: OfflineBackend,
    pub platform: RuntimePlatform,
    pub architecture: RuntimeArchitecture,
    pub private_protocol_revision: NonZeroU32,
    pub policy_sha256: String,
}
impl RuntimeProfileRef {
    /// CrossOver is the parent execution environment; its Windows helper still
    /// reports the Windows guest platform instead of inventing a host platform.
    pub fn expected_worker_platform(&self) -> RuntimePlatform {
        match self.platform {
            RuntimePlatform::Crossover => RuntimePlatform::Windows,
            _ => self.platform.clone(),
        }
    }
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.profile_id)?;
        require_id(&self.engine.engine_id)?;
        digest(&self.policy_sha256)?;
        if self.private_protocol_revision.get() != PRIVATE_PROTOCOL_REVISION {
            return Err(invalid("offline_private_protocol_mismatch"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProcessingValue {
    Boolean { value: bool },
    Integer { value: i64 },
    Text { value: String },
    TextList { value: Vec<String> },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessingSetting {
    pub key: String,
    pub value: ProcessingValue,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProcessingContract {
    pub contract_id: String,
    pub revision: NonZeroU32,
    pub settings: Vec<ProcessingSetting>,
}
impl std::fmt::Debug for RuntimeProcessingContract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeProcessingContract")
            .field("contract_id", &self.contract_id)
            .field("revision", &self.revision)
            .field("setting_count", &self.settings.len())
            .finish_non_exhaustive()
    }
}
impl RuntimeProcessingContract {
    pub fn canonical_sha256(&self) -> AppResult<String> {
        require_id(&self.contract_id)?;
        let mut keys = BTreeSet::new();
        for setting in &self.settings {
            text(&setting.key, 512)?;
            if !keys.insert(&setting.key) {
                return Err(invalid("offline_duplicate_processing_key"));
            }
            let valid = |s: &String| !s.contains('\0');
            match &setting.value {
                ProcessingValue::Text { value } if !valid(value) => {
                    return Err(invalid("offline_invalid_processing_value"));
                }
                ProcessingValue::TextList { value } if !value.iter().all(valid) => {
                    return Err(invalid("offline_invalid_processing_value"));
                }
                _ => {}
            }
        }
        let mut ordered = self.settings.iter().collect::<Vec<_>>();
        ordered.sort_by(|a, b| a.key.as_bytes().cmp(b.key.as_bytes()));
        canonical_hash(&(
            "offline-processing-v1",
            &self.contract_id,
            self.revision,
            ordered,
        ))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAvailability {
    Installed,
    SupportedNotInstalled,
    Unsupported,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedRuntimeObservation {
    pub engine: OfflineEngineKey,
    pub runtime_version: String,
    pub platform: RuntimePlatform,
    pub architecture: RuntimeArchitecture,
    pub resource_version: Option<String>,
    pub physical_cancel_supported: bool,
    pub processing_sha256: String,
}
impl ExpectedRuntimeObservation {
    pub fn canonical_sha256(&self) -> AppResult<String> {
        self.validate()?;
        canonical_hash(&("offline-expected-observation-v1", self))
    }
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.engine.engine_id)?;
        text(&self.runtime_version, 4096)?;
        digest(&self.processing_sha256)?;
        if let Some(version) = &self.resource_version {
            text(version, 512)?
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFileRole {
    Interpreter,
    StandardLibrary,
    PackageCode,
    NativeLibrary,
    RuntimeMetadata,
    Worker,
    Engine,
    Dictionary,
    LanguagePackage,
    Tokenizer,
    SentenceBoundary,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoredFileRole {
    Runtime,
    Worker,
    Engine,
    Dictionary,
    LanguagePackage,
    Tokenizer,
    SentenceBoundary,
}
impl RuntimeFileRole {
    pub fn stored_role(&self) -> StoredFileRole {
        match self {
            Self::Interpreter
            | Self::StandardLibrary
            | Self::PackageCode
            | Self::NativeLibrary
            | Self::RuntimeMetadata => StoredFileRole::Runtime,
            Self::Worker => StoredFileRole::Worker,
            Self::Engine => StoredFileRole::Engine,
            Self::Dictionary => StoredFileRole::Dictionary,
            Self::LanguagePackage => StoredFileRole::LanguagePackage,
            Self::Tokenizer => StoredFileRole::Tokenizer,
            Self::SentenceBoundary => StoredFileRole::SentenceBoundary,
        }
    }
}
pub fn validate_relative_key(key: &str) -> AppResult<()> {
    if key.is_empty()
        || key.len() > MAX_RELATIVE_KEY_BYTES
        || key.starts_with('/')
        || key.contains(['\\', ':', '\0'])
        || key
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(invalid("offline_invalid_relative_key"));
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeFileClaim {
    pub relative_key: String,
    pub byte_length: u64,
    pub sha256: String,
    pub role: RuntimeFileRole,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlArtifactRef {
    pub relative_key: String,
    pub byte_length: u64,
    pub sha256: String,
}
impl ControlArtifactRef {
    pub fn validate(&self) -> AppResult<()> {
        if self.relative_key != CONTROL_RELATIVE_KEY
            || self.byte_length == 0
            || self.byte_length > MAX_CONTROL_BYTES as u64
        {
            return Err(invalid("offline_invalid_control_ref"));
        }
        digest(&self.sha256)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BundleSchema {
    #[serde(rename = "offline-runtime-bundle-v1")]
    V1,
}
/// This is untrusted metadata, never a coverage proof. It deliberately contains
/// no grant, root locator, observation epoch, task, attempt or Complete flag.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeBundleManifest {
    pub schema: BundleSchema,
    pub profile: RuntimeProfileRef,
    pub processing: RuntimeProcessingContract,
    pub expected: ExpectedRuntimeObservation,
    pub payload_files: Vec<RuntimeFileClaim>,
    pub content_sha256: String,
}
impl std::fmt::Debug for RuntimeBundleManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeBundleManifest")
            .field("profile", &self.profile)
            .field("file_count", &self.payload_files.len())
            .field("content_sha256", &self.content_sha256)
            .finish_non_exhaustive()
    }
}
pub fn runtime_content_sha256(
    profile: &RuntimeProfileRef,
    processing: &RuntimeProcessingContract,
    expected: &ExpectedRuntimeObservation,
    files: &[RuntimeFileClaim],
) -> AppResult<String> {
    profile.validate()?;
    expected.validate()?;
    if profile.backend != OfflineBackend::LocalWorker
        || profile.engine != expected.engine
        || profile.expected_worker_platform() != expected.platform
        || profile.architecture != expected.architecture
        || processing.canonical_sha256()? != expected.processing_sha256
    {
        return Err(invalid("offline_runtime_metadata_mismatch"));
    }
    if files.is_empty() || files.len() >= MAX_SNAPSHOT_FILES {
        return Err(invalid("offline_file_count_exceeded"));
    }
    let mut seen = BTreeSet::new();
    let mut singleton = BTreeSet::new();
    let mut total = 0u64;
    for file in files {
        validate_relative_key(&file.relative_key)?;
        digest(&file.sha256)?;
        if file.relative_key == CONTROL_RELATIVE_KEY {
            return Err(invalid("offline_control_self_reference"));
        }
        if !seen.insert(&file.relative_key) {
            return Err(invalid("offline_duplicate_file_key"));
        }
        let name = match file.role {
            RuntimeFileRole::Interpreter => Some("interpreter"),
            RuntimeFileRole::Worker => Some("worker"),
            _ => None,
        };
        if name.is_some_and(|name| !singleton.insert(name)) {
            return Err(invalid("offline_duplicate_singleton_role"));
        }
        total = total
            .checked_add(file.byte_length)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or_else(|| invalid("offline_file_length_overflow"))?;
    }
    let mut ordered = files.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.relative_key.as_bytes().cmp(b.relative_key.as_bytes()));
    canonical_hash(&(
        "offline-runtime-closure-v1",
        profile,
        processing.canonical_sha256()?,
        expected,
        ordered,
    ))
}
impl RuntimeBundleManifest {
    pub fn validate(&self) -> AppResult<()> {
        digest(&self.content_sha256)?;
        if runtime_content_sha256(
            &self.profile,
            &self.processing,
            &self.expected,
            &self.payload_files,
        )? != self.content_sha256
        {
            return Err(invalid("offline_runtime_content_mismatch"));
        }
        Ok(())
    }
}
pub fn decode_bundle_metadata(bytes: &[u8]) -> AppResult<RuntimeBundleManifest> {
    if bytes.len() > MAX_CONTROL_BYTES {
        return Err(invalid("offline_control_too_large"));
    }
    let bundle: RuntimeBundleManifest =
        serde_json::from_slice(bytes).map_err(|_| invalid("offline_invalid_bundle_json"))?;
    bundle.validate()?;
    Ok(bundle)
}
/// Binds parsed metadata to the actual bytes supplied by the resource owner.
/// It performs no filesystem read and is still not a coverage/permission proof.
pub fn decode_control_artifact(
    bytes: &[u8],
    reference: &ControlArtifactRef,
) -> AppResult<RuntimeBundleManifest> {
    reference.validate()?;
    if bytes.len() as u64 != reference.byte_length
        || crate::domain::content_hash(bytes) != reference.sha256
    {
        return Err(invalid("offline_control_bytes_mismatch"));
    }
    decode_bundle_metadata(bytes)
}
pub fn encode_bundle_metadata(bundle: &RuntimeBundleManifest) -> AppResult<Vec<u8>> {
    bundle.validate()?;
    let mut canonical = bundle.clone();
    canonical
        .payload_files
        .sort_by(|a, b| a.relative_key.as_bytes().cmp(b.relative_key.as_bytes()));
    canonical
        .processing
        .settings
        .sort_by(|a, b| a.key.as_bytes().cmp(b.key.as_bytes()));
    let bytes =
        serde_json::to_vec(&canonical).map_err(|_| invalid("offline_invalid_bundle_json"))?;
    if bytes.len() > MAX_CONTROL_BYTES {
        return Err(invalid("offline_control_too_large"));
    }
    Ok(bytes)
}
/// Includes the control artifact's real file hash, unlike the runtime-content
/// domain. The artifact does not recursively contain its own file row.
pub fn snapshot_manifest_sha256(
    bundle: &RuntimeBundleManifest,
    control: &ControlArtifactRef,
) -> AppResult<String> {
    bundle.validate()?;
    control.validate()?;
    let mut rows = bundle
        .payload_files
        .iter()
        .map(|f| {
            (
                f.relative_key.as_str(),
                f.byte_length,
                f.sha256.as_str(),
                f.role.stored_role(),
            )
        })
        .collect::<Vec<_>>();
    rows.push((
        &control.relative_key,
        control.byte_length,
        &control.sha256,
        StoredFileRole::Runtime,
    ));
    rows.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    canonical_hash(&("offline-snapshot-files-v1", rows))
}
