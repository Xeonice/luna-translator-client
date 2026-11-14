//! One pending/Vault/activation transaction boundary shared by legacy and typed credentials.
use crate::{contracts::*, domain::model_config::credentials::*, ports::*};

pub fn store(
    repository: &dyn ProductRepository,
    vault: &dyn crate::ports::runtime::CredentialVault,
    clock: &dyn IdentitySource,
    label: String,
    metadata: CredentialStorageMetadata,
    bytes: &[u8],
) -> AppResult<CredentialReference> {
    let id = clock.new_id("credential");
    let reference = CredentialReference {
        credential_id: id.clone(),
        credential_version_id: id.clone(),
        pool_id: clock.new_id("pool"),
        pool_revision: 1,
        label,
        masked_hint: "••••••••".into(),
    };
    repository.begin_credential_material(reference.clone(), metadata, clock.now_ms())?;
    let stored = vault
        .put(&id, bytes)
        .and_then(|()| repository.activate_credential(id.clone(), clock.now_ms()));
    if let Err(error) = stored {
        let _ = repository.revoke_credential(id.clone(), clock.now_ms());
        if vault.delete(&id).is_ok() {
            let _ = repository.mark_credential_cleaned(id, clock.now_ms());
        }
        return Err(error);
    }
    Ok(reference)
}
