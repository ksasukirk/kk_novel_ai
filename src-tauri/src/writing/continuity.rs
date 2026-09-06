//! 跨章衔接：尾段切边界、章摘要拒复读、时间/亲属/乐乐口吻锁
//! 代码路径: kk_novel_ai/src-tauri/src/writing/continuity.rs

use crate::project::LoreEntry;

/// 章摘要允许上限（超出视为把正文当摘要交回）
pub const CHAPTER_SUMMARY_MAX_CHARS: usize = 400;
/// 章摘要建议下限（过短仍可用块笔记兜底）
#[allow(dead_code)]
pub const CHAPTER_SUMMARY_MIN_CHARS: usize = 80;

/// 取文本尾部，并回退到段/句边界，避免「心的笑意」这种半句开头。
pub fn take_tail_at_boundary(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n == 0 {
        return String::new();
    }
    if n <= max_chars {
        return text.trim_start().to_string();
    }
    let hard_start = n - max_chars;
    let lookback = 240.min(hard_start);
    let search_from = hard_start - lookback;
    let mut snap = hard_start;
    for i in search_from..hard_start {
        match chars[i] {
            '\n' | '。' | '！' | '？' | '；' | '”' | '"' => snap = i + 1,
            _ => {}
        }
    }
    while snap < n && chars[snap].is_whitespace() {
        snap += 1;
    }
    if snap == hard_start && snap < n {
        let fwd_lim = (hard_start + 80).min(n);
        for i in hard_start..fwd_lim {
            match chars[i] {
                '\n' | '。' | '！' | '？' | '；' => {
                    snap = i + 1;
                    while snap < n && chars[snap].is_whitespace() {
                        snap += 1;
                    }
                    break;
                }
                _ => {}
            }
        }
    }
    chars[snap..].iter().collect()
}

fn compact_prefix(s: &str, n: usize) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .take(n)
        .collect()
}

/// 模型把整章正文当「摘要」交回，或明显超长。
pub fn chapter_summary_is_dump(summary: &str, source: &str) -> bool {
    let sum = summary.trim();
    let n = sum.chars().count();
    if n > CHAPTER_SUMMARY_MAX_CHARS {
        return true;
    }
    let src = source.trim();
    if src.is_empty() || n < 24 {
        return false;
    }
    let a = compact_prefix(sum, 48);
    let b = compact_prefix(src, 48);
    if a.chars().count() >= 24 && a == b {
        return true;
    }
    let src_head: String = src.chars().filter(|c| !c.is_whitespace()).take(36).collect();
    if src_head.chars().count() >= 28 && compact_prefix(sum, 200).contains(&src_head) {
        return true;
    }
    false
}

/// 旧版超长总结被丢弃后写入 memory 的占位；禁止再进入 rolling_summary。
pub const DUMP_PLACEHOLDER: &str =
    "（写后总结过长或复读正文，已丢弃。下一章以正文时间地点为准，勿回拨用餐或改亲属。）";

pub fn is_dump_placeholder(s: &str) -> bool {
    s.contains("写后总结过长或复读正文") || s.trim() == DUMP_PLACEHOLDER.trim()
}

fn is_terminal_punct(c: char) -> bool {
    matches!(
        c,
        '。' | '！' | '？' | '…' | '」' | '』' | '"' | '”' | '）' | ')' | '.' | '!' | '?'
    )
}

/// 正文是否停在半句（不以句末标点收束）。
pub fn prose_incomplete(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    match t.chars().rev().find(|c| !c.is_whitespace()) {
        Some(c) => !is_terminal_punct(c),
        None => false,
    }
}

fn last_sentence(s: &str) -> String {
    let chars: Vec<char> = s.trim().chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut start = 0;
    for (i, c) in chars.iter().enumerate() {
        if matches!(c, '。' | '！' | '？' | '\n') && i + 1 < chars.len() {
            start = i + 1;
        }
    }
    chars[start..].iter().collect::<String>().trim().to_string()
}

fn compact_no_punct(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !c.is_whitespace()
                && !matches!(
                    c,
                    '，' | '。' | '、' | '；' | '：' | ',' | '.' | '！' | '？' | '!' | '?' | '…'
                )
        })
        .collect()
}

fn compact_tail_chars(s: &str, n: usize) -> String {
    let compact = compact_no_punct(s);
    let chars: Vec<char> = compact.chars().collect();
    if chars.len() <= n {
        return compact;
    }
    chars[chars.len() - n..].iter().collect()
}

/// 按纲收束是否尚未出现在正文后部。无法判断时返回 false（不要死循环补写）。
pub fn outline_hook_missing(body: &str, outline: &str) -> bool {
    let outline = outline.trim();
    let body = body.trim();
    if outline.is_empty() || body.is_empty() {
        return false;
    }
    let last = last_sentence(outline);
    let needle = compact_tail_chars(&last, 16);
    if needle.chars().count() < 8 {
        return false;
    }
    let body_tail: String = body
        .chars()
        .rev()
        .take(1600)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let compact_body = compact_no_punct(&body_tail);
    !compact_body.contains(&needle)
}

/// 按句切到 120～250 字，供超长/复读摘要回收。
pub fn compress_chapter_summary(text: &str) -> String {
    let t = text.trim();
    if t.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = t.chars().collect();
    let n = chars.len();
    if n <= 250 {
        return t.to_string();
    }
    let min = 120.min(n);
    let max = 250.min(n);
    let mut cut = max;
    for i in (min..max).rev() {
        if matches!(chars[i], '。' | '！' | '？' | '\n' | '.' | '!' | '?') {
            cut = i + 1;
            break;
        }
    }
    chars[..cut].iter().collect()
}

/// 摘要不合格时：块笔记优先，否则把超长/复读稿压到 120～250 字；禁止再写占位句。
pub fn fallback_chapter_summary(rejected: &str, source: &str, block_note: &str) -> String {
    let note = block_note.trim();
    if !note.is_empty() && !is_dump_placeholder(note) && !chapter_summary_is_dump(note, source) {
        return compress_chapter_summary(note);
    }
    if !note.is_empty() && !is_dump_placeholder(note) {
        return compress_chapter_summary(note);
    }
    if !rejected.trim().is_empty() && !is_dump_placeholder(rejected) {
        return compress_chapter_summary(rejected);
    }
    if !source.trim().is_empty() {
        return compress_chapter_summary(source);
    }
    String::new()
}

/// 上章已用餐/饭后时，禁止本章写成还没开饭。
pub fn meal_time_lock(prev_bridge: &str) -> String {
    let blob = prev_bridge;
    let mut rules: Vec<&str> = Vec::new();
    let ate = blob.contains("添饭")
        || blob.contains("开饭")
        || blob.contains("入席")
        || blob.contains("夹菜")
        || blob.contains("扒了")
        || blob.contains("长桌")
        || blob.contains("正餐")
        || blob.contains("晚饭")
        || blob.contains("吃饭");
    let dessert = blob.contains("西瓜")
        || blob.contains("饭后")
        || blob.contains("水果")
        || blob.contains("甜点");
    if ate || dessert {
        rules.push(
            "时间锁：上章已出现用餐或饭后水果。本章禁止写成「还没开饭 / 该吃饭了 / 舅舅喊入席 / 重摆圆桌开饭」。须接上章的时刻继续（饭中就饭中，饭后就饭后）。",
        );
    }
    if dessert {
        rules.push("时间锁：上章已到西瓜/饭后，本章按饭后写，禁止把正餐再开一顿。");
    }
    rules.join("\n")
}

/// 乐乐出场时的口吻与穿着锁（成年；讨厌但接受；禁止直球）。
pub fn lele_voice_lock(blob: &str) -> String {
    if !blob.contains("乐乐") {
        return String::new();
    }
    [
        "乐乐锁（不可违背）：乐乐是kk的成年表妹。对kk态度是讨厌但接受——嘴上嫌麻烦、别扭、爱损，行为上留下、不真推开。",
        "对白含蓄拐弯，禁止她直球说「我喜欢你」、禁止满口黄腔求色；内心可以乱，嘴上要把话咽回去或说半句。",
        "禁止写成单纯娇羞深情小作精，也禁止写成决裂仇视。",
        "穿着：默认超短裙、裙下真空，用弯腰/落座/风吹带出，禁止每段复读设定口号。",
        "憋尿：可用小腹发紧、夹腿等行为带出，禁止每段机械催「来接」。",
        "女体：禁止给乐乐写阴茎/射精。",
    ]
    .join("\n")
}

/// 亲属称谓：禁止把表哥表妹的父母对调。
pub fn kinship_lock(lore: &[&LoreEntry], blob: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for e in lore {
        let kind = e.kind.as_str();
        if !(kind.is_empty() || kind == "character") {
            continue;
        }
        let t = e.title.trim();
        let c = e.content.as_str();
        if t.is_empty() {
            continue;
        }
        if c.contains("二舅") || c.contains("姨妈") || c.contains("表妹") || c.contains("表哥")
        {
            lines.push(format!(
                "亲属锁：{t} 的身份以设定卡为准（{snippet}）。禁止把「谁是谁家的孩子」对调，禁止把表妹写成二舅家闺女若设定写明另门。",
                snippet = c.chars().take(48).collect::<String>()
            ));
        }
    }
    if blob.contains("二舅家的孩子") && blob.contains("表妹") {
        lines.push(
            "亲属锁：若上章写明 kk 是二舅家的孩子，则乐乐不得写成同一门的闺女；须是另一门（如姨妈家）的表妹。"
                .into(),
        );
    }
    lines.join("\n")
}

pub fn append_continuity_locks(
    gender_lock: &str,
    book_outline: &str,
    lore: &[&LoreEntry],
    prev_bridge: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let g = gender_lock.trim();
    if !g.is_empty() && g != "（无）" {
        parts.push(g.to_string());
    }
    let blob = format!("{book_outline}\n{prev_bridge}");
    let meal = meal_time_lock(prev_bridge);
    if !meal.is_empty() {
        parts.push(meal);
    }
    let kin = kinship_lock(lore, &blob);
    if !kin.is_empty() {
        parts.push(kin);
    }
    let lele = lele_voice_lock(&format!("{blob}\n{}", lore.iter().map(|e| e.title.as_str()).collect::<Vec<_>>().join(" ")));
    if !lele.is_empty() {
        parts.push(lele);
    }
    parts.push(
        "跨章禁止回拨：上章已发生的用餐、入睡、出行、表白进度，本章不得写成尚未发生；地点与在场人物须接上章收束。"
            .into(),
    );
    if parts.is_empty() {
        "（无额外连续锁）".into()
    } else {
        parts.join("\n")
    }
}

/// 大纲/章纲点名的角色必须进提示，避免检索漏掉本篇卡。
pub fn force_include_named_lore<'a>(
    lore: &mut Vec<&'a LoreEntry>,
    all_lore: &'a [LoreEntry],
    query: &str,
    cap: usize,
) {
    for e in all_lore {
        let kind = e.kind.as_str();
        if !(kind.is_empty() || kind == "character") {
            continue;
        }
        let title = e.title.trim();
        if title.chars().count() < 2 {
            continue;
        }
        let short = title.rsplit(']').next().unwrap_or(title).trim();
        if !query.contains(title) && (short.is_empty() || !query.contains(short)) {
            continue;
        }
        if lore
            .iter()
            .any(|x| x.id == e.id || x.title.trim() == title)
        {
            continue;
        }
        lore.push(e);
    }
    if lore.len() > cap {
        lore.truncate(cap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_snaps_to_sentence() {
        let text = "第一句结束。带着点关心的笑意，像一阵风。她逃进里屋。念头扎了根。";
        let tail = take_tail_at_boundary(text, 21);
        assert!(
            !tail.starts_with("心的笑意") && !tail.starts_with("的笑意"),
            "{tail}"
        );
        assert!(tail.contains("扎了根"), "{tail}");
    }

    #[test]
    fn dump_detects_verbatim_chapter() {
        let src = "西瓜是表姐切好端出来的，红瓤沙甜，码在搪瓷盘里。乐乐接过一块。";
        assert!(chapter_summary_is_dump(src, src));
        let ok = "饭后院子里吃西瓜，乐乐试探kk有没有对象，kk说工作未稳。钩子：她想今晚问清楚。";
        assert!(!chapter_summary_is_dump(ok, src));
    }

    #[test]
    fn dump_detects_overlong() {
        let src = "短正文。";
        let long: String = "啊".repeat(401);
        assert!(chapter_summary_is_dump(&long, src));
    }

    #[test]
    fn fallback_prefers_block_note() {
        let src = "西瓜是表姐切好端出来的，红瓤沙甜，码在搪瓷盘里。乐乐接过一块。";
        let note = "饭后院子独处，乐乐问对象，kk暧昧，约去堂屋。";
        let out = fallback_chapter_summary(src, src, note);
        assert_eq!(out, note);
    }

    #[test]
    fn fallback_compresses_instead_of_placeholder() {
        let src = "西瓜是表姐切好端出来的，红瓤沙甜，码在搪瓷盘里。乐乐接过一块。";
        let long: String = "饭后院子吃西瓜。".to_string() + &"啊".repeat(400);
        let out = fallback_chapter_summary(&long, src, "");
        assert!(!is_dump_placeholder(&out));
        let n = out.chars().count();
        assert!(n >= 120 && n <= 250, "got {n}: {out}");
    }

    #[test]
    fn prose_incomplete_detects_mid_sentence() {
        assert!(prose_incomplete("妈妈的臀部被他撞得"));
        assert!(!prose_incomplete("妈妈令其去洗澡。"));
        assert!(!prose_incomplete(""));
    }

    #[test]
    fn dump_placeholder_detected() {
        assert!(is_dump_placeholder(DUMP_PLACEHOLDER));
        assert!(!is_dump_placeholder("第三天早晨客厅鞋跟插入，结束后去洗澡。"));
    }

    #[test]
    fn outline_hook_missing_uses_last_sentence() {
        let outline = "早饭后弹击扇打。射精后妈妈令其洗澡，答明天再说。";
        let early = "第四天早上kk醒来吃吐司，妈妈开始弹击。";
        assert!(outline_hook_missing(early, outline));
        let done = "抽插渐快。射精后妈妈令其洗澡，答明天再说。kk去浴室。";
        assert!(!outline_hook_missing(done, outline));
    }

    #[test]
    fn meal_lock_fires_on_dinner() {
        let lock = meal_time_lock("上章摘要：长桌晚饭，kk添饭，后来吃西瓜。");
        assert!(lock.contains("禁止写成"));
        assert!(lock.contains("西瓜"));
    }

    #[test]
    fn lele_lock_only_when_named() {
        assert!(lele_voice_lock("娜娜在场").is_empty());
        assert!(lele_voice_lock("乐乐想成为").contains("讨厌但接受"));
    }
}
