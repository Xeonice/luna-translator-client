//! Operation-owned OAuth loopback. This module returns a native browser intent;
//! it never opens a shell/browser, writes SQL, or exposes callback secrets to UI.
use super::google_token::{validate_loopback_redirect, within_budget};
use app_core::{
    contracts::{AppError, AppResult},
    domain::model_config::credentials::SecretText,
    ports::google_authorization::{GOOGLE_TRANSLATION_SCOPE, GoogleBudget, GooglePendingLogin},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, SocketAddrV4},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const MAX_CALLBACK_BYTES: usize = 8192;
const MAX_CALLBACK_CONNECTIONS: usize = 16;
const MAX_LOGIN_DURATION: Duration = Duration::from_secs(300);
fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "google_authorization", message)
}
fn random_urlsafe(bytes: usize) -> AppResult<String> {
    let mut value = vec![0; bytes];
    SystemRandom::new()
        .fill(&mut value)
        .map_err(|_| failure("google_random_unavailable", "无法建立本次安全授权会话"))?;
    let encoded = URL_SAFE_NO_PAD.encode(&value);
    value.fill(0);
    Ok(encoded)
}
fn same_state(left: &str, right: &str) -> bool {
    let mut difference = left.len() ^ right.len();
    for i in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.as_bytes().get(i).copied().unwrap_or(0)
                ^ right.as_bytes().get(i).copied().unwrap_or(0),
        );
    }
    std::hint::black_box(difference) == 0
}
#[derive(Clone)]
pub struct GoogleBrowserIntent {
    authorization_url: String,
    session_id: String,
}
impl GoogleBrowserIntent {
    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}
impl std::fmt::Debug for GoogleBrowserIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleBrowserIntent([native-only])")
    }
}
pub struct GoogleOAuthCallback {
    pub(crate) pending: GooglePendingLogin,
    pub(crate) code: SecretText,
    pub(crate) verifier: SecretText,
    pub(crate) redirect_uri: String,
    pub(crate) budget: GoogleBudget,
}
impl std::fmt::Debug for GoogleOAuthCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleOAuthCallback([REDACTED])")
    }
}
pub struct GoogleOAuthFlow {
    listener: TcpListener,
    pending: GooglePendingLogin,
    budget: GoogleBudget,
    started: Instant,
    state: SecretText,
    verifier: SecretText,
    redirect_uri: String,
    path: String,
    intent: GoogleBrowserIntent,
}
impl std::fmt::Debug for GoogleOAuthFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GoogleOAuthFlow([operation-owned])")
    }
}
struct SensitiveRequest(Vec<u8>);
impl Drop for SensitiveRequest {
    fn drop(&mut self) {
        self.0.fill(0);
        std::hint::black_box(&mut self.0);
    }
}
impl GoogleOAuthFlow {
    pub async fn bind(pending: GooglePendingLogin, budget: GoogleBudget) -> AppResult<Self> {
        pending.validate()?;
        budget.check()?;
        let listener = within_budget(&budget, async {
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
                .await
                .map_err(|_| failure("google_loopback_unavailable", "无法建立本机授权回调"))
        })
        .await?;
        let port = listener
            .local_addr()
            .map_err(|_| failure("google_loopback_unavailable", "无法核对本机授权回调"))?
            .port();
        let state = SecretText::from_bytes(random_urlsafe(32)?.into_bytes())?;
        let verifier = SecretText::from_bytes(random_urlsafe(64)?.into_bytes())?;
        let path = format!("/oauth2/callback/{}", random_urlsafe(32)?);
        let redirect_uri = format!("http://127.0.0.1:{port}{path}");
        validate_loopback_redirect(&redirect_uri)?;
        let challenge = URL_SAFE_NO_PAD
            .encode(digest::digest(&digest::SHA256, verifier.expose().as_bytes()).as_ref());
        let mut url = reqwest::Url::parse(AUTHORIZATION_ENDPOINT)
            .map_err(|_| failure("google_authorization_url_invalid", "授权入口无效"))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &pending.grant.client.public_client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", GOOGLE_TRANSLATION_SCOPE)
            .append_pair("state", state.expose())
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
        let intent = GoogleBrowserIntent {
            authorization_url: url.into(),
            session_id: random_urlsafe(24)?,
        };
        Ok(Self {
            listener,
            pending,
            budget,
            started: Instant::now(),
            state,
            verifier,
            redirect_uri,
            path,
            intent,
        })
    }
    pub fn intent(&self) -> &GoogleBrowserIntent {
        &self.intent
    }
    pub fn cancel(self) {
        self.budget.cancel();
    }
    pub async fn wait_for_callback(self) -> AppResult<GoogleOAuthCallback> {
        let budget = self.budget.clone();
        let deadline = tokio::time::Instant::from_std(self.started + MAX_LOGIN_DURATION);
        within_budget(&budget, async move {
            tokio::time::timeout_at(deadline, self.read_callback_connections())
                .await
                .map_err(|_| failure("authorization_timeout", "本次登录已超时"))?
        })
        .await
    }
    async fn read_callback_connections(self) -> AppResult<GoogleOAuthCallback> {
        let port = self
            .listener
            .local_addr()
            .map_err(|_| failure("google_loopback_unavailable", "本机授权回调已关闭"))?
            .port();
        for _ in 0..MAX_CALLBACK_CONNECTIONS {
            let (mut stream, peer) = self
                .listener
                .accept()
                .await
                .map_err(|_| failure("google_loopback_unavailable", "本机授权回调已关闭"))?;
            if !peer.ip().is_loopback() {
                continue;
            }
            let parsed = match tokio::time::timeout(
                Duration::from_secs(2),
                read_callback(&mut stream, &self.path, port, self.state.expose()),
            )
            .await
            {
                Ok(value) => value,
                Err(_) => Err(failure("google_callback_invalid", "本机回调读取超时")),
            };
            match parsed {
                Ok(CallbackResult::Code(code)) => {
                    reply(&mut stream, true).await;
                    return Ok(GoogleOAuthCallback {
                        pending: self.pending,
                        code,
                        verifier: self.verifier,
                        redirect_uri: self.redirect_uri,
                        budget: self.budget,
                    });
                }
                Ok(CallbackResult::Declined) => {
                    reply(&mut stream, false).await;
                    return Err(failure(
                        "google_authorization_declined",
                        "本次Google授权未获同意",
                    ));
                }
                Err(_) => reply(&mut stream, false).await,
            }
        }
        Err(failure(
            "google_callback_limit",
            "本次授权回调无效次数过多，请重试",
        ))
    }
}
enum CallbackResult {
    Code(SecretText),
    Declined,
}
async fn reply(stream: &mut TcpStream, ok: bool) {
    let body = if ok {
        "Authorization received. Return to Luna. You can close this window."
    } else {
        "Authorization callback was not accepted. Return to Luna."
    };
    let status = if ok { "200 OK" } else { "400 Bad Request" };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = tokio::time::timeout(Duration::from_millis(200), async {
        stream.write_all(response.as_bytes()).await?;
        stream.shutdown().await
    })
    .await;
}
async fn read_callback(
    stream: &mut TcpStream,
    path: &str,
    port: u16,
    state: &str,
) -> AppResult<CallbackResult> {
    let mut bytes = SensitiveRequest(Vec::new());
    let header_end;
    loop {
        let mut chunk = [0; 1024];
        let count = stream
            .read(&mut chunk)
            .await
            .map_err(|_| failure("google_callback_invalid", "本机回调读取失败"))?;
        if count == 0 {
            return Err(failure("google_callback_invalid", "本机回调不完整"));
        }
        if bytes.0.len().saturating_add(count) > MAX_CALLBACK_BYTES {
            return Err(failure("google_callback_invalid", "本机回调超过上限"));
        }
        bytes.0.extend_from_slice(&chunk[..count]);
        chunk.fill(0);
        if let Some(end) = bytes.0.windows(4).position(|v| v == b"\r\n\r\n") {
            header_end = end;
            break;
        }
    }
    let text = std::str::from_utf8(&bytes.0[..header_end])
        .map_err(|_| failure("google_callback_invalid", "本机回调编码无效"))?;
    let mut lines = text.split("\r\n");
    let first = lines
        .next()
        .unwrap_or_default()
        .split(' ')
        .collect::<Vec<_>>();
    if first.len() != 3
        || first[0] != "GET"
        || first[2] != "HTTP/1.1"
        || !first[1].starts_with('/')
        || first[1].starts_with("//")
        || first[1].contains('#')
    {
        return Err(failure("google_callback_invalid", "本机回调请求无效"));
    }
    let mut hosts = Vec::new();
    let mut count = 0;
    for line in lines {
        count += 1;
        if count > 32 {
            return Err(failure("google_callback_invalid", "本机回调头部过多"));
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| failure("google_callback_invalid", "本机回调头部无效"))?;
        if key.eq_ignore_ascii_case("host") {
            hosts.push(value.trim());
        }
        if key.eq_ignore_ascii_case("transfer-encoding")
            || (key.eq_ignore_ascii_case("content-length") && value.trim() != "0")
        {
            return Err(failure("google_callback_invalid", "本机回调不能含请求正文"));
        }
    }
    let expected_host = format!("127.0.0.1:{port}");
    if hosts != [expected_host.as_str()] {
        return Err(failure("google_callback_invalid", "本机回调Host不符"));
    }
    let uri = reqwest::Url::parse(&format!("http://127.0.0.1:{port}{}", first[1]))
        .map_err(|_| failure("google_callback_invalid", "本机回调地址无效"))?;
    if uri.path() != path || uri.host_str() != Some("127.0.0.1") {
        return Err(failure("google_callback_invalid", "本机回调路径不符"));
    }
    let mut query = BTreeMap::new();
    for (key, value) in uri.query_pairs() {
        if matches!(key.as_ref(), "state" | "code" | "error")
            && query.insert(key.into_owned(), value.into_owned()).is_some()
        {
            return Err(failure(
                "google_callback_invalid",
                "本机回调包含重复身份字段",
            ));
        }
    }
    if !query
        .get("state")
        .is_some_and(|actual| same_state(actual, state))
    {
        return Err(failure("google_callback_invalid", "本机回调state不符"));
    }
    match (query.remove("code"), query.remove("error")) {
        (Some(code), None) => {
            if code.bytes().any(|b| b.is_ascii_control()) {
                return Err(failure("google_callback_invalid", "授权代码无效"));
            }
            Ok(CallbackResult::Code(SecretText::from_bytes(
                code.into_bytes(),
            )?))
        }
        (None, Some(_)) => Ok(CallbackResult::Declined),
        _ => Err(failure(
            "google_callback_invalid",
            "本机回调没有唯一授权结果",
        )),
    }
}
