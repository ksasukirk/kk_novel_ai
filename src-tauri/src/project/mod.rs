//! 作品目录存储
//! 代码路径: kk_novel_ai/src-tauri/src/project/mod.rs
mod digest_sanitize;
pub mod backup;

pub use digest_sanitize::sanitize_block_digest;

use crate::error::{AppError, AppResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneBeat {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub conflict: String,
    #[serde(default)]
    pub emotion: String,
    #[serde(default)]
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterMeta {
    pub id: String,
    pub file: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub pov_lore_id: Option<String>,
    #[serde(default)]
    pub focus_arc_ids: Vec<String>,
    #[serde(default)]
    pub must_do: String,
    #[serde(default)]
    pub must_not: String,
    #[serde(default)]
    pub reader_knows: String,
    #[serde(default)]
    pub character_knows: String,
    #[serde(default)]
    pub beats: Vec<SceneBeat>,
}
fn default_status() -> String {
    "draft".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMeta {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub chapter_ids: Vec<String>,
    #[serde(default)]
    pub arc_goal: String,
    #[serde(default)]
    pub arc_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChapterMetaPatch {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub status: Option<String>,
    /// Some("") 或空串表示清空 POV
    pub pov_lore_id: Option<String>,
    pub focus_arc_ids: Option<Vec<String>>,
    pub must_do: Option<String>,
    pub must_not: Option<String>,
    pub reader_knows: Option<String>,
    pub character_knows: Option<String>,
    pub beats: Option<Vec<SceneBeat>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovelProject {
    pub id: String,
    pub title: String,
    /// novel | knowledge_base | universal
    #[serde(default = "default_kind_novel")]
    pub kind: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub style: String,
    /// 全书大纲（写作页按纲生成 / 大纲页共用）
    #[serde(default)]
    pub book_outline: String,
    /// 导入源 TXT 路径（知识库）
    #[serde(default)]
    pub source_file: Option<String>,
    /// 写作作品挂接的知识库根路径；可用 "@universal" 表示通用库
    #[serde(default)]
    pub linked_kb_roots: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<VolumeMeta>,
    #[serde(default)]
    pub chapters: Vec<ChapterMeta>,
    /// AI/本地整理的情节导图（与章纲分开；旧工程缺字段视为无）
    #[serde(default)]
    pub outline_mindmap: Option<OutlineMindMap>,
    pub created_at: String,
    pub updated_at: String,
}
fn default_kind_novel() -> String {
    "novel".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutlineMindNode {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub children: Vec<OutlineMindNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutlineMindMap {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub root: OutlineMindNode,
}

pub fn is_knowledge_kind(kind: &str) -> bool {
    kind == "knowledge_base" || kind == "universal" || kind == "character_roster"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryStore {
    #[serde(default)]
    pub rolling_summary: String,
    #[serde(default)]
    pub chapter_snapshots: Vec<ChapterSnapshot>,
    /// 块级蒸馏笔记（续写写入后自动追加）
    #[serde(default)]
    pub block_notes: Vec<BlockNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterSnapshot {
    pub chapter_id: String,
    pub summary: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockNote {
    pub id: String,
    pub chapter_id: String,
    pub block_key: String,
    pub summary: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoreLink {
    pub target_id: String,
    #[serde(default)]
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoreSource {
    pub kb_id: String,
    #[serde(default)]
    pub kb_title: String,
    #[serde(default)]
    pub local_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoreEntry {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub links: Vec<LoreLink>,
    #[serde(default)]
    pub attrs: std::collections::BTreeMap<String, String>,
    /// 通用库溯源（跨书不自动合并）
    #[serde(default)]
    pub sources: Vec<LoreSource>,
    /// 唯一性：同名角色跨本篇/全局只保留一条（本篇覆盖全局）；可改
    #[serde(default)]
    pub unique: bool,
    pub updated_at: String,
}

impl LoreEntry {
    /// 是否按唯一角色合并（字段或 attrs.unique=true/1）
    pub fn is_unique(&self) -> bool {
        if self.unique {
            return true;
        }
        self.attrs
            .get("unique")
            .map(|v| {
                let t = v.trim().to_lowercase();
                t == "true" || t == "1" || t == "yes"
            })
            .unwrap_or(false)
    }
    /// 唯一键：规范化标题（去掉挂接库前缀 `[书名]`）
    pub fn unique_key(&self) -> Option<String> {
        if !self.is_unique() {
            return None;
        }
        Some(normalize_lore_title(&self.title))
    }
}
fn normalize_lore_title(title: &str) -> String {
    let t = title.trim();
    let stripped = if t.starts_with('[') {
        if let Some(end) = t.find(']') {
            t[end + 1..].trim()
        } else {
            t
        }
    } else {
        t
    };
    stripped.to_lowercase()
}

/// 合并 lore：带 unique 的同名只留优先级更高的一条；`entries` 须已按优先级从高到低排列。
pub fn coalesce_unique_lore(entries: Vec<LoreEntry>) -> Vec<LoreEntry> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for e in entries {
        if let Some(key) = e.unique_key() {
            if !seen.insert(key) {
                continue;
            }
        }
        out.push(e);
    }
    out
}

#[derive(Debug, Clone)]
pub struct OpenedProject {
    pub root: PathBuf,
    pub project: NovelProject,
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
pub(crate) fn project_json(root: &Path) -> PathBuf {
    root.join("project.json")
}
fn memory_json(root: &Path) -> PathBuf {
    root.join("memory.json")
}
fn chapters_dir(root: &Path) -> PathBuf {
    root.join("chapters")
}

fn beat_progress_dir(root: &Path) -> PathBuf {
    chapters_dir(root).join(".progress")
}

fn beat_progress_path(root: &Path, chapter_id: &str) -> PathBuf {
    beat_progress_dir(root).join(format!("{chapter_id}.json"))
}

/// 章节节拍写作进度（sidecar，与 SceneBeat 定义分离）
/// 路径: chapters/.progress/{chapter_id}.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChapterBeatProgress {
    #[serde(default)]
    pub current_beat_id: String,
    /// beat_id -> pending | in_progress | completed | skipped
    #[serde(default)]
    pub beats: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub updated_at: String,
}

pub fn load_beat_progress(root: &Path, chapter_id: &str) -> AppResult<ChapterBeatProgress> {
    let path = beat_progress_path(root, chapter_id);
    if !path.exists() {
        return Ok(ChapterBeatProgress::default());
    }
    let text = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

pub fn save_beat_progress(
    root: &Path,
    chapter_id: &str,
    progress: &ChapterBeatProgress,
) -> AppResult<()> {
    fs::create_dir_all(beat_progress_dir(root))?;
    let mut p = progress.clone();
    p.updated_at = now();
    fs::write(
        beat_progress_path(root, chapter_id),
        serde_json::to_string_pretty(&p)?,
    )?;
    Ok(())
}

pub fn reset_beat_progress(root: &Path, chapter_id: &str) -> AppResult<()> {
    let path = beat_progress_path(root, chapter_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// 查找章节所属卷（首个包含该 chapter_id 的卷）
pub fn volume_for_chapter<'a>(
    project: &'a NovelProject,
    chapter_id: &str,
) -> Option<&'a VolumeMeta> {
    project
        .volumes
        .iter()
        .find(|v| v.chapter_ids.iter().any(|id| id == chapter_id))
}
fn lore_dir(root: &Path) -> PathBuf {
    root.join("lore")
}

pub fn create_project(root: &Path, title: &str) -> AppResult<OpenedProject> {
    if project_json(root).exists() {
        return Err(AppError::msg("目录已存在作品，请换目录或直接打开"));
    }
    fs::create_dir_all(chapters_dir(root))?;
    fs::create_dir_all(lore_dir(root).join("characters"))?;
    fs::create_dir_all(lore_dir(root).join("world"))?;
    let chapter_id = Uuid::new_v4().to_string();
    let file = "0001-第一章.md".to_string();
    let project = NovelProject {
        id: Uuid::new_v4().to_string(),
        title: title.to_string(),
        kind: "novel".into(),
        genre: String::new(),
        style: "文笔流畅，节奏紧凑，避免流水账。禁止「不是…是…」「并非…而是…」式否定对照修辞，感官与判断直接写。".into(),
        book_outline: String::new(),
        source_file: None,
        linked_kb_roots: vec![crate::kb::CHARACTERS_MARKER.to_string()],
        volumes: vec![VolumeMeta {
            id: Uuid::new_v4().to_string(),
            title: "第一卷".into(),
            chapter_ids: vec![chapter_id.clone()],
            arc_goal: String::new(),
            arc_summary: String::new(),
        }],
        chapters: vec![ChapterMeta {
            id: chapter_id,
            file: file.clone(),
            title: "第一章".into(),
            summary: String::new(),
            status: "draft".into(),
            pov_lore_id: None,
            focus_arc_ids: vec![],
            must_do: String::new(),
            must_not: String::new(),
            reader_knows: String::new(),
            character_knows: String::new(),
            beats: vec![],
        }],
        outline_mindmap: None,
        created_at: now(),
        updated_at: now(),
    };
    save_project_meta(root, &project)?;
    let _ = crate::kb::ensure_character_roster();
    fs::write(
        chapters_dir(root).join(&file),
        format!("# {}\n\n", project.chapters[0].title),
    )?;
    fs::write(
        memory_json(root),
        serde_json::to_string_pretty(&MemoryStore::default())?,
    )?;
    Ok(OpenedProject {
        root: root.to_path_buf(),
        project,
    })
}

/// 创建空知识库（无写作种子章；导入时会 replace_all_chapters）
pub fn create_knowledge_base(
    root: &Path,
    title: &str,
    source_file: Option<&str>,
) -> AppResult<OpenedProject> {
    if project_json(root).exists() {
        return Err(AppError::msg("目录已存在作品/知识库，请换目录或直接打开"));
    }
    fs::create_dir_all(chapters_dir(root))?;
    fs::create_dir_all(lore_dir(root).join("characters"))?;
    fs::create_dir_all(lore_dir(root).join("world"))?;
    fs::create_dir_all(root.join("story"))?;
    let project = NovelProject {
        id: Uuid::new_v4().to_string(),
        title: title.to_string(),
        kind: "knowledge_base".into(),
        genre: String::new(),
        style: String::new(),
        book_outline: String::new(),
        source_file: source_file.map(|s| s.to_string()),
        linked_kb_roots: vec![],
        volumes: vec![VolumeMeta {
            id: Uuid::new_v4().to_string(),
            title: "语料".into(),
            chapter_ids: vec![],
            arc_goal: String::new(),
            arc_summary: String::new(),
        }],
        chapters: vec![],
        outline_mindmap: None,
        created_at: now(),
        updated_at: now(),
    };
    save_project_meta(root, &project)?;
    fs::write(
        memory_json(root),
        serde_json::to_string_pretty(&MemoryStore::default())?,
    )?;
    Ok(OpenedProject {
        root: root.to_path_buf(),
        project,
    })
}

/// 将已有目录标记为知识库并写回 source
pub fn migrate_to_knowledge_base(root: &Path, source_file: Option<&str>) -> AppResult<OpenedProject> {
    let mut opened = open_project(root)?;
    opened.project.kind = "knowledge_base".into();
    if let Some(s) = source_file {
        opened.project.source_file = Some(s.to_string());
    }
    opened.project.updated_at = now();
    save_project_meta(root, &opened.project)?;
    Ok(opened)
}

pub fn open_project(root: &Path) -> AppResult<OpenedProject> {
    let path = project_json(root);
    if !path.exists() {
        return Err(AppError::msg(format!(
            "未找到 project.json: {}",
            path.display()
        )));
    }
    let text = fs::read_to_string(&path)?;
    let project: NovelProject = serde_json::from_str(&text)?;
    if !memory_json(root).exists() {
        fs::write(
            memory_json(root),
            serde_json::to_string_pretty(&MemoryStore::default())?,
        )?;
    }
    fs::create_dir_all(chapters_dir(root))?;
    fs::create_dir_all(lore_dir(root))?;
    Ok(OpenedProject {
        root: root.to_path_buf(),
        project,
    })
}

fn should_skip_scan_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".svn"
            | "node_modules"
            | "target"
            | "frontend-dist"
            | "dist"
            | "__pycache__"
            | ".cursor"
            | ".tmp-napcat-shots"
            | "ref"
            | "outputs"
    ) || name.starts_with('.')
}

/// 在父目录下发现作品根（含自身与子目录中的 `project.json`）。
/// `max_depth=0` 只检查自身；`1` 检查子目录；建议导入用 `2`。
pub fn discover_project_roots(parent: &Path, max_depth: usize) -> AppResult<Vec<PathBuf>> {
    if !parent.exists() {
        return Err(AppError::msg(format!(
            "目录不存在: {}",
            parent.display()
        )));
    }
    if !parent.is_dir() {
        return Err(AppError::msg(format!(
            "不是目录: {}",
            parent.display()
        )));
    }

    let mut found: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut queue: Vec<(PathBuf, usize)> = vec![(parent.to_path_buf(), 0)];

    while let Some((dir, depth)) = queue.pop() {
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if !seen.insert(canonical.clone()) {
            continue;
        }

        if project_json(&dir).is_file() {
            found.push(dir.clone());
            // 作品根内部不再往下扫，避免 chapters 等误命中
            continue;
        }

        if depth >= max_depth {
            continue;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for ent in entries.flatten() {
            let path = ent.path();
            if !path.is_dir() {
                continue;
            }
            let name = ent.file_name().to_string_lossy().to_string();
            if should_skip_scan_dir(&name) {
                continue;
            }
            queue.push((path, depth + 1));
        }
    }

    found.sort_by(|a, b| {
        a.to_string_lossy()
            .to_lowercase()
            .cmp(&b.to_string_lossy().to_lowercase())
    });
    Ok(found)
}

pub fn save_project_meta(root: &Path, project: &NovelProject) -> AppResult<()> {
    let mut p = project.clone();
    p.updated_at = now();
    fs::write(project_json(root), serde_json::to_string_pretty(&p)?)?;
    Ok(())
}

pub fn read_chapter(root: &Path, chapter_id: &str) -> AppResult<(ChapterMeta, String)> {
    let opened = open_project(root)?;
    let meta = opened
        .project
        .chapters
        .iter()
        .find(|c| c.id == chapter_id)
        .cloned()
        .ok_or_else(|| AppError::msg("章节不存在"))?;
    let path = chapters_dir(root).join(&meta.file);
    let content = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    Ok((meta, content))
}

pub fn write_chapter(root: &Path, chapter_id: &str, content: &str) -> AppResult<()> {
    let mut opened = open_project(root)?;
    let meta = opened
        .project
        .chapters
        .iter()
        .find(|c| c.id == chapter_id)
        .cloned()
        .ok_or_else(|| AppError::msg("章节不存在"))?;
    let path = chapters_dir(root).join(&meta.file);
    let old = if path.exists() {
        fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };
    let old_chars = count_non_ws(&old);
    let new_chars = count_non_ws(content);
    fs::write(&path, content)?;
    opened.project.updated_at = now();
    save_project_meta(root, &opened.project)?;
    if new_chars > old_chars {
        let _ = add_daily_chars(root, (new_chars - old_chars) as u64);
    }
    // 正文有变更则写入作品内履历（生成双写之外的保存/修改记录）
    let _ = crate::project_genlog::record_chapter_save(root, chapter_id, &old, content, "chapter_write");
    Ok(())
}

fn genblocks_dir(root: &Path) -> PathBuf {
    chapters_dir(root).join(".genblocks")
}

fn genblocks_path(root: &Path, chapter_id: &str) -> PathBuf {
    genblocks_dir(root).join(format!("{chapter_id}.json"))
}

/// 读章节 UI 分块 sidecar（可能不存在）
pub fn read_genblocks(root: &Path, chapter_id: &str) -> Option<serde_json::Value> {
    let path = genblocks_path(root, chapter_id);
    if !path.exists() {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// 写章节 UI 分块 sidecar
pub fn write_genblocks(root: &Path, chapter_id: &str, blocks: &serde_json::Value) -> AppResult<()> {
    let dir = genblocks_dir(root);
    fs::create_dir_all(&dir)?;
    let path = genblocks_path(root, chapter_id);
    fs::write(path, serde_json::to_string_pretty(blocks)?)?;
    Ok(())
}

pub fn delete_genblocks(root: &Path, chapter_id: &str) {
    let path = genblocks_path(root, chapter_id);
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}
fn count_non_ws(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectStats {
    #[serde(default)]
    pub daily: std::collections::BTreeMap<String, u64>,
    #[serde(default = "default_goal")]
    pub goal_chars: u64,
    #[serde(default)]
    pub title: Option<String>,
}
fn default_goal() -> u64 {
    2000
}
fn stats_json(root: &Path) -> PathBuf {
    root.join("stats.json")
}

pub fn load_stats(root: &Path) -> AppResult<ProjectStats> {
    let path = stats_json(root);
    if !path.exists() {
        return Ok(ProjectStats {
            goal_chars: default_goal(),
            ..Default::default()
        });
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn save_stats(root: &Path, stats: &ProjectStats) -> AppResult<()> {
    fs::write(stats_json(root), serde_json::to_string_pretty(stats)?)?;
    Ok(())
}

pub fn add_daily_chars(root: &Path, delta: u64) -> AppResult<ProjectStats> {
    if delta == 0 {
        return load_stats(root);
    }
    let mut stats = load_stats(root)?;
    let day = Utc::now().format("%Y-%m-%d").to_string();
    *stats.daily.entry(day).or_insert(0) += delta;
    save_stats(root, &stats)?;
    Ok(stats)
}

pub fn set_stats_goal(root: &Path, goal_chars: u64) -> AppResult<ProjectStats> {
    let mut stats = load_stats(root)?;
    stats.goal_chars = goal_chars;
    save_stats(root, &stats)?;
    Ok(stats)
}

/// AI 写入前可选落盘历史，保留最近 20 份
pub fn push_chapter_history(root: &Path, chapter_id: &str, content: &str) -> AppResult<()> {
    let dir = chapters_dir(root).join(".history");
    fs::create_dir_all(&dir)?;
    let ts = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let name = format!("{chapter_id}-{ts}.md");
    fs::write(dir.join(name), content)?;
    let mut files: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(chapter_id))
                .unwrap_or(false)
        })
        .collect();
    files.sort_by_key(|e| e.file_name());
    while files.len() > 20 {
        if let Some(oldest) = files.first() {
            let _ = fs::remove_file(oldest.path());
            files.remove(0);
        } else {
            break;
        }
    }
    Ok(())
}

pub fn create_chapter(root: &Path, title: &str, summary: &str) -> AppResult<ChapterMeta> {
    let mut opened = open_project(root)?;
    let idx = opened.project.chapters.len() + 1;
    let file = format!("{:04}-{}.md", idx, sanitize_filename(title));
    let meta = ChapterMeta {
        id: Uuid::new_v4().to_string(),
        file: file.clone(),
        title: title.to_string(),
        summary: summary.to_string(),
        status: "draft".into(),
        pov_lore_id: None,
        focus_arc_ids: vec![],
        must_do: String::new(),
        must_not: String::new(),
        reader_knows: String::new(),
        character_knows: String::new(),
        beats: vec![],
    };
    fs::write(
        chapters_dir(root).join(&file),
        format!("# {}\n\n", title),
    )?;
    if let Some(vol) = opened.project.volumes.first_mut() {
        vol.chapter_ids.push(meta.id.clone());
    }
    opened.project.chapters.push(meta.clone());
    save_project_meta(root, &opened.project)?;
    Ok(meta)
}

pub fn delete_chapter(root: &Path, chapter_id: &str) -> AppResult<()> {
    let mut opened = open_project(root)?;
    let Some(pos) = opened.project.chapters.iter().position(|c| c.id == chapter_id) else {
        return Err(AppError::msg("章节不存在"));
    };
    let meta = opened.project.chapters.remove(pos);
    let path = chapters_dir(root).join(&meta.file);
    if path.exists() {
        fs::remove_file(path)?;
    }
    delete_genblocks(root, chapter_id);
    let _ = clear_chapter_memory(root, chapter_id);
    for vol in &mut opened.project.volumes {
        vol.chapter_ids.retain(|id| id != chapter_id);
    }
    save_project_meta(root, &opened.project)?;
    Ok(())
}

pub fn update_chapter_meta(
    root: &Path,
    chapter_id: &str,
    patch: ChapterMetaPatch,
) -> AppResult<ChapterMeta> {
    let mut opened = open_project(root)?;
    let meta = opened
        .project
        .chapters
        .iter_mut()
        .find(|c| c.id == chapter_id)
        .ok_or_else(|| AppError::msg("章节不存在"))?;
    if let Some(t) = patch.title {
        meta.title = t;
    }
    if let Some(s) = patch.summary {
        meta.summary = s;
    }
    if let Some(st) = patch.status {
        meta.status = st;
    }
    if let Some(pov) = patch.pov_lore_id {
        meta.pov_lore_id = if pov.is_empty() { None } else { Some(pov) };
    }
    if let Some(ids) = patch.focus_arc_ids {
        meta.focus_arc_ids = ids;
    }
    if let Some(v) = patch.must_do {
        meta.must_do = v;
    }
    if let Some(v) = patch.must_not {
        meta.must_not = v;
    }
    if let Some(v) = patch.reader_knows {
        meta.reader_knows = v;
    }
    if let Some(v) = patch.character_knows {
        meta.character_knows = v;
    }
    if let Some(beats) = patch.beats {
        meta.beats = beats;
    }
    let out = meta.clone();
    save_project_meta(root, &opened.project)?;
    Ok(out)
}

pub fn load_memory(root: &Path) -> AppResult<MemoryStore> {
    let path = memory_json(root);
    if !path.exists() {
        return Ok(MemoryStore::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn save_memory(root: &Path, memory: &MemoryStore) -> AppResult<()> {
    fs::write(memory_json(root), serde_json::to_string_pretty(memory)?)?;
    Ok(())
}

/// 本章块笔记在滚动摘要中的软上限（字符）
const BLOCK_NOTES_BUDGET: usize = 1200;
/// 单条它章快照写入滚动摘要的上限，防止正文复读撑爆上下文
const CROSS_CHAPTER_SNIPPET_MAX: usize = 400;

fn clip_snapshot_for_rolling(s: &str) -> String {
    let t = s.trim();
    let n = t.chars().count();
    if n <= CROSS_CHAPTER_SNIPPET_MAX {
        return t.to_string();
    }
    format!("{}…", t.chars().take(280).collect::<String>())
}

/// 重建 rolling_summary：其它章用 chapter_snapshots（无快照则回退该章最新块笔记）；指定章用块笔记优先
pub fn rebuild_rolling_summary(memory: &mut MemoryStore, focus_chapter_id: Option<&str>) {
    let mut parts: Vec<String> = Vec::new();

    if let Some(fid) = focus_chapter_id {
        // 跨章：先收集其它章摘要（快照优先，否则用该章最新块笔记）
        let mut other_ids: Vec<String> = Vec::new();
        for s in &memory.chapter_snapshots {
            if s.chapter_id != fid && !other_ids.contains(&s.chapter_id) {
                other_ids.push(s.chapter_id.clone());
            }
        }
        for n in &memory.block_notes {
            if n.chapter_id != fid && !other_ids.contains(&n.chapter_id) {
                other_ids.push(n.chapter_id.clone());
            }
        }
        // 取最近出现的若干其它章（保持 snapshots 原顺序偏旧→新，再补笔记章）
        let mut cross: Vec<String> = Vec::new();
        for cid in other_ids.iter().rev().take(8) {
            if let Some(snap) = memory
                .chapter_snapshots
                .iter()
                .find(|s| &s.chapter_id == cid)
            {
                if !snap.summary.trim().is_empty() {
                    cross.push(format!("【它章】{}", clip_snapshot_for_rolling(&snap.summary)));
                    continue;
                }
            }
            if let Some(note) = memory
                .block_notes
                .iter()
                .filter(|n| &n.chapter_id == cid)
                .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
            {
                if !note.summary.trim().is_empty() {
                    cross.push(format!("【它章】{}", clip_snapshot_for_rolling(&note.summary)));
                }
            }
        }
        cross.reverse();
        // 只保留末尾 6 条，避免爆预算
        if cross.len() > 6 {
            cross = cross.split_off(cross.len() - 6);
        }
        parts.extend(cross);
    } else {
        let snaps: Vec<String> = memory
            .chapter_snapshots
            .iter()
            .rev()
            .take(6)
            .map(|s| clip_snapshot_for_rolling(&s.summary))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        parts.extend(snaps);
    }

    if let Some(cid) = focus_chapter_id {
        let mut notes: Vec<&BlockNote> = memory
            .block_notes
            .iter()
            .filter(|n| n.chapter_id == cid)
            .collect();
        notes.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
        let mut chapter_parts: Vec<String> = notes.iter().map(|n| n.summary.clone()).collect();
        // 超预算：合并最旧若干条为一条「前期摘要」占位（截断保留最新）
        let mut total: usize = chapter_parts.iter().map(|s| s.chars().count()).sum();
        while total > BLOCK_NOTES_BUDGET && chapter_parts.len() > 1 {
            let dropped = chapter_parts.remove(0);
            if !chapter_parts.is_empty() {
                let merged = format!(
                    "【本章前期】{}",
                    dropped.chars().take(180).collect::<String>()
                );
                // 若仍超：直接丢掉最旧，只保最新
                if chapter_parts.iter().map(|s| s.chars().count()).sum::<usize>()
                    + merged.chars().count()
                    > BLOCK_NOTES_BUDGET
                {
                    // drop merged too, keep trimming
                } else {
                    chapter_parts.insert(0, merged);
                }
            }
            total = chapter_parts.iter().map(|s| s.chars().count()).sum();
            if chapter_parts.len() == 1 && total > BLOCK_NOTES_BUDGET {
                let t = chapter_parts[0].chars().take(BLOCK_NOTES_BUDGET).collect();
                chapter_parts[0] = t;
                break;
            }
        }
        if !chapter_parts.is_empty() {
            parts.push(format!("【本章进行】\n{}", chapter_parts.join("\n---\n")));
        }
    } else {
        // 无 focus：把各章最新几条块笔记短摘附上
        let mut by_ch: std::collections::BTreeMap<String, Vec<&BlockNote>> =
            std::collections::BTreeMap::new();
        for n in &memory.block_notes {
            by_ch.entry(n.chapter_id.clone()).or_default().push(n);
        }
        for (_cid, mut ns) in by_ch {
            ns.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
            if let Some(last) = ns.last() {
                parts.push(last.summary.clone());
            }
        }
    }

    memory.rolling_summary = parts.join("\n\n");
    if memory.rolling_summary.chars().count() > 4000 {
        memory.rolling_summary = memory
            .rolling_summary
            .chars()
            .rev()
            .take(4000)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
    }
}

/// 用本章块笔记合成章快照（供跨章记忆）；无笔记则用传入 fallback
pub fn sync_chapter_snapshot_from_notes(
    root: &Path,
    chapter_id: &str,
    fallback: &str,
) -> AppResult<String> {
    let memory = load_memory(root)?;
    let mut notes: Vec<&BlockNote> = memory
        .block_notes
        .iter()
        .filter(|n| n.chapter_id == chapter_id)
        .collect();
    notes.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
    let summary = if let Some(last) = notes.last() {
        let head = if notes.len() > 1 {
            let first = notes[0].summary.chars().take(120).collect::<String>();
            format!("【章初】{}…；【章末】{}", first, last.summary.trim())
        } else {
            last.summary.trim().to_string()
        };
        let mut s = head;
        if s.chars().count() > 900 {
            s = s.chars().take(900).collect();
        }
        s
    } else {
        let fb = fallback.trim();
        if fb.is_empty() {
            return Ok(String::new());
        }
        fb.chars().take(900).collect()
    };
    if summary.is_empty() {
        return Ok(String::new());
    }
    upsert_chapter_snapshot(root, chapter_id, &summary)?;
    Ok(summary)
}

pub fn upsert_chapter_snapshot(root: &Path, chapter_id: &str, summary: &str) -> AppResult<()> {
    let mut memory = load_memory(root)?;
    if let Some(snap) = memory
        .chapter_snapshots
        .iter_mut()
        .find(|s| s.chapter_id == chapter_id)
    {
        snap.summary = summary.to_string();
        snap.updated_at = now();
    } else {
        memory.chapter_snapshots.push(ChapterSnapshot {
            chapter_id: chapter_id.to_string(),
            summary: summary.to_string(),
            updated_at: now(),
        });
    }
    rebuild_rolling_summary(&mut memory, Some(chapter_id));
    save_memory(root, &memory)?;
    Ok(())
}

/// 追加或按 block_key 覆盖块笔记，并重建滚动摘要。
/// summary 清洗后为空则删除该笔记（避免空摘要占坑）。
pub fn append_block_note(
    root: &Path,
    chapter_id: &str,
    block_key: &str,
    summary: &str,
) -> AppResult<BlockNote> {
    let summary = sanitize_block_digest(summary.trim());
    if summary.is_empty() {
        remove_block_note(root, chapter_id, block_key)?;
        return Ok(BlockNote {
            id: String::new(),
            chapter_id: chapter_id.to_string(),
            block_key: block_key.to_string(),
            summary: String::new(),
            updated_at: now(),
        });
    }
    let mut memory = load_memory(root)?;
    let note = if let Some(existing) = memory
        .block_notes
        .iter_mut()
        .find(|n| n.chapter_id == chapter_id && n.block_key == block_key)
    {
        existing.summary = summary;
        existing.updated_at = now();
        existing.clone()
    } else {
        let note = BlockNote {
            id: format!("note-{}", uuid_like()),
            chapter_id: chapter_id.to_string(),
            block_key: block_key.to_string(),
            summary,
            updated_at: now(),
        };
        memory.block_notes.push(note.clone());
        note
    };
    rebuild_rolling_summary(&mut memory, Some(chapter_id));
    save_memory(root, &memory)?;
    Ok(note)
}

/// 删除指定块笔记并重建滚动摘要
pub fn remove_block_note(root: &Path, chapter_id: &str, block_key: &str) -> AppResult<bool> {
    let mut memory = load_memory(root)?;
    let before = memory.block_notes.len();
    memory
        .block_notes
        .retain(|n| !(n.chapter_id == chapter_id && n.block_key == block_key));
    let removed = memory.block_notes.len() != before;
    if removed {
        rebuild_rolling_summary(&mut memory, Some(chapter_id));
        save_memory(root, &memory)?;
    }
    Ok(removed)
}

/// 清除某章全部块笔记与章摘要快照，并重建滚动摘要
pub fn clear_chapter_memory(root: &Path, chapter_id: &str) -> AppResult<()> {
    let mut memory = load_memory(root)?;
    memory.block_notes.retain(|n| n.chapter_id != chapter_id);
    memory
        .chapter_snapshots
        .retain(|s| s.chapter_id != chapter_id);
    rebuild_rolling_summary(&mut memory, None);
    save_memory(root, &memory)?;
    Ok(())
}

/// format2 分支文档：沿激活路径收集 (key, text, digest)
fn iter_active_path_variants(blocks: &serde_json::Value) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let Some(obj) = blocks.as_object() else {
        return out;
    };
    let format = obj
        .get("format")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if format < 2 {
        return out;
    }
    let Some(nodes) = obj.get("nodes").and_then(|v| v.as_array()) else {
        return out;
    };

    fn variant_fields(v: &serde_json::Value) -> Option<(String, String, String)> {
        let key = v.get("key").and_then(|x| x.as_str()).unwrap_or("").trim();
        if key.is_empty() {
            return None;
        }
        let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let digest = v
            .get("digest")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        Some((key.to_string(), text, digest))
    }

    fn walk(
        nodes: &[serde_json::Value],
        parent_id: Option<&str>,
        from_variant: Option<&str>,
        out: &mut Vec<(String, String, String)>,
    ) {
        for n in nodes {
            let pid = n
                .get("parentId")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty());
            let from = n
                .get("fromVariantId")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty());
            let match_parent = match (parent_id, pid) {
                (None, None) => true,
                (Some(a), Some(b)) => a == b,
                _ => false,
            };
            if !match_parent {
                continue;
            }
            let match_from = match (from_variant, from) {
                (None, None) => true,
                (Some(a), Some(b)) => a == b,
                (None, Some(_)) => false,
                (Some(_), None) => parent_id.is_none(),
            };
            if !match_from {
                continue;
            }
            let active = n
                .get("activeVariantId")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let Some(variants) = n.get("variants").and_then(|x| x.as_array()) else {
                continue;
            };
            let mut chosen = None;
            for v in variants {
                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
                if id == active {
                    chosen = Some(v);
                    break;
                }
            }
            if chosen.is_none() {
                chosen = variants.first();
            }
            if let Some(v) = chosen {
                if let Some(fields) = variant_fields(v) {
                    let vid = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
                    out.push(fields);
                    let nid = n.get("id").and_then(|x| x.as_str()).unwrap_or("");
                    if !nid.is_empty() && !vid.is_empty() {
                        walk(nodes, Some(nid), Some(vid), out);
                    }
                }
            }
        }
    }

    walk(nodes, None, None, &mut out);
    out
}

fn collect_gen_from_array_item(
    b: &serde_json::Value,
    keys: &mut std::collections::BTreeSet<String>,
    digests: &mut std::collections::BTreeMap<String, String>,
    collect_digest: bool,
) {
    let key = b
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if key.is_empty() {
        return;
    }
    let typ = b.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if typ != "gen" {
        return;
    }
    let text = b.get("text").and_then(|v| v.as_str()).unwrap_or("").trim();
    if text.is_empty() {
        return;
    }
    keys.insert(key.to_string());
    if collect_digest {
        let digest = sanitize_block_digest(
            b.get("digest")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim(),
        );
        if !digest.is_empty() {
            digests.insert(key.to_string(), digest);
        }
    }
}

/// 仍存活的生成块 key：type=gen 且正文非空（数组或 format2 激活路径）
pub fn alive_gen_block_keys(blocks: &serde_json::Value) -> std::collections::BTreeSet<String> {
    let mut keys = std::collections::BTreeSet::new();
    if let Some(arr) = blocks.as_array() {
        let mut digests = std::collections::BTreeMap::new();
        for b in arr {
            collect_gen_from_array_item(b, &mut keys, &mut digests, false);
        }
        return keys;
    }
    for (key, text, _) in iter_active_path_variants(blocks) {
        if !text.trim().is_empty() {
            keys.insert(key);
        }
    }
    keys
}

/// sidecar 里非空 digest：key -> digest（仅激活路径 / 数组 gen 块）
pub fn digests_from_blocks(
    blocks: &serde_json::Value,
) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    if let Some(arr) = blocks.as_array() {
        let mut keys = std::collections::BTreeSet::new();
        for b in arr {
            collect_gen_from_array_item(b, &mut keys, &mut map, true);
        }
        return map;
    }
    for (key, text, digest) in iter_active_path_variants(blocks) {
        if text.trim().is_empty() {
            continue;
        }
        let d = sanitize_block_digest(digest.trim());
        if !d.is_empty() {
            map.insert(key, d);
        }
    }
    map
}

/// 按当前分块同步本章记忆：删孤儿/空 digest 笔记；有 digest 的块写回
pub fn sync_chapter_block_notes_from_blocks(
    root: &Path,
    chapter_id: &str,
    blocks: &serde_json::Value,
) -> AppResult<()> {
    let alive = alive_gen_block_keys(blocks);
    let digests = digests_from_blocks(blocks);
    let mut memory = load_memory(root)?;
    let before_len = memory.block_notes.len();
    // 块已删 / 正文空：丢笔记；digest 显式为空：也丢（旧摘要作废）
    memory.block_notes.retain(|n| {
        if n.chapter_id != chapter_id {
            return true;
        }
        alive.contains(&n.block_key) && digests.contains_key(&n.block_key)
    });

    let mut changed = memory.block_notes.len() != before_len;
    for (key, digest) in &digests {
        if let Some(existing) = memory
            .block_notes
            .iter_mut()
            .find(|n| n.chapter_id == chapter_id && n.block_key == *key)
        {
            if existing.summary != *digest {
                existing.summary = digest.clone();
                existing.updated_at = now();
                changed = true;
            }
        } else {
            memory.block_notes.push(BlockNote {
                id: format!("note-{}", uuid_like()),
                chapter_id: chapter_id.to_string(),
                block_key: key.clone(),
                summary: digest.clone(),
                updated_at: now(),
            });
            changed = true;
        }
    }
    if changed {
        rebuild_rolling_summary(&mut memory, Some(chapter_id));
        save_memory(root, &memory)?;
    }
    Ok(())
}

/// 组装 prompt 用：只保留仍存活生成块的本章笔记（不落盘）
pub fn memory_filtered_for_chapter(root: &Path, chapter_id: &str) -> AppResult<MemoryStore> {
    let mut memory = load_memory(root)?;
    if let Some(blocks) = read_genblocks(root, chapter_id) {
        let alive = alive_gen_block_keys(&blocks);
        let digests = digests_from_blocks(&blocks);
        memory
            .block_notes
            .retain(|n| n.chapter_id != chapter_id || alive.contains(&n.block_key));
        // sidecar 有更新的 digest 时覆盖（防 memory 残留旧摘要）
        for n in memory.block_notes.iter_mut() {
            if n.chapter_id == chapter_id {
                if let Some(d) = digests.get(&n.block_key) {
                    n.summary = d.clone();
                }
            }
        }
    } else {
        // 无分块 sidecar：正文几乎为空则丢掉本章块笔记
        let content = read_chapter(root, chapter_id)
            .map(|(_, c)| c)
            .unwrap_or_default();
        if content.trim().is_empty()
            || content
                .lines()
                .all(|l| l.trim().is_empty() || l.trim().starts_with('#'))
        {
            memory.block_notes.retain(|n| n.chapter_id != chapter_id);
        }
    }
    rebuild_rolling_summary(&mut memory, Some(chapter_id));
    Ok(memory)
}

fn uuid_like() -> String {
    format!(
        "{}{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        (std::process::id() % 0xffff) as u32
    )
}

pub fn list_lore(root: &Path) -> AppResult<Vec<LoreEntry>> {
    let mut entries = Vec::new();
    let base = lore_dir(root);
    if !base.exists() {
        return Ok(entries);
    }
    collect_lore_json(&base, &mut entries)?;
    Ok(entries)
}
fn collect_lore_json(dir: &Path, out: &mut Vec<LoreEntry>) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_lore_json(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let text = fs::read_to_string(&path)?;
            if let Ok(item) = serde_json::from_str::<LoreEntry>(&text) {
                out.push(item);
            }
        }
    }
    Ok(())
}

pub fn upsert_lore(root: &Path, mut entry: LoreEntry) -> AppResult<LoreEntry> {
    // 同步 unique 字段与 attrs.unique
    if entry.is_unique() {
        entry.unique = true;
        entry
            .attrs
            .insert("unique".into(), "true".into());
    } else {
        entry.unique = false;
        entry.attrs.remove("unique");
    }
    entry.updated_at = now();
    if entry.id.is_empty() {
        entry.id = Uuid::new_v4().to_string();
    }
    let kind_dir = match entry.kind.as_str() {
        "character" => "characters",
        _ => "world",
    };
    let dir = lore_dir(root).join(kind_dir);
    fs::create_dir_all(&dir)?;
    let filename = format!("{}.json", sanitize_filename(&entry.title));
    fs::write(dir.join(filename), serde_json::to_string_pretty(&entry)?)?;
    // 清理同 id 旧文件（标题变更时）
    prune_duplicate_lore(root, &entry.id, &entry.title)?;
    Ok(entry)
}
fn prune_duplicate_lore(root: &Path, id: &str, keep_title: &str) -> AppResult<()> {
    let keep = format!("{}.json", sanitize_filename(keep_title));
    let mut all = Vec::new();
    collect_lore_paths(&lore_dir(root), &mut all)?;
    for path in all {
        let text = fs::read_to_string(&path)?;
        if let Ok(item) = serde_json::from_str::<LoreEntry>(&text) {
            if item.id == id {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name != keep {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
    Ok(())
}
fn collect_lore_paths(dir: &Path, out: &mut Vec<PathBuf>) -> AppResult<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_lore_paths(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}

pub fn delete_lore(root: &Path, lore_id: &str) -> AppResult<()> {
    let mut all = Vec::new();
    collect_lore_paths(&lore_dir(root), &mut all)?;
    for path in all {
        let text = fs::read_to_string(&path)?;
        if let Ok(item) = serde_json::from_str::<LoreEntry>(&text) {
            if item.id == lore_id {
                fs::remove_file(path)?;
                return Ok(());
            }
        }
    }
    Err(AppError::msg("设定条目不存在"))
}

pub fn project_to_value(root: &Path, project: &NovelProject) -> serde_json::Value {
    json!({
        "ok": true,
        "root": root.to_string_lossy(),
        "project": project
    })
}

pub fn sanitize_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();
    let s = s.trim();
    if s.is_empty() {
        "untitled".into()
    } else {
        s.chars().take(40).collect()
    }
}

/// 导入用：清空现有章节并按序写入（一次保存元数据，适合大批量 TXT）
pub fn replace_all_chapters(root: &Path, chapters: &[(String, String)]) -> AppResult<()> {
    if chapters.is_empty() {
        return Err(AppError::msg("导入章节为空"));
    }
    let mut opened = open_project(root)?;
    for ch in &opened.project.chapters {
        let path = chapters_dir(root).join(&ch.file);
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }
    let mut metas = Vec::with_capacity(chapters.len());
    let mut ids = Vec::with_capacity(chapters.len());
    for (i, (title, body)) in chapters.iter().enumerate() {
        let idx = i + 1;
        let id = Uuid::new_v4().to_string();
        let file = format!("{:04}-{}.md", idx, sanitize_filename(title));
        let trimmed = body.trim();
        let content = if trimmed.starts_with('#') {
            format!("{trimmed}\n")
        } else {
            format!("# {title}\n\n{trimmed}\n")
        };
        fs::write(chapters_dir(root).join(&file), content)?;
        ids.push(id.clone());
        metas.push(ChapterMeta {
            id,
            file,
            title: title.clone(),
            summary: String::new(),
            status: "draft".into(),
            pov_lore_id: None,
            focus_arc_ids: vec![],
            must_do: String::new(),
            must_not: String::new(),
            reader_knows: String::new(),
            character_knows: String::new(),
            beats: vec![],
        });
    }
    opened.project.chapters = metas;
    if let Some(vol) = opened.project.volumes.first_mut() {
        vol.chapter_ids = ids;
    } else {
        opened.project.volumes.push(VolumeMeta {
            id: Uuid::new_v4().to_string(),
            title: "第一卷".into(),
            chapter_ids: ids,
            arc_goal: String::new(),
            arc_summary: String::new(),
        });
    }
    save_project_meta(root, &opened.project)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(title: &str, unique: bool, id: &str) -> LoreEntry {
        LoreEntry {
            id: id.into(),
            kind: "character".into(),
            title: title.into(),
            content: "x".into(),
            keywords: vec![],
            links: vec![],
            attrs: Default::default(),
            sources: vec![],
            unique,
            updated_at: "t".into(),
        }
    }
    #[test]
    fn coalesce_keeps_higher_priority_unique() {
        let local = sample("Lele", true, "1");
        let mut global = sample("[roster] Lele", true, "2");
        global.content = "global".into();
        let out = coalesce_unique_lore(vec![local, global]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "1");
        assert_eq!(out[0].content, "x");
    }
    #[test]
    fn non_unique_not_deduped() {
        let a = sample("NPC", false, "1");
        let b = sample("NPC", false, "2");
        let out = coalesce_unique_lore(vec![a, b]);
        assert_eq!(out.len(), 2);
    }
}
