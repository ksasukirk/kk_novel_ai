//! 对话会话落盘（本作 / 自由聊），不写入章节
//! 代码路径: kk_novel_ai/src-tauri/src/chat.rs

use crate::error::{AppError, AppResult};
use crate::paths;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatTurn {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatSession {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub messages: Vec<ChatTurn>,
    /// 气泡显示名；空则前端回退「助手」
    #[serde(default)]
    pub assistant_name: String,
    #[serde(default)]
    pub assistant_style: String,
    #[serde(default)]
    pub assistant_persona: String,
    #[serde(default)]
    pub updated_at: String,
}

fn session_path(mode: &str, project_root: Option<&str>) -> AppResult<PathBuf> {
    match mode {
        "novel" => {
            let root = project_root
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::t("errors.needProject"))?;
            let dir = Path::new(root).join("chat");
            fs::create_dir_all(&dir)?;
            Ok(dir.join("novel.json"))
        }
        "free" => {
            let dir = paths::app_data_dir()?.join("chat");
            fs::create_dir_all(&dir)?;
            Ok(dir.join("free.json"))
        }
        _ => Err(AppError::t_fmt("errors.unknownChatMode", &[("mode", mode)])),
    }
}

pub fn chat_session_get(mode: &str, project_root: Option<&str>) -> AppResult<Value> {
    let path = session_path(mode, project_root)?;
    let mut session = if path.exists() {
        let text = fs::read_to_string(&path)?;
        if crate::error::json_is_blank(&text) {
            ChatSession {
                mode: mode.into(),
                ..Default::default()
            }
        } else {
            serde_json::from_str(&text).unwrap_or_else(|_| ChatSession {
                mode: mode.into(),
                ..Default::default()
            })
        }
    } else {
        ChatSession {
            mode: mode.into(),
            ..Default::default()
        }
    };
    if session.mode.is_empty() {
        session.mode = mode.into();
    }
    Ok(json!({ "ok": true, "session": session }))
}

pub fn chat_session_save(
    mode: &str,
    project_root: Option<&str>,
    session: ChatSession,
) -> AppResult<Value> {
    let path = session_path(mode, project_root)?;
    let mut s = session;
    s.mode = mode.into();
    s.updated_at = chrono::Utc::now().to_rfc3339();
    s.messages.retain(|m| {
        let role = m.role.trim();
        role == "user" || role == "assistant"
    });
    fs::write(&path, serde_json::to_string_pretty(&s)?)?;
    Ok(json!({ "ok": true, "session": s }))
}
