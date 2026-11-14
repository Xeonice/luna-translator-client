//! Fixed Google token transport. No configured endpoint, HTTP headers, raw error
//! body, private key or token is exposed through an ordinary application DTO.
use super::google_auth::{GOOGLE_TOKEN_ENDPOINT, GOOGLE_TRANSLATION_SCOPE};
use app_core::{
    contracts::{AppError, AppResult},
    domain::model_config::credentials::SecretText,
    ports::{
        google_authorization::{GoogleBudget, GoogleNetworkPolicy},
        runtime::PortFuture,
    },
};
use serde::Deserialize;
use std::{
    future::Future,
    sync::atomic::{Ordering, compiler_fence},
};

pub const GOOGLE_REVOKE_ENDPOINT: &str = "https://oauth2.googleapis.com/revoke";
pub const MAX_TOKEN_RESPONSE_BYTES: usize = 65_536;
pub const MAX_ACCESS_LIFETIME_SECONDS: u64 = 86_400;
fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "google_authorization", message)
}

pub async fn within_budget<T>(
    budget: &GoogleBudget,
    future: impl Future<Output = AppResult<T>>,
) -> AppResult<T> {
    budget.check()?;
    let result = tokio::select! {
        biased;
        _=budget.cancelled()=>Err(failure("authorization_cancelled","授权操作已取消")),
        _=tokio::time::sleep_until(tokio::time::Instant::from_std(budget.deadline()))=>Err(failure("authorization_timeout","授权操作已超时")),
        value=future=>value,
    };
    budget.check()?;
    result
}

pub enum GoogleTokenRequest<'a> {
    ServiceAccount {
        assertion: &'a SecretText,
    },
    Refresh {
        client_id: &'a str,
        refresh_token: &'a SecretText,
    },
    AuthorizationCode {
        client_id: &'a str,
        code: &'a SecretText,
        verifier: &'a SecretText,
        redirect_uri: &'a str,
    },
    Revoke {
        token: &'a SecretText,
    },
}
impl std::fmt::Debug for GoogleTokenRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleTokenRequest([REDACTED])")
    }
}
pub struct GoogleTokenResponse {
    pub(crate) access_token: SecretText,
    pub(crate) expires_in_seconds: u32,
    pub(crate) refresh_token: Option<SecretText>,
}
impl GoogleTokenResponse {
    pub fn expires_in_seconds(&self) -> u32 {
        self.expires_in_seconds
    }
    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token.is_some()
    }
}
impl std::fmt::Debug for GoogleTokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleTokenResponse([REDACTED])")
    }
}
pub enum GoogleTokenReply {
    Tokens(GoogleTokenResponse),
    Revoked,
}
impl std::fmt::Debug for GoogleTokenReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleTokenReply([REDACTED])")
    }
}
/// Controlled implementations are a test seam, not a production URL override.
/// Root composition must install FixedGoogleTokenTransport for real requests.
pub trait GoogleTokenTransport: Send + Sync {
    fn exchange<'a>(
        &'a self,
        request: GoogleTokenRequest<'a>,
        policy: &'a GoogleNetworkPolicy,
        budget: &'a GoogleBudget,
    ) -> PortFuture<'a, GoogleTokenReply>;
}
struct SensitiveResponse(Vec<u8>);
impl Drop for SensitiveResponse {
    fn drop(&mut self) {
        self.0.fill(0);
        std::hint::black_box(&mut self.0);
        compiler_fence(Ordering::SeqCst);
    }
}
#[derive(Deserialize)]
struct ErrorWire {
    error: Option<String>,
}
#[derive(Deserialize)]
struct TokenWire {
    access_token: SecretText,
    token_type: String,
    expires_in: u64,
    scope: Option<String>,
    refresh_token: Option<SecretText>,
}
fn token_error(bytes: &[u8]) -> AppResult<Option<AppError>> {
    let parsed: ErrorWire = serde_json::from_slice(bytes)
        .map_err(|_| failure("google_token_malformed", "授权服务返回无效数据"))?;
    Ok(parsed.error.map(|kind| {
        if kind == "invalid_grant" {
            failure("google_invalid_grant", "Google授权已失效，请重新授权")
        } else {
            failure("google_authorization_rejected", "Google拒绝本次授权请求")
        }
    }))
}
fn bearer_valid(token: &str) -> bool {
    !token.is_empty()
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~+/=".contains(&b))
}
/// Consumes and best-effort erases the bounded response. Serde's known-field
/// duplicate rejection is retained; unrelated future fields are ignored.
pub fn decode_google_token_response(
    status: u16,
    bytes: Vec<u8>,
    revoke: bool,
) -> AppResult<GoogleTokenReply> {
    let bytes = SensitiveResponse(bytes);
    if bytes.0.len() > MAX_TOKEN_RESPONSE_BYTES {
        return Err(failure(
            "google_token_response_too_large",
            "授权服务响应超过上限",
        ));
    }
    if revoke {
        if status == 200
            && (bytes.0.is_empty() || token_error(&bytes.0).is_ok_and(|error| error.is_none()))
        {
            return Ok(GoogleTokenReply::Revoked);
        }
        return Err(failure(
            "google_revoke_unconfirmed",
            "远端撤权尚未确认，本机授权保持停用",
        ));
    }
    if !bytes.0.is_empty()
        && let Some(error) = token_error(&bytes.0)? {
            return Err(error);
        }
    if status != 200 {
        return Err(failure(
            "google_authorization_rejected",
            "Google拒绝本次授权请求",
        ));
    }
    let wire: TokenWire = serde_json::from_slice(&bytes.0)
        .map_err(|_| failure("google_token_malformed", "授权服务返回无效数据"))?;
    if !wire.token_type.eq_ignore_ascii_case("Bearer") || !bearer_valid(wire.access_token.expose())
    {
        return Err(failure(
            "google_token_type_invalid",
            "授权服务返回的短期材料无效",
        ));
    }
    if wire.expires_in == 0 || wire.expires_in > MAX_ACCESS_LIFETIME_SECONDS {
        return Err(failure(
            "google_token_expiry_invalid",
            "授权有效时间不符合有界策略",
        ));
    }
    if wire.scope.as_ref().is_some_and(|scope| {
        scope.split_ascii_whitespace().collect::<Vec<_>>() != [GOOGLE_TRANSLATION_SCOPE]
    }) {
        return Err(failure("google_scope_mismatch", "Google返回的授权范围不符"));
    }
    if wire
        .refresh_token
        .as_ref()
        .is_some_and(|token| token.expose().bytes().any(|b| b.is_ascii_control()))
    {
        return Err(failure(
            "google_refresh_material_invalid",
            "Google刷新材料无效",
        ));
    }
    Ok(GoogleTokenReply::Tokens(GoogleTokenResponse {
        access_token: wire.access_token,
        expires_in_seconds: wire.expires_in as u32,
        refresh_token: wire.refresh_token,
    }))
}
fn public_client(client: &str) -> AppResult<()> {
    if client.is_empty()
        || client.len() > 512
        || client
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return Err(failure("invalid_google_client", "Google客户端身份无效"));
    }
    Ok(())
}
fn opaque_secret(secret: &SecretText) -> AppResult<()> {
    if secret.expose().bytes().any(|b| b.is_ascii_control()) {
        return Err(failure("invalid_google_secret", "授权材料含无效控制字符"));
    }
    Ok(())
}
pub fn validate_loopback_redirect(value: &str) -> AppResult<()> {
    let uri = reqwest::Url::parse(value)
        .map_err(|_| failure("invalid_google_redirect", "本机授权回调无效"))?;
    let suffix = uri
        .path()
        .strip_prefix("/oauth2/callback/")
        .ok_or_else(|| failure("invalid_google_redirect", "本机授权回调无效"))?;
    if uri.scheme() != "http"
        || uri.host_str() != Some("127.0.0.1")
        || uri.port().is_none_or(|p| p == 0)
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.query().is_some()
        || uri.fragment().is_some()
        || !(32..=128).contains(&suffix.len())
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(failure(
            "invalid_google_redirect",
            "只允许本次明确的loopback授权回调",
        ));
    }
    Ok(())
}
/// Only constants choose the destination. No application configuration can supply
/// a token/revoke URL, proxy header, client secret, or delegated subject.
pub fn validate_google_network_policy(policy: &GoogleNetworkPolicy) -> AppResult<()> {
    policy.validate()?;
    if let GoogleNetworkPolicy::Override { proxy_url } = policy {
        let url = reqwest::Url::parse(proxy_url)
            .map_err(|_| failure("invalid_google_proxy", "代理地址无效"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !matches!(url.path(), "" | "/")
            || url.port() == Some(0)
        {
            return Err(failure(
                "invalid_google_proxy",
                "仅支持无凭据和参数的HTTP(S)代理地址",
            ));
        }
    }
    Ok(())
}
pub struct FixedGoogleTokenTransport;
impl FixedGoogleTokenTransport {
    pub fn new() -> AppResult<Self> {
        Ok(Self)
    }
    fn client(policy: &GoogleNetworkPolicy, budget: &GoogleBudget) -> AppResult<reqwest::Client> {
        validate_google_network_policy(policy)?;
        budget.check()?;
        let remaining = budget
            .deadline()
            .saturating_duration_since(std::time::Instant::now());
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(remaining)
            .connect_timeout(remaining.min(std::time::Duration::from_secs(15)));
        match policy {
            GoogleNetworkPolicy::Inherit => {}
            GoogleNetworkPolicy::Disabled => builder = builder.no_proxy(),
            GoogleNetworkPolicy::Override { proxy_url } => {
                builder = builder.proxy(
                    reqwest::Proxy::all(proxy_url)
                        .map_err(|_| failure("invalid_google_proxy", "代理地址无效"))?,
                )
            }
        }
        builder
            .build()
            .map_err(|_| failure("google_transport_unavailable", "无法建立授权传输"))
    }
}
impl GoogleTokenTransport for FixedGoogleTokenTransport {
    fn exchange<'a>(
        &'a self,
        request: GoogleTokenRequest<'a>,
        policy: &'a GoogleNetworkPolicy,
        budget: &'a GoogleBudget,
    ) -> PortFuture<'a, GoogleTokenReply> {
        Box::pin(within_budget(budget, async move {
            validate_google_network_policy(policy)?;
            let (url, form, revoke) = match request {
                GoogleTokenRequest::ServiceAccount { assertion } => {
                    opaque_secret(assertion)?;
                    (
                        GOOGLE_TOKEN_ENDPOINT,
                        vec![
                            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                            ("assertion", assertion.expose()),
                        ],
                        false,
                    )
                }
                GoogleTokenRequest::Refresh {
                    client_id,
                    refresh_token,
                } => {
                    public_client(client_id)?;
                    opaque_secret(refresh_token)?;
                    (
                        GOOGLE_TOKEN_ENDPOINT,
                        vec![
                            ("grant_type", "refresh_token"),
                            ("client_id", client_id),
                            ("refresh_token", refresh_token.expose()),
                        ],
                        false,
                    )
                }
                GoogleTokenRequest::AuthorizationCode {
                    client_id,
                    code,
                    verifier,
                    redirect_uri,
                } => {
                    public_client(client_id)?;
                    opaque_secret(code)?;
                    validate_loopback_redirect(redirect_uri)?;
                    if !(43..=128).contains(&verifier.expose().len())
                        || !verifier
                            .expose()
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
                    {
                        return Err(failure("invalid_google_pkce", "本次PKCE校验材料无效"));
                    }
                    (
                        GOOGLE_TOKEN_ENDPOINT,
                        vec![
                            ("grant_type", "authorization_code"),
                            ("client_id", client_id),
                            ("code", code.expose()),
                            ("code_verifier", verifier.expose()),
                            ("redirect_uri", redirect_uri),
                        ],
                        false,
                    )
                }
                GoogleTokenRequest::Revoke { token } => {
                    opaque_secret(token)?;
                    (
                        GOOGLE_REVOKE_ENDPOINT,
                        vec![("token", token.expose())],
                        true,
                    )
                }
            };
            let mut response = Self::client(policy, budget)?
                .post(url)
                .header(reqwest::header::ACCEPT, "application/json")
                .form(&form)
                .send()
                .await
                .map_err(|_| failure("google_authorization_network", "授权服务连接失败"))?;
            if response
                .content_length()
                .is_some_and(|n| n > MAX_TOKEN_RESPONSE_BYTES as u64)
            {
                return Err(failure(
                    "google_token_response_too_large",
                    "授权服务响应超过上限",
                ));
            }
            let status = response.status().as_u16();
            let mut bytes = SensitiveResponse(Vec::new());
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| failure("google_authorization_network", "授权服务读取失败"))?
            {
                if bytes.0.len().saturating_add(chunk.len()) > MAX_TOKEN_RESPONSE_BYTES {
                    return Err(failure(
                        "google_token_response_too_large",
                        "授权服务响应超过上限",
                    ));
                }
                bytes.0.extend_from_slice(&chunk);
            }
            decode_google_token_response(status, std::mem::take(&mut bytes.0), revoke)
        }))
    }
}
