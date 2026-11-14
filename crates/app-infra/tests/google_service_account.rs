//! All private material here is deliberately synthetic and has no remote account authority.
use app_core::domain::model_config::credentials::{CredentialMaterial, SecretText};
use app_infra::providers::google_auth::{
    GOOGLE_TOKEN_ENDPOINT, GOOGLE_TRANSLATION_SCOPE, MAX_SERVICE_ACCOUNT_FILE_BYTES,
    import_service_account, service_account_assertion,
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ring::{
    rsa::KeyPair,
    signature::{self, KeyPair as _},
};
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    time::{Duration, UNIX_EPOCH},
};

const KEY: &str = include_str!("fixtures/google-synthetic-only-private.pem");
fn file() -> Value {
    json!({"type":"service_account", "client_email":"synthetic@example.iam.gserviceaccount.com",
        "private_key":KEY, "private_key_id":"synthetic-test-only", "project_id":"synthetic-project",
        "token_uri": GOOGLE_TOKEN_ENDPOINT, "universe_domain":"googleapis.com",
        "client_id":"synthetic", "auth_uri":"https://accounts.google.com/o/oauth2/auth",
        "auth_provider_x509_cert_url":"https://www.googleapis.com/oauth2/v1/certs",
        "client_x509_cert_url":"https://example.invalid/unused"})
}
fn imported(value: &Value) -> CredentialMaterial {
    import_service_account(serde_json::to_vec(value).unwrap().as_slice()).unwrap()
}

#[test]
fn real_rsa_signature_and_fixed_claims_are_verified_and_tampering_rejected() {
    let account = imported(&file());
    let assertion =
        service_account_assertion(&account, UNIX_EPOCH + Duration::from_secs(1_700_000_000))
            .unwrap();
    assert!(!format!("{account:?} {assertion:?}").contains("PRIVATE KEY"));
    let parts: Vec<_> = assertion.expose().split('.').collect();
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|part| !part.contains(['+', '/', '='])));
    let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
    assert_eq!(
        header,
        json!({"alg":"RS256","typ":"JWT","kid":"synthetic-test-only"})
    );
    let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(
        claims,
        json!({"iss":"synthetic@example.iam.gserviceaccount.com",
        "scope":GOOGLE_TRANSLATION_SCOPE, "aud": GOOGLE_TOKEN_ENDPOINT, "iat":1_700_000_000u64,"exp":1_700_003_600u64})
    );
    let encoded = KEY
        .trim()
        .strip_prefix("-----BEGIN PRIVATE KEY-----")
        .unwrap()
        .strip_suffix("-----END PRIVATE KEY-----")
        .unwrap()
        .split_whitespace()
        .collect::<String>();
    let pair = KeyPair::from_pkcs8(&STANDARD.decode(encoded).unwrap()).unwrap();
    let verifier = signature::UnparsedPublicKey::new(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        pair.public_key(),
    );
    let message = format!("{}.{}", parts[0], parts[1]);
    let signature = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
    verifier.verify(message.as_bytes(), &signature).unwrap();
    assert!(
        verifier
            .verify(format!("{message}x").as_bytes(), &signature)
            .is_err()
    );
}

#[test]
fn optional_key_id_is_omitted_and_no_delegation_or_custom_scope_exists() {
    let mut value = file();
    value.as_object_mut().unwrap().remove("private_key_id");
    let assertion =
        service_account_assertion(&imported(&value), UNIX_EPOCH + Duration::from_secs(10)).unwrap();
    let header: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(assertion.expose().split('.').next().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(header, json!({"alg":"RS256","typ":"JWT"}));
    for field in ["scope", "sub", "aud", "privateKey", "headers", "script"] {
        let mut value = file();
        value[field] = json!("not-allowed");
        assert_eq!(
            import_service_account(serde_json::to_vec(&value).unwrap().as_slice())
                .unwrap_err()
                .code,
            "invalid_service_account"
        );
    }
}

#[test]
fn imported_authority_cannot_redirect_tokens_or_select_different_account_kind() {
    for (field, value) in [
        ("type", "authorized_user"),
        ("token_uri", "https://example.invalid/token"),
        ("token_uri", "http://oauth2.googleapis.com/token"),
        ("universe_domain", "example.invalid"),
    ] {
        let mut data = file();
        data[field] = json!(value);
        assert_eq!(
            import_service_account(serde_json::to_vec(&data).unwrap().as_slice())
                .unwrap_err()
                .code,
            "unsupported_service_account"
        );
    }
}

#[test]
fn bounded_read_stops_after_limit_and_errors_never_echo_source_or_key() {
    struct Endless {
        reads: usize,
    }
    impl Read for Endless {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            self.reads += output.len();
            output.fill(b'x');
            Ok(output.len())
        }
    }
    let mut source = Endless { reads: 0 };
    let error = import_service_account(&mut source).unwrap_err();
    assert_eq!(source.reads, MAX_SERVICE_ACCOUNT_FILE_BYTES + 1);
    assert_eq!(error.code, "service_account_file_size");
    assert!(!format!("{error:?}").contains("xxxx"));
    assert!(import_service_account([].as_slice()).is_err());
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("PRIVATE-KEY-MUST-NOT-LEAK"))
        }
    }
    assert!(
        !format!("{:?}", import_service_account(Broken).unwrap_err()).contains("MUST-NOT-LEAK")
    );
}

#[test]
fn invalid_keys_subjects_types_and_duplicate_fields_are_rejected() {
    for value in [
        "",
        "not-pem",
        "-----BEGIN PRIVATE KEY-----\n!!!!\n-----END PRIVATE KEY-----",
        "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----",
    ] {
        let mut data = file();
        data["private_key"] = json!(value);
        assert!(import_service_account(serde_json::to_vec(&data).unwrap().as_slice()).is_err());
    }
    for (field, value) in [
        ("client_email", "no-email"),
        ("client_email", "@example.invalid"),
        ("client_email", "a@b@c"),
        ("client_email", "a\n@example.invalid"),
        ("private_key_id", "bad\r\nid"),
        ("project_id", "has whitespace"),
    ] {
        let mut data = file();
        data[field] = json!(value);
        assert!(import_service_account(serde_json::to_vec(&data).unwrap().as_slice()).is_err());
    }
    let duplicate =
        serde_json::to_string(&file())
            .unwrap()
            .replacen('{', "{\"type\":\"service_account\",", 1);
    assert!(import_service_account(duplicate.as_bytes()).is_err());
    let mut data = file();
    data["private_key"] = json!("s".repeat(16_385));
    assert!(import_service_account(serde_json::to_vec(&data).unwrap().as_slice()).is_err());
}

#[test]
fn vault_material_is_revalidated_and_invalid_time_or_kind_is_not_signed() {
    let account = imported(&file());
    for now in [
        UNIX_EPOCH,
        UNIX_EPOCH - Duration::from_secs(1),
        UNIX_EPOCH + Duration::from_secs(253_402_300_800),
    ] {
        assert_eq!(
            service_account_assertion(&account, now).unwrap_err().code,
            "authorization_clock_invalid"
        );
    }
    let wrong = CredentialMaterial::ApiKey {
        key: SecretText::from_bytes(b"test-only".to_vec()).unwrap(),
    };
    assert_eq!(
        service_account_assertion(&wrong, UNIX_EPOCH + Duration::from_secs(100))
            .unwrap_err()
            .code,
        "credential_kind_mismatch"
    );
    let bad = CredentialMaterial::GoogleServiceAccount {
        client_email: "bad\n@example.invalid".into(),
        private_key: SecretText::from_bytes(KEY.as_bytes().to_vec()).unwrap(),
        private_key_id: None,
        project_id: None,
    };
    assert_eq!(
        service_account_assertion(&bad, UNIX_EPOCH + Duration::from_secs(100))
            .unwrap_err()
            .code,
        "invalid_service_account"
    );
}
