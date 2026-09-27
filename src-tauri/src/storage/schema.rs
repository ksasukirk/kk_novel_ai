//! 各库 DDL
//! 代码路径: kk_novel_ai/src-tauri/src/storage/schema.rs

use crate::error::{AppError, AppResult};
use rusqlite::Connection;

pub const STORAGE_VERSION: i32 = 1;

pub fn init_app_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS kv_settings (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS recent_projects (
            path TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            opened_at TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS work_catalog (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS kb_registry (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT '',
            last_synced_at TEXT,
            registered_at TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS usage_ledger (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS migration_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            version INTEGER NOT NULL DEFAULT 0,
            done INTEGER NOT NULL DEFAULT 0,
            done_at TEXT,
            backup_path TEXT,
            checkpoint_json TEXT,
            last_error TEXT
        );
        INSERT OR IGNORE INTO migration_state (id, version, done) VALUES (1, 0, 0);
        "#,
    )
    .map_err(|e| AppError::msg(format!("init app schema: {e}")))?;
    Ok(())
}

pub fn init_library_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS tropes (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            keywords_json TEXT NOT NULL DEFAULT '[]',
            tags_json TEXT NOT NULL DEFAULT '[]',
            attrs_json TEXT NOT NULL DEFAULT '{}',
            links_json TEXT NOT NULL DEFAULT '[]',
            sources_json TEXT NOT NULL DEFAULT '[]',
            unique_flag INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT '',
            snippet TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS kinks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            keywords_json TEXT NOT NULL DEFAULT '[]',
            tags_json TEXT NOT NULL DEFAULT '[]',
            attrs_json TEXT NOT NULL DEFAULT '{}',
            links_json TEXT NOT NULL DEFAULT '[]',
            sources_json TEXT NOT NULL DEFAULT '[]',
            unique_flag INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT '',
            snippet TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_tropes_title ON tropes(title);
        CREATE INDEX IF NOT EXISTS idx_tropes_updated ON tropes(updated_at);
        CREATE INDEX IF NOT EXISTS idx_kinks_title ON kinks(title);
        CREATE INDEX IF NOT EXISTS idx_kinks_updated ON kinks(updated_at);
        "#,
    )
    .map_err(|e| AppError::msg(format!("init library schema: {e}")))?;
    Ok(())
}

pub fn init_work_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS project_meta (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'novel',
            genre TEXT NOT NULL DEFAULT '',
            style TEXT NOT NULL DEFAULT '',
            book_outline TEXT NOT NULL DEFAULT '',
            source_file TEXT,
            title_src TEXT NOT NULL DEFAULT '',
            linked_kb_roots_json TEXT NOT NULL DEFAULT '[]',
            volumes_json TEXT NOT NULL DEFAULT '[]',
            outline_mindmap_json TEXT,
            created_at TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT '',
            trope_summary_at TEXT,
            trope_summary_fingerprint TEXT,
            trope_summary_dirty INTEGER NOT NULL DEFAULT 0,
            legacy_sections_collapsed INTEGER NOT NULL DEFAULT 0,
            extra_json TEXT NOT NULL DEFAULT '{}'
        );
        CREATE TABLE IF NOT EXISTS chapters (
            id TEXT PRIMARY KEY,
            file TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            title_src TEXT NOT NULL DEFAULT '',
            summary TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'draft',
            pov_lore_id TEXT,
            focus_arc_ids_json TEXT NOT NULL DEFAULT '[]',
            must_do TEXT NOT NULL DEFAULT '',
            must_not TEXT NOT NULL DEFAULT '',
            reader_knows TEXT NOT NULL DEFAULT '',
            character_knows TEXT NOT NULL DEFAULT '',
            beats_json TEXT NOT NULL DEFAULT '[]',
            trope_ids_json TEXT NOT NULL DEFAULT '[]',
            content TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            meta_json TEXT NOT NULL DEFAULT '{}'
        );
        CREATE INDEX IF NOT EXISTS idx_chapters_sort ON chapters(sort_order);
        CREATE TABLE IF NOT EXISTS lore (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            keywords_json TEXT NOT NULL DEFAULT '[]',
            links_json TEXT NOT NULL DEFAULT '[]',
            attrs_json TEXT NOT NULL DEFAULT '{}',
            sources_json TEXT NOT NULL DEFAULT '[]',
            unique_flag INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_lore_kind ON lore(kind);
        CREATE TABLE IF NOT EXISTS genblocks (
            chapter_id TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS beat_progress (
            chapter_id TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS block_notes (
            chapter_id TEXT NOT NULL,
            block_key TEXT NOT NULL,
            value_json TEXT NOT NULL,
            PRIMARY KEY (chapter_id, block_key)
        );
        CREATE TABLE IF NOT EXISTS story_docs (
            name TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memory (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS stats (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS novel_chat (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS gen_activity (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts TEXT,
            line_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS kv_misc (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| AppError::msg(format!("init work schema: {e}")))?;
    Ok(())
}

pub fn init_characters_schema(conn: &Connection) -> AppResult<()> {
    init_roster_like_schema(conn, "characters")
}

pub fn init_kb_schema(conn: &Connection) -> AppResult<()> {
    init_roster_like_schema(conn, "kb")
}

fn init_roster_like_schema(conn: &Connection, label: &str) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS project_meta (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT '',
            meta_json TEXT NOT NULL DEFAULT '{}'
        );
        CREATE TABLE IF NOT EXISTS lore (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            keywords_json TEXT NOT NULL DEFAULT '[]',
            links_json TEXT NOT NULL DEFAULT '[]',
            attrs_json TEXT NOT NULL DEFAULT '{}',
            sources_json TEXT NOT NULL DEFAULT '[]',
            unique_flag INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT '',
            namespace TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_lore_ns_kind ON lore(namespace, kind);
        CREATE TABLE IF NOT EXISTS story_docs (
            name TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS kv_misc (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| AppError::msg(format!("init {label} schema: {e}")))?;
    Ok(())
}

pub fn init_activity_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS gen_log (
            id TEXT PRIMARY KEY,
            ts TEXT NOT NULL DEFAULT '',
            line_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_gen_log_ts ON gen_log(ts);
        CREATE TABLE IF NOT EXISTS chat_free (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS usage_detail (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            value_json TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| AppError::msg(format!("init activity schema: {e}")))?;
    Ok(())
}
