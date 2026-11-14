//! System vault only. No plaintext fallback and no key readback through UI DTOs.
use app_core::{
    contracts::{AppError, AppResult},
    ports::runtime::CredentialVault,
};

pub struct SystemVault {
    service: String,
}
impl SystemVault {
    pub fn new(profile_id: &str) -> Self {
        Self {
            service: format!("com.lunatranslator.reader.credentials.{profile_id}"),
        }
    }
}
fn error() -> AppError {
    AppError::new(
        "vault_unavailable",
        "credentials",
        "系统安全库拒绝访问或凭据不存在，请解锁后重新授权",
    )
}

#[cfg(target_os = "macos")]
impl CredentialVault for SystemVault {
    fn put(&self, id: &str, secret: &[u8]) -> AppResult<()> {
        security_framework::passwords::set_generic_password(&self.service, id, secret)
            .map_err(|_| error())
    }
    fn get(&self, id: &str) -> AppResult<Vec<u8>> {
        security_framework::passwords::get_generic_password(&self.service, id).map_err(|_| error())
    }
    fn delete(&self, id: &str) -> AppResult<()> {
        match security_framework::passwords::delete_generic_password(&self.service, id) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(_) => Err(error()),
        }
    }
}
#[cfg(not(target_os = "macos"))]
impl CredentialVault for SystemVault {
    fn put(&self, _: &str, _: &[u8]) -> AppResult<()> {
        Err(error())
    }
    fn get(&self, _: &str) -> AppResult<Vec<u8>> {
        Err(error())
    }
    fn delete(&self, _: &str) -> AppResult<()> {
        Err(error())
    }
}
/// Secret buffers never implement Debug and are erased after a request completes.
pub struct Secret(pub Vec<u8>);
impl Drop for Secret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
