pub mod offline_runtime;
pub mod service_testing;
pub mod service_execution;
pub mod service_dispatch;
pub mod capture;
pub mod credentials;
pub mod retention;
pub mod translation_status;
use crate::contracts::*;
pub use model_config::testing;
use sha2::{Digest, Sha256};

pub fn content_hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
pub fn canonical_hash<T: serde::Serialize>(value: &T) -> AppResult<String> {
    serde_json::to_vec(value)
        .map(content_hash)
        .map_err(|_| AppError::new("invalid_json", "validation", "无法编码规范对象"))
}
pub fn require_id(value: &str) -> AppResult<()> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(AppError::new(
            "invalid_id",
            "validation",
            "对象身份为空或包含控制字符",
        ))
    } else {
        Ok(())
    }
}
pub mod library;
pub mod model_config;
pub mod preferences;
pub mod reading;
pub use library::{normalize_game_input, normalize_windows_path, validate_game};
pub use model_config::{
    effective_binding, resolve_parameters, validate_configuration, validate_parameter,
};
pub use preferences::validate_preferences;
pub use reading::{canonical_u64, utf16_span};
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn canonical_uint64_preserves_full_width() {
        assert_eq!(canonical_u64("18446744073709551615").unwrap(), u64::MAX);
        for s in ["01", "+1", "1\0evil", "18446744073709551616", ""] {
            assert!(canonical_u64(s).is_err());
        }
    }
    #[test]
    fn utf8_and_utf16_positions_are_distinct() {
        assert_eq!(utf16_span("彼女😀と", 6, 10).unwrap(), (2, 4));
        assert!(utf16_span("彼女", 1, 3).is_err());
    }
    #[test]
    fn parameter_zero_and_omit_survive() {
        let a = BTreeMap::from([(
            "temperature".into(),
            Parameter {
                mode: ParameterMode::Set,
                value: Some(serde_json::json!(0)),
            },
        )]);
        let b = BTreeMap::from([(
            "temperature".into(),
            Parameter {
                mode: ParameterMode::Inherit,
                value: None,
            },
        )]);
        assert_eq!(resolve_parameters(&[&a, &b])["temperature"], 0);
        let b = BTreeMap::from([(
            "temperature".into(),
            Parameter {
                mode: ParameterMode::Omit,
                value: None,
            },
        )]);
        assert!(resolve_parameters(&[&a, &b]).is_empty());
    }
    #[test]
    fn nonfinite_preferences_rejected() {
        let p = Preferences {
            source_font_pt: f64::INFINITY,
            ..Default::default()
        };
        assert!(validate_preferences(&p).is_err());
    }
}

#[derive(Debug, Clone)]
pub struct TaskDispatch {
    pub selected_token: Option<LexicalToken>,
    pub credential_vault_ref: Option<String>,
    pub cached_result: Option<CachedTaskResult>,
    pub frozen_history: Vec<ReadingUnit>,
    pub task_id: String,
    pub attempt_id: String,
    pub unit: ReadingUnit,
    pub configuration: Configuration,
    pub binding: PurposeBinding,
    pub engine: Engine,
    pub model: Model,
    pub connection: Connection,
    pub request_semantics_hash: String,
    pub authorization_epoch: u32,
    pub profile_generation: u32,
    pub server_instance_id: String,
}
/// Internal explicit-retranslation preparation. Replayed operation IDs return
/// their durable tasks without resubmitting any dispatch to the provider.
pub struct PreparedTranslation {
    pub dispatches: Vec<TaskDispatch>,
    pub task_ids: Vec<String>,
    /// Durable operation receipts, independent of the bounded snapshot history.
    pub task_views: Vec<TaskView>,
}
#[derive(Debug, Clone)]
pub struct TaskCompletion {
    pub task_id: String,
    pub attempt_id: String,
    pub text: String,
    pub output_mode: String,
    pub structured: serde_json::Value,
    pub target_language: String,
    pub provider_label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedTaskResult {
    pub text: String,
    pub output_mode: String,
    pub structured: serde_json::Value,
    pub target_language: String,
    pub provider_label: String,
}
