//! 写作上下文与任务
//! 代码路径: kk_novel_ai/src-tauri/src/writing/mod.rs

pub mod advance;
pub mod beat_engine;
pub mod continuity;
pub mod dedupe;
pub mod retrieve;
pub mod rhetoric;

use crate::error::{AppError, AppResult};
use crate::llm::{ChatMessage, ChatOptions, LmStudioClient, TokenUsage};
use crate::project::{self, LoreEntry};
use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritingTask {
    Continue,
    /** 同位置完全重写：按块指令从零重写，禁止续写后续、禁止改写旧稿 */
    SameSlotVariant,
    Polish,
    Outline,
    Consistency,
    ChapterSummary,
    StorySync,
    BlockDigest,
    CastExtract,
    /** 本块情节/性癖抽取，写入全局库 */
    TropeExtract,
    /** 先分析需要几节，再由前端排队续写 */
    SectionPlan,
    /** 从章纲 summary 拆成 beats JSON */
    OutlineToBeats,
    /** 从全书大纲 book_outline 拆成章节列表 JSON */
    OutlineToChapters,
    /** 把全书大纲 / 已有卷章纲整理成思维导图 JSON */
    OutlineToMindmap,
    /** 节拍 → 分镜表 JSON */
    BeatsToStoryboard,
    /** 正文/分镜 → 绘图提示词 JSON */
    ContentToImagePrompt,
}

impl WritingTask {
    pub fn from_str_loose(s: &str) -> AppResult<Self> {
        match s {
            "continue" | "continue_chapter" => Ok(Self::Continue),
            "same_slot_variant" | "variant" | "same_slot" => Ok(Self::SameSlotVariant),
            "polish" => Ok(Self::Polish),
            "outline" | "outline_expand" => Ok(Self::Outline),
            "consistency" | "consistency_check" => Ok(Self::Consistency),
            "chapter_summary" | "summarize" => Ok(Self::ChapterSummary),
            "story_sync" | "sync_story" => Ok(Self::StorySync),
            "block_digest" | "digest" => Ok(Self::BlockDigest),
            "cast_extract" | "auto_cast" => Ok(Self::CastExtract),
            "trope_extract" | "auto_trope" => Ok(Self::TropeExtract),
            "section_plan" | "plan_sections" => Ok(Self::SectionPlan),
            "outline_to_beats" | "split_beats" => Ok(Self::OutlineToBeats),
            "outline_to_chapters" | "split_chapters" => Ok(Self::OutlineToChapters),
            "outline_to_mindmap" | "mindmap_outline" => Ok(Self::OutlineToMindmap),
            "beats_to_storyboard" | "storyboard_from_beats" => Ok(Self::BeatsToStoryboard),
            "content_to_image_prompt" | "image_prompt" => Ok(Self::ContentToImagePrompt),
            _ => Err(AppError::t_fmt("errors.unknownWritingTask", &[("task", s)])),
        }
    }

    fn template(&self) -> &'static str {
        match self {
            Self::Continue => crate::prompt_i18n::prompt("continue_chapter.md"),
            Self::SameSlotVariant => crate::prompt_i18n::prompt("same_slot_variant.md"),
            Self::Polish => crate::prompt_i18n::prompt("polish.md"),
            Self::Outline => crate::prompt_i18n::prompt("outline_expand.md"),
            Self::Consistency => crate::prompt_i18n::prompt("consistency_check.md"),
            Self::ChapterSummary => crate::prompt_i18n::prompt("chapter_summary.md"),
            Self::StorySync => crate::prompt_i18n::prompt("story_sync.md"),
            Self::BlockDigest => crate::prompt_i18n::prompt("block_digest.md"),
            Self::CastExtract => crate::prompt_i18n::prompt("cast_extract.md"),
            Self::TropeExtract => crate::prompt_i18n::prompt("trope_extract.md"),
            Self::SectionPlan => crate::prompt_i18n::prompt("section_plan.md"),
            Self::OutlineToBeats => crate::prompt_i18n::prompt("outline_to_beats.md"),
            Self::OutlineToChapters => crate::prompt_i18n::prompt("outline_to_chapters.md"),
            Self::OutlineToMindmap => crate::prompt_i18n::prompt("outline_to_mindmap.md"),
            Self::BeatsToStoryboard => crate::prompt_i18n::prompt("beats_to_storyboard.md"),
            Self::ContentToImagePrompt => crate::prompt_i18n::prompt("content_to_image_prompt.md"),
        }
    }

    fn as_route_key(&self) -> &str {
        match self {
            Self::Continue => "continue",
            Self::SameSlotVariant => "same_slot_variant",
            Self::Polish => "polish",
            Self::Outline => "outline",
            Self::Consistency => "consistency",
            Self::ChapterSummary => "chapter_summary",
            Self::StorySync => "story_sync",
            Self::BlockDigest => "block_digest",
            Self::CastExtract => "cast_extract",
            Self::TropeExtract => "trope_extract",
            Self::SectionPlan => "section_plan",
            Self::OutlineToBeats => "outline_to_beats",
            Self::OutlineToChapters => "outline_to_chapters",
            Self::OutlineToMindmap => "outline_to_mindmap",
            Self::BeatsToStoryboard => "beats_to_storyboard",
            Self::ContentToImagePrompt => "content_to_image_prompt",
        }
    }

    fn is_analysis(&self) -> bool {
        matches!(
            self,
            Self::Consistency
                | Self::ChapterSummary
                | Self::StorySync
                | Self::BlockDigest
                | Self::CastExtract
                | Self::TropeExtract
                | Self::SectionPlan
                | Self::OutlineToBeats
                | Self::OutlineToChapters
                | Self::OutlineToMindmap
                | Self::BeatsToStoryboard
                | Self::ContentToImagePrompt
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WritingRequest {
    pub project_root: String,
    pub chapter_id: String,
    pub task: String,
    #[serde(default)]
    pub instruction: String,
    #[serde(default)]
    pub selection: String,
    /// 块级蒸馏时对应生成块 key
    #[serde(default)]
    pub block_key: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub frequency_penalty: Option<f32>,
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    /// 主模型失败时回退；空则用 settings.model
    #[serde(default)]
    pub fallback_model: Option<String>,
    /// 覆盖 settings.writing_retry_on_loop；None 跟设置
    #[serde(default)]
    pub retry_on_loop: Option<bool>,
    /// 前端算好的分支激活路径前缀；非空时续写/大纲用它代替章节全文尾巴
    #[serde(default)]
    pub branch_context_text: Option<String>,
    /// 按纲续写：当前要兑现的 beat id
    #[serde(default)]
    pub active_beat_id: Option<String>,
    /// 按纲整章队列：true 时禁止换场升级、写完本章纲再停
    #[serde(default)]
    pub outline_run: Option<bool>,
    /// outline_to_chapters：full | append
    #[serde(default)]
    pub split_mode: Option<String>,
    /// 本轮勾选的情节/性癖 id；Some（含空数组）覆盖本章；None 回退 chapter.trope_ids
    #[serde(default)]
    pub selected_trope_ids: Option<Vec<String>>,
    /// 扫描时冻结的情节/性癖名单；Some 时 `trope_extract` 不再读盘，前缀可跨块缓存
    #[serde(default)]
    pub known_tropes_snapshot: Option<String>,
}

/// 单次生成实际注入的上下文来源（挂到生成块，便于回看情节依据）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextSourceItem {
    /// instruction | outline | pov | arc | must_do | lore | beat | trope | kink
    pub kind: String,
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WritingContextSources {
    #[serde(default)]
    pub items: Vec<ContextSourceItem>,
}

pub struct AssembledWriting {
    pub messages: Vec<ChatMessage>,
    pub context_sources: WritingContextSources,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WritingOutcome {
    /// 复读截断后的定稿（插入章末用这个）
    pub text: String,
    /// 模型原始全文（截断前）；供 GUI 对照
    #[serde(default)]
    pub raw_text: String,
    pub model_used: String,
    #[serde(default)]
    pub fallback_from: Option<String>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub loop_retried: bool,
    #[serde(default)]
    pub usage: TokenUsage,
    #[serde(default)]
    pub prompt_messages: Vec<ChatMessage>,
    #[serde(default)]
    pub log_id: String,
    /// 本次写入 prompt 的设定/章纲来源摘要
    #[serde(default)]
    pub context_sources: WritingContextSources,
}

fn take_chars_brief(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}

fn split_mode_peek(req: &WritingRequest) -> String {
    let m = req
        .split_mode
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if m == "append" {
        "append".into()
    } else {
        "full".into()
    }
}

/// 若补写开头复读了已写正文末尾，裁掉重叠段
fn strip_draft_overlap(draft: &str, piece: &str) -> String {
    let piece = piece.trim_start();
    if piece.is_empty() || draft.chars().count() < 40 {
        return piece.to_string();
    }
    let draft_chars: Vec<char> = draft.chars().collect();
    let piece_chars: Vec<char> = piece.chars().collect();
    let max_check = draft_chars.len().min(piece_chars.len()).min(400);
    for n in (40..=max_check).rev() {
        if draft_chars[draft_chars.len() - n..] == piece_chars[..n] {
            return piece_chars[n..]
                .iter()
                .collect::<String>()
                .trim_start()
                .to_string();
        }
    }
    piece.to_string()
}

fn scrub_dump_memory(s: &str) -> String {
    let parts: Vec<&str> = s
        .split("\n\n")
        .filter(|p| !continuity::is_dump_placeholder(p))
        .collect();
    let out = parts.join("\n\n");
    if out.trim().is_empty() {
        "（无）".into()
    } else {
        out
    }
}

/// 同位置变体目标字数：不低于设定续写字数；参考更长时对齐参考，避免越变越短
fn same_slot_target_chars(settings_target: u32, selection_chars: usize) -> u32 {
    let base = settings_target.max(200);
    if selection_chars > 200 {
        base.max(selection_chars as u32)
    } else {
        base
    }
}

fn same_slot_max_tokens(settings_target: u32, selection_chars: usize) -> u32 {
    let chars = same_slot_target_chars(settings_target, selection_chars);
    // 与 resolve_writing_max_tokens 一致：允许超出规定字数
    let mt = ((chars as f64) * 1.8).ceil() as u32;
    mt.max(256).min(32768)
}

fn approx_tokens(text: &str) -> u32 {
    // 中文粗估：约 1.5 字/token
    ((text.chars().count() as f32) / 1.5).ceil() as u32
}

/// 按优先级裁切 prompt：先缩 lore/plot/recent，保护 outline/focus/beats/anchor/canon
fn trim_user_prompt_to_budget(user: &str, budget: u32) -> String {
    let mut out = user.to_string();
    if approx_tokens(&out) <= budget {
        return out;
    }
    let shrink_pairs: [(&str, &str); 4] = [
        ("前文（滑动窗口）：", "用户指令："),
        ("相关设定：", "本章大纲："),
        ("时间线（近期）：", "关系："),
        ("故事线与承诺：", "时间线（近期）："),
    ];
    for _ in 0..24 {
        if approx_tokens(&out) <= budget {
            break;
        }
        let mut done = false;
        for (start, end) in shrink_pairs {
            if let Some(next) = shrink_marked_section(&out, start, end, 0.82) {
                out = next;
                done = true;
                break;
            }
        }
        if !done {
            let chars: Vec<char> = out.chars().collect();
            if chars.len() <= 500 {
                break;
            }
            let keep = (chars.len() as f32 * 0.92) as usize;
            out = chars[..keep.max(400)].iter().collect();
            out.push_str("\n…(上下文已截断)");
            break;
        }
    }
    out
}

fn shrink_marked_section(text: &str, start_marker: &str, end_marker: &str, ratio: f32) -> Option<String> {
    let start = text.find(start_marker)?;
    let body_start = start + start_marker.len();
    let rest = &text[body_start..];
    let end_off = rest.find(end_marker).unwrap_or(rest.len());
    if end_off < 80 {
        return None;
    }
    let section = &rest[..end_off];
    let trimmed: String = section.chars().take((section.chars().count() as f32 * ratio) as usize).collect();
    if trimmed.chars().count() + 20 >= section.chars().count() {
        return None;
    }
    let mut out = String::new();
    out.push_str(&text[..body_start]);
    out.push_str(trimmed.trim());
    out.push('\n');
    out.push_str(&rest[end_off..]);
    Some(out)
}

fn take_tail(text: &str, max_chars: usize) -> String {
    continuity::take_tail_at_boundary(text, max_chars)
}

/// 作品内章节顺序（卷内顺序优先，其余按 chapters 列表）
fn ordered_chapter_ids(project: &project::NovelProject) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for vol in &project.volumes {
        for id in &vol.chapter_ids {
            if seen.insert(id.clone()) {
                out.push(id.clone());
            }
        }
    }
    for ch in &project.chapters {
        if seen.insert(ch.id.clone()) {
            out.push(ch.id.clone());
        }
    }
    out
}

/// 上一章收束：摘要 + 正文末段，供空章/跨章续写衔接
fn build_prev_chapter_bridge(
    root: &Path,
    project: &project::NovelProject,
    chapter_id: &str,
) -> String {
    let ordered = ordered_chapter_ids(project);
    let Some(idx) = ordered.iter().position(|id| id == chapter_id) else {
        return "（无）".into();
    };
    if idx == 0 {
        return "（本章为开篇，无上章）".into();
    }
    let prev_id = &ordered[idx - 1];
    let Ok((prev_ch, prev_content)) = project::read_chapter(root, prev_id) else {
        return "（无）".into();
    };
    let memory = project::load_memory(root).unwrap_or_default();
    let snap = memory
        .chapter_snapshots
        .iter()
        .find(|s| &s.chapter_id == prev_id)
        .map(|s| s.summary.trim().to_string())
        .filter(|s| !s.is_empty());
    let note = memory
        .block_notes
        .iter()
        .filter(|n| &n.chapter_id == prev_id)
        .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
        .map(|n| n.summary.trim().to_string())
        .filter(|s| !s.is_empty());
    let summary = snap.or(note).unwrap_or_else(|| "（无写后总结）".into());
    let tail = take_tail(&prev_content, 700);
    let tail = if tail.trim().is_empty() || tail.lines().all(|l| {
        let t = l.trim();
        t.is_empty() || t.starts_with('#')
    }) {
        "（上章正文为空）".to_string()
    } else {
        tail
    };
    format!(
        "上章标题：{}\n上章收束摘要：{}\n上章正文末段：\n{}",
        prev_ch.title, summary, tail
    )
}

/// 从设定 / 全书大纲推断人称性别锁
fn character_gender_lock(project: &project::NovelProject, lore: &[&LoreEntry]) -> String {
    let outline = project.book_outline.as_str();
    let mut lines: Vec<String> = Vec::new();
    for e in lore {
        let kind = e.kind.as_str();
        if !(kind.is_empty() || kind == "character") {
            continue;
        }
        let title = e.title.trim();
        if title.is_empty() {
            continue;
        }
        let content = e.content.as_str();
        let attrs_blob = e
            .attrs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        let gender = e
            .attrs
            .get("gender")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                let local = format!("{title} {content} {attrs_blob}");
                if local.contains("女生")
                    || local.contains("女孩")
                    || local.contains("女主")
                    || local.contains("表妹")
                    || local.contains("女同学")
                    || content.contains("女")
                {
                    Some("女".into())
                } else if local.contains("男生")
                    || local.contains("男孩")
                    || local.contains("男主")
                    || local.contains("表弟")
                    || local.contains("男同学")
                    || content.contains("男")
                {
                    Some("男".into())
                } else {
                    None
                }
            })
            .or_else(|| {
                // 大纲：角色名出现在「女生/女孩」语境，或大纲写明女体部位且点名该角色
                if !outline.contains(title) {
                    return None;
                }
                if outline.contains(&format!("女生{title}"))
                    || outline.contains(&format!("女孩{title}"))
                    || outline.contains(&format!("小学女生{title}"))
                    || outline.contains(&format!("{title}让"))
                        && (outline.contains("乳房")
                            || outline.contains("小穴")
                            || outline.contains("阴蒂")
                            || outline.contains("女生"))
                {
                    return Some("女".into());
                }
                if outline.contains("女生") || outline.contains("女孩") || outline.contains("乳房")
                {
                    return Some("女".into());
                }
                None
            });
        if let Some(g) = gender {
            let pronoun = if g.starts_with('女') { "她" } else { "他" };
            lines.push(format!(
                "- {title}：{g}性；叙述人称必须用「{pronoun}」；禁止改成异性，禁止改称表兄/表弟/哥哥/妹妹等错性别亲属称谓"
            ));
        }
    }
    if lines.is_empty() && (outline.contains("女生") || outline.contains("女孩")) {
        lines.push(
            "- 全书大纲含「女生/女孩」：相关女角色叙述必须用「她」，禁止写成「他」或「表弟」"
                .into(),
        );
    }
    if lines.is_empty() {
        "（无额外性别锁；仍遵守「女角色女体」解剖规则）".into()
    } else {
        lines.join("\n")
    }
}

fn render_template(tpl: &str, map: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in map {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

fn lore_to_text(entries: &[&LoreEntry]) -> String {
    if entries.is_empty() {
        return "（无）".into();
    }
    entries
        .iter()
        .map(|e| {
            let mut attrs = String::new();
            if !e.attrs.is_empty() {
                let parts: Vec<String> = e
                    .attrs
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                attrs = format!("\n属性: {}", parts.join(", "));
            }
            let mut links = String::new();
            if !e.links.is_empty() {
                let parts: Vec<String> = e
                    .links
                    .iter()
                    .map(|l| format!("{}→{}", l.relation, l.target_id))
                    .collect();
                links = format!("\n关联: {}", parts.join("; "));
            }
            format!(
                "### {} ({})\n关键词: {}{}{}\n{}\n",
                e.title,
                e.kind,
                e.keywords.join(", "),
                attrs,
                links,
                e.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_trope_kind(kind: &str) -> bool {
    kind == "trope" || kind == "kink"
}

fn tropes_to_text(entries: &[&LoreEntry]) -> String {
    if entries.is_empty() {
        return "（无）".into();
    }
    entries
        .iter()
        .map(|e| {
            let kind_label = if e.kind == "kink" { "性癖" } else { "情节" };
            let intensity = e.attrs.get("intensity").map(|s| s.as_str()).unwrap_or("");
            let do_line = e.attrs.get("do").map(|s| s.as_str()).unwrap_or("");
            let dont_line = e.attrs.get("dont").map(|s| s.as_str()).unwrap_or("");
            let tags = e.attrs.get("tags").map(|s| s.as_str()).unwrap_or("");
            let mut extra = String::new();
            if !intensity.is_empty() {
                extra.push_str(&format!("\n强度: {intensity}"));
            }
            if !tags.is_empty() {
                extra.push_str(&format!("\n标签: {tags}"));
            }
            if !do_line.is_empty() {
                extra.push_str(&format!("\n要写: {do_line}"));
            }
            if !dont_line.is_empty() {
                extra.push_str(&format!("\n不要: {dont_line}"));
            }
            format!(
                "### {}（{}）\n关键词: {}{}\n{}\n",
                e.title,
                kind_label,
                e.keywords.join(", "),
                extra,
                e.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 压缩名单：按 kind 各一行，库内「最新在前」翻成「旧→新」以便新卡只追加在行尾，保住 DeepSeek 前缀缓存。
pub(crate) fn format_known_tropes_compact(entries: &[LoreEntry]) -> String {
    let mut kinks: Vec<String> = Vec::new();
    let mut tropes: Vec<String> = Vec::new();
    for e in entries {
        if !is_trope_kind(&e.kind) {
            continue;
        }
        let title = e.title.trim();
        if title.is_empty() {
            continue;
        }
        let bucket = if e.kind == "kink" {
            &mut kinks
        } else {
            &mut tropes
        };
        if !bucket.iter().any(|x| x == title) {
            bucket.push(title.to_string());
        }
    }
    kinks.reverse();
    tropes.reverse();
    if kinks.is_empty() && tropes.is_empty() {
        return "（无）".into();
    }
    let mut parts = Vec::new();
    if !kinks.is_empty() {
        parts.push(format!("kink: {}", kinks.join(" / ")));
    }
    if !tropes.is_empty() {
        parts.push(format!("trope: {}", tropes.join(" / ")));
    }
    parts.join("\n")
}

fn known_tropes_catalog(entries: &[LoreEntry]) -> String {
    format_known_tropes_compact(entries)
}

fn resolve_selected_trope_ids(req: &WritingRequest, chapter: &project::ChapterMeta) -> Vec<String> {
    match &req.selected_trope_ids {
        Some(ids) => ids.clone(),
        None => chapter.trope_ids.clone(),
    }
}

fn should_inject_selected_tropes(task: &WritingTask) -> bool {
    matches!(
        task,
        WritingTask::Continue
            | WritingTask::SameSlotVariant
            | WritingTask::Outline
            | WritingTask::OutlineToBeats
            | WritingTask::SectionPlan
    )
}

fn collect_selected_tropes<'a>(pool: &'a [LoreEntry], ids: &[String]) -> Vec<&'a LoreEntry> {
    let mut out = Vec::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        if out.iter().any(|e: &&LoreEntry| e.id == id) {
            continue;
        }
        if let Some(e) = pool.iter().find(|e| e.id == id) {
            out.push(e);
        }
    }
    out
}

/// 长全书大纲拆章：按 prompt 字数抬高 max_tokens，避免 JSON 被截断。
fn outline_split_max_tokens(prompt_chars: u32, current: u32) -> u32 {
    let estimated = ((prompt_chars as f64 / 1.5 * 0.9).ceil() as u32)
        .max(2048)
        .min(16384);
    current.max(estimated).clamp(1024, 16384)
}

/// 按纲整章续写：在规定字数预算上按章纲长度略抬，避免一轮写爆半句。
fn outline_continue_max_tokens(base: u32, outline_chars: u32) -> u32 {
    let extra = ((outline_chars as f64 / 2.0).ceil() as u32).min(4096);
    base.saturating_add(extra).clamp(base.max(2048), 8192)
}

fn request_is_outline_run(req: &WritingRequest) -> bool {
    if req.outline_run.unwrap_or(false) {
        return true;
    }
    let instr = req.instruction.trim();
    instr.contains("【按纲生成")
        || instr.contains("[Outline run")
        || instr.contains("【アウトライン実行")
}

fn completeness_goal(locale: &str, mode: &str) -> String {
    let loc = AppSettings::normalize_locale_code(locale);
    if mode == "sentence" {
        if loc == "en" {
            "Finish only the last sentence. Do not start a new scene or the next day.".into()
        } else if loc == "ja" {
            "最後の一文だけ書き切る。新しい場面や翌日へ進まない。".into()
        } else {
            "只写完最后一句，禁止新场景、禁止下一天。".into()
        }
    } else if loc == "en" {
        "Length is already enough. Finish remaining outline beats for THIS chapter only, then stop. Do not jump to the next chapter or the next day.".into()
    } else if loc == "ja" {
        "字数は足りている。本章の章綱でまだ書いていない収束だけ書き切って止める。次章や翌日へ飛ばない。".into()
    } else {
        "字数已够。把本章纲尚未写出的收束写完即停，禁止跳到下一章或下一天。".into()
    }
}

fn resolve_writing_options(
    settings: &AppSettings,
    task: &WritingTask,
    req: &WritingRequest,
    chapter_chars: usize,
) -> ChatOptions {
    let analysis = task.is_analysis();
    let model = req
        .model
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if analysis {
                let m = settings.resolve_analysis_model();
                if m.is_empty() {
                    None
                } else {
                    Some(m.to_string())
                }
            } else {
                Some(settings.resolve_writing_model_for_task(task.as_route_key(), chapter_chars))
            }
        });
    let temperature = req.temperature.or_else(|| {
        if analysis {
            Some(settings.resolve_analysis_temperature())
        } else {
            None
        }
    });
    // 写作任务：max_tokens 始终与规定字数同量级；分析任务用较短上限
    let max_tokens = if matches!(task, WritingTask::TropeExtract) {
        req.max_tokens.or(Some(1024))
    } else if matches!(
        task,
        WritingTask::BlockDigest | WritingTask::CastExtract
    ) {
        req.max_tokens.or(Some(512))
    } else if matches!(
        task,
        WritingTask::OutlineToMindmap | WritingTask::OutlineToChapters
    ) {
        req.max_tokens
            .or(Some(settings.max_tokens.min(8192).max(2048)))
    } else if analysis {
        req.max_tokens
            .or(Some(settings.max_tokens.min(2048).max(512)))
    } else {
        let aligned = if matches!(task, WritingTask::SameSlotVariant) {
            // 与设定续写字数对齐（不再跟 selection/总结长度走）
            same_slot_max_tokens(settings.resolve_writing_target_chars(), 0)
        } else if matches!(task, WritingTask::Continue) && request_is_outline_run(req) {
            outline_continue_max_tokens(
                settings.resolve_writing_max_tokens(),
                req.instruction.chars().count() as u32,
            )
        } else {
            settings.resolve_writing_max_tokens()
        };
        match req.max_tokens {
            // CLI 显式微调：允许不超过对齐值 1.25 倍
            Some(v) if v > 0 && v <= ((aligned as f64) * 1.25).ceil() as u32 => Some(v.max(256)),
            _ => Some(aligned),
        }
    };
    ChatOptions {
        model,
        temperature,
        max_tokens,
        frequency_penalty: req.frequency_penalty,
        presence_penalty: req.presence_penalty,
        stream: true,
    }
}

pub fn assemble_messages(
    settings: &AppSettings,
    req: &WritingRequest,
) -> AppResult<AssembledWriting> {
    assemble_messages_with_scores(settings, req, None)
}

pub fn assemble_messages_with_scores(
    settings: &AppSettings,
    req: &WritingRequest,
    semantic_scores: Option<&std::collections::HashMap<String, f32>>,
) -> AppResult<AssembledWriting> {
    let root = Path::new(&req.project_root);
    let opened = project::open_project(root)?;
    let task = WritingTask::from_str_loose(&req.task)?;
    let (chapter, file_content) = project::read_chapter(root, &req.chapter_id)?;
    // 分支生成：前端传入激活路径前缀，避免吃到兄弟变体
    // 同位置变体：即使 branch_context 为空也不回退整章正文（避免上节全文污染）
    let content = match &req.branch_context_text {
        Some(t) if !t.trim().is_empty() => t.clone(),
        _ if task == WritingTask::SameSlotVariant => String::new(),
        _ => file_content,
    };
    // 按当前分块过滤记忆，避免已删正文的旧笔记继续进 prompt
    let memory = project::memory_filtered_for_chapter(root, &req.chapter_id)?;

    // 块蒸馏：极简组装，不拉 lore/RAG
    if task == WritingTask::BlockDigest {
        let block_text = if !req.selection.trim().is_empty() {
            req.selection.clone()
        } else {
            take_tail(&content, 4000)
        };
        let prev_memory = if memory.rolling_summary.is_empty() {
            "（无）".into()
        } else {
            scrub_dump_memory(&memory.rolling_summary)
        };
        let instruction = if req.instruction.is_empty() {
            "（无）"
        } else {
            req.instruction.as_str()
        };
        let known_digest = format_known_tropes_compact(&collect_known_trope_entries(
            root,
            &opened.project,
        ));
        let user = render_template(
            task.template(),
            &[
                ("prev_memory", &prev_memory),
                ("block_text", &block_text),
                ("instruction", instruction),
                ("known_tropes", &known_digest),
            ],
        );
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "你是小说记忆助理，只输出规定篇幅的剧情摘要。".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user,
            },
        ];
        return Ok(AssembledWriting {
            messages,
            context_sources: WritingContextSources::default(),
        });
    }

    // 本篇新角色抽取：注入已有名单，不拉 RAG
    if task == WritingTask::CastExtract {
        let block_text = if !req.selection.trim().is_empty() {
            req.selection.clone()
        } else {
            take_tail(&content, 4000)
        };
        let known = collect_known_character_names(root, &opened.project);
        let known_characters = if known.is_empty() {
            "（无）".into()
        } else {
            known.join("、")
        };
        let instruction = if req.instruction.is_empty() {
            "（无）"
        } else {
            req.instruction.as_str()
        };
        let user = render_template(
            task.template(),
            &[
                ("known_characters", &known_characters),
                ("block_text", &block_text),
                ("instruction", instruction),
            ],
        );
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "你是小说设定助理，只输出规定 JSON，不要解释。".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user,
            },
        ];
        return Ok(AssembledWriting {
            messages,
            context_sources: WritingContextSources::default(),
        });
    }

    // 情节/性癖抽取：对照已知名单，不拉 RAG。规则+名单放 system，正文单独 user，便于前缀缓存。
    if task == WritingTask::TropeExtract {
        let block_text = if !req.selection.trim().is_empty() {
            req.selection.clone()
        } else {
            take_tail(&content, 8000)
        };
        let known_tropes = if let Some(snap) = &req.known_tropes_snapshot {
            if snap.trim().is_empty() {
                "（无）".into()
            } else {
                snap.clone()
            }
        } else {
            format_known_tropes_compact(&collect_known_trope_entries(root, &opened.project))
        };
        let instruction = if req.instruction.is_empty() {
            "（无）"
        } else {
            req.instruction.as_str()
        };
        let system = render_template(
            task.template(),
            &[
                ("known_tropes", &known_tropes),
                ("instruction", instruction),
            ],
        );
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system,
            },
            ChatMessage {
                role: "user".into(),
                content: block_text,
            },
        ];
        return Ok(AssembledWriting {
            messages,
            context_sources: WritingContextSources::default(),
        });
    }

    if matches!(
        task,
        WritingTask::BeatsToStoryboard | WritingTask::ContentToImagePrompt
    ) {
        let recent = if !req.selection.trim().is_empty() {
            req.selection.clone()
        } else {
            take_tail(&content, 4000)
        };
        let instruction = if req.instruction.is_empty() {
            "（无）"
        } else {
            req.instruction.as_str()
        };
        let outline = if chapter.summary.is_empty() {
            chapter.title.clone()
        } else {
            format!("{}\n{}", chapter.title, chapter.summary)
        };
        let beats = if chapter.beats.is_empty() {
            "（无）".into()
        } else {
            serde_json::to_string_pretty(&chapter.beats).unwrap_or_else(|_| "（无）".into())
        };
        let lore_text = visual_sheets_for_prompt(root);
        let user = render_template(
            task.template(),
            &[
                ("outline", &outline),
                ("beats", &beats),
                ("recent_text", &recent),
                ("instruction", instruction),
                ("lore", &lore_text),
            ],
        );
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "你是小说分镜与插图助理，只输出规定 JSON，不要解释。".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user,
            },
        ];
        return Ok(AssembledWriting {
            messages,
            context_sources: WritingContextSources::default(),
        });
    }

    let mut all_lore = project::list_lore(root)?;
    // 本篇优先：先放入本篇条目，再挂接库（unique 合并时先到者胜）
    let mut ranked: Vec<LoreEntry> = all_lore.drain(..).collect();
    for link in &opened.project.linked_kb_roots {
        let kb_path = match crate::kb::resolve_kb_root(link) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Ok(entries) = project::list_lore(&kb_path) {
            let src_title = project::open_project(&kb_path)
                .map(|o| o.project.title)
                .unwrap_or_else(|_| link.clone());
            for mut e in entries {
                e.attrs
                    .entry("_linked_kb".into())
                    .or_insert_with(|| src_title.clone());
                if !e.title.contains('[') {
                    e.title = format!("[{src_title}] {}", e.title);
                }
                ranked.push(e);
            }
        }
    }
    for e in crate::kb::list_trope_library_entries() {
        ranked.push(e);
    }
    all_lore = project::coalesce_unique_lore(ranked);
    let (entity_lore, trope_pool): (Vec<LoreEntry>, Vec<LoreEntry>) = all_lore
        .into_iter()
        .partition(|e| !is_trope_kind(&e.kind));
    all_lore = entity_lore;
    let selected_trope_ids = resolve_selected_trope_ids(req, &chapter);
    let selected_tropes = collect_selected_tropes(&trope_pool, &selected_trope_ids);
    let tropes_text = if should_inject_selected_tropes(&task) {
        tropes_to_text(&selected_tropes)
    } else {
        "（无）".into()
    };
    let known_tropes_text = known_tropes_catalog(&trope_pool);
    let plot = crate::story::load_plot(root).unwrap_or_default();
    let timeline = crate::story::load_timeline(root).unwrap_or_default();
    let relations = crate::story::load_relations(root).unwrap_or_default();
    let canon = crate::story::load_canon(root).unwrap_or_default();

    let mut seed_ids: Vec<String> = Vec::new();
    if let Some(pov) = &chapter.pov_lore_id {
        if !pov.is_empty() {
            seed_ids.push(pov.clone());
        }
    }
    for arc_id in &chapter.focus_arc_ids {
        if let Some(arc) = plot.arcs.iter().find(|a| &a.id == arc_id) {
            seed_ids.extend(arc.related_lore_ids.clone());
        }
    }
    let neighbor_ids = crate::story::neighbor_lore_ids(&relations, &seed_ids);

    let query = format!(
        "{} {} {} {} {} {}",
        opened.project.book_outline,
        chapter.title,
        chapter.summary,
        chapter.must_do,
        req.instruction,
        req.selection
    );
    let mut lore = retrieve::retrieve_lore_hybrid(&all_lore, &query, 8, semantic_scores);
    // 强制纳入邻居设定
    for id in &neighbor_ids {
        if lore.iter().any(|e| &e.id == id) {
            continue;
        }
        if let Some(e) = all_lore.iter().find(|e| &e.id == id) {
            lore.push(e);
        }
    }
    continuity::force_include_named_lore(&mut lore, &all_lore, &query, 12);
    if lore.len() > 12 {
        lore.truncate(12);
    }

    let recent = match task {
        WritingTask::Polish => req.selection.clone(),
        WritingTask::SameSlotVariant => {
            // 不注入章节前文；上节总结如有则已在 selection / instruction
            String::new()
        }
        WritingTask::Consistency => {
            take_tail(&content, settings.recent_window_chars.saturating_mul(2))
        }
        WritingTask::StorySync => {
            // 一键重建会把激活路径全文放进 branch_context / selection；窗口加长以免只看见章末
            let src = if !req.selection.trim().is_empty() {
                req.selection.as_str()
            } else {
                content.as_str()
            };
            take_tail(src, settings.recent_window_chars.saturating_mul(4))
        }
        WritingTask::ChapterSummary => {
            take_tail(&content, settings.recent_window_chars.saturating_mul(3))
        }
        WritingTask::Continue
        | WritingTask::Outline
        | WritingTask::SectionPlan
        | WritingTask::OutlineToBeats => {
            // 有滚动记忆时缩短原文尾巴，接文风即可
            let win = if memory.rolling_summary.trim().is_empty() {
                settings.recent_window_chars
            } else {
                settings.recent_window_chars.min(800)
            };
            let mut recent = take_tail(&content, win);
            // 空章冷启动：把上章正文末段并入「前文」，避免跨章失忆
            let nearly_empty = content.trim().is_empty()
                || content
                    .lines()
                    .all(|l| l.trim().is_empty() || l.trim().starts_with('#'));
            if nearly_empty && matches!(task, WritingTask::Continue | WritingTask::OutlineToBeats) {
                let bridge = build_prev_chapter_bridge(root, &opened.project, &req.chapter_id);
                if bridge != "（无）" && !bridge.contains("无上章") && !bridge.contains("为空") {
                    recent = format!("【跨章衔接·上章末】\n{bridge}");
                }
            }
            recent
        }
        WritingTask::OutlineToChapters | WritingTask::OutlineToMindmap => String::new(),
        WritingTask::BlockDigest
        | WritingTask::CastExtract
        | WritingTask::TropeExtract
        | WritingTask::BeatsToStoryboard
        | WritingTask::ContentToImagePrompt => req.selection.clone(),
    };
    let style = if opened.project.style.is_empty() {
        format!("书名：{}", opened.project.title)
    } else {
        format!("书名：{}\n{}", opened.project.title, opened.project.style)
    };
    let outline = if chapter.summary.is_empty() {
        chapter.title.clone()
    } else {
        format!("{}\n{}", chapter.title, chapter.summary)
    };
    let focus = format!(
        "POV: {}\n焦点弧: {}\n必达: {}\n禁止: {}\n读者已知: {}\n角色已知: {}",
        chapter.pov_lore_id.as_deref().unwrap_or("（未设）"),
        if chapter.focus_arc_ids.is_empty() {
            "（未绑）".into()
        } else {
            chapter.focus_arc_ids.join(", ")
        },
        if chapter.must_do.is_empty() {
            "（无）"
        } else {
            chapter.must_do.as_str()
        },
        if chapter.must_not.is_empty() {
            "（无）"
        } else {
            chapter.must_not.as_str()
        },
        if chapter.reader_knows.is_empty() {
            "（无）"
        } else {
            chapter.reader_knows.as_str()
        },
        if chapter.character_knows.is_empty() {
            "（无）"
        } else {
            chapter.character_knows.as_str()
        },
    );
    let beats = if chapter.beats.is_empty() {
        "（无节拍；可用章纲）".into()
    } else {
        chapter
            .beats
            .iter()
            .enumerate()
            .map(|(i, b)| {
                format!(
                    "{}. {} | 目的:{} | 冲突:{} | 情绪:{} | 地点:{}",
                    i + 1,
                    b.title,
                    b.purpose,
                    b.conflict,
                    b.emotion,
                    b.location.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut beat_progress = project::load_beat_progress(root, &req.chapter_id).unwrap_or_default();
    beat_progress = beat_engine::load_or_init_progress(&chapter.beats, beat_progress);
    if let Some(ref bid) = req.active_beat_id {
        if !bid.trim().is_empty() {
            beat_engine::ensure_active_beat(&chapter.beats, &mut beat_progress, bid.trim());
        }
    }
    let outline_run = request_is_outline_run(req);
    let active_beat_ref = req
        .active_beat_id
        .as_ref()
        .and_then(|id| chapter.beats.iter().find(|b| b.id == *id))
        .or_else(|| beat_engine::active_beat(&chapter.beats, &beat_progress));
    let beat_status = if chapter.beats.is_empty() {
        advance::build_beat_status_legacy(&chapter.beats, &content)
    } else {
        advance::build_beat_status(&chapter.beats, &beat_progress)
    };
    let ban_list = advance::build_dynamic_ban_list(&recent, &chapter.must_not);
    let instruction = if req.instruction.is_empty() {
        "（无额外指令）"
    } else {
        req.instruction.as_str()
    };
    let direction_anchor = advance::build_direction_anchor(
        &chapter.must_do,
        instruction,
        &beat_status,
        &ban_list,
        active_beat_ref,
        outline_run,
    );
    let volume_arc = project::volume_for_chapter(&opened.project, &req.chapter_id)
        .map(|v| {
            if v.arc_goal.trim().is_empty() && v.arc_summary.trim().is_empty() {
                "（未设卷弧）".to_string()
            } else {
                format!(
                    "卷：{}\n卷目标：{}\n卷摘要：{}",
                    v.title, v.arc_goal, v.arc_summary
                )
            }
        })
        .unwrap_or_else(|| "（未绑卷）".into());
    let active_beat_text = active_beat_ref
        .map(beat_engine::beat_summary)
        .unwrap_or_else(|| "（无）".into());
    let plot_text = crate::story::plot_for_prompt(&plot, &chapter.focus_arc_ids);
    let timeline_text = crate::story::timeline_for_prompt(&timeline, 12);
    let relations_text = crate::story::relations_for_prompt(&relations, &seed_ids, 16);
    let canon_text = crate::story::canon_for_prompt(&canon, true);
    let lore_text = lore_to_text(&lore);
    let prev_chapter_bridge = if matches!(
        task,
        WritingTask::Continue
            | WritingTask::OutlineToBeats
            | WritingTask::SectionPlan
            | WritingTask::Outline
    ) {
        build_prev_chapter_bridge(root, &opened.project, &req.chapter_id)
    } else {
        "（无）".into()
    };
    let character_lock = if matches!(
        task,
        WritingTask::Continue
            | WritingTask::OutlineToBeats
            | WritingTask::Outline
            | WritingTask::Polish
            | WritingTask::SectionPlan
    ) {
        let gender = character_gender_lock(&opened.project, &lore);
        continuity::append_continuity_locks(
            &gender,
            opened.project.book_outline.as_str(),
            &lore,
            &prev_chapter_bridge,
        )
    } else {
        "（无）".into()
    };
    // 拆全书大纲：尽量不灌设定，避免模型用角色仓/lore 改写用户意愿
    let (plot_text, timeline_text, relations_text, canon_text, lore_text, memory_text) =
        if task == WritingTask::OutlineToChapters || task == WritingTask::OutlineToMindmap {
            (
                "（拆章时忽略，以全书大纲为准）".to_string(),
                "（拆章时忽略）".to_string(),
                "（拆章时忽略）".to_string(),
                "（拆章时忽略，以全书大纲为准）".to_string(),
                "（拆章时不注入设定，避免改写大纲）".to_string(),
                "（无）".to_string(),
            )
        } else {
            let memory_text = if task == WritingTask::SameSlotVariant {
                "（无）".into()
            } else if memory.rolling_summary.is_empty() {
                "（无）".into()
            } else {
                scrub_dump_memory(&memory.rolling_summary)
            };
            (plot_text, timeline_text, relations_text, canon_text, lore_text, memory_text)
        };
    let selection = if req.selection.is_empty() {
        "（无）"
    } else {
        req.selection.as_str()
    };
    // 同位置变体：selection 常为短总结，目标字数始终用设定续写字数，避免跟着总结变短
    let target_chars_n = settings.resolve_writing_target_chars() as usize;
    let target_chars = target_chars_n.to_string();

    let book_outline = if !opened.project.book_outline.trim().is_empty() {
        opened.project.book_outline.clone()
    } else if !req.instruction.trim().is_empty()
        && matches!(
            task,
            WritingTask::OutlineToChapters | WritingTask::OutlineToMindmap
        )
    {
        req.instruction.clone()
    } else {
        "（无全书大纲）".into()
    };
    let existing_chapters = {
        // full 拆章：空壳占位章（仅有默认标题、无章纲）不要列进「已有」，否则模型会从第2章起跳
        // append：列出全部已有标题以免撞名
        let titles: Vec<String> = opened
            .project
            .chapters
            .iter()
            .filter(|c| {
                if !matches!(
                    task,
                    WritingTask::OutlineToChapters | WritingTask::OutlineToMindmap
                ) {
                    return !c.summary.trim().is_empty()
                        || !c.beats.is_empty()
                        || c.status == "done"
                        || c.status == "outline_complete";
                }
                if split_mode_peek(req) == "append" {
                    return !c.title.trim().is_empty();
                }
                // full
                !c.summary.trim().is_empty()
                    || !c.beats.is_empty()
                    || c.status == "done"
                    || c.status == "outline_complete"
            })
            .map(|c| c.title.clone())
            .filter(|t| !t.trim().is_empty())
            .collect();
        if titles.is_empty() {
            "（无）".into()
        } else {
            titles
                .iter()
                .enumerate()
                .map(|(i, t)| format!("{}. {}", i + 1, t))
                .collect::<Vec<_>>()
                .join("\n")
        }
    };
    let split_mode = split_mode_peek(req);
    let existing_chapter_summaries = {
        let append = split_mode == "append";
        let lines: Vec<String> = opened
            .project
            .chapters
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let title = c.title.trim();
                let summary = c.summary.trim();
                if !append {
                    // full：无章纲的空壳不注入，避免模型当成「第1章已存在」
                    if summary.is_empty() && c.beats.is_empty() {
                        return None;
                    }
                }
                if title.is_empty() && summary.is_empty() {
                    return None;
                }
                let st = if c.status.is_empty() {
                    ""
                } else {
                    c.status.as_str()
                };
                Some(format!(
                    "{}. [{}] {} — {}",
                    i + 1,
                    if st.is_empty() { "draft" } else { st },
                    if title.is_empty() { "（无标题）" } else { title },
                    if summary.is_empty() {
                        "（无章纲）"
                    } else {
                        summary
                    }
                ))
            })
            .collect();
        if lines.is_empty() {
            "（无）".into()
        } else {
            lines.join("\n")
        }
    };

    let tpl = match task {
        WritingTask::Continue if settings.writing_cache_friendly_prompt => {
            crate::prompt_i18n::prompt("continue_chapter_cache.md")
        }
        _ => task.template(),
    };

    let mut user = render_template(
        tpl,
        &[
            ("style", &style),
            ("lore", &lore_text),
            ("outline", &outline),
            ("book_outline", &book_outline),
            ("existing_chapters", &existing_chapters),
            ("existing_chapter_summaries", &existing_chapter_summaries),
            ("split_mode", &split_mode),
            ("memory", &memory_text),
            ("recent_text", &recent),
            ("selection", selection),
            ("instruction", instruction),
            ("focus", &focus),
            ("beats", &beats),
            ("beat_status", &beat_status),
            ("ban_list", &ban_list),
            ("direction_anchor", &direction_anchor),
            ("volume_arc", &volume_arc),
            ("active_beat", &active_beat_text),
            ("plot", &plot_text),
            ("timeline", &timeline_text),
            ("relations", &relations_text),
            ("canon", &canon_text),
            ("target_chars", &target_chars),
            ("prev_chapter_bridge", &prev_chapter_bridge),
            ("character_lock", &character_lock),
            ("tropes", &tropes_text),
            ("known_tropes", &known_tropes_text),
        ],
    );

    let budget = settings.context_budget;
    user = trim_user_prompt_to_budget(&user, budget);

    // 同步 beats 进度 sidecar
    if !chapter.beats.is_empty() {
        let _ = project::save_beat_progress(root, &req.chapter_id, &beat_progress);
    }

    // 溯源摘要：写入生成块，方便回看「这段凭什么写出来」
    let mut context_sources = WritingContextSources::default();
    if !req.instruction.trim().is_empty() {
        context_sources.items.push(ContextSourceItem {
            kind: "instruction".into(),
            id: String::new(),
            title: "指令".into(),
            detail: take_chars_brief(req.instruction.trim(), 160),
        });
    }
    if !chapter.title.trim().is_empty() || !chapter.summary.trim().is_empty() {
        context_sources.items.push(ContextSourceItem {
            kind: "outline".into(),
            id: chapter.id.clone(),
            title: if chapter.title.trim().is_empty() {
                "本章".into()
            } else {
                chapter.title.clone()
            },
            detail: take_chars_brief(chapter.summary.trim(), 100),
        });
    }
    if let Some(pov) = chapter.pov_lore_id.as_ref().filter(|s| !s.is_empty()) {
        let (title, detail) = if let Some(e) = all_lore.iter().find(|e| &e.id == pov) {
            (e.title.clone(), e.kind.clone())
        } else {
            (pov.clone(), "pov".into())
        };
        context_sources.items.push(ContextSourceItem {
            kind: "pov".into(),
            id: pov.clone(),
            title,
            detail,
        });
    }
    for arc_id in &chapter.focus_arc_ids {
        if let Some(arc) = plot.arcs.iter().find(|a| &a.id == arc_id) {
            context_sources.items.push(ContextSourceItem {
                kind: "arc".into(),
                id: arc.id.clone(),
                title: arc.title.clone(),
                detail: take_chars_brief(&arc.goal, 80),
            });
        }
    }
    if !chapter.must_do.trim().is_empty() {
        context_sources.items.push(ContextSourceItem {
            kind: "must_do".into(),
            id: String::new(),
            title: "必达".into(),
            detail: take_chars_brief(chapter.must_do.trim(), 120),
        });
    }
    for b in chapter.beats.iter().take(6) {
        if b.title.trim().is_empty() {
            continue;
        }
        context_sources.items.push(ContextSourceItem {
            kind: "beat".into(),
            id: b.id.clone(),
            title: b.title.clone(),
            detail: take_chars_brief(&b.purpose, 60),
        });
    }
    for e in &lore {
        context_sources.items.push(ContextSourceItem {
            kind: "lore".into(),
            id: e.id.clone(),
            title: e.title.clone(),
            detail: e.kind.clone(),
        });
    }
    if should_inject_selected_tropes(&task) {
        for e in &selected_tropes {
            context_sources.items.push(ContextSourceItem {
                kind: e.kind.clone(),
                id: e.id.clone(),
                title: e.title.clone(),
                detail: take_chars_brief(e.content.trim(), 80),
            });
        }
    }

    Ok(AssembledWriting {
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: match task {
                    WritingTask::OutlineToBeats
                    | WritingTask::OutlineToChapters
                    | WritingTask::OutlineToMindmap
                    | WritingTask::SectionPlan
                    | WritingTask::CastExtract
                    | WritingTask::TropeExtract
                    | WritingTask::StorySync
                    | WritingTask::BeatsToStoryboard
                    | WritingTask::ContentToImagePrompt => {
                        "你是小说结构助理，只输出规定 JSON，不要解释。".into()
                    }
                    WritingTask::BlockDigest | WritingTask::ChapterSummary => {
                        "你是小说记忆助理，只输出规定篇幅的剧情摘要。".into()
                    }
                    WritingTask::Consistency => {
                        "你是小说设定审查助手，输出问题列表。".into()
                    }
                    _ => "你是本地小说创作助手，严格遵循用户模板中的硬规则。动作主体必须清晰：谁对谁做什么、谁插入、谁射精、射在何处；禁止主宾颠倒或对白与动作矛盾。禁止复读循环。".into(),
                },
            },
            ChatMessage {
                role: "user".into(),
                content: user,
            },
        ],
        context_sources,
    })
}

fn visual_sheets_for_prompt(root: &Path) -> String {
    let lore = project::list_lore(root).unwrap_or_default();
    let keys = ["外貌", "发型", "瞳色", "体态", "常服", "画风锚"];
    let mut lines: Vec<String> = Vec::new();
    for e in &lore {
        if e.kind != "character" {
            continue;
        }
        let mut bits: Vec<String> = vec![e.title.clone()];
        for k in keys {
            if let Some(v) = e.attrs.get(k) {
                let t = v.trim();
                if !t.is_empty() {
                    bits.push(format!("{k}：{t}"));
                }
            }
        }
        let brief = take_chars_brief(&e.content, 80);
        if bits.len() == 1 && brief.is_empty() {
            continue;
        }
        if !brief.is_empty() {
            bits.push(brief);
        }
        lines.push(bits.join("；"));
    }
    if lines.is_empty() {
        "（无锁定形象卡）".into()
    } else {
        lines.join("\n")
    }
}

/// 收集本篇 + 挂接库中的角色名/别称，供 cast_extract 去重
fn collect_known_character_names(root: &Path, project: &project::NovelProject) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut push_name = |s: &str| {
        let t = s.trim();
        if t.is_empty() {
            return;
        }
        // 去掉 [书名] 前缀后再记一份裸名
        let bare = if let Some(rest) = t.strip_prefix('[') {
            rest.find(']')
                .map(|i| rest[i + 1..].trim())
                .filter(|x| !x.is_empty())
                .unwrap_or(t)
        } else {
            t
        };
        for n in [t, bare] {
            if !names.iter().any(|x| x == n) {
                names.push(n.to_string());
            }
        }
    };

    let mut push_entry = |e: &LoreEntry| {
        if e.kind != "character" {
            return;
        }
        push_name(&e.title);
        for k in &e.keywords {
            let k = k.trim();
            if k.is_empty() {
                continue;
            }
            if let Some(alias) = k.strip_prefix("alias:") {
                push_name(alias);
            } else if !k.contains('=') {
                push_name(k);
            }
        }
    };

    if let Ok(local) = project::list_lore(root) {
        for e in &local {
            push_entry(e);
        }
    }
    for link in &project.linked_kb_roots {
        let kb_path = match crate::kb::resolve_kb_root(link) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Ok(entries) = project::list_lore(&kb_path) {
            for e in &entries {
                push_entry(e);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// 收集本篇 + 全局库 + 挂接库中的情节/性癖条目，供 digest / 写后抽取对照
fn collect_known_trope_entries(root: &Path, project: &project::NovelProject) -> Vec<LoreEntry> {
    let mut out: Vec<LoreEntry> = Vec::new();
    let mut push_entry = |e: &LoreEntry| {
        if !is_trope_kind(&e.kind) {
            return;
        }
        if e.title.trim().is_empty() {
            return;
        }
        if out.iter().any(|x| x.id == e.id && !e.id.is_empty()) {
            return;
        }
        out.push(e.clone());
    };
    if let Ok(local) = project::list_lore(root) {
        for e in &local {
            push_entry(e);
        }
    }
    for e in crate::kb::list_trope_library_entries() {
        push_entry(&e);
    }
    if let Ok(roster) = crate::kb::ensure_character_roster() {
        if let Ok(entries) = project::list_lore(&roster.root) {
            for e in &entries {
                push_entry(e);
            }
        }
    }
    for link in &project.linked_kb_roots {
        let kb_path = match crate::kb::resolve_kb_root(link) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Ok(entries) = project::list_lore(&kb_path) {
            for e in &entries {
                push_entry(e);
            }
        }
    }
    out
}

pub async fn run_writing(
    client: &LmStudioClient,
    settings: &AppSettings,
    req: &WritingRequest,
    cancel: Option<Arc<AtomicBool>>,
    mut on_delta: impl FnMut(&str),
) -> AppResult<WritingOutcome> {
    let task = WritingTask::from_str_loose(&req.task)?;
    if task == WritingTask::ChapterSummary {
        let body = project::read_chapter(Path::new(&req.project_root), &req.chapter_id)
            .map(|(_, c)| c)
            .unwrap_or_default();
        let n = body.chars().filter(|c| !c.is_whitespace()).count();
        if n < 80 {
            return Err(AppError::t("errors.chapterSummaryEmptyBody"));
        }
    }
    let scores = if matches!(
        task,
        WritingTask::BlockDigest | WritingTask::CastExtract | WritingTask::TropeExtract | WritingTask::SectionPlan | WritingTask::OutlineToBeats | WritingTask::OutlineToChapters | WritingTask::OutlineToMindmap | WritingTask::BeatsToStoryboard | WritingTask::ContentToImagePrompt
    ) {
        None
    } else {
        let semantic = crate::rag::query_semantic_scores(
            client,
            settings,
            Path::new(&req.project_root),
            &format!("{} {} {}", req.instruction, req.selection, req.task),
        )
        .await
        .unwrap_or_default();
        if semantic.is_empty() {
            None
        } else {
            Some(semantic)
        }
    };
    let assembled = assemble_messages_with_scores(settings, req, scores.as_ref())?;
    let messages = assembled.messages;
    let context_sources = assembled.context_sources;
    let chapter_chars = project::read_chapter(Path::new(&req.project_root), &req.chapter_id)
        .map(|(_, c)| c.chars().count())
        .unwrap_or(0);
    let mut options = resolve_writing_options(settings, &task, req, chapter_chars);
    if matches!(
        task,
        WritingTask::OutlineToChapters | WritingTask::OutlineToMindmap
    ) {
        let prompt_chars: u32 = messages
            .iter()
            .map(|m| m.content.chars().count() as u32)
            .fold(0u32, |a, b| a.saturating_add(b));
        options.max_tokens = Some(outline_split_max_tokens(
            prompt_chars,
            options.max_tokens.unwrap_or(1024),
        ));
    }
    let cancel = cancel.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    let primary_model = options
        .model
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| settings.writing_model().to_string());

    // 指定了非默认模型时先极短探测，避免未加载大模空耗数分钟
    let mut skip_primary = false;
    let mut probe_err: Option<String> = None;
    if let Some(fb) = resolve_fallback_model(settings, req, &primary_model) {
        if primary_model != fb {
            match client.probe_model(settings, &primary_model).await {
                Ok(()) => {}
                Err(e) => {
                    probe_err = Some(e.to_string());
                    skip_primary = true;
                    eprintln!(
                        "[writing] 探测模型 `{primary_model}` 失败，直接回退 `{fb}`：{}",
                        probe_err.as_deref().unwrap_or("")
                    );
                    options.model = Some(fb);
                }
            }
        }
    }

    let (chat_result, model_used, fallback_from) = if skip_primary {
        let fb_model = options.model.clone().unwrap_or_else(|| primary_model.clone());
        let r = client
            .chat_stream(settings, &messages, &options, cancel.clone(), &mut on_delta)
            .await
            .map_err(|e2| {
                AppError::t_fmt(
                    "errors.primaryProbeAndFallbackFailed",
                    &[
                        ("primary", &primary_model),
                        ("probe", &probe_err.unwrap_or_else(|| "unknown".into())),
                        ("fallback", &fb_model),
                        ("e", &e2.to_string()),
                    ],
                )
            })?;
        (r, fb_model, Some(primary_model))
    } else {
        match client
            .chat_stream(settings, &messages, &options, cancel.clone(), &mut on_delta)
            .await
        {
            Ok(r) => (r, primary_model.clone(), None),
            Err(e) => {
                let fb = resolve_fallback_model(settings, req, &primary_model);
                if let Some(fb_model) = fb {
                    eprintln!(
                        "[writing] 模型 `{primary_model}` 失败：{e}；回退到 `{fb_model}`"
                    );
                    options.model = Some(fb_model.clone());
                    let r = client
                        .chat_stream(settings, &messages, &options, cancel.clone(), &mut on_delta)
                        .await
                        .map_err(|e2| {
                            AppError::t_fmt(
                                "errors.primaryAndFallbackFailed",
                                &[
                                    ("primary", &primary_model),
                                    ("e", &e.to_string()),
                                    ("fallback", &fb_model),
                                    ("e2", &e2.to_string()),
                                ],
                            )
                        })?;
                    (r, fb_model, Some(primary_model))
                } else {
                    return Err(e);
                }
            }
        }
    };

    let mut usage = chat_result.usage;
    let mut text = chat_result.text;
    let mut truncated = false;
    let mut loop_retried = false;
    let mut raw_text = text.clone();
    if matches!(
        task,
        WritingTask::Continue
            | WritingTask::SameSlotVariant
            | WritingTask::Polish
            | WritingTask::Outline
    ) {
        let existing = project::read_chapter(Path::new(&req.project_root), &req.chapter_id)
            .ok()
            .map(|(_, c)| c);
        let raw_len = text.chars().count();
        // 复读检测仅用于重试与日志标注；交付/预览/插入一律保留原文，禁止静默截断
        let (clean, was_trunc) =
            dedupe::sanitize_generation(&text, existing.as_deref());
        truncated = was_trunc;
        if was_trunc && raw_len > 0 {
            eprintln!(
                "[writing] dup-detect continue (keep raw): raw_chars={raw_len} clean_chars={} substantial={}",
                clean.chars().count(),
                dedupe::has_substantial_content(&clean)
            );
        }

        let allow_retry = req
            .retry_on_loop
            .unwrap_or(settings.writing_retry_on_loop);
        if truncated
            && allow_retry
            && matches!(task, WritingTask::Continue)
            && clean.chars().count() < 120
        {
            loop_retried = true;
            let mut retry_opts = options.clone();
            let base_temp = retry_opts
                .temperature
                .unwrap_or(settings.temperature)
                .max(0.2);
            retry_opts.temperature = Some((base_temp * 0.75).max(0.25));
            let base_fp = retry_opts
                .frequency_penalty
                .unwrap_or(settings.frequency_penalty);
            retry_opts.frequency_penalty = Some((base_fp + 0.25).min(1.5));
            let base_mt = retry_opts.max_tokens.unwrap_or(settings.max_tokens);
            retry_opts.max_tokens = Some(base_mt.min(700).max(256));

            let mut retry_req = req.clone();
            if !retry_req.instruction.contains("禁止复述") {
                retry_req.instruction = format!(
                    "{}；严禁复述前文任何句子；在同一场景推进并写满规定字数（可超出）；禁止重复已出现的收束动作。",
                    retry_req.instruction
                );
            }
            let retry_assembled =
                assemble_messages_with_scores(settings, &retry_req, scores.as_ref())?;
            let retry_messages = retry_assembled.messages;
            if let Ok(r2) = client
                .chat_stream(
                    settings,
                    &retry_messages,
                    &retry_opts,
                    cancel.clone(),
                    &mut on_delta,
                )
                .await
            {
                let (clean2, trunc2) =
                    dedupe::sanitize_generation(&r2.text, existing.as_deref());
                // 重试更长（按原文）或检测更干净时采用重试原文
                if r2.text.chars().count() > text.chars().count()
                    || clean2.chars().count() > clean.chars().count()
                    || !trunc2
                {
                    raw_text = r2.text;
                    truncated = trunc2;
                    usage.prompt_tokens =
                        usage.prompt_tokens.saturating_add(r2.usage.prompt_tokens);
                    usage.completion_tokens = usage
                        .completion_tokens
                        .saturating_add(r2.usage.completion_tokens);
                    usage.total_tokens =
                        usage.prompt_tokens.saturating_add(usage.completion_tokens);
                    if usage.source == "api" && r2.usage.source != "api" {
                        /* keep api if first was api */
                    } else if usage.source != "api" {
                        usage.source = r2.usage.source;
                    }
                }
            }
        }
        // 无论是否检测复读，交付文本均为原文
        text = raw_text.clone();
    }

    // 定稿清洗否定对照口癖（跨模型保底）
    if settings.writing_strip_rhetoric
        && matches!(
            task,
            WritingTask::Continue
                | WritingTask::SameSlotVariant
                | WritingTask::Polish
                | WritingTask::Outline
        )
    {
        let cleaned = rhetoric::sanitize_rhetoric(&text);
        if cleaned != text {
            eprintln!(
                "[writing] rhetoric-strip: {} → {} chars",
                text.chars().count(),
                cleaned.chars().count()
            );
            text = cleaned.clone();
            raw_text = cleaned;
        }
    }

    // 续写 / 同位置重写：先补规定字数下限，再补半句/按纲未收束（下限可超）
    if matches!(
        task,
        WritingTask::Continue | WritingTask::SameSlotVariant
    ) {
        let min_chars = settings.resolve_writing_target_chars() as usize;
        const MAX_FILLS: u32 = 6;
        const OUTLINE_CHAPTER_CHAR_CAP: usize = 8000;
        let outline_run = request_is_outline_run(req);
        let fill_ctx = build_length_fill_context(settings, req);
        let mut fill_i = 0u32;
        while fill_i < MAX_FILLS {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if outline_run && text.chars().count() >= OUTLINE_CHAPTER_CHAR_CAP {
                eprintln!(
                    "[writing] completeness cap {} chars; stop",
                    text.chars().count()
                );
                break;
            }
            let have = text.chars().count();
            let need = min_chars.saturating_sub(have);
            let need_length = have < min_chars;
            let need_sentence = continuity::prose_incomplete(&text);
            let need_outline = outline_run
                && !need_length
                && continuity::outline_hook_missing(&text, &fill_ctx.outline);
            let mode = if need_length {
                "length"
            } else if need_sentence {
                "sentence"
            } else if need_outline {
                "outline"
            } else {
                break;
            };
            fill_i += 1;
            eprintln!(
                "[writing] completeness-fill {fill_i}/{MAX_FILLS} mode={mode} have={have} min={min_chars}"
            );

            let sep = if text.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            on_delta(sep);
            text.push_str(sep);
            raw_text.push_str(sep);

            let instr = if req.instruction.trim().is_empty() {
                "（无额外指令；承接已写正文在同一场景继续）"
            } else {
                req.instruction.as_str()
            };
            let goal = completeness_goal(&settings.writing_locale, mode);
            let fill_user = if mode == "length" {
                render_template(
                    crate::prompt_i18n::prompt("length_fill.md"),
                    &[
                        ("min_chars", &min_chars.to_string()),
                        ("have_chars", &have.to_string()),
                        ("need_chars", &need.to_string()),
                        ("draft", &text),
                        ("instruction", instr),
                        ("outline", &fill_ctx.outline),
                        ("must_do", &fill_ctx.must_do),
                        ("direction_anchor", &fill_ctx.direction_anchor),
                        ("active_beat", &fill_ctx.active_beat),
                        ("tropes", &fill_ctx.tropes),
                    ],
                )
            } else {
                render_template(
                    crate::prompt_i18n::prompt("scene_complete.md"),
                    &[
                        ("goal", &goal),
                        ("draft", &text),
                        ("instruction", instr),
                        ("outline", &fill_ctx.outline),
                        ("must_do", &fill_ctx.must_do),
                        ("direction_anchor", &fill_ctx.direction_anchor),
                        ("tropes", &fill_ctx.tropes),
                    ],
                )
            };
            let fill_messages = vec![
                ChatMessage {
                    role: "system".into(),
                    content: "你是小说写作助手。只输出正文续写，不要解释、不要标题。".into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: fill_user,
                },
            ];

            let mut fill_opts = options.clone();
            let fill_mt = if mode == "sentence" {
                512
            } else if mode == "outline" {
                2048
            } else {
                (((need as f64) * 2.0).ceil() as u32).max(800).min(32768)
            };
            fill_opts.max_tokens = Some(fill_mt);

            match client
                .chat_stream(
                    settings,
                    &fill_messages,
                    &fill_opts,
                    cancel.clone(),
                    &mut on_delta,
                )
                .await
            {
                Ok(r) => {
                    let piece = r.text.trim_start().to_string();
                    if piece.is_empty() {
                        eprintln!("[writing] completeness-fill empty; stop");
                        break;
                    }
                    let append = strip_draft_overlap(&text, &piece);
                    if append.trim().is_empty() {
                        eprintln!("[writing] completeness-fill no new content; stop");
                        break;
                    }
                    if !text.ends_with('\n') && !append.starts_with('\n') {
                        on_delta("\n");
                        text.push('\n');
                        raw_text.push('\n');
                    }
                    text.push_str(&append);
                    raw_text.push_str(&append);
                    usage.prompt_tokens =
                        usage.prompt_tokens.saturating_add(r.usage.prompt_tokens);
                    usage.completion_tokens = usage
                        .completion_tokens
                        .saturating_add(r.usage.completion_tokens);
                    usage.total_tokens =
                        usage.prompt_tokens.saturating_add(usage.completion_tokens);
                    if usage.source != "api" {
                        usage.source = r.usage.source;
                    }
                    if r.usage.completion_tokens >= fill_mt && continuity::prose_incomplete(&text) {
                        continue;
                    }
                }
                Err(e) => {
                    eprintln!("[writing] completeness-fill failed: {e}");
                    break;
                }
            }
        }
        if text.chars().count() < min_chars {
            eprintln!(
                "[writing] completeness-fill ended short: {} < {}",
                text.chars().count(),
                min_chars
            );
        }
    }

    if task == WritingTask::ChapterSummary {
        let root = Path::new(&req.project_root);
        let source = project::read_chapter(root, &req.chapter_id)
            .map(|(_, c)| c)
            .unwrap_or_default();
        let memory = project::load_memory(root).unwrap_or_default();
        let note = memory
            .block_notes
            .iter()
            .filter(|n| n.chapter_id == req.chapter_id)
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
            .map(|n| n.summary.as_str())
            .unwrap_or("");
        if continuity::chapter_summary_is_dump(&text, &source) {
            eprintln!(
                "[writing] chapter_summary dump/overlong ({} chars); compress",
                text.chars().count()
            );
            let rescued = continuity::fallback_chapter_summary(&text, &source, note);
            if rescued.trim().is_empty() || continuity::is_dump_placeholder(&rescued) {
                return Err(AppError::t("errors.chapterSummaryEmptyBody"));
            }
            text = rescued.clone();
            raw_text = rescued;
        }
        project::upsert_chapter_snapshot(root, &req.chapter_id, &text)?;
    }

    if task == WritingTask::BlockDigest {
        text = project::sanitize_block_digest(&text);
        let key = if req.block_key.trim().is_empty() {
            format!(
                "orphan-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            )
        } else {
            req.block_key.trim().to_string()
        };
        let _ = project::append_block_note(
            Path::new(&req.project_root),
            &req.chapter_id,
            &key,
            &text,
        )?;
    }

    Ok(WritingOutcome {
        text,
        raw_text,
        model_used,
        fallback_from,
        truncated,
        loop_retried,
        usage,
        prompt_messages: messages,
        log_id: String::new(),
        context_sources,
    })
}

struct LengthFillContext {
    outline: String,
    must_do: String,
    direction_anchor: String,
    active_beat: String,
    tropes: String,
}

fn build_length_fill_context(settings: &AppSettings, req: &WritingRequest) -> LengthFillContext {
    let root = Path::new(&req.project_root);
    let Ok((chapter, content)) = project::read_chapter(root, &req.chapter_id) else {
        return LengthFillContext {
            outline: "（无）".into(),
            must_do: "（无）".into(),
            direction_anchor: "（无）".into(),
            active_beat: "（无）".into(),
            tropes: "（无）".into(),
        };
    };
    let mut beat_progress = project::load_beat_progress(root, &req.chapter_id).unwrap_or_default();
    beat_progress = beat_engine::load_or_init_progress(&chapter.beats, beat_progress);
    if let Some(ref bid) = req.active_beat_id {
        if !bid.trim().is_empty() {
            beat_engine::ensure_active_beat(&chapter.beats, &mut beat_progress, bid.trim());
        }
    }
    let outline_run = request_is_outline_run(req);
    let active_beat_ref = req
        .active_beat_id
        .as_ref()
        .and_then(|id| chapter.beats.iter().find(|b| b.id == *id))
        .or_else(|| beat_engine::active_beat(&chapter.beats, &beat_progress));
    let beat_status = if chapter.beats.is_empty() {
        advance::build_beat_status_legacy(&chapter.beats, &content)
    } else {
        advance::build_beat_status(&chapter.beats, &beat_progress)
    };
    let recent = take_tail(&content, settings.recent_window_chars.min(800));
    let ban_list = advance::build_dynamic_ban_list(&recent, &chapter.must_not);
    let instruction = if req.instruction.is_empty() {
        "（无额外指令）"
    } else {
        req.instruction.as_str()
    };
    let direction_anchor = advance::build_direction_anchor(
        &chapter.must_do,
        instruction,
        &beat_status,
        &ban_list,
        active_beat_ref,
        outline_run,
    );
    let outline = if chapter.summary.is_empty() {
        chapter.title.clone()
    } else {
        format!("{}\n{}", chapter.title, chapter.summary)
    };
    let tropes = {
        let mut pool: Vec<LoreEntry> = project::list_lore(root).unwrap_or_default();
        if let Ok(opened) = project::open_project(root) {
            for link in &opened.project.linked_kb_roots {
                if let Ok(kb_path) = crate::kb::resolve_kb_root(link) {
                    if let Ok(entries) = project::list_lore(&kb_path) {
                        pool.extend(entries);
                    }
                }
            }
        }
        pool.extend(crate::kb::list_trope_library_entries());
        pool = project::coalesce_unique_lore(pool);
        let ids = resolve_selected_trope_ids(req, &chapter);
        tropes_to_text(&collect_selected_tropes(&pool, &ids))
    };
    LengthFillContext {
        outline,
        must_do: if chapter.must_do.is_empty() {
            "（无）".into()
        } else {
            chapter.must_do.clone()
        },
        direction_anchor,
        active_beat: active_beat_ref
            .map(beat_engine::beat_summary)
            .unwrap_or_else(|| "（无）".into()),
        tropes,
    }
}

/// 读取章节节拍进度（与 beats 对齐）
pub fn beat_progress_get(root: &Path, chapter_id: &str) -> AppResult<project::ChapterBeatProgress> {
    let opened = project::open_project(root)?;
    let chapter = opened
        .project
        .chapters
        .iter()
        .find(|c| c.id == chapter_id)
        .ok_or_else(|| AppError::t("errors.chapterMissing"))?;
    let stored = project::load_beat_progress(root, chapter_id)?;
    Ok(beat_engine::load_or_init_progress(&chapter.beats, stored))
}

/// 标记节拍完成并落盘
pub fn beat_progress_advance(
    root: &Path,
    chapter_id: &str,
    beat_id: &str,
) -> AppResult<project::ChapterBeatProgress> {
    let opened = project::open_project(root)?;
    let chapter = opened
        .project
        .chapters
        .iter()
        .find(|c| c.id == chapter_id)
        .ok_or_else(|| AppError::t("errors.chapterMissing"))?
        .clone();
    let mut progress = project::load_beat_progress(root, chapter_id)?;
    progress = beat_engine::load_or_init_progress(&chapter.beats, progress);
    beat_engine::mark_completed(&mut progress, &chapter.beats, beat_id);
    project::save_beat_progress(root, chapter_id, &progress)?;
    Ok(progress)
}

pub fn beat_progress_reset(root: &Path, chapter_id: &str) -> AppResult<()> {
    project::reset_beat_progress(root, chapter_id)
}

pub fn beat_progress_skip(
    root: &Path,
    chapter_id: &str,
    beat_id: &str,
) -> AppResult<project::ChapterBeatProgress> {
    let opened = project::open_project(root)?;
    let chapter = opened
        .project
        .chapters
        .iter()
        .find(|c| c.id == chapter_id)
        .ok_or_else(|| AppError::t("errors.chapterMissing"))?
        .clone();
    let mut progress = project::load_beat_progress(root, chapter_id)?;
    progress = beat_engine::load_or_init_progress(&chapter.beats, progress);
    beat_engine::mark_skipped(&mut progress, &chapter.beats, beat_id);
    project::save_beat_progress(root, chapter_id, &progress)?;
    Ok(progress)
}

fn resolve_fallback_model(
    settings: &AppSettings,
    req: &WritingRequest,
    primary: &str,
) -> Option<String> {
    if !settings.writing_model_fallback {
        return None;
    }
    let candidate = req
        .fallback_model
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let m = settings.writing_model().trim();
            if m.is_empty() {
                None
            } else {
                Some(m.to_string())
            }
        })?;
    if candidate == primary {
        None
    } else {
        Some(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_known_tropes_compact, outline_continue_max_tokens, outline_split_max_tokens,
    };
    use crate::project::LoreEntry;

    fn dummy_lore(id: &str, kind: &str, title: &str) -> LoreEntry {
        LoreEntry {
            id: id.into(),
            kind: kind.into(),
            title: title.into(),
            content: String::new(),
            keywords: vec![],
            links: vec![],
            attrs: Default::default(),
            sources: vec![],
            unique: false,
            updated_at: String::new(),
        }
    }

    #[test]
    fn split_tokens_floor_on_short_prompt() {
        assert_eq!(outline_split_max_tokens(100, 1024), 2048);
    }

    #[test]
    fn split_tokens_grows_with_long_outline() {
        let n = outline_split_max_tokens(12_000, 3240);
        assert!(n >= 7200, "got {n}");
        assert!(n <= 16384);
    }

    #[test]
    fn split_tokens_keeps_higher_request() {
        assert_eq!(outline_split_max_tokens(100, 9000), 9000);
    }

    #[test]
    fn continue_tokens_raise_but_cap() {
        let n = outline_continue_max_tokens(1844, 800);
        assert!(n >= 1844, "got {n}");
        assert!(n <= 8192);
        assert_eq!(outline_continue_max_tokens(7000, 50_000), 8192);
    }

    #[test]
    fn compact_catalog_empty() {
        assert_eq!(format_known_tropes_compact(&[]), "（无）");
    }

    #[test]
    fn compact_catalog_appends_newest_at_end() {
        let old = dummy_lore("1", "kink", "旧卡");
        let newer = dummy_lore("2", "kink", "新卡");
        // 库文件最新在前
        let before = format_known_tropes_compact(std::slice::from_ref(&old));
        let after = format_known_tropes_compact(&[newer, old]);
        assert_eq!(before, "kink: 旧卡");
        assert!(
            after.starts_with("kink: 旧卡"),
            "prefix must stay stable, got {after}"
        );
        assert!(
            after.ends_with("新卡"),
            "newest title appends at end, got {after}"
        );
    }

    #[test]
    fn compact_catalog_groups_kinds() {
        let t = dummy_lore("t", "trope", "套路甲");
        let k = dummy_lore("k", "kink", "玩法乙");
        let s = format_known_tropes_compact(&[t, k]);
        assert!(s.contains("kink: 玩法乙"));
        assert!(s.contains("trope: 套路甲"));
        let kink_at = s.find("kink:").unwrap();
        let trope_at = s.find("trope:").unwrap();
        assert!(kink_at < trope_at);
    }
}
