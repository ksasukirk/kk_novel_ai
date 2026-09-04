//! 知识库登记与通用库聚合
//! 代码路径: kk_novel_ai/src-tauri/src/kb/mod.rs

use crate::error::{AppError, AppResult};
use crate::paths::{kb_registry_path, universal_kb_dir, character_roster_dir};
use crate::project::{self, LoreSource, NovelProject};
use crate::story;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const UNIVERSAL_MARKER: &str = "@universal";
/// 全局角色仓标记（写作作品默认挂接）
pub const CHARACTERS_MARKER: &str = "@characters";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbRegistryEntry {
    pub id: String,
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub last_synced_at: Option<String>,
    #[serde(default)]
    pub registered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KbRegistry {
    #[serde(default)]
    pub entries: Vec<KbRegistryEntry>,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

pub fn load_registry() -> AppResult<KbRegistry> {
    let path = kb_registry_path()?;
    if !path.exists() {
        return Ok(KbRegistry::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn save_registry(reg: &KbRegistry) -> AppResult<()> {
    let path = kb_registry_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(reg)?)?;
    Ok(())
}

pub fn register_knowledge_base(root: &Path, title: &str, id: &str) -> AppResult<KbRegistry> {
    let path_s = root.to_string_lossy().to_string();
    let mut reg = load_registry()?;
    if let Some(e) = reg.entries.iter_mut().find(|e| e.path == path_s) {
        e.title = title.to_string();
        e.id = id.to_string();
        e.kind = "knowledge_base".into();
    } else {
        reg.entries.insert(
            0,
            KbRegistryEntry {
                id: id.to_string(),
                path: path_s,
                title: title.to_string(),
                kind: "knowledge_base".into(),
                last_synced_at: None,
                registered_at: now(),
            },
        );
    }
    save_registry(&reg)?;
    Ok(reg)
}

pub fn mark_synced(root: &Path) -> AppResult<()> {
    let path_s = root.to_string_lossy().to_string();
    let mut reg = load_registry()?;
    if let Some(e) = reg.entries.iter_mut().find(|e| e.path == path_s) {
        e.last_synced_at = Some(now());
    }
    save_registry(&reg)?;
    Ok(())
}

fn stable_id(kb_id: &str, local_id: &str) -> String {
    let mut h = DefaultHasher::new();
    kb_id.hash(&mut h);
    local_id.hash(&mut h);
    format!("u{:016x}", h.finish())
}

/// 确保通用知识库目录存在
pub fn ensure_universal() -> AppResult<OpenedKb> {
    let root = universal_kb_dir()?;
    let pj = root.join("project.json");
    if pj.exists() {
        let opened = project::open_project(&root)?;
        return Ok(OpenedKb {
            root,
            project: opened.project,
        });
    }
    fs::create_dir_all(root.join("chapters"))?;
    fs::create_dir_all(root.join("lore").join("characters"))?;
    fs::create_dir_all(root.join("lore").join("world"))?;
    fs::create_dir_all(root.join("story"))?;
    let project = NovelProject {
        id: Uuid::new_v4().to_string(),
        title: "通用知识库".into(),
        kind: "universal".into(),
        genre: String::new(),
        style: String::new(),
        book_outline: String::new(),
        source_file: None,
        linked_kb_roots: vec![],
        volumes: vec![],
        chapters: vec![],
        outline_mindmap: None,
        created_at: now(),
        updated_at: now(),
    };
    project::save_project_meta(&root, &project)?;
    fs::write(
        root.join("memory.json"),
        serde_json::to_string_pretty(&project::MemoryStore::default())?,
    )?;
    fs::write(
        root.join("sources_index.json"),
        serde_json::to_string_pretty(&json!({}))?,
    )?;
    Ok(OpenedKb { root, project })
}

/// 确保全局角色仓存在（kind=character_roster）
pub fn ensure_character_roster() -> AppResult<OpenedKb> {
    let root = character_roster_dir()?;
    let pj = root.join("project.json");
    if pj.exists() {
        let opened = project::open_project(&root)?;
        return Ok(OpenedKb {
            root,
            project: opened.project,
        });
    }
    fs::create_dir_all(root.join("chapters"))?;
    fs::create_dir_all(root.join("lore").join("characters"))?;
    fs::create_dir_all(root.join("lore").join("world"))?;
    fs::create_dir_all(root.join("story"))?;
    let project = NovelProject {
        id: Uuid::new_v4().to_string(),
        title: "全局角色仓".into(),
        kind: "character_roster".into(),
        genre: String::new(),
        style: String::new(),
        book_outline: String::new(),
        source_file: None,
        linked_kb_roots: vec![],
        volumes: vec![],
        chapters: vec![],
        outline_mindmap: None,
        created_at: now(),
        updated_at: now(),
    };
    project::save_project_meta(&root, &project)?;
    fs::write(
        root.join("memory.json"),
        serde_json::to_string_pretty(&project::MemoryStore::default())?,
    )?;
    Ok(OpenedKb { root, project })
}

pub struct OpenedKb {
    pub root: PathBuf,
    pub project: NovelProject,
}

pub fn resolve_kb_root(marker_or_path: &str) -> AppResult<PathBuf> {
    if marker_or_path == UNIVERSAL_MARKER || marker_or_path == "universal" {
        let _ = ensure_universal()?;
        return Ok(universal_kb_dir()?);
    }
    if marker_or_path == CHARACTERS_MARKER || marker_or_path == "characters" {
        let opened = ensure_character_roster()?;
        return Ok(opened.root);
    }
    Ok(PathBuf::from(marker_or_path))
}

pub fn registry_list_json() -> AppResult<Value> {
    let uni = ensure_universal()?;
    let roster = ensure_character_roster()?;
    let reg = load_registry()?;
    Ok(json!({
        "ok": true,
        "universal": {
            "path": uni.root.to_string_lossy(),
            "id": uni.project.id,
            "title": uni.project.title,
            "kind": "universal",
            "marker": UNIVERSAL_MARKER
        },
        "characters": {
            "path": roster.root.to_string_lossy(),
            "id": roster.project.id,
            "title": roster.project.title,
            "kind": "character_roster",
            "marker": CHARACTERS_MARKER
        },
        "entries": reg.entries
    }))
}

/// 将小说知识库 lore/story 增量同步到通用库（跨书不合并，稳定 id）
pub fn sync_to_universal(novel_root: &Path) -> AppResult<Value> {
    let novel = project::open_project(novel_root)?;
    if novel.project.kind != "knowledge_base" && novel.project.kind != "universal" {
        // 允许 migrate 前强制 sync：若无 kind 也当作 KB 源
        if novel.project.kind != "novel" {
            /* ok */
        }
    }
    if novel.project.kind == "universal" {
        return Err(AppError::msg("不能把通用库同步到自身"));
    }

    let uni = ensure_universal()?;
    let kb_id = novel.project.id.clone();
    let kb_title = novel.project.title.clone();
    let lore_list = project::list_lore(novel_root)?;
    let mut lore_n = 0usize;

    let mut sources_index: Value = {
        let p = uni.root.join("sources_index.json");
        if p.exists() {
            serde_json::from_str(&fs::read_to_string(p)?)?
        } else {
            json!({})
        }
    };

    for entry in lore_list {
        let uid = stable_id(&kb_id, &entry.id);
        let mut remote = entry.clone();
        remote.id = uid.clone();
        // 文件名用「书名_原标题」避免撞名覆盖
        remote.title = format!("[{}] {}", kb_title, entry.title);
        remote.sources = vec![LoreSource {
            kb_id: kb_id.clone(),
            kb_title: kb_title.clone(),
            local_id: entry.id.clone(),
        }];
        // attrs 带来源书名便于过滤
        remote
            .attrs
            .insert("_source_kb".into(), kb_title.clone());
        remote
            .attrs
            .insert("_source_kb_id".into(), kb_id.clone());
        project::upsert_lore(&uni.root, remote)?;
        sources_index[&uid] = json!([{
            "novel_kb_id": kb_id,
            "novel_title": kb_title,
            "local_lore_id": entry.id,
        }]);
        lore_n += 1;
    }

    // story: 追加 facts/edges/events/arcs，id 加前缀
    let mut fact_n = 0usize;
    let mut edge_n = 0usize;
    let mut event_n = 0usize;

    {
        let src = story::load_canon(novel_root).unwrap_or_default();
        let mut dst = story::load_canon(&uni.root).unwrap_or_default();
        for mut f in src.facts {
            let old = f.id.clone();
            f.id = stable_id(&kb_id, &old);
            if !f.tags.iter().any(|t| t == &format!("src:{kb_title}")) {
                f.tags.push(format!("src:{kb_title}"));
            }
            if let Some(pos) = dst.facts.iter().position(|x| x.id == f.id) {
                dst.facts[pos] = f;
            } else {
                dst.facts.push(f);
            }
            fact_n += 1;
        }
        story::save_canon(&uni.root, &dst)?;
    }

    {
        let src = story::load_relations(novel_root).unwrap_or_default();
        let mut dst = story::load_relations(&uni.root).unwrap_or_default();
        for mut e in src.edges {
            let old = e.id.clone();
            e.id = stable_id(&kb_id, &old);
            e.from_id = stable_id(&kb_id, &e.from_id);
            e.to_id = stable_id(&kb_id, &e.to_id);
            let note = format!("src:{kb_title}");
            e.note = Some(match e.note {
                Some(n) if n.contains(&note) => n,
                Some(n) => format!("{n}; {note}"),
                None => note,
            });
            if let Some(pos) = dst.edges.iter().position(|x| x.id == e.id) {
                dst.edges[pos] = e;
            } else {
                dst.edges.push(e);
            }
            edge_n += 1;
        }
        story::save_relations(&uni.root, &dst)?;
    }

    {
        let src = story::load_timeline(novel_root).unwrap_or_default();
        let mut dst = story::load_timeline(&uni.root).unwrap_or_default();
        for mut ev in src.events {
            let old = ev.id.clone();
            ev.id = stable_id(&kb_id, &old);
            ev.title = format!("[{kb_title}] {}", ev.title);
            if let Some(pos) = dst.events.iter().position(|x| x.id == ev.id) {
                dst.events[pos] = ev;
            } else {
                dst.events.push(ev);
            }
            event_n += 1;
        }
        story::save_timeline(&uni.root, &dst)?;
    }

    {
        let src = story::load_plot(novel_root).unwrap_or_default();
        let mut dst = story::load_plot(&uni.root).unwrap_or_default();
        for mut a in src.arcs {
            let old = a.id.clone();
            a.id = stable_id(&kb_id, &old);
            a.title = format!("[{kb_title}] {}", a.title);
            if let Some(pos) = dst.arcs.iter().position(|x| x.id == a.id) {
                dst.arcs[pos] = a;
            } else {
                dst.arcs.push(a);
            }
        }
        for mut p in src.promises {
            let old = p.id.clone();
            p.id = stable_id(&kb_id, &old);
            p.text = format!("[{kb_title}] {}", p.text);
            if let Some(pos) = dst.promises.iter().position(|x| x.id == p.id) {
                dst.promises[pos] = p;
            } else {
                dst.promises.push(p);
            }
        }
        story::save_plot(&uni.root, &dst)?;
    }

    fs::write(
        uni.root.join("sources_index.json"),
        serde_json::to_string_pretty(&sources_index)?,
    )?;
    mark_synced(novel_root)?;

    Ok(json!({
        "ok": true,
        "universal_root": uni.root.to_string_lossy(),
        "lore_count": lore_n,
        "fact_count": fact_n,
        "edge_count": edge_n,
        "event_count": event_n
    }))
}

pub fn sync_all() -> AppResult<Value> {
    let reg = load_registry()?;
    let mut results = Vec::new();
    let mut errors = Vec::new();
    for e in &reg.entries {
        match sync_to_universal(Path::new(&e.path)) {
            Ok(v) => results.push(v),
            Err(err) => errors.push(json!({ "path": e.path, "error": err.to_string() })),
        }
    }
    Ok(json!({
        "ok": errors.is_empty(),
        "synced": results,
        "errors": errors
    }))
}

pub fn migrate_root(root: &Path, source_file: Option<&str>, sync: bool) -> AppResult<Value> {
    let opened = project::migrate_to_knowledge_base(root, source_file)?;
    register_knowledge_base(root, &opened.project.title, &opened.project.id)?;
    let mut s = crate::settings::load_settings()?;
    s.touch_recent_knowledge_base(root.to_string_lossy().as_ref(), &opened.project.title);
    let _ = crate::settings::save_settings(&s);
    let sync_report = if sync {
        Some(sync_to_universal(root)?)
    } else {
        None
    };
    Ok(json!({
        "ok": true,
        "root": root.to_string_lossy(),
        "project": opened.project,
        "sync": sync_report
    }))
}
