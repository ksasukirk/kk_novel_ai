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
    /// 模型给的短标题（不含「第N章」）；空表示不改名
    chapter_title: String,
}

fn count_non_ws(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

/// 导入无章标时的占位：第1段（约1–3323字）
pub(crate) fn is_segment_placeholder_title(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() {
        return true;
    }
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"^第\s*[0-9一二三四五六七八九十百千零〇两]+\s*段").expect("segment title")
    });
    re.is_match(t)
}

/// 已有「第N章」但名称空，或整段是占位 → 需要生成章名
pub(crate) fn needs_generated_chapter_title(title: &str) -> bool {
    let t = title.trim();
    if is_segment_placeholder_title(t) {
        return true;
    }
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"^第\s*[0-9一二三四五六七八九十百千零〇两]+\s*章\s*[：:\-]?\s*(.*)$")
            .expect("chapter title")
    });
    if let Some(c) = re.captures(t) {
        let rest = c.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        return rest.is_empty();
    }
    false
}

pub(crate) fn chapter_index_label(index: usize) -> String {
    const CN: [&str; 10] = ["一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];
    if index >= 1 && index <= 10 {
        format!("第{}章", CN[index - 1])
    } else if index > 0 {
        format!("第{index}章")
    } else {
        "第一章".into()
    }
}

/// 把模型短名与序号拼成「第一章 短名」；短名里若已带第N章则剥掉重拼
pub(crate) fn compose_chapter_title(index: usize, raw_name: &str) -> String {
    let label = chapter_index_label(index);
    let mut name = raw_name.trim().trim_start_matches('#').trim().to_string();
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"^第\s*[0-9一二三四五六七八九十百千零〇两]+\s*章\s*[：:\-]?\s*")
            .expect("strip chapter num")
    });
    name = re.replace(&name, "").trim().to_string();
    // 去掉误带的「第N段…」
    static SEG: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let seg = SEG.get_or_init(|| {
        regex::Regex::new(r"^第\s*[0-9一二三四五六七八九十百千零〇两]+\s*段\s*[（(]?.*$")
            .expect("strip segment")
    });
    if seg.is_match(&name) {
        name.clear();
    }
    // 过长截断，避免目录爆炸
    if name.chars().count() > 24 {
        name = name.chars().take(24).collect();
    }
    if name.is_empty() {
        label
    } else {
        format!("{label} {name}")
    }
}

/// 用新标题替换正文首行 Markdown 标题（或插入）
pub(crate) fn ensure_body_title_line(body: &str, title: &str) -> String {
    let title = title.trim();
    let trimmed = body.trim_start_matches('\u{feff}').trim();
    if title.is_empty() {
        return if trimmed.is_empty() {
            String::new()
        } else {
            format!("{trimmed}\n")
        };
    }
    if trimmed.is_empty() {
        return format!("# {title}\n");
    }
    let mut lines = trimmed.lines();
    if let Some(first) = lines.next() {
        let f = first.trim();
        if f.starts_with('#') {
            let rest: String = lines.collect::<Vec<_>>().join("\n");
            let rest = rest.trim_start_matches('\n').trim_start();
            if rest.is_empty() {
                return format!("# {title}\n");
            }
            return format!("# {title}\n\n{rest}\n");
        }
    }
    format!("# {title}\n\n{trimmed}\n")
}

fn title_rules_for_prompt(need: bool) -> &'static str {
    if need {
        "- `chapter_title`：**必填**。根据正文概括的本章短标题（**不要**写「第N章」「第N段」编号，约 2～12 字）；系统会拼成「第一章 xxx」并写入章标题与正文首行"
    } else {
        "- `chapter_title`：留空字符串（当前已有正式章名，不要改）"
    }
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

/// 半句截断时回退到最近完整句/分句，避免章纲停在「从背后」这类半截。
pub(crate) fn polish_field_ending(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() || !crate::writing::continuity::prose_incomplete(t) {
        return t.to_string();
    }
    let chars: Vec<char> = t.chars().collect();
    for i in (0..chars.len()).rev() {
        if matches!(chars[i], '。' | '！' | '？' | '.' | '!' | '?') {
            return chars[..=i].iter().collect();
        }
    }
    // 从后往前找分句；优先够长的，没有长的也接受较短完整分句
    let mut best: Option<(usize, String)> = None;
    for i in (0..chars.len()).rev() {
        if !matches!(chars[i], '，' | '；' | '、' | ',' | ';' | '\n') {
            continue;
        }
        let head: String = chars[..i].iter().collect();
        let head = head.trim_end().to_string();
        let n = head.chars().count();
        if n < 8 {
            continue;
        }
        let candidate = format!("{head}。");
        match &best {
            Some((bn, _)) if *bn >= n => {}
            _ => best = Some((n, candidate)),
        }
        if n >= 40 {
            break;
        }
    }
    best.map(|(_, s)| s).unwrap_or_else(|| t.to_string())
}

fn field_looks_cut(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && crate::writing::continuity::prose_incomplete(t)
}

pub(crate) fn parse_rebuild_json(raw: &str) -> AppResult<RebuildFields> {
    let cleaned = strip_json_fence(raw);
    let v: Value = serde_json::from_str(&cleaned).map_err(|e| {
        AppError::msg(format!("rebuild_chapter_from_body JSON: {e}"))
    })?;
    let summary = polish_field_ending(
        v.get("summary")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim(),
    );
    let instruction = polish_field_ending(
        v.get("instruction")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim(),
    );
    let digest = polish_field_ending(
        v.get("digest")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim(),
    );
    let chapter_title = v
        .get("chapter_title")
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
        chapter_title,
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
        let chapter_index = from + i;
        let need_title = needs_generated_chapter_title(&title);
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
            .replace("{{body}}", &clipped)
            .replace("{{chapter_index}}", &chapter_index.to_string())
            .replace("{{title_rules}}", title_rules_for_prompt(need_title));
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: user.clone(),
            },
            ChatMessage {
                role: "user".into(),
                content: if need_title {
                    format!(
                        "章节序号：{chapter_index}\n当前标题：{title}\n请输出 JSON（须含 chapter_title 短标题）。"
                    )
                } else {
                    format!("章节：{title}\n请输出 JSON。")
                },
            },
        ];
        // summary+instruction+digest 中文合计可达 ~1200 字，1200 tokens 易半句截断
        let max_tokens = 2800u32;
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

        let mut chat_out = match client.chat(&settings, &messages, &options).await {
            Ok(r) => r,
            Err(e) => {
                report.failed.push(format!("{}: llm {e}", ch.title));
                continue;
            }
        };
        acc_usage.saturating_add_assign(&chat_out.usage);
        acc_calls += 1;
        last_model = model.clone();

        let mut fields = match parse_rebuild_json(&chat_out.text) {
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

        // 章纲仍半句收束时重试一次（抬温度略降、强调完整句）
        if field_looks_cut(&fields.summary) && !cancel.load(Ordering::Relaxed) {
            let mut retry_msgs = messages.clone();
            retry_msgs.push(ChatMessage {
                role: "assistant".into(),
                content: chat_out.text.clone(),
            });
            retry_msgs.push(ChatMessage {
                role: "user".into(),
                content: "上一版 summary 停在半句。请重新输出完整 JSON：summary/instruction/digest 都必须用完整句收束，覆盖正文头尾关键事件，禁止半截停笔。".into(),
            });
            let retry_opts = ChatOptions {
                model: Some(model.clone()),
                temperature: Some(settings.analysis_temperature.unwrap_or(0.3).min(0.25)),
                max_tokens: Some(3200),
                stream: false,
                ..Default::default()
            };
            if let Ok(retry_out) = client.chat(&settings, &retry_msgs, &retry_opts).await {
                acc_usage.saturating_add_assign(&retry_out.usage);
                acc_calls += 1;
                if let Ok(f2) = parse_rebuild_json(&retry_out.text) {
                    if !field_looks_cut(&f2.summary)
                        || f2.summary.chars().count() > fields.summary.chars().count()
                    {
                        fields = f2;
                        chat_out = retry_out;
                    }
                }
            }
        }

        let summary = if fields.summary.trim().is_empty() {
            fields.digest.clone()
        } else {
            fields.summary.clone()
        };
        let digest = project::sanitize_block_digest(&fields.digest);
        let instruction = fields.instruction.clone();

        let mut new_title = title.clone();
        let mut body_out = body.clone();
        if need_title {
            let name = if fields.chapter_title.trim().is_empty() {
                // 模型漏给时用摘要首句凑短名，保证仍有「第N章」
                summary
                    .chars()
                    .take(12)
                    .collect::<String>()
                    .trim_matches(|c: char| c.is_whitespace() || "。！？，、；：".contains(c))
                    .to_string()
            } else {
                fields.chapter_title.clone()
            };
            new_title = compose_chapter_title(chapter_index, &name);
            body_out = ensure_body_title_line(&body, &new_title);
            if let Err(e) = project::write_chapter(root, &ch.id, &body_out) {
                report.failed.push(format!("{}: write body title {e}", ch.title));
                continue;
            }
            // 保留导入占位到 title_src，便于对照
            if let Ok(mut opened) = project::open_project(root) {
                if let Some(meta) = opened
                    .project
                    .chapters
                    .iter_mut()
                    .find(|c| c.id == ch.id)
                {
                    if meta.title_src.trim().is_empty() && is_segment_placeholder_title(&title) {
                        meta.title_src = title.clone();
                    }
                }
                let _ = project::save_project_meta(root, &opened.project);
            }
        }

        if let Err(e) = project::update_chapter_meta(
            root,
            &ch.id,
            ChapterMetaPatch {
                title: if need_title {
                    Some(new_title.clone())
                } else {
                    None
                },
                summary: Some(summary.clone()),
                ..Default::default()
            },
        ) {
            report.failed.push(format!("{}: summary {e}", ch.title));
            continue;
        }

        let (sidecar, block_key) =
            build_single_gen_sidecar(&body_out, &instruction, &digest, &last_model);
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
    // write_chapter 改首行标题会标 dirty；总结管线内重建完后按新正文重新盖章
    if report.rebuilt > 0 && !report.cancelled {
        let _ = project::stamp_trope_summary(root);
    }
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
    fn segment_placeholder_detected() {
        assert!(is_segment_placeholder_title("第1段（约1–3323字）"));
        assert!(is_segment_placeholder_title("第十二段"));
        assert!(needs_generated_chapter_title("第3段（约900–1200字）"));
        assert!(needs_generated_chapter_title("第一章"));
        assert!(!needs_generated_chapter_title("第一章 寻仙"));
        assert!(!needs_generated_chapter_title("第2章 后续"));
    }

    #[test]
    fn compose_title_strips_and_numbers() {
        assert_eq!(compose_chapter_title(1, "班主任的秘密"), "第一章 班主任的秘密");
        assert_eq!(compose_chapter_title(2, "第2章 放学后"), "第二章 放学后");
        assert_eq!(compose_chapter_title(11, "尾声"), "第11章 尾声");
    }

    #[test]
    fn body_title_line_replaces_heading() {
        let body = "# 第1段（约1–10字）\n\n正文开始。\n";
        let out = ensure_body_title_line(body, "第一章 开端");
        assert!(out.starts_with("# 第一章 开端\n"), "{out}");
        assert!(out.contains("正文开始。"), "{out}");
    }

    #[test]
    fn polish_cuts_mid_clause_at_comma() {
        let s = "早饭后乐乐来电约自习，午睡后跳绳，回家后乐乐从背后";
        let out = polish_field_ending(s);
        assert!(out.ends_with('。'), "{out}");
        assert!(!out.contains("从背后"), "{out}");
        assert!(out.contains("跳绳"), "{out}");
    }

    #[test]
    fn polish_keeps_complete_sentence() {
        let s = "早饭后乐乐来电约自习，午睡后跳绳。";
        assert_eq!(polish_field_ending(s), s);
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
