//! 块记忆摘要清洗：去掉婴儿相关词汇
//! 代码路径: kk_novel_ai/src-tauri/src/project/digest_sanitize.rs

/// 按长度降序；替换时先长后短，避免截断半词。
const BABY_TERMS: &[&str] = &[
    "breastfeeding",
    "新生儿",
    "小婴儿",
    "婴儿车",
    "婴儿床",
    "尿不湿",
    "小宝宝",
    "初生儿",
    "哺乳期",
    "满月酒",
    "newborn",
    "infant",
    "diaper",
    "cradle",
    "fetus",
    "婴儿",
    "宝宝",
    "胎儿",
    "襁褓",
    "奶嘴",
    "尿布",
    "摇篮",
    "满月",
    "月子",
    "哺乳",
    "孕妇",
    "怀孕",
    "分娩",
    "产检",
    "胎动",
    "羊水",
    "脐带",
    "奶粉",
    "胎教",
    "幼婴",
    "吃奶",
    "母乳",
    "断奶",
    "育婴",
    "baby",
];

/// 去掉摘要中的婴儿相关词，并整理多余空白与标点。
pub fn sanitize_block_digest(text: &str) -> String {
    let mut out = text.to_string();
    for term in BABY_TERMS {
        if term.is_empty() {
            continue;
        }
        out = out.replace(term, "");
        // 英文词大小写变体
        let lower = term.to_ascii_lowercase();
        if lower != *term {
            out = out.replace(&lower, "");
        }
        let titled: String = lower
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i == 0 {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
            .collect();
        if titled != *term && titled != lower {
            out = out.replace(&titled, "");
        }
    }
    tidy_digest(&out)
}

fn tidy_digest(text: &str) -> String {
    let mut s = text.to_string();
    for _ in 0..4 {
        let next = s
            .replace("  ", " ")
            .replace("\n\n\n", "\n\n")
            .replace("，，", "，")
            .replace("、、", "、")
            .replace("。。", "。")
            .replace("；；", "；")
            .replace("：：", "：")
            .replace("，。", "。")
            .replace("。，", "。")
            .replace("（）", "")
            .replace("()", "")
            .replace("「」", "")
            .replace("【】", "");
        if next == s {
            break;
        }
        s = next;
    }
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| {
            c.is_whitespace() || matches!(c, '，' | '、' | '。' | '；' | '：' | ',' | ';' | ':')
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_baby_words() {
        let raw = "娜娜抱着婴儿入睡，宝宝很安静。";
        let cleaned = sanitize_block_digest(raw);
        assert!(!cleaned.contains("婴儿"));
        assert!(!cleaned.contains("宝宝"));
        assert!(cleaned.contains("娜娜"));
    }
}
