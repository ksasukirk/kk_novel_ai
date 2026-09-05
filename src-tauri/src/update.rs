//! GitHub Release 检查更新与下载（便携 exe，不覆盖正在运行的文件）
//! 代码路径: kk_novel_ai/src-tauri/src/update.rs

use crate::error::{AppError, AppResult};
#[cfg(not(target_os = "android"))]
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::AppHandle;
#[cfg(not(target_os = "android"))]
use tauri::Emitter;
#[cfg(not(target_os = "android"))]
#[cfg(not(target_os = "android"))]
use tokio::io::{AsyncWriteExt, BufWriter};

pub const GITHUB_REPO: &str = "ksasukirk/kk_novel_ai";
const USER_AGENT: &str = "kk_novel_ai";

pub fn github_repo_url() -> String {
    format!("https://github.com/{GITHUB_REPO}")
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: Option<String>,
    body: Option<String>,
    html_url: Option<String>,
    assets: Option<Vec<GhAsset>>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    id: Option<u64>,
    url: Option<String>,
    name: Option<String>,
    browser_download_url: Option<String>,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let t = s.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = t.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn version_gt(newer: &str, older: &str) -> bool {
    match (parse_semver(newer), parse_semver(older)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn http_client() -> AppResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(10))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(900))
        .tcp_nodelay(true)
        .http2_keep_alive_interval(Duration::from_secs(15))
        .build()?)
}

fn is_github_api_asset(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    u.contains("api.github.com") && u.contains("/releases/assets/")
}

fn map_download_err(err: reqwest::Error, url: &str) -> AppError {
    if err.is_connect() || err.is_timeout() || err.is_request() {
        AppError::msg(format!(
            "连不上下载地址。检查更新走 api.github.com，安装包实际在 github.com / release-assets.githubusercontent.com，国内常被拦。请开代理后重试，或用浏览器打开 GitHub Release 页面下载。({url}: {err})"
        ))
    } else {
        AppError::msg(format!("下载失败 ({url}): {err}"))
    }
}

fn pick_exe_asset(assets: &[GhAsset]) -> Option<&GhAsset> {
    assets.iter().find(|a| {
        let n = a.name.as_deref().unwrap_or("");
        n.starts_with("kk_novel_ai_") && n.to_ascii_lowercase().ends_with(".exe")
    })
}

/// 查询 GitHub latest release
pub async fn check_update() -> AppResult<Value> {
    let current = current_version();
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let client = http_client()?;
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    let status = resp.status();
    if status.as_u16() == 404 {
        return Ok(json!({
            "current": current,
            "latest": serde_json::Value::Null,
            "has_update": false,
            "notes": "",
            "download_url": "",
            "api_download_url": "",
            "asset_name": "",
            "html_url": "",
        }));
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::msg(format!(
            "GitHub Release 查询失败 HTTP {status}: {body}"
        )));
    }
    let rel: GhRelease = resp.json().await?;
    let latest = rel
        .tag_name
        .unwrap_or_default()
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string();
    let assets = rel.assets.unwrap_or_default();
    let exe = pick_exe_asset(&assets);
    let has_update = !latest.is_empty() && version_gt(&latest, current);
    Ok(json!({
        "current": current,
        "latest": latest,
        "has_update": has_update,
        "notes": rel.body.unwrap_or_default(),
        "download_url": exe.and_then(|a| a.browser_download_url.clone()).unwrap_or_default(),
        "api_download_url": exe.and_then(|a| a.url.clone()).unwrap_or_else(|| {
            exe.and_then(|a| a.id).map(|id| {
                format!("https://api.github.com/repos/{GITHUB_REPO}/releases/assets/{id}")
            }).unwrap_or_default()
        }),
        "asset_name": exe.and_then(|a| a.name.clone()).unwrap_or_default(),
        "html_url": rel.html_url.unwrap_or_default(),
    }))
}

#[cfg(not(target_os = "android"))]
fn download_dest(asset_name: &str, latest: &str) -> PathBuf {
    let name = if asset_name.trim().is_empty() {
        format!("kk_novel_ai_{latest}.exe")
    } else {
        asset_name.trim().to_string()
    };
    std::env::temp_dir().join(name)
}

/// 下载 exe 到临时目录，进度事件 `update-download-progress`
pub async fn download_update(
    app: AppHandle,
    download_url: String,
    asset_name: String,
    latest: String,
    api_download_url: String,
) -> AppResult<Value> {
    #[cfg(target_os = "android")]
    {
        let _ = (app, download_url, asset_name, latest, api_download_url);
        return Err(AppError::msg("Android 请打开 GitHub Release 下载 APK"));
    }
    #[cfg(not(target_os = "android"))]
    {
        download_update_inner(app, download_url, asset_name, latest, api_download_url).await
    }
}

#[cfg(not(target_os = "android"))]
async fn download_update_inner(
    app: AppHandle,
    download_url: String,
    asset_name: String,
    latest: String,
    api_download_url: String,
) -> AppResult<Value> {
    let mut urls: Vec<String> = Vec::new();
    for raw in [api_download_url.trim(), download_url.trim()] {
        if !raw.is_empty() && !urls.iter().any(|u| u == raw) {
            urls.push(raw.to_string());
        }
    }
    if urls.is_empty() {
        return Err(AppError::msg("没有可下载的 Windows 安装包地址"));
    }
    let dest = download_dest(&asset_name, &latest);
    let client = http_client()?;
    let mut last_err: Option<AppError> = None;
    let mut resp = None;
    for url in &urls {
        let req = if is_github_api_asset(url) {
            client.get(url).header("Accept", "application/octet-stream")
        } else {
            client.get(url)
        };
        match req.send().await {
            Ok(r) if r.status().is_success() || r.status().is_redirection() => {
                if r.status().is_success() {
                    resp = Some(r);
                    break;
                }
                last_err = Some(AppError::msg(format!("下载失败 HTTP {} ({url})", r.status())));
            }
            Ok(r) => {
                last_err = Some(AppError::msg(format!("下载失败 HTTP {} ({url})", r.status())));
            }
            Err(e) => {
                last_err = Some(map_download_err(e, url));
            }
        }
    }
    let resp = match resp {
        Some(r) => r,
        None => {
            return Err(last_err.unwrap_or_else(|| AppError::msg("下载失败")));
        }
    };
    let total = resp.content_length().unwrap_or(0);
    let file = tokio::fs::File::create(&dest).await?;
    let mut file = BufWriter::with_capacity(256 * 1024, file);
    let mut received: u64 = 0;
    let mut last_emit = Instant::now();
    let mut last_emit_at: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        received += chunk.len() as u64;
        file.write_all(&chunk).await?;
        let due_time = last_emit.elapsed() >= Duration::from_millis(200);
        let due_bytes = received.saturating_sub(last_emit_at) >= 512 * 1024;
        if due_time || due_bytes {
            let _ = app.emit(
                "update-download-progress",
                json!({ "received": received, "total": total }),
            );
            last_emit = Instant::now();
            last_emit_at = received;
        }
    }
    file.flush().await?;
    let _ = app.emit(
        "update-download-progress",
        json!({ "received": received, "total": total }),
    );
    Ok(json!({
        "ok": true,
        "path": dest.to_string_lossy(),
        "received": received,
        "total": total,
    }))
}

/// 启动已下载的新版本并退出当前进程（不覆盖正在运行的 exe）
pub fn launch_and_quit(app: AppHandle, path: String) -> AppResult<Value> {
    let p = PathBuf::from(path.trim());
    if !p.exists() {
        return Err(AppError::msg("更新文件不存在，无法启动"));
    }
    #[cfg(target_os = "android")]
    {
        let _ = (app, p);
        return Err(AppError::msg("Android 请打开 GitHub Release 下载 APK"));
    }
    #[cfg(not(target_os = "android"))]
    {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &p.to_string_lossy()])
                .spawn()
                .map_err(|e| AppError::msg(format!("无法启动新版本: {e}")))?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new(&p)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| AppError::msg(format!("无法启动新版本: {e}")))?;
        }
        app.exit(0);
        Ok(json!({ "ok": true }))
    }
}

/// 在资源管理器中选中已下载文件
pub fn reveal_path(path: String) -> AppResult<Value> {
    let p = PathBuf::from(path.trim());
    if !p.exists() {
        return Err(AppError::msg("文件不存在"));
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", p.display()))
            .spawn()
            .map_err(|e| AppError::msg(format!("无法打开资源管理器: {e}")))?;
        return Ok(json!({ "ok": true }));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = p;
        Err(AppError::msg("当前平台请手动打开下载目录"))
    }
}

/// 用系统浏览器打开本仓库 GitHub 地址
pub fn open_external_url(url: String) -> AppResult<Value> {
    let url = url.trim().to_string();
    let home = github_repo_url();
    let ok = url == home || url.starts_with(&format!("{home}/"));
    if !ok {
        return Err(AppError::msg("不允许打开该地址"));
    }
    #[cfg(target_os = "android")]
    {
        return Err(AppError::msg("请在系统浏览器中打开该链接"));
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| AppError::msg(format!("无法打开浏览器: {e}")))?;
        return Ok(json!({ "ok": true }));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| AppError::msg(format!("无法打开浏览器: {e}")))?;
        return Ok(json!({ "ok": true }));
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| AppError::msg(format!("无法打开浏览器: {e}")))?;
        return Ok(json!({ "ok": true }));
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "android",
        all(unix, not(target_os = "macos"), not(target_os = "android"))
    )))]
    {
        Err(AppError::msg("当前平台请手动打开该地址"))
    }
}

#[cfg(test)]
mod tests {
    use super::version_gt;

    #[test]
    fn newer_semver_is_update() {
        assert!(version_gt("0.2.11", "0.2.10"));
        assert!(!version_gt("0.2.10", "0.2.10"));
        assert!(!version_gt("0.2.9", "0.2.10"));
        assert!(version_gt("v0.3.0", "0.2.99"));
    }
}
