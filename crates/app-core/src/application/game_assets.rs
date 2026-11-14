//! Optional local cover hydration and first-frame attribution; never a game launcher.
use super::runtime::ApplicationRuntime;
use crate::{contracts::*, ports::GameCoverAssets};
use std::sync::{Arc, atomic::Ordering};

fn error(code: &str) -> AppError {
    AppError::new(code, "library", "游戏封面暂不可用，取文不受影响")
}
fn result(game: &Game, status: GameCoverStatus, code: Option<String>) -> GameCoverResult {
    GameCoverResult {
        game_id: game.game_id.clone(),
        asset_ref: game.cover_asset_ref.clone(),
        status,
        origin: None,
        content_hash: None,
        width: None,
        height: None,
        mime_type: None,
        payload: None,
        error_code: code,
    }
}
fn guard(
    snapshot: &AppSnapshot,
    game: &Game,
    session: Option<CoverSessionGuard>,
) -> CoverCommitGuard {
    CoverCommitGuard {
        profile_id: snapshot.profile_id.clone(),
        profile_generation: snapshot.profile_generation,
        server_instance_id: snapshot.server_instance_id.clone(),
        game_id: game.game_id.clone(),
        row_revision: game.row_revision,
        environment_key: game.target.environment_key.clone(),
        target_path_key: game.target.target_path_key.clone(),
        session,
    }
}
impl ApplicationRuntime {
    /// Bootstrap-only dependency injection keeps historical fixtures/constructors compatible.
    pub fn configure_game_assets(&self, assets: Arc<dyn GameCoverAssets>) -> AppResult<()> {
        let mut slot = self
            .game_assets
            .lock()
            .map_err(|_| error("game_cover_state"))?;
        if slot.is_some() {
            return Err(error("game_cover_already_configured"));
        }
        *slot = Some(assets);
        Ok(())
    }
    fn assets(&self) -> AppResult<Arc<dyn GameCoverAssets>> {
        self.game_assets
            .lock()
            .map_err(|_| error("game_cover_state"))?
            .clone()
            .ok_or_else(|| error("game_cover_unavailable"))
    }
    /// Materializes only missing, local cache assets. Image bytes are never published in snapshots.
    pub fn game_cover(&self, request: GameCoverRequest) -> AppResult<GameCoverResult> {
        crate::domain::require_id(&request.game_id)?;
        let snapshot = self.repository.snapshot()?;
        let mut game = snapshot
            .games
            .iter()
            .find(|g| g.game_id == request.game_id)
            .cloned()
            .ok_or_else(|| error("game_not_active"))?;
        if request.expected_asset_ref.is_some()
            && request.expected_asset_ref != game.cover_asset_ref
        {
            return Ok(result(&game, GameCoverStatus::ReferenceChanged, None));
        }
        let assets = match self.assets() {
            Ok(v) => v,
            Err(e) => return Ok(result(&game, GameCoverStatus::Unavailable, Some(e.code))),
        };
        if game.cover_asset_ref.is_none() && !self.closing.load(Ordering::Acquire) {
            match assets.import_local_cover(&game) {
                Ok(Some(cover)) => {
                    let outcome = self.repository.set_game_cover_if_empty(
                        guard(&snapshot, &game, None),
                        cover,
                        self.clock.now_ms(),
                    )?;
                    if outcome == CoverInstallOutcome::Installed {
                        self.publish()?;
                    }
                    game = self
                        .repository
                        .snapshot()?
                        .games
                        .into_iter()
                        .find(|g| g.game_id == request.game_id)
                        .ok_or_else(|| error("game_not_active"))?;
                }
                Ok(None) => {}
                Err(e) => return Ok(result(&game, GameCoverStatus::Unavailable, Some(e.code))),
            }
        }
        let Some(reference) = game.cover_asset_ref.as_deref() else {
            return Ok(result(&game, GameCoverStatus::Missing, None));
        };
        match assets.read_cover(reference) {
            Ok(image) => Ok(GameCoverResult {
                game_id: game.game_id,
                asset_ref: Some(reference.into()),
                status: GameCoverStatus::Ready,
                origin: Some(image.prepared.origin),
                content_hash: Some(image.prepared.content_hash),
                width: Some(image.prepared.width),
                height: Some(image.prepared.height),
                mime_type: Some("image/jpeg".into()),
                payload: Some(image.bytes),
                error_code: None,
            }),
            Err(e) => Ok(result(&game, GameCoverStatus::Unavailable, Some(e.code))),
        }
    }
    /// Called only by the trusted native session coordinator, never from a frame-bearing IPC.
    pub fn prepare_session_cover(
        &self,
        expected: CoverSessionGuard,
        jpeg: &[u8],
    ) -> AppResult<Option<PreparedSessionCover>> {
        if self.closing.load(Ordering::Acquire) {
            return Err(error("application_closing"));
        }
        let snapshot = self.repository.snapshot()?;
        let session = snapshot
            .session
            .as_ref()
            .filter(|s| {
                s.session_id == expected.session_id
                    && s.source_kind == "hook"
                    && matches!(s.state.as_str(), "opening" | "connected" | "active")
            })
            .ok_or_else(|| error("game_cover_stale_session"))?;
        let game = session
            .game_id
            .as_ref()
            .and_then(|id| snapshot.games.iter().find(|g| &g.game_id == id))
            .ok_or_else(|| error("game_cover_no_game"))?;
        if game.cover_asset_ref.is_some() {
            return Ok(None);
        }
        let assets = self.assets()?;
        let cover = match assets.import_local_cover(game)? {
            Some(local) => local,
            None => assets.import_session_frame(jpeg)?,
        };
        Ok(Some(PreparedSessionCover {
            guard: guard(&snapshot, game, Some(expected)),
            cover,
        }))
    }
    /// Short final commit, invoked while the desktop holds the exact preview generation lease.
    /// No filesystem or image decoding here. The DB writer makes no callbacks into native code.
    pub fn commit_session_cover(
        &self,
        prepared: PreparedSessionCover,
    ) -> AppResult<CoverInstallOutcome> {
        let _capture = self
            .capture_lock
            .lock()
            .map_err(|_| error("game_cover_state"))?;
        if self.closing.load(Ordering::Acquire) {
            return Err(error("application_closing"));
        }
        let expected = prepared
            .guard
            .session
            .as_ref()
            .ok_or_else(|| error("game_cover_stale_session"))?;
        let outcome = self.hook.with_verified_cover_binding_commit(
            GameWindowActivationRequest {
                expected_session_id: expected.session_id.clone(),
                expected_target_identity_sha256: expected.binding_identity_sha256.clone(),
            },
            &mut || {
                self.repository.set_game_cover_if_empty(
                    prepared.guard.clone(),
                    prepared.cover.clone(),
                    self.clock.now_ms(),
                )
            },
        )?;
        if outcome == CoverInstallOutcome::Installed {
            self.publish()?;
        }
        Ok(outcome)
    }
    /// Fresh external window verification runs before the preview mutex is acquired.
    /// The final commit rechecks the same full binding under its association lease.
    pub fn verify_session_cover(&self, prepared: &PreparedSessionCover) -> AppResult<()> {
        let expected = prepared
            .guard
            .session
            .as_ref()
            .ok_or_else(|| error("game_cover_stale_session"))?;
        let actual = self.hook.game_window_binding()?;
        if actual.state == GameWindowBindingState::Unavailable {
            return Err(error("game_cover_verification"));
        }
        if actual.state != GameWindowBindingState::Bound
            || actual.verified_at_ms.is_none()
            || actual.session_id.as_deref() != Some(expected.session_id.as_str())
            || actual.target_identity_sha256.as_deref()
                != Some(expected.binding_identity_sha256.as_str())
        {
            return Err(error("game_cover_stale"));
        }
        Ok(())
    }
}
