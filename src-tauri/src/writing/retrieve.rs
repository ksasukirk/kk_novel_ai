//! ????? + ??????
//! ????: kk_novel_ai/src-tauri/src/writing/retrieve.rs

use crate::project::LoreEntry;
use std::collections::HashMap;

fn keyword_score(entry: &LoreEntry, query_lower: &str) -> f32 {
    let mut score = 0f32;
    let title = entry.title.to_lowercase();
    let content = entry.content.to_lowercase();
    if !title.is_empty() && query_lower.contains(&title) {
        score += 10.0;
    }
    for kw in &entry.keywords {
        let k = kw.to_lowercase();
        if !k.is_empty() && query_lower.contains(&k) {
            score += 5.0;
        }
        if !k.is_empty() && content.contains(&k) {
            score += 1.0;
        }
    }
    for (k, v) in &entry.attrs {
        let blob = format!("{k} {v}").to_lowercase();
        for token in blob.split_whitespace() {
            if token.len() > 1 && query_lower.contains(token) {
                score += 2.0;
            }
        }
    }
    for ch in title.chars().filter(|c| !c.is_whitespace()) {
        if query_lower.contains(ch) {
            score += 1.0;
        }
    }
    score
}

/// ?????????????
pub fn retrieve_lore<'a>(entries: &'a [LoreEntry], query: &str, top_k: usize) -> Vec<&'a LoreEntry> {
    retrieve_lore_hybrid(entries, query, top_k, None)
}

/// ??? * w1 + cosine * w2?????? top_k
pub fn retrieve_lore_hybrid<'a>(
    entries: &'a [LoreEntry],
    query: &str,
    top_k: usize,
    semantic_scores: Option<&HashMap<String, f32>>,
) -> Vec<&'a LoreEntry> {
    const W_KW: f32 = 1.0;
    const W_SEM: f32 = 12.0;

    let q = query.to_lowercase();
    let mut scored: Vec<(f32, &LoreEntry)> = entries
        .iter()
        .map(|e| {
            let kw = keyword_score(e, &q);
            let sem = semantic_scores
                .and_then(|m| m.get(&e.id).copied())
                .unwrap_or(0.0);
            (kw * W_KW + sem * W_SEM, e)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<&LoreEntry> = scored
        .into_iter()
        .filter(|(s, _)| *s > 0.0)
        .take(top_k)
        .map(|(_, e)| e)
        .collect();
    if out.is_empty() {
        out = entries.iter().take(top_k.min(3)).collect();
    }
    out
}
