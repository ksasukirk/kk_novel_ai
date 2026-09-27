//! 分域多 SQLite 存储与启动迁移
//! 代码路径: kk_novel_ai/src-tauri/src/storage/mod.rs

pub mod activity_store;
pub mod app_store;
mod backup_startup;
pub mod characters_store;
pub mod kb_store;
pub mod legacy_archive;
pub mod library_store;
mod migrate;
pub mod schema;
pub mod work_store;

#[allow(unused_imports)]
pub use migrate::{migration_run, migration_status, MigrationProgress, MigrationStatus};

use crate::error::{AppError, AppResult};
use crate::paths::app_data_dir;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

static MIGRATE_LOCK: AtomicBool = AtomicBool::new(false);
static MIGRATED_CACHE: AtomicBool = AtomicBool::new(false);
static MIGRATED_KNOWN: AtomicBool = AtomicBool::new(false);
static PROGRESS_FN: OnceLock<Mutex<Option<ProgressFn>>> = OnceLock::new();

pub type ProgressFn = Arc<dyn Fn(MigrationProgress) + Send + Sync>;

pub fn migration_in_progress() -> bool {
    MIGRATE_LOCK.load(Ordering::SeqCst)
}

pub fn set_migration_in_progress(v: bool) {
    MIGRATE_LOCK.store(v, Ordering::SeqCst);
}

pub fn app_sqlite_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("app.sqlite"))
}

pub fn characters_sqlite_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("characters.sqlite"))
}

pub fn kb_sqlite_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("kb.sqlite"))
}

pub fn activity_sqlite_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("activity.sqlite"))
}

pub fn library_sqlite_path() -> AppResult<PathBuf> {
    Ok(crate::paths::trope_library_dir()?.join("library.sqlite"))
}

pub fn work_sqlite_path(root: &Path) -> PathBuf {
    root.join("work.sqlite")
}

pub fn open_db(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path).map_err(|e| {
        AppError::msg(format!("打开 SQLite 失败 {}: {e}", path.display()))
    })?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA busy_timeout=5000;
         PRAGMA synchronous=NORMAL;",
    )
    .map_err(|e| AppError::msg(format!("PRAGMA 失败: {e}")))?;
    Ok(conn)
}

pub fn invalidate_migrated_cache() {
    MIGRATED_KNOWN.store(false, Ordering::SeqCst);
}

pub fn is_storage_migrated() -> bool {
    if MIGRATED_KNOWN.load(Ordering::SeqCst) {
        return MIGRATED_CACHE.load(Ordering::SeqCst);
    }
    let done = app_store::read_migration_done().unwrap_or(false);
    MIGRATED_CACHE.store(done, Ordering::SeqCst);
    MIGRATED_KNOWN.store(true, Ordering::SeqCst);
    done
}

pub fn mark_storage_migrated_cached(done: bool) {
    MIGRATED_CACHE.store(done, Ordering::SeqCst);
    MIGRATED_KNOWN.store(true, Ordering::SeqCst);
}

pub fn work_db_exists(root: &Path) -> bool {
    work_sqlite_path(root).is_file()
}

pub fn is_project_root(dir: &Path) -> bool {
    work_sqlite_path(dir).is_file() || dir.join("project.json").is_file()
}

pub fn set_progress_emitter(f: Option<ProgressFn>) {
    let cell = PROGRESS_FN.get_or_init(|| Mutex::new(None));
    *cell.lock() = f;
}

pub fn emit_progress(p: MigrationProgress) {
    if let Some(cell) = PROGRESS_FN.get() {
        if let Some(ref f) = *cell.lock() {
            f(p);
        }
    }
}
