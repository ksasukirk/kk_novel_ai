//! 统一错误类型
//! 代码路径: kk_novel_ai/src-tauri/src/error.rs

use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({ "ok": false, "error": self.to_string() })
    }
}

pub type AppResult<T> = Result<T, AppError>;

impl From<AppError> for String {
    fn from(value: AppError) -> Self {
        value.to_string()
    }
}
