//! Domain-layer tests call the real ApplicationRuntime; low-level ports are
//! controlled in-memory adapters. Database and Native behavior have separate gates.
use app_core::{
    application::{binding_journey::*, runtime::ApplicationRuntime},
    contracts::*,
    domain::{self, capture::*},
    ports::{runtime::*, *},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};
fn unused() -> AppError {
    AppError::new("unused_test_port", "test", "port outside this domain test")
}
fn context(s: &AppSnapshot) -> BindingProfileContext {
    BindingProfileContext {
        profile_id: s.profile_id.clone(),
        server_instance_id: s.server_instance_id.clone(),
        profile_generation: s.profile_generation,
    }
}
fn snapshot() -> AppSnapshot {
    AppSnapshot {
        lexical_states: vec![],
        profile_id: "profile".into(),
        server_instance_id: "server".into(),
        profile_generation: 1,
        revision: 0,
        commit_revision: 1,
        games: vec![],
        session: None,
        current: None,
        history: HistoryPage {
            units: vec![],
            next_cursor: None,
        },
        configuration: ConfigurationState {
            draft: ConfigurationDocument::Legacy {
                configuration: Configuration::default(),
            },
            applied: None,
            draft_head_revision: 0,
            applied_head_revision: 0,
            runtime_connections: vec![],
            runtime_purposes: vec![],
        },
        preferences: Preferences::default(),
        diagnostics: vec![],
        tasks: vec![],
        study_results: vec![],
        credentials: vec![],
        credential_pools: vec![],
        configuration_tests: vec![],
        learning_object: None,
    }
}
fn candidate(id: &str, pid: u32) -> TargetCandidate {
    TargetCandidate {
        candidate_id: id.into(),
        window_title: "same display title".into(),
        executable_path: format!("C:\\Games\\{id}.exe"),
        windows_pid: pid,
        process_created_u64: "18446744073709551615".into(),
        architecture: "x64".into(),
        environment_key: "crossover:/bottle".into(),
        bottle_label: "bottle".into(),
        has_window: true,
        window_handle: Some("800".into()),
        macos_host_pid: None,
        macos_window_id: None,
    }
}
fn game(c: &TargetCandidate) -> Game {
    Game {
        game_id: "new-game".into(),
        display_name: "saved game".into(),
        original_title: None,
        source_language: Some("ja".into()),
        cover_asset_ref: None,
        row_revision: 1,
        target: GameTarget {
            environment_kind: "crossover".into(),
            environment_key: c.environment_key.clone(),
            bottle_locator: Some("bottle".into()),
            launch_path: Some(c.executable_path.clone()),
            canonical_exe_path: Some(c.executable_path.clone()),
            target_path_key: Some(
                domain::normalize_windows_path(&c.executable_path)
                    .unwrap()
                    .1,
            ),
            working_directory: None,
            arguments: vec![],
            architecture: Some(c.architecture.clone()),
            verification_state: "verified".into(),
        },
    }
}
fn session(c: &TargetCandidate, id: &str) -> HookSessionInput {
    HookSessionInput {
        session_id: id.into(),
        game_id: None,
        environment_key: c.environment_key.clone(),
        target_exe_path: c.executable_path.clone(),
        windows_pid: c.windows_pid,
        process_created_u64: c.process_created_u64.clone(),
        architecture: c.architecture.clone(),
        broker_instance_id: format!("host-{id}"),
        resource: ResourceSnapshotInput {
            snapshot_id: "resource".into(),
            kind: "core".into(),
            version: "1".into(),
            manifest_sha256: "a".repeat(64),
            contract_version: "1".into(),
            metadata_json: "{}".into(),
        },
        target_identity_sha256: domain::content_hash(format!(
            "{}\0{}\0{}\0{}\0{}",
            c.environment_key,
            c.executable_path,
            c.windows_pid,
            c.process_created_u64,
            c.architecture
        )),
    }
}
fn source(id: &str) -> SourceDetailView {
    SourceDetailView {
        source_id: id.into(),
        hook_name: "TextOutW".into(),
        hook_code: "HWN@game".into(),
        address: Some("18446744073709551615".into()),
        context: Some("9007199254740993".into()),
        context2: Some("0".into()),
        encoding: "utf-16".into(),
        latest_sample: "本文の完全なサンプル。".repeat(120),
        stream_identity_sha256: domain::content_hash(id),
        identity_extensions_json: "{}".into(),
    }
}
#[derive(Default)]
struct Memory {
    snapshot: Option<AppSnapshot>,
    sets: Option<SourceSelectionSet>,
    saved: Option<SavedSourceSelection>,
    commits: BTreeMap<String, (String, SelectionCommitReceipt)>,
    operations: BTreeMap<String, AssociationOperationFacts>,
    revision: u32,
}
struct Repo(Mutex<Memory>);
impl Repo {
    fn new() -> Self {
        Self(Mutex::new(Memory {
            snapshot: Some(snapshot()),
            ..Default::default()
        }))
    }
}
impl ProductRepository for Repo {
    fn snapshot(&self) -> AppResult<AppSnapshot> {
        Ok(self.0.lock().unwrap().snapshot.clone().unwrap())
    }
    fn open_hook_session(&self, input: HookSessionInput, _now: i64) -> AppResult<()> {
        let mut m = self.0.lock().unwrap();
        let s = m.snapshot.as_mut().unwrap();
        if s.session
            .as_ref()
            .is_none_or(|s| s.session_id != input.session_id)
        {
            s.session = Some(Session {
                session_id: input.session_id,
                game_id: input.game_id,
                source_kind: "hook".into(),
                state: "active".into(),
                paused: false,
                translation_enabled: false,
                sources: vec![],
            });
            m.sets = None;
        }
        Ok(())
    }
    fn flush_capture_integrity(&self, _session: Option<String>) -> AppResult<()> {
        Ok(())
    }
    fn begin_capture_operation(&self, id: String, hash: String, now: i64) -> AppResult<()> {
        let mut m = self.0.lock().unwrap();
        if let Some(op) = m.operations.get(&id) {
            if op.request_sha256 != hash {
                return Err(AppError::new("operation_conflict", "test", "conflict"));
            }
            return Ok(());
        }
        let profile = context(m.snapshot.as_ref().unwrap());
        m.operations.insert(
            id.clone(),
            AssociationOperationFacts {
                profile,
                operation_id: id,
                request_sha256: hash,
                session_id: None,
                started_at_ms: now,
                finished_at_ms: None,
                events: vec![],
            },
        );
        Ok(())
    }
    fn finish_capture_operation(&self, id: String, sid: Option<String>, now: i64) -> AppResult<()> {
        let mut m = self.0.lock().unwrap();
        let op = m.operations.get_mut(&id).unwrap();
        op.finished_at_ms = sid.is_none().then_some(now);
        op.session_id = sid;
        Ok(())
    }
    fn record_hook_event(&self, e: HookEventInput) -> AppResult<()> {
        let mut m = self.0.lock().unwrap();
        if let Some(op) = e.operation_id.and_then(|id| m.operations.get_mut(&id)) {
            op.events.push(AssociationJournalFact {
                event_id: e.event_id,
                ingest_sequence: (op.events.len() + 1).to_string(),
                session_id: e.session_id,
                observed_at_ms: e.observed_at_ms,
                level: e.detail.level,
                stage: e.detail.stage,
                code: e.detail.code,
                message: e.detail.message,
            });
        }
        Ok(())
    }
    fn association_progress_facts(&self, id: &str) -> AppResult<AssociationOperationFacts> {
        self.0
            .lock()
            .unwrap()
            .operations
            .get(id)
            .cloned()
            .ok_or_else(unused)
    }
    fn source_selection(&self, q: SourceSelectionQuery) -> AppResult<SourceSelectionProjection> {
        let m = self.0.lock().unwrap();
        let s = m.snapshot.as_ref().unwrap();
        if q.expected_profile_generation != s.profile_generation {
            return Err(unused());
        }
        Ok(SourceSelectionProjection {
            profile: context(s),
            session_id: q.session_id,
            state: if m.sets.is_some() {
                SourceSelectionState::Current
            } else if m.saved.is_some() {
                SourceSelectionState::RequiresReview
            } else {
                SourceSelectionState::Unselected
            },
            current: m.sets.clone(),
            sources: vec![source("a"), source("b")],
            saved: m.saved.clone(),
            remember_allowed: s.session.as_ref().is_some_and(|s| s.game_id.is_some()),
            member_limit: 64,
        })
    }
    fn commit_source_selection(
        &self,
        input: SourceSelectionCommit,
        now: i64,
    ) -> AppResult<SelectionCommitReceipt> {
        let mut m = self.0.lock().unwrap();
        let hash = domain::canonical_hash(&input.request)?;
        if let Some((old, receipt)) = m.commits.get(&input.request.operation_id) {
            if old != &hash {
                return Err(AppError::new("operation_conflict", "test", "conflict"));
            }
            let mut receipt = receipt.clone();
            receipt.replayed = true;
            return Ok(receipt);
        }
        if input.request.expected_set_revision != m.sets.as_ref().map(|s| s.set_revision.clone()) {
            return Err(AppError::new("selection_conflict", "test", "head changed"));
        }
        m.revision += 1;
        let rev = m.revision;
        let set = SourceSelectionSet {
            session_id: input.request.session_id.clone(),
            set_revision: rev.to_string(),
            operation_id: Some(input.request.operation_id.clone()),
            persistence: if input.request.remember {
                SelectionPersistence::SavedSnapshot
            } else {
                SelectionPersistence::Temporary
            },
            selected_at_ms: now.to_string(),
            members: input
                .request
                .source_ids
                .iter()
                .enumerate()
                .map(|(i, id)| SourceSelectionMember {
                    source_id: id.clone(),
                    selection_revision: rev,
                    ordinal: i as u32,
                })
                .collect(),
        };
        let receipt = SelectionCommitReceipt {
            operation_id: input.request.operation_id.clone(),
            profile: input.profile.clone(),
            selection: set.clone(),
            replayed: false,
            effect_authority: ReadingWindowEffectAuthority {
                profile: input.profile,
                session_id: set.session_id.clone(),
                set_revision: set.set_revision.clone(),
                operation_id: input.request.operation_id.clone(),
            },
        };
        m.sets = Some(set.clone());
        if input.request.remember {
            m.saved = Some(SavedSourceSelection {
                set,
                matcher_version: Some(1),
                state: SourceSelectionState::RequiresReview,
                reason: "no portable proof".into(),
                sources: vec![source("a")],
                match_snapshot_json: input.match_snapshot_json,
            });
        }
        m.commits
            .insert(input.request.operation_id, (hash, receipt.clone()));
        Ok(receipt)
    }
    fn validate_reading_window_effect(
        &self,
        a: ReadingWindowEffectAuthority,
    ) -> AppResult<SelectionCommitReceipt> {
        let m = self.0.lock().unwrap();
        if m.sets.as_ref().is_none_or(|s| {
            s.set_revision != a.set_revision || s.operation_id.as_ref() != Some(&a.operation_id)
        }) {
            return Err(AppError::new("reading_effect_stale", "test", "stale"));
        }
        let receipt = m.commits.get(&a.operation_id).unwrap().1.clone();
        if receipt.effect_authority != a {
            return Err(unused());
        }
        Ok(receipt)
    }
    #[allow(unused_variables)]
    fn unit(&self, unit_id: String, text_revision: u32) -> AppResult<ReadingUnit> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn reading_unit_projection(
        &self,
        unit_id: String,
        text_revision: u32,
    ) -> AppResult<ReadingUnitProjection> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn prepare_tasks_for_operation(
        &self,
        unit_id: String,
        text_revision: u32,
        purpose: String,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::TaskDispatch>> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn prepare_uncached_translation_for_operation(
        &self,
        unit_id: String,
        text_revision: u32,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<crate::domain::PreparedTranslation> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn prepare_study_tasks(
        &self,
        unit_id: String,
        text_revision: u32,
        purpose: String,
        token_index: Option<u32>,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::TaskDispatch>> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn record_model_test(
        &self,
        input: crate::domain::testing::ModelTestRecordInput,
    ) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn credential_test_reference(
        &self,
        pool_id: String,
        pool_revision: u32,
        vault_ref: String,
        outcome: String,
    ) -> AppResult<crate::domain::testing::TestCredentialUse> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn maintain_storage(
        &self,
        policy: crate::domain::retention::RetentionPolicy,
        protected: crate::domain::retention::ProtectedObjects,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<crate::domain::retention::MaintenanceReceipt> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn select_credential(
        &self,
        pool_id: String,
        pool_revision: u32,
    ) -> AppResult<crate::domain::credentials::CredentialSelection> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn store_provider_preset(&self, preset: StoredProviderPreset, now_ms: i64) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn provider_preset(&self, key: ProviderPresetKey) -> AppResult<StoredProviderPreset> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn select_credential_for_kind(
        &self,
        pool_id: String,
        pool_revision: u32,
        expected_kind: crate::domain::model_config::credentials::CredentialMaterialKind,
    ) -> AppResult<crate::domain::credentials::CredentialSelection> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn credential_lease_metadata(
        &self,
        pool_id: String,
        pool_revision: u32,
        vault_ref: String,
        expected_kind: crate::domain::model_config::credentials::CredentialMaterialKind,
    ) -> AppResult<crate::domain::credentials::CredentialLeaseMetadata> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn begin_credential_material(
        &self,
        reference: CredentialReference,
        storage: crate::domain::model_config::credentials::CredentialStorageMetadata,
        now_ms: i64,
    ) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn dispatch(&self, envelope: CommandEnvelope, now_ms: i64) -> AppResult<CommandReceipt> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn prepare_tasks(
        &self,
        unit_id: String,
        text_revision: u32,
        purpose: String,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::TaskDispatch>> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn complete_task(
        &self,
        result: crate::domain::TaskCompletion,
        now_ms: i64,
    ) -> AppResult<CommandReceipt> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn validate_task_authority(&self, task_id: String) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn retry_attempt(
        &self,
        task_id: String,
        attempt_id: String,
        error_code: String,
        now_ms: i64,
    ) -> AppResult<String> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn fail_task(&self, task_id: String, code: String, now_ms: i64) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn ingest_hook_text(
        &self,
        input: crate::domain::capture::HookTextInput,
        now_ms: i64,
    ) -> AppResult<crate::domain::capture::CaptureOutcome> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn flush_hook_capture(
        &self,
        request: crate::domain::capture::CaptureFlush,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::capture::CaptureVersion>> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn prepare_capture_tasks(
        &self,
        version: crate::domain::capture::CaptureVersion,
        operation_id: String,
        now_ms: i64,
    ) -> AppResult<Vec<crate::domain::TaskDispatch>> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn set_tokens(
        &self,
        input: crate::domain::capture::TokenSnapshotInput,
        now_ms: i64,
    ) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn record_diagnostic(&self, diagnostic: Diagnostic, now_ms: i64) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn begin_credential(&self, reference: CredentialReference, now_ms: i64) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn activate_credential(&self, credential_id: String, now_ms: i64) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn revoke_credential(&self, credential_id: String, now_ms: i64) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn mark_credential_cleaned(&self, credential_id: String, now_ms: i64) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn pending_credentials(&self) -> AppResult<Vec<CredentialReference>> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn register_credential(&self, reference: CredentialReference, now_ms: i64) -> AppResult<()> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn credential_for_pool(&self, pool_id: String, pool_revision: u32) -> AppResult<String> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn history(
        &self,
        game_id: Option<String>,
        cursor: Option<String>,
        limit: u32,
    ) -> AppResult<HistoryPage> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn history_page(&self, query: HistoryQuery) -> AppResult<HistoryPage> {
        Err(unused())
    }
    #[allow(unused_variables)]
    fn history_sessions(&self, query: HistorySessionsQuery) -> AppResult<HistorySessionsPage> {
        Err(unused())
    }
}
struct Hook {
    candidates: Mutex<BTreeMap<String, TargetCandidate>>,
    active: Mutex<Option<HookSessionInput>>,
    native_lease: Mutex<()>,
    next_guard_active: Mutex<Option<HookSessionInput>>,
    resource_path: Mutex<Option<std::path::PathBuf>>,
    target_lease_path: Mutex<Option<std::path::PathBuf>>,
    stops: AtomicUsize,
    injections: AtomicUsize,
    fail: Mutex<bool>,
}
impl Hook {
    fn new() -> Self {
        Self {
            candidates: Mutex::new(
                [
                    ("one".into(), candidate("one", 1)),
                    ("two".into(), candidate("two", 2)),
                ]
                .into(),
            ),
            active: Mutex::new(None),
            native_lease: Mutex::new(()),
            next_guard_active: Mutex::new(None),
            resource_path: Mutex::new(None),
            target_lease_path: Mutex::new(None),
            stops: AtomicUsize::new(0),
            injections: AtomicUsize::new(0),
            fail: Mutex::new(false),
        }
    }
}
impl HookBackend for Hook {
    fn targets(&self) -> AppResult<TargetCandidates> {
        Ok(TargetCandidates {
            candidates: self.candidates.lock().unwrap().values().cloned().collect(),
            diagnostics: vec![],
        })
    }
    fn association_candidate(&self, id: &str) -> AppResult<TargetCandidate> {
        self.candidates
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(unused)
    }
    fn association_context(&self, id: &str) -> AppResult<AssociationBackendContext> {
        let _l = self.native_lease.lock().unwrap();
        Ok(AssociationBackendContext {
            candidate: self.association_candidate(id)?,
            active: self.active.lock().unwrap().clone(),
        })
    }
    fn active_session_context(&self) -> AppResult<Option<HookSessionInput>> {
        Ok(self.active.lock().unwrap().clone())
    }
    fn with_active_session_guard(
        &self,
        expected: &HookSessionInput,
        effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let _l = self.native_lease.lock().unwrap();
        if let Some(replacement) = self.next_guard_active.lock().unwrap().take() {
            *self.active.lock().unwrap() = Some(replacement);
        }
        if self.active.lock().unwrap().as_ref().is_none_or(|s| {
            s.session_id != expected.session_id
                || s.broker_instance_id != expected.broker_instance_id
                || s.target_identity_sha256 != expected.target_identity_sha256
                || s.environment_key != expected.environment_key
                || s.target_exe_path != expected.target_exe_path
                || s.windows_pid != expected.windows_pid
                || s.process_created_u64 != expected.process_created_u64
                || s.architecture != expected.architecture
                || s.game_id != expected.game_id
                || s.resource.snapshot_id != expected.resource.snapshot_id
                || s.resource.kind != expected.resource.kind
                || s.resource.version != expected.resource.version
                || s.resource.manifest_sha256 != expected.resource.manifest_sha256
                || s.resource.contract_version != expected.resource.contract_version
                || s.resource.metadata_json != expected.resource.metadata_json
        }) {
            return Err(unused());
        }
        effect()
    }
    fn with_no_active_session_guard(
        &self,
        effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<()> {
        let _lease = self.native_lease.lock().unwrap();
        if let Some(replacement) = self.next_guard_active.lock().unwrap().take() {
            *self.active.lock().unwrap() = Some(replacement);
        }
        if self.active.lock().unwrap().is_some() {
            return Err(AppError::new(
                "inactive_session_preview_stale",
                "capture",
                "实际宿主已建立",
            ));
        }
        effect()
    }
    fn associate(&self, _r: AssociateRequest) -> AppResult<HookSessionInput> {
        Err(unused())
    }
    fn associate_reviewed_traced(
        &self,
        r: AssociateRequest,
        g: &AssociationExecutionGuard,
        record: &mut dyn FnMut(HookStageInput) -> AppResult<()>,
    ) -> AppResult<HookSessionInput> {
        let _l = self.native_lease.lock().unwrap();
        let c = self.association_candidate(&r.candidate_id)?;
        let mut active = self.active.lock().unwrap();
        if let Some(replacement) = self.next_guard_active.lock().unwrap().take() {
            *active = Some(replacement);
        }
        let full_matches = match (g.expected_active.as_ref(), active.as_ref()) {
            (Some(expected), Some(current)) => same_hook_session_facts(expected, current),
            (None, None) => true,
            _ => false,
        };
        if !full_matches
            || active.as_ref().map(|s| s.session_id.as_str())
                != g.expected_active_session_id.as_deref()
        {
            return Err(unused());
        }
        let input = session(&c, &format!("sid-{}", r.candidate_id));
        if input.target_identity_sha256 != g.candidate_identity_sha256 {
            return Err(unused());
        }
        if let Some(old) = active
            .as_ref()
            .filter(|s| s.target_identity_sha256 == input.target_identity_sha256)
        {
            return Ok(old.clone());
        }
        if let Some(path) = self.resource_path.lock().unwrap().clone() {
            std::fs::read(path).map_err(|_| {
                AppError::new(
                    "core_resources_missing",
                    "resource",
                    "实际受控资源文件不可读，保留当前会话",
                )
            })?;
        }
        let reserved = if let Some(path) = self.target_lease_path.lock().unwrap().clone() {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path)
                .map_err(|_| AppError::new("target_lock", "capture", "无法打开实际受控目标租约"))?;
            file.try_lock().map_err(|_| {
                AppError::new("target_in_use", "capture", "实际受控目标租约已被占用")
            })?;
            Some(file)
        } else {
            None
        };
        if *self.fail.lock().unwrap() {
            return Err(AppError::new(
                "injection_failed",
                "test",
                "controlled native failure",
            ));
        }
        record(HookStageInput {
            level: "info".into(),
            stage: "injection".into(),
            code: "injection_requested".into(),
            message: "actual controlled port called".into(),
        })?;
        if active.is_some() {
            self.stops.fetch_add(1, Ordering::SeqCst);
        }
        self.injections.fetch_add(1, Ordering::SeqCst);
        *active = Some(input.clone());
        drop(reserved);
        Ok(input)
    }
    fn disconnect(&self) -> AppResult<()> {
        *self.active.lock().unwrap() = None;
        Ok(())
    }
    fn subscribe(&self) -> AppResult<mpsc::Receiver<HookUpdate>> {
        Ok(mpsc::channel().1)
    }
    fn resource_info(&self) -> ResourceInfo {
        ResourceInfo {
            kind: "core".into(),
            version: "test".into(),
            available: true,
            detail: String::new(),
            manifest_sha256: "a".repeat(64),
        }
    }
}
struct Auxiliary(AtomicUsize);
impl IdentitySource for Auxiliary {
    fn new_id(&self, p: &str) -> String {
        format!("{p}-{}", self.0.fetch_add(1, Ordering::SeqCst))
    }
    fn now_ms(&self) -> i64 {
        100
    }
}
impl ModelGateway for Auxiliary {
    fn execute<'a>(
        &'a self,
        _t: &'a domain::TaskDispatch,
        _p: &'a app_core::application::request_plan::RequestPlan,
        _d: Arc<dyn Fn(String) + Send + Sync>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Err(unused()) })
    }
    fn test_request<'a>(
        &'a self,
        _c: &'a Connection,
        _p: &'a app_core::application::request_plan::RequestPlan,
        _r: Option<&'a str>,
    ) -> PortFuture<'a, String> {
        Box::pin(async { Err(unused()) })
    }
    fn discover_models<'a>(
        &'a self,
        _c: &'a Connection,
        _r: Option<&'a str>,
    ) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { Err(unused()) })
    }
}
impl LanguageServices for Auxiliary {
    fn analyze<'a>(&'a self, _t: &'a str) -> PortFuture<'a, Vec<LexicalToken>> {
        Box::pin(async { Err(unused()) })
    }
    fn lookup(&self, _t: &LexicalToken) -> AppResult<Vec<DictionaryEntry>> {
        Err(unused())
    }
    fn resources(&self) -> Vec<ResourceInfo> {
        vec![]
    }
}
impl CredentialVault for Auxiliary {
    fn put(&self, _id: &str, _s: &[u8]) -> AppResult<()> {
        Err(unused())
    }
    fn get(&self, _id: &str) -> AppResult<Vec<u8>> {
        Err(unused())
    }
    fn delete(&self, _id: &str) -> AppResult<()> {
        Err(unused())
    }
}
struct Harness {
    app: Arc<ApplicationRuntime>,
    repo: Arc<Repo>,
    hook: Arc<Hook>,
}
impl Harness {
    fn new() -> Self {
        let repo = Arc::new(Repo::new());
        let hook = Arc::new(Hook::new());
        let aux = Arc::new(Auxiliary(AtomicUsize::new(1)));
        let app = ApplicationRuntime::new(
            repo.clone(),
            aux.clone(),
            hook.clone(),
            aux.clone(),
            aux.clone(),
            aux,
        )
        .unwrap();
        Self { app, repo, hook }
    }
    fn review(&self, id: &str) -> AssociationReview {
        let s = self.app.snapshot().unwrap();
        self.app
            .review_association(AssociationReviewQuery {
                candidate_id: id.into(),
                game_id: None,
                expected_profile_generation: 1,
                expected_active_session_id: s.session.map(|s| s.session_id),
                prior_review_id: None,
                attribution_intent: None,
            })
            .unwrap()
    }
    fn connect(&self, id: &str, op: &str) -> AssociationOperationReceipt {
        let review = self.review(id);
        self.app
            .associate_reviewed(ReviewedAssociateRequest {
                operation_id: op.into(),
                review_id: review.review_id,
            })
            .unwrap()
    }
    fn selection(&self, op: &str, ids: &[&str], remember: bool) -> BeginReadingRequest {
        BeginReadingRequest {
            operation_id: op.into(),
            session_id: self.app.snapshot().unwrap().session.unwrap().session_id,
            expected_profile_generation: 1,
            expected_set_revision: self
                .repo
                .0
                .lock()
                .unwrap()
                .sets
                .as_ref()
                .map(|s| s.set_revision.clone()),
            source_ids: ids.iter().map(|s| (*s).into()).collect(),
            remember,
        }
    }
}
#[test]
fn bj_02_domain() {
    library_attribution_intent_regressions();
    let h = Harness::new();
    assert_eq!(h.review("one").relation, AssociationRelation::None);
    let first = h.connect("one", "op-one");
    let mut c = h.hook.candidates.lock().unwrap();
    c.get_mut("two").unwrap().window_title = "same display title".into();
    drop(c);
    assert_eq!(h.review("one").relation, AssociationRelation::Reuse);
    assert_eq!(h.review("two").relation, AssociationRelation::Switch);
    let legacy_error = h
        .app
        .associate(AssociateRequest {
            operation_id: "legacy-switch".into(),
            game_id: None,
            candidate_id: "two".into(),
            confirm_switch: true,
        })
        .unwrap_err();
    assert_eq!(legacy_error.code, "reviewed_association_required");
    assert_eq!(h.hook.injections.load(Ordering::SeqCst), 1);
    let reused = h.connect("one", "op-reuse");
    assert_eq!(first.session_id, reused.session_id);
    assert_eq!(h.hook.injections.load(Ordering::SeqCst), 1);
}
#[test]
fn bj_05_domain() {
    reviewed_full_active_witness_regressions();
    let h = Harness::new();
    let review = h.review("one");
    h.connect("two", "op-two");
    let e = h
        .app
        .associate_reviewed(ReviewedAssociateRequest {
            operation_id: "stale".into(),
            review_id: review.review_id,
        })
        .unwrap_err();
    assert_eq!(e.code, "association_review_stale");
    assert_eq!(
        h.hook.active.lock().unwrap().as_ref().unwrap().session_id,
        "sid-two"
    );
    assert_eq!(h.hook.injections.load(Ordering::SeqCst), 1);
    let next = Harness::new();
    let original = next.review("one");
    next.repo
        .0
        .lock()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .games
        .push(game(&candidate("one", 1)));
    let saved = next
        .app
        .review_association(AssociationReviewQuery {
            candidate_id: "one".into(),
            game_id: Some("new-game".into()),
            expected_profile_generation: 1,
            expected_active_session_id: None,
            prior_review_id: Some(original.review_id.clone()),
            attribution_intent: None,
        })
        .unwrap();
    assert_eq!(saved.relation, AssociationRelation::None);
    next.connect("two", "another");
    assert!(
        next.app
            .review_association(AssociationReviewQuery {
                candidate_id: "one".into(),
                game_id: Some("new-game".into()),
                expected_profile_generation: 1,
                expected_active_session_id: Some("sid-two".into()),
                prior_review_id: Some(original.review_id),
                attribution_intent: None,
            })
            .is_err()
    );
    let switching = next.review("one");
    next.hook
        .active
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .broker_instance_id = "replaced-broker-same-sid".into();
    assert!(
        next.app
            .review_association(AssociationReviewQuery {
                candidate_id: "one".into(),
                game_id: Some("new-game".into()),
                expected_profile_generation: 1,
                expected_active_session_id: Some("sid-two".into()),
                prior_review_id: Some(switching.review_id),
                attribution_intent: None,
            })
            .is_err()
    );
}

fn reviewed_full_active_witness_regressions() {
    let mutations: [fn(&mut HookSessionInput); 7] = [
        |s| s.game_id = Some("changed-game-owner".into()),
        |s| s.resource.snapshot_id.push_str("-changed"),
        |s| s.resource.kind.push_str("-changed"),
        |s| s.resource.version.push_str("-changed"),
        |s| s.resource.manifest_sha256 = "f".repeat(64),
        |s| s.resource.contract_version.push_str("-changed"),
        |s| s.resource.metadata_json = "{\"changed\":true}".into(),
    ];
    for change_under_native_lease in [false, true] {
        for mutate in mutations {
            let h = Harness::new();
            let old_receipt = h.connect("one", "old-complete-owner");
            let review = h.review("two");
            let mut changed = h.hook.active.lock().unwrap().clone().unwrap();
            mutate(&mut changed);
            let request = ReviewedAssociateRequest {
                operation_id: "switch-with-changed-owner".into(),
                review_id: review.review_id,
            };
            if change_under_native_lease {
                // The read-only preparation sees the old valid context; the
                // controlled Native adapter changes it only after taking its
                // final execution lease, before any host effect.
                h.app.association_preview_intent(&request).unwrap();
                *h.hook.next_guard_active.lock().unwrap() = Some(changed);
            } else {
                *h.hook.active.lock().unwrap() = Some(changed);
                assert_eq!(
                    h.app.association_preview_intent(&request).unwrap_err().code,
                    "association_review_stale"
                );
            }
            assert!(h.app.associate_reviewed(request).is_err());
            assert_eq!(h.hook.stops.load(Ordering::SeqCst), 0);
            assert_eq!(h.hook.injections.load(Ordering::SeqCst), 1);
            let mut callbacks = 0;
            assert!(
                h.app
                    .with_association_preview_authority(&old_receipt, &mut |_, _| {
                        callbacks += 1;
                        Ok(())
                    })
                    .is_err()
            );
            assert_eq!(callbacks, 0);
        }
    }
}
#[test]
fn bj_06_domain() {
    let h = Harness::new();
    h.connect("one", "op");
    let p = h
        .app
        .association_progress(AssociationProgressQuery {
            operation_id: "op".into(),
            expected_profile_generation: 1,
        })
        .unwrap();
    assert_eq!(p.stage, AssociationStage::WaitingHandshake);
    assert!(p.facts.iter().any(|f| f.code == "injection_requested"));
    assert!(
        !p.facts
            .iter()
            .any(|f| f.stage == AssociationStage::CandidatesReady)
    );
    h.connect("two", "new");
    assert!(
        h.app
            .association_progress(AssociationProgressQuery {
                operation_id: "op".into(),
                expected_profile_generation: 1
            })
            .unwrap()
            .superseded
    );
}
#[test]
fn bj_07_domain() {
    inactive_preview_authority_regressions();
    current_preview_origin_regressions();
    current_preview_lifecycle_observations();
    let h = Harness::new();
    h.connect("one", "op");
    for _ in 0..3 {
        let p = h
            .app
            .source_selection(SourceSelectionQuery {
                session_id: "sid-one".into(),
                expected_profile_generation: 1,
            })
            .unwrap();
        assert!(p.current.is_none());
        assert_eq!(p.sources.len(), 2);
    }
    assert!(h.repo.0.lock().unwrap().commits.is_empty());
}

fn inactive_preview_authority_regressions() {
    let h = Harness::new();
    let published = h.app.subscribe().borrow().revision;
    let none = h.app.inactive_session_preview_authority().unwrap().unwrap();
    assert_eq!(
        h.app.subscribe().borrow().revision,
        published,
        "inactive query must not publish"
    );
    let mut callbacks = 0;
    h.app
        .with_inactive_session_preview_authority(&none, &mut || {
            callbacks += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(callbacks, 1);

    // The old None observation is released to a later Core association before
    // its cleanup callback is attempted. A genuine new session must win.
    let (observed_tx, observed_rx) = mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = mpsc::sync_channel(1);
    let observer_runtime = h.app.clone();
    let stale_effects = Arc::new(AtomicUsize::new(0));
    let effects = stale_effects.clone();
    let observer = std::thread::spawn(move || {
        let witness = observer_runtime
            .inactive_session_preview_authority()
            .unwrap()
            .unwrap();
        observed_tx.send(()).unwrap();
        resume_rx
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        observer_runtime.with_inactive_session_preview_authority(&witness, &mut || {
            effects.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    });
    observed_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    let active = h.connect("two", "new-after-inactive-observation");
    let before = serde_json::to_value(h.app.snapshot().unwrap()).unwrap();
    resume_tx.send(()).unwrap();
    assert!(observer.join().unwrap().is_err());
    assert_eq!(stale_effects.load(Ordering::SeqCst), 0);
    assert!(
        h.app
            .inactive_session_preview_authority()
            .unwrap()
            .is_none()
    );
    assert!(
        h.app
            .with_inactive_session_preview_authority(&none, &mut || {
                callbacks += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(callbacks, 1);
    assert_eq!(
        serde_json::to_value(h.app.snapshot().unwrap()).unwrap(),
        before
    );
    assert_eq!(
        h.app.snapshot().unwrap().session.unwrap().session_id,
        active.session_id.unwrap()
    );
    assert_eq!(h.hook.stops.load(Ordering::SeqCst), 0);

    // A Native owner appearing between the repository read and final lease
    // check also rejects cleanup, even before its SQL projection is published.
    let native_race = Harness::new();
    let none = native_race
        .app
        .inactive_session_preview_authority()
        .unwrap()
        .unwrap();
    *native_race.hook.next_guard_active.lock().unwrap() =
        Some(session(&candidate("two", 2), "native-race-session"));
    let mut effects = 0;
    assert!(
        native_race
            .app
            .with_inactive_session_preview_authority(&none, &mut || {
                effects += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(effects, 0);
    assert_eq!(native_race.hook.stops.load(Ordering::SeqCst), 0);

    for mutate in [
        (|s: &mut AppSnapshot| s.profile_id.push_str("-new")) as fn(&mut AppSnapshot),
        |s: &mut AppSnapshot| s.server_instance_id.push_str("-new"),
        |s: &mut AppSnapshot| s.profile_generation += 1,
    ] {
        let changed = Harness::new();
        let none = changed
            .app
            .inactive_session_preview_authority()
            .unwrap()
            .unwrap();
        mutate(changed.repo.0.lock().unwrap().snapshot.as_mut().unwrap());
        let mut effects = 0;
        assert!(
            changed
                .app
                .with_inactive_session_preview_authority(&none, &mut || {
                    effects += 1;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(effects, 0);
    }
}
#[test]
fn bj_08_domain() {
    let h = Harness::new();
    h.connect("one", "op");
    for ids in [vec![], vec!["a", "a"], vec!["missing"]] {
        assert!(
            h.app
                .begin_reading(h.selection("invalid", &ids, false))
                .is_err()
        );
    }
    let too_many = (0..65).map(|i| format!("source-{i}")).collect::<Vec<_>>();
    let mut oversized = h.selection("limit", &["a"], false);
    oversized.source_ids = too_many;
    assert!(h.app.begin_reading(oversized).is_err());
    assert!(h.repo.0.lock().unwrap().sets.is_none());
    let r = h.selection("set", &["a", "b"], false);
    let receipt = h.app.begin_reading(r.clone()).unwrap();
    assert_eq!(receipt.selection.members.len(), 2);
    assert!(h.app.begin_reading(r).unwrap().replayed);
    assert_eq!(h.repo.0.lock().unwrap().commits.len(), 1);
}
#[test]
fn bj_09_domain() {
    let h = Harness::new();
    h.connect("one", "op");
    assert_eq!(
        h.app
            .begin_reading(h.selection("remember", &["a"], true))
            .unwrap_err()
            .code,
        "remember_requires_game"
    );
    h.app
        .begin_reading(h.selection("temporary", &["a"], false))
        .unwrap();
    assert!(h.repo.0.lock().unwrap().saved.is_none());
    assert_eq!(
        build_selection_match_snapshot(false, "", &[]).unwrap(),
        "{}"
    );
    h.hook.active.lock().unwrap().as_mut().unwrap().game_id = Some("game".into());
    h.repo
        .0
        .lock()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .session
        .as_mut()
        .unwrap()
        .game_id = Some("game".into());
    h.app
        .begin_reading(h.selection("saved", &["a"], true))
        .unwrap();
    let saved = h.repo.0.lock().unwrap().saved.clone().unwrap();
    h.app
        .begin_reading(h.selection("temporary-after-saved", &["b"], false))
        .unwrap();
    assert_eq!(h.repo.0.lock().unwrap().saved.as_ref(), Some(&saved));
    h.connect("two", "other-session");
    let projected = h
        .app
        .source_selection(SourceSelectionQuery {
            session_id: "sid-two".into(),
            expected_profile_generation: 1,
        })
        .unwrap();
    assert!(projected.current.is_none());
    assert_eq!(projected.state, SourceSelectionState::RequiresReview);
}
#[test]
fn bj_10_domain() {
    let h = Harness::new();
    h.connect("one", "op");
    let p = h
        .app
        .source_selection(SourceSelectionQuery {
            session_id: "sid-one".into(),
            expected_profile_generation: 1,
        })
        .unwrap();
    assert_eq!(
        p.sources[0].address.as_deref(),
        Some("18446744073709551615")
    );
    assert_eq!(p.sources[0].latest_sample, source("a").latest_sample);
    let raw = build_selection_match_snapshot(true, &"a".repeat(64), &p.sources).unwrap();
    validate_selection_match_snapshot(&raw, true, &"a".repeat(64), &p.sources).unwrap();
    assert!(!raw.contains("本文"));
    let wrong = raw.replace("9007199254740993", "9007199254740992");
    assert!(validate_selection_match_snapshot(&wrong, true, &"a".repeat(64), &p.sources).is_err());
    assert!(canonical_binding_decimal("018").is_err());
}
#[test]
fn bj_12_domain() {
    let h = Harness::new();
    h.connect("one", "op");
    let a = h
        .app
        .begin_reading(h.selection("a1", &["a"], false))
        .unwrap();
    let b = h
        .app
        .begin_reading(h.selection("b", &["b"], false))
        .unwrap();
    let a2 = h
        .app
        .begin_reading(h.selection("a2", &["a"], false))
        .unwrap();
    assert_ne!(
        a.selection.members[0].selection_revision,
        a2.selection.members[0].selection_revision
    );
    assert_ne!(a.selection.set_revision, b.selection.set_revision);
    assert!(
        h.app
            .with_reading_window_effect_authority(a.effect_authority, &mut || panic!(
                "stale callback"
            ))
            .is_err()
    );
}
#[test]
fn bj_13_domain() {
    let h = Harness::new();
    h.connect("one", "op");
    let r = h
        .app
        .begin_reading(h.selection("set", &["a"], false))
        .unwrap();
    let mut sequence = vec![];
    h.app
        .with_reading_window_effect_authority(r.effect_authority.clone(), &mut || {
            assert!(h.app.snapshot()?.session.is_some());
            sequence.push("show_hud");
            sequence.push("hide_main");
            Ok(())
        })
        .unwrap();
    assert_eq!(sequence, ["show_hud", "hide_main"]);
    let mut attempts = 0;
    assert!(
        h.app
            .with_reading_window_effect_authority(r.effect_authority.clone(), &mut || {
                attempts += 1;
                Err(unused())
            })
            .is_err()
    );
    h.app
        .with_reading_window_effect_authority(r.effect_authority, &mut || {
            attempts += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(attempts, 2);
    assert_eq!(h.repo.0.lock().unwrap().commits.len(), 1);
    let authority = h
        .app
        .begin_reading(h.selection("serialized", &["a"], false))
        .unwrap()
        .effect_authority;
    let request = h.selection("next-set", &["b"], false);
    let app = h.app.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let mut task = None;
    h.app
        .with_reading_window_effect_authority(authority, &mut || {
            let app = app.clone();
            let request = request.clone();
            let started = started_tx.clone();
            let done = done_tx.clone();
            task = Some(std::thread::spawn(move || {
                started.send(()).unwrap();
                let result = app.begin_reading(request);
                done.send(result).unwrap();
            }));
            started_rx.recv().unwrap();
            assert!(done_rx.try_recv().is_err());
            Ok(())
        })
        .unwrap();
    task.unwrap().join().unwrap();
    assert!(done_rx.recv().unwrap().is_ok());
}
#[test]
fn bj_15_domain() {
    association_preview_authority_regressions();
    let h = Harness::new();
    h.connect("one", "op");
    let r = h
        .app
        .begin_reading(h.selection("set", &["a"], false))
        .unwrap();
    h.connect("two", "next");
    let mut effects = 0;
    assert!(
        h.app
            .with_reading_window_effect_authority(r.effect_authority, &mut || {
                effects += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(effects, 0);
    let review = h.review("two");
    h.repo
        .0
        .lock()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .profile_generation = 2;
    assert!(
        h.app
            .associate_reviewed(ReviewedAssociateRequest {
                operation_id: "after-profile".into(),
                review_id: review.review_id
            })
            .is_err()
    );
}
#[test]
fn bj_17_domain() {
    let h = Harness::new();
    *h.hook.fail.lock().unwrap() = true;
    let review = h.review("one");
    assert!(
        h.app
            .associate_reviewed(ReviewedAssociateRequest {
                operation_id: "fail".into(),
                review_id: review.review_id
            })
            .is_err()
    );
    let p = h
        .app
        .association_progress(AssociationProgressQuery {
            operation_id: "fail".into(),
            expected_profile_generation: 1,
        })
        .unwrap();
    assert_eq!(p.stage, AssociationStage::Failed);
    assert!(h.app.snapshot().unwrap().session.is_none());
    *h.hook.fail.lock().unwrap() = false;
    let r = h.connect("one", "retry");
    assert_eq!(r.session_id.as_deref(), Some("sid-one"));
    assert_eq!(h.hook.injections.load(Ordering::SeqCst), 1);
}

fn assert_preview_callback_is_rejected(
    app: &ApplicationRuntime,
    receipt: &AssociationOperationReceipt,
) {
    let mut callbacks = 0;
    let result = app.with_association_preview_authority(receipt, &mut |_, _| {
        callbacks += 1;
        Ok(())
    });
    assert!(result.is_err() || result == Ok(false));
    assert_eq!(callbacks, 0);
}

fn association_preview_authority_regressions() {
    let h = Harness::new();
    let first = h.connect("one", "original-operation");
    let expected_candidate = candidate("one", 1);
    let prepared = h.app.association_preview_target(&first).unwrap();
    assert_eq!(prepared.candidate_id, expected_candidate.candidate_id);
    assert_eq!(h.app.association_preview_target(&first).unwrap(), prepared);
    let mut callbacks = 0;
    assert!(
        h.app
            .with_association_preview_authority(&first, &mut |target, sid| {
                callbacks += 1;
                assert_eq!(target.candidate_id, expected_candidate.candidate_id);
                assert_eq!(target.executable_path, expected_candidate.executable_path);
                assert_eq!(
                    target.process_created_u64,
                    expected_candidate.process_created_u64
                );
                assert_eq!(sid, "sid-one");
                assert!(h.app.snapshot()?.session.is_some());
                Ok(())
            })
            .unwrap()
    );
    assert_eq!(callbacks, 1);
    assert_preview_callback_is_rejected(&h.app, &first);
    let replay = h
        .app
        .associate_reviewed(ReviewedAssociateRequest {
            operation_id: first.operation_id.clone(),
            review_id: first.review_id.clone(),
        })
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(h.app.association_preview_target(&replay).unwrap(), prepared);
    assert_preview_callback_is_rejected(&h.app, &replay);

    let before_switch = Harness::new();
    let old = before_switch.connect("one", "old-operation");
    let current = before_switch.connect("two", "new-operation");
    // Reading an older target must not retarget it to the new candidate or
    // change the current effect head. Only the final guarded callback grants.
    assert_eq!(
        before_switch
            .app
            .association_preview_target(&old)
            .unwrap()
            .candidate_id,
        "one"
    );
    assert_eq!(
        before_switch
            .app
            .association_preview_target(&current)
            .unwrap()
            .candidate_id,
        "two"
    );
    assert_preview_callback_is_rejected(&before_switch.app, &old);
    let replay_old = before_switch
        .app
        .associate_reviewed(ReviewedAssociateRequest {
            operation_id: old.operation_id.clone(),
            review_id: old.review_id.clone(),
        })
        .unwrap();
    assert!(replay_old.replayed);
    assert_preview_callback_is_rejected(&before_switch.app, &replay_old);
    assert_eq!(
        before_switch
            .app
            .snapshot()
            .unwrap()
            .session
            .unwrap()
            .session_id,
        "sid-two"
    );
    let mut current_callbacks = 0;
    assert!(
        before_switch
            .app
            .with_association_preview_authority(&current, &mut |target, sid| {
                assert_eq!(target.candidate_id, "two");
                assert_eq!(sid, "sid-two");
                current_callbacks += 1;
                Ok(())
            })
            .unwrap()
    );
    assert_eq!(current_callbacks, 1);

    let reuse = Harness::new();
    let old = reuse.connect("one", "older-same-session");
    reuse.connect("one", "newer-same-session");
    assert_preview_callback_is_rejected(&reuse.app, &old);

    let failed_new_intent = Harness::new();
    let old = failed_new_intent.connect("one", "before-failed-switch");
    *failed_new_intent.hook.fail.lock().unwrap() = true;
    let new_review = failed_new_intent.review("two");
    assert!(
        failed_new_intent
            .app
            .associate_reviewed(ReviewedAssociateRequest {
                operation_id: "failed-new-switch".into(),
                review_id: new_review.review_id
            })
            .is_err()
    );
    assert_preview_callback_is_rejected(&failed_new_intent.app, &old);
    assert_eq!(
        failed_new_intent
            .app
            .snapshot()
            .unwrap()
            .session
            .unwrap()
            .session_id,
        "sid-one"
    );

    let profile_changed = Harness::new();
    let receipt = profile_changed.connect("one", "profile-changed");
    profile_changed
        .repo
        .0
        .lock()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .profile_generation += 1;
    assert_preview_callback_is_rejected(&profile_changed.app, &receipt);

    let changed = Harness::new();
    let receipt = changed.connect("one", "lease-change");
    let mut replacement = changed.hook.active.lock().unwrap().clone().unwrap();
    replacement.broker_instance_id = "new-broker-same-sid".into();
    *changed.hook.next_guard_active.lock().unwrap() = Some(replacement);
    assert_preview_callback_is_rejected(&changed.app, &receipt);

    let changed_before_lookup = Harness::new();
    let receipt = changed_before_lookup.connect("one", "lease-changed-before-lookup");
    changed_before_lookup
        .hook
        .active
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .broker_instance_id = "reused-sid-new-broker".into();
    assert_preview_callback_is_rejected(&changed_before_lookup.app, &receipt);

    let failed = Harness::new();
    *failed.hook.fail.lock().unwrap() = true;
    let review = failed.review("one");
    assert!(
        failed
            .app
            .associate_reviewed(ReviewedAssociateRequest {
                operation_id: "failed-association".into(),
                review_id: review.review_id.clone()
            })
            .is_err()
    );
    let never_committed = AssociationOperationReceipt {
        operation_id: "failed-association".into(),
        review_id: review.review_id,
        profile: review.profile,
        relation: review.relation,
        session_id: Some("sid-one".into()),
        replayed: false,
    };
    assert_preview_callback_is_rejected(&failed.app, &never_committed);

    let corrupted = Harness::new();
    let receipt = corrupted.connect("one", "journal-change");
    corrupted
        .repo
        .0
        .lock()
        .unwrap()
        .operations
        .get_mut("journal-change")
        .unwrap()
        .request_sha256 = "wrong-request".into();
    assert_preview_callback_is_rejected(&corrupted.app, &receipt);

    let partial_failure = Harness::new();
    let receipt = partial_failure.connect("one", "native-metadata-failure");
    let mut attempts = 0;
    assert!(
        partial_failure
            .app
            .with_association_preview_authority(&receipt, &mut |_, _| {
                attempts += 1;
                Err(unused())
            })
            .is_err()
    );
    assert_eq!(attempts, 1);
    assert_preview_callback_is_rejected(&partial_failure.app, &receipt);
}

fn current_authority_value(authority: &CurrentSessionPreviewAuthority) -> serde_json::Value {
    let active = &authority.hook_session;
    serde_json::json!({"profile":authority.profile,"active_witness":{
        "session_id":active.session_id,"game_id":active.game_id,"environment_key":active.environment_key,
        "target_exe_path":active.target_exe_path,"windows_pid":active.windows_pid,
        "process_created_u64":active.process_created_u64,"architecture":active.architecture,
        "broker_instance_id":active.broker_instance_id,"target_identity_sha256":active.target_identity_sha256,
        "resource":{"snapshot_id":active.resource.snapshot_id,"kind":active.resource.kind,"version":active.resource.version,
            "manifest_sha256":active.resource.manifest_sha256,"contract_version":active.resource.contract_version,"metadata_json":active.resource.metadata_json}}})
}

fn current_preview_origin_regressions() {
    let independent = Harness::new();
    let existing = session(&candidate("one", 1), "legacy-existing-host");
    independent
        .repo
        .open_hook_session(existing.clone(), 1)
        .unwrap();
    *independent.hook.active.lock().unwrap() = Some(existing);
    let unreviewed = independent
        .app
        .current_session_preview_authority()
        .unwrap()
        .unwrap();
    assert!(
        unreviewed.origin.is_none(),
        "an existing host is not a reviewed candidate transfer"
    );
    let mut independent_callbacks = 0;
    independent
        .app
        .with_current_session_preview_authority(&unreviewed, &mut || {
            independent_callbacks += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(independent_callbacks, 1);

    let h = Harness::new();
    let review = h.review("one");
    let request = ReviewedAssociateRequest {
        operation_id: "origin-first".into(),
        review_id: review.review_id.clone(),
    };
    let before = serde_json::to_value(h.app.snapshot().unwrap()).unwrap();
    let intent = h.app.association_preview_intent(&request).unwrap();
    assert_eq!(intent, review);
    assert_eq!(
        serde_json::to_value(h.app.snapshot().unwrap()).unwrap(),
        before
    );
    assert!(h.repo.0.lock().unwrap().operations.is_empty());
    assert_eq!(h.hook.injections.load(Ordering::SeqCst), 0);
    let first = h.app.associate_reviewed(request.clone()).unwrap();
    let first_authority = h.app.current_session_preview_authority().unwrap().unwrap();
    let first_origin = first_authority.origin.as_ref().unwrap();
    assert_eq!(first_origin.operation_id, first.operation_id);
    assert_eq!(first_origin.review_id, first.review_id);
    assert_eq!(first_origin.candidate, intent.candidate);
    assert_eq!(
        h.app.association_preview_intent(&request).unwrap_err().code,
        "association_preview_replayed"
    );

    // Successful same-target reuse has a new exact reviewed candidate ticket,
    // while failed attempts and old operation replays retain the last success.
    let second = h.connect("one", "origin-reuse");
    assert_eq!(second.session_id, first.session_id);
    let second_authority = h.app.current_session_preview_authority().unwrap().unwrap();
    assert_eq!(
        current_authority_value(&first_authority),
        current_authority_value(&second_authority)
    );
    assert_eq!(
        second_authority.origin.as_ref().unwrap().operation_id,
        second.operation_id
    );
    let mut callbacks = 0;
    assert!(
        h.app
            .with_current_session_preview_authority(&first_authority, &mut || {
                callbacks += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(callbacks, 0);
    let replayed = h.app.associate_reviewed(request).unwrap();
    assert!(replayed.replayed);
    assert_eq!(
        h.app
            .current_session_preview_authority()
            .unwrap()
            .unwrap()
            .origin,
        second_authority.origin
    );

    let mut invalid = vec![];
    let mut omitted = second_authority.clone();
    omitted.origin = None;
    invalid.push(omitted);
    let mut op = second_authority.clone();
    op.origin.as_mut().unwrap().operation_id.push_str("-other");
    invalid.push(op);
    let mut token = second_authority.clone();
    token.origin.as_mut().unwrap().review_id.push_str("-other");
    invalid.push(token);
    let mut candidate = second_authority.clone();
    candidate
        .origin
        .as_mut()
        .unwrap()
        .candidate
        .executable_path
        .push_str("-other");
    invalid.push(candidate);
    for authority in invalid {
        assert!(
            h.app
                .with_current_session_preview_authority(&authority, &mut || {
                    callbacks += 1;
                    Ok(())
                })
                .is_err()
        );
    }
    assert_eq!(callbacks, 0);
    h.app
        .with_current_session_preview_authority(&second_authority, &mut || {
            callbacks += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(callbacks, 1);

    let stale = h.review("two");
    h.hook
        .candidates
        .lock()
        .unwrap()
        .get_mut("two")
        .unwrap()
        .process_created_u64 = "2".into();
    assert_eq!(
        h.app
            .association_preview_intent(&ReviewedAssociateRequest {
                operation_id: "stale-ticket".into(),
                review_id: stale.review_id,
            })
            .unwrap_err()
            .code,
        "association_review_stale"
    );
    assert_eq!(
        h.app
            .current_session_preview_authority()
            .unwrap()
            .unwrap()
            .origin,
        second_authority.origin
    );
}
struct CurrentPreviewObservations {
    started: std::time::Instant,
    events: Vec<serde_json::Value>,
}
impl CurrentPreviewObservations {
    fn new() -> Self {
        Self {
            started: std::time::Instant::now(),
            events: vec![],
        }
    }
    fn record(&mut self, branch: &str, kind: &str, data: serde_json::Value) {
        self.events.push(serde_json::json!({"sequence":self.events.len()+1,"monotonic_ns":self.started.elapsed().as_nanos()+1,
            "wall_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(),
            "thread_id":format!("{:?}",std::thread::current().id()),"branch":branch,"kind":kind,"data":data}));
    }
    fn emit(self) {
        if let (Ok(nonce), Ok(source)) = (
            std::env::var("LUNA_BINDING_NONCE"),
            std::env::var("LUNA_BINDING_SOURCE_TREE"),
        ) {
            println!(
                "LUNA_BINDING_PREVIEW_OBSERVATIONS_V1 {}",
                serde_json::json!({"kind":"binding-preview-observations/v1","schema_version":1,
                "nonce":nonce,"source_tree_sha256":source,"test_name":"bj_07_domain","pid":std::process::id(),"events":self.events})
            );
        }
    }
}
fn current_preview_lifecycle_observations() {
    let mut observations = CurrentPreviewObservations::new();
    for branch in ["resource_failure", "target_lease_failure"] {
        let h = Harness::new();
        let old = h.connect("one", "s1-operation");
        let before = h.app.current_session_preview_authority().unwrap().unwrap();
        let projection = h.app.subscribe().borrow().clone();
        assert_eq!(
            h.app
                .current_session_preview_authority()
                .unwrap()
                .unwrap()
                .hook_session
                .session_id,
            "sid-one"
        );
        assert_eq!(
            h.app.subscribe().borrow().revision,
            projection.revision,
            "authority query cannot publish"
        );
        observations.record(branch, "s1_active_before", current_authority_value(&before));
        let directory = std::env::temp_dir().join(format!(
            "binding-current-preview-{}-{}",
            std::process::id(),
            branch
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let held = if branch == "resource_failure" {
            let missing = directory.join("missing-resource");
            let _ = std::fs::remove_file(&missing);
            *h.hook.resource_path.lock().unwrap() = Some(missing);
            None
        } else {
            let path = directory.join("target.lock");
            let held = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            held.try_lock().unwrap();
            *h.hook.target_lease_path.lock().unwrap() = Some(path);
            Some(held)
        };
        let review = h.review("two");
        let failed = h
            .app
            .associate_reviewed(ReviewedAssociateRequest {
                operation_id: "s2-operation".into(),
                review_id: review.review_id,
            })
            .unwrap_err();
        observations.record(
            branch,
            "s2_failed_before_stop",
            serde_json::json!({"operation_id":"s2-operation","error":failed}),
        );
        let after = h.app.current_session_preview_authority().unwrap().unwrap();
        assert_eq!(
            before.origin, after.origin,
            "a failed S2 intent cannot replace S1's committed origin"
        );
        assert_eq!(
            current_authority_value(&before),
            current_authority_value(&after)
        );
        observations.record(branch, "s1_active_after", current_authority_value(&after));
        let mut old_callbacks = 0;
        assert!(
            h.app
                .with_association_preview_authority(&old, &mut |_, _| {
                    old_callbacks += 1;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(old_callbacks, 0);
        let fresh = h.app.current_session_preview_authority().unwrap().unwrap();
        observations.record(
            branch,
            "fresh_authority_acquired",
            current_authority_value(&fresh),
        );
        let mut fresh_callbacks = 0;
        h.app
            .with_current_session_preview_authority(&fresh, &mut || {
                fresh_callbacks += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(fresh_callbacks, 1);
        let stops = h.hook.stops.load(Ordering::SeqCst);
        assert_eq!(stops, 0);
        observations.record(branch, "call_counters", serde_json::json!({"stop_s1":stops,"old_handoff_callbacks":old_callbacks,"fresh_current_callbacks":fresh_callbacks}));
        drop(held);
        std::fs::remove_dir_all(directory).unwrap();
    }
    let h = Harness::new();
    h.connect("one", "authority-operation");
    let current = h.app.current_session_preview_authority().unwrap().unwrap();
    let baseline = h.app.snapshot().unwrap();
    let mut mutations = Vec::new();
    macro_rules! changed {
        ($name:literal,$field:expr,$value:expr) => {{
            let mut authority = current.clone();
            $field(&mut authority, $value);
            mutations.push(($name, authority));
        }};
    }
    changed!(
        "profileId",
        |a: &mut CurrentSessionPreviewAuthority, v| a.profile.profile_id = v,
        "another-profile".into()
    );
    changed!(
        "serverInstanceId",
        |a: &mut CurrentSessionPreviewAuthority, v| a.profile.server_instance_id = v,
        "another-server".into()
    );
    changed!(
        "profileGeneration",
        |a: &mut CurrentSessionPreviewAuthority, v| a.profile.profile_generation = v,
        2
    );
    changed!(
        "session_id",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.session_id = v,
        "another-session".into()
    );
    changed!(
        "game_id",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.game_id = v,
        Some("another-game".into())
    );
    changed!(
        "environment_key",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.environment_key = v,
        "crossover:/another".into()
    );
    changed!(
        "target_exe_path",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.target_exe_path = v,
        "C:\\another.exe".into()
    );
    changed!(
        "windows_pid",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.windows_pid = v,
        72
    );
    changed!(
        "process_created_u64",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.process_created_u64 = v,
        "18446744073709551614".into()
    );
    changed!(
        "architecture",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.architecture = v,
        "x86".into()
    );
    changed!(
        "broker_instance_id",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.broker_instance_id = v,
        "another-broker".into()
    );
    changed!(
        "target_identity_sha256",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.target_identity_sha256 = v,
        domain::content_hash("another-target")
    );
    changed!(
        "resource.snapshot_id",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.resource.snapshot_id = v,
        "another-resource".into()
    );
    changed!(
        "resource.kind",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.resource.kind = v,
        "another-kind".into()
    );
    changed!(
        "resource.version",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.resource.version = v,
        "another-version".into()
    );
    changed!(
        "resource.manifest_sha256",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.resource.manifest_sha256 = v,
        domain::content_hash("another-manifest")
    );
    changed!(
        "resource.contract_version",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.resource.contract_version = v,
        "another-contract".into()
    );
    changed!(
        "resource.metadata_json",
        |a: &mut CurrentSessionPreviewAuthority, v| a.hook_session.resource.metadata_json = v,
        "{\"revision\":2}".into()
    );
    for (field, attempted) in mutations {
        let mut callbacks = 0;
        assert!(
            h.app
                .with_current_session_preview_authority(&attempted, &mut || {
                    callbacks += 1;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(callbacks, 0);
        let after = h.app.snapshot().unwrap();
        assert_eq!(baseline, after);
        observations.record("authority_races","mutated_authority_attempt",serde_json::json!({"mutated_field":field,
            "current_authority":current_authority_value(&current),"attempted_authority":current_authority_value(&attempted),
            "effect_callbacks":callbacks,"before":baseline,"after":after}));
    }
    let mut callbacks = 0;
    h.app
        .with_current_session_preview_authority(&current, &mut || {
            callbacks += 1;
            Ok(())
        })
        .unwrap();
    observations.record(
        "authority_races",
        "fresh_authority_attempt",
        serde_json::json!({"effect_callbacks":callbacks}),
    );
    assert_eq!(callbacks, 1);
    observations.emit();
}

fn library_attribution_intent_regressions() {
    let h = Harness::new();
    let mut first = game(&candidate("one", 1));
    first.game_id = "library-a".into();
    let mut second = game(&candidate("two", 2));
    second.game_id = "library-b".into();
    h.repo.0.lock().unwrap().snapshot.as_mut().unwrap().games = vec![first, second.clone()];
    let query = AssociationReviewQuery {
        candidate_id: "two".into(),
        game_id: None,
        expected_profile_generation: 1,
        expected_active_session_id: None,
        prior_review_id: None,
        attribution_intent: Some(AssociationAttributionIntent::AutoMatch),
    };
    let matched = h.app.review_association(query.clone()).unwrap();
    assert_eq!(matched.game_id.as_deref(), Some("library-b"));
    let mut session_only = query.clone();
    session_only.attribution_intent = Some(AssociationAttributionIntent::SessionOnly);
    assert!(
        h.app
            .review_association(session_only)
            .unwrap()
            .game_id
            .is_none()
    );
    let mut legacy_null = query.clone();
    legacy_null.attribution_intent = None;
    assert!(
        h.app
            .review_association(legacy_null)
            .unwrap()
            .game_id
            .is_none(),
        "unmarked null cannot silently attribute a session"
    );
    let mut scoped = query.clone();
    scoped.game_id = Some("library-a".into());
    scoped.attribution_intent = Some(AssociationAttributionIntent::ExplicitGame);
    let conflicted = h.app.review_association(scoped).unwrap();
    assert_eq!(conflicted.relation, AssociationRelation::Unavailable);
    assert_eq!(conflicted.game_id.as_deref(), Some("library-a"));
    assert_eq!(conflicted.candidate.candidate_id, "two");
    assert_eq!(
        conflicted.error.as_ref().unwrap().code,
        "game_target_conflict"
    );
    assert_eq!(
        h.app
            .associate_reviewed(ReviewedAssociateRequest {
                operation_id: "cannot-attach-conflict".into(),
                review_id: conflicted.review_id
            })
            .unwrap_err()
            .code,
        "game_target_conflict"
    );
    let mut missing = query.clone();
    missing.game_id = Some("archived-owner".into());
    missing.attribution_intent = Some(AssociationAttributionIntent::ExplicitGame);
    let missing = h.app.review_association(missing).unwrap();
    assert_eq!(missing.relation, AssociationRelation::Unavailable);
    assert_eq!(missing.game_id.as_deref(), Some("archived-owner"));
    assert_eq!(missing.error.as_ref().unwrap().code, "game_not_active");
    let mut duplicate = second;
    duplicate.game_id = "another-b".into();
    h.repo
        .0
        .lock()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .games
        .push(duplicate);
    let ambiguous = h.app.review_association(query).unwrap();
    assert_eq!(ambiguous.relation, AssociationRelation::Unavailable);
    assert_eq!(
        ambiguous.error.as_ref().unwrap().code,
        "ambiguous_game_target"
    );
    assert_eq!(h.hook.injections.load(Ordering::SeqCst), 0);
    assert!(h.app.snapshot().unwrap().session.is_none());
}
