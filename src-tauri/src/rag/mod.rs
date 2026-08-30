//! Embedding RAG（作品内 embeddings.sqlite，blob + cosine）
//! 代码路径: kk_novel_ai/src-tauri/src/rag/mod.rs

use crate::error::{AppError, AppResult};
use crate::llm::LmStudioClient;
use crate::project::{self, LoreEntry};
use crate::settings::AppSettings;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;

const TABLE: &str = "embeddings";

fn db_path(root: &Path) -> std::path::PathBuf {
    root.join("embeddings.sqlite")
}

fn open_db(root: &Path) -> AppResult<Connection> {
    let path = db_path(root);
    let conn = Connection::open(&path).map_err(|e| AppError::msg(format!("打开 embeddings.sqlite 失败: {e}")))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS embeddings (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            text TEXT NOT NULL,
            dim INTEGER NOT NULL,
            vector BLOB NOT NULL,
            updated_at TEXT NOT NULL
        );",
    )
    .map_err(|e| AppError::msg(format!("初始化 embeddings 表失败: {e}")))?;
    Ok(conn)
}

fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn blob_to_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0f32;
    let mut na = 0f32;
    let mut nb = 0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        dot / denom
    }
}

fn lore_index_text(e: &LoreEntry) -> String {
    let attrs: Vec<String> = e.attrs.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    format!(
        "{} {} {} {} {}",
        e.title,
        e.kind,
        e.keywords.join(" "),
        attrs.join(" "),
        e.content
    )
}

fn upsert_row(conn: &Connection, id: &str, kind: &str, text: &str, vector: &[f32]) -> AppResult<()> {
    let blob = vec_to_blob(vector);
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        &format!(
            "INSERT INTO {TABLE} (id, kind, text, dim, vector, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               kind=excluded.kind, text=excluded.text, dim=excluded.dim,
               vector=excluded.vector, updated_at=excluded.updated_at"
        ),
        params![id, kind, text, vector.len() as i64, blob, now],
    )
    .map_err(|e| AppError::msg(format!("写入 embedding 失败: {e}")))?;
    Ok(())
}

pub async fn embed_text(
    client: &LmStudioClient,
    settings: &AppSettings,
    texts: &[String],
) -> AppResult<Vec<Vec<f32>>> {
    let model = settings
        .resolve_embedding_model()
        .ok_or_else(|| AppError::msg("未配置 embedding_model"))?;
    client.embed(settings, model, texts).await
}

pub async fn upsert_lore(
    client: &LmStudioClient,
    settings: &AppSettings,
    root: &Path,
    entry: &LoreEntry,
) -> AppResult<()> {
    if settings.resolve_embedding_model().is_none() {
        return Ok(());
    }
    let text = lore_index_text(entry);
    let vectors = embed_text(client, settings, &[text.clone()]).await?;
    let Some(vec) = vectors.into_iter().next() else {
        return Ok(());
    };
    let conn = open_db(root)?;
    upsert_row(&conn, &format!("lore:{}", entry.id), "lore", &text, &vec)?;
    Ok(())
}

pub async fn upsert_chapter_tail(
    client: &LmStudioClient,
    settings: &AppSettings,
    root: &Path,
    chapter_id: &str,
    content: &str,
) -> AppResult<()> {
    if settings.resolve_embedding_model().is_none() {
        return Ok(());
    }
    let chars: Vec<char> = content.chars().collect();
    let take = chars.len().min(2000);
    let tail: String = chars[chars.len().saturating_sub(take)..].iter().collect();
    if tail.trim().is_empty() {
        return Ok(());
    }
    let vectors = embed_text(client, settings, &[tail.clone()]).await?;
    let Some(vec) = vectors.into_iter().next() else {
        return Ok(());
    };
    let conn = open_db(root)?;
    upsert_row(
        &conn,
        &format!("chapter:{}", chapter_id),
        "chapter",
        &tail,
        &vec,
    )?;
    Ok(())
}

/// 查询与 query 相似的 lore id → cosine；无 embedding 模型时返回空 map
pub async fn query_semantic_scores(
    client: &LmStudioClient,
    settings: &AppSettings,
    root: &Path,
    query: &str,
) -> AppResult<HashMap<String, f32>> {
    let mut out = HashMap::new();
    if settings.resolve_embedding_model().is_none() {
        return Ok(out);
    }
    if !db_path(root).exists() {
        return Ok(out);
    }
    let q = query.trim();
    if q.is_empty() {
        return Ok(out);
    }
    let vectors = embed_text(client, settings, &[q.to_string()]).await?;
    let Some(qvec) = vectors.into_iter().next() else {
        return Ok(out);
    };
    let conn = open_db(root)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id, vector FROM {TABLE} WHERE kind = 'lore'"
        ))
        .map_err(|e| AppError::msg(format!("查询 embedding 失败: {e}")))?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((id, blob))
        })
        .map_err(|e| AppError::msg(format!("读取 embedding 失败: {e}")))?;
    for row in rows.flatten() {
        let (id, blob) = row;
        let lore_id = id.strip_prefix("lore:").unwrap_or(&id).to_string();
        let vec = blob_to_vec(&blob);
        let score = cosine(&qvec, &vec);
        if score > 0.15 {
            out.insert(lore_id, score);
        }
    }
    Ok(out)
}

pub async fn rebuild_index(
    client: &LmStudioClient,
    settings: &AppSettings,
    root: &Path,
) -> AppResult<usize> {
    if settings.resolve_embedding_model().is_none() {
        return Err(AppError::msg("请先在设置中配置 embedding_model"));
    }
    let path = db_path(root);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let mut count = 0usize;
    let lore = project::list_lore(root)?;
    for entry in &lore {
        upsert_lore(client, settings, root, entry).await?;
        count += 1;
    }
    let opened = project::open_project(root)?;
    for ch in &opened.project.chapters {
        let (_, content) = project::read_chapter(root, &ch.id)?;
        upsert_chapter_tail(client, settings, root, &ch.id, &content).await?;
        count += 1;
    }
    Ok(count)
}

/// 后台尽力索引（失败静默）
pub fn spawn_index_lore(root: String, entry: LoreEntry) {
    tauri::async_runtime::spawn(async move {
        let Ok(settings) = crate::settings::load_settings() else {
            return;
        };
        if settings.resolve_embedding_model().is_none() {
            return;
        }
        let client = LmStudioClient::new();
        let _ = upsert_lore(&client, &settings, Path::new(&root), &entry).await;
    });
}

pub fn spawn_index_chapter(root: String, chapter_id: String, content: String) {
    tauri::async_runtime::spawn(async move {
        let Ok(settings) = crate::settings::load_settings() else {
            return;
        };
        if settings.resolve_embedding_model().is_none() {
            return;
        }
        let client = LmStudioClient::new();
        let _ = upsert_chapter_tail(&client, &settings, Path::new(&root), &chapter_id, &content).await;
    });
}
