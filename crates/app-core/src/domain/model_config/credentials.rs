//! Pure credential codecs. Stored metadata, not secret contents, selects the decoder.
//! Secrets deliberately implement neither Serialize nor TS and never enter ordinary snapshots.
use crate::contracts::{AppError, AppResult};
use crate::domain::require_id;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::num::NonZeroU32;
use std::sync::atomic::{Ordering, compiler_fence};

pub const MAX_SECRET_BYTES: usize = 16_384;
pub const MAX_VAULT_ENVELOPE_BYTES: usize = 131_072;
const MAX_PUBLIC_ID_BYTES: usize = 512;
fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "credentials", message)
}
fn wipe(bytes: &mut [u8]) {
    bytes.fill(0);
    std::hint::black_box(bytes);
    compiler_fence(Ordering::SeqCst);
}
/// Owned UTF-8 secret with a deliberate exposure point for a short-lived adapter lease.
/// The best-effort wipe cannot erase copies made by the OS, allocator, or HTTP library.
/// ```compile_fail
/// use app_core::domain::model_config::credentials::SecretText;
/// let secret = SecretText::from_bytes(b"test-only-key".to_vec()).unwrap();
/// let _ = serde_json::to_vec(&secret); // ordinary serialization must not compile
/// ```
pub struct SecretText(Vec<u8>);
impl SecretText {
    pub fn from_bytes(mut bytes: Vec<u8>) -> AppResult<Self> {
        if bytes.is_empty()
            || bytes.len() > MAX_SECRET_BYTES
            || bytes.contains(&0)
            || std::str::from_utf8(&bytes).is_err()
        {
            wipe(&mut bytes);
            return Err(failure(
                "invalid_secret",
                "秘密材料为空、过长、含NUL或不是UTF-8",
            ));
        }
        Ok(Self(bytes))
    }
    pub fn expose(&self) -> &str {
        // Construction and private ownership preserve this invariant, without unsafe code.
        std::str::from_utf8(&self.0).expect("validated private UTF-8 secret")
    }
}
impl fmt::Debug for SecretText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretText([REDACTED])")
    }
}
impl Drop for SecretText {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}
impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::from_bytes(value.into_bytes())
            .map_err(|_| serde::de::Error::custom("invalid bounded secret"))
    }
}
/// Explicit Vault-only encoded bytes. No Serialize, Clone, or ordinary snapshot projection.
pub struct VaultCredentialBytes(Vec<u8>);
impl VaultCredentialBytes {
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for VaultCredentialBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VaultCredentialBytes([REDACTED])")
    }
}
impl Drop for VaultCredentialBytes {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialMaterialKind {
    ApiKey,
    BaiduAppSecret,
    YoudaoAppSecret,
    GoogleServiceAccount,
    GoogleOauthRefresh,
}
impl CredentialMaterialKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::BaiduAppSecret => "baidu_app_secret",
            Self::YoudaoAppSecret => "youdao_app_secret",
            Self::GoogleServiceAccount => "google_service_account",
            Self::GoogleOauthRefresh => "google_oauth_refresh",
        }
    }
    fn from_key(value: &str) -> AppResult<Self> {
        match value {
            "api_key" => Ok(Self::ApiKey),
            "baidu_app_secret" => Ok(Self::BaiduAppSecret),
            "youdao_app_secret" => Ok(Self::YoudaoAppSecret),
            "google_service_account" => Ok(Self::GoogleServiceAccount),
            "google_oauth_refresh" => Ok(Self::GoogleOauthRefresh),
            _ => Err(failure(
                "unsupported_credential_kind",
                "尚不支持此秘密材料种类",
            )),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStorageMetadata {
    LegacyRawApiKey,
    EnvelopeV1 { kind: CredentialMaterialKind },
}
impl CredentialStorageMetadata {
    /// Only a trusted repository metadata row may choose this format. Never infer it from a Key.
    pub fn from_stored(auth_kind: &str, envelope_schema: Option<u32>) -> AppResult<Self> {
        let kind = CredentialMaterialKind::from_key(auth_kind)?;
        match envelope_schema {
            None if kind == CredentialMaterialKind::ApiKey => Ok(Self::LegacyRawApiKey),
            Some(1) => Ok(Self::EnvelopeV1 { kind }),
            None => Err(failure(
                "credential_schema_missing",
                "只有旧API Key允许没有封套版本",
            )),
            Some(_) => Err(failure(
                "unsupported_credential_schema",
                "秘密材料封套版本不受支持",
            )),
        }
    }
}
/// Fixed material fields only. This is not an authentication header map or an executable script.
/// ```compile_fail
/// use app_core::domain::model_config::credentials::{CredentialMaterial, SecretText};
/// let material = CredentialMaterial::ApiKey { key: SecretText::from_bytes(b"test-only-key".to_vec()).unwrap() };
/// let _ = serde_json::to_vec(&material);
/// ```
pub enum CredentialMaterial {
    ApiKey {
        key: SecretText,
    },
    BaiduAppSecret {
        app_id: String,
        secret: SecretText,
    },
    YoudaoAppSecret {
        app_key: String,
        secret: SecretText,
    },
    GoogleServiceAccount {
        client_email: String,
        private_key: SecretText,
        private_key_id: Option<String>,
        project_id: Option<String>,
    },
    GoogleOauthRefresh {
        refresh_token: SecretText,
    },
}
impl CredentialMaterial {
    pub fn kind(&self) -> CredentialMaterialKind {
        match self {
            Self::ApiKey { .. } => CredentialMaterialKind::ApiKey,
            Self::BaiduAppSecret { .. } => CredentialMaterialKind::BaiduAppSecret,
            Self::YoudaoAppSecret { .. } => CredentialMaterialKind::YoudaoAppSecret,
            Self::GoogleServiceAccount { .. } => CredentialMaterialKind::GoogleServiceAccount,
            Self::GoogleOauthRefresh { .. } => CredentialMaterialKind::GoogleOauthRefresh,
        }
    }
    fn validate(&self) -> AppResult<()> {
        match self {
            Self::BaiduAppSecret { app_id, .. } => public_id(app_id),
            Self::YoudaoAppSecret { app_key, .. } => public_id(app_key),
            Self::GoogleServiceAccount {
                client_email,
                private_key_id,
                project_id,
                ..
            } => {
                public_id(client_email)?;
                if client_email.split('@').count() != 2
                    || client_email.starts_with('@')
                    || client_email.ends_with('@')
                {
                    return Err(failure(
                        "invalid_credential_subject",
                        "服务账号主体格式无效",
                    ));
                }
                for value in [private_key_id, project_id].into_iter().flatten() {
                    public_id(value)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
impl fmt::Debug for CredentialMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CredentialMaterial")
            .field(&self.kind())
            .field(&"[REDACTED]")
            .finish()
    }
}
fn public_id(value: &str) -> AppResult<()> {
    if value.trim().is_empty()
        || value.len() > MAX_PUBLIC_ID_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(failure(
            "invalid_credential_subject",
            "认证主体为空、过长或含控制字符",
        ));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnvelopeInput {
    schema_version: u32,
    material: MaterialInput,
}
#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum MaterialInput {
    ApiKey {
        key: SecretText,
    },
    BaiduAppSecret {
        app_id: String,
        secret: SecretText,
    },
    YoudaoAppSecret {
        app_key: String,
        secret: SecretText,
    },
    GoogleServiceAccount {
        client_email: String,
        private_key: SecretText,
        private_key_id: Option<String>,
        project_id: Option<String>,
    },
    GoogleOauthRefresh {
        refresh_token: SecretText,
    },
}
impl From<MaterialInput> for CredentialMaterial {
    fn from(value: MaterialInput) -> Self {
        match value {
            MaterialInput::ApiKey { key } => Self::ApiKey { key },
            MaterialInput::BaiduAppSecret { app_id, secret } => {
                Self::BaiduAppSecret { app_id, secret }
            }
            MaterialInput::YoudaoAppSecret { app_key, secret } => {
                Self::YoudaoAppSecret { app_key, secret }
            }
            MaterialInput::GoogleServiceAccount {
                client_email,
                private_key,
                private_key_id,
                project_id,
            } => Self::GoogleServiceAccount {
                client_email,
                private_key,
                private_key_id,
                project_id,
            },
            MaterialInput::GoogleOauthRefresh { refresh_token } => {
                Self::GoogleOauthRefresh { refresh_token }
            }
        }
    }
}
/// Owned input is wiped on every return path. Parsing failure never falls back to a raw Key.
pub fn decode_vault_credential(
    metadata: CredentialStorageMetadata,
    bytes: Vec<u8>,
) -> AppResult<CredentialMaterial> {
    let mut bytes = VaultCredentialBytes(bytes);
    if bytes.0.len() > MAX_VAULT_ENVELOPE_BYTES {
        return Err(failure(
            "credential_envelope_too_large",
            "秘密材料封套超过上限",
        ));
    }
    match metadata {
        CredentialStorageMetadata::LegacyRawApiKey => {
            SecretText::from_bytes(std::mem::take(&mut bytes.0))
                .map(|key| CredentialMaterial::ApiKey { key })
        }
        CredentialStorageMetadata::EnvelopeV1 { kind } => {
            let envelope: EnvelopeInput = serde_json::from_slice(bytes.expose()).map_err(|_| {
                failure(
                    "invalid_credential_envelope",
                    "秘密材料封套损坏或含未认可字段",
                )
            })?;
            if envelope.schema_version != 1 {
                return Err(failure(
                    "unsupported_credential_schema",
                    "秘密材料封套版本不受支持",
                ));
            }
            let material = CredentialMaterial::from(envelope.material);
            if material.kind() != kind {
                return Err(failure(
                    "credential_kind_mismatch",
                    "秘密材料种类与可信元数据不匹配",
                ));
            }
            material.validate()?;
            Ok(material)
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeOutput<'a> {
    schema_version: u32,
    material: MaterialOutput<'a>,
}
#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum MaterialOutput<'a> {
    ApiKey {
        key: &'a str,
    },
    BaiduAppSecret {
        app_id: &'a str,
        secret: &'a str,
    },
    YoudaoAppSecret {
        app_key: &'a str,
        secret: &'a str,
    },
    GoogleServiceAccount {
        client_email: &'a str,
        private_key: &'a str,
        private_key_id: Option<&'a str>,
        project_id: Option<&'a str>,
    },
    GoogleOauthRefresh {
        refresh_token: &'a str,
    },
}
/// This is the only serializer for secret materials, intended solely for CredentialVault::put.
pub fn encode_vault_credential(material: &CredentialMaterial) -> AppResult<VaultCredentialBytes> {
    material.validate()?;
    let wire = match material {
        CredentialMaterial::ApiKey { key } => MaterialOutput::ApiKey { key: key.expose() },
        CredentialMaterial::BaiduAppSecret { app_id, secret } => MaterialOutput::BaiduAppSecret {
            app_id,
            secret: secret.expose(),
        },
        CredentialMaterial::YoudaoAppSecret { app_key, secret } => {
            MaterialOutput::YoudaoAppSecret {
                app_key,
                secret: secret.expose(),
            }
        }
        CredentialMaterial::GoogleServiceAccount {
            client_email,
            private_key,
            private_key_id,
            project_id,
        } => MaterialOutput::GoogleServiceAccount {
            client_email,
            private_key: private_key.expose(),
            private_key_id: private_key_id.as_deref(),
            project_id: project_id.as_deref(),
        },
        CredentialMaterial::GoogleOauthRefresh { refresh_token } => {
            MaterialOutput::GoogleOauthRefresh {
                refresh_token: refresh_token.expose(),
            }
        }
    };
    let bytes = VaultCredentialBytes(
        serde_json::to_vec(&EnvelopeOutput {
            schema_version: 1,
            material: wire,
        })
        .map_err(|_| failure("invalid_credential_envelope", "无法编码秘密材料封套"))?,
    );
    if bytes.0.len() > MAX_VAULT_ENVELOPE_BYTES {
        return Err(failure(
            "credential_envelope_too_large",
            "秘密材料封套超过上限",
        ));
    }
    Ok(bytes)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialVersionRef {
    pub credential_id: String,
    pub credential_version_id: String,
}
impl CredentialVersionRef {
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.credential_id)?;
        require_id(&self.credential_version_id)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrincipalState {
    Unknown,
    Verified { subject: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleAuthorizationLeaseIdentity {
    pub grant_id: String,
    pub authorization_epoch: NonZeroU32,
    pub material_revision: NonZeroU32,
    pub credential_version: CredentialVersionRef,
    pub scopes_hash: String,
    pub principal_state: PrincipalState,
}
impl GoogleAuthorizationLeaseIdentity {
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.grant_id)?;
        self.credential_version.validate()?;
        if self.scopes_hash.len() != 64
            || !self
                .scopes_hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(failure(
                "invalid_authorization_identity",
                "授权范围指纹无效",
            ));
        }
        if let PrincipalState::Verified { subject } = &self.principal_state {
            public_id(subject)?;
        }
        Ok(())
    }
}
/// A service account is sourced from an actual pool member and credential
/// lifecycle. It is not a user OAuth grant and must never fabricate one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleServiceAccountLeaseIdentity {
    pub pool_id: String,
    pub pool_revision: NonZeroU32,
    pub member_id: String,
    pub credential_version: CredentialVersionRef,
    pub lifecycle_revision: u32,
    pub scopes_hash: String,
}
impl GoogleServiceAccountLeaseIdentity {
    pub fn validate(&self) -> AppResult<()> {
        require_id(&self.pool_id)?;
        require_id(&self.member_id)?;
        self.credential_version.validate()?;
        if self.scopes_hash.len() != 64
            || !self.scopes_hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(failure("invalid_authorization_identity", "授权范围指纹无效"));
        }
        Ok(())
    }
}
pub enum SignedAppLease<'a> {
    Baidu {
        app_id: &'a str,
        secret: &'a SecretText,
    },
    Youdao {
        app_key: &'a str,
        secret: &'a SecretText,
    },
}
/// Borrowed materials are not runtime permission: the gateway must still check current authority.
pub enum ProviderAuthLease<'a> {
    None,
    ApiKey {
        source: &'a CredentialVersionRef,
        key: &'a SecretText,
    },
    AppSecret {
        source: &'a CredentialVersionRef,
        material: SignedAppLease<'a>,
    },
    GoogleAccess {
        token: &'a SecretText,
        identity: &'a GoogleAuthorizationLeaseIdentity,
    },
    GoogleServiceAccountAccess {
        token: &'a SecretText,
        identity: &'a GoogleServiceAccountLeaseIdentity,
    },
}
impl fmt::Debug for SignedAppLease<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignedAppLease([REDACTED])")
    }
}
impl fmt::Debug for ProviderAuthLease<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProviderAuthLease([REDACTED])")
    }
}
