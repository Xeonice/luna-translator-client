pub mod executables;
pub mod game_assets;
pub(crate) mod game_launch;
pub mod game_paths;
pub mod session_export;
use app_core::ports::IdentitySource;
use std::time::{SystemTime, UNIX_EPOCH};
pub struct SystemIdentity;
impl IdentitySource for SystemIdentity {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_millis().min(i64::MAX as u128) as i64)
            .unwrap_or(0)
    }
}
