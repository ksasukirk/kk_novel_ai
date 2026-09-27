//! 按正文强制重建章纲 + 单 gen 块（生成指令 / digest）
//! 代码路径: kk_novel_ai/src-tauri/src/import/rebuild_structure.rs

use super::strip_json_fence;
use crate::error::{AppError, AppResult};
use crate::genlog;
use crate::llm::{ChatMessage, ChatOptions, LmStudioClient, TokenUsage};
use crate::project::{self, ChapterMetaPatch};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

const MIN_PROSE_CHARS: usize = 80;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RebuildStructureReport {
    pub ok: bool,
    pub root: String,
    pub from: usize,
    pub to: usize,
    pub rebuilt: usize,
    pub skipped: usize,
    pub cancelled: bool,
    #[serde(default)]
    pub failed: Vec<String>,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub calls: u64,
    #[serde(default)]
    pub cost_cny: f64,
    #[serde(default)]
    pub model_used: String,
}

#[derive(Debug, Clone, Default)]
struct RebuildFields {
    summary: String,
    instruction: String,
    digest: String,
}

fn count_non_ws(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

/// 过长正文：头 + 尾窗口，避免超上下文
pub(crate) fn window_body(text: &str, window_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let win = window_chars.max(2000);
    if n <= win {
        return text.to_string();
    }
    let half = win / 2;
    let head: String = chars[..half].iter().collect();
    let tail: String = chars[n - half..].iter().collect();
    format!("{head}\n\n……\n\n{tail}")
}

pub(crate) fn parse_rebuild_json(raw: &str) -> AppResult<RebuildFields> {
    let cleaned = strip_json_fence(raw);
    let v: Value = serde_json::from_str(&cleaned).map_err(|e| {
        AppError::msg(format!("rebuild_chapter_from_body JSON: {e}"))
    })?;
    let summary = v
        .get("summary")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let instruction = v
        .get("instruction")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let digest = v
        .get("digest")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if summary.is_empty() && instruction.is_empty() && digest.is_empty() {
        return Err(AppError::msg("rebuild_chapter_from_body: empty fields"));
    }
    Ok(RebuildFields {
        summary,
        instruction,
        digest,
    })
}

pub(crate) fn build_single_gen_sidecar(
    body: &str,
    instruction: &str,
    digest: &str,
    model: &str,
) -> (Value, String) {
    let block_key = format!("g{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);
    let variant_id = Uuid::new_v4().to_string();
    let node_id = Uuid::new_v4().to_string();
    let chars = body.chars().count();
    let ts = chrono_like_now();
    let sidecar = json!({
        "format": 2,
        "plains": [],
        "nodes": [{
            "id": node_id,
            "parentId": "",
            "fromVariantId": "",
            "activeVariantId": variant_id,
            "trailingPlains": [],
            "variants": [{
                "id": variant_id,
                "key": block_key,
                "label": "",
                "text": body,
                "instruction": instruction,
                "task": "rebuild_from_body",
                "digest": digest,
                "meta": {
                    "id": "",
                    "ts": ts,
                    "model": model,
                    "chars": chars,
                    "tokens": null,
                    "cost": null,
                    "usageSource": "",
                    "sources": []
                }
            }]
        }]
    });
    (sidecar, block_key)
}

fn chrono_like_now() -> String {
    // ISO-ish local; 与前端 new Date().toISOString() 不必严格一致
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn progress_json(
    root: &Path,
    current: usize,
    total: usize,
    title: &str,
    rebuilt: usize,
    skipped: usize,
    failed: usize,
    usage: &TokenUsage,
    calls: u64,
    cost: f64,
    model: &str,
) -> Value {
    let pct = if total == 0 {
        100u32
    } else {
        ((current.min(total) as f64 / total as f64) * 100.0).round() as u32
    };
    json!({
        "phase": "structure",
        "root": root.to_string_lossy(),
        "current": current,
        "total": total,
        "title": title,
        "rebuilt": rebuilt,
        "skipped": skipped,
        "failed": failed,
        "pct": pct,
        "tokens": usage.total_tokens,
        "prompt_tokens": usage.prompt_tokens,
        "completion_tokens": usage.completion_tokens,
        "cache_hit": usage.prompt_cache_hit_tokens,
        "cache_miss": usage.prompt_cache_miss_tokens,
        "usage_source": usage.source,
        "calls": calls,
        "cost_cny": cost,
        "model_used": model,
    })
}

/// 按章范围强制根据正文重写章纲 + 单 gen 块指令/摘要
pub async fn rebuild_structure_from_prose(
    root: &Path,
    from: usize,
    to: usize,
    cancel: Arc<AtomicBool>,
    mut on_progress: impl FnMut(Value),
) -> AppResult<RebuildStructureReport> {
    if from == 0 {
        return Err(AppError::t("errors.fromToMustBeChapterIndex"));
    }
    let opened = project::open_project(root)?;
    if opened.project.kind != "novel" {
        return Ok(RebuildStructureReport {
            ok: true,
            root: root.to_string_lossy().to_string(),
            from,
            to: 0,
            skipped: 0,
            ..Default::default()
        });
    }
    let total_chapters = opened.project.chapters.len();
    if total_chapters == 0 {
        return Ok(RebuildStructureReport {
            ok: true,
            root: root.to_string_lossy().to_string(),
            from,
            to: 0,
            ..Default::default()
        });
    }
    if from > total_chapters {
        return Err(AppError::t_fmt(
            "errors.fromExceedsChapters",
            &[
                ("from", &from.to_string()),
                ("total", &total_chapters.to_string()),
            ],
        ));
    }
    let to = if to == 0 {
        total_chapters
    } else {
        to.min(total_chapters)
    };
    if to < from {
        return Err(AppError::t("errors.fromToMustBeChapterIndex"));
    }

    let settings = crate::settings::load_settings()?;
    let client = LmStudioClient::from_settings(&settings);
    let model = settings.resolve_analysis_model().to_string();
    let window = settings.recent_window_chars.max(1500).saturating_mul(3);
    let slice = &opened.project.chapters[(from - 1)..to];
    let n = slice.len();

    let mut report = RebuildStructureReport {
        ok: true,
        root: root.to_string_lossy().to_string(),
        from,
        to,
        ..Default::default()
    };
    let mut acc_usage = TokenUsage::default();
    let mut acc_calls = 0u64;
    let mut acc_cost = 0.0f64;
    let mut last_model = model.clone();

    on_progress(progress_json(
        root,
        0,
        n,
        "",
        0,
        0,
        0,
        &acc_usage,
        acc_calls,
        acc_cost,
        &last_model,
    ));

    for (i, ch) in slice.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let title = ch.title.clone();
        on_progress(progress_json(
            root,
            i + 1,
            n,
            &title,
            report.rebuilt,
            report.skipped,
            report.failed.len(),
            &acc_usage,
            acc_calls,
            acc_cost,
            &last_model,
        ));

        let body = match project::read_chapter(root, &ch.id) {
            Ok((_, content)) => content,
            Err(e) => {
                report.failed.push(format!("{}: read {e}", ch.title));
                continue;
            }
        };
        if count_non_ws(&body) < MIN_PROSE_CHARS {
            report.skipped += 1;
            continue;
        }

        let clipped = window_body(&body, window);
        let tpl = crate::prompt_i18n::prompt("rebuild_chapter_from_body.md");
        if tpl.is_empty() {
            return Err(AppError::msg("missing prompt rebuild_chapter_from_body.md"));
        }
        let user = tpl
            .replace("{{title}}", &title)
            .replace("{{body}}", &clipped);
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: user.clone(),
            },
            ChatMessage {
                role: "user".into(),
                content: format!("章节：{title}\n请输出 JSON。"),
            },
        ];
        let max_tokens = 1200u32;
        let options = ChatOptions {
            model: Some(model.clone()),
            temperature: Some(settings.analysis_temperature.unwrap_or(0.3)),
            max_tokens: Some(max_tokens),
            stream: false,
            ..Default::default()
        };

        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }

        let chat_out = match client.chat(&settings, &messages, &options).await {
            Ok(r) => r,
            Err(e) => {
                report.failed.push(format!("{}: llm {e}", ch.title));
                continue;
            }
        };
        acc_usage.saturating_add_assign(&chat_out.usage);
        acc_calls += 1;
        last_model = model.clone();

        let fields = match parse_rebuild_json(&chat_out.text) {
            Ok(f) => f,
            Err(e) => {
                report.failed.push(format!("{}: parse {e}", ch.title));
                let _ = genlog::record_llm_call(
                    "rebuild_structure_from_prose",
                    &root.to_string_lossy(),
                    &ch.id,
                    &chat_out.text,
                    "",
                    "rebuild_structure",
                    false,
                    &last_model,
                    "parse_fail",
                    &messages,
                    Some(chat_out.usage.clone()),
                    &settings,
                );
                continue;
            }
        };

        let summary = if fields.summary.trim().is_empty() {
            fields.digest.clone()
        } else {
            fields.summary.clone()
        };
        let digest = project::sanitize_block_digest(&fields.digest);
        let instruction = fields.instruction.clone();

        if let Err(e) = project::update_chapter_meta(
            root,
            &ch.id,
            ChapterMetaPatch {
                summary: Some(summary.clone()),
                ..Default::default()
            },
        ) {
            report.failed.push(format!("{}: summary {e}", ch.title));
            continue;
        }

        let (sidecar, block_key) =
            build_single_gen_sidecar(&body, &instruction, &digest, &last_model);
        if let Err(e) = project::write_genblocks(root, &ch.id, &sidecar) {
            report.failed.push(format!("{}: genblocks {e}", ch.title));
            continue;
        }
        let _ = project::sync_chapter_block_notes_from_blocks(root, &ch.id, &sidecar);
        if !digest.is_empty() {
            let _ = project::append_block_note(root, &ch.id, &block_key, &digest);
        }
        let snap = if !summary.trim().is_empty() {
            summary.as_str()
        } else {
            digest.as_str()
        };
        if !snap.trim().is_empty() {
            let _ = project::upsert_chapter_snapshot(root, &ch.id, snap);
        }

        let entry = genlog::make_entry_full(
            "rebuild_structure_from_prose",
            &root.to_string_lossy(),
            &ch.id,
            &chat_out.text,
            &summary,
            "rebuild_structure",
            false,
            &last_model,
            "rebuild_from_body",
            &messages,
            Some(chat_out.usage.clone()),
            &settings,
        );
        acc_cost += entry.cost_cny;
        let _ = genlog::append_log(&entry);

        report.rebuilt += 1;
        on_progress(progress_json(
            root,
            i + 1,
            n,
            &title,
            report.rebuilt,
            report.skipped,
            report.failed.len(),
            &acc_usage,
            acc_calls,
            acc_cost,
            &last_model,
        ));
    }

    report.total_tokens = u64::from(acc_usage.total_tokens);
    report.prompt_tokens = u64::from(acc_usage.prompt_tokens);
    report.completion_tokens = u64::from(acc_usage.completion_tokens);
    report.calls = acc_calls;
    report.cost_cny = acc_cost;
    report.model_used = last_model;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rebuild_json_ok() {
        let raw = r#"{"summary":"纲A","instruction":"写B","digest":"摘C"}"#;
        let f = parse_rebuild_json(raw).unwrap();
        assert_eq!(f.summary, "纲A");
        assert_eq!(f.instruction, "写B");
        assert_eq!(f.digest, "摘C");
    }

    #[test]
    fn parse_rebuild_json_fence() {
        let raw = "```json\n{\"summary\":\"s\",\"instruction\":\"i\",\"digest\":\"d\"}\n```";
        let f = parse_rebuild_json(raw).unwrap();
        assert_eq!(f.summary, "s");
    }

    #[test]
    fn window_body_keeps_short() {
        assert_eq!(window_body("短正文", 100), "短正文");
    }

    #[test]
    fn window_body_clips_long() {
        let long = "甲".repeat(5000);
        let out = window_body(&long, 1000);
        assert!(out.contains("……"));
        assert!(out.chars().count() < 5000);
    }

    #[test]
    fn sidecar_has_one_gen_matching_body() {
        let body = "第一章正文内容足够长了。";
        let (v, key) = build_single_gen_sidecar(body, "指令", "摘要", "m");
        assert_eq!(v["format"], 2);
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        let variants = nodes[0]["variants"].as_array().unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0]["text"], body);
        assert_eq!(variants[0]["key"], key);
        assert_eq!(variants[0]["instruction"], "指令");
        assert_eq!(variants[0]["digest"], "摘要");
        assert_eq!(variants[0]["task"], "rebuild_from_body");
    }
}
