//! 节拍进度状态机：落盘 progress 驱动续写推进
//! 代码路径: kk_novel_ai/src-tauri/src/writing/beat_engine.rs

use crate::project::{ChapterBeatProgress, SceneBeat};
use chrono::Utc;

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_IN_PROGRESS: &str = "in_progress";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_SKIPPED: &str = "skipped";

fn now() -> String {
    Utc::now().to_rfc3339()
}

/// 从 beats 初始化进度：第一个 in_progress，其余 pending
pub fn init_progress_from_beats(beats: &[SceneBeat]) -> ChapterBeatProgress {
    let mut progress = ChapterBeatProgress::default();
    progress.updated_at = now();
    let mut first = true;
    for b in beats {
        if b.title.trim().is_empty() && b.purpose.trim().is_empty() {
            continue;
        }
        let status = if first {
            first = false;
            STATUS_IN_PROGRESS
        } else {
            STATUS_PENDING
        };
        progress.beats.insert(b.id.clone(), status.to_string());
        if progress.current_beat_id.is_empty() && status == STATUS_IN_PROGRESS {
            progress.current_beat_id = b.id.clone();
        }
    }
    progress
}

/// 将 progress 与当前 beats 列表对齐（新增 pending、移除已删 beat）
pub fn sync_progress_with_beats(beats: &[SceneBeat], progress: &mut ChapterBeatProgress) {
    let valid_ids: std::collections::HashSet<String> =
        beats.iter().map(|b| b.id.clone()).collect();
    progress.beats.retain(|id, _| valid_ids.contains(id));
    let mut has_in_progress = progress
        .beats
        .values()
        .any(|s| s == STATUS_IN_PROGRESS);
    for b in beats {
        if !progress.beats.contains_key(&b.id) {
            let status = if !has_in_progress {
                has_in_progress = true;
                progress.current_beat_id = b.id.clone();
                STATUS_IN_PROGRESS
            } else {
                STATUS_PENDING
            };
            progress.beats.insert(b.id.clone(), status.to_string());
        }
    }
    if progress.current_beat_id.is_empty()
        || !progress.beats.contains_key(&progress.current_beat_id)
    {
        if let Some(b) = active_beat(beats, progress) {
            progress.current_beat_id = b.id.clone();
        }
    }
}

pub fn load_or_init_progress(
    beats: &[SceneBeat],
    stored: ChapterBeatProgress,
) -> ChapterBeatProgress {
    if beats.is_empty() {
        return ChapterBeatProgress::default();
    }
    if stored.beats.is_empty() {
        return init_progress_from_beats(beats);
    }
    let mut progress = stored;
    sync_progress_with_beats(beats, &mut progress);
    progress
}

pub fn beat_status_for(progress: &ChapterBeatProgress, beat_id: &str) -> String {
    progress
        .beats
        .get(beat_id)
        .cloned()
        .unwrap_or_else(|| STATUS_PENDING.to_string())
}

pub fn active_beat<'a>(
    beats: &'a [SceneBeat],
    progress: &ChapterBeatProgress,
) -> Option<&'a SceneBeat> {
    if !progress.current_beat_id.is_empty() {
        if let Some(b) = beats.iter().find(|b| b.id == progress.current_beat_id) {
            if beat_status_for(progress, &b.id) == STATUS_IN_PROGRESS {
                return Some(b);
            }
        }
    }
    for b in beats {
        if beat_status_for(progress, &b.id) == STATUS_IN_PROGRESS {
            return Some(b);
        }
    }
    for b in beats {
        if beat_status_for(progress, &b.id) == STATUS_PENDING {
            return Some(b);
        }
    }
    None
}

/// 强制指定 active_beat_id 为 in_progress（按纲续写队列注入）
pub fn ensure_active_beat(
    beats: &[SceneBeat],
    progress: &mut ChapterBeatProgress,
    beat_id: &str,
) {
    if !beats.iter().any(|b| b.id == beat_id) {
        return;
    }
    for (id, status) in progress.beats.iter_mut() {
        if id == beat_id {
            *status = STATUS_IN_PROGRESS.to_string();
        } else if status.as_str() == STATUS_IN_PROGRESS {
            *status = STATUS_PENDING.to_string();
        }
    }
    if !progress.beats.contains_key(beat_id) {
        progress
            .beats
            .insert(beat_id.to_string(), STATUS_IN_PROGRESS.to_string());
    }
    progress.current_beat_id = beat_id.to_string();
}

/// 标记节拍完成并推进下一 pending
pub fn mark_completed(
    progress: &mut ChapterBeatProgress,
    beats: &[SceneBeat],
    beat_id: &str,
) -> bool {
    if !progress.beats.contains_key(beat_id) {
        return false;
    }
    progress
        .beats
        .insert(beat_id.to_string(), STATUS_COMPLETED.to_string());
    progress.current_beat_id.clear();
    for b in beats {
        if beat_status_for(progress, &b.id) == STATUS_PENDING {
            progress
                .beats
                .insert(b.id.clone(), STATUS_IN_PROGRESS.to_string());
            progress.current_beat_id = b.id.clone();
            return true;
        }
    }
    true
}

pub fn mark_skipped(
    progress: &mut ChapterBeatProgress,
    beats: &[SceneBeat],
    beat_id: &str,
) -> bool {
    if !progress.beats.contains_key(beat_id) {
        return false;
    }
    progress
        .beats
        .insert(beat_id.to_string(), STATUS_SKIPPED.to_string());
    if progress.current_beat_id == beat_id {
        progress.current_beat_id.clear();
        for b in beats {
            if beat_status_for(progress, &b.id) == STATUS_PENDING {
                progress
                    .beats
                    .insert(b.id.clone(), STATUS_IN_PROGRESS.to_string());
                progress.current_beat_id = b.id.clone();
                break;
            }
        }
    }
    true
}

pub fn is_chapter_outline_done(
    beats: &[SceneBeat],
    progress: &ChapterBeatProgress,
    must_do: &str,
) -> bool {
    if !beats.is_empty() {
        return beats.iter().all(|b| {
            let st = beat_status_for(progress, &b.id);
            st == STATUS_COMPLETED || st == STATUS_SKIPPED
        });
    }
    must_do.trim().is_empty()
}

pub fn build_beat_status_lines(beats: &[SceneBeat], progress: &ChapterBeatProgress) -> String {
    if beats.is_empty() {
        return "（无节拍）".into();
    }
    beats
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let status = beat_status_for(progress, &b.id);
            format!(
                "{}. [{}] {} | 目的:{} | 冲突:{} | 情绪:{}",
                i + 1,
                status,
                b.title,
                b.purpose,
                b.conflict,
                b.emotion
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn beat_summary(b: &SceneBeat) -> String {
    format!(
        "{} | 目的:{} | 冲突:{} | 情绪:{} | 地点:{}",
        b.title,
        b.purpose,
        b.conflict,
        b.emotion,
        b.location.as_deref().unwrap_or("")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn mk_beat(title: &str) -> SceneBeat {
        SceneBeat {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            purpose: format!("p-{title}"),
            conflict: String::new(),
            emotion: String::new(),
            location: None,
        }
    }

    #[test]
    fn init_first_in_progress() {
        let beats = vec![mk_beat("a"), mk_beat("b")];
        let p = init_progress_from_beats(&beats);
        assert_eq!(beat_status_for(&p, &beats[0].id), STATUS_IN_PROGRESS);
        assert_eq!(beat_status_for(&p, &beats[1].id), STATUS_PENDING);
    }

    #[test]
    fn mark_completed_advances() {
        let beats = vec![mk_beat("a"), mk_beat("b")];
        let mut p = init_progress_from_beats(&beats);
        mark_completed(&mut p, &beats, &beats[0].id);
        assert_eq!(beat_status_for(&p, &beats[0].id), STATUS_COMPLETED);
        assert_eq!(beat_status_for(&p, &beats[1].id), STATUS_IN_PROGRESS);
        assert_eq!(p.current_beat_id, beats[1].id);
    }

    #[test]
    fn chapter_done_when_all_completed() {
        let beats = vec![mk_beat("a"), mk_beat("b")];
        let mut p = init_progress_from_beats(&beats);
        mark_completed(&mut p, &beats, &beats[0].id);
        assert!(!is_chapter_outline_done(&beats, &p, ""));
        mark_completed(&mut p, &beats, &beats[1].id);
        assert!(is_chapter_outline_done(&beats, &p, ""));
    }
}
