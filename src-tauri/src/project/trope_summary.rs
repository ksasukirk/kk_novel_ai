//! 作品情节/性癖总结状态：正文指纹 + dirty 标记
//! 代码路径: kk_novel_ai/src-tauri/src/project/trope_summary.rs

use super::{
    chapters_dir, count_non_ws, now, open_project, save_project_meta, should_skip_scan_dir,
    NovelProject,
};
use crate::error::AppResult;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

fn has_summary(project: &NovelProject) -> bool {
    project
        .trope_summary_at
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

pub fn mark_trope_summary_dirty(project: &mut NovelProject) {
    if has_summary(project) {
        project.trope_summary_dirty = true;
    }
}

fn chapter_body(root: &Path, file: &str) -> String {
    let path = chapters_dir(root).join(file);
    if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    }
}

pub fn content_fingerprint(root: &Path, project: &NovelProject) -> String {
    let mut hasher = Sha256::new();
    for ch in &project.chapters {
        hasher.update(ch.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(chapter_body(root, &ch.file).as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

pub fn has_chapter_prose(root: &Path, project: &NovelProject) -> bool {
    project.chapters.iter().any(|ch| {
        count_non_ws(&chapter_body(root, &ch.file)) > 0
    })
}

/// 扫描成功后盖章。无正文不盖。返回是否写入。
pub fn stamp_trope_summary(root: &Path) -> AppResult<bool> {
    let mut opened = open_project(root)?;
    if opened.project.kind != "novel" {
        return Ok(false);
    }
    if !has_chapter_prose(root, &opened.project) {
        return Ok(false);
    }
    let fp = content_fingerprint(root, &opened.project);
    opened.project.trope_summary_at = Some(now());
    opened.project.trope_summary_fingerprint = Some(fp);
    opened.project.trope_summary_dirty = false;
    save_project_meta(root, &opened.project)?;
    Ok(true)
}

fn skip_root(root: &Path) -> bool {
    if !root.exists() {
        return true;
    }
    root.file_name()
        .and_then(|s| s.to_str())
        .map(should_skip_scan_dir)
        .unwrap_or(false)
}

fn status_for_project(root: &Path) -> AppResult<Value> {
    let mut opened = open_project(root)?;
    if opened.project.kind != "novel" {
        return Ok(json!({
            "root": root.to_string_lossy(),
            "status": "skip",
            "summary_at": opened.project.trope_summary_at,
            "dirty": opened.project.trope_summary_dirty,
        }));
    }
    if !has_summary(&opened.project) {
        return Ok(json!({
            "root": root.to_string_lossy(),
            "status": "none",
            "summary_at": serde_json::Value::Null,
            "dirty": false,
        }));
    }
    let live = content_fingerprint(root, &opened.project);
    let stored = opened
        .project
        .trope_summary_fingerprint
        .clone()
        .unwrap_or_default();
    let drifted = live != stored;
    if drifted && !opened.project.trope_summary_dirty {
        opened.project.trope_summary_dirty = true;
        save_project_meta(root, &opened.project)?;
    }
    let stale = opened.project.trope_summary_dirty || drifted;
    Ok(json!({
        "root": root.to_string_lossy(),
        "status": if stale { "stale" } else { "current" },
        "summary_at": opened.project.trope_summary_at,
        "dirty": opened.project.trope_summary_dirty,
    }))
}

pub fn trope_summary_status_list(roots: &[String]) -> Value {
    let mut items = Vec::new();
    for raw in roots {
        let root = Path::new(raw.trim());
        if raw.trim().is_empty() || skip_root(root) {
            continue;
        }
        match status_for_project(root) {
            Ok(v) => items.push(v),
            Err(_) => items.push(json!({
                "root": raw,
                "status": "none",
                "summary_at": serde_json::Value::Null,
                "dirty": false,
            })),
        }
    }
    json!({ "ok": true, "items": items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{
        create_chapter, delete_chapter, save_project_meta, write_chapter, ChapterMeta, NovelProject,
        VolumeMeta,
    };
    use uuid::Uuid;

    fn tmp_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("kk_novel_trope_summary_test")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("chapters")).unwrap();
        dir
    }

    fn seed_novel(root: &Path, body: &str) -> String {
        let ch_id = Uuid::new_v4().to_string();
        let file = "0001-第一章.md".to_string();
        fs::write(chapters_dir(root).join(&file), body).unwrap();
        let project = NovelProject {
            id: Uuid::new_v4().to_string(),
            title: "t".into(),
            kind: "novel".into(),
            genre: String::new(),
            style: String::new(),
            book_outline: String::new(),
            source_file: None,
            linked_kb_roots: vec![],
            volumes: vec![VolumeMeta {
                id: Uuid::new_v4().to_string(),
                title: "v".into(),
                chapter_ids: vec![ch_id.clone()],
                arc_goal: String::new(),
                arc_summary: String::new(),
            }],
            chapters: vec![ChapterMeta {
                id: ch_id.clone(),
                file,
                title: "第一章".into(),
                summary: String::new(),
                status: "draft".into(),
                pov_lore_id: None,
                focus_arc_ids: vec![],
                must_do: String::new(),
                must_not: String::new(),
                reader_knows: String::new(),
                character_knows: String::new(),
                beats: vec![],
                trope_ids: vec![],
            }],
            outline_mindmap: None,
            created_at: "t".into(),
            updated_at: "t".into(),
            trope_summary_at: None,
            trope_summary_fingerprint: None,
            trope_summary_dirty: false,
        };
        save_project_meta(root, &project).unwrap();
        ch_id
    }

    #[test]
    fn fingerprint_changes_with_body() {
        let root = tmp_root("fp_change");
        let id = seed_novel(&root, "hello");
        let a = content_fingerprint(&root, &open_project(&root).unwrap().project);
        write_chapter(&root, &id, "hello world").unwrap();
        let b = content_fingerprint(&root, &open_project(&root).unwrap().project);
        assert_ne!(a, b);
    }

    #[test]
    fn write_chapter_marks_dirty_after_stamp() {
        let root = tmp_root("write_dirty");
        let id = seed_novel(&root, "hello body");
        assert!(stamp_trope_summary(&root).unwrap());
        let before = open_project(&root).unwrap().project;
        assert!(!before.trope_summary_dirty);
        assert!(before.trope_summary_at.as_ref().is_some());
        write_chapter(&root, &id, "hello body changed").unwrap();
        let after = open_project(&root).unwrap().project;
        assert!(after.trope_summary_dirty);
        assert_eq!(after.trope_summary_at, before.trope_summary_at);
    }

    #[test]
    fn status_list_backfills_dirty_on_external_edit() {
        let root = tmp_root("external_edit");
        let _id = seed_novel(&root, "hello body");
        stamp_trope_summary(&root).unwrap();
        fs::write(chapters_dir(&root).join("0001-第一章.md"), "tampered").unwrap();
        let v = trope_summary_status_list(&[root.to_string_lossy().to_string()]);
        let item = &v["items"][0];
        assert_eq!(item["status"], "stale");
        assert_eq!(item["dirty"], true);
        let saved = open_project(&root).unwrap().project;
        assert!(saved.trope_summary_dirty);
    }

    #[test]
    fn stamp_clears_dirty() {
        let root = tmp_root("stamp_clear");
        let id = seed_novel(&root, "hello body");
        stamp_trope_summary(&root).unwrap();
        write_chapter(&root, &id, "changed again").unwrap();
        assert!(open_project(&root).unwrap().project.trope_summary_dirty);
        stamp_trope_summary(&root).unwrap();
        let p = open_project(&root).unwrap().project;
        assert!(!p.trope_summary_dirty);
        assert!(p.trope_summary_at.is_some());
    }

    #[test]
    fn empty_book_does_not_stamp() {
        let root = tmp_root("empty_book");
        let _id = seed_novel(&root, "   \n\n  ");
        assert!(!has_chapter_prose(
            &root,
            &open_project(&root).unwrap().project
        ));
        assert!(!stamp_trope_summary(&root).unwrap());
        let p = open_project(&root).unwrap().project;
        assert!(p.trope_summary_at.is_none());
    }

    #[test]
    fn create_and_delete_chapter_mark_dirty() {
        let root = tmp_root("struct_change");
        seed_novel(&root, "hello body");
        stamp_trope_summary(&root).unwrap();
        let meta = create_chapter(&root, "第二章", "").unwrap();
        assert!(open_project(&root).unwrap().project.trope_summary_dirty);
        stamp_trope_summary(&root).unwrap();
        delete_chapter(&root, &meta.id).unwrap();
        assert!(open_project(&root).unwrap().project.trope_summary_dirty);
    }
}
