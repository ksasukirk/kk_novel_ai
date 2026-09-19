//! 统一错误类型
//! 代码路径: kk_novel_ai/src-tauri/src/error.rs

use serde::de::DeserializeOwned;
use serde_json::json;
use std::path::Path;
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

    pub fn t(key: &str) -> Self {
        Self::Message(crate::i18n::t(key))
    }

    pub fn t_fmt(key: &str, args: &[(&str, &str)]) -> Self {
        Self::Message(crate::i18n::t_fmt(key, args))
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

/// 空文件 / 只空白 / 只有 BOM：serde 会报 `EOF while parsing a value at line 1 column 0`
pub fn json_is_blank(text: &str) -> bool {
    text.trim_start_matches('\u{feff}').trim().is_empty()
}

/// 必填 JSON：空文件给人话路径，损坏带 serde 细节
pub fn parse_json_at<T: DeserializeOwned>(text: &str, path: &Path) -> AppResult<T> {
    if json_is_blank(text) {
        return Err(AppError::t_fmt(
            "errors.jsonEmpty",
            &[("path", &path.display().to_string())],
        ));
    }
    serde_json::from_str(text).map_err(|e| {
        AppError::t_fmt(
            "errors.jsonInvalid",
            &[
                ("path", &path.display().to_string()),
                ("e", &e.to_string()),
            ],
        )
    })
}
