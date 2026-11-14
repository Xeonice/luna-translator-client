//! Unknown-only resource inventory over the existing bounded SQLite actor.
//! A repository, hash, inventory row or preparation owner is never a runtime handler.
use super::*;
use crate::storage::SqliteStore;
use app_core::domain::{model_config::service_graph::*, offline_runtime::*};
use app_core::ports::{offline_resources::OfflineProfileFrame, operation_budget::OperationBudget};
use std::{
    num::NonZeroU32,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};
mod authority;
mod facts;
mod observations;
mod preparation;
mod retention;
mod source;
use facts::StoredRows;
pub use facts::{
    OfflineEnvironmentMetadata, OfflineLocalLocation, OfflineMaterialLookup, OfflineResourcePair,
    OfflineSnapshotIdentity, OfflineStoredPreparationReceipt, OfflineUnknownMetadata,
    OfflineUnknownPreparation, offline_route_content_hash, offline_snapshot_content_hash,
};
pub use retention::OfflineRetentionReport;
fn fail(code: &str) -> AppError {
    AppError::new(
        code,
        "offline_resources",
        "离线材料、操作或实际存储身份已改变，请重新核对",
    )
}
#[derive(Clone)]
struct WriterFrame {
    scope: OfflineProfileFrame,
    writer: Weak<()>,
}
impl WriterFrame {
    fn read(db: &Database) -> AppResult<Self> {
        let (profile, generation, server): (String, u32, String) = db.conn.query_row("SELECT profile_id,runtime_generation,server_instance_id FROM profile_meta WHERE singleton=1 AND runtime_state='active'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(err)?;
        if profile != db.profile_id || server != db.server_id {
            return Err(fail("offline_writer_frame_mismatch"));
        }
        Ok(Self {
            scope: OfflineProfileFrame {
                profile_id: profile,
                generation: NonZeroU32::new(generation)
                    .ok_or_else(|| fail("offline_writer_frame_mismatch"))?,
                server_instance_id: server,
            },
            writer: Arc::downgrade(&db.offline_writer_identity),
        })
    }
    fn matches(&self, db: &Database) -> AppResult<()> {
        let current = Self::read(db)?;
        if self.writer.upgrade().is_none()
            || self.scope != current.scope
            || !Weak::ptr_eq(&self.writer, &current.writer)
        {
            return Err(fail("offline_writer_frame_mismatch"));
        }
        Ok(())
    }
}
enum OperationInput {
    Preparation(Arc<OfflineUnknownPreparation>),
    Revoke {
        root: GrantedRootRef,
        frame: OfflineProfileFrame,
    },
}
struct LiveEntry {
    input: OperationInput,
    budget: OperationBudget,
    live: Arc<AtomicBool>,
}
/// Original Arc/budget registry only. No file/network work, business queue or runtime qualification.
#[derive(Default)]
pub struct LiveOfflinePreparations {
    entries: Mutex<BTreeMap<String, Arc<LiveEntry>>>,
}
pub struct OfflinePreparationGuard {
    registry: Arc<LiveOfflinePreparations>,
    id: String,
    entry: Arc<LiveEntry>,
}
impl LiveOfflinePreparations {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn begin(
        self: &Arc<Self>,
        prepared: Arc<OfflineUnknownPreparation>,
        budget: OperationBudget,
    ) -> AppResult<OfflinePreparationGuard> {
        let id = prepared.operation_id().to_owned();
        self.insert(id, OperationInput::Preparation(prepared), budget)
    }
    pub fn begin_revocation(
        self: &Arc<Self>,
        operation_id: String,
        frame: OfflineProfileFrame,
        root: GrantedRootRef,
        budget: OperationBudget,
    ) -> AppResult<OfflinePreparationGuard> {
        frame.validate()?;
        require_id(&root.grant_id)?;
        facts::digest(&root.machine_fingerprint)?;
        self.insert(operation_id, OperationInput::Revoke { root, frame }, budget)
    }
    fn insert(
        self: &Arc<Self>,
        id: String,
        input: OperationInput,
        budget: OperationBudget,
    ) -> AppResult<OfflinePreparationGuard> {
        require_id(&id)?;
        budget.check()?;
        let mut map = self
            .entries
            .lock()
            .map_err(|_| fail("offline_owner_state"))?;
        if map.contains_key(&id) {
            return Err(fail("offline_operation_already_live"));
        }
        let entry = Arc::new(LiveEntry {
            input,
            budget,
            live: Arc::new(AtomicBool::new(true)),
        });
        map.insert(id.clone(), entry.clone());
        Ok(OfflinePreparationGuard {
            registry: self.clone(),
            id,
            entry,
        })
    }
    fn pin(&self, id: &str, budget: &OperationBudget) -> AppResult<Arc<LiveEntry>> {
        let map = self
            .entries
            .lock()
            .map_err(|_| fail("offline_owner_state"))?;
        let entry = map
            .get(id)
            .cloned()
            .ok_or_else(|| fail("offline_operation_not_live"))?;
        if !entry.budget.shares_with(budget) || !entry.live.load(Ordering::Acquire) {
            return Err(fail("offline_operation_identity_mismatch"));
        }
        Ok(entry)
    }
    pub fn cancel(&self, id: &str) -> AppResult<()> {
        let entry = self
            .entries
            .lock()
            .map_err(|_| fail("offline_owner_state"))?
            .get(id)
            .cloned()
            .ok_or_else(|| fail("offline_operation_not_live"))?;
        // No registry mutex may be held while cancellation waits on terminal commit.
        entry.budget.cancel();
        entry.live.store(false, Ordering::Release);
        Ok(())
    }
}
impl Drop for OfflinePreparationGuard {
    fn drop(&mut self) {
        if let Ok(mut map) = self.registry.entries.lock()
            && map
                .get(&self.id)
                .is_some_and(|e| Arc::ptr_eq(e, &self.entry))
        {
            map.remove(&self.id);
        }
        self.entry.budget.cancel();
        self.entry.live.store(false, Ordering::Release);
    }
}
fn check_entry(entry: &LiveEntry, budget: &OperationBudget) -> AppResult<()> {
    if !entry.live.load(Ordering::Acquire) || !entry.budget.shares_with(budget) {
        return Err(fail("offline_operation_not_live"));
    }
    budget.check()
}
/// Private checkpoint freezes all original SQL rows and the real writer/owner/budget.
/// ```compile_fail
/// use app_infra::storage::SqliteOfflineReadCheckpoint;
/// fn export(v: &SqliteOfflineReadCheckpoint) { let _ = serde_json::to_string(v); }
/// ```
pub struct SqliteOfflineReadCheckpoint {
    frame: WriterFrame,
    owner: Weak<LiveOfflinePreparations>,
    budget: OperationBudget,
    lookup: OfflineMaterialLookup,
    rows: StoredRows,
    row_fingerprint: String,
}
impl std::fmt::Debug for SqliteOfflineReadCheckpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteOfflineReadCheckpoint")
            .field("snapshot_id", &self.rows.snapshot_id)
            .finish_non_exhaustive()
    }
}
impl SqliteOfflineReadCheckpoint {
    pub fn local_material(
        &self,
    ) -> AppResult<crate::offline_translation::closure::StoredRuntimeMaterialSkeleton> {
        self.rows.skeleton()
    }
    pub fn local_native_path_for_owner_check(&self) -> Option<&str> {
        self.rows.local_native_path.as_deref()
    }
    pub fn opaque_locator_for_owner(&self) -> Option<&str> {
        self.rows.opaque_locator.as_deref()
    }
    pub fn snapshot_identity(&self) -> &OfflineSnapshotIdentity {
        &self.rows.identity
    }
    pub fn profile(&self) -> &RuntimeProfileRef {
        &self.rows.profile
    }
    pub fn observation(&self) -> &ObservationRef {
        &self.rows.observation
    }
    pub fn state(&self) -> &str {
        &self.rows.observation_state
    }
    pub fn row_fingerprint(&self) -> &str {
        &self.row_fingerprint
    }
}
#[derive(Clone)]
pub struct SqliteOfflineResources {
    store: SqliteStore,
    owner: Arc<LiveOfflinePreparations>,
    frame: WriterFrame,
}
impl SqliteStore {
    /// Trusted composition only. A different still-live original Owner cannot replace it.
    pub fn offline_resources_repository(
        &self,
        owner: Arc<LiveOfflinePreparations>,
    ) -> AppResult<SqliteOfflineResources> {
        let weak = Arc::downgrade(&owner);
        let frame = self.call(move |db| {
            if let Some(previous) = db
                .offline_preparation_owner
                .as_ref()
                .filter(|w| w.upgrade().is_some())
                && !Weak::ptr_eq(previous, &weak)
            {
                return Err(fail("offline_owner_already_configured"));
            }
            db.offline_preparation_owner = Some(weak);
            WriterFrame::read(db)
        })?;
        Ok(SqliteOfflineResources {
            store: self.clone(),
            owner,
            frame,
        })
    }
}
impl SqliteOfflineResources {
    pub fn frame(&self) -> &OfflineProfileFrame {
        &self.frame.scope
    }
    fn validate(&self, db: &Database) -> AppResult<()> {
        validate_repo(db, &self.frame, &Arc::downgrade(&self.owner))
    }
    pub fn commit_unknown(
        &self,
        prepared: Arc<OfflineUnknownPreparation>,
        budget: OperationBudget,
        now: i64,
    ) -> AppResult<OfflineStoredPreparationReceipt> {
        if now < 0 {
            return Err(fail("invalid_time"));
        }
        let entry = self.owner.pin(prepared.operation_id(), &budget)?;
        if !matches!(&entry.input, OperationInput::Preparation(original) if Arc::ptr_eq(original, &prepared))
        {
            return Err(fail("offline_preparation_not_original"));
        }
        if prepared.metadata.frame != self.frame.scope {
            return Err(fail("offline_writer_frame_mismatch"));
        }
        if let Some(receipt) = self.replay_preparation(prepared.clone())? {
            return Ok(receipt);
        }
        let replay = prepared.clone();
        let repo = self.clone();
        let work_budget = budget.clone();
        let result = budget.with_terminal_fence(move || {
            repo.store.call({
                let repo = repo.clone();
                move |db| {
                    repo.validate(db)?;
                    check_entry(&entry, &work_budget)?;
                    preparation::commit(db, &prepared, &entry, &work_budget, now)
                }
            })
        });
        if result.is_err()
            && budget.is_committed()
            && let Some(receipt) = self.replay_preparation(replay)?
        {
            return Ok(receipt);
        }
        result
    }
    fn replay_preparation(
        &self,
        prepared: Arc<OfflineUnknownPreparation>,
    ) -> AppResult<Option<OfflineStoredPreparationReceipt>> {
        let repo = self.clone();
        self.store.call(move |db| {
            repo.validate(db)?;
            preparation::replay(&db.conn, &prepared)
        })
    }
    pub fn read_material(
        &self,
        lookup: OfflineMaterialLookup,
        budget: OperationBudget,
    ) -> AppResult<Arc<SqliteOfflineReadCheckpoint>> {
        budget.check()?;
        let repo = self.clone();
        self.store.call(move |db| {
            repo.validate(db)?;
            budget.check()?;
            let (rows, row_fingerprint) = authority::read(db, &lookup)?;
            budget.check()?;
            Ok(Arc::new(SqliteOfflineReadCheckpoint {
                frame: repo.frame.clone(),
                owner: Arc::downgrade(&repo.owner),
                budget,
                lookup,
                rows,
                row_fingerprint,
            }))
        })
    }
    pub fn validate_material(
        &self,
        checkpoint: Arc<SqliteOfflineReadCheckpoint>,
        budget: OperationBudget,
    ) -> AppResult<()> {
        if !checkpoint.budget.shares_with(&budget)
            || !Weak::ptr_eq(&checkpoint.owner, &Arc::downgrade(&self.owner))
            || !Weak::ptr_eq(&checkpoint.frame.writer, &self.frame.writer)
        {
            return Err(fail("offline_checkpoint_identity_mismatch"));
        }
        budget.check()?;
        let repo = self.clone();
        self.store.call(move |db| {
            repo.validate(db)?;
            checkpoint.frame.matches(db)?;
            budget.check()?;
            let (rows, fingerprint) = authority::read(db, &checkpoint.lookup)?;
            if rows != checkpoint.rows || fingerprint != checkpoint.row_fingerprint {
                return Err(fail("offline_material_changed"));
            }
            budget.check()
        })
    }
    pub fn revoke_root(
        &self,
        operation_id: String,
        root: GrantedRootRef,
        budget: OperationBudget,
        now: i64,
    ) -> AppResult<()> {
        if now < 0 {
            return Err(fail("invalid_time"));
        }
        let entry = self.owner.pin(&operation_id, &budget)?;
        if !matches!(&entry.input, OperationInput::Revoke{root:original,frame} if original == &root && frame == &self.frame.scope)
        {
            return Err(fail("offline_operation_identity_mismatch"));
        }
        if self.replay_revocation(operation_id.clone(), root.clone())? {
            return Ok(());
        }
        let replay_operation = operation_id.clone();
        let replay_root = root.clone();
        let repo = self.clone();
        let wb = budget.clone();
        let result = budget.with_terminal_fence(move || {
            repo.store.call({
                let repo = repo.clone();
                move |db| {
                    repo.validate(db)?;
                    check_entry(&entry, &wb)?;
                    observations::revoke(db, &operation_id, &root, &entry, &wb, now)
                }
            })
        });
        if result.is_err()
            && budget.is_committed()
            && self.replay_revocation(replay_operation, replay_root)?
        {
            return Ok(());
        }
        result
    }
    fn replay_revocation(&self, operation: String, root: GrantedRootRef) -> AppResult<bool> {
        let repo = self.clone();
        self.store.call(move |db| {
            repo.validate(db)?;
            observations::replay_revoke(&db.conn, &operation, &root)
        })
    }
    pub fn inspect_retention(
        &self,
        snapshot_ids: Vec<String>,
    ) -> AppResult<OfflineRetentionReport> {
        let repo = self.clone();
        self.store.call(move |db| {
            repo.validate(db)?;
            retention::inspect(db, &snapshot_ids)
        })
    }
    /// No complete runtime/lease protection Owner is installed by O3A. Never delete under a fabricated empty protected list.
    pub fn collect_unreferenced(&self) -> AppResult<()> {
        Err(fail("offline_retention_owner_unavailable"))
    }
}
fn validate_repo(
    db: &Database,
    frame: &WriterFrame,
    owner: &Weak<LiveOfflinePreparations>,
) -> AppResult<()> {
    frame.matches(db)?;
    if owner.upgrade().is_none()
        || db
            .offline_preparation_owner
            .as_ref()
            .is_none_or(|actual| !Weak::ptr_eq(actual, owner) || actual.upgrade().is_none())
    {
        return Err(fail("offline_owner_not_current"));
    }
    Ok(())
}
