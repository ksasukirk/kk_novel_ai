//! 迁完后将旧 JSON/md 移入 .legacy/
//! 代码路径: kk_novel_ai/src-tauri/src/storage/legacy_archive.rs

use crate::error::{AppError, AppResult};
use std::fs;
use std::path::{Path, PathBuf};

fn move_path(src: &Path, legacy_root: &Path, rel: &Path) -> AppResult<()> {
    if !src.exists() {
        return Ok(());
    }
    let dst = legacy_root.join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if dst.exists() {
        if dst.is_dir() {
            let _ = fs::remove_dir_all(&dst);
        } else {
            let _ = fs::remove_file(&dst);
        }
    }
    if fs::rename(src, &dst).is_err() {
        if src.is_dir() {
            copy_dir_recursive(src, &dst)
                .map_err(|e| AppError::msg(format!("归档失败 {}: {e}", src.display())))?;
            let _ = fs::remove_dir_all(src);
        } else {
            fs::copy(src, &dst)
                .map_err(|e| AppError::msg(format!("归档失败 {}: {e}", src.display())))?;
            let _ = fs::remove_file(src);
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for ent in fs::read_dir(src)? {
        let ent = ent?;
        let fp = ent.path();
        let tp = dst.join(ent.file_name());
        if fp.is_dir() {
            copy_dir_recursive(&fp, &tp)?;
        } else {
            fs::copy(&fp, &tp)?;
        }
    }
    Ok(())
}

/// 归档单作品目录中的旧文件（保留 work.sqlite / embeddings.sqlite）
pub fn archive_work_root(root: &Path) -> AppResult<()> {
    let legacy = root.join(".legacy");
    fs::create_dir_all(&legacy)?;
    for name in [
        "project.json",
        "memory.json",
        "stats.json",
        "chapters",
        "lore",
        "story",
        "chat",
        "gen_activity.jsonl",
        "sources_index.json",
    ] {
        let src = root.join(name);
        move_path(&src, &legacy, Path::new(name))?;
    }
    Ok(())
}

/// 归档全局情节库旧 JSON
pub fn archive_trope_library(root: &Path) -> AppResult<()> {
    let legacy = root.join(".legacy");
    fs::create_dir_all(&legacy)?;
    let lore = root.join("lore");
    if lore.exists() {
        move_path(&lore, &legacy, Path::new("lore"))?;
    }
    let marker = root.join(".trope_lib_maintained");
    move_path(&marker, &legacy, Path::new(".trope_lib_maintained"))?;
    Ok(())
}

/// 归档 app_data 侧旧文件
pub fn archive_app_data_files(app_data: &Path) -> AppResult<()> {
    let legacy = app_data.join(".legacy");
    fs::create_dir_all(&legacy)?;
    for name in [
        "settings.json",
        "kb_registry.json",
        "usage_ledger.json",
        "gen_log.jsonl",
        "work_catalog.json",
    ] {
        move_path(&app_data.join(name), &legacy, Path::new(name))?;
    }
    // character_roster / universal_kb 目录内旧文件（若仍有 project.json）
    for dir_name in ["character_roster", "universal_kb"] {
        let dir = app_data.join(dir_name);
        if dir.is_dir() {
            let _ = archive_work_root(&dir);
        }
    }
    let chat_free = app_data.join("chat").join("free.json");
    if chat_free.exists() {
        move_path(
            &chat_free,
            &legacy,
            &PathBuf::from("chat").join("free.json"),
        )?;
    }
    Ok(())
}
