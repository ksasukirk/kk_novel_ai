//! Novel OS 总谱存储（plot / timeline / relations / canon）
//! 代码路径: kk_novel_ai/src-tauri/src/story/mod.rs

use crate::error::{AppError, AppResult};
use crate::project::{self, LoreLink};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn story_dir(root: &Path) -> PathBuf {
    root.join("story")
}

fn ensure_story_dir(root: &Path) -> AppResult<()> {
    fs::create_dir_all(story_dir(root))?;
    Ok(())
}

fn read_json_or_default<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> AppResult<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

// --- plot ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlotStore {
    #[serde(default)]
    pub arcs: Vec<StoryArc>,
    #[serde(default)]
    pub promises: Vec<StoryPromise>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryArc {
    pub id: String,
    #[serde(default = "default_arc_kind")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default = "default_arc_status")]
    pub status: String,
    #[serde(default)]
    pub progress_note: String,
    #[serde(default)]
    pub payoff_chapter_id: Option<String>,
    #[serde(default)]
    pub related_lore_ids: Vec<String>,
}

fn default_arc_kind() -> String {
    "main".into()
}
fn default_arc_status() -> String {
    "active".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryPromise {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub planted_chapter_id: Option<String>,
    #[serde(default = "default_promise_status")]
    pub status: String,
    #[serde(default)]
    pub payoff_chapter_id: Option<String>,
    #[serde(default)]
    pub arc_id: Option<String>,
}

fn default_promise_status() -> String {
    "open".into()
}

pub fn load_plot(root: &Path) -> AppResult<PlotStore> {
    read_json_or_default(&story_dir(root).join("plot.json"))
}

pub fn save_plot(root: &Path, plot: &PlotStore) -> AppResult<()> {
    ensure_story_dir(root)?;
    write_json(&story_dir(root).join("plot.json"), plot)
}

// --- timeline ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimelineStore {
    #[serde(default)]
    pub calendar_note: String,
    #[serde(default)]
    pub events: Vec<TimelineEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    #[serde(default)]
    pub story_time: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub chapter_ids: Vec<String>,
    #[serde(default)]
    pub participant_lore_ids: Vec<String>,
    #[serde(default)]
    pub consequences: Option<String>,
}

pub fn load_timeline(root: &Path) -> AppResult<TimelineStore> {
    read_json_or_default(&story_dir(root).join("timeline.json"))
}

pub fn save_timeline(root: &Path, timeline: &TimelineStore) -> AppResult<()> {
    ensure_story_dir(root)?;
    write_json(&story_dir(root).join("timeline.json"), timeline)
}

// --- relations ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelationsStore {
    #[serde(default)]
    pub edges: Vec<RelationEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationEdge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    #[serde(default = "default_edge_kind")]
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_strength")]
    pub strength: u8,
    #[serde(default = "default_true")]
    pub public: bool,
    #[serde(default)]
    pub since_story_time: Option<String>,
    #[serde(default)]
    pub until_story_time: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_edge_kind() -> String {
    "related".into()
}
fn default_strength() -> u8 {
    3
}
fn default_true() -> bool {
    true
}

pub fn load_relations(root: &Path) -> AppResult<RelationsStore> {
    read_json_or_default(&story_dir(root).join("relations.json"))
}

/// 保存关系边，并回写相关 lore.links 摘要
pub fn save_relations(root: &Path, relations: &RelationsStore) -> AppResult<()> {
    ensure_story_dir(root)?;
    write_json(&story_dir(root).join("relations.json"), relations)?;
    sync_lore_links_from_relations(root, relations)?;
    Ok(())
}

fn sync_lore_links_from_relations(root: &Path, relations: &RelationsStore) -> AppResult<()> {
    let mut lore = project::list_lore(root)?;
    for entry in &mut lore {
        let mut links: Vec<LoreLink> = relations
            .edges
            .iter()
            .filter(|e| e.from_id == entry.id)
            .map(|e| LoreLink {
                target_id: e.to_id.clone(),
                relation: if e.label.is_empty() {
                    e.kind.clone()
                } else {
                    e.label.clone()
                },
            })
            .collect();
        // 去重 target
        links.sort_by(|a, b| a.target_id.cmp(&b.target_id));
        links.dedup_by(|a, b| a.target_id == b.target_id);
        if entry.links != links {
            entry.links = links;
            let _ = project::upsert_lore(root, entry.clone());
        }
    }
    Ok(())
}

// --- canon ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CanonStore {
    #[serde(default)]
    pub facts: Vec<CanonFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonFact {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub evidence_chapter_ids: Vec<String>,
    #[serde(default)]
    pub related_lore_ids: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

pub fn load_canon(root: &Path) -> AppResult<CanonStore> {
    read_json_or_default(&story_dir(root).join("canon.json"))
}

pub fn save_canon(root: &Path, canon: &CanonStore) -> AppResult<()> {
    ensure_story_dir(root)?;
    write_json(&story_dir(root).join("canon.json"), canon)
}

// --- storyboard（分镜表，不接入 apply_story_patch / story_sync） ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryboardImageMeta {
    #[serde(default)]
    pub rel: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub negative: String,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryboardShot {
    pub id: String,
    #[serde(default)]
    pub beat_id: Option<String>,
    #[serde(default)]
    pub seq: u32,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub character_lore_ids: Vec<String>,
    #[serde(default)]
    pub visual: String,
    #[serde(default)]
    pub dialogue: String,
    #[serde(default)]
    pub mood: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub image: Option<StoryboardImageMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryboardChapter {
    pub chapter_id: String,
    #[serde(default)]
    pub shots: Vec<StoryboardShot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryboardStore {
    #[serde(default)]
    pub style_prefix: String,
    #[serde(default)]
    pub negative: String,
    #[serde(default)]
    pub chapters: Vec<StoryboardChapter>,
}

pub fn load_storyboard(root: &Path) -> AppResult<StoryboardStore> {
    read_json_or_default(&story_dir(root).join("storyboard.json"))
}

pub fn save_storyboard(root: &Path, board: &StoryboardStore) -> AppResult<()> {
    ensure_story_dir(root)?;
    write_json(&story_dir(root).join("storyboard.json"), board)
}

// --- prompt helpers ---

pub fn plot_for_prompt(plot: &PlotStore, focus_arc_ids: &[String]) -> String {
    let mut arcs: Vec<&StoryArc> = if focus_arc_ids.is_empty() {
        plot.arcs.iter().filter(|a| a.status == "active" || a.status == "planted").collect()
    } else {
        plot.arcs
            .iter()
            .filter(|a| focus_arc_ids.contains(&a.id))
            .collect()
    };
    if arcs.is_empty() {
        arcs = plot.arcs.iter().take(5).collect();
    }
    let mut lines = Vec::new();
    if arcs.is_empty() {
        lines.push("（无故事弧）".into());
    } else {
        for a in arcs {
            lines.push(format!(
                "- [{}|{}] {} | 目标:{} | 状态:{} | 进度:{}",
                a.kind, a.id, a.title, a.goal, a.status, a.progress_note
            ));
        }
    }
    let open: Vec<_> = plot
        .promises
        .iter()
        .filter(|p| p.status == "open")
        .take(12)
        .collect();
    if open.is_empty() {
        lines.push("未回收承诺:（无）".into());
    } else {
        lines.push("未回收承诺:".into());
        for p in open {
            lines.push(format!("- [{}] {}", p.id, p.text));
        }
    }
    lines.join("\n")
}

pub fn timeline_for_prompt(tl: &TimelineStore, limit: usize) -> String {
    if tl.events.is_empty() {
        return "（无时间线事件）".into();
    }
    let mut evs = tl.events.clone();
    evs.sort_by(|a, b| a.story_time.cmp(&b.story_time));
    let start = evs.len().saturating_sub(limit);
    let mut out = String::new();
    if !tl.calendar_note.is_empty() {
        out.push_str(&format!("纪年: {}\n", tl.calendar_note));
    }
    for e in &evs[start..] {
        out.push_str(&format!(
            "- [{}] {} | {} | {}\n",
            e.story_time, e.title, e.summary, e.location.as_deref().unwrap_or("")
        ));
    }
    out
}

pub fn relations_for_prompt(rel: &RelationsStore, seed_ids: &[String], limit: usize) -> String {
    if rel.edges.is_empty() {
        return "（无关系边）".into();
    }
    let mut picked: Vec<&RelationEdge> = if seed_ids.is_empty() {
        rel.edges.iter().take(limit).collect()
    } else {
        rel.edges
            .iter()
            .filter(|e| seed_ids.contains(&e.from_id) || seed_ids.contains(&e.to_id))
            .take(limit)
            .collect()
    };
    if picked.is_empty() {
        picked = rel.edges.iter().take(limit).collect();
    }
    picked
        .iter()
        .map(|e| {
            format!(
                "- {} -[{}/{}]-> {} (str={})",
                e.from_id, e.kind, e.label, e.to_id, e.strength
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn canon_for_prompt(canon: &CanonStore, locked_only: bool) -> String {
    let facts: Vec<_> = canon
        .facts
        .iter()
        .filter(|f| !locked_only || f.locked)
        .collect();
    if facts.is_empty() {
        return if locked_only {
            "（无锁定 Canon）".into()
        } else {
            "（无 Canon）".into()
        };
    }
    facts
        .iter()
        .map(|f| {
            format!(
                "- [{}] {}{}",
                f.id,
                f.text,
                if f.locked { " [LOCKED]" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn neighbor_lore_ids(rel: &RelationsStore, seeds: &[String]) -> Vec<String> {
    let mut out = seeds.to_vec();
    for e in &rel.edges {
        if seeds.contains(&e.from_id) {
            out.push(e.to_id.clone());
        }
        if seeds.contains(&e.to_id) {
            out.push(e.from_id.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// 应用 story_sync 产出的 JSON patch（增量 upsert）
pub fn apply_story_patch(root: &Path, patch: &Value) -> AppResult<Value> {
    let mut report = json!({ "ok": true, "updated": [] });

    if let Some(arcs) = patch.get("arcs").and_then(|v| v.as_array()) {
        let mut plot = load_plot(root)?;
        for item in arcs {
            let mut arc: StoryArc = serde_json::from_value(item.clone())?;
            if arc.id.is_empty() {
                arc.id = Uuid::new_v4().to_string();
            }
            if let Some(pos) = plot.arcs.iter().position(|a| a.id == arc.id) {
                plot.arcs[pos] = arc;
            } else {
                plot.arcs.push(arc);
            }
        }
        save_plot(root, &plot)?;
        report["updated"]
            .as_array_mut()
            .unwrap()
            .push(json!("arcs"));
    }

    if let Some(promises) = patch.get("promises").and_then(|v| v.as_array()) {
        let mut plot = load_plot(root)?;
        for item in promises {
            let mut p: StoryPromise = serde_json::from_value(item.clone())?;
            if p.id.is_empty() {
                p.id = Uuid::new_v4().to_string();
            }
            if let Some(pos) = plot.promises.iter().position(|x| x.id == p.id) {
                plot.promises[pos] = p;
            } else {
                plot.promises.push(p);
            }
        }
        save_plot(root, &plot)?;
        report["updated"]
            .as_array_mut()
            .unwrap()
            .push(json!("promises"));
    }

    if let Some(events) = patch.get("events").and_then(|v| v.as_array()) {
        let mut tl = load_timeline(root)?;
        for item in events {
            let mut e: TimelineEvent = serde_json::from_value(item.clone())?;
            if e.id.is_empty() {
                e.id = Uuid::new_v4().to_string();
            }
            if let Some(pos) = tl.events.iter().position(|x| x.id == e.id) {
                tl.events[pos] = e;
            } else {
                tl.events.push(e);
            }
        }
        save_timeline(root, &tl)?;
        report["updated"]
            .as_array_mut()
            .unwrap()
            .push(json!("events"));
    }

    if let Some(edges) = patch.get("edges").and_then(|v| v.as_array()) {
        let mut rel = load_relations(root)?;
        for item in edges {
            let mut e: RelationEdge = serde_json::from_value(item.clone())?;
            if e.id.is_empty() {
                e.id = Uuid::new_v4().to_string();
            }
            if let Some(pos) = rel.edges.iter().position(|x| x.id == e.id) {
                rel.edges[pos] = e;
            } else {
                rel.edges.push(e);
            }
        }
        save_relations(root, &rel)?;
        report["updated"]
            .as_array_mut()
            .unwrap()
            .push(json!("edges"));
    }

    if let Some(facts) = patch.get("facts").and_then(|v| v.as_array()) {
        let mut canon = load_canon(root)?;
        for item in facts {
            let mut f: CanonFact = serde_json::from_value(item.clone())?;
            if f.id.is_empty() {
                f.id = Uuid::new_v4().to_string();
            }
            if let Some(pos) = canon.facts.iter().position(|x| x.id == f.id) {
                canon.facts[pos] = f;
            } else {
                canon.facts.push(f);
            }
        }
        save_canon(root, &canon)?;
        report["updated"]
            .as_array_mut()
            .unwrap()
            .push(json!("facts"));
    }

    if report["updated"].as_array().map(|a| a.is_empty()).unwrap_or(true) {
        return Err(AppError::msg("patch 中无可识别字段（arcs/promises/events/edges/facts）"));
    }
    Ok(report)
}

pub fn dashboard_summary(root: &Path) -> AppResult<Value> {
    let plot = load_plot(root)?;
    let tl = load_timeline(root)?;
    let canon = load_canon(root)?;
    let rel = load_relations(root)?;
    let open_promises = plot.promises.iter().filter(|p| p.status == "open").count();
    let locked = canon.facts.iter().filter(|f| f.locked).count();
    let active_arcs: Vec<_> = plot
        .arcs
        .iter()
        .filter(|a| a.status == "active" || a.status == "planted")
        .map(|a| {
            json!({
                "id": a.id,
                "kind": a.kind,
                "title": a.title,
                "status": a.status,
                "progress_note": a.progress_note
            })
        })
        .collect();
    let main = plot.arcs.iter().find(|a| a.kind == "main");
    let mut events = tl.events.clone();
    events.sort_by(|a, b| a.story_time.cmp(&b.story_time));
    let current_story_time = events.last().map(|e| e.story_time.clone()).unwrap_or_default();
    Ok(json!({
        "ok": true,
        "open_promises": open_promises,
        "locked_canon": locked,
        "edge_count": rel.edges.len(),
        "event_count": tl.events.len(),
        "current_story_time": current_story_time,
        "main_arc": main,
        "active_arcs": active_arcs
    }))
}
