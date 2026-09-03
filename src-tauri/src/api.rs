//! 共享业务 API（Tauri 与 CLI 共用）
//! 代码路径: kk_novel_ai/src-tauri/src/api.rs

use crate::error::{AppError, AppResult};
use crate::export;
use crate::genlog;
use crate::import::{self, ApplyMode};
use crate::llm::{ChatMessage, ChatOptions, LmStudioClient};
use crate::project::{self, LoreEntry};
use crate::settings::{self, AppSettings};
use crate::writing::{self, WritingRequest};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub fn settings_get() -> AppResult<Value> {
    let settings = settings::load_settings()?;
    let peak = settings.resolve_deepseek_peak();
    Ok(json!({
        "ok": true,
        "settings": settings,
        "deepseek_peak_now": settings.is_deepseek() && peak,
        "deepseek_peak_notice": settings.deepseek_peak_notice(),
    }))
}

pub fn settings_save(settings: AppSettings) -> AppResult<Value> {
    settings::validate_for_platform(&settings)?;
    settings::save_settings(&settings)?;
    Ok(json!({ "ok": true, "settings": settings }))
}

pub async fn llm_health() -> AppResult<Value> {
    let s = settings::load_settings()?;
    LmStudioClient::new().health(&s).await
}

pub async fn llm_list_models() -> AppResult<Value> {
    let s = settings::load_settings()?;
    let body = LmStudioClient::new().list_models(&s).await?;
    Ok(json!({ "ok": true, "models": body }))
}

pub async fn llm_chat(messages: Vec<ChatMessage>, options: ChatOptions) -> AppResult<Value> {
    let s = settings::load_settings()?;
    let r = LmStudioClient::new().chat(&s, &messages, &options).await?;
    Ok(json!({ "ok": true, "text": r.text, "usage": r.usage }))
}

fn log_and_enrich_outcome(
    req: &WritingRequest,
    mut out: writing::WritingOutcome,
    source: &str,
    settings: &AppSettings,
) -> writing::WritingOutcome {
    let mut entry = genlog::make_entry_full(
        &req.task,
        &req.project_root,
        &req.chapter_id,
        &out.raw_text,
        &out.text,
        source,
        out.truncated,
        &out.model_used,
        &req.instruction,
        &out.prompt_messages,
        Some(out.usage.clone()),
        settings,
    );
    entry.context_sources = serde_json::to_value(&out.context_sources).ok();
    out.log_id = entry.id.clone();
    if let Some(u) = entry.usage.clone() {
        out.usage = u;
    }
    let _ = genlog::append_log(&entry);
    out
}

fn writing_outcome_json(out: &writing::WritingOutcome, task: &str, settings: &AppSettings) -> Value {
    json!({
        "ok": true,
        "text": out.text,
        "raw_text": out.raw_text,
        "task": task,
        "model_used": out.model_used,
        "fallback_from": out.fallback_from,
        "truncated": out.truncated,
        "loop_retried": out.loop_retried,
        "usage": out.usage,
        "log_id": out.log_id,
        "cost_cny": crate::usage::calc_cost_cny(&out.usage, settings, &out.model_used),
        "context_sources": out.context_sources,
    })
}

pub async fn writing_run_blocking(
    req: WritingRequest,
    source: &str,
    on_delta: impl FnMut(&str),
) -> AppResult<Value> {
    let s = settings::load_settings()?;
    let client = LmStudioClient::from_settings(&s);
    let out = writing::run_writing(&client, &s, &req, None, on_delta).await?;
    let out = log_and_enrich_outcome(&req, out, source, &s);
    Ok(writing_outcome_json(&out, &req.task, &s))
}

pub fn project_create(root: &str, title: &str) -> AppResult<Value> {
    let opened = project::create_project(Path::new(root), title)?;
    let mut s = settings::load_settings()?;
    s.touch_recent_project(&opened.root.to_string_lossy(), title);
    let _ = settings::save_settings(&s);
    Ok(project::project_to_value(&opened.root, &opened.project))
}

/// 默认在软件运行根目录/novels 下按书名建独立文件夹（重名自动加数字）
pub fn project_create_in_novels(title: &str) -> AppResult<Value> {
    let title = title.trim();
    let title = if title.is_empty() { "未命名小说" } else { title };
    let root = crate::paths::allocate_novel_folder(title)?;
    std::fs::create_dir_all(&root)?;
    let opened = project::create_project(&root, title)?;
    let mut s = settings::load_settings()?;
    s.touch_recent_project(&opened.root.to_string_lossy(), title);
    let _ = settings::save_settings(&s);
    Ok(project::project_to_value(&opened.root, &opened.project))
}

/// 返回默认 novels 目录路径（供 UI 提示）
pub fn novels_dir_info() -> AppResult<Value> {
    let root = crate::paths::runtime_root_dir()?;
    let novels = crate::paths::novels_dir()?;
    Ok(json!({
        "ok": true,
        "runtime_root": root.to_string_lossy(),
        "novels_dir": novels.to_string_lossy(),
    }))
}

pub fn project_open(root: &str) -> AppResult<Value> {
    let opened = project::open_project(Path::new(root))?;
    let mut s = settings::load_settings()?;
    if project::is_knowledge_kind(&opened.project.kind) {
        s.touch_recent_knowledge_base(root, &opened.project.title);
    } else {
        s.touch_recent_project(root, &opened.project.title);
    }
    let _ = settings::save_settings(&s);
    let emb_path = Path::new(root).join("embeddings.sqlite");
    if !emb_path.exists() && s.resolve_embedding_model().is_some() {
        let root_owned = root.to_string();
        tauri::async_runtime::spawn(async move {
            let Ok(settings) = settings::load_settings() else {
                return;
            };
            let client = LmStudioClient::new();
            let _ = crate::rag::rebuild_index(&client, &settings, Path::new(&root_owned)).await;
        });
    }
    Ok(project::project_to_value(&opened.root, &opened.project))
}

/// 扫描目录（含自身与最多两级子目录），把找到的 `project.json` 作品登记到最近列表。
/// 不切换当前打开作品；知识库类会进最近知识库列表。
pub fn project_import_directory(parent: &str, max_depth: Option<u32>) -> AppResult<Value> {
    if crate::paths::is_mobile() {
        return Err(AppError::msg(
            "手机端不支持从任意路径批量导入，请用「导入备份」或应用内目录",
        ));
    }
    let parent_path = Path::new(parent);
    let depth = max_depth.unwrap_or(2).min(4) as usize;
    let roots = project::discover_project_roots(parent_path, depth)?;
    if roots.is_empty() {
        return Ok(json!({
            "ok": true,
            "parent": parent,
            "found": 0,
            "imported_novels": 0,
            "imported_knowledge": 0,
            "failed": [],
            "items": [],
            "settings": settings::load_settings()?,
            "message": format!("未在「{parent}」下发现含 project.json 的作品（扫描深度 {depth}）"),
        }));
    }

    let mut s = settings::load_settings()?;
    let mut items: Vec<Value> = Vec::new();
    let mut failed: Vec<Value> = Vec::new();
    let mut imported_novels = 0u32;
    let mut imported_knowledge = 0u32;

    // 倒序 touch，使扫描排序靠前的作品排在最近列表前面
    for root in roots.iter().rev() {
        let root_str = root.to_string_lossy().to_string();
        match project::open_project(root) {
            Ok(opened) => {
                let title = opened.project.title.clone();
                let kind = opened.project.kind.clone();
                let is_kb = project::is_knowledge_kind(&kind);
                if is_kb {
                    s.touch_recent_knowledge_base(&root_str, &title);
                    imported_knowledge += 1;
                } else {
                    s.touch_recent_project(&root_str, &title);
                    imported_novels += 1;
                }
                items.push(json!({
                    "root": root_str,
                    "title": title,
                    "kind": kind,
                    "is_knowledge": is_kb,
                }));
            }
            Err(e) => {
                failed.push(json!({
                    "root": root_str,
                    "error": e.to_string(),
                }));
            }
        }
    }
    // items 当前是倒序登记顺序，翻回与扫描排序一致
    items.reverse();
    settings::save_settings(&s)?;

    Ok(json!({
        "ok": true,
        "parent": parent,
        "found": roots.len(),
        "imported_novels": imported_novels,
        "imported_knowledge": imported_knowledge,
        "failed": failed,
        "items": items,
        "settings": s,
        "message": format!(
            "已导入 {} 个写作作品、{} 个知识库（共发现 {}，失败 {}）",
            imported_novels,
            imported_knowledge,
            roots.len(),
            failed.len()
        ),
    }))
}

pub fn project_forget_recent(root: &str) -> AppResult<Value> {
    let mut s = settings::load_settings()?;
    s.remove_recent_project(root);
    settings::save_settings(&s)?;
    Ok(json!({ "ok": true, "settings": s }))
}

/// 从最近列表移除；`purge=true` 时若目录含 `project.json` 则删除整目录。
/// 硬安全：无 project.json 不删；Downloads/Desktop/Documents 等用户根目录永不 purge。
pub fn project_delete(root: &str, purge: bool) -> AppResult<Value> {
    let path = Path::new(root);
    let meta = path.join("project.json");
    let had_meta = meta.exists();
    let mut s = settings::load_settings()?;
    s.remove_recent_project(root);
    settings::save_settings(&s)?;

    let mut purged = false;
    if purge {
        if let Some(reason) = purge_path_blocked(path) {
            return Err(AppError::msg(format!(
                "拒绝 purge（{reason}），仅已从最近列表移除：{root}"
            )));
        }
        if !had_meta {
            return Err(AppError::msg(format!(
                "拒绝 purge：路径无 project.json，仅已从最近列表移除：{root}"
            )));
        }
        if path.exists() {
            fs::remove_dir_all(path).map_err(|e| {
                AppError::msg(format!("删除作品目录失败 {root}: {e}"))
            })?;
            purged = true;
        }
    }
    Ok(json!({
        "ok": true,
        "root": root,
        "forgotten": true,
        "purged": purged,
        "had_project_json": had_meta
    }))
}

fn purge_path_blocked(path: &Path) -> Option<&'static str> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    const BLOCK_NAMES: &[&str] = &[
        "downloads",
        "download",
        "desktop",
        "documents",
        "pictures",
        "music",
        "videos",
        "onedrive",
        "appdata",
        "users",
        "windows",
        "program files",
        "program files (x86)",
    ];
    if BLOCK_NAMES.iter().any(|b| name == *b) {
        return Some("系统/用户常用根目录禁止 purge");
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        let home_p = Path::new(&home);
        if path == home_p {
            return Some("不能删除用户主目录");
        }
    }
    if path.parent().is_none() {
        return Some("不能删除盘符根");
    }
    None
}

/// 清空全部最近「小说」作品；`purge=true` 时删除各自目录（仅含 project.json 的）。
pub fn project_forget_all_novels(purge: bool) -> AppResult<Value> {
    let s = settings::load_settings()?;
    let roots: Vec<String> = s.recent_projects.iter().map(|p| p.path.clone()).collect();
    let mut results = Vec::new();
    for root in &roots {
        match project_delete(root, purge) {
            Ok(v) => results.push(v),
            Err(e) => results.push(json!({
                "ok": false,
                "root": root,
                "error": e.to_string()
            })),
        }
    }
    let s2 = settings::load_settings()?;
    Ok(json!({
        "ok": true,
        "purge": purge,
        "count": roots.len(),
        "results": results,
        "recent_projects_left": s2.recent_projects.len()
    }))
}

pub fn project_get(root: &str) -> AppResult<Value> {
    let opened = project::open_project(Path::new(root))?;
    Ok(project::project_to_value(&opened.root, &opened.project))
}

pub fn project_save_meta(root: &str, project: project::NovelProject) -> AppResult<Value> {
    project::save_project_meta(Path::new(root), &project)?;
    Ok(json!({ "ok": true }))
}

fn take_chars(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

/// 汇总大纲 / 章纲 / 记忆 / 正文摘录，供 AI 起书名
fn build_book_title_seed(root: &Path, project: &project::NovelProject) -> (String, usize) {
    let mut parts: Vec<String> = Vec::new();
    let mut substance = 0usize;

    parts.push(format!("当前书名：{}", project.title.trim()));
    let outline = project.book_outline.trim();
    if !outline.is_empty() {
        substance += outline.chars().count();
        parts.push(format!("全书大纲：\n{}", take_chars(outline, 1400)));
    }

    if !project.chapters.is_empty() {
        let mut lines = Vec::new();
        for ch in project.chapters.iter().take(16) {
            let title = ch.title.trim();
            let summary = ch.summary.trim();
            substance += title.chars().count() + summary.chars().count();
            if summary.is_empty() {
                lines.push(format!("- {}", title));
            } else {
                lines.push(format!("- {}：{}", title, take_chars(summary, 180)));
            }
        }
        parts.push(format!("章节目录：\n{}", lines.join("\n")));
    }

    if let Ok(mem) = project::load_memory(root) {
        let roll = mem.rolling_summary.trim();
        if !roll.is_empty() {
            substance += roll.chars().count();
            parts.push(format!("记忆摘要：\n{}", take_chars(roll, 900)));
        }
        for snap in mem.chapter_snapshots.iter().take(8) {
            let s = snap.summary.trim();
            if s.is_empty() {
                continue;
            }
            substance += s.chars().count();
            parts.push(format!("章快照：\n{}", take_chars(s, 240)));
        }
    }

    // 取前两章正文摘录（空章跳过）
    let mut body_budget = 1800usize;
    for ch in project.chapters.iter().take(3) {
        if body_budget == 0 {
            break;
        }
        if let Ok((_, content)) = project::read_chapter(root, &ch.id) {
            let clean = content.trim();
            if clean.is_empty() {
                continue;
            }
            let take = body_budget.min(900);
            substance += clean.chars().count().min(take);
            parts.push(format!(
                "正文摘录（{}）：\n{}",
                ch.title.trim(),
                take_chars(clean, take)
            ));
            body_budget = body_budget.saturating_sub(take);
        }
    }

    (parts.join("\n\n"), substance)
}

const EMPTY_SUBSTANCE_THRESHOLD: usize = 20;

/// 检测作品有效内容量（与 AI 起书名同一套汇总逻辑）
pub fn project_content_substance(root: &str) -> AppResult<Value> {
    let path = Path::new(root);
    let opened = project::open_project(path)?;
    if project::is_knowledge_kind(&opened.project.kind) {
        return Ok(json!({
            "ok": true,
            "substance_chars": 0,
            "is_empty": false,
            "empty_threshold": EMPTY_SUBSTANCE_THRESHOLD,
            "kind": opened.project.kind,
        }));
    }
    let (_, substance) = build_book_title_seed(path, &opened.project);
    Ok(json!({
        "ok": true,
        "substance_chars": substance,
        "is_empty": substance < EMPTY_SUBSTANCE_THRESHOLD,
        "empty_threshold": EMPTY_SUBSTANCE_THRESHOLD,
    }))
}

fn sanitize_book_title(raw: &str) -> AppResult<String> {
    let mut t = raw.trim().to_string();
    if let Some(line) = t.lines().next() {
        t = line.trim().to_string();
    }
    for prefix in ["书名：", "书名:", "标题：", "标题:", "Title:", "title:"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest.trim().to_string();
        }
    }
    t = t
        .trim_matches(|c: char| {
            matches!(
                c,
                '"' | '\''
                    | '“'
                    | '”'
                    | '‘'
                    | '’'
                    | '《'
                    | '》'
                    | '「'
                    | '」'
                    | '【'
                    | '】'
                    | '*'
                    | '`'
                    | '#'
            )
        })
        .trim()
        .to_string();
    // 去掉尾部句号类
    while t.ends_with('。') || t.ends_with('.') || t.ends_with('！') || t.ends_with('!') {
        t.pop();
        t = t.trim_end().to_string();
    }
    if t.is_empty() {
        return Err(AppError::msg("模型未返回有效书名"));
    }
    if t.chars().count() > 24 {
        t = t.chars().take(24).collect();
    }
    Ok(t)
}

/// AI 根据作品内容建议书名（不自动写入）
pub async fn project_suggest_title(root: &str) -> AppResult<Value> {
    let path = Path::new(root);
    let opened = project::open_project(path)?;
    if project::is_knowledge_kind(&opened.project.kind) {
        return Err(AppError::msg("知识库不支持生成书名"));
    }
    let (seed, substance) = build_book_title_seed(path, &opened.project);
    if substance < EMPTY_SUBSTANCE_THRESHOLD {
        return Err(AppError::msg(
            "内容太少，请先写全书大纲、章纲或正文再生成书名",
        ));
    }

    let s = settings::load_settings()?;
    let model = s.resolve_analysis_model().to_string();
    let tpl = include_str!("../prompts/suggest_book_title.md");
    let user = tpl.replace("{{seed}}", &seed);
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: "你是网文书名助理，只输出一个中文书名，不要解释。".into(),
        },
        ChatMessage {
            role: "user".into(),
            content: user,
        },
    ];
    let options = ChatOptions {
        model: Some(model.clone()),
        temperature: Some(0.75),
        max_tokens: Some(64),
        stream: false,
        ..Default::default()
    };
    let client = LmStudioClient::from_settings(&s);
    let r = client.chat(&s, &messages, &options).await?;
    let title = sanitize_book_title(&r.text)?;
    Ok(json!({
        "ok": true,
        "title": title,
        "previous_title": opened.project.title,
        "model": model,
        "substance_chars": substance,
    }))
}

/// 将书名写入 project.json，并刷新最近列表标题；可选同步重命名作品文件夹
pub fn project_apply_title(root: &str, title: &str, rename_folder: bool) -> AppResult<Value> {
    let title = sanitize_book_title(title)?;
    let path = Path::new(root);
    let mut opened = project::open_project(path)?;
    if project::is_knowledge_kind(&opened.project.kind) {
        return Err(AppError::msg("知识库不支持改书名"));
    }
    opened.project.title = title.clone();
    project::save_project_meta(path, &opened.project)?;

    let mut final_path = path.to_path_buf();
    let mut folder_renamed = false;
    let mut folder_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    if rename_folder {
        let parent = path
            .parent()
            .ok_or_else(|| AppError::msg(format!("无法解析作品父目录：{root}")))?;
        let dest = crate::paths::allocate_folder_in_parent(parent, &title, path)?;
        if dest != path {
            fs::rename(path, &dest).map_err(|e| {
                AppError::msg(format!(
                    "重命名作品文件夹失败（{} → {}）：{e}",
                    path.display(),
                    dest.display()
                ))
            })?;
            final_path = dest;
            folder_renamed = true;
            folder_name = final_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
        }
    }

    let final_root = final_path.to_string_lossy().to_string();
    let mut s = settings::load_settings()?;
    if folder_renamed {
        s.replace_recent_project_path(root, &final_root, &title);
    } else {
        s.touch_recent_project(root, &title);
    }
    let _ = settings::save_settings(&s);
    Ok(json!({
        "ok": true,
        "title": title,
        "root": final_root,
        "previous_root": root,
        "folder_renamed": folder_renamed,
        "folder_name": folder_name,
        "project": project::project_to_value(&final_path, &opened.project),
        "settings": s,
    }))
}

pub fn chapter_read(root: &str, chapter_id: &str) -> AppResult<Value> {
    let (meta, content) = project::read_chapter(Path::new(root), chapter_id)?;
    let blocks = project::read_genblocks(Path::new(root), chapter_id);
    Ok(json!({ "ok": true, "meta": meta, "content": content, "blocks": blocks }))
}

pub fn chapter_write(
    root: &str,
    chapter_id: &str,
    content: &str,
    blocks: Option<&Value>,
) -> AppResult<Value> {
    let clean = export::strip_kk_gen_markers(content);
    project::write_chapter(Path::new(root), chapter_id, &clean)?;
    if let Some(b) = blocks {
        project::write_genblocks(Path::new(root), chapter_id, b)?;
        // 删块/清空正文后同步清掉孤儿记忆，避免续写仍吃旧摘要
        let _ = project::sync_chapter_block_notes_from_blocks(Path::new(root), chapter_id, b);
    } else if clean.trim().is_empty()
        || clean
            .lines()
            .all(|l| l.trim().is_empty() || l.trim().starts_with('#'))
    {
        let _ = project::clear_chapter_memory(Path::new(root), chapter_id);
    }
    crate::rag::spawn_index_chapter(root.to_string(), chapter_id.to_string(), clean.clone());
    Ok(json!({ "ok": true }))
}

pub fn chapter_create(root: &str, title: &str, summary: &str) -> AppResult<Value> {
    let meta = project::create_chapter(Path::new(root), title, summary)?;
    Ok(json!({ "ok": true, "chapter": meta }))
}

pub fn chapter_delete(root: &str, chapter_id: &str) -> AppResult<Value> {
    project::delete_chapter(Path::new(root), chapter_id)?;
    Ok(json!({ "ok": true }))
}

pub fn chapter_update_meta(
    root: &str,
    chapter_id: &str,
    patch: project::ChapterMetaPatch,
) -> AppResult<Value> {
    let meta = project::update_chapter_meta(Path::new(root), chapter_id, patch)?;
    Ok(json!({ "ok": true, "chapter": meta }))
}

pub fn beat_progress_get(root: &str, chapter_id: &str) -> AppResult<Value> {
    let progress = crate::writing::beat_progress_get(Path::new(root), chapter_id)?;
    Ok(json!({ "ok": true, "progress": progress }))
}

pub fn beat_progress_advance(root: &str, chapter_id: &str, beat_id: &str) -> AppResult<Value> {
    let progress =
        crate::writing::beat_progress_advance(Path::new(root), chapter_id, beat_id)?;
    Ok(json!({ "ok": true, "progress": progress }))
}

pub fn beat_progress_reset(root: &str, chapter_id: &str) -> AppResult<Value> {
    crate::writing::beat_progress_reset(Path::new(root), chapter_id)?;
    Ok(json!({ "ok": true }))
}

pub fn beat_progress_skip(root: &str, chapter_id: &str, beat_id: &str) -> AppResult<Value> {
    let progress = crate::writing::beat_progress_skip(Path::new(root), chapter_id, beat_id)?;
    Ok(json!({ "ok": true, "progress": progress }))
}

pub fn lore_list(root: &str) -> AppResult<Value> {
    Ok(json!({ "ok": true, "items": project::list_lore(Path::new(root))? }))
}

/// 列出本篇 + 全局角色仓（带 scope），供设定页分栏
pub fn lore_list_scoped(novel_root: &str) -> AppResult<Value> {
    let local = project::list_lore(Path::new(novel_root))?;
    let roster = crate::kb::ensure_character_roster()?;
    let global = project::list_lore(&roster.root)?;
    let local_items: Vec<Value> = local
        .into_iter()
        .map(|e| {
            json!({
                "scope": "local",
                "root": novel_root,
                "entry": e,
            })
        })
        .collect();
    let global_items: Vec<Value> = global
        .into_iter()
        .map(|e| {
            json!({
                "scope": "global",
                "root": roster.root.to_string_lossy(),
                "entry": e,
            })
        })
        .collect();
    Ok(json!({
        "ok": true,
        "character_roster": {
            "path": roster.root.to_string_lossy(),
            "marker": crate::kb::CHARACTERS_MARKER,
            "title": roster.project.title,
        },
        "local": local_items,
        "global": global_items,
    }))
}

pub fn character_roster_ensure() -> AppResult<Value> {
    let roster = crate::kb::ensure_character_roster()?;
    Ok(json!({
        "ok": true,
        "root": roster.root.to_string_lossy(),
        "marker": crate::kb::CHARACTERS_MARKER,
        "project": roster.project,
    }))
}

/// 确保作品挂接了 @characters；已有则不动
pub fn project_ensure_characters_link(root: &str) -> AppResult<Value> {
    let mut opened = project::open_project(Path::new(root))?;
    let marker = crate::kb::CHARACTERS_MARKER.to_string();
    let _ = crate::kb::ensure_character_roster()?;
    if !opened.project.linked_kb_roots.iter().any(|l| l == &marker || l == "characters") {
        opened.project.linked_kb_roots.insert(0, marker);
        project::save_project_meta(Path::new(root), &opened.project)?;
    }
    Ok(json!({
        "ok": true,
        "linked_kb_roots": opened.project.linked_kb_roots,
        "project": opened.project,
    }))
}

pub fn lore_upsert(root: &str, entry: LoreEntry) -> AppResult<Value> {
    let item = project::upsert_lore(Path::new(root), entry)?;
    crate::rag::spawn_index_lore(root.to_string(), item.clone());
    Ok(json!({ "ok": true, "item": item }))
}

/// 手动写入/覆盖块记忆摘要（会清洗婴儿相关词并重建 rolling_summary）
pub fn memory_upsert_block_note(
    root: &str,
    chapter_id: &str,
    block_key: &str,
    summary: &str,
) -> AppResult<Value> {
    let note = project::append_block_note(Path::new(root), chapter_id, block_key, summary)?;
    Ok(json!({
        "ok": true,
        "note": note,
        "summary": note.summary,
    }))
}

/// 删除指定块记忆笔记
pub fn memory_remove_block_note(
    root: &str,
    chapter_id: &str,
    block_key: &str,
) -> AppResult<Value> {
    let removed = project::remove_block_note(Path::new(root), chapter_id, block_key)?;
    Ok(json!({ "ok": true, "removed": removed }))
}

/// 用本章块笔记合成章快照，供跨章记忆
pub fn memory_sync_chapter_snapshot(
    root: &str,
    chapter_id: &str,
    fallback: &str,
) -> AppResult<Value> {
    let summary =
        project::sync_chapter_snapshot_from_notes(Path::new(root), chapter_id, fallback)?;
    Ok(json!({ "ok": true, "summary": summary }))
}

pub fn lore_delete(root: &str, lore_id: &str) -> AppResult<Value> {
    project::delete_lore(Path::new(root), lore_id)?;
    Ok(json!({ "ok": true }))
}

pub fn story_plot_get(root: &str) -> AppResult<Value> {
    Ok(json!({ "ok": true, "plot": crate::story::load_plot(Path::new(root))? }))
}

pub fn story_plot_save(root: &str, plot: crate::story::PlotStore) -> AppResult<Value> {
    crate::story::save_plot(Path::new(root), &plot)?;
    Ok(json!({ "ok": true, "plot": plot }))
}

pub fn story_timeline_get(root: &str) -> AppResult<Value> {
    Ok(json!({ "ok": true, "timeline": crate::story::load_timeline(Path::new(root))? }))
}

pub fn story_timeline_save(root: &str, timeline: crate::story::TimelineStore) -> AppResult<Value> {
    crate::story::save_timeline(Path::new(root), &timeline)?;
    Ok(json!({ "ok": true, "timeline": timeline }))
}

pub fn story_relations_get(root: &str) -> AppResult<Value> {
    Ok(json!({ "ok": true, "relations": crate::story::load_relations(Path::new(root))? }))
}

pub fn story_relations_save(root: &str, relations: crate::story::RelationsStore) -> AppResult<Value> {
    crate::story::save_relations(Path::new(root), &relations)?;
    Ok(json!({ "ok": true, "relations": relations }))
}

pub fn story_canon_get(root: &str) -> AppResult<Value> {
    Ok(json!({ "ok": true, "canon": crate::story::load_canon(Path::new(root))? }))
}

pub fn story_canon_save(root: &str, canon: crate::story::CanonStore) -> AppResult<Value> {
    crate::story::save_canon(Path::new(root), &canon)?;
    Ok(json!({ "ok": true, "canon": canon }))
}

pub fn story_apply_patch(root: &str, patch: Value) -> AppResult<Value> {
    crate::story::apply_story_patch(Path::new(root), &patch)
}

pub fn story_dashboard(root: &str) -> AppResult<Value> {
    crate::story::dashboard_summary(Path::new(root))
}

pub fn export_txt(root: &str, output: &str) -> AppResult<Value> {
    export::export_txt(Path::new(root), Path::new(output))?;
    Ok(json!({ "ok": true, "output": output }))
}

pub fn export_epub(root: &str, output: &str) -> AppResult<Value> {
    export::export_epub(Path::new(root), Path::new(output))?;
    Ok(json!({ "ok": true, "output": output }))
}

pub fn export_pdf(root: &str, output: &str) -> AppResult<Value> {
    export::export_pdf(Path::new(root), Path::new(output))?;
    Ok(json!({ "ok": true, "output": output }))
}

pub fn import_txt(root: &str, file: &str, title: &str) -> AppResult<Value> {
    let report = import::import_txt(Path::new(root), Path::new(file), title)?;
    // import_txt 内部已登记 registry + recent_knowledge_bases
    Ok(serde_json::to_value(report)?)
}

pub async fn import_distill(
    root: &str,
    from: u64,
    to: u64,
    apply: &str,
    resume: bool,
    job_id: Option<&str>,
    instruction: &str,
) -> AppResult<Value> {
    let mode = ApplyMode::parse(apply)?;
    let report = import::distill_range(
        Path::new(root),
        from as usize,
        to as usize,
        mode,
        resume,
        job_id,
        instruction,
    )
    .await?;
    Ok(serde_json::to_value(report)?)
}

pub fn import_apply_pending(root: &str, job_id: &str) -> AppResult<Value> {
    import::apply_pending_job(Path::new(root), job_id)
}

pub fn kb_registry_list() -> AppResult<Value> {
    crate::kb::registry_list_json()
}

pub fn kb_universal_open() -> AppResult<Value> {
    let opened = crate::kb::ensure_universal()?;
    let mut s = settings::load_settings()?;
    s.touch_recent_knowledge_base(
        opened.root.to_string_lossy().as_ref(),
        &opened.project.title,
    );
    let _ = settings::save_settings(&s);
    Ok(project::project_to_value(&opened.root, &opened.project))
}

pub fn kb_sync(root: &str) -> AppResult<Value> {
    crate::kb::sync_to_universal(Path::new(root))
}

pub fn kb_sync_all() -> AppResult<Value> {
    crate::kb::sync_all()
}

pub async fn kb_universal_rebuild_rag() -> AppResult<Value> {
    let uni = crate::kb::ensure_universal()?;
    let s = settings::load_settings()?;
    let client = LmStudioClient::new();
    let count = crate::rag::rebuild_index(&client, &s, &uni.root).await?;
    Ok(json!({ "ok": true, "indexed": count, "root": uni.root.to_string_lossy() }))
}

pub fn kb_migrate(root: &str, source_file: Option<&str>, sync: bool) -> AppResult<Value> {
    crate::kb::migrate_root(Path::new(root), source_file, sync)
}

pub fn pick_file(title: Option<&str>, extensions: Option<Vec<String>>) -> AppResult<Value> {
    if crate::paths::is_mobile() {
        return Err(AppError::msg(
            "手机端请使用系统文件选择（导入备份 / 导入 TXT），不支持桌面路径对话框",
        ));
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (title, extensions);
        return Err(AppError::msg("当前平台不支持桌面文件对话框"));
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let mut dlg = rfd::FileDialog::new().set_title(title.unwrap_or("选择文件"));
        if let Some(exts) = extensions {
            if !exts.is_empty() {
                let owned: Vec<String> = exts;
                let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
                dlg = dlg.add_filter("files", &refs);
            }
        }
        let file = dlg
            .pick_file()
            .ok_or_else(|| AppError::msg("已取消选择"))?;
        Ok(json!({ "ok": true, "path": file.to_string_lossy() }))
    }
}

pub fn stats_get(root: &str) -> AppResult<Value> {
    let stats = project::load_stats(Path::new(root))?;
    Ok(json!({ "ok": true, "stats": stats }))
}

pub fn stats_set_goal(root: &str, goal_chars: u64) -> AppResult<Value> {
    let stats = project::set_stats_goal(Path::new(root), goal_chars)?;
    Ok(json!({ "ok": true, "stats": stats }))
}

pub fn chapter_push_history(root: &str, chapter_id: &str, content: &str) -> AppResult<Value> {
    project::push_chapter_history(Path::new(root), chapter_id, content)?;
    Ok(json!({ "ok": true }))
}

pub async fn rag_rebuild(root: &str) -> AppResult<Value> {
    let s = settings::load_settings()?;
    let client = LmStudioClient::new();
    let count = crate::rag::rebuild_index(&client, &s, Path::new(root)).await?;
    Ok(json!({ "ok": true, "indexed": count }))
}

pub fn pick_directory() -> AppResult<Value> {
    if crate::paths::is_mobile() {
        return Err(AppError::msg(
            "手机端作品保存在应用私有目录，请用「新建」或「导入备份」，不支持打开任意路径",
        ));
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        return Err(AppError::msg("当前平台不支持桌面目录对话框"));
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let folder = rfd::FileDialog::new()
            .set_title("选择作品目录")
            .pick_folder()
            .ok_or_else(|| AppError::msg("已取消选择"))?;
        Ok(json!({ "ok": true, "path": folder.to_string_lossy() }))
    }
}

/// 选择要批量扫描导入的父目录
pub fn pick_import_directory() -> AppResult<Value> {
    if crate::paths::is_mobile() {
        return Err(AppError::msg(
            "手机端不支持从任意路径批量导入，请用「导入备份」",
        ));
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        return Err(AppError::msg("当前平台不支持桌面目录对话框"));
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let folder = rfd::FileDialog::new()
            .set_title("选择含有多个作品的目录")
            .pick_folder()
            .ok_or_else(|| AppError::msg("已取消选择"))?;
        Ok(json!({ "ok": true, "path": folder.to_string_lossy() }))
    }
}

pub fn project_export_backup(root: &str) -> AppResult<Value> {
    crate::project::backup::export_project_zip(Path::new(root))
}

pub fn project_import_backup_base64(data_b64: &str, title: Option<&str>) -> AppResult<Value> {
    let imported = crate::project::backup::import_project_zip_base64(data_b64, title)?;
    if let Some(root) = imported.get("root").and_then(|v| v.as_str()) {
        if let Some(project) = imported.get("project") {
            let title = project
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("导入作品");
            let mut s = settings::load_settings()?;
            s.touch_recent_project(root, title);
            let _ = settings::save_settings(&s);
        }
    }
    Ok(imported)
}

pub fn export_file_read_base64(path: &str) -> AppResult<Value> {
    crate::project::backup::read_export_file_base64(path)
}

pub fn platform_info() -> AppResult<Value> {
    Ok(json!({
        "ok": true,
        "os": std::env::consts::OS,
        "mobile": crate::paths::is_mobile(),
        "novels_dir": crate::paths::novels_dir()?.to_string_lossy(),
        "app_data_dir": crate::paths::app_data_dir()?.to_string_lossy(),
    }))
}

pub async fn dispatch_rpc(req: Value) -> AppResult<Value> {
    let cmd = req
        .get("cmd")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("缺少 cmd"))?;

    match cmd {
        "ping" => Ok(json!({ "ok": true, "message": "pong" })),
        "settings_get" => settings_get(),
        "settings_save" => {
            let settings: AppSettings = serde_json::from_value(
                req.get("settings")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 settings"))?,
            )?;
            settings_save(settings)
        }
        "llm_health" => llm_health().await,
        "llm_list_models" => llm_list_models().await,
        "llm_chat" => {
            let messages: Vec<ChatMessage> = serde_json::from_value(
                req.get("messages")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 messages"))?,
            )?;
            let options: ChatOptions = req
                .get("options")
                .cloned()
                .map(serde_json::from_value)
                .transpose()?
                .unwrap_or_default();
            llm_chat(messages, options).await
        }
        "writing_run" => {
            let writing_req: WritingRequest = serde_json::from_value(
                req.get("request")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 request"))?,
            )?;
            writing_run_blocking(writing_req, "cli-rpc", |_| {}).await
        }
        "project_create" => {
            let root = req_str(&req, "root")?;
            let title = req.get("title").and_then(|v| v.as_str()).unwrap_or("未命名");
            project_create(root, title)
        }
        "project_create_in_novels" => {
            let title = req.get("title").and_then(|v| v.as_str()).unwrap_or("未命名小说");
            project_create_in_novels(title)
        }
        "novels_dir_info" => novels_dir_info(),
        "project_open" | "project_get" => {
            let root = req_str(&req, "root")?;
            if cmd == "project_open" {
                project_open(root)
            } else {
                project_get(root)
            }
        }
        "project_forget_recent" => project_forget_recent(req_str(&req, "root")?),
        "project_import_directory" => {
            let root = req_str(&req, "root")?;
            let max_depth = req
                .get("max_depth")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32);
            project_import_directory(root, max_depth)
        }
        "project_delete" => {
            let purge = req
                .get("purge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            project_delete(req_str(&req, "root")?, purge)
        }
        "project_forget_all_novels" => {
            let purge = req
                .get("purge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            project_forget_all_novels(purge)
        }
        "project_save_meta" => {
            let root = req_str(&req, "root")?;
            let project: project::NovelProject = serde_json::from_value(
                req.get("project")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 project"))?,
            )?;
            project_save_meta(root, project)
        }
        "project_suggest_title" => {
            project_suggest_title(req_str(&req, "root")?).await
        }
        "project_content_substance" => project_content_substance(req_str(&req, "root")?),
        "project_apply_title" => {
            let rename_folder = req
                .get("rename_folder")
                .or_else(|| req.get("renameFolder"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            project_apply_title(
                req_str(&req, "root")?,
                req_str(&req, "title")?,
                rename_folder,
            )
        }
        "chapter_read" => {
            chapter_read(req_str(&req, "root")?, req_str(&req, "chapter_id")?)
        }
        "chapter_write" => {
            let content = req_str(&req, "content")?;
            let blocks = req.get("blocks");
            chapter_write(
                req_str(&req, "root")?,
                req_str(&req, "chapter_id")?,
                content,
                blocks,
            )
        }
        "chapter_create" => {
            let title = req_str(&req, "title")?;
            let summary = req.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            chapter_create(req_str(&req, "root")?, title, summary)
        }
        "chapter_delete" => chapter_delete(req_str(&req, "root")?, req_str(&req, "chapter_id")?),
        "chapter_update_meta" => {
            let mut patch = project::ChapterMetaPatch::default();
            patch.title = req
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            patch.summary = req
                .get("summary")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            patch.status = req
                .get("status")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(p) = req.get("patch") {
                patch = serde_json::from_value(p.clone())?;
            } else {
                // 兼容扁平字段
                if req.get("focus_arc_ids").is_some() {
                    patch.focus_arc_ids = serde_json::from_value(req["focus_arc_ids"].clone()).ok();
                }
                if req.get("must_do").is_some() {
                    patch.must_do = req.get("must_do").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
                if req.get("must_not").is_some() {
                    patch.must_not = req.get("must_not").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
                if req.get("pov_lore_id").is_some() {
                    patch.pov_lore_id =
                        req.get("pov_lore_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
                if req.get("beats").is_some() {
                    patch.beats = serde_json::from_value(req["beats"].clone()).ok();
                }
                if req.get("reader_knows").is_some() {
                    patch.reader_knows =
                        req.get("reader_knows").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
                if req.get("character_knows").is_some() {
                    patch.character_knows = req
                        .get("character_knows")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
            chapter_update_meta(req_str(&req, "root")?, req_str(&req, "chapter_id")?, patch)
        }
        "story_plot_get" => story_plot_get(req_str(&req, "root")?),
        "story_plot_save" => {
            let plot = serde_json::from_value(
                req.get("plot")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 plot"))?,
            )?;
            story_plot_save(req_str(&req, "root")?, plot)
        }
        "story_timeline_get" => story_timeline_get(req_str(&req, "root")?),
        "story_timeline_save" => {
            let timeline = serde_json::from_value(
                req.get("timeline")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 timeline"))?,
            )?;
            story_timeline_save(req_str(&req, "root")?, timeline)
        }
        "story_relations_get" => story_relations_get(req_str(&req, "root")?),
        "story_relations_save" => {
            let relations = serde_json::from_value(
                req.get("relations")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 relations"))?,
            )?;
            story_relations_save(req_str(&req, "root")?, relations)
        }
        "story_canon_get" => story_canon_get(req_str(&req, "root")?),
        "story_canon_save" => {
            let canon = serde_json::from_value(
                req.get("canon")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 canon"))?,
            )?;
            story_canon_save(req_str(&req, "root")?, canon)
        }
        "story_apply_patch" => {
            let patch = req
                .get("patch")
                .cloned()
                .ok_or_else(|| AppError::msg("缺少 patch"))?;
            story_apply_patch(req_str(&req, "root")?, patch)
        }
        "story_dashboard" => story_dashboard(req_str(&req, "root")?),
        "lore_list" => lore_list(req_str(&req, "root")?),
        "lore_list_scoped" => lore_list_scoped(req_str(&req, "root")?),
        "character_roster_ensure" => character_roster_ensure(),
        "project_ensure_characters_link" => {
            project_ensure_characters_link(req_str(&req, "root")?)
        }
        "lore_upsert" => {
            let entry: LoreEntry = serde_json::from_value(
                req.get("entry")
                    .cloned()
                    .ok_or_else(|| AppError::msg("缺少 entry"))?,
            )?;
            lore_upsert(req_str(&req, "root")?, entry)
        }
        "memory_upsert_block_note" => memory_upsert_block_note(
            req_str(&req, "root")?,
            req_str(&req, "chapter_id")?,
            req_str(&req, "block_key")?,
            req_str(&req, "summary")?,
        ),
        "memory_remove_block_note" => memory_remove_block_note(
            req_str(&req, "root")?,
            req_str(&req, "chapter_id")?,
            req_str(&req, "block_key")?,
        ),
        "memory_sync_chapter_snapshot" => memory_sync_chapter_snapshot(
            req_str(&req, "root")?,
            req_str(&req, "chapter_id")?,
            req.get("fallback").and_then(|v| v.as_str()).unwrap_or(""),
        ),
        "lore_delete" => lore_delete(req_str(&req, "root")?, req_str(&req, "lore_id")?),
        "export_txt" => export_txt(req_str(&req, "root")?, req_str(&req, "output")?),
        "export_epub" => export_epub(req_str(&req, "root")?, req_str(&req, "output")?),
        "export_pdf" => export_pdf(req_str(&req, "root")?, req_str(&req, "output")?),
        "import_txt" => {
            let title = req
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("未命名小说");
            import_txt(req_str(&req, "root")?, req_str(&req, "file")?, title)
        }
        "import_distill" => {
            let from = req.get("from").and_then(|v| v.as_u64()).unwrap_or(1);
            let to = req.get("to").and_then(|v| v.as_u64()).unwrap_or(20);
            let apply = req.get("apply").and_then(|v| v.as_str()).unwrap_or("none");
            let resume = req
                .get("resume")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let job_id = req.get("job_id").and_then(|v| v.as_str());
            let instruction = req.get("instruction").and_then(|v| v.as_str()).unwrap_or("");
            import_distill(
                req_str(&req, "root")?,
                from,
                to,
                apply,
                resume,
                job_id,
                instruction,
            )
            .await
        }
        "import_apply_pending" => {
            import_apply_pending(req_str(&req, "root")?, req_str(&req, "job_id")?)
        }
        "kb_registry_list" => kb_registry_list(),
        "kb_universal_open" => kb_universal_open(),
        "kb_sync" => kb_sync(req_str(&req, "root")?),
        "kb_sync_all" => kb_sync_all(),
        "kb_universal_rebuild_rag" => kb_universal_rebuild_rag().await,
        "kb_migrate" => {
            let source = req.get("source_file").and_then(|v| v.as_str());
            let sync = req.get("sync").and_then(|v| v.as_bool()).unwrap_or(true);
            kb_migrate(req_str(&req, "root")?, source, sync)
        }
        "pick_file" => {
            let title = req.get("title").and_then(|v| v.as_str());
            let extensions = req
                .get("extensions")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                });
            pick_file(title, extensions)
        }
        "stats_get" => stats_get(req_str(&req, "root")?),
        "stats_set_goal" => {
            let goal = req
                .get("goal_chars")
                .and_then(|v| v.as_u64())
                .unwrap_or(2000);
            stats_set_goal(req_str(&req, "root")?, goal)
        }
        "chapter_push_history" => {
            chapter_push_history(
                req_str(&req, "root")?,
                req_str(&req, "chapter_id")?,
                req_str(&req, "content")?,
            )
        }
        "rag_rebuild" => rag_rebuild(req_str(&req, "root")?).await,
        "gen_log_list" => {
            let limit = req.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            genlog::list_as_json(limit)
        }
        "usage_summary" => {
            let root = req.get("root").and_then(|v| v.as_str());
            usage_summary(root)
        }
        other => Err(AppError::msg(format!("未知 cmd: {other}"))),
    }
}

fn req_str<'a>(req: &'a Value, key: &str) -> AppResult<&'a str> {
    req.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg(format!("缺少 {key}")))
}

/// 供 GUI 流式任务使用
pub async fn writing_run_stream(
    req: WritingRequest,
    cancel: Arc<AtomicBool>,
    on_delta: impl FnMut(&str),
    source: &str,
) -> AppResult<String> {
    let out = writing_run_stream_full(req, cancel, on_delta, source).await?;
    Ok(out.text)
}

/// 带元信息的流式写作（模型回退 / 复读截断标记）
pub async fn writing_run_stream_full(
    req: WritingRequest,
    cancel: Arc<AtomicBool>,
    on_delta: impl FnMut(&str),
    source: &str,
) -> AppResult<writing::WritingOutcome> {
    let s = settings::load_settings()?;
    let client = LmStudioClient::from_settings(&s);
    let out = writing::run_writing(&client, &s, &req, Some(cancel), on_delta).await?;
    Ok(log_and_enrich_outcome(&req, out, source, &s))
}

pub fn usage_summary(project_root: Option<&str>) -> AppResult<Value> {
    crate::usage::summary_json(project_root)
}
