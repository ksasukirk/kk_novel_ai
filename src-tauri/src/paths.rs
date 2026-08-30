//! 应用数据路径
//! 代码路径: kk_novel_ai/src-tauri/src/paths.rs

use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

pub fn app_data_dir() -> AppResult<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| AppError::msg("无法解析系统数据目录"))?;
    let dir = base.join("kk_novel_ai");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn settings_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("settings.json"))
}

pub fn kb_registry_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("kb_registry.json"))
}

pub fn universal_kb_dir() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("universal_kb"))
}

/// 全局角色仓（跨作品共享；写作默认挂接 @characters）
pub fn character_roster_dir() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("character_roster"))
}

pub fn gen_log_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("gen_log.jsonl"))
}

pub fn usage_ledger_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("usage_ledger.json"))
}

/// GUI IPC 发现文件（host/port/token）
pub fn ipc_endpoint_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("ipc.json"))
}

/// 临时导出目录（手机分享前落盘）
pub fn export_cache_dir() -> AppResult<PathBuf> {
    let dir = app_data_dir()?.join("export_cache");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 是否为移动端（Android / iOS）
pub fn is_mobile() -> bool {
    cfg!(target_os = "android") || cfg!(target_os = "ios")
}

/// 软件运行根目录：可执行文件所在目录；失败则退回当前工作目录
pub fn runtime_root_dir() -> AppResult<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return Ok(parent.to_path_buf());
        }
    }
    std::env::current_dir().map_err(|e| AppError::msg(format!("无法解析运行目录: {e}")))
}

/// 默认小说库
/// - 桌面：`{运行根}/novels`
/// - 移动端：`{应用数据}/novels`（APK 目录不可写）
pub fn novels_dir() -> AppResult<PathBuf> {
    let dir = if is_mobile() {
        app_data_dir()?.join("novels")
    } else {
        runtime_root_dir()?.join("novels")
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 书名 → 安全文件夹名（去非法字符；空则「未命名小说」）
pub fn sanitize_folder_name(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    s = s.trim().trim_matches('.').trim().to_string();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    if s.is_empty() {
        "未命名小说".into()
    } else {
        // Windows 保留名兜底
        let upper = s.to_ascii_uppercase();
        match upper.as_str() {
            "CON" | "PRN" | "AUX" | "NUL"
            | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9" => {
                format!("_{s}")
            }
            _ => s,
        }
    }
}

/// 在 novels 下为书名分配不冲突的文件夹路径（重名则书名2、书名3…）
pub fn allocate_novel_folder(title: &str) -> AppResult<PathBuf> {
    let base_name = sanitize_folder_name(title);
    let root = novels_dir()?;
    let candidate = root.join(&base_name);
    if !candidate.exists() {
        return Ok(candidate);
    }
    for n in 2u32..=9999 {
        let name = format!("{base_name}{n}");
        let path = root.join(&name);
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(AppError::msg(format!(
        "无法在 {} 下为「{base_name}」分配文件夹（重名过多）",
        root.display()
    )))
}

/// 是否已是作品根（含 project.json）
#[allow(dead_code)]
pub fn is_project_root(path: &Path) -> bool {
    path.join("project.json").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_illegal() {
        assert_eq!(sanitize_folder_name("a/b:c"), "a_b_c");
    }
}
