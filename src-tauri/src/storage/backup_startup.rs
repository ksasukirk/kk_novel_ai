//! 启动全量备份（目录拷贝）
//! 代码路径: kk_novel_ai/src-tauri/src/storage/backup_startup.rs

use crate::error::{AppError, AppResult};
use crate::paths::{app_data_dir, novels_dir};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

const SKIP_NAMES: &[&str] = &[
    ".legacy",
    ".history",
    ".git",
    "node_modules",
    "migrations",
    "export_cache",
];

fn should_skip(name: &str) -> bool {
    SKIP_NAMES.iter().any(|s| *s == name)
        || name.ends_with(".sqlite-wal")
        || name.ends_with(".sqlite-shm")
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in entries.flatten() {
            let p = ent.path();
            let name = ent.file_name().to_string_lossy().to_string();
            if should_skip(&name) {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(meta) = ent.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

fn copy_tree(src: &Path, dst: &Path, on_file: &mut dyn FnMut()) -> AppResult<()> {
    fs::create_dir_all(dst)?;
    let mut stack = vec![(src.to_path_buf(), dst.to_path_buf())];
    while let Some((from, to)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&from) else {
            continue;
        };
        for ent in entries.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if should_skip(&name) {
                continue;
            }
            let fp = ent.path();
            let tp = to.join(&name);
            if fp.is_dir() {
                fs::create_dir_all(&tp)?;
                stack.push((fp, tp));
            } else {
                if let Some(parent) = tp.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&fp, &tp).map_err(|e| {
                    AppError::msg(format!(
                        "备份复制失败 {} -> {}: {e}",
                        fp.display(),
                        tp.display()
                    ))
                })?;
                on_file();
            }
        }
    }
    Ok(())
}

/// 备份 app_data 关键数据 + novels 到 migrations/backup_*，返回备份目录
pub fn backup_app_and_novels(progress: &mut dyn FnMut(u64, u64, &str)) -> AppResult<PathBuf> {
    let app_data = app_data_dir()?;
    let novels = novels_dir()?;
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let backup_root = app_data
        .join("migrations")
        .join(format!("backup_{stamp}"));
    fs::create_dir_all(&backup_root)?;

    let size_app = estimate_app_data_size(&app_data);
    let size_novels = if novels.exists() {
        dir_size(&novels)
    } else {
        0
    };
    let total_bytes = size_app.saturating_add(size_novels);
    // 写入预估信息供排查；具体空间不足由复制 IO 报错
    let _ = fs::write(
        backup_root.join("backup_meta.txt"),
        format!("estimated_source_bytes={total_bytes}\n"),
    );

    let mut copied = 0u64;
    let mut on_file = || {
        copied += 1;
        progress(copied, copied.max(1), "backup");
    };

    let app_dst = backup_root.join("app_data");
    fs::create_dir_all(&app_dst)?;
    for name in [
        "settings.json",
        "kb_registry.json",
        "usage_ledger.json",
        "gen_log.jsonl",
        "work_catalog.json",
        "character_roster",
        "universal_kb",
        "chat",
    ] {
        let src = app_data.join(name);
        if !src.exists() {
            continue;
        }
        let dst = app_dst.join(name);
        if src.is_dir() {
            copy_tree(&src, &dst, &mut on_file)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| AppError::msg(e.to_string()))?;
            on_file();
        }
    }

    if novels.exists() {
        let novels_dst = backup_root.join("novels");
        copy_tree(&novels, &novels_dst, &mut on_file)?;
    }

    progress(100, 100, "backup_done");
    Ok(backup_root)
}

fn estimate_app_data_size(app_data: &Path) -> u64 {
    let mut total = 0u64;
    for name in [
        "settings.json",
        "kb_registry.json",
        "usage_ledger.json",
        "gen_log.jsonl",
        "work_catalog.json",
        "character_roster",
        "universal_kb",
        "chat",
    ] {
        let p = app_data.join(name);
        if p.is_file() {
            if let Ok(m) = fs::metadata(&p) {
                total = total.saturating_add(m.len());
            }
        } else if p.is_dir() {
            total = total.saturating_add(dir_size(&p));
        }
    }
    total
}
