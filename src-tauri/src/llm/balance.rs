//! DeepSeek 等提供商账户余额
//! 代码路径: kk_novel_ai/src-tauri/src/llm/balance.rs

use crate::error::AppResult;
use crate::settings::AppSettings;
use chrono::Utc;
use serde_json::{json, Value};
use std::time::Duration;

/// 拉取当前配置对应提供商的余额；失败不抛崩，返回可读 reason。
pub async fn fetch_provider_balance(settings: &AppSettings) -> AppResult<Value> {
    let fetched_at = Utc::now().to_rfc3339();
    if !settings.is_deepseek() {
        return Ok(json!({
            "ok": false,
            "provider": "local_or_other",
            "reason": "本机或其它提供商无余额接口",
            "fetched_at": fetched_at,
        }));
    }
    let key = settings.api_key.trim();
    if key.is_empty() {
        return Ok(json!({
            "ok": false,
            "provider": "deepseek",
            "reason": "未配置 DeepSeek API Key",
            "fetched_at": fetched_at,
        }));
    }

    let base = AppSettings::normalize_base_url(&settings.base_url);
    let url = format!("{}/user/balance", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| crate::error::AppError::msg(format!("HTTP 客户端创建失败: {e}")))?;

    let resp = match client
        .get(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Ok(json!({
                "ok": false,
                "provider": "deepseek",
                "reason": format!("请求余额失败: {e}"),
                "fetched_at": fetched_at,
            }));
        }
    };

    let status = resp.status();
    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return Ok(json!({
                "ok": false,
                "provider": "deepseek",
                "reason": format!("余额响应解析失败 ({status}): {e}"),
                "fetched_at": fetched_at,
            }));
        }
    };

    if !status.is_success() {
        let msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .or_else(|| body.get("message").and_then(|m| m.as_str()))
            .unwrap_or("余额接口返回错误");
        return Ok(json!({
            "ok": false,
            "provider": "deepseek",
            "reason": format!("{status}: {msg}"),
            "fetched_at": fetched_at,
            "raw": body,
        }));
    }

    let is_available = body
        .get("is_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let infos = body
        .get("balance_infos")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let pick = infos
        .iter()
        .find(|x| {
            x.get("currency")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .eq_ignore_ascii_case("CNY")
        })
        .or_else(|| infos.first());

    let (currency, total, granted, topped_up) = match pick {
        Some(info) => (
            info.get("currency")
                .and_then(|c| c.as_str())
                .unwrap_or("CNY")
                .to_string(),
            info.get("total_balance")
                .and_then(|v| v.as_str())
                .unwrap_or("0")
                .to_string(),
            info.get("granted_balance")
                .and_then(|v| v.as_str())
                .unwrap_or("0")
                .to_string(),
            info.get("topped_up_balance")
                .and_then(|v| v.as_str())
                .unwrap_or("0")
                .to_string(),
        ),
        None => ("CNY".into(), "0".into(), "0".into(), "0".into()),
    };

    Ok(json!({
        "ok": true,
        "provider": "deepseek",
        "is_available": is_available,
        "currency": currency,
        "total": total,
        "granted": granted,
        "topped_up": topped_up,
        "fetched_at": fetched_at,
    }))
}
