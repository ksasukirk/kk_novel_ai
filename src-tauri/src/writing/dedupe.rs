//! 续写输出复读截断
//! 代码路径: kk_novel_ai/src-tauri/src/writing/dedupe.rs

/// 规范化段落，便于判重。
fn norm_para(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
}

fn paras(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn near_dup(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if a.is_empty() || b.is_empty() {
        return false;
    }
    // 短段：包含关系也算复读
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if short.len() >= 24 && long.contains(short) {
        return true;
    }
    // 字符集合 Jaccard（粗）
    let sa: std::collections::HashSet<char> = a.chars().collect();
    let sb: std::collections::HashSet<char> = b.chars().collect();
    let inter = sa.intersection(&sb).count() as f32;
    let uni = sa.union(&sb).count() as f32;
    if uni > 0.0 && inter / uni >= 0.92 && a.len().min(b.len()) >= 40 {
        return true;
    }
    false
}

fn is_short_stub(n: &str) -> bool {
    n.chars().count() < 8
}

/// 是否含有实质段落（非「张嘴。」这类短台词碎屑）
pub fn has_substantial_content(text: &str) -> bool {
    paras(text)
        .iter()
        .any(|p| !is_short_stub(&norm_para(p)))
}

/// 女角色解剖漂移标记（命中则从该段起整段丢弃后续）
const ANATOMY_DRIFT_MARKERS: &[&str] = &[
    "乐乐的阴茎",
    "乐乐阴茎",
    "乐乐的鸡巴",
    "乐乐鸡巴",
    "她的阴茎",
    "她的鸡巴",
    "乐乐在他嘴里持续射精",
    "乐乐的阴茎在他嘴里",
    "她的阴茎反复",
    "从她握着的阴茎",
    "她握住根部，手指收紧", // 与「乐乐射进kk嘴」连用时的典型漂移句；见 strip 二次校验
];

fn paragraph_has_anatomy_drift(p: &str) -> bool {
    let hard = [
        "乐乐的阴茎",
        "乐乐阴茎",
        "乐乐的鸡巴",
        "乐乐鸡巴",
        "她的阴茎",
        "她的鸡巴",
        "乐乐在他嘴里持续射精",
        "乐乐的阴茎在他嘴里",
        "她的阴茎反复",
        "从她握着的阴茎",
    ];
    if hard.iter().any(|m| p.contains(m)) {
        return true;
    }
    // 「她射了」+「精液」+「他嘴里」且同段无「kk的阴茎」→ 可疑漂移
    if p.contains("精液")
        && (p.contains("他嘴里") || p.contains("kk嘴里") || p.contains("灌进kk"))
        && (p.contains("她射") || p.contains("乐乐射"))
        && !p.contains("kk的阴茎")
        && !p.contains("他的阴茎")
    {
        return true;
    }
    let _ = ANATOMY_DRIFT_MARKERS; // 文档锚点；实际用 hard 列表
    false
}

/// 从首个解剖漂移段起截断。
pub fn strip_anatomy_drift(generated: &str) -> (String, bool) {
    let raw_paras = paras(generated);
    if raw_paras.is_empty() {
        return (generated.trim().to_string(), false);
    }
    let mut kept: Vec<String> = Vec::new();
    let mut hit = false;
    for p in raw_paras {
        if paragraph_has_anatomy_drift(&p) {
            hit = true;
            break;
        }
        kept.push(p);
    }
    if !hit {
        return (generated.trim().to_string(), false);
    }
    (kept.join("\n\n"), true)
}

/// 截断生成文本内部的段落循环；可选剔除与既有正文重复的段。
pub fn sanitize_generation(generated: &str, existing: Option<&str>) -> (String, bool) {
    let (after_anatomy, anatomy_hit) = strip_anatomy_drift(generated);
    let raw_paras = paras(&after_anatomy);
    if raw_paras.is_empty() {
        return (after_anatomy.trim().to_string(), anatomy_hit);
    }

    let existing_norm: Vec<String> = existing
        .map(|ex| paras(ex).into_iter().map(|p| norm_para(&p)).collect())
        .unwrap_or_default();

    let mut kept: Vec<String> = Vec::new();
    let mut kept_norm: Vec<String> = Vec::new();
    let mut truncated = anatomy_hit;
    let raw_chars: usize = raw_paras.iter().map(|p| p.chars().count()).sum();

    for p in raw_paras {
        let n = norm_para(&p);

        // 短台词：与既有/已保留完全相同则丢弃，避免「张嘴。」之类漏网
        if is_short_stub(&n) {
            if existing_norm.iter().any(|e| e == &n || (n.chars().count() >= 2 && e.contains(&n)))
                || kept_norm.iter().any(|e| e == &n)
            {
                truncated = true;
                continue;
            }
            kept.push(p);
            kept_norm.push(n);
            continue;
        }

        // 与既有正文重复 → 跳过
        if existing_norm.iter().any(|e| near_dup(e, &n)) {
            truncated = true;
            continue;
        }
        // 与已保留段近重复 → 截断循环（不再接收后续）
        if kept_norm.iter().any(|e| near_dup(e, &n)) {
            truncated = true;
            break;
        }
        kept.push(p);
        kept_norm.push(n);
    }

    // 丢掉末尾明显截断半句（以逗号/顿号收尾且很短）
    if let Some(last) = kept.last() {
        let t = last.trim_end();
        if t.ends_with('，') || t.ends_with('、') || t.ends_with(',') {
            if t.chars().count() < 40 {
                kept.pop();
                truncated = true;
            }
        }
    }

    let out = kept.join("\n\n");

    // 长文被裁到只剩短碎屑 → 视为无效结果（交由上层重试 / 空预览），勿把「张嘴。」当正文
    if truncated && raw_chars >= 80 && !has_substantial_content(&out) {
        return (String::new(), true);
    }

    // 与既有高度重叠：保留不足原文 15% 且不足 80 字 → 同样作废
    if truncated && raw_chars >= 200 {
        let kept_chars = out.chars().count();
        if kept_chars < 80 && (kept_chars as f32) / (raw_chars as f32) < 0.15 {
            return (String::new(), true);
        }
    }

    (out, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_exact_loop() {
        let g = "第一段内容足够长用来测试啊啊啊。\n\n第二段也不短用来区分一下。\n\n第一段内容足够长用来测试啊啊啊。\n\n第二段也不短用来区分一下。";
        let (out, truncated) = sanitize_generation(g, None);
        assert!(truncated);
        assert!(!out.contains("第一段内容足够长用来测试啊啊啊。\n\n第二段也不短用来区分一下。\n\n第一段"));
        assert!(out.starts_with("第一段"));
    }

    #[test]
    fn drops_existing() {
        let existing = "开场已经写过了这段话要够长才行啊。";
        let g = "开场已经写过了这段话要够长才行啊。\n\n全新的后续段落也要足够长才不会被跳过。";
        let (out, truncated) = sanitize_generation(g, Some(existing));
        assert!(truncated);
        assert!(out.contains("全新的后续"));
        assert!(!out.contains("开场已经写过"));
    }

    #[test]
    fn drops_short_stub_already_in_existing() {
        let existing = "kk 蹲下来，抬头看她：「张嘴。」\n\n娜娜还没从刚才的余韵里缓过来。";
        let g = "kk 把手指举到她面前，液体还挂着。\n\n「张嘴。」\n\n「张嘴。」\n\n娜娜又把同一段清理演了一遍，舌尖卷过指腹把尿和精舔净，动作几乎和前文一样长。";
        // 后段若与既有近重复会被踢；短「张嘴。」也应被踢，不能只剩它
        let existing2 = format!(
            "{existing}\n\n娜娜又把同一段清理演了一遍，舌尖卷过指腹把尿和精舔净，动作几乎和前文一样长。"
        );
        let (out, truncated) = sanitize_generation(g, Some(&existing2));
        assert!(truncated);
        assert!(!out.contains("张嘴"));
        assert!(out.is_empty() || has_substantial_content(&out));
    }

    #[test]
    fn strips_female_penis_drift() {
        let g = "乐乐背靠货架，超短裙卷在腰上。\n\n她把裙摆抚平，耳尖发红。\n\n乐乐的阴茎在他嘴里持续射精，精液一股一股灌进去。\n\n然后她拉开门走了。";
        let (out, truncated) = sanitize_generation(g, None);
        assert!(truncated);
        assert!(out.contains("裙摆抚平"));
        assert!(!out.contains("乐乐的阴茎"));
        assert!(!out.contains("拉开门"));
    }
}
