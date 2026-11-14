//! Typed Vault lifecycle via the real SQLite repository and shared application storage use case.
use app_core::{
    application::credential_storage,
    contracts::*,
    domain::{content_hash, model_config::credentials::*},
    ports::{IdentitySource, ProductRepository, runtime::CredentialVault},
};
use app_infra::{platform::SystemIdentity, storage::SqliteStore};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
fn dir() -> PathBuf {
    let p = std::env::var_os("FP_STORAGE_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("http-envelopes-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn secret(v: &str) -> SecretText {
    SecretText::from_bytes(v.as_bytes().to_vec()).unwrap()
}
fn materials() -> Vec<CredentialMaterial> {
    vec![
        CredentialMaterial::ApiKey {
            key: secret("typed-only-sensitive-key"),
        },
        CredentialMaterial::BaiduAppSecret {
            app_id: "application-id".into(),
            secret: secret("baidu-only-sensitive-secret"),
        },
        CredentialMaterial::YoudaoAppSecret {
            app_key: "application-key".into(),
            secret: secret("youdao-only-sensitive-secret"),
        },
        CredentialMaterial::GoogleServiceAccount {
            client_email: "reader@example.invalid".into(),
            private_key: secret("controlled-only-private-key-material"),
            private_key_id: Some("key-identity".into()),
            project_id: Some("project-identity".into()),
        },
    ]
}
#[derive(Default)]
struct MemoryVault {
    data: Mutex<BTreeMap<String, Vec<u8>>>,
    fail: bool,
    revoke_during_put: Option<Arc<SqliteStore>>,
}
impl CredentialVault for MemoryVault {
    fn put(&self, id: &str, secret: &[u8]) -> AppResult<()> {
        if self.fail {
            return Err(AppError::new(
                "controlled_vault_failure",
                "test",
                "controlled",
            ));
        }
        self.data.lock().unwrap().insert(id.into(), secret.to_vec());
        if let Some(repo) = &self.revoke_during_put {
            repo.revoke_credential(id.into(), i64::MAX - 1)?;
        }
        Ok(())
    }
    fn get(&self, id: &str) -> AppResult<Vec<u8>> {
        self.data
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::new("credential_missing", "test", "controlled"))
    }
    fn delete(&self, id: &str) -> AppResult<()> {
        self.data.lock().unwrap().remove(id);
        Ok(())
    }
}
fn emit(case: &str, path: &Path, assertions: u64, scope: &str) {
    if let Some(out) = std::env::var_os("FP_STORAGE_OUTPUT") {
        use std::io::Write;
        let d = PathBuf::from(out).join("components/v3-delta");
        std::fs::create_dir_all(&d).unwrap();
        let v = serde_json::json!({"kind":"http_storage_v3_supporting_case","case":case,"run_id":std::env::var("FP_STORAGE_RUN_ID").unwrap(),"source_tree_sha256":std::env::var("FP_STORAGE_SOURCE_SHA256").unwrap(),"execution_nonce":std::env::var("FP_STORAGE_EXECUTION_NONCE").unwrap(),"completed_at_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(),"assertions":assertions,"detail":{"database":{"path":path.canonicalize().unwrap(),"sha256":content_hash(std::fs::read(path).unwrap())},"scope":scope}});
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(d.join(format!("{case}.json")))
            .unwrap();
        f.write_all(&serde_json::to_vec_pretty(&v).unwrap())
            .unwrap();
        f.sync_all().unwrap();
    }
}
#[test]
fn each_http_material_uses_trusted_metadata_and_never_persists_plaintext_in_sqlite() {
    let d = dir();
    let store = SqliteStore::open(d.join("profile.sqlite3"), "envelopes", "server", 1).unwrap();
    let vault = MemoryVault::default();
    let clock = SystemIdentity;
    for material in materials() {
        let kind = material.kind();
        let encoded = encode_vault_credential(&material).unwrap();
        let r = credential_storage::store(
            &store,
            &vault,
            &clock,
            "controlled".into(),
            CredentialStorageMetadata::EnvelopeV1 { kind },
            encoded.expose(),
        )
        .unwrap();
        let m = store
            .credential_lease_metadata(r.pool_id, 1, r.credential_version_id.clone(), kind)
            .unwrap();
        assert_eq!(m.storage, CredentialStorageMetadata::EnvelopeV1 { kind });
        let value = vault.get(&r.credential_version_id).unwrap();
        let decoded = decode_vault_credential(m.storage, value).unwrap();
        assert_eq!(decoded.kind(), kind);
        assert_eq!(
            encode_vault_credential(&decoded).unwrap().expose(),
            encoded.expose()
        );
        vault.delete(&r.credential_version_id).unwrap();
    }
    let path = d.join("clean-sqlite.sqlite3");
    store.backup(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    for needle in [
        "typed-only-sensitive-key",
        "baidu-only-sensitive-secret",
        "youdao-only-sensitive-secret",
        "controlled-only-private-key-material",
        "application-id",
        "application-key",
        "reader@example.invalid",
    ] {
        assert!(
            !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "private authentication field leaked into SQLite: {needle}"
        );
    }
    emit(
        "typed-envelope-sqlite",
        &path,
        19,
        "real repository + controlled Vault; not OS Keychain evidence",
    );
}
#[test]
fn vault_failure_and_activation_revocation_race_compensate_without_active_material() {
    for race in [false, true] {
        let d = dir();
        let store =
            Arc::new(SqliteStore::open(d.join("profile.sqlite3"), "failure", "server", 1).unwrap());
        let vault = MemoryVault {
            fail: !race,
            revoke_during_put: race.then(|| store.clone()),
            ..Default::default()
        };
        let material = materials().remove(1);
        let encoded = encode_vault_credential(&material).unwrap();
        let error = credential_storage::store(
            store.as_ref(),
            &vault,
            &SystemIdentity,
            "controlled".into(),
            CredentialStorageMetadata::EnvelopeV1 {
                kind: material.kind(),
            },
            encoded.expose(),
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            if race {
                "credential_state_conflict"
            } else {
                "controlled_vault_failure"
            }
        );
        assert!(vault.data.lock().unwrap().is_empty());
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.credentials.len(), 1);
        assert_ne!(snapshot.credentials[0].state, "active");
        assert!(store.pending_credentials().unwrap().is_empty());
        let path = d.join("compensated.sqlite3");
        store.backup(&path).unwrap();
        if race {
            emit(
                "typed-activation-compensation",
                &path,
                4,
                "actual revoked-during-Vault-put race",
            );
        } else {
            emit(
                "typed-vault-failure",
                &path,
                4,
                "actual pending metadata + controlled Vault write failure",
            );
        }
    }
}
#[test]
fn legacy_json_like_key_remains_raw_through_same_shared_storage_lifecycle() {
    let d = dir();
    let store = SqliteStore::open(d.join("profile.sqlite3"), "raw", "server", 1).unwrap();
    let vault = MemoryVault::default();
    let raw = br#"{"schemaVersion":1,"material":{"kind":"api_key","key":"nested"}}"#;
    let r = credential_storage::store(
        &store,
        &vault,
        &SystemIdentity,
        "raw".into(),
        CredentialStorageMetadata::LegacyRawApiKey,
        raw,
    )
    .unwrap();
    let m = store
        .credential_lease_metadata(
            r.pool_id,
            1,
            r.credential_version_id.clone(),
            CredentialMaterialKind::ApiKey,
        )
        .unwrap();
    assert_eq!(m.storage, CredentialStorageMetadata::LegacyRawApiKey);
    let material =
        decode_vault_credential(m.storage, vault.get(&r.credential_version_id).unwrap()).unwrap();
    let CredentialMaterial::ApiKey { key } = material else {
        panic!()
    };
    assert_eq!(key.expose().as_bytes(), raw);
    let path = d.join("raw.sqlite3");
    store.backup(&path).unwrap();
    emit(
        "legacy-raw-vault",
        &path,
        2,
        "controlled JSON-like raw key retained whole; metadata alone chooses decoder",
    );
}
#[cfg(target_os = "macos")]
#[test]
fn actual_system_keychain_roundtrips_all_http_envelopes_with_real_sql_metadata() {
    use app_infra::credentials::SystemVault;
    let d = dir();
    let profile = format!("schema3-controlled-{}", uuid::Uuid::new_v4());
    let store = SqliteStore::open(d.join("profile.sqlite3"), &profile, "server", 1).unwrap();
    let vault = SystemVault::new(&profile);
    for material in materials() {
        let kind = material.kind();
        let encoded = encode_vault_credential(&material).unwrap();
        let r = credential_storage::store(
            &store,
            &vault,
            &SystemIdentity,
            "controlled-system-keychain".into(),
            CredentialStorageMetadata::EnvelopeV1 { kind },
            encoded.expose(),
        )
        .unwrap();
        let metadata = store
            .credential_lease_metadata(r.pool_id, 1, r.credential_version_id.clone(), kind)
            .unwrap();
        let actual = vault.get(&r.credential_version_id);
        let cleanup = vault.delete(&r.credential_version_id);
        let actual = actual.unwrap();
        cleanup.unwrap();
        assert_eq!(
            decode_vault_credential(metadata.storage, actual)
                .unwrap()
                .kind(),
            kind
        );
        store
            .revoke_credential(r.credential_id.clone(), SystemIdentity.now_ms())
            .unwrap();
        store
            .mark_credential_cleaned(r.credential_id, SystemIdentity.now_ms())
            .unwrap();
        assert!(vault.get(&r.credential_version_id).is_err());
    }
    let snapshot = d.join("system-keychain.sqlite3");
    store.backup(&snapshot).unwrap();
    emit(
        "typed-real-system-keychain",
        &snapshot,
        8,
        "actual macOS isolated Keychain namespace; every test item deleted after read",
    );
}
