//! 生成日志
//! 代码路径: kk_novel_ai/src-tauri/src/genlog.rs

use crate::error::AppResult;
use crate::llm::{ChatMessage, TokenUsage};
use crate::paths::gen_log_path;
use crate::settings::AppSettings;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::OpenOptions;
use std::io::Write;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenLogEntry {
    #[serde(default)]
    pub id: String,
    pub ts: String,
    pub task: String,
    pub project_root: String,
    pub chapter_id: String,
    /// 定稿预览（前 200 字，兼容旧 UI）
    pub preview: String,
    pub source: String,
    /// 模型原始全文（可较长）
    #[serde(default)]
    pub raw_text: String,
    /// 截断后定稿全文
    #[serde(default)]
    pub final_text: String,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub model_used: String,
    #[serde(default)]
    pub instruction: String,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub usage: Option<TokenUsage>,
    #[serde(default)]
    pub cost_cny: f64,
    #[serde(default)]
    pub chars_raw: usize,
    #[serde(default)]
    pub chars_final: usize,
    /// 本次注入的设定/章纲来源（可选；旧日志无此字段）
    #[serde(default)]
    pub context_sources: Option<serde_json::Value>,
}

fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn cap_messages(messages: &[ChatMessage], per_msg_cap: usize, total_cap: usize) -> Vec<ChatMessage> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for m in messages {
        if used >= total_cap {
            break;
        }
        let content = take_chars(&m.content, per_msg_cap.min(total_cap.saturating_sub(used)));
        used += content.chars().count();
        out.push(ChatMessage {
            role: m.role.clone(),
            content,
        });
    }
    out
}

pub fn append_log(entry: &GenLogEntry) -> AppResult<()> {
    let path = gen_log_path()?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(entry)?)?;
    let _ = crate::usage::record_entry(entry);
    Ok(())
}

pub fn read_recent(limit: usize) -> AppResult<Vec<GenLogEntry>> {
    let path = gen_log_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(path)?;
    let mut items: Vec<GenLogEntry> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if items.len() > limit {
        items = items.split_off(items.len() - limit);
    }
    Ok(items)
}

pub fn make_entry(
    task: &str,
    project_root: &str,
    chapter_id: &str,
    text: &str,
    source: &str,
) -> GenLogEntry {
    make_entry_full(
        task,
        project_root,
        chapter_id,
        text,
        text,
        source,
        false,
        "",
        "",
        &[],
        None,
        &AppSettings::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn make_entry_full(
    task: &str,
    project_root: &str,
    chapter_id: &str,
    raw_text: &str,
    final_text: &str,
    source: &str,
    truncated: bool,
    model_used: &str,
    instruction: &str,
    messages: &[ChatMessage],
    usage: Option<TokenUsage>,
    settings: &AppSettings,
) -> GenLogEntry {
    const CAP: usize = 80_000;
    let usage = usage.unwrap_or_else(|| {
        TokenUsage::estimate_from_messages(messages, final_text)
    });
    let cost_cny = crate::usage::calc_cost_cny(&usage, settings);
    GenLogEntry {
        id: Uuid::new_v4().to_string(),
        ts: Utc::now().to_rfc3339(),
        task: task.to_string(),
        project_root: project_root.to_string(),
        chapter_id: chapter_id.to_string(),
        preview: take_chars(final_text, 200),
        source: source.to_string(),
        raw_text: take_chars(raw_text, CAP),
        final_text: take_chars(final_text, CAP),
        truncated,
        model_used: model_used.to_string(),
        instruction: take_chars(instruction, 4000),
        messages: cap_messages(messages, 20_000, 60_000),
        usage: Some(usage),
        cost_cny,
        chars_raw: raw_text.chars().count(),
        chars_final: final_text.chars().count(),
        context_sources: None,
    }
}

pub fn list_as_json(limit: usize) -> AppResult<serde_json::Value> {
    Ok(json!({ "ok": true, "items": read_recent(limit)? }))
}
