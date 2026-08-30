//! 正文修辞口癖清洗：去掉「不是A，是B」等非角色定义套话
//! 代码路径: kk_novel_ai/src-tauri/src/writing/rhetoric.rs

use regex::Regex;
use std::sync::OnceLock;

fn re_not_but() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // 不是X，是Y / 不是X，而是Y / 并非X，而是Y
        // 不匹配「是不是」问句（「是不是」不含此结构）
        Regex::new(
            r"(?:不是|并非)([^，。；！？\n]{0,40})[，,](?:而)?是([^，。；！？\n]{1,40})",
        )
        .expect("rhetoric regex")
    })
}

fn re_not_kind() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"不是那种([^，。；！？\n]{0,30})[，,](?:而)?是那种([^，。；！？\n]{1,30})")
            .expect("rhetoric kind regex")
    })
}

fn re_not_period_is() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // 「不是X。是Y」跨句定义
    RE.get_or_init(|| {
        Regex::new(r"不是([^。！？\n]{1,40})。[ \t]*是([^，。；！？\n]{1,40})")
            .expect("rhetoric period regex")
    })
}

/// 去掉否定对照套话，尽量保留肯定侧语义；保留「是不是」等正常用法。
pub fn sanitize_rhetoric(text: &str) -> String {
    let mut out = text.to_string();
    for _ in 0..4 {
        let next = re_not_kind().replace_all(&out, |caps: &regex::Captures| {
            let b = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if b.is_empty() {
                caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string()
            } else {
                b.to_string()
            }
        });
        let next = re_not_but().replace_all(&next, |caps: &regex::Captures| {
            let b = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if b.is_empty() {
                caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string()
            } else {
                b.to_string()
            }
        });
        let next = re_not_period_is().replace_all(&next, |caps: &regex::Captures| {
            let b = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if b.is_empty() {
                caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string()
            } else {
                format!("{b}。")
            }
        });
        let next = tidy(&next);
        if next == out {
            break;
        }
        out = next;
    }
    out
}

fn tidy(s: &str) -> String {
    let mut t = s.to_string();
    for _ in 0..3 {
        let n = t
            .replace("，，", "，")
            .replace("。。", "。")
            .replace("  ", " ")
            .replace("，。", "。")
            .replace("。，", "。");
        if n == t {
            break;
        }
        t = n;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_not_but() {
        let raw = "她觉得不是疼，是快感。旁边是不是在看？";
        let out = sanitize_rhetoric(raw);
        assert!(!out.contains("不是疼"));
        assert!(out.contains("快感"));
        assert!(out.contains("是不是"));
    }

    #[test]
    fn strips_not_rather() {
        let raw = "并非惩罚，而是公平。";
        let out = sanitize_rhetoric(raw);
        assert!(out.contains("公平"));
        assert!(!out.contains("并非"));
    }
}
