//! 写作 prompt 按 AppSettings.writing_locale 选择
//! 代码路径: kk_novel_ai/src-tauri/src/prompt_i18n.rs

/// 当前写作语言：`zh-CN` | `en` | `ja`
pub fn writing_locale() -> String {
    match crate::settings::load_settings() {
        Ok(s) => crate::settings::AppSettings::normalize_locale_code(&s.writing_locale),
        Err(_) => "zh-CN".into(),
    }
}

/// 按 writing_locale 取编译期嵌入的 prompt。根目录 `prompts/*.md` 为 zh-CN 回退。
pub fn prompt(name: &str) -> &'static str {
    match (writing_locale().as_str(), name) {
        ("en", "continue_chapter.md") => include_str!("../prompts/en/continue_chapter.md"),
        ("ja", "continue_chapter.md") => include_str!("../prompts/ja/continue_chapter.md"),
        (_, "continue_chapter.md") => include_str!("../prompts/continue_chapter.md"),

        ("en", "continue_chapter_cache.md") => include_str!("../prompts/en/continue_chapter_cache.md"),
        ("ja", "continue_chapter_cache.md") => include_str!("../prompts/ja/continue_chapter_cache.md"),
        (_, "continue_chapter_cache.md") => include_str!("../prompts/continue_chapter_cache.md"),

        ("en", "same_slot_variant.md") => include_str!("../prompts/en/same_slot_variant.md"),
        ("ja", "same_slot_variant.md") => include_str!("../prompts/ja/same_slot_variant.md"),
        (_, "same_slot_variant.md") => include_str!("../prompts/same_slot_variant.md"),

        ("en", "polish.md") => include_str!("../prompts/en/polish.md"),
        ("ja", "polish.md") => include_str!("../prompts/ja/polish.md"),
        (_, "polish.md") => include_str!("../prompts/polish.md"),

        ("en", "outline_expand.md") => include_str!("../prompts/en/outline_expand.md"),
        ("ja", "outline_expand.md") => include_str!("../prompts/ja/outline_expand.md"),
        (_, "outline_expand.md") => include_str!("../prompts/outline_expand.md"),

        ("en", "consistency_check.md") => include_str!("../prompts/en/consistency_check.md"),
        ("ja", "consistency_check.md") => include_str!("../prompts/ja/consistency_check.md"),
        (_, "consistency_check.md") => include_str!("../prompts/consistency_check.md"),

        ("en", "chapter_summary.md") => include_str!("../prompts/en/chapter_summary.md"),
        ("ja", "chapter_summary.md") => include_str!("../prompts/ja/chapter_summary.md"),
        (_, "chapter_summary.md") => include_str!("../prompts/chapter_summary.md"),

        ("en", "story_sync.md") => include_str!("../prompts/en/story_sync.md"),
        ("ja", "story_sync.md") => include_str!("../prompts/ja/story_sync.md"),
        (_, "story_sync.md") => include_str!("../prompts/story_sync.md"),

        ("en", "block_digest.md") => include_str!("../prompts/en/block_digest.md"),
        ("ja", "block_digest.md") => include_str!("../prompts/ja/block_digest.md"),
        (_, "block_digest.md") => include_str!("../prompts/block_digest.md"),

        ("en", "cast_extract.md") => include_str!("../prompts/en/cast_extract.md"),
        ("ja", "cast_extract.md") => include_str!("../prompts/ja/cast_extract.md"),
        (_, "cast_extract.md") => include_str!("../prompts/cast_extract.md"),

        ("en", "section_plan.md") => include_str!("../prompts/en/section_plan.md"),
        ("ja", "section_plan.md") => include_str!("../prompts/ja/section_plan.md"),
        (_, "section_plan.md") => include_str!("../prompts/section_plan.md"),

        ("en", "outline_to_beats.md") => include_str!("../prompts/en/outline_to_beats.md"),
        ("ja", "outline_to_beats.md") => include_str!("../prompts/ja/outline_to_beats.md"),
        (_, "outline_to_beats.md") => include_str!("../prompts/outline_to_beats.md"),

        ("en", "outline_to_chapters.md") => include_str!("../prompts/en/outline_to_chapters.md"),
        ("ja", "outline_to_chapters.md") => include_str!("../prompts/ja/outline_to_chapters.md"),
        (_, "outline_to_chapters.md") => include_str!("../prompts/outline_to_chapters.md"),

        ("en", "outline_to_mindmap.md") => include_str!("../prompts/en/outline_to_mindmap.md"),
        ("ja", "outline_to_mindmap.md") => include_str!("../prompts/ja/outline_to_mindmap.md"),
        (_, "outline_to_mindmap.md") => include_str!("../prompts/outline_to_mindmap.md"),

        ("en", "beats_to_storyboard.md") => include_str!("../prompts/en/beats_to_storyboard.md"),
        ("ja", "beats_to_storyboard.md") => include_str!("../prompts/ja/beats_to_storyboard.md"),
        (_, "beats_to_storyboard.md") => include_str!("../prompts/beats_to_storyboard.md"),

        ("en", "content_to_image_prompt.md") => {
            include_str!("../prompts/en/content_to_image_prompt.md")
        }
        ("ja", "content_to_image_prompt.md") => {
            include_str!("../prompts/ja/content_to_image_prompt.md")
        }
        (_, "content_to_image_prompt.md") => include_str!("../prompts/content_to_image_prompt.md"),

        ("en", "length_fill.md") => include_str!("../prompts/en/length_fill.md"),
        ("ja", "length_fill.md") => include_str!("../prompts/ja/length_fill.md"),
        (_, "length_fill.md") => include_str!("../prompts/length_fill.md"),

        ("en", "suggest_book_title.md") => include_str!("../prompts/en/suggest_book_title.md"),
        ("ja", "suggest_book_title.md") => include_str!("../prompts/ja/suggest_book_title.md"),
        (_, "suggest_book_title.md") => include_str!("../prompts/suggest_book_title.md"),

        ("en", "lore_extract.md") => include_str!("../prompts/en/lore_extract.md"),
        ("ja", "lore_extract.md") => include_str!("../prompts/ja/lore_extract.md"),
        (_, "lore_extract.md") => include_str!("../prompts/lore_extract.md"),

        _ => "",
    }
}
