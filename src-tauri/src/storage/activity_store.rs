//! 全局活动库 activity.sqlite
//! 代码路径: kk_novel_ai/src-tauri/src/storage/activity_store.rs

use super::schema;
use super::{activity_sqlite_path, open_db};
use crate::chat::ChatSession;
use crate::error::{AppError, AppResult};
use crate::genlog::GenLogEntry;
use rusqlite::{params, OptionalExtension};

pub fn open_activity_db() -> AppResult<rusqlite::Connection> {
    let path = activity_sqlite_path()?;
    let conn = open_db(&path)?;
    schema::init_activity_schema(&conn)?;
    Ok(conn)
}

pub fn append_gen_log(entry: &GenLogEntry) -> AppResult<()> {
    let conn = open_activity_db()?;
    let id = if entry.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        entry.id.clone()
    };
    conn.execute(
        "INSERT INTO gen_log(id, ts, line_json) VALUES(?1,?2,?3)
         ON CONFLICT(id) DO UPDATE SET ts=excluded.ts, line_json=excluded.line_json",
        params![id, entry.ts, serde_json::to_string(entry)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn list_gen_log(limit: usize) -> AppResult<Vec<GenLogEntry>> {
    let path = activity_sqlite_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let conn = open_activity_db()?;
    let mut stmt = conn
        .prepare("SELECT line_json FROM gen_log ORDER BY ts DESC LIMIT ?1")
        .map_err(|e| AppError::msg(e.to_string()))?;
    let rows = stmt
        .query_map(params![limit as i64], |r| r.get::<_, String>(0))
        .map_err(|e| AppError::msg(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        let s = row.map_err(|e| AppError::msg(e.to_string()))?;
        if let Ok(e) = serde_json::from_str(&s) {
            out.push(e);
        }
    }
    Ok(out)
}

pub fn replace_gen_log(entries: &[GenLogEntry]) -> AppResult<()> {
    let conn = open_activity_db()?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::msg(e.to_string()))?;
    tx.execute("DELETE FROM gen_log", [])
        .map_err(|e| AppError::msg(e.to_string()))?;
    for entry in entries {
        let id = if entry.id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            entry.id.clone()
        };
        tx.execute(
            "INSERT INTO gen_log(id, ts, line_json) VALUES(?1,?2,?3)",
            params![id, entry.ts, serde_json::to_string(entry)?],
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    }
    tx.commit().map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn save_chat_free(session: &ChatSession) -> AppResult<()> {
    let conn = open_activity_db()?;
    conn.execute(
        "INSERT INTO chat_free(id, value_json) VALUES(1, ?1)
         ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
        params![serde_json::to_string(session)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_chat_free() -> AppResult<Option<ChatSession>> {
    let path = activity_sqlite_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let conn = open_activity_db()?;
    let json: Option<String> = conn
        .query_row(
            "SELECT value_json FROM chat_free WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()))
}
