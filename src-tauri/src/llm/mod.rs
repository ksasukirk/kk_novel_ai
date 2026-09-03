//! LLM Provider（LM Studio OpenAI 兼容）
//! 代码路径: kk_novel_ai/src-tauri/src/llm/mod.rs

pub mod balance;
pub mod stream;

use crate::error::{AppError, AppResult};
use crate::settings::AppSettings;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    /// DeepSeek：缓存命中输入 tokens（usage.prompt_cache_hit_tokens）
    #[serde(default)]
    pub prompt_cache_hit_tokens: u32,
    /// DeepSeek：缓存未命中输入 tokens（usage.prompt_cache_miss_tokens）
    #[serde(default)]
    pub prompt_cache_miss_tokens: u32,
    /// "api" | "estimate"
    #[serde(default = "default_usage_source")]
    pub source: String,
}

fn default_usage_source() -> String {
    "estimate".into()
}

impl TokenUsage {
    pub fn from_api_json(v: &Value) -> Option<Self> {
        let prompt = v
            .get("prompt_tokens")
            .and_then(|x| x.as_u64())
            .or_else(|| v.get("input_tokens").and_then(|x| x.as_u64()))?;
        let completion = v
            .get("completion_tokens")
            .and_then(|x| x.as_u64())
            .or_else(|| v.get("output_tokens").and_then(|x| x.as_u64()))
            .unwrap_or(0);
        let total = v
            .get("total_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(prompt + completion);
        let cache_hit = v
            .get("prompt_cache_hit_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let cache_miss = v
            .get("prompt_cache_miss_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        Some(Self {
            prompt_tokens: prompt as u32,
            completion_tokens: completion as u32,
            total_tokens: total as u32,
            prompt_cache_hit_tokens: cache_hit,
            prompt_cache_miss_tokens: cache_miss,
            source: "api".into(),
        })
    }

    pub fn estimate_text(text: &str) -> u32 {
        // 中文粗估：约 1.5 字/token
        ((text.chars().count() as f32) / 1.5).ceil() as u32
    }

    pub fn estimate_from_messages(messages: &[ChatMessage], completion: &str) -> Self {
        let mut prompt = 0u32;
        for m in messages {
            prompt = prompt.saturating_add(Self::estimate_text(&m.content));
            prompt = prompt.saturating_add(4);
        }
        let completion_tokens = Self::estimate_text(completion);
        Self {
            prompt_tokens: prompt,
            completion_tokens,
            total_tokens: prompt.saturating_add(completion_tokens),
            prompt_cache_hit_tokens: 0,
            prompt_cache_miss_tokens: 0,
            source: "estimate".into(),
        }
    }

    pub fn or_estimate(self_opt: Option<Self>, messages: &[ChatMessage], completion: &str) -> Self {
        self_opt.unwrap_or_else(|| Self::estimate_from_messages(messages, completion))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResult {
    pub text: String,
    #[serde(default)]
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOptions {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub frequency_penalty: Option<f32>,
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    #[serde(default)]
    pub stream: bool,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            model: None,
            temperature: None,
            max_tokens: None,
            frequency_penalty: None,
            presence_penalty: None,
            stream: false,
        }
    }
}

#[derive(Clone)]
pub struct LmStudioClient {
    http: reqwest::Client,
}

impl Default for LmStudioClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LmStudioClient {
    pub fn new() -> Self {
        Self::with_timeout_secs(600)
    }

    pub fn with_timeout_secs(secs: u64) -> Self {
        let secs = secs.max(60);
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(secs))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn from_settings(settings: &AppSettings) -> Self {
        Self::with_timeout_secs(settings.llm_timeout_secs.max(60))
    }

    fn chat_url(settings: &AppSettings) -> String {
        format!("{}/chat/completions", settings.base_url.trim_end_matches('/'))
    }

    fn models_url(settings: &AppSettings) -> String {
        format!("{}/models", settings.base_url.trim_end_matches('/'))
    }

    fn chat_body(
        settings: &AppSettings,
        model: &str,
        messages: &[ChatMessage],
        options: &ChatOptions,
        stream: bool,
    ) -> Value {
        let mut body = json!({
            "model": model,
            "messages": messages,
            "temperature": options.temperature.unwrap_or(settings.temperature),
            "max_tokens": options.max_tokens.unwrap_or(settings.max_tokens),
            "frequency_penalty": options
                .frequency_penalty
                .unwrap_or(settings.frequency_penalty),
            "presence_penalty": options
                .presence_penalty
                .unwrap_or(settings.presence_penalty),
            "stream": stream
        });
        if stream {
            body["stream_options"] = json!({ "include_usage": true });
        }
        if settings.resolve_disable_thinking() {
            // SenseNova: 不关思考链时 max_tokens 常被 reasoning 吃光，content 为空
            body["thinking"] = json!({ "type": "disabled" });
        }
        body
    }

    fn resolve_model(settings: &AppSettings, options: &ChatOptions) -> AppResult<String> {
        options
            .model
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                if settings.model.is_empty() {
                    None
                } else {
                    Some(settings.model.clone())
                }
            })
            .ok_or_else(|| AppError::msg("未指定模型，请在设置中选择或传入 model"))
    }

    fn extract_message_content(data: &Value) -> Option<String> {
        let content = data
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if content.is_some() {
            return content;
        }
        None
    }

    fn format_api_error(data: &Value) -> String {
        if let Some(s) = data.pointer("/error/message").and_then(|v| v.as_str()) {
            return s.to_string();
        }
        if let Some(s) = data.get("error").and_then(|v| v.as_str()) {
            return s.to_string();
        }
        data.to_string()
    }

    /// 列出本地可见模型 id（已下载/已注册，不等于已加载进内存）。
    pub async fn model_ids(&self, settings: &AppSettings) -> AppResult<Vec<String>> {
        let body = self.list_models(settings).await?;
        let mut ids = Vec::new();
        if let Some(arr) = body
            .pointer("/data")
            .and_then(|v| v.as_array())
            .or_else(|| body.get("data").and_then(|v| v.as_array()))
        {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    ids.push(id.to_string());
                }
            }
        }
        Ok(ids)
    }

    /// 探测模型：先校验 id 在列表中，再用短超时试一次极短生成。
    pub async fn probe_model(&self, settings: &AppSettings, model: &str) -> AppResult<()> {
        let ids = self.model_ids(settings).await?;
        if !ids.iter().any(|id| id == model) {
            return Err(AppError::msg(format!(
                "模型不在 LM Studio /v1/models 列表中: `{model}`（请核对 id，或先下载该模型）"
            )));
        }
        // 短超时：未加载进内存的大模会卡住，尽快失败以便回退
        let probe_client = Self::with_timeout_secs(25);
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "只回复一个字：好".into(),
        }];
        let options = ChatOptions {
            model: Some(model.to_string()),
            temperature: Some(0.0),
            max_tokens: Some(4),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            stream: false,
        };
        let _ = probe_client.chat(settings, &messages, &options).await?;
        Ok(())
    }

    pub async fn health(&self, settings: &AppSettings) -> AppResult<Value> {
        let url = Self::models_url(settings);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", settings.api_key))
            .send()
            .await
            .map_err(|e| AppError::msg(format!("无法连接 LM Studio: {e}")))?;
        if !resp.status().is_success() {
            return Err(AppError::msg(format!(
                "LM Studio 健康检查失败: HTTP {}",
                resp.status()
            )));
        }
        Ok(json!({
            "ok": true,
            "base_url": settings.base_url,
            "status": "online"
        }))
    }

    pub async fn list_models(&self, settings: &AppSettings) -> AppResult<Value> {
        let url = Self::models_url(settings);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", settings.api_key))
            .send()
            .await
            .map_err(|e| AppError::msg(format!("无法连接 LM Studio: {e}")))?;
        if !resp.status().is_success() {
            return Err(AppError::msg(format!(
                "获取模型列表失败: HTTP {}",
                resp.status()
            )));
        }
        let body: Value = resp.json().await?;
        Ok(body)
    }

    pub async fn chat(
        &self,
        settings: &AppSettings,
        messages: &[ChatMessage],
        options: &ChatOptions,
    ) -> AppResult<ChatResult> {
        let model = Self::resolve_model(settings, options)?;
        let body = Self::chat_body(settings, &model, messages, options, false);

        let resp = self
            .http
            .post(Self::chat_url(settings))
            .header("Authorization", format!("Bearer {}", settings.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                AppError::msg(format!(
                    "chat 请求失败（模型 {model}）: {e}。请确认 LM Studio 已加载该模型。"
                ))
            })?;

        let status = resp.status();
        let raw = resp
            .text()
            .await
            .map_err(|e| AppError::msg(format!("chat 读响应失败（模型 {model}）: {e}")))?;
        let data: Value = serde_json::from_str(&raw).map_err(|e| {
            AppError::msg(format!(
                "chat JSON 无效（模型 {model}，HTTP {status}）: {e}; 原文前200字: {}",
                raw.chars().take(200).collect::<String>()
            ))
        })?;
        if !status.is_success() {
            return Err(AppError::msg(format!(
                "chat 失败 HTTP {status}（模型 {model}）: {}",
                Self::format_api_error(&data)
            )));
        }
        if data.get("error").is_some() {
            return Err(AppError::msg(format!(
                "chat API error（模型 {model}）: {}",
                Self::format_api_error(&data)
            )));
        }
        let text = Self::extract_message_content(&data).unwrap_or_default();
        let usage = TokenUsage::or_estimate(
            data.get("usage").and_then(TokenUsage::from_api_json),
            messages,
            &text,
        );
        Ok(ChatResult { text, usage })
    }

    /// 流式 chat；失败或空结果时自动回退非流式一次。
    /// 若仍空且未关思考链，再强制 `thinking.disabled` 重试一次（DeepSeek 等推理模常见坑）。
    pub async fn chat_stream<F>(
        &self,
        settings: &AppSettings,
        messages: &[ChatMessage],
        options: &ChatOptions,
        cancel: Arc<AtomicBool>,
        mut on_delta: F,
    ) -> AppResult<ChatResult>
    where
        F: FnMut(&str),
    {
        let result = match self
            .chat_stream_inner(settings, messages, options, cancel.clone(), &mut on_delta)
            .await
        {
            Ok(r) if !r.text.trim().is_empty() => return Ok(r),
            Ok(_) => {
                let mut opts = options.clone();
                opts.stream = false;
                let r = self.chat(settings, messages, &opts).await?;
                if !r.text.is_empty() {
                    on_delta(&r.text);
                    return Ok(r);
                }
                r
            }
            Err(e) => {
                let msg = e.to_string();
                let recoverable = msg.contains("decoding")
                    || msg.contains("decode")
                    || msg.contains("UTF-8")
                    || msg.contains("utf8")
                    || msg.contains("connection")
                    || msg.contains("timed out")
                    || msg.contains("empty stream")
                    || msg.contains("非 SSE");
                if !recoverable {
                    return Err(e);
                }
                let mut opts = options.clone();
                opts.stream = false;
                match self.chat(settings, messages, &opts).await {
                    Ok(r) if !r.text.trim().is_empty() => {
                        on_delta(&r.text);
                        return Ok(r);
                    }
                    Ok(r) => r,
                    Err(e2) => {
                        return Err(AppError::msg(format!(
                            "流式失败（{msg}）；非流式亦失败: {e2}"
                        )));
                    }
                }
            }
        };

        if result.text.trim().is_empty() && !settings.resolve_disable_thinking() {
            let mut forced = settings.clone();
            forced.disable_thinking = Some(true);
            let mut opts = options.clone();
            opts.stream = false;
            eprintln!(
                "[llm] content 为空，强制 disable_thinking 后重试（model={})",
                Self::resolve_model(settings, options).unwrap_or_default()
            );
            let retry = self.chat(&forced, messages, &opts).await?;
            if !retry.text.is_empty() {
                on_delta(&retry.text);
            }
            return Ok(retry);
        }

        Ok(result)
    }

    async fn chat_stream_inner<F>(
        &self,
        settings: &AppSettings,
        messages: &[ChatMessage],
        options: &ChatOptions,
        cancel: Arc<AtomicBool>,
        on_delta: &mut F,
    ) -> AppResult<ChatResult>
    where
        F: FnMut(&str),
    {
        let model = Self::resolve_model(settings, options)?;
        let body = Self::chat_body(settings, &model, messages, options, true);

        let resp = self
            .http
            .post(Self::chat_url(settings))
            .header("Authorization", format!("Bearer {}", settings.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                AppError::msg(format!(
                    "stream 请求失败（模型 {model}）: {e}。请确认该模型已在 LM Studio 加载。"
                ))
            })?;

        let status = resp.status();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // 部分后端在 stream=true 时仍返回 JSON（模型未加载 / 错误）
        if content_type.contains("application/json") && !content_type.contains("text/event-stream")
        {
            let raw = resp.text().await.unwrap_or_default();
            let data: Value = serde_json::from_str(&raw).unwrap_or(json!({ "raw": raw }));
            if !status.is_success() || data.get("error").is_some() {
                return Err(AppError::msg(format!(
                    "stream 收到 JSON 错误（模型 {model}，HTTP {status}）: {}。请在 LM Studio 加载该模型后重试。",
                    Self::format_api_error(&data)
                )));
            }
            if let Some(content) = Self::extract_message_content(&data) {
                if !content.is_empty() {
                    on_delta(&content);
                }
                let usage = TokenUsage::or_estimate(
                    data.get("usage").and_then(TokenUsage::from_api_json),
                    messages,
                    &content,
                );
                return Ok(ChatResult {
                    text: content,
                    usage,
                });
            }
            return Err(AppError::msg(format!(
                "stream 非 SSE（模型 {model}）且无正文"
            )));
        }

        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::msg(format!(
                "stream chat 失败 HTTP {status}（模型 {model}）: {text}"
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut byte_buf: Vec<u8> = Vec::new();
        let mut line_buf = String::new();
        let mut full = String::new();
        let mut api_usage: Option<TokenUsage> = None;

        while let Some(item) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::msg("生成已取消"));
            }
            let chunk = item.map_err(|e| {
                AppError::msg(format!(
                    "stream 读块失败（模型 {model}）: {e}"
                ))
            })?;
            byte_buf.extend_from_slice(&chunk);

            // 尽量按完整 UTF-8 切分，避免多字节汉字被切断
            let ok_upto = match std::str::from_utf8(&byte_buf) {
                Ok(_) => byte_buf.len(),
                Err(e) => e.valid_up_to(),
            };
            if ok_upto == 0 {
                if byte_buf.len() > 8 {
                    return Err(AppError::msg(format!(
                        "stream UTF-8 无效（模型 {model}）"
                    )));
                }
                continue;
            }
            let piece = String::from_utf8_lossy(&byte_buf[..ok_upto]).into_owned();
            byte_buf.drain(..ok_upto);
            line_buf.push_str(&piece);

            while let Some(pos) = line_buf.find('\n') {
                let line = line_buf[..pos].trim_end_matches('\r').to_string();
                line_buf.drain(..=pos);
                if line.is_empty() {
                    continue;
                }
                // 偶发整包 JSON
                if line.starts_with('{') {
                    if let Ok(v) = serde_json::from_str::<Value>(&line) {
                        if v.get("error").is_some() {
                            return Err(AppError::msg(format!(
                                "stream 中途错误（模型 {model}）: {}",
                                Self::format_api_error(&v)
                            )));
                        }
                        if let Some(u) = v.get("usage").and_then(TokenUsage::from_api_json) {
                            api_usage = Some(u);
                        }
                        if let Some(content) = Self::extract_message_content(&v) {
                            full.push_str(&content);
                            on_delta(&content);
                            continue;
                        }
                    }
                }
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        let usage =
                            TokenUsage::or_estimate(api_usage.take(), messages, &full);
                        return Ok(ChatResult { text: full, usage });
                    }
                    if let Ok(v) = serde_json::from_str::<Value>(data) {
                        if let Some(u) = v.get("usage").and_then(TokenUsage::from_api_json) {
                            api_usage = Some(u);
                        }
                        if let Some(delta) = v
                            .pointer("/choices/0/delta/content")
                            .and_then(|x| x.as_str())
                        {
                            if !delta.is_empty() {
                                full.push_str(delta);
                                on_delta(delta);
                            }
                        }
                    }
                }
            }
        }
        if full.trim().is_empty() {
            return Err(AppError::msg(format!(
                "empty stream（模型 {model}）：无增量。可能模型未加载或后端未返回 SSE。"
            )));
        }
        let usage = TokenUsage::or_estimate(api_usage, messages, &full);
        Ok(ChatResult { text: full, usage })
    }

    fn embeddings_url(settings: &AppSettings) -> String {
        format!("{}/embeddings", settings.base_url.trim_end_matches('/'))
    }

    /// OpenAI 兼容 embeddings；返回向量列表（通常一条）
    pub async fn embed(
        &self,
        settings: &AppSettings,
        model: &str,
        inputs: &[String],
    ) -> AppResult<Vec<Vec<f32>>> {
        if model.trim().is_empty() {
            return Err(AppError::msg("未配置 embedding_model"));
        }
        if inputs.is_empty() {
            return Ok(vec![]);
        }
        let body = json!({
            "model": model,
            "input": inputs
        });
        let resp = self
            .http
            .post(Self::embeddings_url(settings))
            .header("Authorization", format!("Bearer {}", settings.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::msg(format!("embeddings 请求失败: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::msg(format!(
                "embeddings 失败 HTTP {status}: {text}"
            )));
        }
        let data: Value = resp.json().await?;
        let mut out = Vec::new();
        if let Some(arr) = data.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(emb) = item.get("embedding").and_then(|v| v.as_array()) {
                    let vec: Vec<f32> = emb
                        .iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect();
                    out.push(vec);
                }
            }
        }
        if out.is_empty() {
            return Err(AppError::msg("embeddings 响应无向量"));
        }
        Ok(out)
    }
}
