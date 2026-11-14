//! Secret-only Google service-account import and assertion signing.
//! This leaf never selects an endpoint from imported JSON and does not imply a live grant.
use app_core::{
    contracts::{AppError, AppResult},
    domain::model_config::credentials::{CredentialMaterial, MAX_SECRET_BYTES, SecretText},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ring::{rand::SystemRandom, rsa::KeyPair, signature};
use serde::Deserialize;
use serde_json::json;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

pub const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
pub const GOOGLE_TRANSLATION_SCOPE: &str = "https://www.googleapis.com/auth/cloud-translation";
pub const MAX_SERVICE_ACCOUNT_FILE_BYTES: usize = 65_536;
const ASSERTION_LIFETIME_SECONDS: u64 = 3600;

fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "google_authorization", message)
}

// All buffers owned by this module are wiped on every return path. Dependencies/OS may copy.
struct SensitiveBytes(Vec<u8>);
impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        self.0.fill(0);
        std::hint::black_box(&mut self.0);
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServiceAccountFile {
    #[serde(rename = "type")]
    kind: String,
    client_email: String,
    private_key: SecretText,
    private_key_id: Option<String>,
    project_id: Option<String>,
    token_uri: String,
    // Standard download fields are parsed but never treated as routing or authority inputs.
    #[serde(default, rename = "client_id")]
    _client_id: Option<String>,
    #[serde(default, rename = "auth_uri")]
    _auth_uri: Option<String>,
    #[serde(default, rename = "auth_provider_x509_cert_url")]
    _auth_provider_x509_cert_url: Option<String>,
    #[serde(default, rename = "client_x509_cert_url")]
    _client_x509_cert_url: Option<String>,
    #[serde(default, rename = "universe_domain")]
    universe_domain: Option<String>,
}

fn public_identifier(value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.len() > 512
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(failure("invalid_service_account", "服务账号标识无效"));
    }
    Ok(())
}

fn validate_subject(email: &str, key_id: Option<&str>, project_id: Option<&str>) -> AppResult<()> {
    public_identifier(email)?;
    let mut parts = email.split('@');
    if parts.next().is_none_or(str::is_empty)
        || parts.next().is_none_or(str::is_empty)
        || parts.next().is_some()
    {
        return Err(failure("invalid_service_account", "服务账号主体格式无效"));
    }
    for value in [key_id, project_id].into_iter().flatten() {
        public_identifier(value)?;
    }
    Ok(())
}

fn rsa_key(pem: &SecretText) -> AppResult<KeyPair> {
    let payload = pem
        .expose()
        .trim()
        .strip_prefix("-----BEGIN PRIVATE KEY-----")
        .and_then(|body| body.strip_suffix("-----END PRIVATE KEY-----"))
        .ok_or_else(|| {
            failure(
                "invalid_service_account_key",
                "服务账号需要有效RSA PKCS8私钥",
            )
        })?;
    if payload.bytes().any(|b| {
        !b.is_ascii_alphanumeric() && !matches!(b, b'+' | b'/' | b'=') && !b.is_ascii_whitespace()
    }) {
        return Err(failure(
            "invalid_service_account_key",
            "服务账号私钥编码无效",
        ));
    }
    let encoded = SensitiveBytes(
        payload
            .bytes()
            .filter(|b| !b.is_ascii_whitespace())
            .collect(),
    );
    let der = SensitiveBytes(
        STANDARD
            .decode(&encoded.0)
            .map_err(|_| failure("invalid_service_account_key", "服务账号私钥编码无效"))?,
    );
    // ring rejects non-RSA, malformed, insecure or unsupported modulus/private parameters.
    KeyPair::from_pkcs8(&der.0)
        .map_err(|_| failure("invalid_service_account_key", "服务账号RSA私钥不可用"))
}

/// A bounded reader is used before JSON parsing; a huge file is never fully allocated.
/// Imported private material has no ordinary Serialize or Debug projection.
pub fn import_service_account_owned(bytes: Vec<u8>) -> AppResult<CredentialMaterial> {
    let bytes = SensitiveBytes(bytes);
    import_service_account(bytes.0.as_slice())
}
pub fn import_service_account(reader: impl Read) -> AppResult<CredentialMaterial> {
    let mut bytes = SensitiveBytes(Vec::new());
    reader
        .take((MAX_SERVICE_ACCOUNT_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes.0)
        .map_err(|_| failure("service_account_read_failed", "无法读取服务账号文件"))?;
    if bytes.0.is_empty() || bytes.0.len() > MAX_SERVICE_ACCOUNT_FILE_BYTES {
        return Err(failure(
            "service_account_file_size",
            "服务账号文件为空或超过上限",
        ));
    }
    let file: ServiceAccountFile = serde_json::from_slice(&bytes.0)
        .map_err(|_| failure("invalid_service_account", "服务账号文件结构或字段无效"))?;
    if file.kind != "service_account"
        || file.token_uri != GOOGLE_TOKEN_ENDPOINT
        || file
            .universe_domain
            .as_deref()
            .is_some_and(|domain| domain != "googleapis.com")
    {
        return Err(failure(
            "unsupported_service_account",
            "仅支持标准Google服务账号认证",
        ));
    }
    validate_subject(
        &file.client_email,
        file.private_key_id.as_deref(),
        file.project_id.as_deref(),
    )?;
    rsa_key(&file.private_key)?;
    Ok(CredentialMaterial::GoogleServiceAccount {
        client_email: file.client_email,
        private_key: file.private_key,
        private_key_id: file.private_key_id,
        project_id: file.project_id,
    })
}

/// Produces a fixed RS256 assertion. Neither delegation, arbitrary scopes nor audience is exposed.
/// The caller must separately prove the current SQL/Vault authorization before exchanging it.
pub fn service_account_assertion(
    material: &CredentialMaterial,
    now: SystemTime,
) -> AppResult<SecretText> {
    let CredentialMaterial::GoogleServiceAccount {
        client_email,
        private_key,
        private_key_id,
        project_id,
    } = material
    else {
        return Err(failure("credential_kind_mismatch", "需要服务账号秘密材料"));
    };
    validate_subject(
        client_email,
        private_key_id.as_deref(),
        project_id.as_deref(),
    )?;
    let issued_at = now
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|time| time.as_secs())
        .filter(|seconds| *seconds > 0)
        .ok_or_else(|| failure("authorization_clock_invalid", "系统时间不能用于授权"))?;
    let expires_at = issued_at
        .checked_add(ASSERTION_LIFETIME_SECONDS)
        .filter(|seconds| *seconds <= 253_402_300_799)
        .ok_or_else(|| failure("authorization_clock_invalid", "系统时间超出授权范围"))?;
    let key = rsa_key(private_key)?;
    let mut header = json!({"alg": "RS256", "typ": "JWT"});
    if let Some(key_id) = private_key_id {
        header["kid"] = json!(key_id);
    }
    let claims = json!({"iss": client_email, "scope": GOOGLE_TRANSLATION_SCOPE,
        "aud": GOOGLE_TOKEN_ENDPOINT, "iat": issued_at, "exp": expires_at});
    let header = serde_json::to_vec(&header)
        .map_err(|_| failure("assertion_encoding", "授权请求编码失败"))?;
    let claims = serde_json::to_vec(&claims)
        .map_err(|_| failure("assertion_encoding", "授权请求编码失败"))?;
    let mut message = SensitiveBytes(
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header),
            URL_SAFE_NO_PAD.encode(claims)
        )
        .into_bytes(),
    );
    let mut signature = SensitiveBytes(vec![0; key.public().modulus_len()]);
    key.sign(
        &signature::RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        &message.0,
        &mut signature.0,
    )
    .map_err(|_| failure("assertion_signing_failed", "服务账号签名失败"))?;
    message.0.push(b'.');
    message
        .0
        .extend_from_slice(URL_SAFE_NO_PAD.encode(&signature.0).as_bytes());
    if message.0.len() > MAX_SECRET_BYTES {
        return Err(failure("assertion_too_large", "授权请求超过上限"));
    }
    SecretText::from_bytes(std::mem::take(&mut message.0))
}
