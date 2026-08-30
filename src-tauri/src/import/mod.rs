//! TXT 导入与知识库蒸馏
//! 代码路径: kk_novel_ai/src-tauri/src/import/mod.rs

use crate::error::{AppError, AppResult};
use crate::genlog;
use crate::llm::{ChatMessage, ChatOptions, LmStudioClient};
use crate::project::{self, LoreEntry, LoreLink};
use crate::settings::AppSettings;
use crate::writing::{self, WritingRequest};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedChapter {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub ok: bool,
    pub root: String,
    pub title: String,
    pub chapter_count: usize,
    pub titles_sample: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyMode {
    None,
    Auto,
}

impl ApplyMode {
    pub fn parse(s: &str) -> AppResult<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" | "" => Ok(Self::None),
            "auto" => Ok(Self::Auto),
            other => Err(AppError::msg(format!(
                "未知 apply 模式: {other}（none|auto）"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillReport {
    pub ok: bool,
    pub root: String,
    pub job_id: String,
    pub job_dir: String,
    pub from: usize,
    pub to: usize,
    pub processed: usize,
    pub skipped_resume: usize,
    pub failed: Vec<String>,
    pub entity_count: usize,
    pub fact_count: usize,
    pub edge_count: usize,
    pub event_count: usize,
    pub applied: bool,
}

fn eq_heading() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^===\s*(.+?)\s*===$").expect("eq heading regex"))
}

fn cn_chapter_heading() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^第[0-9一二三四五六七八九十百千零〇两]+章\s*.*$").expect("cn chapter regex")
    })
}

fn match_heading(line: &str) -> Option<String> {
    let t = line.trim();
    if let Some(c) = eq_heading().captures(t) {
        let title = c.get(1).map(|m| m.as_str().trim()).unwrap_or("").to_string();
        if !title.is_empty() {
            return Some(title);
        }
    }
    None
}

fn match_fallback_heading(line: &str) -> Option<String> {
    let t = line.trim();
    if cn_chapter_heading().is_match(t) {
        return Some(t.to_string());
    }
    None
}

/// 流式按行切章：优先 `===标题===`；若全文无此类标记则回退「第N章」
pub fn parse_txt_chapters(path: &Path) -> AppResult<Vec<ParsedChapter>> {
    let file = File::open(path).map_err(|e| AppError::msg(format!("打开 TXT 失败: {e}")))?;
    let reader = BufReader::new(file);
    let mut chapters: Vec<ParsedChapter> = Vec::new();
    let mut cur_title: Option<String> = None;
    let mut cur_body = String::new();
    let mut saw_eq = false;
    let mut all_lines: Vec<String> = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| AppError::msg(format!("读 TXT 失败: {e}")))?;
        all_lines.push(line.clone());
        if let Some(title) = match_heading(&line) {
            saw_eq = true;
            if let Some(prev) = cur_title.take() {
                chapters.push(ParsedChapter {
                    title: prev,
                    body: cur_body.trim().to_string(),
                });
                cur_body.clear();
            }
            cur_title = Some(title);
        } else if cur_title.is_some() {
            cur_body.push_str(&line);
            cur_body.push('\n');
        }
    }
    if let Some(prev) = cur_title.take() {
        chapters.push(ParsedChapter {
            title: prev,
            body: cur_body.trim().to_string(),
        });
    }

    if saw_eq && !chapters.is_empty() {
        return Ok(chapters);
    }

    // 回退：第N章
    chapters.clear();
    cur_title = None;
    cur_body.clear();
    for line in &all_lines {
        if let Some(title) = match_fallback_heading(line) {
            if let Some(prev) = cur_title.take() {
                chapters.push(ParsedChapter {
                    title: prev,
                    body: cur_body.trim().to_string(),
                });
                cur_body.clear();
            }
            cur_title = Some(title);
        } else if cur_title.is_some() {
            cur_body.push_str(line);
            cur_body.push('\n');
        }
    }
    if let Some(prev) = cur_title {
        chapters.push(ParsedChapter {
            title: prev,
            body: cur_body.trim().to_string(),
        });
    }

    if chapters.is_empty() {
        return Err(AppError::msg(
            "未能切出章节：需要 ===标题=== 或「第N章」行首标记",
        ));
    }
    Ok(chapters)
}

pub fn import_txt(root: &Path, source: &Path, title: &str) -> AppResult<ImportReport> {
    if !source.is_file() {
        return Err(AppError::msg(format!(
            "源文件不存在: {}",
            source.display()
        )));
    }
    let parsed = parse_txt_chapters(source)?;
    let count = parsed.len();
    let sample: Vec<String> = parsed
        .iter()
        .take(5)
        .map(|c| c.title.clone())
        .chain(
            parsed
                .iter()
                .rev()
                .take(if count > 5 { 1 } else { 0 })
                .map(|c| c.title.clone()),
        )
        .collect();

    let source_s = source.to_string_lossy().to_string();
    let opened = project::create_knowledge_base(root, title, Some(&source_s))?;
    let pairs: Vec<(String, String)> = parsed
        .into_iter()
        .map(|c| (c.title, c.body))
        .collect();
    project::replace_all_chapters(root, &pairs)?;

    let _ = crate::kb::register_knowledge_base(root, title, &opened.project.id);
    let mut s = crate::settings::load_settings().unwrap_or_default();
    s.touch_recent_knowledge_base(&root.to_string_lossy(), title);
    let _ = crate::settings::save_settings(&s);

    let _ = genlog::append_log(&genlog::make_entry(
        "kb_import_txt",
        &root.to_string_lossy(),
        "",
        &format!("imported {count} chapters into knowledge_base from {}", source.display()),
        "kb",
    ));

    Ok(ImportReport {
        ok: true,
        root: root.to_string_lossy().to_string(),
        title: title.to_string(),
        chapter_count: count,
        titles_sample: sample,
        source: source_s,
    })
}

fn jobs_dir(root: &Path) -> PathBuf {
    root.join("import").join("jobs")
}

fn ensure_job_dir(root: &Path, job_id: &str) -> AppResult<PathBuf> {
    let dir = jobs_dir(root).join(job_id);
    fs::create_dir_all(dir.join("per_chapter"))?;
    Ok(dir)
}

fn normalize_name(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn take_tail(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    chars[chars.len() - max_chars..].iter().collect()
}

fn strip_json_fence(raw: &str) -> String {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix("```json") {
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().to_string();
        }
    }
    if let Some(rest) = t.strip_prefix("```") {
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().to_string();
        }
    }
    // 尝试截取第一个 { 到最后一个 }
    if let (Some(s), Some(e)) = (t.find('{'), t.rfind('}')) {
        if e > s {
            return t[s..=e].to_string();
        }
    }
    t.to_string()
}

fn lore_catalog_text(entries: &[LoreEntry]) -> String {
    if entries.is_empty() {
        return "（无）".into();
    }
    entries
        .iter()
        .take(80)
        .map(|e| {
            let aliases: Vec<&str> = e
                .keywords
                .iter()
                .filter(|k| k.starts_with("alias:"))
                .map(|k| k.trim_start_matches("alias:"))
                .collect();
            if aliases.is_empty() {
                format!("- {} [{}] id={}", e.title, e.kind, e.id)
            } else {
                format!(
                    "- {} [{}] id={} aliases={}",
                    e.title,
                    e.kind,
                    e.id,
                    aliases.join("/")
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct AliasIndex {
    map: HashMap<String, String>,
}

impl AliasIndex {
    fn from_lore(entries: &[LoreEntry]) -> Self {
        let mut map = HashMap::new();
        for e in entries {
            map.insert(normalize_name(&e.title), e.id.clone());
            for k in &e.keywords {
                if let Some(a) = k.strip_prefix("alias:") {
                    map.insert(normalize_name(a), e.id.clone());
                }
            }
        }
        Self { map }
    }

    fn resolve(&self, name: &str) -> Option<String> {
        let n = normalize_name(name);
        if n.is_empty() {
            return None;
        }
        self.map.get(&n).cloned()
    }

    fn register(&mut self, title: &str, id: &str, aliases: &[String]) {
        self.map.insert(normalize_name(title), id.to_string());
        for a in aliases {
            self.map.insert(normalize_name(a), id.to_string());
        }
    }
}

fn render_lore_extract(
    lore: &str,
    canon: &str,
    outline: &str,
    recent: &str,
    instruction: &str,
) -> String {
    let tpl = include_str!("../../prompts/lore_extract.md");
    tpl.replace("{{lore}}", lore)
        .replace("{{canon}}", canon)
        .replace("{{outline}}", outline)
        .replace("{{recent_text}}", recent)
        .replace(
            "{{instruction}}",
            if instruction.is_empty() {
                "（无额外指令）"
            } else {
                instruction
            },
        )
}

async fn call_lore_extract(
    client: &LmStudioClient,
    settings: &AppSettings,
    root: &Path,
    chapter_title: &str,
    content: &str,
    instruction: &str,
) -> AppResult<Value> {
    let lore_entries = project::list_lore(root)?;
    let canon = crate::story::load_canon(root).unwrap_or_default();
    let lore_text = lore_catalog_text(&lore_entries);
    let canon_text = crate::story::canon_for_prompt(&canon, false);
    let recent = take_tail(content, settings.recent_window_chars.max(4000));
    let user = render_lore_extract(&lore_text, &canon_text, chapter_title, &recent, instruction);
    let model = settings.resolve_analysis_model();
    if model.is_empty() {
        return Err(AppError::msg(
            "未配置 analysis_model / model，无法蒸馏知识库",
        ));
    }
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: "你是本地小说知识库助手，严格只输出 JSON。".into(),
        },
        ChatMessage {
            role: "user".into(),
            content: user,
        },
    ];
    let options = ChatOptions {
        model: Some(model.to_string()),
        temperature: Some(settings.resolve_analysis_temperature()),
        max_tokens: Some(settings.max_tokens.max(8192)),
        frequency_penalty: Some(0.0),
        presence_penalty: Some(0.0),
        stream: false,
    };
    let raw = client.chat(settings, &messages, &options).await?.text;
    let cleaned = strip_json_fence(&raw);
    serde_json::from_str(&cleaned)
        .map_err(|e| AppError::msg(format!("lore_extract JSON 解析失败: {e}; 原文前200字: {}", raw.chars().take(200).collect::<String>())))
}

fn merge_entity_into_lore(
    existing: Option<LoreEntry>,
    kind: &str,
    title: &str,
    aliases: &[String],
    content: &str,
    keywords: &[String],
    attrs: &BTreeMap<String, String>,
    links: &[Value],
    index: &AliasIndex,
) -> LoreEntry {
    let mut entry = existing.unwrap_or_else(|| LoreEntry {
        id: Uuid::new_v4().to_string(),
        kind: if kind == "character" {
            "character".into()
        } else {
            "world".into()
        },
        title: title.to_string(),
        content: String::new(),
        keywords: vec![],
        links: vec![],
        attrs: BTreeMap::new(),
        sources: vec![],
        unique: false,
        updated_at: String::new(),
    });
    if entry.kind.is_empty() {
        entry.kind = if kind == "character" {
            "character".into()
        } else {
            "world".into()
        };
    }
    if !content.is_empty() {
        if entry.content.is_empty() {
            entry.content = content.to_string();
        } else if !entry.content.contains(content) {
            entry.content = format!("{}\n{}", entry.content, content);
        }
    }
    for (k, v) in attrs {
        entry.attrs.insert(k.clone(), v.clone());
    }
    for k in keywords {
        if !entry.keywords.iter().any(|x| x == k) {
            entry.keywords.push(k.clone());
        }
    }
    for a in aliases {
        let key = format!("alias:{a}");
        if !entry.keywords.iter().any(|x| x == &key) {
            entry.keywords.push(key);
        }
    }
    for link in links {
        let target_name = link
            .get("target")
            .or_else(|| link.get("target_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let relation = link
            .get("relation")
            .and_then(|v| v.as_str())
            .unwrap_or("related");
        let target_id = index
            .resolve(target_name)
            .unwrap_or_else(|| target_name.to_string());
        if target_id.is_empty() {
            continue;
        }
        if !entry
            .links
            .iter()
            .any(|l| l.target_id == target_id && l.relation == relation)
        {
            entry.links.push(LoreLink {
                target_id,
                relation: relation.to_string(),
            });
        }
    }
    entry
}

fn extract_to_story_patch(
    extracted: &Value,
    chapter_id: &str,
    index: &AliasIndex,
) -> Value {
    let mut facts = Vec::new();
    if let Some(arr) = extracted.get("facts").and_then(|v| v.as_array()) {
        for f in arr {
            let text = f.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if text.is_empty() {
                continue;
            }
            let evidence = f.get("evidence").and_then(|v| v.as_str()).unwrap_or("");
            let full_text = if evidence.is_empty() {
                text
            } else {
                format!("{text}（证据：{evidence}）")
            };
            let mut related = Vec::new();
            if let Some(titles) = f.get("related_titles").and_then(|v| v.as_array()) {
                for t in titles {
                    if let Some(name) = t.as_str() {
                        if let Some(id) = index.resolve(name) {
                            related.push(id);
                        }
                    }
                }
            }
            facts.push(json!({
                "id": f.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "text": full_text,
                "locked": f.get("locked").and_then(|v| v.as_bool()).unwrap_or(false),
                "evidence_chapter_ids": [chapter_id],
                "related_lore_ids": related,
                "tags": f.get("tags").cloned().unwrap_or(json!([]))
            }));
        }
    }

    let mut events = Vec::new();
    if let Some(arr) = extracted.get("events").and_then(|v| v.as_array()) {
        for e in arr {
            let title = e.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() {
                continue;
            }
            let mut participants = Vec::new();
            if let Some(titles) = e.get("participant_titles").and_then(|v| v.as_array()) {
                for t in titles {
                    if let Some(name) = t.as_str() {
                        if let Some(id) = index.resolve(name) {
                            participants.push(id);
                        }
                    }
                }
            }
            events.push(json!({
                "id": e.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "story_time": e.get("story_time").and_then(|v| v.as_str()).unwrap_or(""),
                "title": title,
                "summary": e.get("summary").and_then(|v| v.as_str()).unwrap_or(""),
                "location": e.get("location").and_then(|v| v.as_str()).unwrap_or(""),
                "chapter_ids": [chapter_id],
                "participant_lore_ids": participants
            }));
        }
    }

    let mut edges = Vec::new();
    if let Some(arr) = extracted.get("edges").and_then(|v| v.as_array()) {
        for e in arr {
            let from_n = e.get("from").or_else(|| e.get("from_id")).and_then(|v| v.as_str()).unwrap_or("");
            let to_n = e.get("to").or_else(|| e.get("to_id")).and_then(|v| v.as_str()).unwrap_or("");
            let from_id = index.resolve(from_n).unwrap_or_else(|| from_n.to_string());
            let to_id = index.resolve(to_n).unwrap_or_else(|| to_n.to_string());
            if from_id.is_empty() || to_id.is_empty() {
                continue;
            }
            edges.push(json!({
                "id": e.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "from_id": from_id,
                "to_id": to_id,
                "kind": e.get("kind").and_then(|v| v.as_str()).unwrap_or("related"),
                "label": e.get("label").and_then(|v| v.as_str()).unwrap_or(""),
                "strength": e.get("strength").and_then(|v| v.as_u64()).unwrap_or(3),
                "public": e.get("public").and_then(|v| v.as_bool()).unwrap_or(true),
                "note": e.get("note").cloned().unwrap_or(Value::Null)
            }));
        }
    }

    let mut arcs = Vec::new();
    if let Some(arr) = extracted.get("arcs").and_then(|v| v.as_array()) {
        for a in arr {
            let title = a.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if title.is_empty() {
                continue;
            }
            let mut related = Vec::new();
            if let Some(titles) = a.get("related_titles").and_then(|v| v.as_array()) {
                for t in titles {
                    if let Some(name) = t.as_str() {
                        if let Some(id) = index.resolve(name) {
                            related.push(id);
                        }
                    }
                }
            }
            arcs.push(json!({
                "id": a.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "kind": a.get("kind").and_then(|v| v.as_str()).unwrap_or("sub"),
                "title": title,
                "goal": a.get("goal").and_then(|v| v.as_str()).unwrap_or(""),
                "status": a.get("status").and_then(|v| v.as_str()).unwrap_or("planted"),
                "progress_note": a.get("progress_note").and_then(|v| v.as_str()).unwrap_or(""),
                "related_lore_ids": related
            }));
        }
    }

    let mut promises = Vec::new();
    if let Some(arr) = extracted.get("promises").and_then(|v| v.as_array()) {
        for p in arr {
            let text = p.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if text.is_empty() {
                continue;
            }
            promises.push(json!({
                "id": p.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "text": text,
                "status": p.get("status").and_then(|v| v.as_str()).unwrap_or("open"),
                "planted_chapter_id": chapter_id,
                "arc_id": ""
            }));
        }
    }

    json!({
        "facts": facts,
        "events": events,
        "edges": edges,
        "arcs": arcs,
        "promises": promises
    })
}

fn apply_chapter_extract(
    root: &Path,
    chapter_id: &str,
    extracted: &Value,
    index: &mut AliasIndex,
) -> AppResult<(usize, usize, usize, usize)> {
    let mut entity_n = 0usize;
    // First pass: register / upsert entities so edges can resolve
    if let Some(entities) = extracted.get("entities").and_then(|v| v.as_array()) {
        for ent in entities {
            let title = ent.get("title").and_then(|v| v.as_str()).unwrap_or("").trim();
            if title.is_empty() {
                continue;
            }
            let kind = ent.get("kind").and_then(|v| v.as_str()).unwrap_or("world");
            let aliases: Vec<String> = ent
                .get("aliases")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let content = ent.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let keywords: Vec<String> = ent
                .get("keywords")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let attrs: BTreeMap<String, String> = ent
                .get("attrs")
                .and_then(|v| v.as_object())
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            let links = ent
                .get("links")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let existing_id = index.resolve(title).or_else(|| {
                aliases.iter().find_map(|a| index.resolve(a))
            });
            let existing = if let Some(id) = &existing_id {
                project::list_lore(root)?
                    .into_iter()
                    .find(|e| &e.id == id)
            } else {
                None
            };
            let entry = merge_entity_into_lore(
                existing,
                kind,
                title,
                &aliases,
                content,
                &keywords,
                &attrs,
                &links,
                index,
            );
            let saved = project::upsert_lore(root, entry)?;
            index.register(&saved.title, &saved.id, &aliases);
            entity_n += 1;
        }
    }

    // Refresh index from disk for link resolution
    let lore_now = project::list_lore(root)?;
    *index = AliasIndex::from_lore(&lore_now);

    let patch = extract_to_story_patch(extracted, chapter_id, index);
    let mut fact_n = 0usize;
    let mut edge_n = 0usize;
    let mut event_n = 0usize;
    if let Some(a) = patch.get("facts").and_then(|v| v.as_array()) {
        fact_n = a.len();
    }
    if let Some(a) = patch.get("edges").and_then(|v| v.as_array()) {
        edge_n = a.len();
    }
    if let Some(a) = patch.get("events").and_then(|v| v.as_array()) {
        event_n = a.len();
    }

    let has_any = ["facts", "events", "edges", "arcs", "promises"]
        .iter()
        .any(|k| {
            patch
                .get(*k)
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false)
        });
    if has_any {
        let _ = crate::story::apply_story_patch(root, &patch)?;
    }

    if let Some(summary) = extracted.get("summary").and_then(|v| v.as_str()) {
        if !summary.is_empty() {
            let _ = project::update_chapter_meta(
                root,
                chapter_id,
                project::ChapterMetaPatch {
                    summary: Some(summary.to_string()),
                    ..Default::default()
                },
            );
            let _ = project::upsert_chapter_snapshot(root, chapter_id, summary);
        }
    }

    Ok((entity_n, fact_n, edge_n, event_n))
}

fn merge_pending_json(path: &Path, key: &str, items: &[Value]) -> AppResult<()> {
    let mut root: Value = if path.exists() {
        serde_json::from_str(&fs::read_to_string(path)?)?
    } else {
        json!({})
    };
    let arr = root
        .as_object_mut()
        .ok_or_else(|| AppError::msg("pending json 根须为对象"))?
        .entry(key.to_string())
        .or_insert_with(|| json!([]));
    if let Some(a) = arr.as_array_mut() {
        a.extend(items.iter().cloned());
    }
    fs::write(path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

/// 按章蒸馏知识库。`from`/`to` 为 1-based 章序（含端点）。
pub async fn distill_range(
    root: &Path,
    from: usize,
    to: usize,
    apply: ApplyMode,
    resume: bool,
    job_id: Option<&str>,
    instruction: &str,
) -> AppResult<DistillReport> {
    if from == 0 || to == 0 || to < from {
        return Err(AppError::msg("from/to 须为从 1 起的章序，且 to >= from"));
    }
    let opened = project::open_project(root)?;
    let total_chapters = opened.project.chapters.len();
    if from > total_chapters {
        return Err(AppError::msg(format!(
            "from={from} 超出章数 {total_chapters}"
        )));
    }
    let to = to.min(total_chapters);
    let job_id = job_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let job_dir = ensure_job_dir(root, &job_id)?;
    let pending_lore = job_dir.join("pending_lore.json");
    let pending_story = job_dir.join("pending_story.json");

    let settings = crate::settings::load_settings()?;
    let client = LmStudioClient::new();
    let mut index = AliasIndex::from_lore(&project::list_lore(root)?);

    let mut processed = 0usize;
    let mut skipped = 0usize;
    let mut failed: Vec<String> = Vec::new();
    let mut entity_count = 0usize;
    let mut fact_count = 0usize;
    let mut edge_count = 0usize;
    let mut event_count = 0usize;

    let slice: Vec<_> = opened.project.chapters[(from - 1)..to].to_vec();
    let n = slice.len();

    for (i, ch) in slice.iter().enumerate() {
        let chap_no = from + i;
        let per_path = job_dir.join("per_chapter").join(format!("{chap_no}.json"));
        let _ = writeln!(
            std::io::stderr(),
            "[import distill] {}/{} chapter {chap_no}/{} {}",
            i + 1,
            n,
            to,
            ch.title
        );

        if resume && per_path.exists() {
            if let Ok(prev) = serde_json::from_str::<Value>(&fs::read_to_string(&per_path)?) {
                if prev.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                    skipped += 1;
                    if apply == ApplyMode::Auto {
                        if let Some(extracted) = prev.get("extract") {
                            match apply_chapter_extract(root, &ch.id, extracted, &mut index) {
                                Ok((e, f, ed, ev)) => {
                                    entity_count += e;
                                    fact_count += f;
                                    edge_count += ed;
                                    event_count += ev;
                                }
                                Err(err) => failed.push(format!("{chap_no}: resume-apply {err}")),
                            }
                        }
                    }
                    continue;
                }
            }
        }

        let (_, content) = match project::read_chapter(root, &ch.id) {
            Ok(v) => v,
            Err(e) => {
                failed.push(format!("{chap_no}: read {e}"));
                continue;
            }
        };

        // 1) lore extract（含 summary；不再单独跑 chapter_summary，省一次 LLM）
        let extracted = match call_lore_extract(
            &client,
            &settings,
            root,
            &ch.title,
            &content,
            instruction,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                failed.push(format!("{chap_no}: lore_extract {e}"));
                let err_doc = json!({
                    "ok": false,
                    "chapter_id": ch.id,
                    "title": ch.title,
                    "error": e.to_string()
                });
                let _ = fs::write(&per_path, serde_json::to_string_pretty(&err_doc)?);
                continue;
            }
        };
        let summary_text = extracted
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 2) story_sync 增量（失败不阻断）
        let sync_req = WritingRequest {
            project_root: root.to_string_lossy().to_string(),
            chapter_id: ch.id.clone(),
            task: "story_sync".into(),
            instruction: instruction.to_string(),
            selection: String::new(),
            block_key: String::new(),
            model: None,
            temperature: None,
            max_tokens: None,
            frequency_penalty: None,
            presence_penalty: None,
            fallback_model: None,
            retry_on_loop: Some(false),
            branch_context_text: None,
            active_beat_id: None,
            split_mode: None,
        };
        let sync_patch = match writing::run_writing(&client, &settings, &sync_req, None, |_| {}).await
        {
            Ok(out) => {
                let cleaned = strip_json_fence(&out.text);
                serde_json::from_str::<Value>(&cleaned).ok()
            }
            Err(e) => {
                eprintln!("[import distill] story_sync failed {chap_no}: {e}");
                None
            }
        };

        let doc = json!({
            "ok": true,
            "chapter_id": ch.id,
            "chapter_index": chap_no,
            "title": ch.title,
            "summary": summary_text,
            "extract": extracted,
            "story_sync": sync_patch
        });
        fs::write(&per_path, serde_json::to_string_pretty(&doc)?)?;

        // accumulate pending
        if let Some(ents) = extracted.get("entities").and_then(|v| v.as_array()) {
            let _ = merge_pending_json(&pending_lore, "entities", ents);
        }
        let story_bits = extract_to_story_patch(&extracted, &ch.id, &index);
        for key in ["facts", "events", "edges", "arcs", "promises"] {
            if let Some(arr) = story_bits.get(key).and_then(|v| v.as_array()) {
                let _ = merge_pending_json(&pending_story, key, arr);
            }
        }
        if let Some(sp) = &sync_patch {
            for key in ["facts", "events", "edges", "arcs", "promises"] {
                if let Some(arr) = sp.get(key).and_then(|v| v.as_array()) {
                    let _ = merge_pending_json(&pending_story, key, arr);
                }
            }
        }

        if apply == ApplyMode::Auto {
            match apply_chapter_extract(root, &ch.id, &extracted, &mut index) {
                Ok((e, f, ed, ev)) => {
                    entity_count += e;
                    fact_count += f;
                    edge_count += ed;
                    event_count += ev;
                }
                Err(err) => failed.push(format!("{chap_no}: apply {err}")),
            }
            if let Some(sp) = &sync_patch {
                let has_any = ["facts", "events", "edges", "arcs", "promises"]
                    .iter()
                    .any(|k| {
                        sp.get(*k)
                            .and_then(|v| v.as_array())
                            .map(|a| !a.is_empty())
                            .unwrap_or(false)
                    });
                if has_any {
                    if let Err(err) = crate::story::apply_story_patch(root, sp) {
                        failed.push(format!("{chap_no}: story_sync apply {err}"));
                    }
                }
            }
        } else if let Some(ents) = extracted.get("entities").and_then(|v| v.as_array()) {
            entity_count += ents.len();
            if let Some(a) = extracted.get("facts").and_then(|v| v.as_array()) {
                fact_count += a.len();
            }
            if let Some(a) = extracted.get("edges").and_then(|v| v.as_array()) {
                edge_count += a.len();
            }
            if let Some(a) = extracted.get("events").and_then(|v| v.as_array()) {
                event_count += a.len();
            }
        }

        let _ = genlog::append_log(&genlog::make_entry(
            "import_distill",
            &root.to_string_lossy(),
            &ch.id,
            &format!("distilled chapter {chap_no} {}", ch.title),
            "import",
        ));
        processed += 1;

        // flush progress hint
        let _ = writeln!(
            std::io::stderr(),
            "[import distill] {chap_no}/{to} {} ok",
            ch.title
        );
    }

    let applied = apply == ApplyMode::Auto;
    if applied && failed.is_empty() {
        if let Err(e) = crate::kb::sync_to_universal(root) {
            eprintln!("[import distill] sync_to_universal failed: {e}");
        }
    }
    let report = DistillReport {
        ok: failed.is_empty(),
        root: root.to_string_lossy().to_string(),
        job_id: job_id.clone(),
        job_dir: job_dir.to_string_lossy().to_string(),
        from,
        to,
        processed,
        skipped_resume: skipped,
        failed: failed.clone(),
        entity_count,
        fact_count,
        edge_count,
        event_count,
        applied,
    };
    fs::write(
        job_dir.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    Ok(report)
}

/// 将某次 job 的 pending 应用到作品（apply none 之后的确认步骤）
pub fn apply_pending_job(root: &Path, job_id: &str) -> AppResult<Value> {
    let job_dir = jobs_dir(root).join(job_id);
    if !job_dir.is_dir() {
        return Err(AppError::msg(format!("job 不存在: {}", job_dir.display())));
    }
    let mut index = AliasIndex::from_lore(&project::list_lore(root)?);
    let mut entity_count = 0usize;
    let mut fact_count = 0usize;
    let mut edge_count = 0usize;
    let mut event_count = 0usize;

    let per_dir = job_dir.join("per_chapter");
    if per_dir.is_dir() {
        let mut files: Vec<_> = fs::read_dir(&per_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        files.sort();
        for path in files {
            let doc: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            if doc.get("ok").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            let chapter_id = doc
                .get("chapter_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if chapter_id.is_empty() {
                continue;
            }
            if let Some(extracted) = doc.get("extract") {
                let (e, f, ed, ev) =
                    apply_chapter_extract(root, chapter_id, extracted, &mut index)?;
                entity_count += e;
                fact_count += f;
                edge_count += ed;
                event_count += ev;
            }
            if let Some(sp) = doc.get("story_sync") {
                let has_any = ["facts", "events", "edges", "arcs", "promises"]
                    .iter()
                    .any(|k| {
                        sp.get(*k)
                            .and_then(|v| v.as_array())
                            .map(|a| !a.is_empty())
                            .unwrap_or(false)
                    });
                if has_any {
                    let _ = crate::story::apply_story_patch(root, sp);
                }
            }
        }
    }

    Ok(json!({
        "ok": true,
        "job_id": job_id,
        "entity_count": entity_count,
        "fact_count": fact_count,
        "edge_count": edge_count,
        "event_count": event_count
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("kk_novel_import_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(name);
        let mut f = File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn parse_eq_headings() {
        let path = write_tmp(
            "sample.txt",
            "来源: http://example.com\n\n===序章 缘起===\n\n正文甲\n\n===第一章 寻仙===\n\n正文乙\n",
        );
        let ch = parse_txt_chapters(&path).unwrap();
        assert_eq!(ch.len(), 2);
        assert_eq!(ch[0].title, "序章 缘起");
        assert!(ch[0].body.contains("正文甲"));
        assert_eq!(ch[1].title, "第一章 寻仙");
        assert!(ch[1].body.contains("正文乙"));
    }
}
