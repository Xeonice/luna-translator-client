use super::queries;
use app_core::{contracts::*, domain::*, ports::ProductRepository};
use rusqlite::{Connection, params};
use std::{
    path::Path,
    sync::{
        Arc,
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};
pub const PROFILE_SCHEMA_VERSION: u32 = 5;
pub(super) const SESSION_RECORDS_SQL: &str = include_str!("migrations/002_session_records.sql");
pub(super) const PROFILE_SQL: &str = include_str!("migrations/001_profile.sql");
pub(super) const CONFIG_SQL: &str = include_str!("migrations/001_config.sql");
pub(super) const HTTP_IDENTITY_SQL: &str = include_str!("migrations/003_http_service_identity.sql");
pub(super) const SERVICE_GRAPH_SQL: &str = include_str!("migrations/004_service_graph.sql");
pub(super) const SOURCE_SELECTION_SQL: &str = include_str!("migrations/005_source_selection_sets.sql");
pub(super) fn source_selection_migration_sha256() -> String { content_hash(SOURCE_SELECTION_SQL) }
const DICTIONARY_SQL: &str = include_str!("migrations/001_dictionary.sql");
pub fn profile_migration_sha256() -> String {
    content_hash(format!("{PROFILE_SQL}\n{CONFIG_SQL}"))
}
pub fn session_records_migration_sha256() -> String {
    content_hash(SESSION_RECORDS_SQL)
}
pub fn http_service_identity_migration_sha256() -> String {
    content_hash(HTTP_IDENTITY_SQL)
}
pub(super) fn err(e: impl std::fmt::Display) -> AppError {
    let text = e.to_string();
    let code = if text.contains("UNIQUE constraint") {
        "duplicate_record"
    } else if text.contains("FOREIGN KEY") {
        "invalid_reference"
    } else if text.contains("sealed") || text.contains("immutable") {
        "immutable_record"
    } else if text.contains("locked") || text.contains("busy") {
        "storage_busy"
    } else {
        "storage_failure"
    };
    AppError::new(code, "storage", text)
}
fn migration_sql_error(path: &Path, phase: &str, error: rusqlite::Error) -> AppError {
    #[cfg(test)]
    source_selection_migration_fault::observe_error(path, phase, &error);
    #[cfg(not(test))]
    let _ = (path, phase);
    err(error)
}
fn schema_identity(conn: &Connection) -> AppResult<String> {
    let mut q=conn.prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name").map_err(err)?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    canonical_hash(&rows)
}
fn expected_schema_identity(version: i64) -> AppResult<String> {
    let conn = Connection::open_in_memory().map_err(err)?;
    conn.execute_batch(PROFILE_SQL).map_err(err)?;
    conn.execute_batch(CONFIG_SQL).map_err(err)?;
    if version >= 2 {
        conn.execute_batch(SESSION_RECORDS_SQL).map_err(err)?;
    }
    if version >= 3 {
        conn.execute_batch("PRAGMA foreign_keys=OFF").map_err(err)?;
        conn.execute_batch(HTTP_IDENTITY_SQL).map_err(err)?;
    }
    if version >= 4 {
        conn.execute_batch("PRAGMA foreign_keys=OFF").map_err(err)?;
        conn.execute_batch(SERVICE_GRAPH_SQL).map_err(err)?;
    }
    if version >= 5 { conn.execute_batch(SOURCE_SELECTION_SQL).map_err(err)?; }
    schema_identity(&conn)
}
pub fn service_graph_migration_sha256() -> String {
    content_hash(SERVICE_GRAPH_SQL)
}
fn seal_backup_destination(path: &Path) -> AppResult<()> {
    let check = Connection::open(path).map_err(err)?;
    check.busy_timeout(Duration::from_secs(3)).map_err(err)?;
    let checkpoint: (i64, i64, i64) = check
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(err)?;
    if checkpoint.0 != 0 {
        return Err(AppError::new(
            "backup_busy",
            "storage",
            "独立备份目标仍有未完成的SQLite读写",
        ));
    }
    let journal: String = check
        .query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))
        .map_err(err)?;
    if journal != "delete" {
        return Err(AppError::new(
            "backup_not_standalone",
            "storage",
            "无法封存独立备份文件",
        ));
    }
    let result: String = check
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(err)?;
    let foreign_keys: u32 = check
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .map_err(err)?;
    if result != "ok" || foreign_keys != 0 {
        return Err(AppError::new(
            "backup_corrupt",
            "storage",
            "独立备份完整性或关系校验失败",
        ));
    }
    check.close().map_err(|(_, error)| err(error))?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
        match std::fs::metadata(&sidecar) {
            Ok(metadata) if metadata.len() != 0 => {
                return Err(AppError::new(
                    "backup_not_standalone",
                    "storage",
                    "独立备份仍依赖SQLite旁文件",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(err(error)),
        }
    }
    Ok(())
}
type Job = Box<dyn FnOnce(&mut Database) + Send>;
struct ProfileLease {
    file: std::fs::File,
    owner_pid: u32,
}
impl ProfileLease {
    fn acquire(file: std::fs::File) -> AppResult<Self> {
        file.try_lock().map_err(|_| {
            AppError::new(
                "profile_in_use",
                "storage",
                "此配置档已由另一个进程持有，请先退出原实例",
            )
        })?;
        Ok(Self {
            file,
            owner_pid: std::process::id(),
        })
    }
}
impl Drop for ProfileLease {
    fn drop(&mut self) {
        // close alone can leave flock held by an unrelated child before exec.
        // A forked child must never unlock the original process's live lease.
        if self.owner_pid == std::process::id() {
            let _ = self.file.unlock();
        }
    }
}
#[derive(Clone)]
pub struct SqliteStore {
    inner: Arc<StoreInner>,
}
struct StoreInner {
    sender: Option<SyncSender<Job>>,
    writer: Option<thread::JoinHandle<()>>,
    capture_pending: std::sync::Mutex<
        std::collections::BTreeMap<String, app_core::ports::runtime::HookEventInput>,
    >,
}
impl Drop for StoreInner {
    fn drop(&mut self) {
        // The final owner closes the queue before joining. The actor then drains
        // accepted work, checkpoints and releases its profile lease before a
        // caller can immediately open that profile again.
        drop(self.sender.take());
        if let Some(writer) = self.writer.take()
            && writer.thread().id() != thread::current().id()
        {
            let _ = writer.join();
        }
    }
}
pub(super) struct Database {
    // Fields drop in declaration order: close SQLite before releasing its lease.
    pub conn: Connection,
    pub profile_id: String,
    pub server_id: String,
    pub diagnostics: Vec<Diagnostic>,
    _lease: ProfileLease,
    pub pool_cursors: std::collections::BTreeMap<(String, u32), u64>,
    pub record_ranges: std::collections::BTreeMap<String, queries::session_records::FrozenRange>,
    pub record_exports: std::collections::BTreeMap<String, String>,
    pub record_range_counter: u64,
    pub service_directory: Option<Arc<dyn app_core::ports::ServiceDirectory>>,
    pub service_test_authority: Option<Arc<dyn app_core::ports::service_testing::ServiceTestOperationAuthority>>,
    pub service_model_discovery_authority: Option<Arc<dyn app_core::ports::service_discovery::ServiceModelDiscoveryAuthority>>,
    pub google_writer_identity: Arc<()>,
    pub offline_writer_identity: Arc<()>,
    pub offline_preparation_owner: Option<std::sync::Weak<queries::offline_resources::LiveOfflinePreparations>>,
    pub google_handler: Option<std::sync::Weak<app_core::ports::google_authorization::GoogleHandlerRegistration>>,
}
impl SqliteStore {
    fn needs_capture_marker(error: &AppError) -> bool {
        matches!(
            error.code.as_str(),
            "event_identity_conflict"
                | "source_identity_conflict"
                | "storage_queue_full"
                | "storage_busy"
                | "storage_failure"
                | "sequence_exhausted"
        )
    }
    fn note_capture_unconfirmed(&self, session: &str, now: i64) {
        if let Ok(mut pending) = self.inner.capture_pending.lock() {
            pending.entry(session.into()).or_insert_with(|| {
                app_core::ports::runtime::HookEventInput {
                    event_id: format!("storage-unconfirmed:{}", uuid::Uuid::new_v4()),
                    operation_id: None,
                    session_id: Some(session.into()),
                    source: None,
                    wire_sequence: None,
                    observed_at_ms: now.max(0),
                    detail: app_core::ports::runtime::HookStageInput {
                        level: "error".into(),
                        stage: "capture".into(),
                        code: "storage_unconfirmed".into(),
                        message: "接收写入未能确认，未知范围已保留".into(),
                    },
                    lost_count: None,
                    integrity: "unknown".into(),
                }
            });
        }
    }
    fn flush_store_integrity(&self, session: Option<&str>) -> AppResult<()> {
        let facts = self
            .inner
            .capture_pending
            .lock()
            .map_err(|_| {
                AppError::new(
                    "storage_integrity_state",
                    "storage",
                    "完整性待确认状态不可用",
                )
            })?
            .values()
            .filter(|p| session.is_none() || p.session_id.as_deref() == session)
            .cloned()
            .collect::<Vec<_>>();
        for fact in facts {
            let saved = fact.clone();
            self.call(move |db| queries::session_records::record_event(db, saved))?;
            if let Some(session) = fact.session_id {
                let mut pending = self.inner.capture_pending.lock().map_err(err)?;
                if pending
                    .get(&session)
                    .is_some_and(|p| p.event_id == fact.event_id)
                {
                    pending.remove(&session);
                }
            }
        }
        Ok(())
    }
    // Observation-only test host entry. Neither this method nor its private
    // path-bound journal exists in a non-test library or product executable.
    #[cfg(test)]
    pub(crate) fn observe_schema5_open(path: &Path, route: &str, exact_test: &str, profile: &str, server: &str) -> source_selection_migration_fault::Scope {
        source_selection_migration_fault::Scope::new(path, source_selection_migration_fault::Mode::Observe, route, exact_test, profile, server)
    }
    #[cfg(test)]
    pub(crate) fn observe_schema5_fresh_open(path: &Path, exact_test: &str, profile: &str, server: &str) -> source_selection_migration_fault::Scope {
        source_selection_migration_fault::Scope::new_fresh(path, exact_test, profile, server)
    }
    #[cfg(test)]
    pub(crate) fn observe_schema5_kill_boundary(path: &Path, profile: &str, server: &str) -> source_selection_migration_fault::Scope {
        source_selection_migration_fault::Scope::new(path, source_selection_migration_fault::Mode::PauseAfterDdl, "kill", "bj_11_storage", profile, server)
    }
    pub fn open(
        path: impl AsRef<Path>,
        profile_id: &str,
        server_id: &str,
        now_ms: i64,
    ) -> AppResult<Self> {
        #[cfg(test)]
        source_selection_migration_fault::api_entry(path.as_ref(), profile_id, server_id, now_ms);
        require_id(profile_id)?;
        require_id(server_id)?;
        if now_ms < 0 {
            return Err(AppError::new("invalid_time", "storage", "时间不得为负"));
        }
        let path = path.as_ref().to_path_buf();
        #[cfg(test)]
        let observed_path = path.clone();
        let profile_id = profile_id.to_owned();
        let server_id = server_id.to_owned();
        let (sender, receiver) = mpsc::sync_channel::<Job>(128);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let writer = thread::Builder::new()
            .name("luna-sqlite-writer".into())
            .spawn(
                move || match Database::open(&path, profile_id, server_id, now_ms) {
                    Ok(mut db) => {
                        let _ = ready_tx.send(Ok(()));
                        while let Ok(job) = receiver.recv() {
                            job(&mut db);
                        }
                        let _ = db.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
                    }
                    Err(e) => {
                        #[cfg(test)]
                        source_selection_migration_fault::boundary(&path, "writer_error_published", serde_json::json!({"error_code":e.code}));
                        let _ = ready_tx.send(Err(e));
                    }
                },
            )
            .map_err(err)?;
        if let Err(error) = ready_rx.recv().map_err(err).and_then(|result| result) {
            drop(sender);
            let _ = writer.join();
            #[cfg(test)]
            source_selection_migration_fault::boundary(&observed_path, "api_error_return_ready", serde_json::json!({"error_code":error.code}));
            return Err(error);
        }
        #[cfg(test)]
        source_selection_migration_fault::boundary(&observed_path, "api_success_return_ready", serde_json::json!({}));
        Ok(Self {
            inner: Arc::new(StoreInner {
                sender: Some(sender),
                writer: Some(writer),
                capture_pending: std::sync::Mutex::new(std::collections::BTreeMap::new()),
            }),
        })
    }
    /// Startup-only injection of the same immutable directory shared by application/gateway.
    pub fn configure_service_directory(&self, directory: Arc<dyn app_core::ports::ServiceDirectory>) -> AppResult<()> {
        self.call(move |db| {
            if db.service_directory.is_some() {
                return Err(AppError::new("service_directory_already_configured", "configuration", "服务目录仅允许在启动时注入一次"));
            }
            db.service_directory = Some(directory);
            Ok(())
        })
    }
    pub(super) fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Database) -> AppResult<T> + Send + 'static,
    ) -> AppResult<T> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.inner
            .sender
            .as_ref()
            .ok_or_else(|| AppError::new("storage_closed", "storage", "存储执行器已关闭"))?
            .try_send(Box::new(move |db| {
                let _ = tx.send(f(db));
            }))
            .map_err(|_| {
                AppError::new(
                    "storage_queue_full",
                    "storage",
                    "存储执行器繁忙，请稍后重试",
                )
            })?;
        rx.recv().map_err(err)?
    }
    pub fn backup(&self, destination: impl AsRef<Path>) -> AppResult<()> {
        let destination = destination.as_ref().to_path_buf();
        self.call(move |db| {
            if destination.exists() {
                return Err(AppError::new("backup_exists", "storage", "备份目标已存在"));
            }
            if let Some(p) = destination.parent() {
                std::fs::create_dir_all(p).map_err(err)?;
            }
            db.conn.backup("main", &destination, None).map_err(err)?;
            // This is the new, independent destination, never the live source.
            // A read-only WAL verifier can leave SHM after close. Let SQLite
            // checkpoint and close its own destination journal before delivery.
            seal_backup_destination(&destination)?;
            std::fs::File::open(&destination)
                .and_then(|file| file.sync_all())
                .map_err(err)?;
            Ok(())
        })
    }
    pub fn checkpoint(&self) -> AppResult<()> {
        self.call(|db| {
            db.conn
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .map_err(err)
        })
    }
    pub fn database_versions(&self) -> AppResult<(String, String)> {
        self.call(|db| {
            Ok((
                db.conn
                    .query_row("SELECT sqlite_version()", [], |r| r.get(0))
                    .map_err(err)?,
                db.conn
                    .query_row("SELECT sqlite_source_id()", [], |r| r.get(0))
                    .map_err(err)?,
            ))
        })
    }
}
impl Database {
    fn open(path: &Path, profile_id: String, server_id: String, now: i64) -> AppResult<Self> {
        #[cfg(test)]
        source_selection_migration_fault::boundary(path, "database_writer_entry", serde_json::json!({}));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(err)?;
        }
        let lease_path = path.with_extension("sqlite3.lock");
        if lease_path.exists()
            && std::fs::symlink_metadata(&lease_path)
                .map_err(err)?
                .file_type()
                .is_symlink()
        {
            return Err(AppError::new(
                "unsafe_profile_lock",
                "storage",
                "配置档锁不得是符号链接",
            ));
        }
        let lease = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lease_path)
            .map_err(err)?;
        let lease = ProfileLease::acquire(lease)?;

        let mut installed_version = None;
        if path.exists() {
            if std::fs::symlink_metadata(path)
                .map_err(err)?
                .file_type()
                .is_symlink()
            {
                return Err(AppError::new(
                    "unsafe_profile_path",
                    "storage",
                    "配置档不得是符号链接",
                ));
            }
            let check =
                Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .map_err(err)?;
            let version: Option<i64> = check
                .query_row("SELECT max(version) FROM schema_migrations", [], |r| {
                    r.get(0)
                })
                .map_err(|_| {
                    AppError::new(
                        "unsupported_schema",
                        "storage",
                        "拒绝打开无版本旧配置档；原件未改动",
                    )
                })?;
            if !matches!(version, Some(1..=5)) {
                return Err(AppError::new(
                    "unsupported_schema",
                    "storage",
                    "不支持此配置档版本；原件未改动",
                ));
            }
            installed_version = version;
            let actual: String = check
                .query_row(
                    "SELECT profile_id FROM profile_meta WHERE singleton=1",
                    [],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if actual != profile_id {
                return Err(AppError::new(
                    "profile_mismatch",
                    "storage",
                    "配置档身份不匹配；原件未改动",
                ));
            }
            let migration: String = check
                .query_row(
                    "SELECT migration_sha256 FROM schema_migrations WHERE version=1",
                    [],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if migration != profile_migration_sha256() {
                return Err(AppError::new(
                    "migration_mismatch",
                    "storage",
                    "已安装迁移身份不一致；原件未改动",
                ));
            }
            if version.is_some_and(|v| v >= 2) {
                let migration2: String = check
                    .query_row(
                        "SELECT migration_sha256 FROM schema_migrations WHERE version=2",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                if migration2 != session_records_migration_sha256() {
                    return Err(AppError::new(
                        "migration_mismatch",
                        "storage",
                        "已安装会话迁移身份不一致；原件未改动",
                    ));
                }
            }
            if version.is_some_and(|v| v >= 3) {
                let migration3: String = check
                    .query_row(
                        "SELECT migration_sha256 FROM schema_migrations WHERE version=3",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                if migration3 != http_service_identity_migration_sha256() {
                    return Err(AppError::new(
                        "migration_mismatch",
                        "storage",
                        "已安装HTTP身份迁移不一致；原件未改动",
                    ));
                }
            }
            if version.is_some_and(|v| v >= 4) {
                let actual: String = check
                    .query_row(
                        "SELECT migration_sha256 FROM schema_migrations WHERE version=4",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                if actual != service_graph_migration_sha256() {
                    return Err(AppError::new(
                        "migration_mismatch",
                        "storage",
                        "已安装服务图迁移不一致；原件未改动",
                    ));
                }
            }
            if version == Some(5) {
                let actual: String = check.query_row("SELECT migration_sha256 FROM schema_migrations WHERE version=5", [], |r| r.get(0)).map_err(err)?;
                if actual != source_selection_migration_sha256() {
                    return Err(AppError::new("migration_mismatch", "storage", "已安装正文集合迁移身份不一致；原件未改动"));
                }
            }
            if schema_identity(&check)? != expected_schema_identity(version.unwrap_or(0))? {
                return Err(AppError::new(
                    "schema_contract_mismatch",
                    "storage",
                    "配置档字段、关系、索引或触发器与生产迁移不一致；原件未改动",
                ));
            }
            let violations: u32 = check
                .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })
                .map_err(err)?;
            if violations != 0 {
                return Err(AppError::new(
                    "database_relationship_corrupt",
                    "storage",
                    "配置档存在关系完整性错误；原件未改动",
                ));
            }
            let integrity: String = check
                .query_row("PRAGMA integrity_check", [], |r| r.get(0))
                .map_err(err)?;
            if integrity != "ok" {
                return Err(AppError::new(
                    "database_corrupt",
                    "storage",
                    "配置档完整性检查失败；请从备份恢复",
                ));
            }
            queries::source_selection::validate_database(&check, version.unwrap_or(0))?;
            if version.is_some_and(|v| v < 5) {
                let backup = path.with_file_name(format!(
                    "{}.pre-v5-{now}.sqlite3",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
                let partial = backup.with_extension("sqlite3.incomplete");
                if std::fs::symlink_metadata(&backup).is_ok()
                    || std::fs::symlink_metadata(&partial).is_ok()
                {
                    return Err(AppError::new(
                        "migration_backup_failed",
                        "storage",
                        "迁移备份或未完成文件已存在，请保留并检查后重试",
                    ));
                }
                check.backup("main", &partial, None).map_err(|_| {
                    AppError::new(
                        "migration_backup_failed",
                        "storage",
                        "迁移前独立备份失败，拒绝升级；未完成文件保留",
                    )
                })?;
                seal_backup_destination(&partial)?;
                let restored = Connection::open_with_flags(
                    &partial,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )
                .map_err(err)?;
                let integrity: String = restored
                    .query_row("PRAGMA integrity_check", [], |r| r.get(0))
                    .map_err(err)?;
                let fk: i64 = restored
                    .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                        r.get(0)
                    })
                    .map_err(err)?;
                if integrity != "ok"
                    || fk != 0
                    || schema_identity(&restored)?
                        != expected_schema_identity(version.unwrap_or(0))?
                {
                    return Err(AppError::new(
                        "migration_backup_failed",
                        "storage",
                        "迁移备份校验失败，原配置档未升级",
                    ));
                }
                drop(restored);
                // Same-directory atomic create without replacing a reviewed
                // backup, including an existing/dangling symbolic link.
                std::fs::hard_link(&partial, &backup).map_err(|_| {
                    AppError::new(
                        "migration_backup_failed",
                        "storage",
                        "无法原子交付已验证备份，拒绝升级",
                    )
                })?;
                std::fs::remove_file(&partial).map_err(err)?;
                std::fs::File::open(&backup)
                    .and_then(|f| f.sync_all())
                    .map_err(err)?;
                #[cfg(unix)]
                if let Some(parent) = backup.parent() {
                    std::fs::File::open(parent)
                        .and_then(|f| f.sync_all())
                        .map_err(err)?;
                }
                #[cfg(test)]
                source_selection_migration_fault::boundary(path, "backup_ready", serde_json::json!({"backup":backup,"backup_sha256":content_hash(std::fs::read(&backup).map_err(err)?)}));
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(err)?;
        }
        #[cfg(test)]
        source_selection_migration_fault::before_write(path)?;
        let mut conn = Connection::open(path).map_err(|error| migration_sql_error(path, "connection_open", error))?;
        #[cfg(test)]
        source_selection_migration_fault::connection_opened(path);
        conn.busy_timeout(Duration::from_secs(3)).map_err(err)?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )
        .map_err(|error| migration_sql_error(path, "pragmas", error))?;
        let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",[],|r|r.get(0)).map_err(err)?;
        // Rebuilding FK targets requires OFF before BEGIN, never toggled inside a transaction.
        conn.execute_batch("PRAGMA foreign_keys=OFF").map_err(err)?;
        #[cfg(test)]
        let _transaction_exit = source_selection_migration_fault::TransactionExit::new(path);
        #[cfg(test)]
        source_selection_migration_fault::boundary(path, "transaction_attempted", serde_json::json!({"sql":"BEGIN IMMEDIATE"}));
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|error| migration_sql_error(path, "begin", error))?;
        #[cfg(test)]
        source_selection_migration_fault::transaction_started(path);
        let old_rows = if exists && installed_version.is_some_and(|v| v < 4) {
            Some(queries::configuration_codec::legacy_rows(&tx)?)
        } else {
            None
        };
        if !exists {
            tx.execute_batch(PROFILE_SQL).map_err(err)?;
            tx.execute_batch(CONFIG_SQL).map_err(err)?;
            tx.execute(
                "INSERT INTO schema_migrations VALUES(1,?1,?2,?3,0)",
                params![profile_migration_sha256(), now, env!("CARGO_PKG_VERSION")],
            )
            .map_err(err)?;
            tx.execute_batch(SESSION_RECORDS_SQL).map_err(err)?;
            tx.execute(
                "INSERT INTO schema_migrations VALUES(2,?1,?2,?3,1)",
                params![
                    session_records_migration_sha256(),
                    now,
                    env!("CARGO_PKG_VERSION")
                ],
            )
            .map_err(err)?;
            tx.execute_batch(HTTP_IDENTITY_SQL).map_err(err)?;
            tx.execute(
                "INSERT INTO schema_migrations VALUES(3,?1,?2,?3,2)",
                params![
                    http_service_identity_migration_sha256(),
                    now,
                    env!("CARGO_PKG_VERSION")
                ],
            )
            .map_err(err)?;
            tx.execute_batch(SERVICE_GRAPH_SQL).map_err(err)?;
            tx.execute(
                "INSERT INTO schema_migrations VALUES(4,?1,?2,?3,3)",
                params![
                    service_graph_migration_sha256(),
                    now,
                    env!("CARGO_PKG_VERSION")
                ],
            )
            .map_err(err)?;
            tx.execute(
                "INSERT INTO profile_meta VALUES(1,?1,0,1,?2,'active',?3,?3)",
                params![profile_id, server_id, now],
            )
            .map_err(err)?;
            tx.execute_batch(SOURCE_SELECTION_SQL).map_err(|error| migration_sql_error(path, "ddl", error))?;
            #[cfg(test)]
            source_selection_migration_fault::after_ddl(path, &tx)?;
            tx.execute("INSERT INTO schema_migrations VALUES(5,?1,?2,?3,4)", params![source_selection_migration_sha256(), now, env!("CARGO_PKG_VERSION")]).map_err(err)?;
        } else {
            if installed_version == Some(1) {
                tx.execute_batch(SESSION_RECORDS_SQL).map_err(err)?;
                tx.execute(
                    "INSERT INTO schema_migrations VALUES(2,?1,?2,?3,1)",
                    params![
                        session_records_migration_sha256(),
                        now,
                        env!("CARGO_PKG_VERSION")
                    ],
                )
                .map_err(err)?;
                if schema_identity(&tx)? != expected_schema_identity(2)? {
                    return Err(AppError::new(
                        "schema_contract_mismatch",
                        "storage",
                        "升级后字段或关系不符合已评审迁移；事务已回滚",
                    ));
                }
            }
            if installed_version.is_some_and(|v| v < 3) {
                tx.execute_batch(HTTP_IDENTITY_SQL).map_err(err)?;
                #[cfg(test)]
                http_migration_fault::inject_sqlite_failure_once(&tx)?;
                tx.execute(
                    "INSERT INTO schema_migrations VALUES(3,?1,?2,?3,2)",
                    params![
                        http_service_identity_migration_sha256(),
                        now,
                        env!("CARGO_PKG_VERSION")
                    ],
                )
                .map_err(err)?;
                if let Some(before) = &old_rows {
                    queries::configuration_codec::verify_legacy_rows(&tx, before)?;
                }
            }
            if installed_version.is_some_and(|v| v < 4) {
                tx.execute_batch(SERVICE_GRAPH_SQL).map_err(err)?;
                #[cfg(test)]
                service_graph_migration_fault::inject_sqlite_failure_once(&tx)?;
                tx.execute(
                    "INSERT INTO schema_migrations VALUES(4,?1,?2,?3,3)",
                    params![
                        service_graph_migration_sha256(),
                        now,
                        env!("CARGO_PKG_VERSION")
                    ],
                )
                .map_err(err)?;
                if let Some(before) = &old_rows {
                    queries::configuration_codec::verify_legacy_rows(&tx, before)?;
                }
            }
            if installed_version.is_some_and(|v| v < 5) {
                tx.execute_batch(SOURCE_SELECTION_SQL).map_err(|error| migration_sql_error(path, "ddl", error))?;
                #[cfg(test)]
                source_selection_migration_fault::after_ddl(path, &tx)?;
                queries::source_selection::validate_database(&tx, 5)?;
                tx.execute("INSERT INTO schema_migrations VALUES(5,?1,?2,?3,4)", params![source_selection_migration_sha256(), now, env!("CARGO_PKG_VERSION")]).map_err(err)?;
            }
            if schema_identity(&tx)? != expected_schema_identity(5)? {
                return Err(AppError::new(
                    "schema_contract_mismatch",
                    "storage",
                    "HTTP迁移后结构不符，事务已回滚",
                ));
            }
            let violations: u32 = tx
                .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })
                .map_err(err)?;
            if violations != 0 {
                return Err(AppError::new(
                    "database_relationship_corrupt",
                    "storage",
                    "HTTP迁移后关系不符，事务已回滚",
                ));
            }
            tx.execute("UPDATE profile_meta SET runtime_generation=runtime_generation+1,server_instance_id=?1,runtime_state='active',updated_at_ms=?2 WHERE singleton=1",params![server_id,now]).map_err(err)?;
            tx.execute("UPDATE request_tasks SET state='invalidated',invalidation_code='application_restarted',finished_at_ms=?1 WHERE state IN ('queued','running')",[now]).map_err(err)?;
            tx.execute("UPDATE request_attempts SET state='cancelled',error_code='application_restarted',finished_at_ms=?1 WHERE state='started'",[now]).map_err(err)?;
            tx.execute("UPDATE sessions SET state='crashed',ended_at_ms=?1 WHERE state IN ('opening','active')",[now]).map_err(err)?;
            tx.execute("UPDATE capture_operations SET state=CASE WHEN session_id IS NULL THEN 'failed' ELSE 'recovered' END,ended_at_ms=?1 WHERE state IN ('opening','active')",[now]).map_err(err)?;
            tx.execute(
                "UPDATE reading_units SET state='finalized' WHERE state='open'",
                [],
            )
            .map_err(err)?;
        }
        if schema_identity(&tx)? != expected_schema_identity(5)? {
            return Err(AppError::new(
                "schema_contract_mismatch",
                "storage",
                "新库结构不符，事务已回滚",
            ));
        }
        let violations: u32 = tx
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .map_err(err)?;
        if violations != 0 {
            return Err(AppError::new(
                "database_relationship_corrupt",
                "storage",
                "新库关系不符，事务已回滚",
            ));
        }
        tx.commit().map_err(|error| migration_sql_error(path, "commit", error))?;
        #[cfg(test)]
        source_selection_migration_fault::boundary(path, "commit_succeeded", serde_json::json!({}));
        conn.execute_batch("PRAGMA foreign_keys=ON").map_err(err)?;
        if conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get::<_, u32>(0))
            .map_err(err)?
            != 1
        {
            return Err(AppError::new(
                "foreign_keys_disabled",
                "storage",
                "无法恢复关系约束",
            ));
        }
        Ok(Self {
            conn,
            profile_id,
            server_id,
            diagnostics: vec![],
            _lease: lease,
            pool_cursors: std::collections::BTreeMap::new(),
            record_ranges: std::collections::BTreeMap::new(),
            record_exports: std::collections::BTreeMap::new(),
            record_range_counter: 0,
            service_directory: None,
            service_test_authority: None,
            service_model_discovery_authority: None,
            google_writer_identity: Arc::new(()),
            offline_writer_identity: Arc::new(()),
            offline_preparation_owner: None,
            google_handler: None,
        })
    }
}
impl ProductRepository for SqliteStore {
    fn association_progress_facts(&self, operation_id: &str) -> AppResult<app_core::ports::runtime::AssociationOperationFacts> {
        let operation = operation_id.to_owned();
        self.call(move |db| queries::session_records::association_progress_facts(db, &operation))
    }
    fn source_selection(&self, query: SourceSelectionQuery) -> AppResult<SourceSelectionProjection> {
        self.call(move |db| queries::source_selection::source_selection(db, query))
    }
    fn commit_source_selection(&self, input: app_core::ports::runtime::SourceSelectionCommit, now: i64) -> AppResult<SelectionCommitReceipt> {
        self.call(move |db| queries::source_selection::commit(db, input, now))
    }
    fn validate_reading_window_effect(&self, authority: ReadingWindowEffectAuthority) -> AppResult<SelectionCommitReceipt> {
        self.call(move |db| queries::source_selection::validate_effect(db, authority))
    }
    fn google_runtime_frame(&self) -> AppResult<app_core::ports::google_authorization::GoogleRuntimeFrame> {
        self.call(move |db| queries::google_runtime_frame(db))
    }

    fn prepare_work_tasks_for_operation(&self,unit_id:String,text_revision:u32,purpose:String,operation_id:String,now:i64)->AppResult<Vec<app_core::domain::service_dispatch::WorkDispatch>>{
        self.call(move|db|queries::service_tasks::prepare(db,queries::service_tasks::TaskPreparation{unit_id:&unit_id,text_revision,purpose:&purpose,operation_id:Some(&operation_id),selected:None,bypass_cache:false,now}))
    }
    fn prepare_work_study_tasks(&self,unit_id:String,text_revision:u32,purpose:String,token_index:Option<u32>,operation_id:String,now:i64)->AppResult<Vec<app_core::domain::service_dispatch::WorkDispatch>>{
        self.call(move|db|{
            if text_revision==0||!matches!(purpose.as_str(),"grammar"|"context_word"){return Err(AppError::new("invalid_study_object","study","学习必须固定原文版本和用途"))}
            let unit=queries::unit(&db.conn,&unit_id,text_revision)?;
            let selected=if purpose=="context_word"{Some(unit.tokens.get(token_index.ok_or_else(||AppError::new("missing_selection","study","请明确选择词语"))? as usize).ok_or_else(||AppError::new("stale_token","study","所选词语已失效"))?.clone())}else{None};
            queries::service_tasks::prepare(db,queries::service_tasks::TaskPreparation{unit_id:&unit_id,text_revision,purpose:&purpose,operation_id:Some(&operation_id),selected,bypass_cache:false,now})
        })
    }
    fn prepare_work_uncached_translation_for_operation(&self,unit_id:String,text_revision:u32,operation_id:String,now:i64)->AppResult<app_core::domain::service_dispatch::PreparedWorkTranslation>{self.call(move|db|queries::service_tasks::prepared_translation(db,&unit_id,text_revision,&operation_id,now))}
    fn prepare_work_capture_tasks(&self,version:app_core::domain::capture::CaptureVersion,operation_id:String,now:i64)->AppResult<Vec<app_core::domain::service_dispatch::WorkDispatch>>{
        self.call(move|db|{
            let authorized:bool=db.conn.query_row("SELECT EXISTS(SELECT 1 FROM reading_unit_revisions r JOIN reading_units u USING(unit_id) JOIN sessions s ON s.session_id=u.session_id WHERE r.unit_id=?1 AND r.text_revision=?2 AND u.state='finalized' AND r.text_revision=(SELECT max(text_revision) FROM reading_unit_revisions WHERE unit_id=u.unit_id) AND s.source_kind='hook' AND s.state IN ('opening','active') AND EXISTS(SELECT 1 FROM current_source_selection_members m WHERE m.session_id=s.session_id AND m.selection_revision=r.selection_revision))",params![version.unit_id,version.text_revision],|r|r.get(0)).map_err(err)?;
            if !authorized{return Ok(vec![])}
            queries::service_tasks::prepare(db,queries::service_tasks::TaskPreparation{unit_id:&version.unit_id,text_revision:version.text_revision,purpose:"translation",operation_id:Some(&operation_id),selected:None,bypass_cache:false,now})
        })
    }
    fn validate_service_dispatch(&self,dispatch:&app_core::domain::service_dispatch::ServiceTaskDispatch)->AppResult<()>{let dispatch=dispatch.clone();self.call(move|db|queries::service_tasks::validate_dispatch(db,&dispatch))}

    fn set_game_cover_if_empty(
        &self,
        guard: CoverCommitGuard,
        cover: PreparedGameCover,
        now: i64,
    ) -> AppResult<CoverInstallOutcome> {
        self.call(move |db| queries::game_assets::set_game_cover_if_empty(db, guard, cover, now))
    }
    fn preflight_window_unbind(
        &self,
        envelope: CommandEnvelope,
    ) -> AppResult<Option<CommandReceipt>> {
        self.call(move |db| queries::preflight_window_unbind(db, envelope))
    }
    fn commit_window_unbind(
        &self,
        envelope: CommandEnvelope,
        now: i64,
    ) -> AppResult<CommandReceipt> {
        self.call(move |db| queries::commit_window_unbind(db, envelope, now))
    }
    fn preflight_disconnect(&self, envelope: CommandEnvelope) -> AppResult<Option<CommandReceipt>> {
        self.call(move |db| queries::preflight_disconnect(db, envelope))
    }
    fn flush_capture_integrity(&self, session: Option<String>) -> AppResult<()> {
        self.flush_store_integrity(session.as_deref())
    }

    fn preview_preferences(&self, preferences: Preferences) -> AppResult<Preferences> {
        self.call(move |db| queries::preview_preferences(&db.conn, &preferences))
    }
    fn preferences_scope(&self, scope_key: String) -> AppResult<ScopedPreferences> {
        self.call(move |db| queries::preferences_scope(&db.conn, &scope_key))
    }
    fn begin_capture_operation(&self, operation: String, hash: String, now: i64) -> AppResult<()> {
        self.call(move |db| queries::session_records::begin_operation(db, &operation, &hash, now))
    }
    fn finish_capture_operation(
        &self,
        operation: String,
        session: Option<String>,
        now: i64,
    ) -> AppResult<()> {
        self.call(move |db| {
            queries::session_records::finish_operation(db, &operation, session.as_deref(), now)
        })
    }
    fn record_hook_event(&self, input: app_core::ports::runtime::HookEventInput) -> AppResult<()> {
        let session = input.session_id.clone();
        let now = input.observed_at_ms;
        let result = self.call(move |db| queries::session_records::record_event(db, input));
        if result.as_ref().is_err_and(Self::needs_capture_marker)
            && let Some(session) = session
        {
            self.note_capture_unconfirmed(&session, now);
        }
        result
    }
    fn ingest_hook_text_with_policy(
        &self,
        input: app_core::domain::capture::HookTextInput,
        consume_body: bool,
        now: i64,
    ) -> AppResult<app_core::domain::capture::CaptureOutcome> {
        let session = input.session_id.clone();
        let ready = self.flush_store_integrity(Some(&session)).is_ok();
        let result = self.call(move |db| {
            queries::ingest_hook_text_with_policy(db, input, consume_body && ready, now)
        });
        if result.as_ref().is_err_and(Self::needs_capture_marker) {
            self.note_capture_unconfirmed(&session, now);
        }
        result
    }
    fn session_records(
        &self,
        query: SessionRecordQuery,
        _now: i64,
    ) -> AppResult<SessionRecordPage> {
        self.flush_store_integrity(Some(&query.session_id))?;
        self.call(move |db| queries::session_records::page(db, query))
    }
    fn prepare_session_export(
        &self,
        request: SessionExportRequest,
        _now: i64,
    ) -> AppResult<SessionExportPlan> {
        self.flush_store_integrity(Some(&request.query.session_id))?;
        self.call(move |db| queries::session_records::prepare_export(db, request))
    }
    fn session_export_page(
        &self,
        id: String,
        cursor: Option<String>,
        size: u32,
        _now: i64,
    ) -> AppResult<SessionRecordPage> {
        let export = id.clone();
        let session = self.call(move |db| queries::session_records::export_session(db, &export))?;
        self.flush_store_integrity(Some(&session))?;
        self.call(move |db| queries::session_records::export_page(db, &id, cursor, size))
    }
    fn close_session_export(&self, id: String) -> AppResult<()> {
        self.call(move |db| queries::session_records::close_export(db, &id))
    }
    fn prepare_uncached_translation_for_operation(
        &self,
        unit: String,
        revision: u32,
        operation: String,
        now: i64,
    ) -> AppResult<PreparedTranslation> {
        self.call(move |db| {
            queries::prepare_uncached_translation_for_operation(
                db, &unit, revision, &operation, now,
            )
        })
    }
    fn unit(&self, id: String, revision: u32) -> AppResult<ReadingUnit> {
        self.call(move |db| queries::unit(&db.conn, &id, revision))
    }
    fn reading_unit_projection(
        &self,
        id: String,
        revision: u32,
    ) -> AppResult<ReadingUnitProjection> {
        self.call(move |db| {
            let tx = db
                .conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)
                .map_err(err)?;
            let projection = queries::reading_unit_projection(&tx, &id, revision)?;
            tx.commit().map_err(err)?;
            Ok(projection)
        })
    }
    fn prepare_tasks_for_operation(
        &self,
        unit: String,
        revision: u32,
        purpose: String,
        operation: String,
        now: i64,
    ) -> AppResult<Vec<TaskDispatch>> {
        self.call(move |db| {
            queries::prepare_tasks_for_operation(
                db,
                &unit,
                revision,
                &purpose,
                Some(&operation),
                now,
            )
        })
    }
    fn record_model_test(
        &self,
        input: app_core::domain::testing::ModelTestRecordInput,
    ) -> AppResult<()> {
        self.call(move |db| queries::record_model_test(db, input))
    }
    fn credential_test_reference(
        &self,
        pool: String,
        revision: u32,
        vault: String,
        outcome: String,
    ) -> AppResult<app_core::domain::testing::TestCredentialUse> {
        self.call(move |db| {
            queries::credential_test_reference(db, &pool, revision, &vault, &outcome)
        })
    }
    fn prepare_study_tasks(
        &self,
        unit: String,
        revision: u32,
        purpose: String,
        token: Option<u32>,
        operation: String,
        now: i64,
    ) -> AppResult<Vec<TaskDispatch>> {
        self.call(move |db| {
            queries::prepare_study_tasks(db, &unit, revision, &purpose, token, &operation, now)
        })
    }
    fn maintain_storage(
        &self,
        policy: app_core::domain::retention::RetentionPolicy,
        protected: app_core::domain::retention::ProtectedObjects,
        operation: String,
        now: i64,
    ) -> AppResult<app_core::domain::retention::MaintenanceReceipt> {
        self.call(move |db| queries::maintain_storage(db, policy, protected, &operation, now))
    }
    fn select_credential(
        &self,
        pool: String,
        rev: u32,
    ) -> AppResult<app_core::domain::credentials::CredentialSelection> {
        self.call(move |db| queries::select_credential(db, &pool, rev))
    }
    fn store_provider_preset(&self, preset: StoredProviderPreset, now: i64) -> AppResult<()> {
        self.call(move |db| queries::provider_presets::store_provider_preset(db, preset, now))
    }
    fn provider_preset(&self, key: ProviderPresetKey) -> AppResult<StoredProviderPreset> {
        self.call(move |db| queries::provider_presets::provider_preset(&db.conn, &key))
    }
    fn select_credential_for_kind(
        &self,
        pool: String,
        rev: u32,
        kind: app_core::domain::model_config::credentials::CredentialMaterialKind,
    ) -> AppResult<app_core::domain::credentials::CredentialSelection> {
        self.call(move |db| queries::select_credential_for_kind(db, &pool, rev, kind))
    }
    fn credential_lease_metadata(
        &self,
        pool: String,
        rev: u32,
        vault: String,
        kind: app_core::domain::model_config::credentials::CredentialMaterialKind,
    ) -> AppResult<app_core::domain::credentials::CredentialLeaseMetadata> {
        self.call(move |db| queries::credential_lease_metadata(db, &pool, rev, &vault, kind))
    }
    fn credential_pool_materials(
        &self,
        pool_id: String,
        pool_revision: u32,
        expected_kind: app_core::domain::model_config::credentials::CredentialMaterialKind,
    ) -> AppResult<Vec<app_core::domain::credentials::CredentialLeaseMetadata>> {
        self.call(move |db| queries::credential_pool_materials(db, &pool_id, pool_revision, expected_kind))
    }
    fn begin_credential_material(
        &self,
        reference: CredentialReference,
        storage: app_core::domain::model_config::credentials::CredentialStorageMetadata,
        now: i64,
    ) -> AppResult<()> {
        self.call(move |db| queries::begin_credential_material(db, reference, storage, now))
    }
    fn snapshot(&self) -> AppResult<AppSnapshot> {
        self.call(queries::snapshot)
    }
    fn dispatch(&self, e: CommandEnvelope, now: i64) -> AppResult<CommandReceipt> {
        if matches!(e.command, Command::Disconnect) {
            self.flush_store_integrity(None)?;
        }
        self.call(move |db| queries::dispatch(db, e, now))
    }
    fn history(
        &self,
        game: Option<String>,
        cursor: Option<String>,
        limit: u32,
    ) -> AppResult<HistoryPage> {
        self.call(move |db| {
            queries::history(
                &db.conn,
                game.as_deref(),
                cursor.as_deref(),
                limit.clamp(1, 100),
            )
        })
    }
    fn history_page(&self, query: HistoryQuery) -> AppResult<HistoryPage> {
        self.call(move |db| queries::history_page(&db.conn, query))
    }
    fn history_sessions(&self, query: HistorySessionsQuery) -> AppResult<HistorySessionsPage> {
        self.call(move |db| queries::history_sessions(&db.conn, query))
    }
    fn prepare_tasks(
        &self,
        unit: String,
        revision: u32,
        purpose: String,
        now: i64,
    ) -> AppResult<Vec<TaskDispatch>> {
        self.call(move |db| queries::prepare_tasks(db, &unit, revision, &purpose, now))
    }
    fn complete_task(&self, result: TaskCompletion, now: i64) -> AppResult<CommandReceipt> {
        self.call(move |db| queries::complete_task(db, result, now))
    }
    fn open_hook_session(
        &self,
        input: app_core::domain::capture::HookSessionInput,
        now: i64,
    ) -> AppResult<()> {
        self.call(move |db| queries::open_hook_session(db, input, now))
    }
    fn ingest_hook_text(
        &self,
        input: app_core::domain::capture::HookTextInput,
        now: i64,
    ) -> AppResult<app_core::domain::capture::CaptureOutcome> {
        let session = input.session_id.clone();
        let result = self.call(move |db| queries::ingest_hook_text(db, input, now));
        if result.as_ref().is_err_and(|e| {
            matches!(
                e.code.as_str(),
                "event_identity_conflict"
                    | "source_identity_conflict"
                    | "storage_queue_full"
                    | "storage_busy"
                    | "storage_failure"
            )
        }) {
            self.note_capture_unconfirmed(&session, now);
        }
        result
    }
    fn flush_hook_capture(
        &self,
        request: app_core::domain::capture::CaptureFlush,
        now: i64,
    ) -> AppResult<Vec<app_core::domain::capture::CaptureVersion>> {
        self.call(move |db| queries::flush_hook_capture(db, request, now))
    }
    fn prepare_capture_tasks(
        &self,
        version: app_core::domain::capture::CaptureVersion,
        operation: String,
        now: i64,
    ) -> AppResult<Vec<app_core::domain::TaskDispatch>> {
        // One writer job: no command can change selection/session authority
        // between this check and task preparation. Explicit history requests
        // keep using the separate, unrestricted prepare_tasks port.
        self.call(move |db| {
            let authorized:bool=db.conn.query_row("SELECT EXISTS(SELECT 1 FROM reading_unit_revisions r JOIN reading_units u USING(unit_id) JOIN sessions s ON s.session_id=u.session_id WHERE r.unit_id=?1 AND r.text_revision=?2 AND u.state='finalized' AND r.text_revision=(SELECT max(text_revision) FROM reading_unit_revisions WHERE unit_id=u.unit_id) AND s.source_kind='hook' AND s.state IN ('opening','active') AND EXISTS(SELECT 1 FROM current_source_selection_members m WHERE m.session_id=s.session_id AND m.selection_revision=r.selection_revision))",rusqlite::params![version.unit_id,version.text_revision],|r|r.get(0)).map_err(err)?;
            if !authorized { return Ok(vec![]); }
            queries::prepare_tasks_for_operation(db,&version.unit_id,version.text_revision,"translation",Some(&operation),now)
        })
    }
    fn set_tokens(
        &self,
        input: app_core::domain::capture::TokenSnapshotInput,
        now: i64,
    ) -> AppResult<()> {
        self.call(move |db| queries::set_tokens(db, input, now))
    }
    fn set_tokens_for_retry(
        &self,
        input: app_core::domain::capture::TokenSnapshotInput,
        now: i64,
    ) -> AppResult<()> {
        self.call(move |db| queries::set_tokens_for_retry(db, input, now))
    }
    fn record_diagnostic(&self, d: Diagnostic, now: i64) -> AppResult<()> {
        if matches!(
            d.code.as_str(),
            "capture_connected" | "game_disconnected" | "capture_failed" | "integrity_lost"
        ) {
            self.flush_store_integrity(None)?;
        }
        self.call(move|db|{
            let state=match d.code.as_str(){"capture_connected"=>Some("active"),"game_disconnected"=>Some("stopped"),"capture_failed"|"integrity_lost"=>Some("failed"),_=>None};
            if let Some(state)=state {let tx=db.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;if state!="active" { queries::seal_capture_units(&tx,None,now,false)?; }let changed=tx.execute("UPDATE sessions SET state=?1,ended_at_ms=CASE WHEN ?1='active' THEN NULL ELSE ?2 END WHERE session_id=(SELECT session_id FROM sessions WHERE source_kind='hook' ORDER BY started_at_ms DESC,session_id DESC LIMIT 1) AND state IN ('opening','active')",params![state,now]).map_err(err)?;if changed>0{tx.execute("UPDATE profile_meta SET commit_revision=commit_revision+1,updated_at_ms=?1 WHERE singleton=1",[now]).map_err(err)?;}tx.commit().map_err(err)?;}
            db.diagnostics.push(d);if db.diagnostics.len()>100{db.diagnostics.remove(0);}Ok(())
        })
    }
    fn record_hook_diagnostic(
        &self,
        session: Option<String>,
        d: Diagnostic,
        now: i64,
    ) -> AppResult<()> {
        let state = match d.code.as_str() {
            "capture_connected" => Some("active"),
            "game_disconnected" => Some("stopped"),
            "capture_failed" | "integrity_lost" => Some("failed"),
            _ => None,
        };
        if state.is_some()
            && let Some(session) = session.as_deref()
        {
            self.flush_store_integrity(Some(session))?;
        }
        self.call(move |db| {
            if let (Some(state), Some(session)) = (state, session) {
                let tx = db.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
                if state != "active" {
                    queries::seal_capture_units(&tx, Some(&session), now, false)?;
                }
                let changed = tx.execute(
                    "UPDATE sessions SET state=?1,ended_at_ms=CASE WHEN ?1='active' THEN NULL ELSE ?2 END WHERE session_id=?3 AND source_kind='hook' AND state IN ('opening','active')",
                    params![state,now,session],
                ).map_err(err)?;
                if changed > 0 {
                    if state != "active" {
                        // The natural-exit ACK must cover the whole durable
                        // capture lifecycle for this immutable SID.
                        tx.execute(
                            "UPDATE capture_operations SET state=?1,ended_at_ms=?2 WHERE session_id=?3 AND state='active'",
                            params![state,now,session],
                        ).map_err(err)?;
                    }
                    tx.execute("UPDATE profile_meta SET commit_revision=commit_revision+1,updated_at_ms=?1 WHERE singleton=1", [now]).map_err(err)?;
                }
                tx.commit().map_err(err)?;
            }
            db.diagnostics.push(d);
            if db.diagnostics.len() > 100 { db.diagnostics.remove(0); }
            Ok(())
        })
    }

    fn begin_credential(&self, r: CredentialReference, now: i64) -> AppResult<()> {
        self.call(move |db| queries::begin_credential(db, r, now))
    }
    fn activate_credential(&self, id: String, now: i64) -> AppResult<()> {
        self.call(move |db| queries::activate_credential(db, &id, now))
    }
    fn revoke_credential(&self, id: String, now: i64) -> AppResult<()> {
        self.call(move |db| queries::revoke_credential(db, &id, now))
    }
    fn mark_credential_cleaned(&self, id: String, now: i64) -> AppResult<()> {
        self.call(move |db| queries::mark_credential_cleaned(db, &id, now))
    }
    fn pending_credentials(&self) -> AppResult<Vec<CredentialReference>> {
        self.call(queries::pending_credentials)
    }
    fn register_credential(&self, reference: CredentialReference, now: i64) -> AppResult<()> {
        self.call(move |db| queries::register_credential(db, reference, now))
    }
    fn credential_for_pool(&self, pool: String, revision: u32) -> AppResult<String> {
        self.call(move|db|db.conn.query_row("SELECT c.vault_ref FROM credential_pool_members m JOIN credential_refs c USING(credential_id) JOIN credential_pool_seals s USING(pool_id,pool_revision) WHERE m.pool_id=?1 AND m.pool_revision=?2 AND m.enabled=1 AND c.state='active' ORDER BY m.position LIMIT 1",params![pool,revision],|r|r.get(0)).map_err(err))
    }
    fn validate_task_authority(&self, task_id: String) -> AppResult<()> {
        self.call(move |db| {
            let handler = queries::google_handler_context(db);
            queries::validate_task_authority(&db.conn, &handler, &task_id)
        })
    }
    fn retry_attempt(
        &self,
        task_id: String,
        attempt_id: String,
        error_code: String,
        now_ms: i64,
    ) -> AppResult<String> {
        self.call(move |db| queries::retry_attempt(db, task_id, attempt_id, error_code, now_ms))
    }
    fn fail_task(&self, task_id: String, code: String, now: i64) -> AppResult<()> {
        self.call(move |db| queries::fail_task(db, task_id, code, now))
    }
}
/// Build-time dictionary index construction; runtime clients must open it read-only.
pub fn create_dictionary(path: &Path) -> AppResult<()> {
    if path.exists() {
        return Err(AppError::new(
            "dictionary_exists",
            "storage",
            "拒绝覆盖已有辞书",
        ));
    }
    let conn = Connection::open(path).map_err(err)?;
    conn.execute_batch(DICTIONARY_SQL).map_err(err)
}

#[cfg(all(test, unix))]
mod profile_lease_tests {
    use super::ProfileLease;
    use std::{
        io::{Read, Write},
        os::{fd::AsRawFd, raw::c_int, unix::net::UnixStream},
        time::Duration,
    };

    unsafe extern "C" {
        fn fork() -> c_int;
        fn read(fd: c_int, buffer: *mut u8, count: usize) -> isize;
        fn write(fd: c_int, buffer: *const u8, count: usize) -> isize;
        fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
        fn _exit(status: c_int) -> !;
    }

    #[test]
    fn dropping_an_inherited_guard_cannot_unlock_its_original_process() {
        let path =
            std::env::temp_dir().join(format!("luna-child-lease-{}.lock", uuid::Uuid::new_v4()));
        let open = || {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .unwrap()
        };
        let lease = ProfileLease::acquire(open()).unwrap();
        let contender = open();
        let (mut parent, child) = UnixStream::pair().unwrap();
        parent
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let child_fd = child.as_raw_fd();
        // SAFETY: the child only drops the guard (getpid/close), writes one byte,
        // and _exits; it never accesses SQLite or copied Rust synchronization.
        let pid = unsafe { fork() };
        assert!(pid >= 0, "{}", std::io::Error::last_os_error());
        if pid == 0 {
            drop(lease);
            let mut byte = 1_u8;
            // SAFETY: child_fd and these one-byte buffers are valid until _exit.
            unsafe {
                if write(child_fd, &byte, 1) != 1 {
                    _exit(11);
                }
                if read(child_fd, &mut byte, 1) != 1 {
                    _exit(12);
                }
                _exit(0);
            }
        }
        drop(child);
        let mut ready = [0];
        let ready_result = parent.read_exact(&mut ready);
        let lock_result = contender.try_lock();
        // Reap the child before asserting, including the deliberately broken
        // guard case, so a failing regression cannot strand a gated process.
        let release_result = parent.write_all(&[1]);
        let mut status = 0;
        let waited = loop {
            // SAFETY: pid is our unreaped child and status is a valid pointer.
            let waited = unsafe { waitpid(pid, &mut status, 0) };
            if waited == pid
                || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
            {
                break waited;
            }
        };
        ready_result.unwrap();
        assert_eq!(ready, [1]);
        release_result.unwrap();
        assert_eq!(waited, pid);
        assert_eq!(status, 0);
        assert!(matches!(
            lock_result,
            Err(std::fs::TryLockError::WouldBlock)
        ));
        drop(lease);
        contender.try_lock().unwrap();
        contender.unlock().unwrap();
        drop(contender);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
#[path = "closure_integrity_tests.rs"]
mod closure_integrity_tests;
#[cfg(test)]
#[path = "database_fault_tests.rs"]
mod fault_tests;

// Test-only single-thread fault scope; no release/env selector or production policy exists.
#[cfg(test)]
mod http_migration_fault {
    use super::*;
    use std::cell::Cell;
    thread_local! { static FAIL_AFTER_DDL:Cell<bool>=const {Cell::new(false)}; }
    struct FaultScope;
    impl FaultScope {
        fn once() -> Self {
            FAIL_AFTER_DDL.with(|v| {
                assert!(!v.replace(true));
            });
            Self
        }
    }
    impl Drop for FaultScope {
        fn drop(&mut self) {
            FAIL_AFTER_DDL.with(|v| v.set(false));
        }
    }
    pub(super) fn inject_sqlite_failure_once(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
        if FAIL_AFTER_DDL.with(|v| v.replace(false)) {
            // Real CHECK failure after the actual production rebuild, before its commit.
            tx.execute("INSERT INTO config_revisions(revision_id,content_hash,created_at_ms,save_reason,codec_version) VALUES('controlled-fault',?1,0,'migrate',3)",["a".repeat(64)]).map_err(err)?;
        }
        Ok(())
    }
    #[test]
    fn actual_migration_host_sql_failure_after_ddl_rolls_back_and_then_recovers() {
        let directory = std::env::var_os("FP_STORAGE_OUTPUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("http-host-fault-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("failure.sqlite3");
        let old = Connection::open(&path).unwrap();
        old.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        old.execute_batch(include_str!(
            "../../tests/fixtures/http-schema2-original.sql"
        ))
        .unwrap();
        let original = queries::configuration_codec::legacy_rows(&old).unwrap();
        let original_schema = schema_identity(&old).unwrap();
        drop(old);
        let guard = FaultScope::once();
        let failure = match Database::open(&path, "legacy-http-profile".into(), "fault".into(), 100)
        {
            Ok(_) => panic!("injected SQLite CHECK must abort this migration"),
            Err(e) => e,
        };
        drop(guard);
        assert!(failure.message.contains("CHECK constraint failed"));
        let old = Connection::open(&path).unwrap();
        assert_eq!(schema_identity(&old).unwrap(), original_schema);
        assert_eq!(
            old.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            2
        );
        queries::configuration_codec::verify_legacy_rows(&old, &original).unwrap();
        assert_eq!(
            old.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, u32>(
                0
            ))
            .unwrap(),
            0
        );
        drop(old);
        let backup = directory.join("failure.sqlite3.pre-v5-100.sqlite3");
        assert!(backup.is_file());
        let backup_db = Connection::open(&backup).unwrap();
        assert_eq!(schema_identity(&backup_db).unwrap(), original_schema);
        queries::configuration_codec::verify_legacy_rows(&backup_db, &original).unwrap();
        drop(backup_db);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_ne!(
                std::fs::metadata(&path).unwrap().ino(),
                std::fs::metadata(&backup).unwrap().ino()
            );
        }
        let recovered =
            Database::open(&path, "legacy-http-profile".into(), "recovered".into(), 200).unwrap();
        assert_eq!(
            recovered
                .conn
                .query_row("SELECT version FROM schema_migrations WHERE version=3", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            recovered
                .conn
                .query_row("PRAGMA foreign_keys", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(recovered.conn.query_row("SELECT max(version) FROM schema_migrations", [], |r| r.get::<_,u32>(0)).unwrap(), 5);
        assert_eq!(recovered.conn.query_row("SELECT migration_sha256 FROM schema_migrations WHERE version=3", [], |r| r.get::<_,String>(0)).unwrap(), http_service_identity_migration_sha256());
        assert_eq!(recovered.conn.query_row("SELECT migration_sha256 FROM schema_migrations WHERE version=4", [], |r| r.get::<_,String>(0)).unwrap(), service_graph_migration_sha256());
        let after = directory.join("recovered.sqlite3");
        recovered.conn.backup("main", &after, None).unwrap();
        if let Some(output) = std::env::var_os("FP_STORAGE_OUTPUT") {
            use std::io::Write;
            let evidence = std::path::PathBuf::from(output).join("components/v3-delta");
            std::fs::create_dir_all(&evidence).unwrap();
            let artifact = |p: &std::path::Path| serde_json::json!({"path":p.canonicalize().unwrap(),"sha256":content_hash(std::fs::read(p).unwrap())});
            let receipt = serde_json::json!({"kind":"http_storage_v3_supporting_case","case":"host-rollback-failure","run_id":std::env::var("FP_STORAGE_RUN_ID").unwrap(),"source_tree_sha256":std::env::var("FP_STORAGE_SOURCE_SHA256").unwrap(),"execution_nonce":std::env::var("FP_STORAGE_EXECUTION_NONCE").unwrap(),"completed_at_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(),"assertions":10,"detail":{"database":artifact(&after),"backup":artifact(&backup),"scope":"same production migration host, cfg(test) one-shot real SQLite CHECK failure after DDL; distinct from physical ENOSPC","actual_error":failure.code}});
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(evidence.join("host-rollback-failure.json"))
                .unwrap();
            file.write_all(&serde_json::to_vec_pretty(&receipt).unwrap())
                .unwrap();
            file.sync_all().unwrap();
        }
    }
}

// Test-only actual host failure after the real004 DDL. Never compiled into release or selected by environment.
#[cfg(test)]
mod service_graph_migration_fault {
    use super::*;
    use std::cell::Cell;
    thread_local! {static FAIL:Cell<bool>=const {Cell::new(false)};}
    struct Scope;
    impl Scope {
        fn once() -> Self {
            FAIL.with(|flag| assert!(!flag.replace(true)));
            Self
        }
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            FAIL.with(|flag| flag.set(false));
        }
    }
    pub(super) fn inject_sqlite_failure_once(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
        if FAIL.with(|flag| flag.replace(false)) {
            tx.execute("INSERT INTO config_revisions(revision_id,content_hash,created_at_ms,save_reason,codec_version) VALUES('controlled-graph-fault',?1,0,'migrate',4)",["a".repeat(64)]).map_err(err)?;
        }
        Ok(())
    }
    #[test]
    fn actual_service_graph_host_failure_rolls_back_ddl_rows_seals_heads_and_then_recovers() {
        let dir =
            std::env::temp_dir().join(format!("service-graph-host-fault-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("legacy-three.sqlite3");
        let old = Connection::open(&path).unwrap();
        old.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        old.execute_batch(include_str!(
            "../../tests/fixtures/http-schema2-original.sql"
        ))
        .unwrap();
        old.execute_batch(HTTP_IDENTITY_SQL).unwrap();
        old.execute("INSERT INTO schema_migrations VALUES(3,?1,30,'synthetic-original-writer2-plus-frozen003',2)",[http_service_identity_migration_sha256()]).unwrap();
        let rows = queries::configuration_codec::legacy_rows(&old).unwrap();
        let shape = schema_identity(&old).unwrap();
        drop(old);
        let guard = Scope::once();
        let result = Database::open(&path, "legacy-http-profile".into(), "fault".into(), 100);
        drop(guard);
        assert!(matches!(result,Err(e) if e.message.contains("CHECK constraint failed")));
        let read = Connection::open(&path).unwrap();
        assert_eq!(schema_identity(&read).unwrap(), shape);
        queries::configuration_codec::verify_legacy_rows(&read, &rows).unwrap();
        assert_eq!(
            read.query_row("SELECT max(version) FROM schema_migrations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            read.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, u32>(
                0
            ))
            .unwrap(),
            0
        );
        drop(read);
        let backup = path.with_file_name("legacy-three.sqlite3.pre-v5-100.sqlite3");
        assert!(backup.is_file());
        let backup = Connection::open(backup).unwrap();
        assert_eq!(schema_identity(&backup).unwrap(), shape);
        queries::configuration_codec::verify_legacy_rows(&backup, &rows).unwrap();
        drop(backup);
        let recovered =
            Database::open(&path, "legacy-http-profile".into(), "recovered".into(), 101).unwrap();
        assert_eq!(
            recovered
                .conn
                .query_row("SELECT max(version) FROM schema_migrations", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            5
        );
    }
}

// Scoped, path-bound real SQLite failures. This module and every hook are
// absent from release builds; no environment variable selects a product fault.
#[cfg(test)]
pub(crate) mod source_selection_migration_fault {
    use super::*;
    use serde_json::{Value,json};
    use std::{os::unix::fs::MetadataExt,sync::{Mutex,OnceLock,atomic::{AtomicBool,Ordering}}};
    const TEST_ROLLBACK:&str="storage::database::source_selection_migration_fault::actual_schema5_host_rolls_back_after_ddl_and_recovers_same_original_profile";
    const TEST_OS:&str="storage::database::source_selection_migration_fault::actual_schema5_host_readonly_and_busy_leave_the_original_unchanged";
    const TEST_FULL:&str="storage::database::source_selection_migration_fault::actual_schema5_host_full_isolated_volume_preserves_the_old_database";
    const BOUNDARIES:[&str;19]=["api_entry","api_error_return_ready","backup_ready","before_write_barrier","caller_received_error","cleanup_complete","commit_succeeded","database_writer_entry","ddl_applied","fault_applied","native_failure","post_error_invariants","recovery_complete","scope_created","transaction_attempted","transaction_scope_exited_after_drop","transaction_started","writer_error_published","writer_resumed"];
    pub(crate) fn wall_ms()->u64 {u64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()).unwrap()}
    fn source_root()->std::path::PathBuf {Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()}
    pub(crate) fn artifact(path:&Path)->Value {let path=path.canonicalize().unwrap();json!({"path":path.strip_prefix(source_root()).unwrap(),"sha256":content_hash(std::fs::read(&path).unwrap())})}
    pub(crate) fn identity(path:&Path)->Value {let metadata=std::fs::metadata(path).unwrap();json!({"device":metadata.dev(),"inode":metadata.ino()})}
    pub(crate) fn write_json(path:&Path,value:&Value) {use std::io::Write;let mut f=std::fs::OpenOptions::new().create_new(true).write(true).open(path).unwrap();f.write_all(&serde_json::to_vec_pretty(value).unwrap()).unwrap();f.sync_all().unwrap();}
    pub(crate) fn monotonic_ns()->u64 {
        let mut now=std::mem::MaybeUninit::<libc::timespec>::uninit();
        // SAFETY: now has the space/alignment for timespec, initialized by clock_gettime on success.
        assert_eq!(unsafe{libc::clock_gettime(libc::CLOCK_MONOTONIC,now.as_mut_ptr())},0);
        // SAFETY: successful clock_gettime immediately above initialized now.
        let now=unsafe{now.assume_init()};u64::try_from(now.tv_sec).unwrap()*1_000_000_000+u64::try_from(now.tv_nsec).unwrap()
    }
    fn ppid()->u32 {
        // SAFETY: getppid has no arguments, does not access caller memory, and cannot fail.
        u32::try_from(unsafe{libc::getppid()}).unwrap()
    }
    pub(crate) fn fixture_environment()->Value {json!({"LUNA_BINDING_NONCE":std::env::var("LUNA_BINDING_NONCE").unwrap_or_default(),"LUNA_BINDING_SOURCE_TREE":std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap_or_default(),"LUNA_BINDING_OUTPUT":std::env::var("LUNA_BINDING_OUTPUT").unwrap_or_default()})}
    fn emit_identity() {let manifest=std::path::PathBuf::from(std::env::var_os("FP_STORAGE_OLD_STORE4_READER_MANIFEST").expect("actual frozen Store4 reader required; missing is blocked"));println!("LUNA_STORAGE_V5_CHILD_IDENTITY {}",json!({"pid":std::process::id(),"ppid":ppid(),"binary":artifact(&std::env::current_exe().unwrap()),"nonce":std::env::var("LUNA_BINDING_NONCE").unwrap_or_default(),"source_tree_sha256":std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap_or_default(),"output":std::env::var("LUNA_BINDING_OUTPUT").unwrap_or_default(),"old_writer_manifest":artifact(&manifest)}));}
    fn symbol(kind:&str)->Option<&'static str> {Some(match kind {
        "api_entry"=>"SqliteStore::open", "api_error_return_ready"=>"SqliteStore::open Err branch after writer.join",
        "backup_ready"=>"Database::open migration backup block", "before_write_barrier"=>"Database::open -> source_selection_migration_fault::before_write",
        "caller_received_error"=>"actual fixed private fault test calls SqliteStore::open or joins wrapper", "cleanup_complete"=>"private test permissions/holder/filler/Scope/owned mount cleanup",
        "commit_succeeded"=>"Database::open tx.commit", "database_writer_entry"=>"SqliteStore::open spawned luna-sqlite-writer -> Database::open",
        "ddl_applied"=>"Database::open execute_batch SOURCE_SELECTION_SQL -> after_ddl", "fault_applied"=>"private test exact fault source",
        "native_failure"=>"migration_sql_error -> observe_error; after_ddl -> observe_error", "post_error_invariants"=>"private fault exact tests independent byte/SQL/backup verification",
        "recovery_complete"=>"rollback fixed test reopens same original after removing Scope", "scope_created"=>"source_selection_migration_fault::Scope::new",
        "transaction_attempted"=>"Database::open before transaction_with_behavior", "transaction_scope_exited_after_drop"=>"Database::open error-unwind after real Transaction drop",
        "transaction_started"=>"Database::open -> source_selection_migration_fault::transaction_started", "writer_error_published"=>"SqliteStore::open worker Err ready_tx",
        "writer_resumed"=>"source_selection_migration_fault::before_write receiver resumes", _=>return None })}
    #[derive(Clone,Copy)]
    pub(crate) enum Mode { AfterDdl, PauseBeforeWrite, Observe, PauseAfterDdl }
    pub(crate) struct Control {
        mode:Mode, armed:AtomicBool,reached:mpsc::SyncSender<()>,resume:Mutex<mpsc::Receiver<()>>,
        pub(crate) sqlite_code:Mutex<Option<i32>>,events:Mutex<Vec<Value>>,binding:Value,
        fresh_journal:Option<Mutex<std::fs::File>>,first_identity:Mutex<Option<Value>>,identity_invalid:AtomicBool,sealed:AtomicBool,
    }
    impl Control {
        fn verify_fresh_original_identity(&self)->Option<Value> {
            assert!(!self.identity_invalid.load(Ordering::SeqCst),"fresh original identity was previously invalidated; cannot seal a later replacement");
            let first=self.first_identity.lock().unwrap().clone()?;let path=Path::new(self.binding["original_database_path"].as_str().unwrap());
            let metadata=std::fs::symlink_metadata(path);let observed_wall_ms=wall_ms();let observed_monotonic_ns=monotonic_ns();
            let actual=metadata.as_ref().ok().filter(|m|m.is_file()&&!m.file_type().is_symlink()).map(|m|json!({"device":m.dev(),"inode":m.ino()}));
            let valid=actual.as_ref()==Some(&first["file_identity"]);if !valid{self.identity_invalid.store(true,Ordering::SeqCst);}
            assert!(valid,"fresh original missing/replaced/renamed/symlinked since the post-open identity observation");
            Some(json!({"operation":"symlink_metadata","normalized_path":path,"file_missing":false,"file_identity":actual.unwrap(),"observed_wall_ms":observed_wall_ms,"monotonic_ns":observed_monotonic_ns}))
        }
        pub(crate) fn emit(&self,kind:&str,phase:&str,mut payload:Value) {
            if let Some(symbol)=symbol(kind){payload["production_symbol"]=json!(symbol);}
            if self.fresh_journal.is_some() {
                assert!(!self.sealed.load(Ordering::SeqCst),"a sealed raw journal cannot receive more callbacks");
                payload["identity_binding_semantics"]=json!("invocation_artifact_binding_added_only_at_seal_not_callback_time_stat");
                if let Some(observation)=self.verify_fresh_original_identity(){payload["actual_existing_file_observation"]=observation;}
                if matches!(kind,"api_entry"|"database_writer_entry") {
                    let path=Path::new(self.binding["original_database_path"].as_str().unwrap());
                    // This is a separate actual callback-time filesystem query.
                    // It is not inferred from route, Scope, or the final DB.
                    let observation=missing_observation(path);
                    assert_eq!(observation["file_missing"],true,"fresh API/writer entry must actually observe no original database");
                    payload["filesystem_observation"]=observation;
                }
            }
            let mut events=self.events.lock().unwrap();let mut event=self.binding.clone();
            event["event_kind"]=json!(kind);event["event_sequence"]=json!(events.len()+if self.binding["route"]=="kill"{2}else{1});
            event["monotonic_ns"]=json!(monotonic_ns());event["observed_wall_ms"]=json!(wall_ms());
            event["actual_thread_id"]=json!(format!("{}:{:?}",std::process::id(),std::thread::current().id()));event["actual_phase"]=json!(phase);event["payload"]=payload;
            if let Some(journal)=&self.fresh_journal {
                use std::io::Write;
                let raw=json!({"kind":"actual-schema5-original-boundary-callback/v1","callback":event});
                let mut journal=journal.lock().unwrap();writeln!(&mut *journal,"{}",raw).unwrap();journal.sync_all().unwrap();
                println!("LUNA_STORAGE_V5_RAW_CALLBACK {}",raw);
            } else {println!("LUNA_STORAGE_V5_BOUNDARY {}",event);}
            events.push(event);
        }
        pub(crate) fn binding(&self)->Value {
            let mut binding=self.binding.clone();
            if self.fresh_journal.is_some() {assert!(self.sealed.load(Ordering::SeqCst),"fresh artifact binding is unavailable before a successful seal");binding["database_file_device_identity"]=self.first_identity.lock().unwrap().as_ref().expect("no real file identity; invocation remains blocked")["file_identity"].clone();}
            binding
        }
        pub(crate) fn timeline(&self)->Value {
            let mut events=self.events.lock().unwrap().clone();
            if self.fresh_journal.is_some() {let binding=self.binding();for event in &mut events {event["database_file_device_identity"]=binding["database_file_device_identity"].clone();}}
            let phase=events.iter().find(|e|e["event_kind"]=="native_failure").and_then(|e|e["actual_phase"].as_str());
            let mut absent=serde_json::Map::new();
            if let Some(phase)=phase {for kind in BOUNDARIES {if !events.iter().any(|e|e["event_kind"]==kind) {
                let not_applicable=matches!(kind,"commit_succeeded")||self.binding["route"]=="rollback"&&matches!(kind,"before_write_barrier"|"writer_resumed")||self.binding["route"]!="rollback"&&kind=="recovery_complete";
                absent.insert(kind.into(),json!({"event":null,"reason":if not_applicable{"route_not_applicable"}else if kind=="transaction_scope_exited_after_drop"{"no_transaction_started"}else{"not_reached"},"failure_phase":phase}));
            }}}
            if self.fresh_journal.is_some() {absent.insert("backup_ready".into(),json!({"event":null,"reason":"no_old_database","failure_phase":null}));}
            json!({"kind":"private-schema5-production-timeline/v1","events":events,"not_reached":absent})
        }
    }
    fn controls()->&'static Mutex<std::collections::BTreeMap<std::path::PathBuf,Arc<Control>>>{static C:OnceLock<Mutex<std::collections::BTreeMap<std::path::PathBuf,Arc<Control>>>>=OnceLock::new();C.get_or_init(||Mutex::new(std::collections::BTreeMap::new()))}
    fn control(path:&Path)->Option<Arc<Control>> {controls().lock().unwrap().get(path).cloned()}
    pub(crate) struct Scope {path:std::path::PathBuf,pub(crate) control:Arc<Control>,reached:mpsc::Receiver<()>,resume:mpsc::SyncSender<()>}
    impl Scope {
        pub(super) fn new_fresh(path:&Path,exact_test:&str,profile:&str,server:&str)->Self {
            let parent=path.parent().unwrap().canonicalize().unwrap();let path=parent.join(path.file_name().unwrap());
            assert_eq!(missing_observation(&path)["file_missing"],true,"precreated empty or foreign database cannot be presented as fresh");
            let raw_path=parent.join("fresh-original-callbacks.jsonl");let raw=std::fs::OpenOptions::new().create_new(true).write(true).open(raw_path).unwrap();
            let (reached_tx,reached)=mpsc::sync_channel(1);let (resume,resume_rx)=mpsc::sync_channel(1);
            let control=Arc::new(Control{mode:Mode::Observe,armed:AtomicBool::new(false),reached:reached_tx,resume:Mutex::new(resume_rx),sqlite_code:Mutex::new(None),events:Mutex::new(vec![]),fresh_journal:Some(Mutex::new(raw)),first_identity:Mutex::new(None),identity_invalid:AtomicBool::new(false),sealed:AtomicBool::new(false),binding:json!({"route":"fresh","case_id":"BJ-11","invocation_id":uuid::Uuid::new_v4().to_string(),"exact_test":exact_test,"source_tree_sha256":std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap_or_default(),"nonce":std::env::var("LUNA_BINDING_NONCE").unwrap_or_default(),"child_pid":std::process::id(),"child_ppid":ppid(),"original_database_path":path,"profile_id":profile,"server_id":server,"scope_id":uuid::Uuid::new_v4().to_string()})});
            assert!(controls().lock().unwrap().insert(path.clone(),control.clone()).is_none());control.emit("scope_created","test_control",json!({}));Self{path,control,reached,resume}
        }
        pub(crate) fn seal_fresh(&self,snapshot_capture:Value)->Value {
            let c=&self.control;assert!(c.fresh_journal.is_some());assert!(!c.sealed.load(Ordering::SeqCst));
            let first=c.first_identity.lock().unwrap().clone().expect("creating failed before a real DB existed; no positive artifact identity is available");
            let final_observation=c.verify_fresh_original_identity().expect("no successful Connection::open identity observation; invocation remains incomplete");let final_identity=final_observation["file_identity"].clone();assert_eq!(first["file_identity"],final_identity);

            assert_eq!(snapshot_capture["source_path"],json!(self.path));assert_eq!(snapshot_capture["source_identity_before"],final_identity);assert_eq!(snapshot_capture["source_identity_after"],final_identity);
            let snapshot_base=Path::new(snapshot_capture["snapshot_base"].as_str().unwrap()).canonicalize().unwrap();let snapshot=snapshot_base.join(snapshot_capture["snapshot"]["path"].as_str().unwrap());let snapshot=snapshot.canonicalize().unwrap();assert!(snapshot.starts_with(source_root()));assert_ne!(snapshot,self.path);assert_ne!(identity(&snapshot),final_identity,"snapshot identity cannot stand in for the original database");assert_eq!(content_hash(std::fs::read(&snapshot).unwrap()),snapshot_capture["snapshot"]["sha256"]);assert_eq!(database_image(&self.path),database_image(&snapshot),"fresh snapshot must preserve every original schema object and row");

            let sealed_at=json!({"observed_wall_ms":wall_ms(),"monotonic_ns":monotonic_ns(),"pid":std::process::id(),"actual_thread_id":format!("{}:{:?}",std::process::id(),std::thread::current().id()),"final_original_file_identity":final_identity});
            c.fresh_journal.as_ref().unwrap().lock().unwrap().sync_all().unwrap();c.sealed.store(true,Ordering::SeqCst);
            let timeline=c.timeline();let originals=c.events.lock().unwrap().clone();let mut mapping=vec![];
            for (original,sealed) in originals.iter().zip(timeline["events"].as_array().unwrap()) {let mut unchanged=sealed.clone();assert!(original.get("database_file_device_identity").is_none());unchanged.as_object_mut().unwrap().remove("database_file_device_identity");assert_eq!(&unchanged,original,"sealing may add only common artifact identity; original callback facts must be unchanged");mapping.push(json!({"event_sequence":original["event_sequence"],"original_callback_sha256":content_hash(serde_json::to_vec(original).unwrap()),"sealed_callback_sha256":content_hash(serde_json::to_vec(sealed).unwrap())}));println!("LUNA_STORAGE_V5_BOUNDARY {}",sealed);}
            let parent=self.path.parent().unwrap();let timeline_path=parent.join("fresh-sealed-timeline.json");write_json(&timeline_path,&timeline);
            let proof=json!({"kind":"actual-fresh-invocation-artifact-sealing-proof/v1","semantics":"invocation_artifact_binding_not_callback_time_stat_not_kernel_creation_time","binding":c.binding(),"original_callbacks":artifact(&parent.join("fresh-original-callbacks.jsonl")),"first_observed_file_identity":artifact(&parent.join("fresh-first-observed-identity.json")),"sealed_timeline":artifact(&timeline_path),"mapping":mapping,"snapshot_capture":snapshot_capture,"seal_observation":sealed_at});
            let proof_path=parent.join("fresh-raw-closure.json");write_json(&proof_path,&proof);println!("LUNA_STORAGE_V5_FRESH_RAW_CLOSURE {}",proof);proof
        }
        pub(crate) fn new(path:&Path,mode:Mode,route:&str,exact_test:&str,profile:&str,server:&str)->Self {
            let path=path.canonicalize().unwrap();let (reached_tx,reached)=mpsc::sync_channel(1);let (resume,resume_rx)=mpsc::sync_channel(1);
            let control=Arc::new(Control{mode,armed:AtomicBool::new(true),reached:reached_tx,resume:Mutex::new(resume_rx),sqlite_code:Mutex::new(None),events:Mutex::new(vec![]),fresh_journal:None,first_identity:Mutex::new(None),identity_invalid:AtomicBool::new(false),sealed:AtomicBool::new(false),binding:json!({"route":route,"case_id":"BJ-11","invocation_id":uuid::Uuid::new_v4().to_string(),"exact_test":exact_test,"source_tree_sha256":std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap_or_default(),"nonce":std::env::var("LUNA_BINDING_NONCE").unwrap_or_default(),"child_pid":std::process::id(),"child_ppid":ppid(),"original_database_path":path,"database_file_device_identity":identity(&path),"profile_id":profile,"server_id":server,"scope_id":uuid::Uuid::new_v4().to_string()})});
            assert!(controls().lock().unwrap().insert(path.clone(),control.clone()).is_none());control.emit("scope_created","test_control",json!({}));Self{path,control,reached,resume}
        }
    }
    impl Drop for Scope {fn drop(&mut self){controls().lock().unwrap().remove(&self.path);}}
    fn missing_observation(path:&Path)->Value {
        let result=std::fs::symlink_metadata(path);let wall=wall_ms();let mono=monotonic_ns();
        match result {Ok(metadata)=>json!({"operation":"symlink_metadata","normalized_path":path,"file_missing":false,"file_is_symlink":metadata.file_type().is_symlink(),"file_identity":{"device":metadata.dev(),"inode":metadata.ino()},"observed_wall_ms":wall,"monotonic_ns":mono}),Err(error) if error.kind()==std::io::ErrorKind::NotFound=>json!({"operation":"symlink_metadata","normalized_path":path,"file_missing":true,"os_error":error.raw_os_error(),"observed_wall_ms":wall,"monotonic_ns":mono}),Err(error)=>panic!("actual fresh filesystem observation failed: {error}")}
    }
    pub(super) fn connection_opened(path:&Path) {
        if let Some(c)=control(path)&&c.fresh_journal.is_some() {
            let metadata=std::fs::symlink_metadata(path).unwrap();assert!(metadata.is_file()&&!metadata.file_type().is_symlink());
            let observation=json!({"kind":"actual-first-file-identity-after-production-connection-open/v1","meaning":"first_observed_existing_file_after_Connection_open_not_kernel_creation_timestamp","invocation_id":c.binding["invocation_id"],"nonce":c.binding["nonce"],"source_tree_sha256":c.binding["source_tree_sha256"],"original_database_path":path,"pid":std::process::id(),"ppid":ppid(),"actual_thread_id":format!("{}:{:?}",std::process::id(),std::thread::current().id()),"observed_wall_ms":wall_ms(),"monotonic_ns":monotonic_ns(),"after_callback_sequence":c.events.lock().unwrap().len(),"file_identity":{"device":metadata.dev(),"inode":metadata.ino()},"production_symbol":"Database::open immediately after successful Connection::open"});
            let mut first=c.first_identity.lock().unwrap();assert!(first.is_none(),"first existence observation cannot be overwritten");*first=Some(observation.clone());write_json(&path.parent().unwrap().join("fresh-first-observed-identity.json"),&observation);println!("LUNA_STORAGE_V5_FILE_IDENTITY_OBSERVATION {}",observation);
        }
    }
    pub(super) fn boundary(path:&Path,kind:&str,payload:Value) {if let Some(c)=control(path){c.emit(kind,"production",payload);}}
    pub(super) fn api_entry(path:&Path,profile:&str,server:&str,now:i64) {if let Some(c)=control(path){c.emit("api_entry","arguments",json!({"arguments":{"original_database_path":path,"profile_id":profile,"server_id":server,"now_ms":now}}));}}
    pub(super) fn observe_error(path:&Path,phase:&str,error:&rusqlite::Error) {if let rusqlite::Error::SqliteFailure(code,_)=error {if let Some(c)=control(path){*c.sqlite_code.lock().unwrap()=Some(code.extended_code&255);c.emit("native_failure",phase,json!({"failure_phase":phase,"native_primary_code":code.extended_code&255,"native_extended_code":code.extended_code,"error_code":err(error).code}));}}}
    pub(super) fn transaction_started(path:&Path){boundary(path,"transaction_started",json!({"sql":"BEGIN IMMEDIATE","success":true}));}
    pub(super) struct TransactionExit {path:std::path::PathBuf}
    impl TransactionExit {pub(super) fn new(path:&Path)->Self{Self{path:path.into()}}}
    impl Drop for TransactionExit {fn drop(&mut self){if let Some(c)=control(&self.path){let events=c.events.lock().unwrap();let started=events.iter().any(|e|e["event_kind"]=="transaction_started");let committed=events.iter().any(|e|e["event_kind"]=="commit_succeeded");drop(events);if started&&!committed{c.emit("transaction_scope_exited_after_drop","error_unwind",json!({"observation":"observer declared before real Transaction; reverse lexical drop order"}));}}}}
    pub(super) fn before_write(path:&Path)->AppResult<()> {if let Some(c)=control(path)&&matches!(c.mode,Mode::PauseBeforeWrite)&&c.armed.swap(false,Ordering::SeqCst){c.emit("before_write_barrier","before_connection_open",json!({}));c.reached.send(()).map_err(err)?;c.resume.lock().unwrap().recv_timeout(Duration::from_secs(30)).map_err(err)?;c.emit("writer_resumed","before_connection_open",json!({}));}Ok(())}
    pub(super) fn after_ddl(path:&Path,tx:&rusqlite::Transaction<'_>)->AppResult<()> {
        boundary(path,"ddl_applied",json!({"migration_sha256":source_selection_migration_sha256(),"migration":"crates/app-infra/src/storage/migrations/005_source_selection_sets.sql"}));
        if let Some(c)=control(path)&&matches!(c.mode,Mode::AfterDdl)&&c.armed.swap(false,Ordering::SeqCst){
            let sql="INSERT INTO source_selection_sets VALUES('controlled-fault',1,NULL,'legacy_singleton','temporary','{}',0,1,'sealed')";
            c.emit("fault_applied","post_ddl",json!({"sql":sql,"fault":"actual invalid INSERT immediately attempted"}));
            let error=tx.execute(sql,[]).unwrap_err();observe_error(path,"post_ddl",&error);return Err(err(error));
        }
        if let Some(c)=control(path)&&matches!(c.mode,Mode::PauseAfterDdl) {c.reached.send(()).map_err(err)?;c.resume.lock().unwrap().recv_timeout(Duration::from_secs(60)).map_err(err)?;}
        Ok(())
    }
    fn original_four(path:&Path) {
        let manifest=std::path::PathBuf::from(std::env::var_os("FP_STORAGE_OLD_STORE4_READER_MANIFEST").expect("actual frozen Store4 reader required; missing is blocked"));
        let value:Value=serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();let binary=value["binary_path"].as_str().unwrap();
        assert_eq!(content_hash(std::fs::read(binary).unwrap()),value["binary_sha256"]);
        let run=std::process::Command::new(binary).arg(path).arg("migration-fault-profile").arg("accept").output().unwrap();assert!(run.status.success(),"{}",String::from_utf8_lossy(&run.stderr));
        let db=Connection::open(path).unwrap();assert_eq!(db.query_row("SELECT max(version) FROM schema_migrations",[],|r|r.get::<_,u32>(0)).unwrap(),4);
        db.execute("INSERT INTO games(game_id,display_name,row_revision,created_at_ms,updated_at_ms) VALUES('nonempty-old-game','既存の原件',1,1,1)",[]).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    }
    fn directory(case:&str)->std::path::PathBuf {let base=std::env::var_os("LUNA_BINDING_OUTPUT").map(std::path::PathBuf::from).unwrap_or_else(||source_root().join(".build/source-selection-test-artifacts"));let dir=base.join(format!("schema5-host-{case}-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&dir).unwrap();dir.canonicalize().unwrap()}
    fn capture(source:&Path,dest:&Path)->Value {let source_identity=identity(source);std::fs::copy(source,dest).unwrap();json!({"source_path":source,"source_identity":source_identity,"snapshot":artifact(dest),"captured_at_ms":wall_ms()})}
    fn immutable(path:&Path)->Connection {Connection::open_with_flags(format!("file:{}?mode=ro&immutable=1",path.to_str().unwrap()),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY|rusqlite::OpenFlags::SQLITE_OPEN_URI).unwrap()}
    fn database_image(path:&Path)->Value {
        let db=immutable(path);assert_eq!(db.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");assert!(db.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());
        let tables=db.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap().query_map([],|r|r.get::<_,String>(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();let mut image=serde_json::Map::new();
        for table in tables {let mut q=db.prepare(&format!("SELECT * FROM \"{}\"",table.replace('"',"\"\""))).unwrap();let n=q.column_count();let mut rows=q.query_map([],|r|Ok((0..n).map(|i|match r.get_ref(i).unwrap(){rusqlite::types::ValueRef::Null=>Value::Null,rusqlite::types::ValueRef::Integer(v)=>json!(v),rusqlite::types::ValueRef::Real(v)=>json!(v),rusqlite::types::ValueRef::Text(v)=>json!({"text":std::str::from_utf8(v).unwrap()}),rusqlite::types::ValueRef::Blob(v)=>json!({"blob":v})}).collect::<Vec<_>>())).unwrap().collect::<Result<Vec<_>,_>>().unwrap();rows.sort_by_key(|r|serde_json::to_string(r).unwrap());image.insert(table,json!(rows));}
        json!({"schema":schema_identity(&db).unwrap(),"tables":image})
    }
    fn check_old(path:&Path,before:&Value,after:&Value,backup:&Value) {
        assert_eq!(before["snapshot"]["sha256"],after["snapshot"]["sha256"]);let resolve=|capture:&Value|source_root().join(capture["snapshot"]["path"].as_str().unwrap());
        let original=database_image(&resolve(before));assert_eq!(database_image(path),original);assert_eq!(database_image(&resolve(after)),original);assert_eq!(database_image(&resolve(backup)),original);
        let db=immutable(path);assert_eq!(db.query_row("SELECT max(version) FROM schema_migrations",[],|r|r.get::<_,u32>(0)).unwrap(),4);assert_eq!(db.query_row("SELECT count(*) FROM sqlite_schema WHERE name='source_selection_sets'",[],|r|r.get::<_,u32>(0)).unwrap(),0);assert_eq!(db.query_row("SELECT display_name FROM games WHERE game_id='nonempty-old-game'",[],|r|r.get::<_,String>(0)).unwrap(),"既存の原件");
    }
    fn complete_fault(dir:&Path,control:&Control,captures:Value,recovery:Value,os_facts:Value) {
        let timeline_path=dir.join("production-timeline.json");write_json(&timeline_path,&control.timeline());let mut record=control.binding();record["kind"]=json!("actual-schema5-migration-host-fault/v2");record["now_ms"]=json!(100);record["writer"]=json!("SqliteStore::open");record["writer_process_binary"]=artifact(&std::env::current_exe().unwrap());
        for key in ["before","after","backup"]{record[key]=captures[key]["snapshot"].clone();}record["captures"]=captures;record["recovery"]=recovery;record["timeline"]=artifact(&timeline_path);record["os_facts"]=os_facts;
        let path=dir.join(format!("{}.json",control.binding["route"].as_str().unwrap()));write_json(&path,&record);println!("LUNA_STORAGE_V5_FAULT {}",record);
    }
    fn fault_snapshots(dir:&Path,path:&Path,before:Value)->Value {let backup=path.with_file_name(format!("{}.pre-v5-100.sqlite3",path.file_name().unwrap().to_string_lossy()));json!({"before":before,"after":capture(path,&dir.join("after.sqlite3")),"backup":capture(&backup,&dir.join("backup.sqlite3"))})}
    #[test]
    fn actual_schema5_host_rolls_back_after_ddl_and_recovers_same_original_profile() {
        emit_identity();let dir=directory("rollback");let path=dir.join("original-four.sqlite3");original_four(&path);let before_bytes=std::fs::read(&path).unwrap();let before=capture(&path,&dir.join("before.sqlite3"));
        let guard=Scope::new(&path,Mode::AfterDdl,"rollback",TEST_ROLLBACK,"migration-fault-profile","controlled-fault");let control=guard.control.clone();
        let failure=match SqliteStore::open(&path,"migration-fault-profile","controlled-fault",100){Ok(_)=>panic!("actual post005 SQL failure must reject"),Err(error)=>error};control.emit("caller_received_error","caller",json!({"error_code":failure.code}));
        assert!(failure.message.contains("invalid_source_selection_set"));assert_eq!(*control.sqlite_code.lock().unwrap(),Some(19));assert_eq!(std::fs::read(&path).unwrap(),before_bytes);
        let captures=fault_snapshots(&dir,&path,before);check_old(&path,&captures["before"],&captures["after"],&captures["backup"]);control.emit("post_error_invariants","independent_readback",json!({"snapshots":{"before":captures["before"]["snapshot"],"after":captures["after"]["snapshot"],"backup":captures["backup"]["snapshot"]}}));
        drop(guard);control.emit("cleanup_complete","test_cleanup",json!({"scope_removed":true}));drop(SqliteStore::open(&path,"migration-fault-profile","recovered",101).unwrap());
        let db=immutable(&path);assert_eq!(db.query_row("SELECT max(version) FROM schema_migrations",[],|r|r.get::<_,u32>(0)).unwrap(),5);assert_eq!(schema_identity(&db).unwrap(),expected_schema_identity(5).unwrap());assert_eq!(db.query_row("SELECT display_name FROM games WHERE game_id='nonempty-old-game'",[],|r|r.get::<_,String>(0)).unwrap(),"既存の原件");drop(db);
        let snapshot=capture(&path,&dir.join("recovered.sqlite3"))["snapshot"].clone();let recovery=json!({"snapshot":snapshot,"original_database_path":path,"profile_id":"migration-fault-profile","server_id":"recovered","now_ms":101});control.emit("recovery_complete","independent_readback",recovery.clone());complete_fault(&dir,&control,captures,recovery,json!({}));
    }
    #[test]
    fn actual_schema5_host_readonly_and_busy_leave_the_original_unchanged() {
        use std::os::unix::fs::PermissionsExt;
        emit_identity();for case in ["readonly","busy"] {
            let dir=directory(case);let path=dir.join("original-four.sqlite3");original_four(&path);let before_bytes=std::fs::read(&path).unwrap();let before=capture(&path,&dir.join("before.sqlite3"));
            let guard=Scope::new(&path,Mode::PauseBeforeWrite,case,TEST_OS,"migration-fault-profile","controlled-fault");let control=guard.control.clone();let writer_path=path.clone();let caller_control=control.clone();
            let child=std::thread::spawn(move||{let result=SqliteStore::open(&writer_path,"migration-fault-profile","controlled-fault",100);if let Err(error)=&result{caller_control.emit("caller_received_error","caller",json!({"error_code":error.code}));}result});guard.reached.recv_timeout(Duration::from_secs(15)).unwrap();
            let original_permissions=std::fs::metadata(&path).unwrap().permissions();let before_mode=original_permissions.mode();let mut os_facts;
            let lock=if case=="busy" {let db=Connection::open(&path).unwrap();// SAFETY: the live owned Connection outlives this identity observation; the returned SQLite pointer is formatted only, never dereferenced.
                let connection_id=format!("sqlite-connection-{:p}",unsafe{db.handle()});db.execute_batch("BEGIN IMMEDIATE").unwrap();os_facts=json!({"holder_pid":std::process::id(),"connection_id":connection_id,"acquired_at_ms":wall_ms(),"released_at_ms":0,"begin_sql":"BEGIN IMMEDIATE","release_sql":"ROLLBACK","begin_success":true,"release_success":false});Some(db)}else{let mut readonly=original_permissions.clone();readonly.set_readonly(true);std::fs::set_permissions(&path,readonly).unwrap();os_facts=json!({"before_mode":before_mode,"applied_mode":std::fs::metadata(&path).unwrap().permissions().mode(),"restored_mode":0,"applied_at_ms":wall_ms(),"restored_at_ms":0});assert_eq!(os_facts["applied_mode"].as_u64().unwrap()&0o222,0);None};
            control.emit("fault_applied","test_fault",os_facts.clone());guard.resume.send(()).unwrap();let failure=match child.join().unwrap(){Ok(_)=>panic!("actual {case} writer fault must reject"),Err(error)=>error};
            if let Some(lock)=lock {lock.execute_batch("ROLLBACK").unwrap();os_facts["released_at_ms"]=json!(wall_ms());os_facts["release_success"]=json!(true);}else{std::fs::set_permissions(&path,original_permissions).unwrap();os_facts["restored_mode"]=json!(std::fs::metadata(&path).unwrap().permissions().mode());os_facts["restored_at_ms"]=json!(wall_ms());assert_eq!(os_facts["restored_mode"],before_mode);}
            let expected=if case=="busy"{5}else{8};assert_eq!(*control.sqlite_code.lock().unwrap(),Some(expected),"actual native SQLite code: {}",failure.message);assert_eq!(failure.code,if case=="busy"{"storage_busy"}else{"storage_failure"});assert_eq!(std::fs::read(&path).unwrap(),before_bytes);
            let captures=fault_snapshots(&dir,&path,before);check_old(&path,&captures["before"],&captures["after"],&captures["backup"]);control.emit("post_error_invariants","independent_readback",json!({"snapshots":{"before":captures["before"]["snapshot"],"after":captures["after"]["snapshot"],"backup":captures["backup"]["snapshot"]}}));drop(guard);control.emit("cleanup_complete","test_cleanup",json!({"scope_removed":true,"os_facts":os_facts}));complete_fault(&dir,&control,captures,Value::Null,os_facts);
        }
    }
    fn timestamp(ms:u64)->String {
        let seconds=libc::time_t::try_from(ms/1000).unwrap();let mut value=std::mem::MaybeUninit::<libc::tm>::uninit();
        // SAFETY: seconds points to initialized time_t; value has space/alignment for tm. gmtime_r initializes it on non-null return.
        assert!(!unsafe{libc::gmtime_r(&seconds,value.as_mut_ptr())}.is_null());
        // SAFETY: successful gmtime_r immediately above initialized all fields read here.
        let value=unsafe{value.assume_init()};format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",value.tm_year+1900,value.tm_mon+1,value.tm_mday,value.tm_hour,value.tm_min,value.tm_sec,ms%1000)
    }
    fn run_hdiutil(dir:&Path,label:&str,args:&[&str])->(std::process::Output,Value) {
        let start=wall_ms();let child=std::process::Command::new("hdiutil").args(args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();let pid=child.id();let output=child.wait_with_output().unwrap();let end=wall_ms();
        let stdout=dir.join(format!("{label}.stdout"));let stderr=dir.join(format!("{label}.stderr"));std::fs::write(&stdout,&output.stdout).unwrap();std::fs::write(&stderr,&output.stderr).unwrap();let record=json!({"command":std::iter::once("hdiutil").chain(args.iter().copied()).collect::<Vec<_>>(),"cwd":".","pid":pid,"timed_out":false,"started_at":timestamp(start),"finished_at":timestamp(end),"exit_code":output.status.code().unwrap(),"fixture_environment":fixture_environment(),"stdout":artifact(&stdout),"stderr":artifact(&stderr)});let path=dir.join(format!("{label}.execution.json"));write_json(&path,&record);assert!(output.status.success(),"actual owned image {label}: {}",String::from_utf8_lossy(&output.stderr));(output,artifact(&path))
    }
    #[cfg(target_os="macos")]
    #[test]
    fn actual_schema5_host_full_isolated_volume_preserves_the_old_database() {
        use std::{io::Write,process::{Command,Stdio}};
        emit_identity();let dir=directory("enospc");let mount=dir.join("mount");std::fs::create_dir(&mount).unwrap();let image=dir.join("limited-64m.dmg");let (_,created)=run_hdiutil(&dir,"create",&["create","-size","64m","-fs","HFS+","-volname","LunaSchema5Full","-type","UDIF","-nospotlight",image.to_str().unwrap()]);
        let (attached,attached_ref)=run_hdiutil(&dir,"attach",&["attach","-plist","-nobrowse","-mountpoint",mount.to_str().unwrap(),image.to_str().unwrap()]);
        struct Mounted {device:Option<String>,dir:std::path::PathBuf}impl Drop for Mounted {fn drop(&mut self){if let Some(device)=self.device.take(){let result=Command::new("hdiutil").args(["detach",&device]).output().unwrap();let _=std::fs::write(self.dir.join("emergency-detach.stdout"),result.stdout);let _=std::fs::write(self.dir.join("emergency-detach.stderr"),result.stderr);if !std::thread::panicking(){assert!(result.status.success(),"owned test volume normal detach failed");}}}}
        let mut mounted=Mounted{device:Some(mount.to_string_lossy().into_owned()),dir:dir.clone()};let mut decode=Command::new("python3").args(["-c","import json,plistlib,sys;print(json.dumps(plistlib.loads(sys.stdin.buffer.read())))"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();decode.stdin.take().unwrap().write_all(&attached.stdout).unwrap();let decoded=decode.wait_with_output().unwrap();assert!(decoded.status.success());let plist:Value=serde_json::from_slice(&decoded.stdout).unwrap();let entities=plist["system-entities"].as_array().unwrap();let device=entities.iter().filter_map(|r|r["dev-entry"].as_str()).find(|d|d.strip_prefix("/dev/disk").is_some_and(|n|!n.is_empty()&&n.bytes().all(|b|b.is_ascii_digit()))).unwrap().to_owned();mounted.device=Some(device.clone());assert!(entities.iter().any(|r|r["mount-point"].as_str()==mount.to_str()&&r["dev-entry"].as_str().is_some_and(|p|p.starts_with(&device))));
        let mount_device=std::fs::metadata(&mount).unwrap().dev();let output_device=std::fs::metadata(&dir).unwrap().dev();assert_ne!(mount_device,output_device,"never fill the worktree filesystem");
        fn capacity(path:&Path)->(u64,u64){let path=std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();let mut info=std::mem::MaybeUninit::<libc::statvfs>::uninit();
            // SAFETY: path is a NUL-terminated live CString; info has space/alignment for statvfs, initialized on success.
            assert_eq!(unsafe{libc::statvfs(path.as_ptr(),info.as_mut_ptr())},0);
            // SAFETY: statvfs returned success immediately above and initialized info.
            let info=unsafe{info.assume_init()};(u64::from(info.f_blocks)*info.f_frsize,u64::from(info.f_bavail)*info.f_frsize)}
        let (total,_)=capacity(&mount);assert!(total>16*1024*1024&&total<=64*1024*1024);let path=mount.join("old-four.sqlite3");original_four(&path);let before_bytes=std::fs::read(&path).unwrap();let before=capture(&path,&dir.join("before.sqlite3"));let guard=Scope::new(&path,Mode::PauseBeforeWrite,"enospc",TEST_FULL,"migration-fault-profile","controlled-fault");let control=guard.control.clone();let writer_path=path.clone();let caller_control=control.clone();
        let child=std::thread::spawn(move||{let result=SqliteStore::open(&writer_path,"migration-fault-profile","controlled-fault",100);if let Err(error)=&result{caller_control.emit("caller_received_error","caller",json!({"error_code":error.code}));}result});guard.reached.recv_timeout(Duration::from_secs(15)).unwrap();let filler_path=mount.join("test-only-capacity-filler");let mut filler=std::fs::File::create(&filler_path).unwrap();let bytes=vec![0x5a_u8;65536];let error=loop{match filler.write_all(&bytes){Ok(())=>{},Err(error)=>break error}};let os_error_at=wall_ms();assert_eq!(error.raw_os_error(),Some(libc::ENOSPC));let filler_bytes=filler.metadata().unwrap().len();let _=filler.sync_all();drop(filler);let available=capacity(&mount).1;let sampled=wall_ms();control.emit("fault_applied","test_fault",json!({"os_errno":libc::ENOSPC,"filler_path":filler_path,"filler_bytes":filler_bytes,"capacity_bytes":total,"available_bytes_at_fault":available}));guard.resume.send(()).unwrap();
        let failure=match child.join().unwrap(){Ok(_)=>panic!("actual capacity-full volume must reject migration"),Err(error)=>error};assert_eq!(*control.sqlite_code.lock().unwrap(),Some(13),"native SQLite FULL required: {}",failure.message);assert_eq!(failure.code,"storage_failure");std::fs::remove_file(&filler_path).unwrap();let removed=wall_ms();assert_eq!(std::fs::read(&path).unwrap(),before_bytes);let captures=fault_snapshots(&dir,&path,before);check_old(&path,&captures["before"],&captures["after"],&captures["backup"]);control.emit("post_error_invariants","independent_readback",json!({"snapshots":{"before":captures["before"]["snapshot"],"after":captures["after"]["snapshot"],"backup":captures["backup"]["snapshot"]}}));let copied=wall_ms();drop(guard);let (_,detached)=run_hdiutil(&dir,"detach",&["detach",&device]);mounted.device=None;drop(mounted);let os_facts=json!({"commands":[created,attached_ref,detached],"image":image,"mount":mount,"device":device,"mount_device":mount_device,"output_device":output_device,"capacity_bytes":total,"available_bytes_at_fault":available,"observed_os_error":libc::ENOSPC,"os_error_at_ms":os_error_at,"capacity_sampled_at_ms":sampled,"filler_path":filler_path,"filler_bytes":filler_bytes,"filler_removed_at_ms":removed,"evidence_copied_at_ms":copied});control.emit("cleanup_complete","test_cleanup",json!({"scope_removed":true,"normal_detach_completed":true}));complete_fault(&dir,&control,captures,Value::Null,os_facts);
    }
}
