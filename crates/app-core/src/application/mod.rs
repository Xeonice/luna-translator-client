pub mod binding_journey;
pub mod google_interactive;
pub mod service_request_plan;
pub mod configuration_controls;
pub mod credential_storage;
pub mod game_assets;
pub mod history;
pub mod model_tests;
pub mod request_plan;
pub mod runtime;
pub mod service_configuration;
pub mod google_test_operations;
pub mod service_testing;
pub mod service_discovery;
pub mod session_records;
use crate::{contracts::*, domain::*, ports::*};
use std::sync::Arc;
pub struct AppService {
    repository: Arc<dyn ProductRepository>,
    clock: Arc<dyn IdentitySource>,
}
impl AppService {
    pub fn new(repository: Arc<dyn ProductRepository>, clock: Arc<dyn IdentitySource>) -> Self {
        Self { repository, clock }
    }
    pub fn snapshot(&self) -> AppResult<AppSnapshot> {
        self.repository.snapshot()
    }
    pub fn history(
        &self,
        game_id: Option<String>,
        cursor: Option<String>,
        limit: u32,
    ) -> AppResult<HistoryPage> {
        self.repository
            .history(game_id, cursor, limit.clamp(1, 100))
    }
    pub fn dispatch(&self, mut envelope: CommandEnvelope) -> AppResult<CommandReceipt> {
        require_id(&envelope.operation_id)?;
        match &mut envelope.command {
            Command::SaveGame(input) => normalize_game_input(input)?,
            Command::SaveGames { items } => {
                crate::domain::library::normalize_game_batch(items)?;
                if envelope.expected_revision.is_none() {
                    return Err(AppError::new(
                        "revision_required",
                        "library",
                        "批量添加需要当前游戏库版本，请重新核对后保存",
                    ));
                }
            }
            _ => {}
        }
        match &envelope.command {
            Command::SaveCredentialPool { pool } => {
                crate::domain::credentials::validate_pool(pool)?
            }
            Command::SaveGame(g) => validate_game(g)?,
            Command::ManualInput {
                text,
                source_language,
            } => {
                if text.trim().is_empty() {
                    return Err(AppError::new("empty_text", "reading", "请输入原文"));
                }
                if text.contains('\0') || source_language.is_empty() {
                    return Err(AppError::new("invalid_text", "reading", "原文或语言无效"));
                }
            }
            Command::SaveConfiguration { configuration, .. } => {
                validate_configuration(configuration, false)?
            }
            Command::SavePreferences { preferences }
            | Command::SaveScopedPreferences { preferences, .. } => {
                validate_preferences(preferences)?
            }
            // Typed booleans; the repository atomically patches only these two global fields.
            Command::SetReadingAnnotations { .. } => {}
            Command::SaveCorrection { text, .. } => {
                if text.trim().is_empty() {
                    return Err(AppError::new(
                        "empty_correction",
                        "history",
                        "人工修正不能为空",
                    ));
                }
            }
            _ => {}
        }
        self.repository.dispatch(envelope, self.clock.now_ms())
    }
    pub fn history_page(&self, mut query: HistoryQuery) -> AppResult<HistoryPage> {
        history::validate_filter(&query.filter)?;
        query.limit = query.limit.clamp(1, 100);
        self.repository.history_page(query)
    }
    pub fn history_sessions(
        &self,
        mut query: HistorySessionsQuery,
    ) -> AppResult<HistorySessionsPage> {
        history::validate_session_filter(&query.filter)?;
        query.limit = query.limit.clamp(1, 100);
        self.repository.history_sessions(query)
    }
}
