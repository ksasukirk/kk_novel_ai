//! 每作品 work.sqlite
//! 代码路径: kk_novel_ai/src-tauri/src/storage/work_store.rs

use super::schema;
use super::{open_db, work_sqlite_path};
use crate::error::{AppError, AppResult};
use crate::project::{
    ChapterMeta, LoreEntry, LoreLink, LoreSource, MemoryStore, NovelProject, OutlineMindMap,
    VolumeMeta,
};
use crate::chat::ChatSession;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use std::path::Path;

pub fn open_work_db(root: &Path) -> AppResult<rusqlite::Connection> {
    let path = work_sqlite_path(root);
    let conn = open_db(&path)?;
    schema::init_work_schema(&conn)?;
    Ok(conn)
}

fn snippet_of(content: &str, n: usize) -> String {
    content.chars().take(n).collect()
}

pub fn save_full_project(
    root: &Path,
    project: &NovelProject,
    chapter_bodies: &[(String, String)],
) -> AppResult<()> {
    let conn = open_work_db(root)?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::msg(e.to_string()))?;

    let mindmap = project
        .outline_mindmap
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    tx.execute(
        "INSERT INTO project_meta(
            id, title, kind, genre, style, book_outline, source_file, title_src,
            linked_kb_roots_json, volumes_json, outline_mindmap_json,
            created_at, updated_at, trope_summary_at, trope_summary_fingerprint,
            trope_summary_dirty, legacy_sections_collapsed, extra_json
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,'{}')
         ON CONFLICT(id) DO UPDATE SET
            title=excluded.title, kind=excluded.kind, genre=excluded.genre, style=excluded.style,
            book_outline=excluded.book_outline, source_file=excluded.source_file,
            title_src=excluded.title_src, linked_kb_roots_json=excluded.linked_kb_roots_json,
            volumes_json=excluded.volumes_json, outline_mindmap_json=excluded.outline_mindmap_json,
            created_at=excluded.created_at, updated_at=excluded.updated_at,
            trope_summary_at=excluded.trope_summary_at,
            trope_summary_fingerprint=excluded.trope_summary_fingerprint,
            trope_summary_dirty=excluded.trope_summary_dirty,
            legacy_sections_collapsed=excluded.legacy_sections_collapsed",
        params![
            project.id,
            project.title,
            project.kind,
            project.genre,
            project.style,
            project.book_outline,
            project.source_file,
            project.title_src,
            serde_json::to_string(&project.linked_kb_roots)?,
            serde_json::to_string(&project.volumes)?,
            mindmap,
            project.created_at,
            project.updated_at,
            project.trope_summary_at,
            project.trope_summary_fingerprint,
            if project.trope_summary_dirty { 1 } else { 0 },
            if project.legacy_sections_collapsed { 1 } else { 0 },
        ],
    )
    .map_err(|e| AppError::msg(format!("save project_meta: {e}")))?;

    // 若只更新 meta，保留已有 content
    let mut body_map: std::collections::HashMap<String, String> = chapter_bodies
        .iter()
        .cloned()
        .collect();
    if body_map.is_empty() {
        let mut stmt = tx
            .prepare("SELECT id, content FROM chapters")
            .map_err(|e| AppError::msg(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| AppError::msg(e.to_string()))?;
        for row in rows {
            let (id, c) = row.map_err(|e| AppError::msg(e.to_string()))?;
            body_map.insert(id, c);
        }
    }

    tx.execute("DELETE FROM chapters", [])
        .map_err(|e| AppError::msg(e.to_string()))?;
    for (i, ch) in project.chapters.iter().enumerate() {
        let content = body_map.get(&ch.id).cloned().unwrap_or_default();
        tx.execute(
            "INSERT INTO chapters(
                id, file, title, title_src, summary, status, pov_lore_id,
                focus_arc_ids_json, must_do, must_not, reader_knows, character_knows,
                beats_json, trope_ids_json, content, sort_order, meta_json
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,'{}')",
            params![
                ch.id,
                ch.file,
                ch.title,
                ch.title_src,
                ch.summary,
                ch.status,
                ch.pov_lore_id,
                serde_json::to_string(&ch.focus_arc_ids)?,
                ch.must_do,
                ch.must_not,
                ch.reader_knows,
                ch.character_knows,
                serde_json::to_string(&ch.beats)?,
                serde_json::to_string(&ch.trope_ids)?,
                content,
                i as i64,
            ],
        )
        .map_err(|e| AppError::msg(format!("save chapter: {e}")))?;
    }

    tx.commit()
        .map_err(|e| AppError::msg(format!("commit work: {e}")))?;
    Ok(())
}

pub fn save_project_meta_only(root: &Path, project: &NovelProject) -> AppResult<()> {
    save_full_project(root, project, &[])
}

pub fn load_project(root: &Path) -> AppResult<NovelProject> {
    let conn = open_work_db(root)?;
    let mut project = conn
        .query_row(
            "SELECT id, title, kind, genre, style, book_outline, source_file, title_src,
                    linked_kb_roots_json, volumes_json, outline_mindmap_json,
                    created_at, updated_at, trope_summary_at, trope_summary_fingerprint,
                    trope_summary_dirty, legacy_sections_collapsed
             FROM project_meta LIMIT 1",
            [],
            |r| {
                let linked: String = r.get(8)?;
                let volumes: String = r.get(9)?;
                let mindmap: Option<String> = r.get(10)?;
                Ok(NovelProject {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    kind: r.get(2)?,
                    genre: r.get(3)?,
                    style: r.get(4)?,
                    book_outline: r.get(5)?,
                    source_file: r.get(6)?,
                    title_src: r.get(7)?,
                    linked_kb_roots: serde_json::from_str(&linked).unwrap_or_default(),
                    volumes: serde_json::from_str::<Vec<VolumeMeta>>(&volumes).unwrap_or_default(),
                    chapters: vec![],
                    outline_mindmap: mindmap
                        .and_then(|s| serde_json::from_str::<OutlineMindMap>(&s).ok()),
                    created_at: r.get(11)?,
                    updated_at: r.get(12)?,
                    trope_summary_at: r.get(13)?,
                    trope_summary_fingerprint: r.get(14)?,
                    trope_summary_dirty: r.get::<_, i64>(15)? != 0,
                    legacy_sections_collapsed: r.get::<_, i64>(16)? != 0,
                })
            },
        )
        .map_err(|e| AppError::msg(format!("load project_meta: {e}")))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, file, title, title_src, summary, status, pov_lore_id,
                    focus_arc_ids_json, must_do, must_not, reader_knows, character_knows,
                    beats_json, trope_ids_json
             FROM chapters ORDER BY sort_order ASC",
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            let focus: String = r.get(7)?;
            let beats: String = r.get(12)?;
            let tropes: String = r.get(13)?;
            Ok(ChapterMeta {
                id: r.get(0)?,
                file: r.get(1)?,
                title: r.get(2)?,
                title_src: r.get(3)?,
                summary: r.get(4)?,
                status: r.get(5)?,
                pov_lore_id: r.get(6)?,
                focus_arc_ids: serde_json::from_str(&focus).unwrap_or_default(),
                must_do: r.get(8)?,
                must_not: r.get(9)?,
                reader_knows: r.get(10)?,
                character_knows: r.get(11)?,
                beats: serde_json::from_str(&beats).unwrap_or_default(),
                trope_ids: serde_json::from_str(&tropes).unwrap_or_default(),
            })
        })
        .map_err(|e| AppError::msg(e.to_string()))?;
    for row in rows {
        project
            .chapters
            .push(row.map_err(|e| AppError::msg(e.to_string()))?);
    }
    Ok(project)
}

pub fn read_chapter_content(root: &Path, chapter_id: &str) -> AppResult<(ChapterMeta, String)> {
    let conn = open_work_db(root)?;
    conn.query_row(
        "SELECT id, file, title, title_src, summary, status, pov_lore_id,
                focus_arc_ids_json, must_do, must_not, reader_knows, character_knows,
                beats_json, trope_ids_json, content
         FROM chapters WHERE id = ?1",
        params![chapter_id],
        |r| {
            let focus: String = r.get(7)?;
            let beats: String = r.get(12)?;
            let tropes: String = r.get(13)?;
            let meta = ChapterMeta {
                id: r.get(0)?,
                file: r.get(1)?,
                title: r.get(2)?,
                title_src: r.get(3)?,
                summary: r.get(4)?,
                status: r.get(5)?,
                pov_lore_id: r.get(6)?,
                focus_arc_ids: serde_json::from_str(&focus).unwrap_or_default(),
                must_do: r.get(8)?,
                must_not: r.get(9)?,
                reader_knows: r.get(10)?,
                character_knows: r.get(11)?,
                beats: serde_json::from_str(&beats).unwrap_or_default(),
                trope_ids: serde_json::from_str(&tropes).unwrap_or_default(),
            };
            let content: String = r.get(14)?;
            Ok((meta, content))
        },
    )
    .map_err(|_| AppError::t("errors.chapterMissing"))
}

pub fn write_chapter_content(root: &Path, chapter_id: &str, content: &str) -> AppResult<()> {
    let conn = open_work_db(root)?;
    let n = conn
        .execute(
            "UPDATE chapters SET content = ?1 WHERE id = ?2",
            params![content, chapter_id],
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    if n == 0 {
        return Err(AppError::t("errors.chapterMissing"));
    }
    Ok(())
}

pub fn replace_lore(root: &Path, entries: &[LoreEntry]) -> AppResult<()> {
    let conn = open_work_db(root)?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::msg(e.to_string()))?;
    tx.execute("DELETE FROM lore", [])
        .map_err(|e| AppError::msg(e.to_string()))?;
    for e in entries {
        upsert_lore_tx(&tx, e)?;
    }
    tx.commit().map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

fn upsert_lore_tx(tx: &rusqlite::Transaction<'_>, e: &LoreEntry) -> AppResult<()> {
    tx.execute(
        "INSERT INTO lore(id, kind, title, content, keywords_json, links_json, attrs_json,
                          sources_json, unique_flag, updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
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

pub fn upsert_lore(root: &Path, e: &LoreEntry) -> AppResult<()> {
    let conn = open_work_db(root)?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::msg(e.to_string()))?;
    upsert_lore_tx(&tx, e)?;
    tx.commit().map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn delete_lore(root: &Path, lore_id: &str) -> AppResult<()> {
    let conn = open_work_db(root)?;
    conn.execute("DELETE FROM lore WHERE id = ?1", params![lore_id])
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn list_lore(root: &Path) -> AppResult<Vec<LoreEntry>> {
    let conn = open_work_db(root)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, title, content, keywords_json, links_json, attrs_json,
                    sources_json, unique_flag, updated_at FROM lore",
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| row_to_lore(r))
        .map_err(|e| AppError::msg(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| AppError::msg(e.to_string()))?);
    }
    Ok(out)
}

fn row_to_lore(r: &rusqlite::Row<'_>) -> rusqlite::Result<LoreEntry> {
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
}

pub fn save_story_doc(root: &Path, name: &str, value: &Value) -> AppResult<()> {
    let conn = open_work_db(root)?;
    conn.execute(
        "INSERT INTO story_docs(name, value_json) VALUES(?1,?2)
         ON CONFLICT(name) DO UPDATE SET value_json = excluded.value_json",
        params![name, serde_json::to_string(value)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_story_doc(root: &Path, name: &str) -> AppResult<Option<Value>> {
    if !work_sqlite_path(root).is_file() {
        return Ok(None);
    }
    let conn = open_work_db(root)?;
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

pub fn save_memory(root: &Path, mem: &MemoryStore) -> AppResult<()> {
    let conn = open_work_db(root)?;
    conn.execute(
        "INSERT INTO memory(id, value_json) VALUES(1, ?1)
         ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
        params![serde_json::to_string(mem)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_memory(root: &Path) -> AppResult<MemoryStore> {
    if !work_sqlite_path(root).is_file() {
        return Ok(MemoryStore::default());
    }
    let conn = open_work_db(root)?;
    let json: Option<String> = conn
        .query_row("SELECT value_json FROM memory WHERE id = 1", [], |r| r.get(0))
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(json
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

pub fn save_stats_json(root: &Path, value: &Value) -> AppResult<()> {
    let conn = open_work_db(root)?;
    conn.execute(
        "INSERT INTO stats(id, value_json) VALUES(1, ?1)
         ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
        params![serde_json::to_string(value)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_stats_json(root: &Path) -> AppResult<Option<Value>> {
    if !work_sqlite_path(root).is_file() {
        return Ok(None);
    }
    let conn = open_work_db(root)?;
    let json: Option<String> = conn
        .query_row("SELECT value_json FROM stats WHERE id = 1", [], |r| r.get(0))
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()))
}

pub fn save_genblocks(root: &Path, chapter_id: &str, value: &Value) -> AppResult<()> {
    let conn = open_work_db(root)?;
    conn.execute(
        "INSERT INTO genblocks(chapter_id, value_json) VALUES(?1,?2)
         ON CONFLICT(chapter_id) DO UPDATE SET value_json = excluded.value_json",
        params![chapter_id, serde_json::to_string(value)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_genblocks(root: &Path, chapter_id: &str) -> Option<Value> {
    let conn = open_work_db(root).ok()?;
    let json: String = conn
        .query_row(
            "SELECT value_json FROM genblocks WHERE chapter_id = ?1",
            params![chapter_id],
            |r| r.get(0),
        )
        .ok()?;
    serde_json::from_str(&json).ok()
}

pub fn save_beat_progress(root: &Path, chapter_id: &str, value: &Value) -> AppResult<()> {
    let conn = open_work_db(root)?;
    conn.execute(
        "INSERT INTO beat_progress(chapter_id, value_json) VALUES(?1,?2)
         ON CONFLICT(chapter_id) DO UPDATE SET value_json = excluded.value_json",
        params![chapter_id, serde_json::to_string(value)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_beat_progress(root: &Path, chapter_id: &str) -> Option<Value> {
    let conn = open_work_db(root).ok()?;
    let json: String = conn
        .query_row(
            "SELECT value_json FROM beat_progress WHERE chapter_id = ?1",
            params![chapter_id],
            |r| r.get(0),
        )
        .ok()?;
    serde_json::from_str(&json).ok()
}

pub fn save_novel_chat(root: &Path, session: &ChatSession) -> AppResult<()> {
    let conn = open_work_db(root)?;
    conn.execute(
        "INSERT INTO novel_chat(id, value_json) VALUES(1, ?1)
         ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
        params![serde_json::to_string(session)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn load_novel_chat(root: &Path) -> AppResult<Option<ChatSession>> {
    if !work_sqlite_path(root).is_file() {
        return Ok(None);
    }
    let conn = open_work_db(root)?;
    let json: Option<String> = conn
        .query_row(
            "SELECT value_json FROM novel_chat WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(json.and_then(|s| serde_json::from_str(&s).ok()))
}

pub fn append_gen_activity(root: &Path, line: &Value) -> AppResult<()> {
    let conn = open_work_db(root)?;
    let ts = line
        .get("ts")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    conn.execute(
        "INSERT INTO gen_activity(ts, line_json) VALUES(?1,?2)",
        params![ts, serde_json::to_string(line)?],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn list_gen_activity(root: &Path, limit: usize) -> AppResult<Vec<Value>> {
    if !work_sqlite_path(root).is_file() {
        return Ok(vec![]);
    }
    let conn = open_work_db(root)?;
    let mut stmt = conn
        .prepare(
            "SELECT line_json FROM gen_activity ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| AppError::msg(e.to_string()))?;
    let rows = stmt
        .query_map(params![limit as i64], |r| r.get::<_, String>(0))
        .map_err(|e| AppError::msg(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        let s = row.map_err(|e| AppError::msg(e.to_string()))?;
        if let Ok(v) = serde_json::from_str(&s) {
            out.push(v);
        }
    }
    Ok(out)
}

pub fn chapter_count(root: &Path) -> AppResult<i64> {
    let conn = open_work_db(root)?;
    conn.query_row("SELECT COUNT(*) FROM chapters", [], |r| r.get(0))
        .map_err(|e| AppError::msg(e.to_string()))
}

pub fn lore_count(root: &Path) -> AppResult<i64> {
    let conn = open_work_db(root)?;
    conn.query_row("SELECT COUNT(*) FROM lore", [], |r| r.get(0))
        .map_err(|e| AppError::msg(e.to_string()))
}

#[allow(dead_code)]
pub fn make_snippet(content: &str) -> String {
    snippet_of(content, 160)
}
