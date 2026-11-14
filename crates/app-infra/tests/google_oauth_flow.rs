//! Real loopback TCP with synthetic pending-login facts; no real SQL/Google grant.
use app_core::{domain::model_config::credentials::PrincipalState, ports::google_authorization::*};
use app_infra::providers::google_oauth::GoogleOAuthFlow;
use std::{collections::BTreeMap, num::NonZeroU32, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
fn pending() -> GooglePendingLogin {
    let key = GoogleClientKey {
        client_profile_id: "synthetic-client-profile".into(),
        client_profile_revision: NonZeroU32::new(1).unwrap(),
    };
    GooglePendingLogin {
        request: GoogleLoginRequest {
            operation_id: "synthetic-login".into(),
            client: key.clone(),
            profile_generation: NonZeroU32::new(1).unwrap(),
            server_instance_id: "synthetic-server".into(),
            network_policy: GoogleNetworkPolicy::Inherit,
        },
        grant: GoogleOwnedGrant {
            cursor: GoogleGrantCursor {
                grant_id: "synthetic-grant".into(),
                authorization_epoch: None,
                material_revision: None,
                control_revision: 0,
            },
            client: GoogleClientProfile {
                key,
                public_client_id: "synthetic-desktop-client".into(),
                client_mode: GoogleClientMode::Custom,
                policy_sha256: "a".repeat(64),
                setup_state: GoogleClientSetup::Verified,
            },
            state: GoogleGrantState::Pending,
            scopes_sha256: google_scope_hash(),
            principal_state: PrincipalState::Unknown,
            material: None,
        },
    }
}
fn callback(url: &str) -> (reqwest::Url, String) {
    let uri = reqwest::Url::parse(url).unwrap();
    assert_eq!(uri.host_str(), Some("accounts.google.com"));
    let values: BTreeMap<_, _> = uri.query_pairs().into_owned().collect();
    assert_eq!(values["scope"], GOOGLE_TRANSLATION_SCOPE);
    assert_eq!(values["code_challenge_method"], "S256");
    assert_eq!(values["code_challenge"].len(), 43);
    assert!(!values.contains_key("client_secret"));
    (
        reqwest::Url::parse(&values["redirect_uri"]).unwrap(),
        values["state"].clone(),
    )
}
async fn send(uri: &reqwest::Url, query: &str, host: Option<&str>) -> String {
    let mut socket = TcpStream::connect(("127.0.0.1", uri.port().unwrap()))
        .await
        .unwrap();
    let normal = format!("127.0.0.1:{}", uri.port().unwrap());
    let host = host.unwrap_or(&normal);
    let req = format!(
        "GET {}?{query} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n",
        uri.path()
    );
    socket.write_all(req.as_bytes()).await.unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    response
}
#[tokio::test]
async fn actual_loopback_accepts_once_and_never_echoes_callback_secrets() {
    let flow = GoogleOAuthFlow::bind(
        pending(),
        GoogleBudget::new(Duration::from_secs(3)).unwrap(),
    )
    .await
    .unwrap();
    let (uri, state) = callback(flow.intent().authorization_url());
    let handle = tokio::spawn(flow.wait_for_callback());
    let response = send(
        &uri,
        &format!("state={state}&code=synthetic-secret-code"),
        None,
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(!response.contains(&state));
    assert!(!response.contains("synthetic-secret-code"));
    assert!(response.contains("no-store"));
    let callback = handle.await.unwrap().unwrap();
    assert!(!format!("{callback:?}").contains("synthetic-secret-code"));
    assert!(
        TcpStream::connect(("127.0.0.1", uri.port().unwrap()))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn wrong_state_host_duplicate_and_error_mix_do_not_complete_the_flow() {
    let flow = GoogleOAuthFlow::bind(
        pending(),
        GoogleBudget::new(Duration::from_secs(4)).unwrap(),
    )
    .await
    .unwrap();
    let (uri, state) = callback(flow.intent().authorization_url());
    let handle = tokio::spawn(flow.wait_for_callback());
    for (query, host) in [
        ("state=wrong&code=synthetic".into(), None),
        (
            format!("state={state}&code=synthetic"),
            Some("example.invalid"),
        ),
        (format!("state={state}&state={state}&code=synthetic"), None),
        (format!("state={state}&code=synthetic&error=denied"), None),
    ] {
        assert!(send(&uri, &query, host).await.starts_with("HTTP/1.1 400"));
        assert!(!handle.is_finished());
    }
    assert!(
        send(
            &uri,
            &format!("state={state}&code=synthetic-accepted"),
            None
        )
        .await
        .starts_with("HTTP/1.1 200")
    );
    handle.await.unwrap().unwrap();
}
#[tokio::test]
async fn valid_denial_finishes_without_leaking_provider_error_description() {
    let flow = GoogleOAuthFlow::bind(
        pending(),
        GoogleBudget::new(Duration::from_secs(3)).unwrap(),
    )
    .await
    .unwrap();
    let (uri, state) = callback(flow.intent().authorization_url());
    let handle = tokio::spawn(flow.wait_for_callback());
    let response = send(
        &uri,
        &format!("state={state}&error=access_denied&error_description=synthetic-private-error"),
        None,
    )
    .await;
    assert!(!response.contains("synthetic-private-error"));
    let error = handle.await.unwrap().unwrap_err();
    assert_eq!(error.code, "google_authorization_declined");
    assert!(!format!("{error:?}").contains("synthetic-private-error"));
}
#[tokio::test]
async fn timeout_and_explicit_cancel_close_the_actual_listener() {
    let budget = GoogleBudget::new(Duration::from_millis(40)).unwrap();
    let flow = GoogleOAuthFlow::bind(pending(), budget).await.unwrap();
    let (uri, _) = callback(flow.intent().authorization_url());
    assert_eq!(
        flow.wait_for_callback().await.unwrap_err().code,
        "authorization_timeout"
    );
    assert!(
        TcpStream::connect(("127.0.0.1", uri.port().unwrap()))
            .await
            .is_err()
    );
    let flow = GoogleOAuthFlow::bind(
        pending(),
        GoogleBudget::new(Duration::from_secs(2)).unwrap(),
    )
    .await
    .unwrap();
    let (uri, _) = callback(flow.intent().authorization_url());
    flow.cancel();
    assert!(
        TcpStream::connect(("127.0.0.1", uri.port().unwrap()))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn new_flows_have_unique_pkce_state_path_and_bad_client_never_starts() {
    let a = GoogleOAuthFlow::bind(
        pending(),
        GoogleBudget::new(Duration::from_secs(2)).unwrap(),
    )
    .await
    .unwrap();
    let b = GoogleOAuthFlow::bind(
        pending(),
        GoogleBudget::new(Duration::from_secs(2)).unwrap(),
    )
    .await
    .unwrap();
    let (ua, sa) = callback(a.intent().authorization_url());
    let (ub, sb) = callback(b.intent().authorization_url());
    assert_ne!(sa, sb);
    assert_ne!(ua.path(), ub.path());
    assert_ne!(a.intent().session_id(), b.intent().session_id());
    let mut bad = pending();
    bad.grant.client.setup_state = GoogleClientSetup::Unknown;
    assert_eq!(
        GoogleOAuthFlow::bind(bad, GoogleBudget::new(Duration::from_secs(1)).unwrap())
            .await
            .unwrap_err()
            .code,
        "google_client_setup_unverified"
    );
    let mut bad = pending();
    bad.grant.state = GoogleGrantState::Active;
    assert!(
        GoogleOAuthFlow::bind(bad, GoogleBudget::new(Duration::from_secs(1)).unwrap())
            .await
            .is_err()
    );
}
