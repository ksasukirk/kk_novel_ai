//! 全局情节库 library.sqlite
//! 代码路径: kk_novel_ai/src-tauri/src/storage/library_store.rs

use super::schema;
use super::{library_sqlite_path, open_db};
use crate::error::{AppError, AppResult};
use crate::project::{LoreEntry, LoreLink, LoreSource};
use rusqlite::{params, OptionalExtension};

fn snippet_of(content: &str) -> String {
    content.chars().take(160).collect()
}

fn table_for_kind(kind: &str) -> &'static str {
    if kind == "kink" || kind.starts_with("kink") {
        "kinks"
    } else {
        "tropes"
    }
}

pub fn open_library_db() -> AppResult<rusqlite::Connection> {
    let path = library_sqlite_path()?;
    let conn = open_db(&path)?;
    schema::init_library_schema(&conn)?;
    Ok(conn)
}

pub fn replace_kind(kind: &str, entries: &[LoreEntry]) -> AppResult<()> {
    let table = table_for_kind(kind);
    let conn = open_library_db()?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::msg(e.to_string()))?;
    tx.execute(&format!("DELETE FROM {table}"), [])
        .map_err(|e| AppError::msg(e.to_string()))?;
    for e in entries {
        upsert_tx(&tx, table, e)?;
    }
    tx.commit().map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

fn upsert_tx(tx: &rusqlite::Transaction<'_>, table: &str, e: &LoreEntry) -> AppResult<()> {
    let tags = e
        .attrs
        .get("tags")
        .cloned()
        .unwrap_or_else(|| "[]".into());
    let sql = format!(
        "INSERT INTO {table}(id, title, content, keywords_json, tags_json, attrs_json,
            links_json, sources_json, unique_flag, updated_at, snippet)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
         ON CONFLICT(id) DO UPDATE SET
            title=excluded.title, content=excluded.content, keywords_json=excluded.keywords_json,
            tags_json=excluded.tags_json, attrs_json=excluded.attrs_json,
            links_json=excluded.links_json, sources_json=excluded.sources_json,
            unique_flag=excluded.unique_flag, updated_at=excluded.updated_at,
            snippet=excluded.snippet"
    );
    tx.execute(
        &sql,
        params![
            e.id,
            e.title,
            e.content,
            serde_json::to_string(&e.keywords)?,
            tags,
            serde_json::to_string(&e.attrs)?,
            serde_json::to_string(&e.links)?,
            serde_json::to_string(&e.sources)?,
            if e.unique { 1 } else { 0 },
            e.updated_at,
            snippet_of(&e.content),
        ],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn upsert_entry(e: &LoreEntry) -> AppResult<()> {
    let table = table_for_kind(&e.kind);
    let conn = open_library_db()?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::msg(e.to_string()))?;
    upsert_tx(&tx, table, e)?;
    tx.commit().map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn delete_entry(kind: &str, id: &str) -> AppResult<()> {
    let table = table_for_kind(kind);
    let conn = open_library_db()?;
    conn.execute(
        &format!("DELETE FROM {table} WHERE id = ?1"),
        params![id],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

fn list_table(table: &str, lite: bool) -> AppResult<Vec<LoreEntry>> {
    let conn = open_library_db()?;
    let cols = if lite {
        "id, title, snippet, keywords_json, tags_json, attrs_json, links_json, sources_json, unique_flag, updated_at, ''"
    } else {
        "id, title, content, keywords_json, tags_json, attrs_json, links_json, sources_json, unique_flag, updated_at, content"
    };
    // lite: content = snippet for display; full content empty-ish via snippet field as content
    let sql = if lite {
        format!(
            "SELECT id, title, snippet as content, keywords_json, attrs_json, links_json,
                    sources_json, unique_flag, updated_at FROM {table} ORDER BY updated_at DESC, title ASC"
        )
    } else {
        format!(
            "SELECT id, title, content, keywords_json, attrs_json, links_json,
                    sources_json, unique_flag, updated_at FROM {table} ORDER BY updated_at DESC, title ASC"
        )
    };
    let _ = cols;
    let kind = if table == "kinks" { "kink" } else { "trope" };
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| AppError::msg(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            let keywords: String = r.get(3)?;
            let attrs: String = r.get(4)?;
            let links: String = r.get(5)?;
            let sources: String = r.get(6)?;
            Ok(LoreEntry {
                id: r.get(0)?,
                kind: kind.into(),
                title: r.get(1)?,
                content: r.get(2)?,
                keywords: serde_json::from_str(&keywords).unwrap_or_default(),
                links: serde_json::from_str::<Vec<LoreLink>>(&links).unwrap_or_default(),
                attrs: serde_json::from_str(&attrs).unwrap_or_default(),
                sources: serde_json::from_str::<Vec<LoreSource>>(&sources).unwrap_or_default(),
                unique: r.get::<_, i64>(7)? != 0,
                updated_at: r.get(8)?,
            })
        })
        .map_err(|e| AppError::msg(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| AppError::msg(e.to_string()))?);
    }
    Ok(out)
}

/// 列表摘要（content 为 snippet，无全文）
pub fn list_lite() -> AppResult<Vec<LoreEntry>> {
    let mut out = list_table("tropes", true)?;
    out.extend(list_table("kinks", true)?);
    Ok(out)
}

pub fn list_full() -> AppResult<Vec<LoreEntry>> {
    let mut out = list_table("tropes", false)?;
    out.extend(list_table("kinks", false)?);
    Ok(out)
}

pub fn get_entry(kind: &str, id: &str) -> AppResult<Option<LoreEntry>> {
    let table = table_for_kind(kind);
    let kind_s = if table == "kinks" { "kink" } else { "trope" };
    let conn = open_library_db()?;
    let row = conn
        .query_row(
            &format!(
                "SELECT id, title, content, keywords_json, attrs_json, links_json,
                        sources_json, unique_flag, updated_at FROM {table} WHERE id = ?1"
            ),
            params![id],
            |r| {
                let keywords: String = r.get(3)?;
                let attrs: String = r.get(4)?;
                let links: String = r.get(5)?;
                let sources: String = r.get(6)?;
                Ok(LoreEntry {
                    id: r.get(0)?,
                    kind: kind_s.into(),
                    title: r.get(1)?,
                    content: r.get(2)?,
                    keywords: serde_json::from_str(&keywords).unwrap_or_default(),
                    links: serde_json::from_str::<Vec<LoreLink>>(&links).unwrap_or_default(),
                    attrs: serde_json::from_str(&attrs).unwrap_or_default(),
                    sources: serde_json::from_str::<Vec<LoreSource>>(&sources).unwrap_or_default(),
                    unique: r.get::<_, i64>(7)? != 0,
                    updated_at: r.get(8)?,
                })
            },
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(row)
}

pub fn count_all() -> AppResult<(i64, i64)> {
    let conn = open_library_db()?;
    let t: i64 = conn
        .query_row("SELECT COUNT(*) FROM tropes", [], |r| r.get(0))
        .unwrap_or(0);
    let k: i64 = conn
        .query_row("SELECT COUNT(*) FROM kinks", [], |r| r.get(0))
        .unwrap_or(0);
    Ok((t, k))
}
