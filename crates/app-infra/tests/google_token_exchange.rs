//! Synthetic protocol boundary checks only. No Google account or remote call.
use app_core::{
    domain::model_config::credentials::SecretText,
    ports::google_authorization::{GoogleBudget, GoogleNetworkPolicy},
};
use app_infra::providers::google_token::*;
use serde_json::{Value, json};
use std::time::Duration;
fn body() -> Value {
    json!({"access_token":"synthetic.access-token","token_type":"Bearer","expires_in":3600,"scope":"https://www.googleapis.com/auth/cloud-translation"})
}
fn decode(value: &Value) -> Result<GoogleTokenReply, app_core::contracts::AppError> {
    decode_google_token_response(200, serde_json::to_vec(value).unwrap(), false)
}
#[test]
fn bounded_token_decode_accepts_fixed_scope_and_ignores_unrelated_future_fields() {
    let mut value = body();
    value["future_metadata"] = json!({"unused":true});
    let GoogleTokenReply::Tokens(token) = decode(&value).unwrap() else {
        panic!("not token")
    };
    assert_eq!(token.expires_in_seconds(), 3600);
    assert!(!token.has_refresh_token());
    assert!(!format!("{token:?}").contains("synthetic.access-token"));
    value.as_object_mut().unwrap().remove("scope");
    value["refresh_token"] = json!("synthetic-refresh");
    let GoogleTokenReply::Tokens(token) = decode(&value).unwrap() else {
        panic!("not token")
    };
    assert!(token.has_refresh_token());
}
#[test]
fn expiry_types_ranges_scopes_and_header_unsafe_materials_are_rejected() {
    for expiry in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(true),
        json!("3600"),
        json!(86401),
        json!(u64::MAX),
    ] {
        let mut value = body();
        value["expires_in"] = expiry;
        assert!(decode(&value).is_err());
    }
    for scope in [
        "",
        "https://www.googleapis.com/auth/cloud-platform",
        "https://www.googleapis.com/auth/cloud-translation openid",
    ] {
        let mut value = body();
        value["scope"] = json!(scope);
        assert_eq!(decode(&value).unwrap_err().code, "google_scope_mismatch");
    }
    for token in ["", "bad\nheader", "has space", "unicode語", "contains\0nul"] {
        let mut value = body();
        value["access_token"] = json!(token);
        assert!(decode(&value).is_err());
    }
    let mut value = body();
    value["token_type"] = json!("DPoP");
    assert!(decode(&value).is_err());
}
#[test]
fn every_sensitive_duplicate_field_is_rejected_without_echoing_body() {
    let original = serde_json::to_string(&body()).unwrap();
    for (key, value) in [
        ("access_token", "\"synthetic-secret-never-log\""),
        ("expires_in", "3600"),
        ("token_type", "\"Bearer\""),
        (
            "scope",
            "\"https://www.googleapis.com/auth/cloud-translation\"",
        ),
    ] {
        let duplicate = original.replacen('{', &format!("{{\"{key}\":{value},"), 1);
        let error = decode_google_token_response(200, duplicate.into_bytes(), false).unwrap_err();
        assert!(!format!("{error:?}").contains("synthetic-secret-never-log"));
    }
    for bytes in [
        br#"{"error":"invalid_grant","error":"private-body"}"#.to_vec(),
        b"not-json synthetic-private-body".to_vec(),
        vec![b'x'; MAX_TOKEN_RESPONSE_BYTES + 1],
    ] {
        let error = decode_google_token_response(200, bytes, false).unwrap_err();
        assert!(!format!("{error:?}").contains("private-body"));
    }
}
#[test]
fn invalid_grant_and_revoke_outcomes_have_no_raw_error_leak_or_false_success() {
    let bytes =
        br#"{"error":"invalid_grant","error_description":"synthetic-refresh-must-not-leak"}"#
            .to_vec();
    let error = decode_google_token_response(400, bytes, false).unwrap_err();
    assert_eq!(error.code, "google_invalid_grant");
    assert!(!format!("{error:?}").contains("must-not-leak"));
    assert!(matches!(
        decode_google_token_response(200, vec![], true).unwrap(),
        GoogleTokenReply::Revoked
    ));
    for (status, bytes) in [
        (400, vec![]),
        (302, b"redirect".to_vec()),
        (200, br#"{"error":"denied"}"#.to_vec()),
        (200, b"not-json".to_vec()),
    ] {
        assert_eq!(
            decode_google_token_response(status, bytes, true)
                .unwrap_err()
                .code,
            "google_revoke_unconfirmed"
        );
    }
}
#[test]
fn callback_url_cannot_redirect_code_verifier_to_any_other_origin() {
    let path = "a".repeat(43);
    let valid = format!("http://127.0.0.1:43127/oauth2/callback/{path}");
    assert!(validate_loopback_redirect(&valid).is_ok());
    for value in [
        valid.replace("127.0.0.1", "localhost"),
        valid.replace("127.0.0.1", "example.invalid"),
        valid.replace("http:", "https:"),
        format!("{valid}?q=x"),
        format!("{valid}#fragment"),
        valid.replace(":43127", ":0"),
        valid.replace("/oauth2/", "/other/"),
    ] {
        assert!(validate_loopback_redirect(&value).is_err(), "{value}");
    }
}
#[tokio::test]
async fn invalid_pkce_or_redirect_is_refused_before_any_network() {
    let transport = FixedGoogleTokenTransport::new().unwrap();
    let budget = GoogleBudget::new(Duration::from_secs(1)).unwrap();
    let code = SecretText::from_bytes(b"synthetic-code".to_vec()).unwrap();
    let verifier = SecretText::from_bytes(b"short".to_vec()).unwrap();
    let redirect = format!("http://127.0.0.1:43127/oauth2/callback/{}", "a".repeat(43));
    assert_eq!(
        transport
            .exchange(
                GoogleTokenRequest::AuthorizationCode {
                    client_id: "synthetic-client",
                    code: &code,
                    verifier: &verifier,
                    redirect_uri: &redirect
                },
                &GoogleNetworkPolicy::Disabled,
                &budget
            )
            .await
            .unwrap_err()
            .code,
        "invalid_google_pkce"
    );
    assert_eq!(
        transport
            .exchange(
                GoogleTokenRequest::AuthorizationCode {
                    client_id: "synthetic-client",
                    code: &code,
                    verifier: &verifier,
                    redirect_uri: "https://example.invalid/steal"
                },
                &GoogleNetworkPolicy::Disabled,
                &budget
            )
            .await
            .unwrap_err()
            .code,
        "invalid_google_redirect"
    );
}
#[tokio::test]
async fn cancellation_and_deadline_drop_the_operation_future() {
    let budget = GoogleBudget::new(Duration::from_millis(20)).unwrap();
    assert_eq!(
        within_budget::<()>(&budget, async { std::future::pending().await })
            .await
            .unwrap_err()
            .code,
        "authorization_timeout"
    );
    let budget = GoogleBudget::new(Duration::from_secs(1)).unwrap();
    budget.cancel();
    assert_eq!(
        within_budget::<()>(&budget, async {
            panic!("cancelled future must not execute")
        })
        .await
        .unwrap_err()
        .code,
        "authorization_cancelled"
    );
}

#[test]
fn proxy_policy_accepts_only_supported_credential_free_http_routes() {
    for value in [
        "http://127.0.0.1:8080",
        "https://proxy.example.invalid/",
        "http://[::1]:3128",
    ] {
        validate_google_network_policy(&GoogleNetworkPolicy::Override {
            proxy_url: value.into(),
        })
        .unwrap();
    }
    for value in [
        "socks5://127.0.0.1:1080",
        "http://user:synthetic-secret@proxy.invalid",
        "http://proxy.invalid?q=synthetic-secret",
        "http://proxy.invalid#secret",
        "http://proxy.invalid/path",
        "http://127.0.0.1:0",
        "http://127.0.0.1:70000",
        "http:///",
        "http://proxy.invalid\\other",
        "http://proxy.invalid/\n",
    ] {
        let policy = GoogleNetworkPolicy::Override {
            proxy_url: value.into(),
        };
        let error = validate_google_network_policy(&policy).unwrap_err();
        assert_eq!(error.code, "invalid_google_proxy");
        assert!(!format!("{error:?} {policy:?}").contains("synthetic-secret"));
    }
    assert!(GoogleNetworkPolicy::from_connection("override", &Default::default()).is_err());
    assert!(GoogleNetworkPolicy::from_connection("unsupported", &Default::default()).is_err());
}

#[tokio::test]
async fn actual_local_proxy_observes_only_fixed_google_connect_before_tls() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = GoogleNetworkPolicy::Override {
        proxy_url: format!("http://{}", listener.local_addr().unwrap()),
    };
    let local_proxy = async {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut part = [0; 512];
            let n = stream.read(&mut part).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&part[..n]);
            assert!(bytes.len() < 8192);
        }
        stream
            .write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        String::from_utf8(bytes).unwrap()
    };
    let secret = SecretText::from_bytes(b"synthetic-refresh-never-in-connect".to_vec()).unwrap();
    let transport = FixedGoogleTokenTransport::new().unwrap();
    let budget = GoogleBudget::new(Duration::from_secs(3)).unwrap();
    let (headers, outcome) = tokio::time::timeout(Duration::from_secs(4), async {
        tokio::join!(
            local_proxy,
            transport.exchange(
                GoogleTokenRequest::Refresh {
                    client_id: "synthetic-desktop-client",
                    refresh_token: &secret
                },
                &proxy,
                &budget
            )
        )
    })
    .await
    .unwrap();
    assert!(
        headers.starts_with("CONNECT oauth2.googleapis.com:443 HTTP/1.1\r\n"),
        "{headers}"
    );
    assert!(!headers.contains("synthetic"));
    assert!(!headers.to_ascii_lowercase().contains("authorization:"));
    let error = outcome.unwrap_err();
    assert_eq!(error.code, "google_authorization_network");
    assert!(!format!("{error:?}").contains(secret.expose()));
}
