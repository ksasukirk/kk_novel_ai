//! 跨书压缩目录卡：作品页智能搜索缓存
//! 代码路径: kk_novel_ai/src-tauri/src/project/work_catalog.rs

use super::{is_knowledge_kind, is_trope_kind, list_lore, load_memory, open_project};
use crate::error::{AppError, AppResult};
use crate::genlog;
use crate::llm::{ChatMessage, ChatOptions, LmStudioClient};
use crate::settings;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const CACHE_VERSION: u32 = 1;
const OUTLINE_CHARS: usize = 400;
const MEMORY_CHARS: usize = 300;
const CHAPTER_SUMMARY_CHARS: usize = 80;
const MAX_CHAPTERS: usize = 20;
const MAX_TROPES_EACH: usize = 30;
const LLM_CARD_CHARS: usize = 400;
const LLM_MAX_CARDS: usize = 80;
const LLM_MIX_ZEROS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkCatalogCard {
    pub root: String,
    pub title: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub outline: String,
    #[serde(default)]
    pub chapters: String,
    #[serde(default)]
    pub memory: String,
    #[serde(default)]
    pub tropes: Vec<String>,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    items: HashMap<String, WorkCatalogCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub root: String,
    pub title: String,
    pub score: f32,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub match_fields: Vec<String>,
}

#[derive(Deserialize)]
struct LlmHitsFile {
    #[serde(default)]
    hits: Vec<LlmHit>,
}

#[derive(Deserialize, Default)]
struct LlmHit {
    #[serde(default)]
    id: String,
    #[serde(default)]
    root: String,
    #[serde(default)]
    score: f64,
    #[serde(default)]
    reason: String,
}

pub fn catalog_cache_path() -> AppResult<PathBuf> {
    Ok(crate::paths::app_data_dir()?.join("work_catalog.json"))
}

pub fn normalize_root(s: &str) -> String {
    s.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn take_chars(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

fn is_punct(c: char) -> bool {
    matches!(
        c,
        ',' | '.'
            | ';'
            | ':'
            | '!'
            | '?'
            | '"'
            | '\''
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '/'
            | '\\'
            | '|'
            | '_'
            | '~'
            | '`'
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '&'
            | '*'
            | '+'
            | '='
            | '，'
            | '。'
            | '、'
            | '；'
            | '：'
            | '！'
            | '？'
            | '「'
            | '」'
            | '『'
            | '』'
            | '（'
            | '）'
            | '【'
            | '】'
            | '《'
            | '》'
            | '—'
            | '…'
            | '·'
            | '・'
            | '\u{3000}'
    )
}

fn file_sig(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(m) => {
            let len = m.len();
            let mt = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("{len}:{mt}")
        }
        Err(_) => "0:0".into(),
    }
}

pub fn fingerprint_root(root: &Path) -> String {
    [
        file_sig(&root.join("project.json")),
        file_sig(&root.join("memory.json")),
        file_sig(&root.join("lore").join("tropes.json")),
        file_sig(&root.join("lore").join("kinks.json")),
    ]
    .join("|")
}

fn load_cache() -> CatalogFile {
    let path = match catalog_cache_path() {
        Ok(p) => p,
        Err(_) => return empty_cache(),
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return empty_cache();
    };
    serde_json::from_str(&text).unwrap_or_else(|_| empty_cache())
}

fn empty_cache() -> CatalogFile {
    CatalogFile {
        version: CACHE_VERSION,
        updated_at: String::new(),
        items: HashMap::new(),
    }
}

fn save_cache(file: &CatalogFile) -> AppResult<()> {
    let path = catalog_cache_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(file)?)?;
    Ok(())
}

pub fn build_card(root: &Path, title_hint: Option<&str>) -> AppResult<WorkCatalogCard> {
    let opened = open_project(root)?;
    if is_knowledge_kind(&opened.project.kind) {
        return Err(AppError::t("errors.kbNoSuggestTitle"));
    }
    let p = &opened.project;
    let title = if p.title.trim().is_empty() {
        title_hint.unwrap_or("").to_string()
    } else {
        p.title.clone()
    };
    let outline = take_chars(p.book_outline.trim(), OUTLINE_CHARS);
    let mut chapter_lines: Vec<String> = Vec::new();
    for ch in p.chapters.iter().take(MAX_CHAPTERS) {
        let t = ch.title.trim();
        let s = take_chars(ch.summary.trim(), CHAPTER_SUMMARY_CHARS);
        if s.is_empty() {
            chapter_lines.push(format!("- {t}"));
        } else {
            chapter_lines.push(format!("- {t}：{s}"));
        }
    }
    let mut memory = String::new();
    if let Ok(mem) = load_memory(root) {
        memory = take_chars(mem.rolling_summary.trim(), MEMORY_CHARS);
    }
    let mut tropes: Vec<String> = Vec::new();
    if let Ok(lore) = list_lore(root) {
        let mut trope_n = 0usize;
        let mut kink_n = 0usize;
        for e in lore {
            if !is_trope_kind(&e.kind) {
                continue;
            }
            let cap = if e.kind == "kink" {
                &mut kink_n
            } else {
                &mut trope_n
            };
            if *cap >= MAX_TROPES_EACH {
                continue;
            }
            *cap += 1;
            let mut label = e.title.trim().to_string();
            let kws: Vec<&str> = e
                .keywords
                .iter()
                .map(|k| k.trim())
                .filter(|k| !k.is_empty())
                .take(4)
                .collect();
            if !kws.is_empty() {
                label.push(' ');
                label.push_str(&kws.join(" "));
            }
            if !label.is_empty() {
                tropes.push(label);
            }
        }
    }
    Ok(WorkCatalogCard {
        root: root.to_string_lossy().to_string(),
        title,
        genre: p.genre.clone(),
        style: p.style.clone(),
        outline,
        chapters: chapter_lines.join("\n"),
        memory,
        tropes,
        fingerprint: fingerprint_root(root),
    })
}

/// `roots`: (path, title_hint)
pub fn ensure_cards(roots: &[(String, String)], force: bool) -> AppResult<Vec<WorkCatalogCard>> {
    let mut cache = load_cache();
    if cache.version != CACHE_VERSION {
        cache = empty_cache();
    }
    let mut keep: HashSet<String> = HashSet::new();
    let mut out: Vec<WorkCatalogCard> = Vec::new();
    for (root_s, hint) in roots {
        let path = Path::new(root_s);
        let key = normalize_root(root_s);
        keep.insert(key.clone());
        let fp = fingerprint_root(path);
        if !force {
            if let Some(cached) = cache.items.get(&key) {
                if cached.fingerprint == fp && !cached.title.is_empty() {
                    let mut card = cached.clone();
                    card.root = root_s.clone();
                    out.push(card);
                    continue;
                }
            }
        }
        match build_card(path, Some(hint.as_str())) {
            Ok(card) => {
                cache.items.insert(key, card.clone());
                out.push(card);
            }
            Err(_) => {
                if let Some(cached) = cache.items.get(&key) {
                    let mut card = cached.clone();
                    card.root = root_s.clone();
                    out.push(card);
                }
            }
        }
    }
    cache.items.retain(|k, _| keep.contains(k));
    cache.version = CACHE_VERSION;
    cache.updated_at = Utc::now().to_rfc3339();
    let _ = save_cache(&cache);
    Ok(out)
}

fn norm_text(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !is_punct(*c))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn jaccard_chars(a: &str, b: &str) -> f32 {
    let sa: HashSet<char> = a.chars().collect();
    let sb: HashSet<char> = b.chars().collect();
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    if union == 0 {
        0.0
    } else {
        inter as f32 / union as f32
    }
}

fn titles_similar(a: &str, b: &str) -> bool {
    let na = norm_text(a);
    let nb = norm_text(b);
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    if na == nb {
        return true;
    }
    let (short, long) = if na.chars().count() <= nb.chars().count() {
        (&na, &nb)
    } else {
        (&nb, &na)
    };
    if short.chars().count() >= 3 && long.contains(short) {
        return true;
    }
    let sa: HashSet<char> = na.chars().collect();
    let sb: HashSet<char> = nb.chars().collect();
    let inter = sa.intersection(&sb).count();
    inter >= 2 && jaccard_chars(&na, &nb) >= 0.55
}

pub fn tokenize_query(q: &str) -> Vec<String> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out = vec![q.clone()];
    for part in q.split(|c: char| c.is_whitespace() || is_punct(c)) {
        let p = part.trim();
        if p.chars().count() >= 2 && p != q && !out.iter().any(|x| x == p) {
            out.push(p.to_string());
        }
    }
    out
}

fn contains_ci(hay: &str, token: &str) -> bool {
    hay.to_lowercase().contains(token)
}

pub fn score_card(card: &WorkCatalogCard, tokens: &[String]) -> SearchHit {
    let mut score = 0.0f32;
    let mut fields: Vec<String> = Vec::new();
    let tropes_hay = card.tropes.join(" ");
    let push_field = |name: &str, bag: &mut Vec<String>| {
        if !bag.iter().any(|x| x == name) {
            bag.push(name.to_string());
        }
    };
    for tok in tokens {
        if contains_ci(&card.title, tok) {
            score += 100.0;
            push_field("title", &mut fields);
        } else if titles_similar(&card.title, tok) {
            score += 70.0;
            push_field("title", &mut fields);
        }
        if contains_ci(&tropes_hay, tok)
            || card.tropes.iter().any(|t| titles_similar(t, tok))
        {
            score += if contains_ci(&tropes_hay, tok) {
                50.0
            } else {
                35.0
            };
            push_field("tropes", &mut fields);
        }
        if contains_ci(&card.outline, tok) {
            score += 25.0;
            push_field("outline", &mut fields);
        }
        if contains_ci(&card.chapters, tok) {
            score += 25.0;
            push_field("chapters", &mut fields);
        }
        if contains_ci(&card.memory, tok) {
            score += 25.0;
            push_field("memory", &mut fields);
        }
        if contains_ci(&card.genre, tok) || contains_ci(&card.style, tok) {
            score += 20.0;
            push_field("genre", &mut fields);
        }
        if contains_ci(&card.root, tok) {
            score += 10.0;
            push_field("path", &mut fields);
        }
    }
    SearchHit {
        root: card.root.clone(),
        title: card.title.clone(),
        score,
        reason: String::new(),
        match_fields: fields,
    }
}

pub fn score_local(query: &str, cards: &[WorkCatalogCard]) -> Vec<SearchHit> {
    let tokens = tokenize_query(query);
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<SearchHit> = cards
        .iter()
        .map(|c| score_card(c, &tokens))
        .filter(|h| h.score > 0.0)
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
    });
    hits
}

fn compact_card(card: &WorkCatalogCard, id: &str) -> String {
    let tropes = take_chars(&card.tropes.iter().take(16).cloned().collect::<Vec<_>>().join(" / "), 160);
    let chapters = take_chars(&card.chapters, 160);
    let raw = format!(
        "### {id}\ntitle: {}\nroot: {}\ngenre: {} {}\noutline: {}\ntropes: {}\nchapters: {}\nmemory: {}",
        card.title,
        card.root,
        card.genre,
        card.style,
        take_chars(&card.outline, 180),
        tropes,
        chapters,
        take_chars(&card.memory, 120)
    );
    take_chars(&raw, LLM_CARD_CHARS)
}

fn pick_for_llm(query: &str, cards: &[WorkCatalogCard]) -> Vec<(String, WorkCatalogCard)> {
    let tokens = tokenize_query(query);
    let mut ranked: Vec<(f32, usize)> = cards
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let s = if tokens.is_empty() {
                0.0
            } else {
                score_card(c, &tokens).score
            };
            (s, i)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    if ranked.len() <= LLM_MAX_CARDS {
        return ranked
            .into_iter()
            .enumerate()
            .map(|(n, (_, i))| (format!("w{:02}", n + 1), cards[i].clone()))
            .collect();
    }
    let mut chosen: Vec<usize> = Vec::new();
    let mut zeros: Vec<usize> = Vec::new();
    for (score, i) in &ranked {
        if *score > 0.0 {
            if chosen.len() < LLM_MAX_CARDS - LLM_MIX_ZEROS {
                chosen.push(*i);
            }
        } else if zeros.len() < LLM_MIX_ZEROS {
            zeros.push(*i);
        }
    }
    for i in zeros {
        if chosen.len() >= LLM_MAX_CARDS {
            break;
        }
        chosen.push(i);
    }
    for (_score, i) in &ranked {
        if chosen.len() >= LLM_MAX_CARDS {
            break;
        }
        if !chosen.contains(i) {
            chosen.push(*i);
        }
    }
    chosen
        .into_iter()
        .enumerate()
        .map(|(n, i)| (format!("w{:02}", n + 1), cards[i].clone()))
        .collect()
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
    if let (Some(s), Some(e)) = (t.find('{'), t.rfind('}')) {
        if e > s {
            return t[s..=e].to_string();
        }
    }
    t.to_string()
}

fn ui_lang_label() -> (String, &'static str) {
    let loc = match settings::load_settings() {
        Ok(s) => settings::AppSettings::normalize_locale_code(&s.ui_locale),
        Err(_) => "zh-CN".into(),
    };
    match loc.as_str() {
        "en" => (loc, "English"),
        "ja" => (loc, "日本語"),
        _ => (loc, "中文"),
    }
}

pub async fn search_ai(query: &str, cards: &[WorkCatalogCard]) -> AppResult<Value> {
    let picked = pick_for_llm(query, cards);
    if picked.is_empty() {
        return Ok(json!({
            "ok": true,
            "mode": "ai",
            "items": [],
        }));
    }
    let mut by_id: HashMap<String, WorkCatalogCard> = HashMap::new();
    let mut by_root: HashMap<String, WorkCatalogCard> = HashMap::new();
    let mut catalog_parts: Vec<String> = Vec::new();
    for (id, card) in &picked {
        catalog_parts.push(compact_card(card, id));
        by_id.insert(id.clone(), card.clone());
        by_root.insert(normalize_root(&card.root), card.clone());
    }
    let catalog = catalog_parts.join("\n\n");
    let (_loc, lang) = ui_lang_label();
    let s = settings::load_settings()?;
    let model = s.resolve_analysis_model().to_string();
    let tpl = crate::prompt_i18n::prompt("novel_search.md");
    let user = tpl
        .replace("{{query}}", query)
        .replace("{{catalog}}", &catalog)
        .replace("{{lang}}", lang);
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: "You rank novels from a given catalog. Output JSON only.".into(),
        },
        ChatMessage {
            role: "user".into(),
            content: user,
        },
    ];
    let options = ChatOptions {
        model: Some(model.clone()),
        temperature: Some(0.2),
        max_tokens: Some(2048),
        stream: false,
        ..Default::default()
    };
    let client = LmStudioClient::from_settings(&s);
    let r = client.chat(&s, &messages, &options).await?;
    let parsed: LlmHitsFile =
        serde_json::from_str(&strip_json_fence(&r.text)).unwrap_or(LlmHitsFile { hits: vec![] });
    let mut seen = HashSet::new();
    let mut items: Vec<SearchHit> = Vec::new();
    for hit in parsed.hits {
        let card = by_id
            .get(hit.id.trim())
            .or_else(|| by_root.get(&normalize_root(&hit.root)));
        let Some(card) = card else {
            continue;
        };
        let key = normalize_root(&card.root);
        if !seen.insert(key) {
            continue;
        }
        let score = if hit.score.is_finite() {
            hit.score.clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        items.push(SearchHit {
            root: card.root.clone(),
            title: card.title.clone(),
            score,
            reason: hit.reason.trim().chars().take(80).collect(),
            match_fields: vec!["ai".into()],
        });
    }
    items.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let novels_dir = crate::paths::novels_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let entry = genlog::record_llm_call(
        "novel_search",
        &novels_dir,
        "",
        &r.text,
        &serde_json::to_string(&items).unwrap_or_default(),
        "novels_search",
        false,
        &model,
        "novel_search",
        &messages,
        Some(r.usage.clone()),
        &s,
    )?;
    Ok(json!({
        "ok": true,
        "mode": "ai",
        "items": items,
        "model": model,
        "usage": r.usage,
        "log_id": entry.id,
        "cost_cny": entry.cost_cny,
        "catalog_sent": picked.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_keeps_whole_and_parts() {
        let t = tokenize_query("露出 憋尿");
        assert!(t.contains(&"露出 憋尿".to_string()) || t.iter().any(|x| x.contains("露出")));
        assert!(t.iter().any(|x| x == "露出"));
        assert!(t.iter().any(|x| x == "憋尿"));
    }

    #[test]
    fn score_hits_title() {
        let card = WorkCatalogCard {
            root: "D:/novels/foo".into(),
            title: "露出日记".into(),
            ..Default::default()
        };
        let hits = score_local("露出", &[card]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].match_fields.contains(&"title".to_string()));
    }
}
