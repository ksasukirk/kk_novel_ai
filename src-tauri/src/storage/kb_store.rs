//! 通用知识库 kb.sqlite
//! 代码路径: kk_novel_ai/src-tauri/src/storage/kb_store.rs

use super::schema;
use super::{kb_sqlite_path, open_db};
use crate::error::{AppError, AppResult};
use crate::project::{LoreEntry, LoreLink, LoreSource, NovelProject};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

pub fn open_kb_db() -> AppResult<rusqlite::Connection> {
    let path = kb_sqlite_path()?;
    let conn = open_db(&path)?;
    schema::init_kb_schema(&conn)?;
    Ok(conn)
}

pub fn save_meta(project: &NovelProject) -> AppResult<()> {
    let conn = open_kb_db()?;
    conn.execute(
        "INSERT INTO project_meta(id, title, kind, meta_json) VALUES(?1,?2,?3,?4)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title, kind=excluded.kind, meta_json=excluded.meta_json",
        params![
            project.id,
            project.title,
            project.kind,
            serde_json::to_string(project)?
        ],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_meta() -> AppResult<Option<NovelProject>> {
    let path = kb_sqlite_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let conn = open_kb_db()?;
    let json: Option<String> = conn
        .query_row(
            "SELECT meta_json FROM project_meta LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()))
}

pub fn replace_lore(entries: &[LoreEntry]) -> AppResult<()> {
    let conn = open_kb_db()?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::msg(e.to_string()))?;
    tx.execute("DELETE FROM lore", [])
        .map_err(|e| AppError::msg(e.to_string()))?;
    for e in entries {
        tx.execute(
            "INSERT INTO lore(id, kind, title, content, keywords_json, links_json, attrs_json,
                              sources_json, unique_flag, updated_at, namespace)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'')",
            params![
                e.id,
                e.kind,
                e.title,
                e.content,
                serde_json::to_string(&e.keywords)?,
                serde_json::to_string(&e.links)?,
                serde_json::to_string(&e.attrs)?,
                serde_json::to_string(&e.sources)?,
                if e.unique { 1 } else { 0 },
                e.updated_at,
            ],
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    }
    tx.commit().map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn list_lore() -> AppResult<Vec<LoreEntry>> {
    let path = kb_sqlite_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let conn = open_kb_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, title, content, keywords_json, links_json, attrs_json,
                    sources_json, unique_flag, updated_at FROM lore",
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            let keywords: String = r.get(4)?;
            let links: String = r.get(5)?;
            let attrs: String = r.get(6)?;
            let sources: String = r.get(7)?;
            Ok(LoreEntry {
                id: r.get(0)?,
                kind: r.get(1)?,
                title: r.get(2)?,
                content: r.get(3)?,
                keywords: serde_json::from_str(&keywords).unwrap_or_default(),
                links: serde_json::from_str::<Vec<LoreLink>>(&links).unwrap_or_default(),
                attrs: serde_json::from_str(&attrs).unwrap_or_default(),
                sources: serde_json::from_str::<Vec<LoreSource>>(&sources).unwrap_or_default(),
                unique: r.get::<_, i64>(8)? != 0,
                updated_at: r.get(9)?,
            })
        })
        .map_err(|e| AppError::msg(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| AppError::msg(e.to_string()))?);
    }
    Ok(out)
}

pub fn upsert_lore(e: &LoreEntry) -> AppResult<()> {
    let conn = open_kb_db()?;
    conn.execute(
        "INSERT INTO lore(id, kind, title, content, keywords_json, links_json, attrs_json,
                          sources_json, unique_flag, updated_at, namespace)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'')
         ON CONFLICT(id) DO UPDATE SET
            kind=excluded.kind, title=excluded.title, content=excluded.content,
            keywords_json=excluded.keywords_json, links_json=excluded.links_json,
            attrs_json=excluded.attrs_json, sources_json=excluded.sources_json,
            unique_flag=excluded.unique_flag, updated_at=excluded.updated_at",
        params![
            e.id,
            e.kind,
            e.title,
            e.content,
            serde_json::to_string(&e.keywords)?,
            serde_json::to_string(&e.links)?,
            serde_json::to_string(&e.attrs)?,
            serde_json::to_string(&e.sources)?,
            if e.unique { 1 } else { 0 },
            e.updated_at,
        ],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn delete_lore(id: &str) -> AppResult<()> {
    let conn = open_kb_db()?;
    conn.execute("DELETE FROM lore WHERE id = ?1", params![id])
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn save_story_doc(name: &str, value: &Value) -> AppResult<()> {
    let conn = open_kb_db()?;
    conn.execute(
        "INSERT INTO story_docs(name, value_json) VALUES(?1,?2)
         ON CONFLICT(name) DO UPDATE SET value_json = excluded.value_json",
        params![name, serde_json::to_string(value)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_story_doc(name: &str) -> AppResult<Option<Value>> {
    let path = kb_sqlite_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let conn = open_kb_db()?;
    let json: Option<String> = conn
        .query_row(
            "SELECT value_json FROM story_docs WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()))
}
