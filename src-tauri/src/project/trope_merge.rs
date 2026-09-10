//! 情节/性癖近义合并：同类补内容，不新开重复条、不覆盖旧正文
//! 代码路径: kk_novel_ai/src-tauri/src/project/trope_merge.rs

use super::{
    is_trope_kind, normalize_lore_title, open_project, read_lore_kind_list, save_project_meta,
    write_lore_kind_list, LoreEntry,
};
use crate::error::AppResult;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const PUNCT: &str = "，。、；：！？「」『』（）【】《》—…·・,.;:!?'\"()[]{}<>/\\|-_~`@#$%^&*+=　";

/// 太泛的词不当「两词拼接=标题」的零件，避免 自慰+高潮 误并到别的卡
const GENERIC_ALIAS: &[&str] = &[
    "高潮", "自慰", "插入", "触碰", "口交", "手指", "身体", "亲吻", "接吻", "射精",
    "湿润", "疼痛", "暴露", "裸体", "交合", "后入", "前戏", "磨蹭", "轮廓",
];

pub fn trope_norm(s: &str) -> String {
    normalize_lore_title(s)
        .chars()
        .filter(|c| !c.is_whitespace() && !PUNCT.contains(*c))
        .collect()
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn char_set(s: &str) -> HashSet<char> {
    s.chars().collect()
}

fn jaccard_chars(a: &str, b: &str) -> f32 {
    let sa = char_set(a);
    let sb = char_set(b);
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f32;
    let union = sa.union(&sb).count() as f32;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// 标题近义：全等、较长包含较短（短边≥3字）、或字集合 Jaccard≥0.55 且交集≥2
pub fn titles_similar(a: &str, b: &str) -> bool {
    let a = trope_norm(a);
    let b = trope_norm(b);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let (short, long) = if char_len(&a) <= char_len(&b) {
        (&a, &b)
    } else {
        (&b, &a)
    };
    if char_len(short) >= 3 && long.contains(short) {
        return true;
    }
    let inter = char_set(&a).intersection(&char_set(&b)).count();
    inter >= 2 && jaccard_chars(&a, &b) >= 0.55
}

fn alias_list(entry: &LoreEntry) -> Vec<String> {
    let mut out = Vec::new();
    let t = trope_norm(&entry.title);
    if !t.is_empty() {
        out.push(t);
    }
    for k in &entry.keywords {
        let n = trope_norm(k);
        if char_len(&n) >= 2 && !out.iter().any(|x| x == &n) {
            out.push(n);
        }
    }
    out
}

fn two_aliases_make_title(aliases: &[String], title: &str) -> bool {
    let t = trope_norm(title);
    if char_len(&t) < 3 {
        return false;
    }
    for i in 0..aliases.len() {
        for j in 0..aliases.len() {
            if i == j {
                continue;
            }
            if GENERIC_ALIAS.contains(&aliases[i].as_str())
                || GENERIC_ALIAS.contains(&aliases[j].as_str())
            {
                continue;
            }
            let cat = format!("{}{}", aliases[i], aliases[j]);
            if cat == t {
                return true;
            }
        }
    }
    false
}

fn distinctive_keyword_overlap(a: &LoreEntry, b: &LoreEntry) -> bool {
    let ka: HashSet<String> = a
        .keywords
        .iter()
        .map(|k| trope_norm(k))
        .filter(|k| char_len(k) >= 2)
        .collect();
    let kb: HashSet<String> = b
        .keywords
        .iter()
        .map(|k| trope_norm(k))
        .filter(|k| char_len(k) >= 2)
        .collect();
    let shared: Vec<&String> = ka.intersection(&kb).collect();
    if shared.is_empty() {
        return false;
    }
    let has_long = shared.iter().any(|k| char_len(k) >= 3);
    has_long && shared.len() >= 2
}

/// 两条是否视为同一情节/性癖（须同 kind）
pub fn tropes_are_similar(a: &LoreEntry, b: &LoreEntry) -> bool {
    if !is_trope_kind(&a.kind) || !is_trope_kind(&b.kind) || a.kind != b.kind {
        return false;
    }
    if !a.id.is_empty() && a.id == b.id {
        return true;
    }
    if titles_similar(&a.title, &b.title) {
        return true;
    }
    let aa = alias_list(a);
    let ba = alias_list(b);
    let tb = trope_norm(&b.title);
    let ta = trope_norm(&a.title);
    if aa.iter().any(|x| titles_similar(x, &tb) || x == &tb) {
        return true;
    }
    if ba.iter().any(|x| titles_similar(x, &ta) || x == &ta) {
        return true;
    }
    if two_aliases_make_title(&aa, &b.title) || two_aliases_make_title(&ba, &a.title) {
        return true;
    }
    distinctive_keyword_overlap(a, b)
}

pub fn find_similar_trope<'a>(
    entries: &'a [LoreEntry],
    title: &str,
    keywords: &[String],
    kind: &str,
) -> Option<&'a LoreEntry> {
    let kind = if kind == "kink" { "kink" } else { "trope" };
    let probe = LoreEntry {
        id: String::new(),
        kind: kind.into(),
        title: title.to_string(),
        content: String::new(),
        keywords: keywords.to_vec(),
        links: vec![],
        attrs: Default::default(),
        sources: vec![],
        unique: true,
        updated_at: String::new(),
    };
    entries.iter().find(|e| tropes_are_similar(e, &probe))
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join("")
}

fn already_has(hay: &str, needle: &str) -> bool {
    let n = needle.trim();
    if n.is_empty() {
        return true;
    }
    hay.contains(n) || collapse_ws(hay).contains(&collapse_ws(n))
}

fn append_unique_block(dst: &mut String, add: &str) {
    let add = add.trim();
    if add.is_empty() || already_has(dst, add) {
        return;
    }
    if dst.trim().is_empty() {
        *dst = add.to_string();
    } else {
        dst.push('\n');
        dst.push_str(add);
    }
}

fn push_keyword(dst: &mut Vec<String>, raw: &str) {
    let t = raw.trim();
    if t.is_empty() {
        return;
    }
    let key = trope_norm(t);
    if key.is_empty() {
        return;
    }
    if dst.iter().any(|x| trope_norm(x) == key) {
        return;
    }
    dst.push(t.to_string());
}

fn merge_attr_append(attrs: &mut std::collections::BTreeMap<String, String>, key: &str, add: Option<&String>) {
    let Some(add) = add.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) else {
        return;
    };
    let prev = attrs.get(key).cloned().unwrap_or_default();
    if already_has(&prev, &add) {
        return;
    }
    if prev.trim().is_empty() {
        attrs.insert(key.into(), add);
    } else {
        attrs.insert(key.into(), format!("{prev}; {add}"));
    }
}

/// 把 add 补进 keep：保留 keep 的 id/标题，只追加未见过的正文/关键词
pub fn merge_trope_lore(keep: &LoreEntry, add: &LoreEntry) -> LoreEntry {
    let mut out = keep.clone();
    out.unique = true;
    append_unique_block(&mut out.content, &add.content);
    for k in add.keywords.iter().chain(std::iter::once(&add.title)) {
        if trope_norm(k) == trope_norm(&out.title) {
            continue;
        }
        push_keyword(&mut out.keywords, k);
    }
    merge_attr_append(&mut out.attrs, "evidence", add.attrs.get("evidence"));
    merge_attr_append(&mut out.attrs, "do", add.attrs.get("do"));
    merge_attr_append(&mut out.attrs, "dont", add.attrs.get("dont"));
    let merged_tags = super::trope_tags::merge_tag_strings(
        out.attrs.get("tags").map(|s| s.as_str()).unwrap_or(""),
        add.attrs.get("tags").map(|s| s.as_str()).unwrap_or(""),
    );
    if merged_tags.is_empty() {
        out.attrs.remove("tags");
    } else {
        out.attrs.insert("tags".into(), merged_tags);
    }
    out
}

fn prefer_keeper<'a>(a: &'a LoreEntry, b: &'a LoreEntry) -> &'a LoreEntry {
    let sa = a.content.chars().count() + a.keywords.len() * 8;
    let sb = b.content.chars().count() + b.keywords.len() * 8;
    if sb > sa {
        b
    } else {
        a
    }
}

/// 压缩仓内近义重复。返回 被删 id → 保留 id。
pub fn compact_similar_tropes(root: &Path) -> AppResult<HashMap<String, String>> {
    let mut id_map = HashMap::new();
    for kind in ["trope", "kink"] {
        let mut items = read_lore_kind_list(root, kind)?;
        if items.len() < 2 {
            continue;
        }
        let mut changed = false;
        loop {
            let mut pair: Option<(usize, usize)> = None;
            'outer: for i in 0..items.len() {
                for j in (i + 1)..items.len() {
                    if tropes_are_similar(&items[i], &items[j]) {
                        pair = Some((i, j));
                        break 'outer;
                    }
                }
            }
            let Some((i, j)) = pair else {
                break;
            };
            let keep_idx = if prefer_keeper(&items[i], &items[j]).id == items[i].id {
                i
            } else {
                j
            };
            let drop_idx = if keep_idx == i { j } else { i };
            let merged = merge_trope_lore(&items[keep_idx], &items[drop_idx]);
            changed = true;
            id_map.insert(items[drop_idx].id.clone(), merged.id.clone());
            let drop_id = items[drop_idx].id.clone();
            let keep_id = items[keep_idx].id.clone();
            let mut next = Vec::with_capacity(items.len() - 1);
            for e in items {
                if e.id == drop_id {
                    continue;
                }
                if e.id == keep_id {
                    next.push(merged.clone());
                } else {
                    next.push(e);
                }
            }
            items = next;
        }
        if changed {
            write_lore_kind_list(root, kind, &items)?;
        }
    }
    Ok(id_map)
}

pub fn remap_chapter_trope_ids(root: &Path, id_map: &HashMap<String, String>) -> AppResult<bool> {
    if id_map.is_empty() {
        return Ok(false);
    }
    let mut opened = open_project(root)?;
    let mut changed = false;
    for ch in &mut opened.project.chapters {
        let mut next = Vec::new();
        for id in &ch.trope_ids {
            let nid = id_map.get(id).unwrap_or(id).clone();
            if !next.iter().any(|x| x == &nid) {
                next.push(nid);
            }
        }
        if next != ch.trope_ids {
            ch.trope_ids = next;
            changed = true;
        }
    }
    if changed {
        save_project_meta(root, &opened.project)?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kink(title: &str, kws: &[&str]) -> LoreEntry {
        LoreEntry {
            id: title.into(),
            kind: "kink".into(),
            title: title.into(),
            content: "old".into(),
            keywords: kws.iter().map(|s| s.to_string()).collect(),
            links: vec![],
            attrs: Default::default(),
            sources: vec![],
            unique: true,
            updated_at: "t".into(),
        }
    }

    #[test]
    fn similar_jaccard_vacuum_outing() {
        assert!(titles_similar("真空出门", "真空短裙出门"));
        assert!(!titles_similar("手交", "口交吞精"));
        assert!(!titles_similar("隔衣触碰", "喉结触碰"));
    }

    #[test]
    fn similar_via_keyword_concat() {
        let existing = kink("裙下暴露", &["短裙", "真空", "不穿内裤", "裙摆"]);
        let draft = kink("真空短裙", &["短裙"]);
        assert!(tropes_are_similar(&existing, &draft));
    }

    #[test]
    fn skirt_outline_not_same_as_upskirt() {
        assert!(!titles_similar("裙下暴露", "裙下轮廓"));
        let a = kink("裙下暴露", &["裙摆", "真空"]);
        let b = kink("裙下轮廓", &["轮廓", "贴身"]);
        assert!(!tropes_are_similar(&a, &b));
    }

    #[test]
    fn does_not_merge_generic_climax_overlap() {
        let a = kink("自慰高潮", &["手指", "高潮", "自慰高潮"]);
        let b = kink("久旷生涩入体", &["干燥", "自慰", "高潮"]);
        assert!(!tropes_are_similar(&a, &b));
    }

    #[test]
    fn merge_appends_and_keeps_title() {
        let keep = kink("裙下暴露", &["裙摆"]);
        let mut add = kink("真空短裙", &["真空"]);
        add.content = "凉风钻入腿心".into();
        let out = merge_trope_lore(&keep, &add);
        assert_eq!(out.title, "裙下暴露");
        assert_eq!(out.id, "裙下暴露");
        assert!(out.content.contains("old"));
        assert!(out.content.contains("凉风钻入腿心"));
        assert!(out.keywords.iter().any(|k| k == "真空"));
        assert!(out.keywords.iter().any(|k| k == "真空短裙"));
        let again = merge_trope_lore(&out, &add);
        assert_eq!(
            again.content.matches("凉风钻入腿心").count(),
            1,
            "must not duplicate content"
        );
    }
}
