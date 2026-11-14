//! Application authority for the reviewed binding journey; no SQL, Native UI or FFI.
use crate::{application::runtime::ApplicationRuntime, contracts::*, ports::runtime::*};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::atomic::Ordering,
};

#[derive(Default)]
pub(super) struct BindingJourneyState {
    reviews: BTreeMap<
        String,
        (
            AssociationReview,
            Option<crate::domain::capture::HookSessionInput>,
        ),
    >,
    review_order: VecDeque<String>,
    operations: BTreeMap<String, (ReviewedAssociateRequest, AssociationOperationReceipt)>,
    latest_association_operation: Option<String>,
    preview_effects_attempted: BTreeSet<String>,
    committed_association_sessions: BTreeMap<String, crate::domain::capture::HookSessionInput>,
    committed_preview_origin: Option<(
        BindingProfileContext,
        crate::domain::capture::HookSessionInput,
        CurrentSessionPreviewOrigin,
    )>,
}

pub fn build_selection_match_snapshot(
    remember: bool,
    target_identity_sha256: &str,
    selected_sources: &[SourceDetailView],
) -> AppResult<String> {
    if !remember {
        return Ok("{}".into());
    }
    if target_identity_sha256.len() != 64
        || !target_identity_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || selected_sources.is_empty()
        || selected_sources.len() > SOURCE_SELECTION_MEMBER_LIMIT as usize
    {
        return Err(AppError::new(
            "invalid_saved_selection",
            "capture",
            "保存选择缺少可核对的目标与来源",
        ));
    }
    let sources = selected_sources
        .iter()
        .map(|source| {
            crate::domain::require_id(&source.source_id)?;
            for value in [&source.address, &source.context, &source.context2]
                .into_iter()
                .flatten()
            {
                canonical_binding_decimal(value)?;
            }
            Ok(SavedSourceDescriptor {
                source_id: source.source_id.clone(),
                stream_identity_sha256: source.stream_identity_sha256.clone(),
                hook_name: source.hook_name.clone(),
                hook_code: source.hook_code.clone(),
                encoding: source.encoding.clone(),
                address: source.address.clone(),
                context: source.context.clone(),
                context2: source.context2.clone(),
                identity_extensions_json: source.identity_extensions_json.clone(),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    serde_json::to_string(&SavedSelectionSnapshotV1 {
        schema_version: 1,
        match_capability: SavedSelectionMatchCapability::RequiresReview,
        target_identity_sha256: target_identity_sha256.into(),
        sources,
    })
    .map_err(|_| AppError::new("invalid_saved_selection", "capture", "保存来源说明编码失败"))
}

pub fn validate_selection_match_snapshot(
    raw: &str,
    remember: bool,
    target_identity_sha256: &str,
    selected_sources: &[SourceDetailView],
) -> AppResult<()> {
    if raw.contains('\0') {
        return Err(AppError::new(
            "invalid_saved_selection",
            "capture",
            "保存来源说明含无效字符",
        ));
    }
    let expected =
        build_selection_match_snapshot(remember, target_identity_sha256, selected_sources)?;
    if !remember {
        let actual: serde_json::Value = serde_json::from_str(raw)
            .map_err(|_| AppError::new("invalid_saved_selection", "capture", "临时选择说明无效"))?;
        if actual.as_object().is_none_or(|object| !object.is_empty()) {
            return Err(AppError::new(
                "invalid_saved_selection",
                "capture",
                "临时选择不能替换保存规则",
            ));
        }
        return Ok(());
    }
    let actual: SavedSelectionSnapshotV1 = serde_json::from_str(raw).map_err(|_| {
        AppError::new(
            "invalid_saved_selection",
            "capture",
            "保存来源说明版本或字段无效",
        )
    })?;
    let expected: SavedSelectionSnapshotV1 = serde_json::from_str(&expected)
        .map_err(|_| AppError::new("invalid_saved_selection", "capture", "保存来源说明编码失败"))?;
    if actual != expected {
        return Err(AppError::new(
            "invalid_saved_selection",
            "capture",
            "保存来源说明与当前实际来源不一致",
        ));
    }
    Ok(())
}

fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "capture", message)
}
fn lock_failure() -> AppError {
    failure("application_state", "接入状态锁不可用")
}
fn profile(snapshot: &AppSnapshot) -> BindingProfileContext {
    BindingProfileContext {
        profile_id: snapshot.profile_id.clone(),
        server_instance_id: snapshot.server_instance_id.clone(),
        profile_generation: snapshot.profile_generation,
    }
}
fn target_identity(candidate: &TargetCandidate) -> AppResult<AssociationTargetIdentity> {
    crate::domain::require_id(&candidate.candidate_id)?;
    canonical_binding_decimal(&candidate.process_created_u64)?;
    if candidate.environment_key.is_empty()
        || candidate.executable_path.is_empty()
        || candidate.windows_pid == 0
        || !matches!(candidate.architecture.as_str(), "x86" | "x64")
    {
        return Err(failure("incomplete_identity", "候选缺少完整进程身份"));
    }
    let identity = crate::domain::content_hash(format!(
        "{}\0{}\0{}\0{}\0{}",
        candidate.environment_key,
        candidate.executable_path,
        candidate.windows_pid,
        candidate.process_created_u64,
        candidate.architecture
    ));
    Ok(AssociationTargetIdentity {
        candidate_id: candidate.candidate_id.clone(),
        environment_key: candidate.environment_key.clone(),
        executable_path: candidate.executable_path.clone(),
        windows_pid: candidate.windows_pid,
        process_created_u64: candidate.process_created_u64.clone(),
        architecture: candidate.architecture.clone(),
        target_identity_sha256: identity,
    })
}
fn stage_rank(stage: &AssociationStage) -> u8 {
    match stage {
        AssociationStage::Verifying => 0,
        AssociationStage::PreparingResources => 1,
        AssociationStage::RequestingHook => 2,
        AssociationStage::WaitingHandshake => 3,
        AssociationStage::WaitingText => 4,
        AssociationStage::CandidatesReady => 5,
        AssociationStage::Failed => 6,
        AssociationStage::Superseded => 7,
    }
}
fn resolve_review_game(
    snapshot: &AppSnapshot,
    candidate: &AssociationTargetIdentity,
    query: &AssociationReviewQuery,
) -> AppResult<Option<String>> {
    let intent = query
        .attribution_intent
        .unwrap_or(if query.game_id.is_some() {
            AssociationAttributionIntent::ExplicitGame
        } else {
            AssociationAttributionIntent::SessionOnly
        });
    let actual_path = crate::domain::normalize_windows_path(&candidate.executable_path)?.1;
    let compatible = |game: &Game, exact: bool| -> AppResult<bool> {
        let declared = game
            .target
            .canonical_exe_path
            .as_deref()
            .map(crate::domain::normalize_windows_path)
            .transpose()?
            .map(|(_, key)| key);
        Ok(game.target.environment_kind == "crossover"
            && game.target.environment_key == candidate.environment_key
            && if exact {
                declared.as_deref() == Some(actual_path.as_str())
                    && game.target.architecture.as_deref() == Some(candidate.architecture.as_str())
            } else {
                declared.as_deref().is_none_or(|path| path == actual_path)
                    && game
                        .target
                        .architecture
                        .as_deref()
                        .is_none_or(|architecture| architecture == candidate.architecture)
            })
    };
    match intent {
        AssociationAttributionIntent::AutoMatch => {
            if query.game_id.is_some() {
                return Err(failure(
                    "invalid_attribution_intent",
                    "全局自动匹配不能携带某张游戏卡片的归属",
                ));
            }
            let matches = snapshot
                .games
                .iter()
                .filter_map(|game| match compatible(game, true) {
                    Ok(true) => Some(Ok(game.game_id.clone())),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<AppResult<Vec<_>>>()?;
            match matches.len() {
                0 => Ok(None),
                1 => Ok(matches.into_iter().next()),
                _ => Err(failure(
                    "ambiguous_game_target",
                    "多份游戏资料对应此运行目标，请明确选择归属",
                )),
            }
        }
        AssociationAttributionIntent::ExplicitGame => {
            let game = query
                .game_id
                .as_ref()
                .and_then(|id| snapshot.games.iter().find(|game| &game.game_id == id))
                .ok_or_else(|| {
                    failure("game_not_active", "原指定游戏资料已不存在，请明确重新选择")
                })?;
            if !compatible(game, false)? {
                return Err(failure(
                    "game_target_conflict",
                    "原指定游戏与运行目标的环境、路径或位数冲突，请明确调整归属",
                ));
            }
            Ok(Some(game.game_id.clone()))
        }
        AssociationAttributionIntent::SessionOnly => {
            if query.game_id.is_some() {
                return Err(failure(
                    "invalid_attribution_intent",
                    "仅本次连接不能同时指定游戏库归属",
                ));
            }
            Ok(None)
        }
    }
}

fn active_id(snapshot: &AppSnapshot) -> Option<String> {
    snapshot
        .session
        .as_ref()
        .filter(|s| matches!(s.state.as_str(), "opening" | "active"))
        .map(|s| s.session_id.clone())
}

fn same_active_context(
    left: Option<&crate::domain::capture::HookSessionInput>,
    right: Option<&crate::domain::capture::HookSessionInput>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => same_hook_session_facts(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn target_matches_session(
    candidate: &AssociationTargetIdentity,
    session: &crate::domain::capture::HookSessionInput,
) -> bool {
    candidate.target_identity_sha256 == session.target_identity_sha256
        && candidate.environment_key == session.environment_key
        && candidate.executable_path == session.target_exe_path
        && candidate.windows_pid == session.windows_pid
        && candidate.process_created_u64 == session.process_created_u64
        && candidate.architecture == session.architecture
}

fn verify_backend_current(
    snapshot: &AppSnapshot,
    active: Option<&crate::domain::capture::HookSessionInput>,
) -> AppResult<()> {
    if active_id(snapshot).as_deref() != active.map(|s| s.session_id.as_str()) {
        return Err(failure(
            "association_context_stale",
            "活动会话与宿主事实不同步，请等待收尾后重新核对",
        ));
    }
    Ok(())
}
impl ApplicationRuntime {
    /// Read-only review. The token captures the observed active context, including
    /// none; a later save/review never silently upgrades that observation to switch.
    pub fn review_association(
        &self,
        query: AssociationReviewQuery,
    ) -> AppResult<AssociationReview> {
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        let snapshot = self.repository.snapshot()?;
        if query.expected_profile_generation != snapshot.profile_generation
            || query.expected_active_session_id != active_id(&snapshot)
        {
            return Err(failure(
                "association_review_stale",
                "活动会话已变化，请重新核对运行目标",
            ));
        }
        let context = self.hook.association_context(&query.candidate_id)?;
        verify_backend_current(&snapshot, context.active.as_ref())?;
        let candidate = target_identity(&context.candidate)?;
        let (game_id, attribution_error) = match resolve_review_game(&snapshot, &candidate, &query)
        {
            Ok(game_id) => (game_id, None),
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "game_not_active" | "game_target_conflict" | "ambiguous_game_target"
                ) =>
            {
                (query.game_id.clone(), Some(error))
            }
            Err(error) => return Err(error),
        };
        if let Some(prior_id) = &query.prior_review_id {
            let (prior, reviewed_active) = self
                .binding_journey
                .lock()
                .map_err(|_| lock_failure())?
                .reviews
                .get(prior_id)
                .cloned()
                .ok_or_else(|| {
                    failure("association_review_stale", "原窗口确认已经过期，请重新选择")
                })?;
            if prior.error.is_some()
                || prior.relation == AssociationRelation::Unavailable
                || prior.profile != profile(&snapshot)
                || prior.candidate != candidate
                || prior.active_session_id != query.expected_active_session_id
                || prior.active_target_identity_sha256.as_ref()
                    != context.active.as_ref().map(|s| &s.target_identity_sha256)
                || !same_active_context(reviewed_active.as_ref(), context.active.as_ref())
                || prior.game_id.is_some()
                || query.game_id.is_none()
            {
                return Err(failure(
                    "association_review_stale",
                    "保存游戏资料期间原确认上下文已变化，请重新核对",
                ));
            }
            let game = snapshot
                .games
                .iter()
                .find(|game| Some(&game.game_id) == query.game_id.as_ref())
                .ok_or_else(|| failure("game_not_active", "已保存的游戏资料不存在"))?;
            let declared_path = game
                .target
                .canonical_exe_path
                .as_deref()
                .map(crate::domain::normalize_windows_path)
                .transpose()?
                .map(|(_, key)| key);
            let actual_path = crate::domain::normalize_windows_path(&candidate.executable_path)?.1;
            if game.target.environment_kind != "crossover"
                || game.target.environment_key != candidate.environment_key
                || declared_path.as_deref() != Some(actual_path.as_str())
                || game.target.architecture.as_deref() != Some(candidate.architecture.as_str())
            {
                return Err(failure(
                    "game_target_conflict",
                    "保存的游戏资料与原确认的运行目标不一致",
                ));
            }
        }
        let relation = if attribution_error.is_some() {
            AssociationRelation::Unavailable
        } else {
            match context.active.as_ref() {
                None => AssociationRelation::None,
                Some(active)
                    if active.target_identity_sha256 == candidate.target_identity_sha256 =>
                {
                    AssociationRelation::Reuse
                }
                Some(_) => AssociationRelation::Switch,
            }
        };
        let review = AssociationReview {
            review_id: self.clock.new_id("association-review"),
            profile: profile(&snapshot),
            relation: relation.clone(),
            candidate,
            game_id,
            active_session_id: context.active.as_ref().map(|s| s.session_id.clone()),
            active_target_identity_sha256: context
                .active
                .as_ref()
                .map(|s| s.target_identity_sha256.clone()),
            detail: attribution_error
                .as_ref()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| {
                    match relation {
                        AssociationRelation::Reuse => "将复用当前会话，保留正文选择与阅读记录",
                        AssociationRelation::Switch => "确认后切换正在取文的游戏",
                        _ => "确认窗口后自动接入，收到来源后选择正文",
                    }
                    .into()
                }),
            error: attribution_error,
        };
        let mut state = self.binding_journey.lock().map_err(|_| lock_failure())?;
        while state.reviews.len() >= 256 {
            if let Some(oldest) = state.review_order.pop_front() {
                state.reviews.remove(&oldest);
            } else {
                break;
            }
        }
        state.review_order.push_back(review.review_id.clone());
        state
            .reviews
            .insert(review.review_id.clone(), (review.clone(), context.active));
        Ok(review)
    }

    /// Read only the still-current reviewed intent for Native's bounded
    /// candidate ticket preparation. It grants no effect authority and does
    /// not advance an operation, stop a host, or change any projection.
    pub fn association_preview_intent(
        &self,
        request: &ReviewedAssociateRequest,
    ) -> AppResult<AssociationReview> {
        crate::domain::require_id(&request.operation_id)?;
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(failure("application_closing", "应用正在收尾"));
        }
        let (review, reviewed_active) = {
            let state = self.binding_journey.lock().map_err(|_| lock_failure())?;
            if let Some((previous, _)) = state.operations.get(&request.operation_id) {
                return Err(if previous == request {
                    failure(
                        "association_preview_replayed",
                        "原操作已提交，不再准备候选转交",
                    )
                } else {
                    failure("operation_conflict", "操作身份已用于不同请求")
                });
            }
            state
                .reviews
                .get(&request.review_id)
                .cloned()
                .ok_or_else(|| failure("association_review_stale", "目标确认已过期，请重新核对"))?
        };
        if review.relation == AssociationRelation::Unavailable || review.error.is_some() {
            return Err(review.error.clone().unwrap_or_else(|| {
                failure(
                    "association_unavailable",
                    "原归属或运行目标尚未解决，不能接入",
                )
            }));
        }
        let snapshot = self.repository.snapshot()?;
        let context = self
            .hook
            .association_context(&review.candidate.candidate_id)?;
        verify_backend_current(&snapshot, context.active.as_ref())?;
        if profile(&snapshot) != review.profile
            || active_id(&snapshot) != review.active_session_id
            || context.active.as_ref().map(|s| &s.target_identity_sha256)
                != review.active_target_identity_sha256.as_ref()
            || !same_active_context(reviewed_active.as_ref(), context.active.as_ref())
            || target_identity(&context.candidate)? != review.candidate
        {
            return Err(failure(
                "association_review_stale",
                "原预览候选的确认上下文已经变化",
            ));
        }
        Ok(review)
    }

    pub fn associate_reviewed(
        &self,
        request: ReviewedAssociateRequest,
    ) -> AppResult<AssociationOperationReceipt> {
        crate::domain::require_id(&request.operation_id)?;
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(failure("application_closing", "应用正在收尾"));
        }
        let (review, reviewed_active) = {
            let state = self.binding_journey.lock().map_err(|_| lock_failure())?;
            if let Some((previous, receipt)) = state.operations.get(&request.operation_id) {
                if previous != &request {
                    return Err(failure("operation_conflict", "操作身份已用于不同请求"));
                }
                let mut receipt = receipt.clone();
                receipt.replayed = true;
                return Ok(receipt);
            }
            state
                .reviews
                .get(&request.review_id)
                .cloned()
                .ok_or_else(|| failure("association_review_stale", "目标确认已过期，请重新核对"))?
        };
        if review.relation == AssociationRelation::Unavailable || review.error.is_some() {
            return Err(review.error.clone().unwrap_or_else(|| {
                failure(
                    "association_unavailable",
                    "原归属或运行目标尚未解决，不能接入",
                )
            }));
        }
        let snapshot = self.repository.snapshot()?;
        let context = self
            .hook
            .association_context(&review.candidate.candidate_id)?;
        verify_backend_current(&snapshot, context.active.as_ref())?;
        if profile(&snapshot) != review.profile
            || active_id(&snapshot) != review.active_session_id
            || context.active.as_ref().map(|s| &s.target_identity_sha256)
                != review.active_target_identity_sha256.as_ref()
            || !same_active_context(reviewed_active.as_ref(), context.active.as_ref())
            || target_identity(&context.candidate)? != review.candidate
        {
            return Err(failure(
                "association_review_stale",
                "确认后的目标或活动会话已变化，请重新核对",
            ));
        }
        let guard = AssociationExecutionGuard {
            candidate_identity_sha256: review.candidate.target_identity_sha256.clone(),
            expected_active_session_id: review.active_session_id.clone(),
            expected_active_target_identity_sha256: review.active_target_identity_sha256.clone(),
            expected_broker_instance_id: reviewed_active
                .as_ref()
                .map(|session| session.broker_instance_id.clone()),
            expected_active: reviewed_active,
        };
        let legacy = AssociateRequest {
            operation_id: request.operation_id.clone(),
            game_id: review.game_id.clone(),
            candidate_id: review.candidate.candidate_id.clone(),
            confirm_switch: review.relation == AssociationRelation::Switch,
        };
        // A newly confirmed intent supersedes an earlier preview transfer even
        // when its native start later fails. An old operation replay never
        // changes this head because it returns before reaching this boundary.
        self.binding_journey
            .lock()
            .map_err(|_| lock_failure())?
            .latest_association_operation = Some(request.operation_id.clone());
        let result = self.associate_guarded_locked(legacy, Some(&guard))?;
        let receipt = AssociationOperationReceipt {
            operation_id: request.operation_id.clone(),
            review_id: request.review_id.clone(),
            profile: review.profile,
            relation: review.relation,
            session_id: result.result_id,
            replayed: result.replayed,
        };
        // Bind the post-commit native owner, including broker generation. A
        // naturally exited owner leaves no transfer authority; the durable
        // association receipt itself remains truthful and queryable.
        let committed_session =
            self.hook
                .active_session_context()
                .ok()
                .flatten()
                .filter(|session| {
                    Some(&session.session_id) == receipt.session_id.as_ref()
                        && target_matches_session(&review.candidate, session)
                });
        let mut state = self.binding_journey.lock().map_err(|_| lock_failure())?;
        if let Some(session) = committed_session {
            state.committed_preview_origin = Some((
                receipt.profile.clone(),
                session.clone(),
                CurrentSessionPreviewOrigin {
                    operation_id: request.operation_id.clone(),
                    review_id: request.review_id.clone(),
                    candidate: review.candidate,
                },
            ));
            state
                .committed_association_sessions
                .insert(request.operation_id.clone(), session);
        }
        state
            .operations
            .insert(request.operation_id.clone(), (request, receipt.clone()));
        Ok(receipt)
    }

    /// Read-only current-session witness; deliberately does not publish a
    /// projection or refer to latest association operations. M12 subscription
    /// queries must not create a feedback loop or revive an old operation.
    pub fn current_session_preview_authority(
        &self,
    ) -> AppResult<Option<CurrentSessionPreviewAuthority>> {
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        let snapshot = self.repository.snapshot()?;
        let Some(session_id) = active_id(&snapshot) else {
            return Ok(None);
        };
        let active = self.hook.active_session_context()?.ok_or_else(|| {
            failure(
                "current_session_preview_stale",
                "当前取文宿主尚未完成身份核对",
            )
        })?;
        if active.session_id != session_id
            || snapshot
                .session
                .as_ref()
                .and_then(|session| session.game_id.as_ref())
                != active.game_id.as_ref()
        {
            return Err(failure(
                "current_session_preview_stale",
                "当前会话归属与实际宿主不一致",
            ));
        }
        Ok(Some(CurrentSessionPreviewAuthority {
            profile: profile(&snapshot),
            origin: self.current_preview_origin(&profile(&snapshot), &active)?,
            hook_session: active,
        }))
    }

    /// Read-only inactive witness. Native may only prepare a slice of exact
    /// obsolete tickets outside the guard; this query alone permits no cleanup.
    pub fn inactive_session_preview_authority(
        &self,
    ) -> AppResult<Option<InactiveSessionPreviewAuthority>> {
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        let snapshot = self.repository.snapshot()?;
        if active_id(&snapshot).is_some() || self.hook.active_session_context()?.is_some() {
            return Ok(None);
        }
        Ok(Some(InactiveSessionPreviewAuthority {
            profile: profile(&snapshot),
        }))
    }

    /// Keep capture ordering and the native no-active lease across the entire
    /// check-to-cleanup interval. Native must only retire its previously captured
    /// exact obsolete tickets, never enumerate new owners in this callback.
    pub fn with_inactive_session_preview_authority(
        &self,
        authority: &InactiveSessionPreviewAuthority,
        callback: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<bool> {
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(failure("application_closing", "应用正在收尾"));
        }
        let validate = |snapshot: &AppSnapshot| -> AppResult<()> {
            if profile(snapshot) != authority.profile || active_id(snapshot).is_some() {
                return Err(failure(
                    "inactive_session_preview_stale",
                    "已有新的会话或配置档，旧清理不能继续",
                ));
            }
            Ok(())
        };
        validate(&self.repository.snapshot()?)?;
        self.hook.with_no_active_session_guard(&mut || {
            validate(&self.repository.snapshot()?)?;
            callback()
        })?;
        Ok(true)
    }

    fn current_preview_origin(
        &self,
        current_profile: &BindingProfileContext,
        active: &crate::domain::capture::HookSessionInput,
    ) -> AppResult<Option<CurrentSessionPreviewOrigin>> {
        let state = self.binding_journey.lock().map_err(|_| lock_failure())?;
        Ok(state
            .committed_preview_origin
            .as_ref()
            .filter(|(committed_profile, committed, origin)| {
                committed_profile == current_profile
                    && same_hook_session_facts(committed, active)
                    && target_matches_session(&origin.candidate, active)
            })
            .map(|(_, _, origin)| origin.clone()))
    }

    /// Short current-owner guard, independent of one-shot association effects.
    /// Native may prepare files off-lock using this witness, then change only
    /// its own metadata or enqueue bounded work inside this callback.
    pub fn with_current_session_preview_authority(
        &self,
        authority: &CurrentSessionPreviewAuthority,
        callback: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<bool> {
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(failure("application_closing", "应用正在收尾"));
        }
        let validate_profile = |snapshot: &AppSnapshot| -> AppResult<()> {
            if profile(snapshot) != authority.profile
                || active_id(snapshot).as_deref()
                    != Some(authority.hook_session.session_id.as_str())
                || snapshot
                    .session
                    .as_ref()
                    .and_then(|session| session.game_id.as_ref())
                    != authority.hook_session.game_id.as_ref()
            {
                return Err(failure(
                    "current_session_preview_stale",
                    "当前预览会话或配置档已经变化",
                ));
            }
            Ok(())
        };
        validate_profile(&self.repository.snapshot()?)?;
        let active = self
            .hook
            .active_session_context()?
            .ok_or_else(|| failure("current_session_preview_stale", "当前宿主已经停止"))?;
        if !same_hook_session_facts(&active, &authority.hook_session) {
            return Err(failure(
                "current_session_preview_stale",
                "当前宿主、目标、归属或资源身份已经变化",
            ));
        }
        if self.current_preview_origin(&authority.profile, &active)? != authority.origin {
            return Err(failure(
                "current_session_preview_stale",
                "当前预览来源已经由另一成功确认更新",
            ));
        }
        self.hook
            .with_active_session_guard(&authority.hook_session, &mut || {
                // Recheck the actual profile after acquiring the native lease; a
                // replaced writer/profile cannot inherit an earlier read witness.
                validate_profile(&self.repository.snapshot()?)?;
                callback()
            })?;
        Ok(true)
    }

    /// Read the original reviewed target for off-lock Native file preparation.
    /// This is historical data, not a transfer grant: it never changes the
    /// association head or consumes effects, including on an old/replayed
    /// receipt. Native must still enter with_association_preview_authority after
    /// preparing the original candidate and before changing any metadata.
    pub fn association_preview_target(
        &self,
        receipt: &AssociationOperationReceipt,
    ) -> AppResult<AssociationTargetIdentity> {
        let state = self.binding_journey.lock().map_err(|_| lock_failure())?;
        let (request, stored) = state
            .operations
            .get(&receipt.operation_id)
            .ok_or_else(|| failure("association_preview_stale", "该接入操作没有成功的原始回执"))?;
        let mut original = receipt.clone();
        original.replayed = stored.replayed;
        if &original != stored || request.review_id != receipt.review_id {
            return Err(failure(
                "association_preview_stale",
                "预览准备回执与原接入操作不一致",
            ));
        }
        let (review, _) = state
            .reviews
            .get(&request.review_id)
            .ok_or_else(|| failure("association_preview_stale", "原目标核对事实已经过期"))?;
        Ok(review.candidate.clone())
    }

    /// Apply candidate-to-session preview metadata at most once for the exact
    /// committed association intent. No callback runs for an operation replay.
    /// Capture ordering and the native active-session lease cover the complete
    /// validation-to-callback interval. The callback must be short/synchronous
    /// and must not re-enter capture or Hook lease methods.
    pub fn with_association_preview_authority(
        &self,
        receipt: &AssociationOperationReceipt,
        callback: &mut dyn FnMut(&AssociationTargetIdentity, &str) -> AppResult<()>,
    ) -> AppResult<bool> {
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(failure("application_closing", "应用正在收尾"));
        }
        let (review, stored, committed_session) = {
            let state = self.binding_journey.lock().map_err(|_| lock_failure())?;
            let (request, stored) =
                state.operations.get(&receipt.operation_id).ok_or_else(|| {
                    failure("association_preview_stale", "该接入操作没有成功的原始回执")
                })?;
            let mut expected = receipt.clone();
            expected.replayed = stored.replayed;
            if &expected != stored || request.review_id != receipt.review_id {
                return Err(failure(
                    "association_preview_stale",
                    "预览转交回执与原接入操作不一致",
                ));
            }
            if receipt.replayed
                || state
                    .preview_effects_attempted
                    .contains(&receipt.operation_id)
            {
                return Ok(false);
            }
            if state.latest_association_operation.as_deref() != Some(receipt.operation_id.as_str())
            {
                return Err(failure(
                    "association_preview_stale",
                    "已有更新的接入意图，旧操作不能改变预览",
                ));
            }
            let (review, _) = state
                .reviews
                .get(&request.review_id)
                .ok_or_else(|| failure("association_preview_stale", "原目标核对事实已经过期"))?;
            let committed_session = state
                .committed_association_sessions
                .get(&receipt.operation_id)
                .ok_or_else(|| {
                    failure(
                        "association_preview_stale",
                        "原接入操作没有稳定的已提交宿主身份",
                    )
                })?;
            (review.clone(), stored.clone(), committed_session.clone())
        };
        let snapshot = self.repository.snapshot()?;
        let session_id = stored
            .session_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| failure("association_preview_stale", "接入操作没有成功建立会话"))?;
        if profile(&snapshot) != stored.profile
            || review.profile != stored.profile
            || active_id(&snapshot).as_deref() != Some(session_id)
        {
            return Err(failure(
                "association_preview_stale",
                "原接入操作的配置档或会话已经变化",
            ));
        }
        let operation = self
            .repository
            .association_progress_facts(&stored.operation_id)?;
        let original_request = AssociateRequest {
            operation_id: stored.operation_id.clone(),
            game_id: review.game_id.clone(),
            candidate_id: review.candidate.candidate_id.clone(),
            confirm_switch: review.relation == AssociationRelation::Switch,
        };
        if operation.profile != stored.profile
            || operation.session_id.as_deref() != Some(session_id)
            // capture_operations remains active (no end time) after a
            // successful association; its end time is a terminal lifecycle.
            || operation.finished_at_ms.is_some()
            || operation.request_sha256 != crate::domain::canonical_hash(&original_request)?
        {
            return Err(failure(
                "association_preview_stale",
                "实际接入日志与原确认回执不一致",
            ));
        }
        let active = self
            .hook
            .active_session_context()?
            .filter(|active| active.session_id == session_id)
            .ok_or_else(|| failure("association_preview_stale", "原宿主会话已经变化"))?;
        let candidate = &review.candidate;
        if !same_hook_session_facts(&active, &committed_session)
            || !target_matches_session(candidate, &active)
        {
            return Err(failure(
                "association_preview_stale",
                "原确认目标的完整进程身份已经变化",
            ));
        }
        self.hook.with_active_session_guard(&active, &mut || {
            // Consume before calling Native: a partial Native metadata failure
            // cannot be replayed using the same original receipt. A fresh
            // reviewed association is required for a new transfer attempt.
            self.binding_journey
                .lock()
                .map_err(|_| lock_failure())?
                .preview_effects_attempted
                .insert(stored.operation_id.clone());
            callback(candidate, session_id)
        })?;
        Ok(true)
    }

    /// Only actual persisted operation/Hook facts advance the product stage.
    pub fn association_progress(
        &self,
        query: AssociationProgressQuery,
    ) -> AppResult<AssociationProgress> {
        let snapshot = self.repository.snapshot()?;
        if query.expected_profile_generation != snapshot.profile_generation {
            return Err(failure("stale_generation", "配置档已经变化"));
        }
        let recorded = self
            .repository
            .association_progress_facts(&query.operation_id)?;
        let superseded = recorded.profile != profile(&snapshot)
            || recorded
                .session_id
                .as_ref()
                .is_some_and(|sid| active_id(&snapshot).as_ref() != Some(sid));
        let mut stage = AssociationStage::Verifying;
        let mut error = None;
        let facts = recorded
            .events
            .into_iter()
            .map(|fact| {
                let next = match fact.code.as_str() {
                    "association_started" | "target_identity_verified" => {
                        AssociationStage::Verifying
                    }
                    "resource_check_started"
                    | "resource_verified"
                    | "selected_core_resources_verified" => AssociationStage::PreparingResources,
                    "injection_requested" => AssociationStage::RequestingHook,
                    "association_ready" | "host_ready" | "host_response" => {
                        AssociationStage::WaitingHandshake
                    }
                    "capture_connected" | "process_connected" => AssociationStage::WaitingText,
                    "stream_created" | "source_discovered" | "text_received" => {
                        AssociationStage::CandidatesReady
                    }
                    "association_failed" | "host_failed" | "capture_failed" | "core_error" => {
                        AssociationStage::Failed
                    }
                    _ => stage.clone(),
                };
                // Later host responses are observations, not permission to
                // regress an already verified connection/candidate stage.
                if stage_rank(&next) > stage_rank(&stage) {
                    stage = next.clone();
                }
                if next == AssociationStage::Failed {
                    error = Some(failure(&fact.code, &fact.message));
                }
                AssociationProgressFact {
                    event_id: fact.event_id,
                    sequence: fact.ingest_sequence,
                    observed_at_ms: fact.observed_at_ms.to_string(),
                    stage: next,
                    level: fact.level,
                    code: fact.code,
                    message: fact.message,
                }
            })
            .collect();
        if !superseded
            && snapshot.session.as_ref().is_some_and(|s| {
                Some(&s.session_id) == recorded.session_id.as_ref() && !s.sources.is_empty()
            })
            && stage != AssociationStage::Failed
        {
            stage = AssociationStage::CandidatesReady;
        }
        if superseded {
            stage = AssociationStage::Superseded;
        }
        Ok(AssociationProgress {
            operation_id: recorded.operation_id.clone(),
            profile: recorded.profile,
            session_id: recorded.session_id,
            generation: crate::domain::content_hash(format!(
                "{}\0{}\0{}",
                snapshot.server_instance_id, snapshot.profile_generation, recorded.operation_id
            )),
            stage,
            facts,
            superseded,
            error,
        })
    }

    pub fn source_selection(
        &self,
        query: SourceSelectionQuery,
    ) -> AppResult<SourceSelectionProjection> {
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        let snapshot = self.repository.snapshot()?;
        if query.expected_profile_generation != snapshot.profile_generation {
            return Err(failure("stale_generation", "配置档已经变化"));
        }
        if active_id(&snapshot).as_deref() != Some(query.session_id.as_str()) {
            return Err(failure("stale_session", "当前会话已变化"));
        }
        let result = self.repository.source_selection(query)?;
        if result.profile != profile(&snapshot) {
            return Err(failure("stale_generation", "来源投影的配置档已经变化"));
        }
        for source in &result.sources {
            for numeric in [&source.address, &source.context, &source.context2]
                .into_iter()
                .flatten()
            {
                canonical_binding_decimal(numeric)?;
            }
        }
        Ok(result)
    }

    /// A durable set receipt is returned before any Native window operation.
    pub fn begin_reading(&self, request: BeginReadingRequest) -> AppResult<SelectionCommitReceipt> {
        crate::domain::require_id(&request.operation_id)?;
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(failure("application_closing", "应用正在收尾"));
        }
        if request.source_ids.is_empty()
            || request.source_ids.len() > SOURCE_SELECTION_MEMBER_LIMIT as usize
            || request.source_ids.iter().collect::<BTreeSet<_>>().len() != request.source_ids.len()
        {
            return Err(failure(
                "invalid_source_selection",
                "请勾选一到六十四条不同的正文来源",
            ));
        }
        for source in &request.source_ids {
            crate::domain::require_id(source)?;
        }
        let snapshot = self.repository.snapshot()?;
        if request.expected_profile_generation != snapshot.profile_generation {
            return Err(failure("stale_generation", "配置档已经变化"));
        }
        let active = self
            .hook
            .active_session_context()?
            .ok_or_else(|| failure("stale_session", "游戏取文已断开"))?;
        verify_backend_current(&snapshot, Some(&active))?;
        if active.session_id != request.session_id {
            return Err(failure("stale_session", "当前会话已变化"));
        }
        let projection = self.repository.source_selection(SourceSelectionQuery {
            session_id: request.session_id.clone(),
            expected_profile_generation: request.expected_profile_generation,
        })?;
        if request.remember && (!projection.remember_allowed || active.game_id.is_none()) {
            return Err(failure(
                "remember_requires_game",
                "请先将游戏加入游戏库，再记住正文选择",
            ));
        }
        let selected = request
            .source_ids
            .iter()
            .map(|id| {
                projection
                    .sources
                    .iter()
                    .find(|s| &s.source_id == id)
                    .cloned()
                    .ok_or_else(|| failure("stale_source", "勾选的来源已不属于当前会话"))
            })
            .collect::<AppResult<Vec<_>>>()?;
        let match_snapshot_json = build_selection_match_snapshot(
            request.remember,
            &active.target_identity_sha256,
            &selected,
        )?;
        let input = SourceSelectionCommit {
            request_sha256: crate::domain::canonical_hash(&request)?,
            request,
            profile: profile(&snapshot),
            active_session: active.clone(),
            match_snapshot_json,
        };
        let mut receipt = None;
        self.hook.with_active_session_guard(&active, &mut || {
            receipt = Some(
                self.repository
                    .commit_source_selection(input.clone(), self.clock.now_ms())?,
            );
            Ok(())
        })?;
        self.publish()?;
        receipt.ok_or_else(|| failure("selection_commit_missing", "正文选择未返回保存回执"))
    }

    /// The capture lock and native lease cover validation through the final
    /// synchronous window effect. The callback may read snapshots/preferences,
    /// but must never re-enter capture/association or block on asynchronous I/O.
    pub fn with_reading_window_effect_authority(
        &self,
        authority: ReadingWindowEffectAuthority,
        effect: &mut dyn FnMut() -> AppResult<()>,
    ) -> AppResult<SelectionCommitReceipt> {
        let _capture = self.capture_lock.lock().map_err(|_| lock_failure())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(failure("application_closing", "应用正在收尾"));
        }
        let snapshot = self.repository.snapshot()?;
        if profile(&snapshot) != authority.profile
            || active_id(&snapshot).as_deref() != Some(authority.session_id.as_str())
        {
            return Err(failure(
                "reading_effect_stale",
                "窗口操作已被新的会话或正文选择取代",
            ));
        }
        let active = self
            .hook
            .active_session_context()?
            .filter(|s| s.session_id == authority.session_id)
            .ok_or_else(|| failure("reading_effect_stale", "窗口操作的宿主已经变化"))?;
        let mut receipt = None;
        self.hook.with_active_session_guard(&active, &mut || {
            receipt = Some(
                self.repository
                    .validate_reading_window_effect(authority.clone())?,
            );
            effect()
        })?;
        receipt.ok_or_else(|| failure("reading_effect_stale", "窗口操作缺少当前保存回执"))
    }
}
