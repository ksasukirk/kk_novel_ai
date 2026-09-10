//! 情节/性癖规范分类标签（与前端 tropeCategories.js 对齐）
//! 代码路径: kk_novel_ai/src-tauri/src/project/trope_tags.rs

pub const CANON_TAGS: &[&str] = &[
    "暴露", "公开", "后庭", "口交", "性交", "排泄", "体罚", "羞耻", "束缚", "控制",
    "群戏", "道具", "体液", "禁忌", "撞见", "秘密",
];

const ALIASES: &[(&str, &str)] = &[
    ("真空", "暴露"),
    ("裙下", "暴露"),
    ("露出", "暴露"),
    ("裸露", "暴露"),
    ("暴露", "暴露"),
    ("半公开", "公开"),
    ("当众", "公开"),
    ("户外", "公开"),
    ("公开", "公开"),
    ("后穴", "后庭"),
    ("屁穴", "后庭"),
    ("肛交", "后庭"),
    ("肛", "后庭"),
    ("后庭", "后庭"),
    ("口侍", "口交"),
    ("口交", "口交"),
    ("抽插", "性交"),
    ("插入", "性交"),
    ("性交", "性交"),
    ("憋尿", "排泄"),
    ("失禁", "排泄"),
    ("排尿", "排泄"),
    ("排泄", "排泄"),
    ("打屁股", "体罚"),
    ("体罚", "体罚"),
    ("羞耻", "羞耻"),
    ("束缚", "束缚"),
    ("捆绑", "束缚"),
    ("遥控", "控制"),
    ("控制", "控制"),
    ("群交", "群戏"),
    ("群戏", "群戏"),
    ("跳蛋", "道具"),
    ("玩具", "道具"),
    ("道具", "道具"),
    ("精液", "体液"),
    ("体液", "体液"),
    ("乱伦", "禁忌"),
    ("禁忌", "禁忌"),
    ("撞见", "撞见"),
    ("被抓", "撞见"),
    ("秘密", "秘密"),
    ("隐瞒", "秘密"),
];

fn is_canon(s: &str) -> bool {
    CANON_TAGS.iter().any(|c| *c == s)
}

/// 单段文本收到规范名；对不上返回空串
pub fn normalize_tag(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return String::new();
    }
    if is_canon(s) {
        return s.to_string();
    }
    let hay: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut best = "";
    let mut best_len = 0usize;
    for (alias, canon) in ALIASES {
        if hay.contains(alias) && alias.chars().count() >= best_len {
            best = canon;
            best_len = alias.chars().count();
        }
    }
    best.to_string()
}

fn split_raw(raw: &str) -> Vec<String> {
    raw.split(|c: char| {
        matches!(c, ',' | '，' | '、' | ';' | '；' | '/' | '|')
    })
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .collect()
}

/// 白名单排序后的规范标签
pub fn parse_tag_list(raw: &str) -> Vec<String> {
    let mut found = Vec::new();
    for p in split_raw(raw) {
        let n = normalize_tag(&p);
        if !n.is_empty() && !found.iter().any(|x| x == &n) {
            found.push(n);
        }
    }
    CANON_TAGS
        .iter()
        .filter(|c| found.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect()
}

pub fn parse_tag_values(values: &[String]) -> Vec<String> {
    parse_tag_list(&values.join(","))
}

pub fn format_tag_list(tags: &[String]) -> String {
    parse_tag_list(&tags.join(",")).join(", ")
}

pub fn merge_tag_strings(a: &str, b: &str) -> String {
    format_tag_list(
        &split_raw(a)
            .into_iter()
            .chain(split_raw(b))
            .collect::<Vec<_>>(),
    )
}

pub fn infer_tags(title: &str, keywords: &[String]) -> Vec<String> {
    let mut hay = title.to_string();
    for k in keywords {
        hay.push(' ');
        hay.push_str(k);
    }
    let compact: String = hay.chars().filter(|c| !c.is_whitespace()).collect();
    let mut found = Vec::new();
    for (alias, canon) in ALIASES {
        if compact.contains(alias) && !found.iter().any(|x| x == canon) {
            found.push((*canon).to_string());
        }
    }
    CANON_TAGS
        .iter()
        .filter(|c| found.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_vacuum_to_expose() {
        assert_eq!(normalize_tag("真空出门"), "暴露");
        assert_eq!(normalize_tag("裙下轮廓"), "暴露");
    }

    #[test]
    fn alias_anus_to_rear() {
        assert_eq!(normalize_tag("后穴"), "后庭");
        assert_eq!(normalize_tag("肛交"), "后庭");
    }

    #[test]
    fn drops_unknown() {
        assert!(normalize_tag("龙傲天").is_empty());
        assert!(parse_tag_list("foo, bar").is_empty());
    }

    #[test]
    fn merge_unions_and_dedupes() {
        let s = merge_tag_strings("暴露, 后庭", "后庭；口交");
        assert_eq!(s, "暴露, 后庭, 口交");
    }

    #[test]
    fn infer_from_title_keywords() {
        let tags = infer_tags("真空短裙", &["憋尿".into()]);
        assert!(tags.contains(&"暴露".to_string()));
        assert!(tags.contains(&"排泄".to_string()));
    }
}
