//! app.sqlite CRUD
//! 代码路径: kk_novel_ai/src-tauri/src/storage/app_store.rs

use super::schema::{self, STORAGE_VERSION};
use super::{app_sqlite_path, open_db};
use crate::error::{AppError, AppResult};
use crate::kb::{KbRegistry, KbRegistryEntry};
use crate::settings::{AppSettings, RecentProject};
use crate::usage::UsageLedger;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

pub fn open_app_db() -> AppResult<rusqlite::Connection> {
    let path = app_sqlite_path()?;
    let conn = open_db(&path)?;
    schema::init_app_schema(&conn)?;
    Ok(conn)
}

pub fn read_migration_done() -> AppResult<bool> {
    let path = app_sqlite_path()?;
    if !path.exists() {
        return Ok(false);
    }
    let conn = open_app_db()?;
    let done: i64 = conn
        .query_row(
            "SELECT done FROM migration_state WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(done != 0)
}

pub fn write_migration_state(
    done: bool,
    backup_path: Option<&str>,
    checkpoint_json: Option<&str>,
    last_error: Option<&str>,
) -> AppResult<()> {
    let conn = open_app_db()?;
    let done_at = if done {
        Some(Utc::now().to_rfc3339())
    } else {
        None
    };
    conn.execute(
        "UPDATE migration_state SET version = ?1, done = ?2, done_at = COALESCE(?3, done_at),
         backup_path = COALESCE(?4, backup_path), checkpoint_json = COALESCE(?5, checkpoint_json),
         last_error = ?6 WHERE id = 1",
        params![
            STORAGE_VERSION,
            if done { 1 } else { 0 },
            done_at,
            backup_path,
            checkpoint_json,
            last_error,
        ],
    )
    .map_err(|e| AppError::msg(format!("write migration_state: {e}")))?;
    Ok(())
}

pub fn read_checkpoint_json() -> AppResult<Option<String>> {
    let conn = open_app_db()?;
    let v: Option<String> = conn
        .query_row(
            "SELECT checkpoint_json FROM migration_state WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?
        .flatten();
    Ok(v)
}

pub fn read_backup_path() -> AppResult<Option<String>> {
    let conn = open_app_db()?;
    let v: Option<String> = conn
        .query_row(
            "SELECT backup_path FROM migration_state WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?
        .flatten();
    Ok(v)
}

pub fn save_settings_json(settings: &AppSettings) -> AppResult<()> {
    let conn = open_app_db()?;
    let json = serde_json::to_string(settings)?;
    conn.execute(
        "INSERT INTO kv_settings(key, value_json) VALUES('settings', ?1)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
        params![json],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    // recent 同步到表
    conn.execute("DELETE FROM recent_projects", [])
        .map_err(|e| AppError::msg(e.to_string()))?;
    for r in &settings.recent_projects {
        conn.execute(
            "INSERT OR REPLACE INTO recent_projects(path, title, opened_at) VALUES(?1,?2,?3)",
            params![r.path, r.title, r.opened_at],
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    }
    Ok(())
}

pub fn load_settings_json() -> AppResult<Option<AppSettings>> {
    let path = app_sqlite_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let conn = open_app_db()?;
    let json: Option<String> = conn
        .query_row(
            "SELECT value_json FROM kv_settings WHERE key = 'settings'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    match json {
        Some(s) if !s.trim().is_empty() => Ok(Some(serde_json::from_str(&s)?)),
        _ => Ok(None),
    }
}

pub fn save_usage_ledger(ledger: &UsageLedger) -> AppResult<()> {
    let conn = open_app_db()?;
    let json = serde_json::to_string(ledger)?;
    conn.execute(
        "INSERT INTO usage_ledger(id, value_json) VALUES(1, ?1)
         ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
        params![json],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_usage_ledger() -> AppResult<Option<UsageLedger>> {
    let path = app_sqlite_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let conn = open_app_db()?;
    let json: Option<String> = conn
        .query_row(
            "SELECT value_json FROM usage_ledger WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    match json {
        Some(s) => Ok(Some(serde_json::from_str(&s).unwrap_or_default())),
        None => Ok(None),
    }
}

pub fn save_kb_registry(reg: &KbRegistry) -> AppResult<()> {
    let conn = open_app_db()?;
    conn.execute("DELETE FROM kb_registry", [])
        .map_err(|e| AppError::msg(e.to_string()))?;
    for e in &reg.entries {
        conn.execute(
            "INSERT INTO kb_registry(id, path, title, kind, last_synced_at, registered_at)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                e.id,
                e.path,
                e.title,
                e.kind,
                e.last_synced_at,
                e.registered_at
            ],
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    }
    Ok(())
}

pub fn load_kb_registry() -> AppResult<Option<KbRegistry>> {
    let path = app_sqlite_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let conn = open_app_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, path, title, kind, last_synced_at, registered_at FROM kb_registry",
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(KbRegistryEntry {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                kind: r.get(3)?,
                last_synced_at: r.get(4)?,
                registered_at: r.get(5)?,
            })
        })
        .map_err(|e| AppError::msg(e.to_string()))?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| AppError::msg(e.to_string()))?);
    }
    if entries.is_empty() {
        return Ok(None);
    }
    Ok(Some(KbRegistry { entries }))
}

pub fn save_work_catalog_blob(key: &str, value: &Value) -> AppResult<()> {
    let conn = open_app_db()?;
    let json = serde_json::to_string(value)?;
    conn.execute(
        "INSERT INTO work_catalog(key, value_json) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
        params![key, json],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_work_catalog_blob(key: &str) -> AppResult<Option<Value>> {
    let path = app_sqlite_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let conn = open_app_db()?;
    let json: Option<String> = conn
        .query_row(
            "SELECT value_json FROM work_catalog WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    match json {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

pub fn load_recent_projects() -> AppResult<Vec<RecentProject>> {
    let conn = open_app_db()?;
    let mut stmt = conn
        .prepare("SELECT path, title, opened_at FROM recent_projects ORDER BY opened_at DESC")
        .map_err(|e| AppError::msg(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(RecentProject {
                path: r.get(0)?,
                title: r.get(1)?,
                opened_at: r.get(2)?,
            })
        })
        .map_err(|e| AppError::msg(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| AppError::msg(e.to_string()))?);
    }
    Ok(out)
}
