//! 作品 ZIP 备份导入导出
//! 代码路径: kk_novel_ai/src-tauri/src/project/backup.rs

use crate::error::{AppError, AppResult};
use crate::paths::{allocate_novel_folder, export_cache_dir, sanitize_folder_name};
use crate::project::{open_project, project_json};
use serde_json::json;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_ZIP_BYTES: u64 = 200 * 1024 * 1024;
const SKIP_DIR_NAMES: &[&str] = &[".history", "node_modules", ".git"];

fn should_skip(path: &Path) -> bool {
    path.components().any(|c| match c {
        Component::Normal(name) => {
            let s = name.to_string_lossy();
            SKIP_DIR_NAMES.iter().any(|x| *x == s)
        }
        _ => false,
    })
}

fn collect_files(root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if should_skip(&dir) {
            continue;
        }
        for entry in fs::read_dir(&dir).map_err(|e| AppError::msg(e.to_string()))? {
            let entry = entry.map_err(|e| AppError::msg(e.to_string()))?;
            let path = entry.path();
            if path.is_dir() {
                if !should_skip(&path) {
                    stack.push(path);
                }
            } else if path.is_file() {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// 将作品目录打包为 ZIP，写入导出缓存，返回路径与文件名
pub fn export_project_zip(root: &Path) -> AppResult<serde_json::Value> {
    let opened = open_project(root)?;
    let title = sanitize_folder_name(&opened.project.title);
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let filename = format!("{title}_{stamp}.zip");
    let out_path = export_cache_dir()?.join(&filename);

    let file = File::create(&out_path).map_err(|e| AppError::msg(format!("创建备份失败: {e}")))?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let files = collect_files(root)?;
    for path in files {
        let rel = path
            .strip_prefix(root)
            .map_err(|_| AppError::msg("备份路径异常"))?;
        let name = rel.to_string_lossy().replace('\\', "/");
        if name.is_empty() || name.contains("..") {
            continue;
        }
        let mut buf = Vec::new();
        File::open(&path)
            .and_then(|mut f| f.read_to_end(&mut buf))
            .map_err(|e| AppError::msg(format!("读取 {} 失败: {e}", path.display())))?;
        zip.start_file(name, opts)
            .map_err(|e| AppError::msg(format!("写入 zip 失败: {e}")))?;
        zip.write_all(&buf)
            .map_err(|e| AppError::msg(format!("写入 zip 数据失败: {e}")))?;
    }
    zip.finish()
        .map_err(|e| AppError::msg(format!("完成备份失败: {e}")))?;

    let meta = fs::metadata(&out_path).map_err(|e| AppError::msg(e.to_string()))?;
    Ok(json!({
        "ok": true,
        "path": out_path.to_string_lossy(),
        "filename": filename,
        "bytes": meta.len(),
        "title": opened.project.title,
    }))
}

fn validate_zip_entry_name(name: &str) -> AppResult<PathBuf> {
    let name = name.replace('\\', "/");
    if name.is_empty() || name.starts_with('/') || name.contains('\0') {
        return Err(AppError::msg(format!("非法备份条目: {name}")));
    }
    let path = PathBuf::from(&name);
    for c in path.components() {
        match c {
            Component::Normal(_) => {}
            Component::CurDir => {}
            _ => return Err(AppError::msg(format!("非法备份路径: {name}"))),
        }
    }
    if name.contains("..") {
        return Err(AppError::msg(format!("非法备份路径: {name}")));
    }
    Ok(path)
}

fn decode_base64(data: &str) -> AppResult<Vec<u8>> {
    use std::io::Cursor;
    // 简易 base64：借助标准库不可用时用手动；这里用简单实现
    fn decode_inner(input: &str) -> Result<Vec<u8>, String> {
        const TABLE: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut buf: u32 = 0;
        let mut bits: i32 = 0;
        for b in input.bytes() {
            if b == b'=' || b.is_ascii_whitespace() {
                continue;
            }
            let val = TABLE
                .iter()
                .position(|&c| c == b)
                .ok_or_else(|| "无效的 base64".to_string())? as u32;
            buf = (buf << 6) | val;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((buf >> bits) & 0xff) as u8);
            }
        }
        let _ = Cursor::new(&out);
        Ok(out)
    }
    decode_inner(data).map_err(AppError::msg)
}

/// 从 ZIP 字节导入作品到 novels 目录
pub fn import_project_zip_bytes(bytes: &[u8], preferred_title: Option<&str>) -> AppResult<serde_json::Value> {
    if bytes.len() as u64 > MAX_ZIP_BYTES {
        return Err(AppError::msg("备份文件过大（上限 200MB）"));
    }
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| AppError::msg(format!("无法打开备份 ZIP: {e}")))?;

    let mut has_project = false;
    let mut staged: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AppError::msg(format!("读取备份条目失败: {e}")))?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        let rel = validate_zip_entry_name(&name)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| AppError::msg(format!("解压失败: {e}")))?;
        if rel.file_name().and_then(|s| s.to_str()) == Some("project.json")
            && rel.components().count() == 1
        {
            has_project = true;
        }
        // 兼容 zip 根下多一层文件夹
        if !has_project {
            if let Some(name) = rel.file_name().and_then(|s| s.to_str()) {
                if name == "project.json" && rel.components().count() == 2 {
                    has_project = true;
                }
            }
        }
        staged.push((rel, buf));
    }
    if !has_project {
        return Err(AppError::msg("备份中缺少 project.json，不是有效作品包"));
    }

    // 若所有文件都在同一个顶层目录下，剥掉该前缀
    let strip_prefix = {
        let mut tops = std::collections::BTreeSet::new();
        for (rel, _) in &staged {
            if let Some(Component::Normal(c)) = rel.components().next() {
                tops.insert(c.to_os_string());
            }
        }
        if tops.len() == 1
            && staged.iter().all(|(rel, _)| {
                rel.components().count() >= 2
                    || rel.file_name().and_then(|s| s.to_str()) != Some("project.json")
            })
            && staged.iter().any(|(rel, _)| {
                rel.components().count() == 2
                    && rel.file_name().and_then(|s| s.to_str()) == Some("project.json")
            })
        {
            tops.into_iter().next()
        } else {
            None
        }
    };

    let title = preferred_title
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            for (rel, buf) in &staged {
                let name = if let Some(ref pref) = strip_prefix {
                    rel.strip_prefix(pref).ok().map(|p| p.to_path_buf())
                } else {
                    Some(rel.clone())
                };
                if let Some(p) = name {
                    if p.as_os_str() == "project.json" || p == Path::new("project.json") {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(buf) {
                            return v
                                .get("title")
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string());
                        }
                    }
                }
            }
            None
        })
        .unwrap_or_else(|| "导入作品".into());

    let dest = allocate_novel_folder(&title)?;
    if dest.exists() && project_json(&dest).exists() {
        return Err(AppError::msg("目标目录已存在作品"));
    }
    fs::create_dir_all(&dest)?;

    for (rel, buf) in staged {
        let target_rel = if let Some(ref pref) = strip_prefix {
            match rel.strip_prefix(pref) {
                Ok(p) => p.to_path_buf(),
                Err(_) => continue,
            }
        } else {
            rel
        };
        if target_rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(&target_rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out, buf).map_err(|e| AppError::msg(format!("写入失败: {e}")))?;
    }

    if !project_json(&dest).is_file() {
        let _ = fs::remove_dir_all(&dest);
        return Err(AppError::msg("导入后未找到 project.json"));
    }

    let opened = open_project(&dest)?;
    Ok(json!({
        "ok": true,
        "root": opened.root.to_string_lossy(),
        "project": opened.project,
    }))
}

pub fn import_project_zip_base64(data_b64: &str, preferred_title: Option<&str>) -> AppResult<serde_json::Value> {
    let bytes = decode_base64(data_b64.trim())?;
    import_project_zip_bytes(&bytes, preferred_title)
}

/// 读取导出缓存文件为 base64，供前端分享/下载
pub fn read_export_file_base64(path: &str) -> AppResult<serde_json::Value> {
    let p = PathBuf::from(path);
    let cache = export_cache_dir()?;
    if !p.starts_with(&cache) {
        return Err(AppError::msg("只能读取导出缓存内的文件"));
    }
    let bytes = fs::read(&p).map_err(|e| AppError::msg(format!("读取失败: {e}")))?;
    Ok(json!({
        "ok": true,
        "filename": p.file_name().and_then(|s| s.to_str()).unwrap_or("export.bin"),
        "bytes": bytes.len(),
        "base64": encode_base64(&bytes),
    }))
}

fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let mut buf = [0u8; 3];
        for (i, b) in chunk.iter().enumerate() {
            buf[i] = *b;
        }
        let n = chunk.len();
        let b0 = buf[0] as u32;
        let b1 = buf[1] as u32;
        let b2 = buf[2] as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if n > 1 {
            out.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if n > 2 {
            out.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_path_traversal() {
        assert!(validate_zip_entry_name("../etc/passwd").is_err());
        assert!(validate_zip_entry_name("a/../../b").is_err());
        assert!(validate_zip_entry_name("chapters/0001.md").is_ok());
    }
}
