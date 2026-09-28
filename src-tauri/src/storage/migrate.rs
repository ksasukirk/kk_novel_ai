//! 启动分阶段迁移引擎
//! 代码路径: kk_novel_ai/src-tauri/src/storage/migrate.rs

use super::activity_store;
use super::app_store;
use super::backup_startup;
use super::characters_store;
use super::kb_store;
use super::legacy_archive;
use super::library_store;
use super::schema;
use super::work_store;
use super::{
    emit_progress, mark_storage_migrated_cached, set_migration_in_progress, work_sqlite_path,
};
use crate::error::{AppError, AppResult};
use crate::genlog::GenLogEntry;
use crate::paths::{
    app_data_dir, character_roster_dir, novels_dir, settings_path, trope_library_dir,
    universal_kb_dir, usage_ledger_path, gen_log_path, kb_registry_path,
};
use crate::project;
use crate::settings::AppSettings;
use crate::usage::UsageLedger;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub use schema::STORAGE_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProgress {
    pub percent: f64,
    pub stage: String,
    pub label: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    pub needed: bool,
    pub done: bool,
    pub version: i32,
    pub backup_path: Option<String>,
    pub last_error: Option<String>,
    pub empty_install: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Checkpoint {
    stage: String,
    completed_works: Vec<String>,
    backup_path: Option<String>,
}

fn stage_base(stage: &str) -> f64 {
    match stage {
        "backup" => 0.0,
        "app" => 15.0,
        "characters" => 23.0,
        "kb" => 30.0,
        "library" => 37.0,
        "works" => 50.0,
        "activity" => 95.0,
        "verify" => 97.0,
        "done" => 100.0,
        _ => 0.0,
    }
}

fn stage_span(stage: &str) -> f64 {
    match stage {
        "backup" => 15.0,
        "app" => 8.0,
        "characters" => 7.0,
        "kb" => 7.0,
        "library" => 13.0,
        "works" => 45.0,
        "activity" => 5.0,
        "verify" => 3.0,
        _ => 1.0,
    }
}

fn report(stage: &str, label: &str, current: u64, total: u64) {
    let span = stage_span(stage);
    let base = stage_base(stage);
    let frac = if total == 0 {
        1.0
    } else {
        (current as f64 / total as f64).clamp(0.0, 1.0)
    };
    let percent = (base + span * frac).clamp(0.0, 100.0);
    emit_progress(MigrationProgress {
        percent,
        stage: stage.into(),
        label: label.into(),
        current,
        total,
    });
}

fn has_legacy_data() -> bool {
    if settings_path().map(|p| p.exists()).unwrap_or(false) {
        return true;
    }
    if usage_ledger_path().map(|p| p.exists()).unwrap_or(false) {
        return true;
    }
    if gen_log_path().map(|p| p.exists()).unwrap_or(false) {
        return true;
    }
    if kb_registry_path().map(|p| p.exists()).unwrap_or(false) {
        return true;
    }
    if let Ok(novels) = novels_dir() {
        if novels.exists() {
            if let Ok(roots) = discover_legacy_roots(&novels) {
                if !roots.is_empty() {
                    return true;
                }
            }
            let lib = novels.join("_library").join("lore");
            if lib.exists() {
                return true;
            }
        }
    }
    if let Ok(r) = character_roster_dir() {
        if r.join("project.json").exists() || r.join("lore").exists() {
            return true;
        }
    }
    if let Ok(u) = universal_kb_dir() {
        if u.join("project.json").exists() || u.join("lore").exists() {
            return true;
        }
    }
    false
}

fn discover_legacy_roots(parent: &Path) -> AppResult<Vec<PathBuf>> {
    // 用文件发现，避免依赖已切流的 discover
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(parent) else {
        return Ok(found);
    };
    for ent in entries.flatten() {
        let p = ent.path();
        if !p.is_dir() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().to_string();
        if name == "_library" || name.starts_with('.') {
            continue;
        }
        if p.join("project.json").is_file() || p.join("work.sqlite").is_file() {
            found.push(p);
        }
    }
    Ok(found)
}

pub fn migration_status() -> AppResult<MigrationStatus> {
    let _ = app_store::open_app_db()?;
    let done = app_store::read_migration_done()?;
    let backup_path = app_store::read_backup_path()?;
    let empty = !has_legacy_data();
    Ok(MigrationStatus {
        needed: !done,
        done,
        version: STORAGE_VERSION,
        backup_path,
        last_error: None,
        empty_install: empty,
    })
}

pub fn migration_run() -> AppResult<Value> {
    set_migration_in_progress(true);
    let result = migration_run_inner();
    set_migration_in_progress(false);
    match &result {
        Ok(_) => {
            mark_storage_migrated_cached(true);
        }
        Err(e) => {
            let _ = app_store::write_migration_state(false, None, None, Some(&e.to_string()));
        }
    }
    result
}

fn migration_run_inner() -> AppResult<Value> {
    let _ = app_store::open_app_db()?;
    if app_store::read_migration_done()? {
        report("done", "already_done", 1, 1);
        return Ok(json!({ "ok": true, "skipped": true }));
    }

    let mut checkpoint: Checkpoint = app_store::read_checkpoint_json()?
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // 空安装：建空库并标记完成
    if !has_legacy_data() {
        report("app", "empty_install", 0, 1);
        ensure_empty_dbs()?;
        app_store::write_migration_state(true, None, Some(r#"{"stage":"done"}"#), None)?;
        mark_storage_migrated_cached(true);
        report("done", "empty_install_done", 1, 1);
        return Ok(json!({ "ok": true, "empty_install": true }));
    }

    // backup
    if checkpoint.backup_path.is_none() {
        report("backup", "backing_up", 0, 1);
        let mut last = 0u64;
        let backup = backup_startup::backup_app_and_novels(&mut |cur, _total, _label| {
            if cur != last {
                last = cur;
                report("backup", "backing_up", cur, cur.max(1));
            }
        })?;
        checkpoint.backup_path = Some(backup.to_string_lossy().to_string());
        checkpoint.stage = "backup".into();
        save_checkpoint(&checkpoint)?;
        app_store::write_migration_state(
            false,
            checkpoint.backup_path.as_deref(),
            None,
            None,
        )?;
    }
    report("backup", "backup_done", 1, 1);

    // app
    report("app", "migrating_app", 0, 1);
    migrate_app()?;
    checkpoint.stage = "app".into();
    save_checkpoint(&checkpoint)?;
    report("app", "app_done", 1, 1);

    // characters
    report("characters", "migrating_characters", 0, 1);
    migrate_characters()?;
    checkpoint.stage = "characters".into();
    save_checkpoint(&checkpoint)?;
    report("characters", "characters_done", 1, 1);

    // kb
    report("kb", "migrating_kb", 0, 1);
    migrate_universal_kb()?;
    checkpoint.stage = "kb".into();
    save_checkpoint(&checkpoint)?;
    report("kb", "kb_done", 1, 1);

    // library
    report("library", "migrating_library", 0, 1);
    migrate_library()?;
    checkpoint.stage = "library".into();
    save_checkpoint(&checkpoint)?;
    report("library", "library_done", 1, 1);

    // works
    let novels = novels_dir()?;
    let roots = discover_legacy_roots(&novels)?;
    let total = roots.len() as u64;
    for (i, root) in roots.iter().enumerate() {
        let key = root.to_string_lossy().to_string();
        if checkpoint.completed_works.iter().any(|x| x == &key) {
            report("works", &format!("skip {key}"), (i + 1) as u64, total.max(1));
            continue;
        }
        // 跳过已是 sqlite 且无 project.json 的
        if work_sqlite_path(root).is_file() && !root.join("project.json").is_file() {
            checkpoint.completed_works.push(key);
            continue;
        }
        report(
            "works",
            &format!("work {}", root.file_name().and_then(|s| s.to_str()).unwrap_or("?")),
            i as u64,
            total.max(1),
        );
        migrate_one_work(root)?;
        checkpoint.completed_works.push(key);
        checkpoint.stage = "works".into();
        save_checkpoint(&checkpoint)?;
        report("works", "work_done", (i + 1) as u64, total.max(1));
    }

    // activity
    report("activity", "migrating_activity", 0, 1);
    migrate_activity()?;
    checkpoint.stage = "activity".into();
    save_checkpoint(&checkpoint)?;
    report("activity", "activity_done", 1, 1);

    // verify
    report("verify", "verifying", 0, 1);
    verify_basic(&roots)?;
    report("verify", "verify_ok", 1, 1);

    // archive
    report("verify", "archiving", 0, 1);
    let _ = legacy_archive::archive_app_data_files(&app_data_dir()?);
    if let Ok(lib) = trope_library_dir() {
        let _ = legacy_archive::archive_trope_library(&lib);
    }
    for root in &roots {
        let _ = legacy_archive::archive_work_root(root);
    }
    if let Ok(r) = character_roster_dir() {
        let _ = legacy_archive::archive_work_root(&r);
    }
    if let Ok(u) = universal_kb_dir() {
        let _ = legacy_archive::archive_work_root(&u);
    }

    checkpoint.stage = "done".into();
    save_checkpoint(&checkpoint)?;
    app_store::write_migration_state(
        true,
        checkpoint.backup_path.as_deref(),
        Some(r#"{"stage":"done"}"#),
        None,
    )?;
    mark_storage_migrated_cached(true);
    report("done", "migration_complete", 1, 1);
    Ok(json!({
        "ok": true,
        "backup_path": checkpoint.backup_path,
        "works": roots.len(),
    }))
}

fn save_checkpoint(cp: &Checkpoint) -> AppResult<()> {
    let s = serde_json::to_string(cp)?;
    app_store::write_migration_state(false, cp.backup_path.as_deref(), Some(&s), None)
}

fn ensure_empty_dbs() -> AppResult<()> {
    let _ = app_store::open_app_db()?;
    let _ = characters_store::open_characters_db()?;
    let _ = kb_store::open_kb_db()?;
    let _ = activity_store::open_activity_db()?;
    let _ = library_store::open_library_db()?;
    Ok(())
}

fn migrate_app() -> AppResult<()> {
    if let Ok(path) = settings_path() {
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            if !crate::error::json_is_blank(&text) {
                if let Ok(s) = serde_json::from_str::<AppSettings>(&text) {
                    app_store::save_settings_json(&s)?;
                }
            }
        }
    }
    if let Ok(path) = usage_ledger_path() {
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            if !crate::error::json_is_blank(&text) {
                if let Ok(l) = serde_json::from_str::<UsageLedger>(&text) {
                    app_store::save_usage_ledger(&l)?;
                }
            }
        }
    }
    if let Ok(path) = kb_registry_path() {
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            if !crate::error::json_is_blank(&text) {
                if let Ok(reg) = serde_json::from_str::<crate::kb::KbRegistry>(&text) {
                    app_store::save_kb_registry(&reg)?;
                }
            }
        }
    }
    let catalog = app_data_dir()?.join("work_catalog.json");
    if catalog.exists() {
        let text = fs::read_to_string(&catalog)?;
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            app_store::save_work_catalog_blob("catalog", &v)?;
        }
    }
    Ok(())
}

fn migrate_characters() -> AppResult<()> {
    let root = character_roster_dir()?;
    if !root.join("project.json").exists() && !root.join("lore").exists() {
        let _ = characters_store::open_characters_db()?;
        return Ok(());
    }
    let opened: AppResult<project::OpenedProject> = project::open_project_fs(&root).or_else(|_| {
        let text = fs::read_to_string(root.join("project.json"))
            .map_err(|e| AppError::msg(e.to_string()))?;
        let project: project::NovelProject = serde_json::from_str(&text)
            .map_err(|e| AppError::msg(e.to_string()))?;
        Ok(project::OpenedProject {
            root: root.clone(),
            project,
        })
    });
    if let Ok(opened) = opened {
        characters_store::save_meta(&opened.project)?;
    }
    let lore = project::list_lore_fs(&root).unwrap_or_default();
    characters_store::replace_lore(&lore)?;
    Ok(())
}

fn migrate_universal_kb() -> AppResult<()> {
    let root = universal_kb_dir()?;
    if !root.join("project.json").exists() && !root.join("lore").exists() {
        let _ = kb_store::open_kb_db()?;
        return Ok(());
    }
    if let Ok(opened) = project::open_project_fs(&root) {
        kb_store::save_meta(&opened.project)?;
        let lore = project::list_lore_fs(&root).unwrap_or_default();
        kb_store::replace_lore(&lore)?;
        for name in ["plot", "timeline", "relations", "canon", "storyboard"] {
            let p = root.join("story").join(format!("{name}.json"));
            if p.exists() {
                if let Ok(text) = fs::read_to_string(&p) {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        let _ = kb_store::save_story_doc(name, &v);
                    }
                }
            }
        }
    }
    Ok(())
}

fn migrate_library() -> AppResult<()> {
    let root = trope_library_dir()?;
    let _ = library_store::open_library_db()?;
    for kind in ["trope", "kink", "style"] {
        let items = project::read_lore_kind_list_fs(&root, kind).unwrap_or_default();
        library_store::replace_kind(kind, &items)?;
    }
    Ok(())
}

fn migrate_one_work(root: &Path) -> AppResult<()> {
    let opened = project::open_project_fs(root)?;
    let mut bodies = Vec::new();
    for ch in &opened.project.chapters {
        let path = root.join("chapters").join(&ch.file);
        let content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };
        bodies.push((ch.id.clone(), content));
    }
    work_store::save_full_project(root, &opened.project, &bodies)?;

    if let Ok(lore) = project::list_lore_fs(root) {
        work_store::replace_lore(root, &lore)?;
    }
    let mem_path = root.join("memory.json");
    if mem_path.exists() {
        if let Ok(text) = fs::read_to_string(&mem_path) {
            if let Ok(mem) = serde_json::from_str(&text) {
                let _ = work_store::save_memory(root, &mem);
            }
        }
    }
    let stats_path = root.join("stats.json");
    if stats_path.exists() {
        if let Ok(text) = fs::read_to_string(&stats_path) {
            if let Ok(v) = serde_json::from_str(&text) {
                let _ = work_store::save_stats_json(root, &v);
            }
        }
    }
    for name in ["plot", "timeline", "relations", "canon", "storyboard"] {
        let p = root.join("story").join(format!("{name}.json"));
        if p.exists() {
            if let Ok(text) = fs::read_to_string(&p) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    let _ = work_store::save_story_doc(root, name, &v);
                }
            }
        }
    }
    let chat = root.join("chat").join("novel.json");
    if chat.exists() {
        if let Ok(text) = fs::read_to_string(&chat) {
            if let Ok(s) = serde_json::from_str(&text) {
                let _ = work_store::save_novel_chat(root, &s);
            }
        }
    }
    // genblocks
    let gb_dir = root.join("chapters").join(".genblocks");
    if gb_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&gb_dir) {
            for ent in entries.flatten() {
                let p = ent.path();
                if p.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let id = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                if let Ok(text) = fs::read_to_string(&p) {
                    if let Ok(v) = serde_json::from_str(&text) {
                        let _ = work_store::save_genblocks(root, &id, &v);
                    }
                }
            }
        }
    }
    // beat progress
    let prog_dir = root.join("chapters").join(".progress");
    if prog_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&prog_dir) {
            for ent in entries.flatten() {
                let p = ent.path();
                if p.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let id = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                if let Ok(text) = fs::read_to_string(&p) {
                    if let Ok(v) = serde_json::from_str(&text) {
                        let _ = work_store::save_beat_progress(root, &id, &v);
                    }
                }
            }
        }
    }
    let act = root.join("gen_activity.jsonl");
    if act.exists() {
        if let Ok(file) = fs::File::open(&act) {
            for line in BufReader::new(file).lines().flatten() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    let _ = work_store::append_gen_activity(root, &v);
                }
            }
        }
    }
    Ok(())
}

fn migrate_activity() -> AppResult<()> {
    let _ = activity_store::open_activity_db()?;
    if let Ok(path) = gen_log_path() {
        if path.exists() {
            let file = fs::File::open(&path)?;
            let mut entries = Vec::new();
            for line in BufReader::new(file).lines().flatten() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(e) = serde_json::from_str::<GenLogEntry>(&line) {
                    entries.push(e);
                }
            }
            activity_store::replace_gen_log(&entries)?;
        }
    }
    let free = app_data_dir()?.join("chat").join("free.json");
    if free.exists() {
        if let Ok(text) = fs::read_to_string(&free) {
            if let Ok(s) = serde_json::from_str(&text) {
                activity_store::save_chat_free(&s)?;
            }
        }
    }
    Ok(())
}

fn verify_basic(roots: &[PathBuf]) -> AppResult<()> {
    for root in roots {
        if !work_sqlite_path(root).is_file() {
            return Err(AppError::msg(format!(
                "校验失败：缺少 work.sqlite {}",
                root.display()
            )));
        }
        let _ = work_store::load_project(root)?;
    }
    Ok(())
}
