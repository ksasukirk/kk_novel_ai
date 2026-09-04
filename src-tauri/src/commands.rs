//! Tauri 命令
//! 代码路径: kk_novel_ai/src-tauri/src/commands.rs

use crate::api;
use crate::genlog;
use crate::gui_writing;
use crate::ipc::PrepareRegistry;
use crate::llm::{stream::CancelRegistry, ChatMessage, ChatOptions, LmStudioClient};
use crate::project::LoreEntry;
use crate::settings::AppSettings;
use crate::writing::WritingRequest;
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[tauri::command]
pub fn ping() -> Value {
    json!({ "ok": true, "message": "pong" })
}

#[tauri::command]
pub fn settings_get() -> Result<Value, String> {
    api::settings_get().map_err(Into::into)
}

#[tauri::command]
pub fn settings_save(settings: AppSettings) -> Result<Value, String> {
    api::settings_save(settings).map_err(Into::into)
}

#[tauri::command]
pub async fn llm_health() -> Result<Value, String> {
    api::llm_health().await.map_err(Into::into)
}

#[tauri::command]
pub async fn llm_list_models() -> Result<Value, String> {
    api::llm_list_models().await.map_err(Into::into)
}

#[tauri::command]
pub async fn llm_chat(messages: Vec<ChatMessage>, options: Option<ChatOptions>) -> Result<Value, String> {
    api::llm_chat(messages, options.unwrap_or_default())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn llm_chat_stream(
    app: AppHandle,
    cancel_reg: State<'_, Arc<CancelRegistry>>,
    messages: Vec<ChatMessage>,
    options: Option<ChatOptions>,
) -> Result<Value, String> {
    let request_id = Uuid::new_v4().to_string();
    let cancel = cancel_reg.register(&request_id);
    let settings = crate::settings::load_settings().map_err(|e| e.to_string())?;
    let client = LmStudioClient::new();
    let opts = options.unwrap_or(ChatOptions {
        stream: true,
        ..Default::default()
    });
    let app2 = app.clone();
    let rid = request_id.clone();
    let result = client
        .chat_stream(&settings, &messages, &opts, cancel, move |delta| {
            let _ = app2.emit(
                "llm-chunk",
                json!({ "request_id": rid, "delta": delta }),
            );
        })
        .await;
    cancel_reg.remove(&request_id);
    match result {
        Ok(r) => {
            let model = opts
                .model
                .clone()
                .unwrap_or_else(|| settings.model.clone());
            let (log_id, cost_cny) =
                match crate::genlog::record_llm_call(
                    "llm_chat",
                    "",
                    "",
                    &r.text,
                    &r.text,
                    "llm_chat_stream",
                    false,
                    &model,
                    "",
                    &messages,
                    Some(r.usage.clone()),
                    &settings,
                ) {
                    Ok(entry) => (entry.id, entry.cost_cny),
                    Err(_) => (String::new(), 0.0),
                };
            let _ = app.emit(
                "llm-done",
                json!({
                    "request_id": request_id,
                    "text": r.text,
                    "usage": r.usage,
                    "log_id": log_id,
                    "cost_cny": cost_cny,
                }),
            );
            Ok(json!({
                "ok": true,
                "request_id": request_id,
                "text": r.text,
                "usage": r.usage,
                "log_id": log_id,
                "cost_cny": cost_cny,
            }))
        }
        Err(e) => {
            let _ = app.emit(
                "llm-error",
                json!({ "request_id": request_id, "error": e.to_string() }),
            );
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub fn llm_cancel(cancel_reg: State<'_, Arc<CancelRegistry>>, request_id: String) -> Value {
    let ok = cancel_reg.cancel(&request_id);
    json!({ "ok": ok, "request_id": request_id })
}

#[tauri::command]
pub async fn writing_run(
    app: AppHandle,
    cancel_reg: State<'_, Arc<CancelRegistry>>,
    request: WritingRequest,
) -> Result<Value, String> {
    gui_writing::run_writing_emit(app, cancel_reg.inner().clone(), request, "gui", |_| {})
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub fn ipc_prepare_ack(
    prepare_reg: State<'_, Arc<PrepareRegistry>>,
    prepare_id: String,
) -> Value {
    crate::ipc::prepare_ack(prepare_reg.inner(), &prepare_id)
}

#[tauri::command]
pub fn project_create(root: String, title: String) -> Result<Value, String> {
    api::project_create(&root, &title).map_err(Into::into)
}

#[tauri::command]
pub fn project_create_in_novels(title: String) -> Result<Value, String> {
    api::project_create_in_novels(&title).map_err(Into::into)
}

#[tauri::command]
pub fn novels_dir_info() -> Result<Value, String> {
    api::novels_dir_info().map_err(Into::into)
}

#[tauri::command]
pub fn novels_list_projects() -> Result<Value, String> {
    api::novels_list_projects().map_err(Into::into)
}

#[tauri::command]
pub fn project_open(root: String) -> Result<Value, String> {
    api::project_open(&root).map_err(Into::into)
}

#[tauri::command]
pub fn project_import_directory(root: String, max_depth: Option<u32>) -> Result<Value, String> {
    api::project_import_directory(&root, max_depth).map_err(Into::into)
}

#[tauri::command]
pub fn project_forget_recent(root: String) -> Result<Value, String> {
    api::project_forget_recent(&root).map_err(Into::into)
}

#[tauri::command]
pub fn project_delete(root: String, purge: Option<bool>) -> Result<Value, String> {
    api::project_delete(&root, purge.unwrap_or(false)).map_err(Into::into)
}

#[tauri::command]
pub fn project_get(root: String) -> Result<Value, String> {
    api::project_get(&root).map_err(Into::into)
}

#[tauri::command]
pub fn project_save_meta(root: String, project: crate::project::NovelProject) -> Result<Value, String> {
    api::project_save_meta(&root, project).map_err(Into::into)
}

#[tauri::command]
pub async fn project_suggest_title(root: String) -> Result<Value, String> {
    api::project_suggest_title(&root).await.map_err(Into::into)
}

#[tauri::command]
pub fn project_content_substance(root: String) -> Result<Value, String> {
    api::project_content_substance(&root).map_err(Into::into)
}

#[tauri::command]
pub fn project_apply_title(
    root: String,
    title: String,
    rename_folder: Option<bool>,
) -> Result<Value, String> {
    api::project_apply_title(&root, &title, rename_folder.unwrap_or(false)).map_err(Into::into)
}

#[tauri::command]
pub fn chapter_read(root: String, chapter_id: String) -> Result<Value, String> {
    api::chapter_read(&root, &chapter_id).map_err(Into::into)
}

#[tauri::command]
pub fn chapter_write(
    root: String,
    chapter_id: String,
    content: String,
    blocks: Option<Value>,
) -> Result<Value, String> {
    api::chapter_write(&root, &chapter_id, &content, blocks.as_ref()).map_err(Into::into)
}

#[tauri::command]
pub fn chapter_create(root: String, title: String, summary: Option<String>) -> Result<Value, String> {
    api::chapter_create(&root, &title, summary.as_deref().unwrap_or("")).map_err(Into::into)
}

#[tauri::command]
pub fn chapter_delete(root: String, chapter_id: String) -> Result<Value, String> {
    api::chapter_delete(&root, &chapter_id).map_err(Into::into)
}

#[tauri::command]
pub fn chapter_update_meta(
    root: String,
    chapter_id: String,
    patch: Option<crate::project::ChapterMetaPatch>,
    title: Option<String>,
    summary: Option<String>,
    status: Option<String>,
) -> Result<Value, String> {
    let mut p = patch.unwrap_or_default();
    if title.is_some() {
        p.title = title;
    }
    if summary.is_some() {
        p.summary = summary;
    }
    if status.is_some() {
        p.status = status;
    }
    api::chapter_update_meta(&root, &chapter_id, p).map_err(Into::into)
}

#[tauri::command]
pub fn beat_progress_get(root: String, chapter_id: String) -> Result<Value, String> {
    api::beat_progress_get(&root, &chapter_id).map_err(Into::into)
}

#[tauri::command]
pub fn beat_progress_advance(
    root: String,
    chapter_id: String,
    beat_id: String,
) -> Result<Value, String> {
    api::beat_progress_advance(&root, &chapter_id, &beat_id).map_err(Into::into)
}

#[tauri::command]
pub fn beat_progress_reset(root: String, chapter_id: String) -> Result<Value, String> {
    api::beat_progress_reset(&root, &chapter_id).map_err(Into::into)
}

#[tauri::command]
pub fn beat_progress_skip(
    root: String,
    chapter_id: String,
    beat_id: String,
) -> Result<Value, String> {
    api::beat_progress_skip(&root, &chapter_id, &beat_id).map_err(Into::into)
}

#[tauri::command]
pub fn story_plot_get(root: String) -> Result<Value, String> {
    api::story_plot_get(&root).map_err(Into::into)
}

#[tauri::command]
pub fn story_plot_save(root: String, plot: crate::story::PlotStore) -> Result<Value, String> {
    api::story_plot_save(&root, plot).map_err(Into::into)
}

#[tauri::command]
pub fn story_timeline_get(root: String) -> Result<Value, String> {
    api::story_timeline_get(&root).map_err(Into::into)
}

#[tauri::command]
pub fn story_timeline_save(
    root: String,
    timeline: crate::story::TimelineStore,
) -> Result<Value, String> {
    api::story_timeline_save(&root, timeline).map_err(Into::into)
}

#[tauri::command]
pub fn story_relations_get(root: String) -> Result<Value, String> {
    api::story_relations_get(&root).map_err(Into::into)
}

#[tauri::command]
pub fn story_relations_save(
    root: String,
    relations: crate::story::RelationsStore,
) -> Result<Value, String> {
    api::story_relations_save(&root, relations).map_err(Into::into)
}

#[tauri::command]
pub fn story_canon_get(root: String) -> Result<Value, String> {
    api::story_canon_get(&root).map_err(Into::into)
}

#[tauri::command]
pub fn story_canon_save(root: String, canon: crate::story::CanonStore) -> Result<Value, String> {
    api::story_canon_save(&root, canon).map_err(Into::into)
}

#[tauri::command]
pub fn story_apply_patch(root: String, patch: Value) -> Result<Value, String> {
    api::story_apply_patch(&root, patch).map_err(Into::into)
}

#[tauri::command]
pub fn story_dashboard(root: String) -> Result<Value, String> {
    api::story_dashboard(&root).map_err(Into::into)
}

#[tauri::command]
pub fn lore_list(root: String) -> Result<Value, String> {
    api::lore_list(&root).map_err(Into::into)
}

#[tauri::command]
pub fn lore_list_scoped(root: String) -> Result<Value, String> {
    api::lore_list_scoped(&root).map_err(Into::into)
}

#[tauri::command]
pub fn character_roster_ensure() -> Result<Value, String> {
    api::character_roster_ensure().map_err(Into::into)
}

#[tauri::command]
pub fn project_ensure_characters_link(root: String) -> Result<Value, String> {
    api::project_ensure_characters_link(&root).map_err(Into::into)
}

#[tauri::command]
pub fn lore_upsert(root: String, entry: LoreEntry) -> Result<Value, String> {
    api::lore_upsert(&root, entry).map_err(Into::into)
}

#[tauri::command]
pub fn memory_get(root: String) -> Result<Value, String> {
    api::memory_get(&root).map_err(Into::into)
}

#[tauri::command]
pub fn memory_upsert_block_note(
    root: String,
    chapter_id: String,
    block_key: String,
    summary: String,
) -> Result<Value, String> {
    api::memory_upsert_block_note(&root, &chapter_id, &block_key, &summary).map_err(Into::into)
}

#[tauri::command]
pub fn memory_remove_block_note(
    root: String,
    chapter_id: String,
    block_key: String,
) -> Result<Value, String> {
    api::memory_remove_block_note(&root, &chapter_id, &block_key).map_err(Into::into)
}

#[tauri::command]
pub fn memory_sync_chapter_snapshot(
    root: String,
    chapter_id: String,
    fallback: Option<String>,
) -> Result<Value, String> {
    api::memory_sync_chapter_snapshot(&root, &chapter_id, fallback.as_deref().unwrap_or(""))
        .map_err(Into::into)
}

#[tauri::command]
pub fn lore_delete(root: String, lore_id: String) -> Result<Value, String> {
    api::lore_delete(&root, &lore_id).map_err(Into::into)
}

#[tauri::command]
pub fn export_txt(root: String, output: String) -> Result<Value, String> {
    api::export_txt(&root, &output).map_err(Into::into)
}

#[tauri::command]
pub fn export_epub(root: String, output: String) -> Result<Value, String> {
    api::export_epub(&root, &output).map_err(Into::into)
}

#[tauri::command]
pub fn export_pdf(root: String, output: String) -> Result<Value, String> {
    api::export_pdf(&root, &output).map_err(Into::into)
}

#[tauri::command]
pub fn import_txt(root: String, file: String, title: String) -> Result<Value, String> {
    api::import_txt(&root, &file, &title).map_err(Into::into)
}

#[tauri::command]
pub async fn import_distill(
    root: String,
    from: u64,
    to: u64,
    apply: Option<String>,
    resume: Option<bool>,
    job_id: Option<String>,
    instruction: Option<String>,
) -> Result<Value, String> {
    api::import_distill(
        &root,
        from,
        to,
        apply.as_deref().unwrap_or("none"),
        resume.unwrap_or(false),
        job_id.as_deref(),
        instruction.as_deref().unwrap_or(""),
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub fn import_apply_pending(root: String, job_id: String) -> Result<Value, String> {
    api::import_apply_pending(&root, &job_id).map_err(Into::into)
}

#[tauri::command]
pub fn pick_file(title: Option<String>, extensions: Option<Vec<String>>) -> Result<Value, String> {
    api::pick_file(title.as_deref(), extensions).map_err(Into::into)
}

#[tauri::command]
pub fn kb_registry_list() -> Result<Value, String> {
    api::kb_registry_list().map_err(Into::into)
}

#[tauri::command]
pub fn kb_universal_open() -> Result<Value, String> {
    api::kb_universal_open().map_err(Into::into)
}

#[tauri::command]
pub fn kb_sync(root: String) -> Result<Value, String> {
    api::kb_sync(&root).map_err(Into::into)
}

#[tauri::command]
pub fn kb_sync_all() -> Result<Value, String> {
    api::kb_sync_all().map_err(Into::into)
}

#[tauri::command]
pub async fn kb_universal_rebuild_rag() -> Result<Value, String> {
    api::kb_universal_rebuild_rag().await.map_err(Into::into)
}

#[tauri::command]
pub fn kb_migrate(
    root: String,
    source_file: Option<String>,
    sync: Option<bool>,
) -> Result<Value, String> {
    api::kb_migrate(&root, source_file.as_deref(), sync.unwrap_or(true)).map_err(Into::into)
}

#[tauri::command]
pub fn stats_get(root: String) -> Result<Value, String> {
    api::stats_get(&root).map_err(Into::into)
}

#[tauri::command]
pub fn stats_set_goal(root: String, goal_chars: u64) -> Result<Value, String> {
    api::stats_set_goal(&root, goal_chars).map_err(Into::into)
}

#[tauri::command]
pub fn chapter_push_history(root: String, chapter_id: String, content: String) -> Result<Value, String> {
    api::chapter_push_history(&root, &chapter_id, &content).map_err(Into::into)
}

#[tauri::command]
pub async fn rag_rebuild(root: String) -> Result<Value, String> {
    api::rag_rebuild(&root).await.map_err(Into::into)
}

#[tauri::command]
pub fn pick_directory() -> Result<Value, String> {
    api::pick_directory().map_err(Into::into)
}

#[tauri::command]
pub fn pick_import_directory() -> Result<Value, String> {
    api::pick_import_directory().map_err(Into::into)
}

#[tauri::command]
pub async fn provider_balance() -> Result<Value, String> {
    let s = crate::settings::load_settings().map_err(|e| e.to_string())?;
    crate::llm::balance::fetch_provider_balance(&s)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub fn gen_log_list(limit: Option<u32>) -> Result<Value, String> {
    genlog::list_as_json(limit.unwrap_or(50) as usize).map_err(Into::into)
}

#[tauri::command]
pub fn project_gen_log_list(root: String, limit: Option<u32>) -> Result<Value, String> {
    crate::project_genlog::list_as_json(
        std::path::Path::new(&root),
        limit.unwrap_or(200) as usize,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn usage_summary(root: Option<String>) -> Result<Value, String> {
    api::usage_summary(root.as_deref()).map_err(Into::into)
}

/// 按当前单价重算历史花费，重建账本，并回写各作品 gen_activity / .genlog
#[tauri::command]
pub fn usage_backfill_costs() -> Result<Value, String> {
    let s = crate::settings::load_settings().map_err(|e| e.to_string())?;
    crate::usage::backfill_costs_from_genlog(&s).map_err(Into::into)
}

#[tauri::command]
pub fn project_export_backup(root: String) -> Result<Value, String> {
    api::project_export_backup(&root).map_err(Into::into)
}

#[tauri::command]
pub fn project_import_backup_base64(
    data_b64: String,
    title: Option<String>,
) -> Result<Value, String> {
    api::project_import_backup_base64(&data_b64, title.as_deref()).map_err(Into::into)
}

#[tauri::command]
pub fn export_file_read_base64(path: String) -> Result<Value, String> {
    api::export_file_read_base64(&path).map_err(Into::into)
}

#[tauri::command]
pub fn platform_info() -> Result<Value, String> {
    api::platform_info().map_err(Into::into)
}
