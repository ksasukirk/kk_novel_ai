//! 写作上下文与任务
//! 代码路径: kk_novel_ai/src-tauri/src/writing/mod.rs

pub mod advance;
pub mod beat_engine;
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
    /** 先分析需要几节，再由前端排队续写 */
    SectionPlan,
    /** 从章纲 summary 拆成 beats JSON */
    OutlineToBeats,
    /** 从全书大纲 book_outline 拆成章节列表 JSON */
    OutlineToChapters,
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
            "section_plan" | "plan_sections" => Ok(Self::SectionPlan),
            "outline_to_beats" | "split_beats" => Ok(Self::OutlineToBeats),
            "outline_to_chapters" | "split_chapters" => Ok(Self::OutlineToChapters),
            _ => Err(AppError::msg(format!("未知写作任务: {s}"))),
        }
    }

    fn template(&self) -> &'static str {
        match self {
            Self::Continue => include_str!("../../prompts/continue_chapter.md"),
            Self::SameSlotVariant => include_str!("../../prompts/same_slot_variant.md"),
            Self::Polish => include_str!("../../prompts/polish.md"),
            Self::Outline => include_str!("../../prompts/outline_expand.md"),
            Self::Consistency => include_str!("../../prompts/consistency_check.md"),
            Self::ChapterSummary => include_str!("../../prompts/chapter_summary.md"),
            Self::StorySync => include_str!("../../prompts/story_sync.md"),
            Self::BlockDigest => include_str!("../../prompts/block_digest.md"),
            Self::CastExtract => include_str!("../../prompts/cast_extract.md"),
            Self::SectionPlan => include_str!("../../prompts/section_plan.md"),
            Self::OutlineToBeats => include_str!("../../prompts/outline_to_beats.md"),
            Self::OutlineToChapters => include_str!("../../prompts/outline_to_chapters.md"),
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
            Self::SectionPlan => "section_plan",
            Self::OutlineToBeats => "outline_to_beats",
            Self::OutlineToChapters => "outline_to_chapters",
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
                | Self::SectionPlan
                | Self::OutlineToBeats
                | Self::OutlineToChapters
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
    /// outline_to_chapters：full | append
    #[serde(default)]
    pub split_mode: Option<String>,
}

/// 单次生成实际注入的上下文来源（挂到生成块，便于回看情节依据）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextSourceItem {
    /// instruction | outline | pov | arc | must_do | lore | beat
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
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    chars[chars.len() - max_chars..].iter().collect()
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
    let meta_sum = prev_ch.summary.trim();
    let summary = snap
        .or(note)
        .unwrap_or_else(|| {
            if meta_sum.is_empty() {
                "（无摘要）".into()
            } else {
                meta_sum.to_string()
            }
        });
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
    let max_tokens = if matches!(task, WritingTask::BlockDigest | WritingTask::CastExtract) {
        req.max_tokens.or(Some(512))
    } else if analysis {
        req.max_tokens
            .or(Some(settings.max_tokens.min(2048).max(512)))
    } else {
        let aligned = if matches!(task, WritingTask::SameSlotVariant) {
            // 与设定续写字数对齐（不再跟 selection/总结长度走）
            same_slot_max_tokens(settings.resolve_writing_target_chars(), 0)
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
            memory.rolling_summary.clone()
        };
        let instruction = if req.instruction.is_empty() {
            "（无）"
        } else {
            req.instruction.as_str()
        };
        let user = render_template(
            task.template(),
            &[
                ("prev_memory", &prev_memory),
                ("block_text", &block_text),
                ("instruction", instruction),
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
    all_lore = project::coalesce_unique_lore(ranked);
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
        "{} {} {} {} {}",
        chapter.title, chapter.summary, chapter.must_do, req.instruction, req.selection
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
    if lore.len() > 10 {
        lore.truncate(10);
    }

    let recent = match task {
        WritingTask::Polish => req.selection.clone(),
        WritingTask::SameSlotVariant => {
            // 不注入章节前文；上节总结如有则已在 selection / instruction
            String::new()
        }
        WritingTask::Consistency | WritingTask::StorySync => {
            take_tail(&content, settings.recent_window_chars.saturating_mul(2))
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
        WritingTask::OutlineToChapters => String::new(),
        WritingTask::BlockDigest | WritingTask::CastExtract => req.selection.clone(),
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
    let outline_run = req
        .active_beat_id
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
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
        character_gender_lock(&opened.project, &lore)
    } else {
        "（无）".into()
    };
    // 拆全书大纲：尽量不灌设定，避免模型用角色仓/lore 改写用户意愿
    let (plot_text, timeline_text, relations_text, canon_text, lore_text, memory_text) =
        if task == WritingTask::OutlineToChapters {
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
                memory.rolling_summary.clone()
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
    } else if !req.instruction.trim().is_empty() && task == WritingTask::OutlineToChapters {
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
                if task != WritingTask::OutlineToChapters {
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

    let mut user = render_template(
        task.template(),
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

    Ok(AssembledWriting {
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: match task {
                    WritingTask::OutlineToBeats
                    | WritingTask::OutlineToChapters
                    | WritingTask::SectionPlan
                    | WritingTask::CastExtract
                    | WritingTask::StorySync => {
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

pub async fn run_writing(
    client: &LmStudioClient,
    settings: &AppSettings,
    req: &WritingRequest,
    cancel: Option<Arc<AtomicBool>>,
    mut on_delta: impl FnMut(&str),
) -> AppResult<WritingOutcome> {
    let task = WritingTask::from_str_loose(&req.task)?;
    let scores = if matches!(
        task,
        WritingTask::BlockDigest | WritingTask::CastExtract | WritingTask::SectionPlan | WritingTask::OutlineToBeats | WritingTask::OutlineToChapters
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
                AppError::msg(format!(
                    "主模型 `{primary_model}` 探测失败：{}；回退 `{fb_model}` 亦失败：{e2}",
                    probe_err.unwrap_or_else(|| "unknown".into())
                ))
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
                            AppError::msg(format!(
                                "主模型 `{primary_model}` 失败：{e}；回退 `{fb_model}` 亦失败：{e2}"
                            ))
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

    // 续写 / 同位置重写：不足规定字数则自动补写，直到达标或达上限次数（允许超出）
    if matches!(
        task,
        WritingTask::Continue | WritingTask::SameSlotVariant
    ) {
        let min_chars = settings.resolve_writing_target_chars() as usize;
        const MAX_LENGTH_FILLS: u32 = 4;
        let mut fill_i = 0u32;
        while text.chars().count() < min_chars && fill_i < MAX_LENGTH_FILLS {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            fill_i += 1;
            let have = text.chars().count();
            let need = min_chars.saturating_sub(have);
            eprintln!(
                "[writing] length-fill {fill_i}/{MAX_LENGTH_FILLS}: have={have} need≈{need} min={min_chars}"
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
            let fill_ctx = build_length_fill_context(settings, req);
            let fill_user = render_template(
                include_str!("../../prompts/length_fill.md"),
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
                ],
            );
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
            let fill_mt = (((need as f64) * 2.0).ceil() as u32).max(800).min(32768);
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
                        eprintln!("[writing] length-fill empty; stop");
                        break;
                    }
                    let append = strip_draft_overlap(&text, &piece);
                    if append.trim().is_empty() {
                        eprintln!("[writing] length-fill no new content; stop");
                        break;
                    }
                    if !text.ends_with('\n') && !append.starts_with('\n') {
                        on_delta("\n");
                        text.push('\n');
                        raw_text.push('\n');
                    }
                    // 重叠裁掉的部分已在模型流里发出；定稿以裁后为准，预览可能略含回声，可接受
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
                }
                Err(e) => {
                    eprintln!("[writing] length-fill failed: {e}");
                    break;
                }
            }
        }
        if text.chars().count() < min_chars {
            eprintln!(
                "[writing] length-fill ended short: {} < {}",
                text.chars().count(),
                min_chars
            );
        }
    }

    if task == WritingTask::ChapterSummary {
        project::upsert_chapter_snapshot(Path::new(&req.project_root), &req.chapter_id, &text)?;
        let mut opened = project::open_project(Path::new(&req.project_root))?;
        if let Some(ch) = opened
            .project
            .chapters
            .iter_mut()
            .find(|c| c.id == req.chapter_id)
        {
            ch.summary = text.clone();
        }
        project::save_project_meta(Path::new(&req.project_root), &opened.project)?;
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
}

fn build_length_fill_context(settings: &AppSettings, req: &WritingRequest) -> LengthFillContext {
    let root = Path::new(&req.project_root);
    let Ok((chapter, content)) = project::read_chapter(root, &req.chapter_id) else {
        return LengthFillContext {
            outline: "（无）".into(),
            must_do: "（无）".into(),
            direction_anchor: "（无）".into(),
            active_beat: "（无）".into(),
        };
    };
    let mut beat_progress = project::load_beat_progress(root, &req.chapter_id).unwrap_or_default();
    beat_progress = beat_engine::load_or_init_progress(&chapter.beats, beat_progress);
    if let Some(ref bid) = req.active_beat_id {
        if !bid.trim().is_empty() {
            beat_engine::ensure_active_beat(&chapter.beats, &mut beat_progress, bid.trim());
        }
    }
    let outline_run = req
        .active_beat_id
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
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
        .ok_or_else(|| AppError::msg("章节不存在"))?;
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
        .ok_or_else(|| AppError::msg("章节不存在"))?
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
        .ok_or_else(|| AppError::msg("章节不存在"))?
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
