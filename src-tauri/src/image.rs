//! OpenAI 兼容文生图（与写作 API 分离）
//! 代码路径: kk_novel_ai/src-tauri/src/image.rs

use crate::error::{AppError, AppResult};
use crate::settings::AppSettings;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerateRequest {
    pub project_root: String,
    /// 相对作品根的目标路径，如 assets/illustrations/{chapter}/{id}.png
    pub rel: String,
    pub prompt: String,
    #[serde(default)]
    pub negative: String,
    #[serde(default)]
    pub size: Option<String>,
}

fn http_client(timeout_secs: u64) -> AppResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs.max(30).min(600)))
        .connect_timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()?)
}

fn normalize_base(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn safe_rel_path(rel: &str) -> AppResult<PathBuf> {
    let rel = rel.replace('\\', "/").trim().trim_start_matches('/').to_string();
    if rel.is_empty() || rel.contains("..") {
        return Err(AppError::msg("非法图像路径"));
    }
    if !rel.starts_with("assets/") {
        return Err(AppError::msg("图像只能写到作品 assets/ 下"));
    }
    Ok(PathBuf::from(rel))
}

pub fn abs_under_root(root: &Path, rel: &str) -> AppResult<PathBuf> {
    let rel_path = safe_rel_path(rel)?;
    let abs = root.join(&rel_path);
    let root_c = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf());
    if let Ok(canon) = abs.canonicalize() {
        if !canon.starts_with(&root_c) {
            return Err(AppError::msg("图像路径超出作品目录"));
        }
    }
    Ok(abs)
}

fn guess_mime(bytes: &[u8], rel: &str) -> &'static str {
    if bytes.len() >= 8 && bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return "image/png";
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        return "image/jpeg";
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    let lower = rel.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/png"
    }
}

fn decode_b64(s: &str) -> AppResult<Vec<u8>> {
    let t = s.trim();
    let payload = if let Some((_, rest)) = t.split_once("base64,") {
        rest
    } else {
        t
    };
    base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload.trim()))
        .map_err(|e| AppError::msg(format!("图像 base64 解码失败: {e}")))
}

async fn download_url(client: &reqwest::Client, url: &str) -> AppResult<Vec<u8>> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::msg(format!(
            "下载生成图失败 HTTP {}",
            resp.status()
        )));
    }
    Ok(resp.bytes().await?.to_vec())
}

async fn post_images(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    body: &Value,
) -> AppResult<(reqwest::StatusCode, Value)> {
    let mut builder = client.post(url).header("Content-Type", "application/json");
    if !key.is_empty() {
        builder = builder.bearer_auth(key);
    }
    let resp = builder.json(body).send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let parsed: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "raw": text }));
    Ok((status, parsed))
}

fn extract_error_message(body: &Value) -> String {
    if let Some(err) = body.get("error") {
        if let Some(m) = err.get("message").and_then(|v| v.as_str()) {
            return m.to_string();
        }
        if let Some(m) = err.as_str() {
            return m.to_string();
        }
    }
    if let Some(m) = body.get("message").and_then(|v| v.as_str()) {
        return m.to_string();
    }
    serde_json::to_string(body).unwrap_or_else(|_| "图像接口返回错误".into())
}

pub async fn generate(settings: &AppSettings, req: ImageGenerateRequest) -> AppResult<Value> {
    let prompt = req.prompt.trim();
    if prompt.is_empty() {
        return Err(AppError::msg("绘图提示词为空"));
    }
    let base = normalize_base(&settings.image_base_url);
    if base.is_empty() {
        return Err(AppError::msg("请先在设置里填写图像 Base URL"));
    }
    if settings.image_provider != "openai_compat" && !settings.image_provider.is_empty() {
        return Err(AppError::msg(format!(
            "暂不支持图像供应商 {}",
            settings.image_provider
        )));
    }
    let model = if settings.image_model.trim().is_empty() {
        "dall-e-3"
    } else {
        settings.image_model.trim()
    };
    let size = req
        .size
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            if settings.image_size.trim().is_empty() {
                "1024x1024"
            } else {
                settings.image_size.trim()
            }
        });
    let root = Path::new(&req.project_root);
    if !root.exists() {
        return Err(AppError::msg("作品目录不存在"));
    }
    let rel = req.rel.replace('\\', "/");
    let abs = abs_under_root(root, &rel)?;
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent)?;
    }

    let timeout = settings.llm_timeout_secs.min(180).max(60);
    let client = http_client(timeout)?;
    let url = format!("{base}/images/generations");
    let mut body = json!({
        "model": model,
        "prompt": prompt,
        "n": 1,
        "size": size,
        "response_format": "b64_json",
    });
    if !req.negative.trim().is_empty() {
        body["negative_prompt"] = json!(req.negative.trim());
    }
    let key = settings.image_api_key.trim();
    let (status, parsed) = post_images(&client, &url, key, &body).await?;
    let parsed = if status.is_success() {
        parsed
    } else {
        // 部分兼容网关不接受 response_format，去掉后再试，改走 url
        body.as_object_mut().map(|m| m.remove("response_format"));
        let (st2, parsed2) = post_images(&client, &url, key, &body).await?;
        if !st2.is_success() {
            return Err(AppError::msg(format!(
                "图像接口 HTTP {st2}: {}",
                extract_error_message(&parsed2)
            )));
        }
        parsed2
    };
    let first = parsed
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first());
    let bytes = if let Some(item) = first {
        if let Some(b64) = item.get("b64_json").and_then(|v| v.as_str()) {
            decode_b64(b64)?
        } else if let Some(u) = item.get("url").and_then(|v| v.as_str()) {
            download_url(&client, u).await?
        } else {
            return Err(AppError::msg("图像接口未返回 b64_json 或 url"));
        }
    } else {
        return Err(AppError::msg(extract_error_message(&parsed)));
    };
    if bytes.len() < 32 {
        return Err(AppError::msg("图像接口返回空数据"));
    }
    fs::write(&abs, &bytes)?;
    Ok(json!({
        "ok": true,
        "rel": rel,
        "bytes_len": bytes.len(),
        "model": model,
        "size": size,
    }))
}

pub fn read_data_url(project_root: &str, rel: &str) -> AppResult<Value> {
    let root = Path::new(project_root);
    let abs = abs_under_root(root, rel)?;
    if !abs.exists() {
        return Err(AppError::msg("插图文件不存在"));
    }
    let bytes = fs::read(&abs)?;
    let mime = guess_mime(&bytes, rel);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(json!({
        "ok": true,
        "rel": rel.replace('\\', "/"),
        "mime": mime,
        "data_url": format!("data:{mime};base64,{b64}"),
        "bytes_len": bytes.len(),
    }))
}
