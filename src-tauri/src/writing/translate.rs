//! 章节/块正文翻译（按段切块，点一下翻一段或一章）
//! 代码路径: kk_novel_ai/src-tauri/src/writing/translate.rs

use crate::error::{AppError, AppResult};
use crate::genlog;
use crate::llm::{ChatMessage, ChatOptions, LmStudioClient};
use crate::settings::{self, AppSettings};
use serde_json::json;

pub fn lang_name(locale: &str) -> &'static str {
    match AppSettings::normalize_locale_code(locale).as_str() {
        "en" => "English",
        "ja" => "日本語",
        _ => "简体中文",
    }
}

fn is_cjk_han(c: char) -> bool {
    matches!(
        c,
        '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{F900}'..='\u{FAFF}'
    )
}

fn is_kana(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}')
}

/// 取正文前若干字母类字符，判断是否已经像目标语。
pub fn prose_looks_like_locale(text: &str, locale: &str) -> bool {
    let sample: String = text
        .chars()
        .filter(|c| c.is_alphabetic() || is_cjk_han(*c) || is_kana(*c))
        .take(80)
        .collect();
    if sample.chars().count() < 12 {
        return false;
    }
    let has_han = sample.chars().any(is_cjk_han);
    let has_kana = sample.chars().any(is_kana);
    let has_latin = sample.chars().any(|c| c.is_ascii_alphabetic());
    match AppSettings::normalize_locale_code(locale).as_str() {
        "en" => has_latin && !has_han && !has_kana,
        "ja" => has_kana,
        _ => has_han && !has_kana && !has_latin,
    }
}

/// 翻译切块：偏短，好给输出留 max_tokens。
pub fn chunk_for_translate(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    const SOFT: usize = 2800;
    const HARD: usize = 3600;
    if trimmed.chars().count() <= SOFT {
        return vec![trimmed.to_string()];
    }
    let paras: Vec<&str> = if trimmed.contains("\n\n") {
        trimmed.split("\n\n").collect()
    } else {
        trimmed.split('\n').collect()
    };
    let mut chunks: Vec<String> = Vec::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, chunks: &mut Vec<String>| {
        let t = buf.trim();
        if !t.is_empty() {
            chunks.push(t.to_string());
        }
        buf.clear();
    };
    for p in paras {
        let extra = if buf.is_empty() { 0 } else { 2 };
        let next = buf.chars().count() + extra + p.chars().count();
        if !buf.is_empty() && next > HARD {
            flush(&mut buf, &mut chunks);
        }
        if !buf.is_empty() {
            buf.push_str("\n\n");
        }
        buf.push_str(p);
        while buf.chars().count() > HARD {
            let take: String = buf.chars().take(HARD).collect();
            chunks.push(take);
            buf = buf.chars().skip(HARD).collect();
        }
    }
    flush(&mut buf, &mut chunks);
    chunks
}

pub async fn translate_prose(
    text: &str,
    locale: &str,
    project_root: &str,
    chapter_id: &str,
) -> AppResult<(String, bool, usize)> {
    let locale = AppSettings::normalize_locale_code(locale);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::t("errors.translateEmpty"));
    }
    if prose_looks_like_locale(trimmed, &locale) {
        return Ok((trimmed.to_string(), true, 0));
    }
    let chunks = chunk_for_translate(trimmed);
    if chunks.is_empty() {
        return Err(AppError::t("errors.translateEmpty"));
    }
    let s = settings::load_settings()?;
    let model = s.resolve_analysis_model().to_string();
    let tpl = crate::prompt_i18n::body_translate_prompt(&locale);
    let system = tpl
        .replace("{{locale}}", &locale)
        .replace("{{lang_name}}", lang_name(&locale));
    let client = LmStudioClient::from_settings(&s);
    let mut out = String::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system.clone(),
            },
            ChatMessage {
                role: "user".into(),
                content: chunk.clone(),
            },
        ];
        let max_tokens = ((chunk.chars().count() as u32).saturating_mul(3) + 256).clamp(1024, 8192);
        let options = ChatOptions {
            model: Some(model.clone()),
            temperature: Some(0.2),
            max_tokens: Some(max_tokens),
            stream: false,
            ..Default::default()
        };
        let r = client.chat(&s, &messages, &options).await?;
        let piece = r.text.trim();
        if piece.is_empty() {
            return Err(AppError::t("errors.translateEmpty"));
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(piece);
        let preview: String = piece.chars().take(80).collect();
        let _ = genlog::record_llm_call(
            "chapter_translate",
            project_root,
            chapter_id,
            &r.text,
            &preview,
            "chapter_translate",
            false,
            &model,
            &format!("body_translate {}/{}", i + 1, chunks.len()),
            &messages,
            Some(r.usage.clone()),
            &s,
        );
    }
    Ok((out, false, chunks.len()))
}

pub async fn chapter_translate_json(
    root: &str,
    chapter_id: &str,
    locale: &str,
    selection: &str,
) -> AppResult<serde_json::Value> {
    let text = if selection.trim().is_empty() {
        let path = std::path::Path::new(root);
        let (_, content) = crate::project::read_chapter(path, chapter_id)?;
        content
    } else {
        selection.to_string()
    };
    let (translated, skipped, chunks) =
        translate_prose(&text, locale, root, chapter_id).await?;
    Ok(json!({
        "ok": true,
        "text": translated,
        "skipped": skipped,
        "chunks": chunks,
        "locale": AppSettings::normalize_locale_code(locale),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_one_chunk() {
        let c = chunk_for_translate("hello world this is a short paragraph");
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn long_text_splits() {
        let text = "甲".repeat(9000);
        let c = chunk_for_translate(&text);
        assert!(c.len() >= 2, "got {}", c.len());
        assert!(c.iter().all(|x| x.chars().count() <= 3600));
    }

    #[test]
    fn latin_body_looks_english() {
        let t = "The rain kept falling on the empty street while she waited by the door. ";
        assert!(prose_looks_like_locale(&t.repeat(3), "en"));
        assert!(!prose_looks_like_locale(&t.repeat(3), "zh-CN"));
        let zh = "雨一直下着，她站在门口等那辆不会来的车。雨一直下着，她站在门口。";
        assert!(prose_looks_like_locale(zh, "zh-CN"));
        assert!(!prose_looks_like_locale(zh, "en"));
    }
}
