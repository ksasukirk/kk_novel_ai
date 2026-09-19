//! 导入时一批翻书名和章标题（不翻正文）
//! 代码路径: kk_novel_ai/src-tauri/src/import/title_translate.rs

use crate::error::{AppError, AppResult};
use crate::genlog;
use crate::llm::{ChatMessage, ChatOptions, LmStudioClient};
use crate::settings::{self, AppSettings};
use serde_json::{json, Value};

use super::strip_json_fence;

pub struct TitleTranslateOut {
    pub book_title: String,
    pub chapters: Vec<String>,
}

pub fn lang_name(locale: &str) -> &'static str {
    match AppSettings::normalize_locale_code(locale).as_str() {
        "en" => "English",
        "ja" => "日本語",
        _ => "简体中文",
    }
}

pub fn title_looks_like_locale(text: &str, locale: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    let has_han = t.chars().any(is_cjk_han);
    let has_kana = t.chars().any(is_kana);
    let has_latin = t.chars().any(|c| c.is_ascii_alphabetic());
    match AppSettings::normalize_locale_code(locale).as_str() {
        "en" => has_latin && !has_han && !has_kana,
        "ja" => has_kana,
        _ => has_han && !has_kana && !has_latin,
    }
}

fn is_cjk_han(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
    )
}

fn is_kana(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}')
}

/// 书名和全部章标题都已像目标语时，整批跳过模型。
pub fn all_titles_look_like(book: &str, chapters: &[String], locale: &str) -> bool {
    if !title_looks_like_locale(book, locale) {
        return false;
    }
    chapters.iter().all(|t| title_looks_like_locale(t, locale))
}

pub fn parse_title_translate_json(raw: &str, chapter_count: usize) -> AppResult<TitleTranslateOut> {
    let cleaned = strip_json_fence(raw);
    if cleaned.is_empty() {
        return Err(AppError::t("errors.titleTranslateEmpty"));
    }
    let obj: Value = serde_json::from_str(&cleaned).map_err(|_| AppError::t("errors.titleTranslateEmpty"))?;
    let book_title = obj
        .get("book_title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if book_title.is_empty() {
        return Err(AppError::t("errors.titleTranslateEmpty"));
    }
    let chapters = obj
        .get("chapters")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if chapters.len() != chapter_count {
        return Err(AppError::t("errors.titleTranslateEmpty"));
    }
    Ok(TitleTranslateOut {
        book_title,
        chapters,
    })
}

/// 成功则 Some；已像目标语则 None（不调模型）。失败由调用方吞掉、沿用原文。
pub async fn maybe_translate_titles(
    book: &str,
    chapters: &[String],
    locale: &str,
    project_root: &str,
) -> AppResult<Option<TitleTranslateOut>> {
    let locale = AppSettings::normalize_locale_code(locale);
    if all_titles_look_like(book, chapters, &locale) {
        return Ok(None);
    }
    let s = settings::load_settings()?;
    let model = s.resolve_analysis_model().to_string();
    let tpl = crate::prompt_i18n::title_translate_prompt(&locale);
    let payload = json!({
        "book_title": book,
        "chapters": chapters,
    });
    let user = tpl
        .replace("{{locale}}", &locale)
        .replace("{{lang_name}}", lang_name(&locale))
        .replace("{{payload}}", &payload.to_string());
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: user,
        },
        ChatMessage {
            role: "user".into(),
            content: payload.to_string(),
        },
    ];
    let max_tokens = ((chapters.len() as u32).saturating_mul(24) + 128).clamp(256, 4096);
    let options = ChatOptions {
        model: Some(model.clone()),
        temperature: Some(0.2),
        max_tokens: Some(max_tokens),
        stream: false,
        ..Default::default()
    };
    let client = LmStudioClient::from_settings(&s);
    let r = client.chat(&s, &messages, &options).await?;
    let parsed = parse_title_translate_json(&r.text, chapters.len())?;
    let _ = genlog::record_llm_call(
        "title_translate",
        project_root,
        "",
        &r.text,
        &parsed.book_title,
        "import_novel_txt",
        false,
        &model,
        "title_translate",
        &messages,
        Some(r.usage.clone()),
        &s,
    );
    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_looks_english() {
        assert!(title_looks_like_locale("The Silent Sea", "en"));
        assert!(!title_looks_like_locale("寂静之海", "en"));
        assert!(title_looks_like_locale("寂静之海", "zh-CN"));
        assert!(!title_looks_like_locale("The Silent Sea", "zh-CN"));
        assert!(title_looks_like_locale("静かなる海", "ja"));
    }

    #[test]
    fn parse_json_counts_must_match() {
        let raw = r#"{"book_title":"Sea","chapters":["One","Two"]}"#;
        let out = parse_title_translate_json(raw, 2).unwrap();
        assert_eq!(out.book_title, "Sea");
        assert_eq!(out.chapters, vec!["One", "Two"]);
        assert!(parse_title_translate_json(raw, 3).is_err());
    }

    #[test]
    fn skip_when_already_target() {
        let ch = vec!["Chapter One".into(), "Chapter Two".into()];
        assert!(all_titles_look_like("The Book", &ch, "en"));
        assert!(!all_titles_look_like("那本书", &ch, "en"));
    }
}
