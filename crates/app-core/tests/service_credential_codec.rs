use app_core::{
    contracts::*,
    domain::{
        content_hash,
        model_config::{codec::*, credentials::*},
    },
};
use std::{collections::BTreeMap, num::NonZeroU32};
fn secret(value: &str) -> SecretText {
    SecretText::from_bytes(value.as_bytes().to_vec()).unwrap()
}
fn decode(kind: &str, schema: Option<u32>, bytes: &[u8]) -> AppResult<CredentialMaterial> {
    decode_vault_credential(
        CredentialStorageMetadata::from_stored(kind, schema)?,
        bytes.to_vec(),
    )
}
fn key_of(material: &CredentialMaterial) -> &str {
    match material {
        CredentialMaterial::ApiKey { key } => key.expose(),
        _ => panic!("expected fixed ApiKey material"),
    }
}
#[test]
fn legacy_raw_key_that_looks_like_json_is_not_reinterpreted_as_an_envelope() {
    let raw = br#"{"schemaVersion":1,"material":{"kind":"api_key","key":"nested-key"}}"#;
    let result = decode("api_key", None, raw).unwrap();
    assert_eq!(key_of(&result), std::str::from_utf8(raw).unwrap());
    assert_ne!(key_of(&result), "nested-key");
    let material = decode("api_key", Some(1), raw).unwrap();
    assert_eq!(key_of(&material), "nested-key");
}
#[test]
fn trusted_metadata_rejects_unknown_kind_or_schema_without_guessing_raw_key() {
    for (kind, schema) in [
        ("api_key", Some(0)),
        ("api_key", Some(2)),
        ("api_key", Some(u32::MAX)),
        ("unknown", None),
        ("baidu_app_secret", None),
    ] {
        assert!(CredentialStorageMetadata::from_stored(kind, schema).is_err());
    }
    assert_eq!(
        decode("api_key", Some(1), b"a perfectly usable raw key")
            .unwrap_err()
            .code,
        "invalid_credential_envelope"
    );
    assert_eq!(
        decode(
            "api_key",
            Some(1),
            br#"{"schemaVersion":2,"material":{"kind":"api_key","key":"k"}}"#
        )
        .unwrap_err()
        .code,
        "unsupported_credential_schema"
    );
}
#[test]
fn every_fixed_material_kind_roundtrips_only_through_explicit_vault_serializer() {
    let materials = [
        CredentialMaterial::ApiKey {
            key: secret("日文Keyの値"),
        },
        CredentialMaterial::BaiduAppSecret {
            app_id: "application-1".into(),
            secret: secret("密钥-1"),
        },
        CredentialMaterial::YoudaoAppSecret {
            app_key: "app-2".into(),
            secret: secret("密钥-2"),
        },
        CredentialMaterial::GoogleServiceAccount {
            client_email: "reader@example.invalid".into(),
            private_key: secret("test-only-private-key-material"),
            private_key_id: Some("key-id".into()),
            project_id: Some("project-id".into()),
        },
        CredentialMaterial::GoogleOauthRefresh {
            refresh_token: secret("test-only-refresh-token"),
        },
    ];
    for material in materials {
        let bytes = encode_vault_credential(&material).unwrap();
        let metadata =
            CredentialStorageMetadata::from_stored(material.kind().key(), Some(1)).unwrap();
        let restored = decode_vault_credential(metadata, bytes.expose().to_vec()).unwrap();
        assert_eq!(restored.kind(), material.kind());
        let reencoded = encode_vault_credential(&restored).unwrap();
        assert_eq!(reencoded.expose(), bytes.expose());
    }
}
#[test]
fn mismatched_metadata_and_material_never_become_another_authentication_type() {
    let bytes=br#"{"schemaVersion":1,"material":{"kind":"baidu_app_secret","appId":"appid","secret":"secret"}}"#;
    assert_eq!(
        decode("api_key", Some(1), bytes).unwrap_err().code,
        "credential_kind_mismatch"
    );
    assert_eq!(
        decode("youdao_app_secret", Some(1), bytes)
            .unwrap_err()
            .code,
        "credential_kind_mismatch"
    );
}
#[test]
fn envelope_and_material_reject_unknown_fields_scripts_headers_and_token_urls() {
    let root =
        serde_json::json!({"schemaVersion":1,"material":{"kind":"api_key","key":"test-only-key"}});
    for extra in ["headers", "script", "tokenUrl", "credentialRef", "authKind"] {
        let mut value = root.clone();
        value[extra] = serde_json::json!("forbidden");
        assert_eq!(
            decode("api_key", Some(1), &serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "invalid_credential_envelope"
        );
        let mut value = root.clone();
        value["material"][extra] = serde_json::json!("forbidden");
        assert_eq!(
            decode("api_key", Some(1), &serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .code,
            "invalid_credential_envelope"
        );
    }
    assert!(
        decode(
            "api_key",
            Some(1),
            br#"{"schemaVersion":1,"material":{"kind":"api_key","key":"one","key":"two"}}"#
        )
        .is_err()
    );
    assert!(
        decode(
            "api_key",
            Some(1),
            br#"{"schemaVersion":1,"material":{"kind":"new_adapter","key":"one"}}"#
        )
        .is_err()
    );
    assert!(
        decode(
            "baidu_app_secret",
            Some(1),
            br#"{"schemaVersion":1,"material":{"kind":"baidu_app_secret","secret":"one"}}"#
        )
        .is_err()
    );
}
#[test]
fn secret_boundaries_are_utf8_bytes_not_characters_and_errors_are_redacted() {
    assert!(SecretText::from_bytes(vec![]).is_err());
    assert!(SecretText::from_bytes(vec![0xff]).is_err());
    assert!(SecretText::from_bytes(b"sensitive\0value".to_vec()).is_err());
    assert!(SecretText::from_bytes(vec![b'x'; MAX_SECRET_BYTES]).is_ok());
    assert!(SecretText::from_bytes(vec![b'x'; MAX_SECRET_BYTES + 1]).is_err());
    let japanese = "日".repeat(MAX_SECRET_BYTES / 3);
    assert!(SecretText::from_bytes(japanese.into_bytes()).is_ok());
    let too_large = "SECRET-DO-NOT-ECHO".repeat(MAX_SECRET_BYTES);
    let error = SecretText::from_bytes(too_large.into_bytes()).unwrap_err();
    assert!(!error.to_string().contains("SECRET-DO-NOT-ECHO"));
    assert_eq!(
        decode(
            "api_key",
            Some(1),
            &vec![b'x'; MAX_VAULT_ENVELOPE_BYTES + 1]
        )
        .unwrap_err()
        .code,
        "credential_envelope_too_large"
    );
}
#[test]
fn every_secret_debug_surface_is_redacted_and_leases_borrow_exact_materials() {
    let material = CredentialMaterial::BaiduAppSecret {
        app_id: "public-application".into(),
        secret: secret("never-debug-this-secret"),
    };
    let bytes = encode_vault_credential(&material).unwrap();
    assert!(!format!("{material:?} {bytes:?}").contains("never-debug-this-secret"));
    let source = CredentialVersionRef {
        credential_id: "credential-1".into(),
        credential_version_id: "version-1".into(),
    };
    source.validate().unwrap();
    if let CredentialMaterial::BaiduAppSecret { app_id, secret } = &material {
        let lease = ProviderAuthLease::AppSecret {
            source: &source,
            material: SignedAppLease::Baidu { app_id, secret },
        };
        assert!(!format!("{lease:?}").contains("never-debug-this-secret"));
        match lease {
            ProviderAuthLease::AppSecret {
                material: SignedAppLease::Baidu { app_id, secret },
                source,
            } => {
                assert_eq!(app_id, "public-application");
                assert_eq!(secret.expose(), "never-debug-this-secret");
                assert_eq!(source.credential_version_id, "version-1");
            }
            _ => panic!("fixed lease shape changed"),
        }
    }
}
#[test]
fn public_authentication_subjects_are_bounded_and_service_account_fields_are_fixed() {
    let material = CredentialMaterial::BaiduAppSecret {
        app_id: "application\0other".into(),
        secret: secret("secret"),
    };
    assert!(encode_vault_credential(&material).is_err());
    let material = CredentialMaterial::YoudaoAppSecret {
        app_key: "x".repeat(513),
        secret: secret("secret"),
    };
    assert!(encode_vault_credential(&material).is_err());
    let material = CredentialMaterial::GoogleServiceAccount {
        client_email: "not-email".into(),
        private_key: secret("private"),
        private_key_id: None,
        project_id: None,
    };
    assert!(encode_vault_credential(&material).is_err());
    let with_endpoint=br#"{"schemaVersion":1,"material":{"kind":"google_service_account","clientEmail":"r@example.invalid","privateKey":"private","tokenUri":"https://attacker.invalid/token"}}"#;
    assert_eq!(
        decode("google_service_account", Some(1), with_endpoint)
            .unwrap_err()
            .code,
        "invalid_credential_envelope"
    );
}
#[test]
fn google_lease_identity_keeps_unknown_principal_and_rejects_invalid_scope_fingerprint() {
    let mut identity = GoogleAuthorizationLeaseIdentity {
        grant_id: "grant".into(),
        authorization_epoch: NonZeroU32::new(1).unwrap(),
        material_revision: NonZeroU32::new(2).unwrap(),
        credential_version: CredentialVersionRef {
            credential_id: "credential".into(),
            credential_version_id: "v2".into(),
        },
        scopes_hash: "a".repeat(64),
        principal_state: PrincipalState::Unknown,
    };
    identity.validate().unwrap();
    assert_eq!(identity.principal_state, PrincipalState::Unknown);
    identity.scopes_hash = "not-hash".into();
    assert_eq!(
        identity.validate().unwrap_err().code,
        "invalid_authorization_identity"
    );
    assert!(NonZeroU32::new(0).is_none());
}

// Literal old wire, manually retained in its original caller Vec order, not generated by either tool.
const LEGACY_GOLDEN: &str = r#"{"revisionId":"legacy-r1","connections":[{"connectionId":"z","displayName":"旧服务Z","protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:8181/v1","credentialPoolId":null,"credentialPoolRevision":null,"proxyMode":"disabled","networkMetadata":{},"availabilityIntent":false},{"connectionId":"a","displayName":"旧服务A","protocol":"anthropic","deployment":"cloud","authMode":"api_key","endpoint":null,"credentialPoolId":null,"credentialPoolRevision":null,"proxyMode":"inherit","networkMetadata":{},"availabilityIntent":false}],"models":[],"bindings":[],"templates":[],"termSets":[]}"#;
fn legacy() -> Configuration {
    serde_json::from_str(LEGACY_GOLDEN).unwrap()
}
#[test]
fn original_serializer_golden_is_byte_exact_without_new_fields_or_root_wrapper() {
    let value = legacy();
    let encoded = encode_legacy_configuration(&value).unwrap();
    assert_eq!(encoded, LEGACY_GOLDEN.as_bytes());
    assert_eq!(
        app_core::domain::canonical_hash(&value).unwrap(),
        content_hash(LEGACY_GOLDEN)
    );
    assert!(
        !std::str::from_utf8(&encoded)
            .unwrap()
            .contains("codecVersion")
    );
}
#[test]
fn lost_legacy_vector_order_cannot_be_recovered_by_rehashing_sorted_loaded_rows() {
    let original = legacy();
    let mut loaded = original.clone();
    loaded
        .connections
        .sort_by(|a, b| a.connection_id.cmp(&b.connection_id));
    assert_ne!(
        content_hash(encode_legacy_configuration(&original).unwrap()),
        content_hash(encode_legacy_configuration(&loaded).unwrap())
    );
    assert_eq!(
        encode_normalized_service_graph_configuration(&original).unwrap(),
        encode_normalized_service_graph_configuration(&loaded).unwrap()
    );
    assert_eq!(original.connections[0].connection_id, "z");
}
fn engine(id: &str) -> Engine {
    Engine {
        engine_id: id.into(),
        model_id: "m".into(),
        enabled: true,
        template_id: None,
        parameters: BTreeMap::new(),
    }
}
fn term(id: &str) -> Term {
    Term {
        term_id: id.into(),
        source_text: Some(id.into()),
        target_text: Some(id.into()),
        source_language: None,
        target_language: None,
        enabled: true,
    }
}
#[test]
fn normalization_sorts_unordered_entities_but_preserves_engine_and_term_positions() {
    let mut config = legacy();
    config.bindings.push(PurposeBinding {
        binding_id: "binding".into(),
        scope_key: "global".into(),
        game_id: None,
        purpose: "translation".into(),
        binding_mode: "override".into(),
        enabled: false,
        output_mode: None,
        source_language: None,
        target_language: None,
        template_id: None,
        context: BTreeMap::new(),
        schedule: BTreeMap::new(),
        parameters: BTreeMap::new(),
        engines: vec![engine("z-slot"), engine("a-slot")],
    });
    config.term_sets.push(TermSet {
        term_set_id: "terms".into(),
        scope_key: "global".into(),
        game_id: None,
        display_name: "术语".into(),
        enabled: true,
        terms: vec![term("z-term"), term("a-term")],
    });
    let normalized = normalize_service_graph_configuration(&config).unwrap();
    assert_eq!(normalized.connections[0].connection_id, "a");
    assert_eq!(normalized.bindings[0].engines[0].engine_id, "z-slot");
    assert_eq!(normalized.term_sets[0].terms[0].term_id, "z-term");
    let bytes = encode_normalized_service_graph_configuration(&config).unwrap();
    config.bindings[0].engines.reverse();
    assert_ne!(
        bytes,
        encode_normalized_service_graph_configuration(&config).unwrap()
    );
    config.bindings[0].engines.reverse();
    config.term_sets[0].terms.reverse();
    assert_ne!(
        bytes,
        encode_normalized_service_graph_configuration(&config).unwrap()
    );
}
#[test]
fn normalization_refuses_duplicate_or_invalid_identities_instead_of_dropping_data() {
    let mut config = legacy();
    config.connections.push(config.connections[0].clone());
    assert_eq!(
        normalize_service_graph_configuration(&config)
            .unwrap_err()
            .code,
        "duplicate_configuration_identity"
    );
    let mut config = legacy();
    config.connections[0].connection_id = "bad\0id".into();
    assert!(normalize_service_graph_configuration(&config).is_err());
    assert_eq!(legacy().connections.len(), 2);
}

const CODEC_TWO_GOLDEN: &str = r#"{"revisionId":"fixed-v2","connections":[{"connectionId":"c","displayName":"Example","protocol":"openai_compatible","deployment":"local","authMode":"none","endpoint":"http://127.0.0.1:1/v1","credentialPoolId":null,"credentialPoolRevision":null,"proxyMode":"disabled","networkMetadata":{},"availabilityIntent":false,"adapterBinding":{"protocol":{"protocolId":"openai_compatible","contractRevision":1},"profile":{"profileId":"custom-text","profileRevision":1}}}],"models":[],"bindings":[],"templates":[],"termSets":[],"codecVersion":2}"#;
#[test]
fn codec_two_has_frozen_actual_flat_bytes_and_explicit_identity() {
    let c: Configuration = serde_json::from_str(CODEC_TWO_GOLDEN).unwrap();
    assert_eq!(
        encode_configuration(&c).unwrap(),
        CODEC_TWO_GOLDEN.as_bytes()
    );
    assert_eq!(
        content_hash(encode_configuration(&c).unwrap()),
        content_hash(CODEC_TWO_GOLDEN)
    );
    let mut bad = c.clone();
    bad.codec_version = None;
    assert_eq!(
        encode_configuration(&bad).unwrap_err().code,
        "legacy_codec_extension_forbidden"
    );
    for version in [0, 1, 3, u32::MAX] {
        bad.codec_version = Some(version);
        assert_eq!(
            encode_configuration(&bad).unwrap_err().code,
            "unsupported_configuration_codec"
        );
    }
    let mut bad = c.clone();
    bad.connections[0]
        .adapter_binding
        .as_mut()
        .unwrap()
        .protocol
        .protocol_id = "anthropic".into();
    assert_eq!(
        encode_configuration(&bad).unwrap_err().code,
        "adapter_protocol_mismatch"
    );
    let mut bad = c.clone();
    bad.connections[0].network_metadata.insert(
        "headers".into(),
        serde_json::json!({"Authorization":"secret"}),
    );
    assert_eq!(
        encode_configuration(&bad).unwrap_err().code,
        "protected_network_metadata"
    );
}

#[test]
fn bound_traditional_contract_defaults_are_nonstreaming_and_preserve_legacy_none() {
    use app_core::domain::model_config::{default_request_streaming, validate_protocol_purpose};
    let mut c: Configuration = serde_json::from_str(CODEC_TWO_GOLDEN).unwrap();
    for protocol in [
        "azure_translator",
        "deepl_v2",
        "google_translate_v2",
        "google_translate_v3",
        "baidu_vip",
        "youdao_v3",
        "tencent_tokenhub_translation",
    ] {
        c.connections[0].protocol = protocol.into();
        c.connections[0]
            .adapter_binding
            .as_mut()
            .unwrap()
            .protocol
            .protocol_id = protocol.into();
        assert!(!default_request_streaming(&c.connections[0]), "{protocol}");
        assert!(validate_protocol_purpose(protocol, "translation").is_ok());
        assert_eq!(
            validate_protocol_purpose(protocol, "grammar")
                .unwrap_err()
                .code,
            "unsupported_protocol_purpose"
        );
        let mut legacy = c.connections[0].clone();
        legacy.adapter_binding = None;
        assert!(default_request_streaming(&legacy));
    }
    c.connections[0].protocol = "openai_compatible".into();
    assert!(default_request_streaming(&c.connections[0]));
}
