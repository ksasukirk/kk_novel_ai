//! 续写事前约束：动态禁止清单、节拍状态、叙事方向锚点
//! 代码路径: kk_novel_ai/src-tauri/src/writing/advance.rs

use crate::project::{ChapterBeatProgress, SceneBeat};
use crate::writing::beat_engine;
use std::collections::HashMap;

/// 高频易复读动作词（内部词库；命中近期正文则写入禁止清单）
const ACTION_LEXICON: &[&str] = &[
    "张嘴",
    "舔干净",
    "舔净",
    "跪下来",
    "跪在",
    "清理",
    "把地上也舔",
    "对准我的鞋",
    "尿在我身上",
    "深喉",
    "夹好了",
    "余韵",
];

/// 女体解剖硬约束（通用；不点名具体角色，避免跨书污染）
const ANATOMY_BANS: &[&str] = &[
    "禁止写女角色拥有阴茎/鸡巴/睾丸（设定明确男体/双性除外）；女体仅有阴道、尿道、后穴、乳房",
    "阴茎/鸡巴与射精主体只能是明确男角色；禁止「她的阴茎」「她射精」等主宾颠倒",
    "女角色支配口交时须写清入口的是男角色的阴茎；禁止写成女角色胯下长出阴茎",
];

fn needs_anatomy_bans(recent: &str, must_not: &str) -> bool {
    must_not.contains("女体")
        || recent.contains("超短裙")
        || recent.contains("真空")
        || recent.contains("阴茎")
        || recent.contains("鸡巴")
        || recent.contains("射精")
        || recent.contains("口交")
        || recent.contains("后穴")
        || recent.contains("小穴")
        || recent.contains("阴道")
}

/// 从近期正文提取动态禁止项。
pub fn build_dynamic_ban_list(recent: &str, must_not: &str) -> String {
    let mut bans: Vec<String> = Vec::new();
    if !must_not.trim().is_empty() {
        for part in must_not.split(|c| c == '；' || c == ';' || c == '，' || c == ',') {
            let t = part.trim();
            if t.chars().count() >= 2 {
                bans.push(t.to_string());
            }
        }
    }
    // 涩向/女体语境才注入解剖硬禁；不因某个角色名全局触发
    if needs_anatomy_bans(recent, must_not) {
        for a in ANATOMY_BANS {
            if !bans.iter().any(|b| b == a) {
                bans.push((*a).to_string());
            }
        }
    }
    // 近期正文若已出现否定对照，本轮强制禁止
    let bu_shi_n = recent.matches("不是").count();
    let rhetorical_n = recent.matches("不是").count().saturating_add(
        recent.matches("并非").count(),
    );
    // 粗检「不是…是」：有「不是」且同窗有「，是」或「而是」
    let has_contrast = (recent.contains("不是") || recent.contains("并非"))
        && (recent.contains("，是")
            || recent.contains(",是")
            || recent.contains("而是")
            || recent.contains("，而是"));
    if bu_shi_n >= 1 || rhetorical_n >= 1 || has_contrast {
        bans.push(
            "禁止「不是A，是B / 并非…而是… / 不是那种…是那种…」式否定对照；感官与判断直接正面写"
                .into(),
        );
    }
    for w in ACTION_LEXICON {
        if recent.contains(w) {
            let item = format!("勿再重复动作/收束：「{w}」");
            if !bans.iter().any(|b| b.contains(w)) {
                bans.push(item);
            }
        }
    }
    // 短对白复现（如「张嘴。」出现 ≥2）
    let mut quoted: HashMap<String, usize> = HashMap::new();
    for cap in extract_short_quotes(recent) {
        *quoted.entry(cap).or_insert(0) += 1;
    }
    for (q, n) in quoted {
        if n >= 2 {
            bans.push(format!("勿再写已出现对白：「{q}」"));
        }
    }
    if bans.is_empty() {
        "（暂无；仍禁止复述前文句子）".into()
    } else {
        bans
            .into_iter()
            .take(15)
            .enumerate()
            .map(|(i, b)| format!("{}. {b}", i + 1))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn extract_short_quotes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '「' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '」') {
                let q: String = chars[i + 1..i + 1 + end].iter().collect();
                let n = q.chars().count();
                if (2..=12).contains(&n) {
                    out.push(q);
                }
                i += end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 根据落盘 progress 输出节拍状态（关键词启发式不再单独标 completed）。
pub fn build_beat_status(beats: &[SceneBeat], progress: &ChapterBeatProgress) -> String {
    beat_engine::build_beat_status_lines(beats, progress)
}

/// 兼容旧调用：无 progress 时按 pending/in_progress 推断（仅 beats 为空时回退关键词）
pub fn build_beat_status_legacy(beats: &[SceneBeat], content: &str) -> String {
    if beats.is_empty() {
        return "（无节拍）".into();
    }
    // 无 sidecar 时用关键词仅作提示，不写入 completed
    let mut lines = Vec::new();
    let mut first_pending = true;
    for (i, b) in beats.iter().enumerate() {
        let keys = beat_keys(b);
        let keyword_hit = keys
            .iter()
            .any(|k| !k.is_empty() && content.contains(k.as_str()));
        let status = if keyword_hit && !first_pending {
            "completed"
        } else if first_pending {
            first_pending = false;
            "in_progress"
        } else {
            "pending"
        };
        lines.push(format!(
            "{}. [{}] {} | 目的:{} | 冲突:{} | 情绪:{}",
            i + 1,
            status,
            b.title,
            b.purpose,
            b.conflict,
            b.emotion
        ));
    }
    lines.join("\n")
}

fn beat_keys(b: &SceneBeat) -> Vec<String> {
    let mut keys = Vec::new();
    for s in [&b.title, &b.purpose, &b.conflict, &b.emotion] {
        let t = s.trim();
        if t.chars().count() >= 2 {
            keys.push(t.to_string());
        }
    }
    if let Some(loc) = &b.location {
        let t = loc.trim();
        if t.chars().count() >= 2 {
            keys.push(t.to_string());
        }
    }
    keys
}

/// 叙事方向锚点：告诉模型「下一步必须推进什么」，避免清理变奏。
pub fn build_direction_anchor(
    must_do: &str,
    instruction: &str,
    beat_status: &str,
    ban_list: &str,
    active_beat: Option<&SceneBeat>,
    outline_run: bool,
) -> String {
    let mut parts = Vec::new();
    if let Some(b) = active_beat {
        parts.push(format!(
            "【按纲续写】本轮只写此节拍，写满规定字数后停；禁止写后续节拍：{}",
            beat_engine::beat_summary(b)
        ));
    } else if let Some(pending) = first_in_progress_beat(beat_status) {
        parts.push(format!("当前节拍（仅推进此项，完成后即停）：{pending}"));
    } else if !must_do.trim().is_empty() {
        parts.push(format!("本章必达（择一未兑现项推进）：{must_do}"));
    }
    if !instruction.trim().is_empty() && instruction != "（无额外指令）" {
        if outline_run {
            parts.push(format!("用户微调（不得覆盖当前节拍）：{instruction}"));
        } else {
            parts.push(format!("用户本轮指令优先：{instruction}"));
        }
    }
    parts.push(
        "禁止把篇幅用在已完成的清理/收束变奏上；必须出现新的地点、决定或冲突升级。".into(),
    );
    if ban_list.contains("张嘴") || ban_list.contains("舔") {
        parts.push("若前文已含口侍/清理，本轮禁止再写「张嘴/舔净」收束。".into());
    }
    parts.join("\n")
}

fn first_in_progress_beat(beat_status: &str) -> Option<String> {
    for line in beat_status.lines() {
        if line.contains("[in_progress]") {
            return Some(line.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bans_repeated_quote() {
        let recent = "他抬手。「张嘴。」\n\n她照做。「张嘴。」又来一次。";
        let ban = build_dynamic_ban_list(recent, "");
        assert!(ban.contains("张嘴"));
    }

    #[test]
    fn anatomy_bans_not_tied_to_character_name() {
        let plain = build_dynamic_ban_list("乐乐坐在沙发上喝水。", "");
        assert!(!plain.contains("女角色拥有阴茎"));
        let spicy = build_dynamic_ban_list("她真空穿超短裙，腿心发烫。", "");
        assert!(spicy.contains("女角色拥有阴茎"));
    }
}
