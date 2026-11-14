pub mod google_interactive;
pub mod offline_resources;
pub mod runtime;
pub mod google_authorization;
pub mod operation_budget;
pub mod physical_jobs;
pub mod service_testing;
pub mod service_test_execution;
pub mod service_configuration;
pub mod service_discovery;
pub use service_configuration::{ServiceConfigurationRepository, ServiceDirectory};
use crate::contracts::*;
pub trait GameCoverAssets: Send + Sync {
    fn import_local_cover(&self, game: &Game) -> AppResult<Option<PreparedGameCover>>;
    fn import_session_frame(&self, jpeg: &[u8]) -> AppResult<PreparedGameCover>;
    fn read_cover(&self, asset_ref: &str) -> AppResult<GameCoverBytes>;
}
/// Synchronous port implemented by a bounded, dedicated database writer. Call from blocking/application executor.
pub trait ProductRepository: Send + Sync {
    fn association_progress_facts(
        &self, _operation_id: &str,
    ) -> AppResult<runtime::AssociationOperationFacts> {
        Err(binding_journey_unavailable())
    }
    fn source_selection(&self, _query: SourceSelectionQuery) -> AppResult<SourceSelectionProjection> {
        Err(binding_journey_unavailable())
    }
    fn commit_source_selection(
        &self, _input: runtime::SourceSelectionCommit, _now_ms: i64,
    ) -> AppResult<SelectionCommitReceipt> {
        Err(binding_journey_unavailable())
    }
    fn validate_reading_window_effect(
        &self, _authority: ReadingWindowEffectAuthority,
    ) -> AppResult<SelectionCommitReceipt> {
        Err(binding_journey_unavailable())
    }
    /// Actual storage identity, read during trusted handler composition only.
    fn google_runtime_frame(&self) -> AppResult<google_authorization::GoogleRuntimeFrame> {
        Err(AppError::new("google_runtime_frame_unavailable", "google_authorization", "目标存储未提供真实运行身份"))
    }

    /// Compare the complete immutable dispatch with its stored admitted plan
    /// and current attempt, not merely a caller-supplied task ID.
    fn validate_service_dispatch(
        &self, _task: &crate::domain::service_dispatch::ServiceTaskDispatch,
    ) -> AppResult<()> {
        Err(AppError::new("service_dispatch_authority_unavailable", "models", "存储适配器尚未提供完整服务任务核验"))
    }
    fn prepare_work_tasks_for_operation(
        &self, unit_id: String, text_revision: u32, purpose: String,
        operation_id: String, now_ms: i64,
    ) -> AppResult<Vec<crate::domain::service_dispatch::WorkDispatch>> {
        self.prepare_tasks_for_operation(unit_id, text_revision, purpose, operation_id, now_ms)
            .map(|tasks| tasks.into_iter().map(Into::into).collect())
    }
    fn prepare_work_capture_tasks(
        &self, version: crate::domain::capture::CaptureVersion,
        operation_id: String, now_ms: i64,
    ) -> AppResult<Vec<crate::domain::service_dispatch::WorkDispatch>> {
        self.prepare_capture_tasks(version, operation_id, now_ms)
            .map(|tasks| tasks.into_iter().map(Into::into).collect())
    }
    fn prepare_work_study_tasks(
        &self, unit_id: String, text_revision: u32, purpose: String,
        token_index: Option<u32>, operation_id: String, now_ms: i64,
    ) -> AppResult<Vec<crate::domain::service_dispatch::WorkDispatch>> {
        self.prepare_study_tasks(unit_id, text_revision, purpose, token_index, operation_id, now_ms)
            .map(|tasks| tasks.into_iter().map(Into::into).collect())
    }
    fn prepare_work_uncached_translation_for_operation(
        &self, unit_id: String, text_revision: u32, operation_id: String, now_ms: i64,
    ) -> AppResult<crate::domain::service_dispatch::PreparedWorkTranslation> {
        self.prepare_uncached_translation_for_operation(unit_id, text_revision, operation_id, now_ms)
            .map(|value| crate::domain::service_dispatch::PreparedWorkTranslation {
                dispatches: value.dispatches.into_iter().map(Into::into).collect(),
                task_ids: value.task_ids, task_views: value.task_views,
            })
    }
    fn set_game_cover_if_empty(
        &self,
        _guard: CoverCommitGuard,
        _cover: PreparedGameCover,
        _now_ms: i64,
    ) -> AppResult<CoverInstallOutcome> {
        Err(AppError::new(
            "game_cover_unavailable",
            "library",
            "当前存储不支持托管封面",
        ))
    }
    /// Read-only operation replay/conflict check, called inside the binding
    /// lease. Full expected-window identity is checked by the Hook adapter.
    fn preflight_window_unbind(
        &self,
        _envelope: CommandEnvelope,
    ) -> AppResult<Option<CommandReceipt>> {
        Err(AppError::new(
            "command_preflight_unavailable",
            "command",
            "存储适配器未提供窗口解绑核对",
        ))
    }
    /// SQL-only commit after a reversible local association removal. The
    /// writer must never call Client/native/AppKit while the caller holds its
    /// binding locks. Ordinary dispatch cannot perform this command.
    fn commit_window_unbind(
        &self,
        _envelope: CommandEnvelope,
        _now_ms: i64,
    ) -> AppResult<CommandReceipt> {
        Err(AppError::new(
            "window_unbind_unavailable",
            "command",
            "存储适配器未提供窗口解绑提交",
        ))
    }
    /// Read-only preflight before a non-transactional native stop. The final
    /// durable dispatch still performs its own receipt/revision checks.
    fn preflight_disconnect(
        &self,
        _envelope: CommandEnvelope,
    ) -> AppResult<Option<CommandReceipt>> {
        Err(AppError::new(
            "command_preflight_unavailable",
            "command",
            "存储适配器未提供断开操作核对",
        ))
    }
    fn flush_capture_integrity(&self, _session_id: Option<String>) -> AppResult<()> {
        Err(session_records_unavailable())
    }
    fn preview_preferences(&self, _preferences: Preferences) -> AppResult<Preferences> {
        Err(session_records_unavailable())
    }
    fn preferences_scope(&self, _scope_key: String) -> AppResult<ScopedPreferences> {
        Err(AppError::new(
            "preferences_scope_unavailable",
            "preferences",
            "存储适配器未提供范围偏好查询",
        ))
    }
    fn begin_capture_operation(
        &self,
        _operation_id: String,
        _request_sha256: String,
        _now_ms: i64,
    ) -> AppResult<()> {
        Err(session_records_unavailable())
    }
    fn finish_capture_operation(
        &self,
        _operation_id: String,
        _session_id: Option<String>,
        _now_ms: i64,
    ) -> AppResult<()> {
        Err(session_records_unavailable())
    }
    fn record_hook_event(&self, _input: runtime::HookEventInput) -> AppResult<()> {
        Err(session_records_unavailable())
    }
    fn ingest_hook_text_with_policy(
        &self,
        input: crate::domain::capture::HookTextInput,
        consume_body: bool,
        now_ms: i64,
    ) -> AppResult<crate::domain::capture::CaptureOutcome> {
        if !consume_body {
            return Err(session_records_unavailable());
        }
        self.ingest_hook_text(input, now_ms)
    }
    fn session_records(
        &self,
        _query: SessionRecordQuery,
        _now_ms: i64,
    ) -> AppResult<SessionRecordPage> {
        Err(session_records_unavailable())
    }
    fn prepare_session_export(
        &self,
        _request: SessionExportRequest,
        _now_ms: i64,
    ) -> AppResult<SessionExportPlan> {
        Err(session_records_unavailable())
    }
    fn session_export_page(
        &self,
        _export_id: String,
        _cursor: Option<String>,
        _page_size: u32,
        _now_ms: i64,
    ) -> AppResult<SessionRecordPage> {
        Err(session_records_unavailable())
    }
    fn close_session_export(&self, _export_id: String) -> AppResult<()> {
        Err(session_records_unavailable())
    }
    fn unit(&self, unit_id: String, text_revision: u32) -> AppResult<ReadingUnit>;
    fn reading_unit_projection(
        &self,
        unit_id: String,
        text_revision: u32,
    ) -> AppResult<ReadingUnitProjection>;
    fn prepare_tasks_for_operation(
        &self,
        unit_id: String,
        text_revision: u32,
        purpose: String,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::TaskDispatch>>;
    fn prepare_uncached_translation_for_operation(
        &self,
        unit_id: String,
        text_revision: u32,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<crate::domain::PreparedTranslation>;
    fn prepare_study_tasks(
        &self,
        unit_id: String,
        text_revision: u32,
        purpose: String,
        token_index: Option<u32>,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::TaskDispatch>>;
    fn record_model_test(
        &self,
        input: crate::domain::testing::ModelTestRecordInput,
    ) -> AppResult<()>;
    fn credential_test_reference(
        &self,
        pool_id: String,
        pool_revision: u32,
        vault_ref: String,
        outcome: String,
    ) -> AppResult<crate::domain::testing::TestCredentialUse>;
    fn maintain_storage(
        &self,
        policy: crate::domain::retention::RetentionPolicy,
        protected: crate::domain::retention::ProtectedObjects,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<crate::domain::retention::MaintenanceReceipt>;
    fn select_credential(
        &self,
        pool_id: String,
        pool_revision: u32,
    ) -> AppResult<crate::domain::credentials::CredentialSelection>;
    fn store_provider_preset(&self, preset: StoredProviderPreset, now_ms: i64) -> AppResult<()>;
    fn provider_preset(&self, key: ProviderPresetKey) -> AppResult<StoredProviderPreset>;
    fn select_credential_for_kind(
        &self,
        pool_id: String,
        pool_revision: u32,
        expected_kind: crate::domain::model_config::credentials::CredentialMaterialKind,
    ) -> AppResult<crate::domain::credentials::CredentialSelection>;
    fn credential_lease_metadata(
        &self,
        pool_id: String,
        pool_revision: u32,
        vault_ref: String,
        expected_kind: crate::domain::model_config::credentials::CredentialMaterialKind,
    ) -> AppResult<crate::domain::credentials::CredentialLeaseMetadata>;
    /// Read-only pool inspection for a Rust preparation owner. This must never
    /// advance rotation cursors or expose Vault locators to ordinary snapshots.
    fn credential_pool_materials(
        &self,
        _pool_id: String,
        _pool_revision: u32,
        _expected_kind: crate::domain::model_config::credentials::CredentialMaterialKind,
    ) -> AppResult<Vec<crate::domain::credentials::CredentialLeaseMetadata>> {
        Err(AppError::new("credential_inspection_unavailable", "credentials", "当前凭据仓库不支持服务准备检查"))
    }
    fn begin_credential_material(
        &self,
        reference: CredentialReference,
        storage: crate::domain::model_config::credentials::CredentialStorageMetadata,
        now_ms: i64,
    ) -> AppResult<()>;
    fn snapshot(&self) -> AppResult<AppSnapshot>;
    fn dispatch(&self, envelope: CommandEnvelope, now_ms: i64) -> AppResult<CommandReceipt>;
    fn prepare_tasks(
        &self,
        unit_id: String,
        text_revision: u32,
        purpose: String,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::TaskDispatch>>;
    fn complete_task(
        &self,
        result: crate::domain::TaskCompletion,
        now_ms: i64,
    ) -> AppResult<CommandReceipt>;
    fn validate_task_authority(&self, task_id: String) -> AppResult<()>;
    /// Start a bounded, explicitly configured retry of the same frozen engine
    /// and immutable credential. Each wire attempt has its own durable identity.
    fn retry_attempt(
        &self,
        task_id: String,
        attempt_id: String,
        error_code: String,
        now_ms: i64,
    ) -> AppResult<String>;
    fn fail_task(&self, task_id: String, code: String, now_ms: i64) -> AppResult<()>;
    fn open_hook_session(
        &self,
        input: crate::domain::capture::HookSessionInput,
        now_ms: i64,
    ) -> AppResult<()>;
    fn receive_hook_text(
        &self,
        input: crate::domain::capture::HookTextInput,
        now_ms: i64,
    ) -> AppResult<Option<String>> {
        self.ingest_hook_text(input, now_ms)
            .map(|outcome| outcome.visible_version.map(|version| version.unit_id))
    }
    fn ingest_hook_text(
        &self,
        input: crate::domain::capture::HookTextInput,
        now_ms: i64,
    ) -> AppResult<crate::domain::capture::CaptureOutcome>;
    fn flush_hook_capture(
        &self,
        request: crate::domain::capture::CaptureFlush,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::capture::CaptureVersion>>;
    fn prepare_capture_tasks(
        &self,
        version: crate::domain::capture::CaptureVersion,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::TaskDispatch>>;
    fn set_tokens(
        &self,
        input: crate::domain::capture::TokenSnapshotInput,
        now_ms: i64,
    ) -> AppResult<()>;
    /// Explicit offline retry may acknowledge an identical immutable analysis;
    /// the ordinary insertion port remains strict about duplicate identities.
    fn set_tokens_for_retry(
        &self,
        input: crate::domain::capture::TokenSnapshotInput,
        now_ms: i64,
    ) -> AppResult<()> {
        self.set_tokens(input, now_ms)
    }
    fn record_diagnostic(&self, diagnostic: Diagnostic, now_ms: i64) -> AppResult<()>;
    /// Native lifecycle facts must name their originating session. An absent
    /// SID is informational and must never select the latest Hook session.
    fn record_hook_diagnostic(
        &self,
        _session_id: Option<String>,
        _diagnostic: Diagnostic,
        _now_ms: i64,
    ) -> AppResult<()> {
        Err(AppError::new(
            "capture_diagnostic_unavailable",
            "capture",
            "存储未提供会话归属诊断",
        ))
    }
    fn begin_credential(&self, reference: CredentialReference, now_ms: i64) -> AppResult<()>;
    fn activate_credential(&self, credential_id: String, now_ms: i64) -> AppResult<()>;
    fn revoke_credential(&self, credential_id: String, now_ms: i64) -> AppResult<()>;
    fn mark_credential_cleaned(&self, credential_id: String, now_ms: i64) -> AppResult<()>;
    fn pending_credentials(&self) -> AppResult<Vec<CredentialReference>>;
    fn register_credential(&self, reference: CredentialReference, now_ms: i64) -> AppResult<()>;
    fn credential_for_pool(&self, pool_id: String, pool_revision: u32) -> AppResult<String>;
    fn history(
        &self,
        game_id: Option<String>,
        cursor: Option<String>,
        limit: u32,
    ) -> AppResult<HistoryPage>;
    fn history_page(&self, query: HistoryQuery) -> AppResult<HistoryPage>;
    fn history_sessions(&self, query: HistorySessionsQuery) -> AppResult<HistorySessionsPage>;
}
fn session_records_unavailable() -> AppError {
    AppError::new(
        "session_records_unavailable",
        "session_records",
        "当前存储适配器不支持会话全量记录",
    )
}
pub trait IdentitySource: Send + Sync {
    fn new_id(&self, prefix: &str) -> String;
    fn now_ms(&self) -> i64;
}
