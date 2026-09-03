//! 作品目录内生成/修改履历
//! 路径：`{root}/chapters/.genlog/{chapter_id}.jsonl` + `{root}/gen_activity.jsonl`
//! 代码路径: kk_novel_ai/src-tauri/src/project_genlog.rs

use crate::error::AppResult;
use crate::genlog::GenLogEntry;
use crate::llm::TokenUsage;
use chrono::Utc;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn chapters_dir(root: &Path) -> PathBuf {
    root.join("chapters")
}

fn genlog_dir(root: &Path) -> PathBuf {
    chapters_dir(root).join(".genlog")
}

fn chapter_genlog_path(root: &Path, chapter_id: &str) -> PathBuf {
    let safe: String = chapter_id
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    genlog_dir(root).join(format!("{safe}.jsonl"))
}

fn project_activity_path(root: &Path) -> PathBuf {
    root.join("gen_activity.jsonl")
}

fn is_novel_project(root: &Path) -> bool {
    root.join("project.json").exists()
}

fn append_line(path: &Path, line: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// 将一条履历写入作品目录（章级文件 + 作品索引）。无 project.json 则静默跳过（旧/无效路径）。
/// `chapter_id` 为空时仍写 `gen_activity.jsonl`，章级落到 `_project.jsonl`。
pub fn append_entry(entry: &GenLogEntry) -> AppResult<()> {
    let root_s = entry.project_root.trim();
    if root_s.is_empty() {
        return Ok(());
    }
    let root = Path::new(root_s);
    if !is_novel_project(root) {
        return Ok(());
    }
    let chapter_id = entry.chapter_id.trim();
    let chapter_key = if chapter_id.is_empty() {
        "_project"
    } else {
        chapter_id
    };
    let line = serde_json::to_string(entry)?;
    append_line(&chapter_genlog_path(root, chapter_key), &line)?;
    append_line(&project_activity_path(root), &line)?;
    Ok(())
}

fn read_jsonl(path: &Path) -> Vec<GenLogEntry> {
    if !path.exists() {
        return vec![];
    }
    let Ok(text) = fs::read_to_string(path) else {
        return vec![];
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// 列出作品内履历；无文件返回空（旧项目可忽略）。
pub fn list_entries(root: &Path, limit: usize) -> AppResult<Vec<GenLogEntry>> {
    if !is_novel_project(root) {
        return Ok(vec![]);
    }
    let index = project_activity_path(root);
    let mut items = if index.exists() {
        read_jsonl(&index)
    } else {
        let dir = genlog_dir(root);
        let mut all = Vec::new();
        if dir.is_dir() {
            if let Ok(rd) = fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                        all.extend(read_jsonl(&p));
                    }
                }
            }
        }
        all
    };
    items.sort_by(|a, b| a.ts.cmp(&b.ts));
    if items.len() > limit {
        items = items.split_off(items.len() - limit);
    }
    Ok(items)
}

pub fn list_as_json(root: &Path, limit: usize) -> AppResult<serde_json::Value> {
    Ok(json!({
        "ok": true,
        "items": list_entries(root, limit)?,
        "source": "project",
    }))
}

fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 章节保存（手动或定稿写盘）轻量履历
pub fn record_chapter_save(
    root: &Path,
    chapter_id: &str,
    old_content: &str,
    new_content: &str,
    source: &str,
) -> AppResult<()> {
    if !is_novel_project(root) {
        return Ok(());
    }
    if old_content == new_content {
        return Ok(());
    }
    let chars_final = new_content.chars().count();
    let chars_raw = old_content.chars().count();
    let delta = chars_final as i64 - chars_raw as i64;
    let usage = TokenUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
        prompt_cache_hit_tokens: 0,
        prompt_cache_miss_tokens: 0,
        source: "estimate".into(),
    };
    let entry = GenLogEntry {
        id: Uuid::new_v4().to_string(),
        ts: Utc::now().to_rfc3339(),
        task: "chapter_save".into(),
        event: "chapter_save".into(),
        project_root: root.to_string_lossy().to_string(),
        chapter_id: chapter_id.to_string(),
        preview: take_chars(new_content, 200),
        source: source.to_string(),
        raw_text: String::new(),
        final_text: take_chars(new_content, 8_000),
        truncated: new_content.chars().count() > 8_000,
        model_used: String::new(),
        instruction: format!("chars_delta={delta}"),
        messages: vec![],
        usage: Some(usage),
        cost_cny: 0.0,
        chars_raw,
        chars_final,
        context_sources: Some(json!({
            "items": [{
                "kind": "chapter_save",
                "id": "",
                "title": "章节保存",
                "detail": format!("Δ字数 {delta}")
            }]
        })),
    };
    append_entry(&entry)
}
