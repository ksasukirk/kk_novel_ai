//! Kk Novel Ai Tauri 主逻辑
//! 代码路径: kk_novel_ai/src-tauri/src/lib.rs

#![recursion_limit = "256"]

pub mod cli;

mod api;
mod commands;
mod error;
mod export;
mod genlog;
mod import;
mod gui_writing;
mod ipc;
mod kb;
mod llm;
mod paths;
mod project;
mod rag;
mod settings;
mod story;
mod usage;
mod writing;

use ipc::PrepareRegistry;
use llm::stream::CancelRegistry;
use std::sync::Arc;
use tauri::RunEvent;

pub use api::{dispatch_rpc, writing_run_stream, writing_run_stream_full};
pub use error::{AppError, AppResult};
pub use writing::WritingRequest;

/// 启动 GUI（无 CLI 参数时由 main 调用）
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cancel_reg = Arc::new(CancelRegistry::new());
    let prepare_reg = Arc::new(PrepareRegistry::new());
    let cancel_for_setup = cancel_reg.clone();
    let prepare_for_setup = prepare_reg.clone();

    tauri::Builder::default()
        .manage(cancel_reg)
        .manage(prepare_reg)
        .setup(move |app| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                ipc::start_ipc_server(app.handle().clone(), cancel_for_setup, prepare_for_setup);
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                let _ = (app, cancel_for_setup, prepare_for_setup);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::settings_get,
            commands::settings_save,
            commands::llm_health,
            commands::llm_list_models,
            commands::llm_chat,
            commands::llm_chat_stream,
            commands::llm_cancel,
            commands::writing_run,
            commands::ipc_prepare_ack,
            commands::project_create,
            commands::project_create_in_novels,
            commands::novels_dir_info,
            commands::project_open,
            commands::project_forget_recent,
            commands::project_get,
            commands::project_save_meta,
            commands::project_suggest_title,
            commands::project_apply_title,
            commands::chapter_read,
            commands::chapter_write,
            commands::chapter_create,
            commands::chapter_delete,
            commands::chapter_update_meta,
            commands::beat_progress_get,
            commands::beat_progress_advance,
            commands::beat_progress_reset,
            commands::beat_progress_skip,
            commands::lore_list,
            commands::lore_list_scoped,
            commands::character_roster_ensure,
            commands::project_ensure_characters_link,
            commands::lore_upsert,
            commands::memory_upsert_block_note,
            commands::memory_remove_block_note,
            commands::memory_sync_chapter_snapshot,
            commands::lore_delete,
            commands::story_plot_get,
            commands::story_plot_save,
            commands::story_timeline_get,
            commands::story_timeline_save,
            commands::story_relations_get,
            commands::story_relations_save,
            commands::story_canon_get,
            commands::story_canon_save,
            commands::story_apply_patch,
            commands::story_dashboard,
            commands::export_txt,
            commands::export_epub,
            commands::export_pdf,
            commands::import_txt,
            commands::import_distill,
            commands::import_apply_pending,
            commands::pick_file,
            commands::kb_registry_list,
            commands::kb_universal_open,
            commands::kb_sync,
            commands::kb_sync_all,
            commands::kb_universal_rebuild_rag,
            commands::kb_migrate,
            commands::stats_get,
            commands::stats_set_goal,
            commands::chapter_push_history,
            commands::rag_rebuild,
            commands::pick_directory,
            commands::gen_log_list,
            commands::usage_summary,
            commands::project_export_backup,
            commands::project_import_backup_base64,
            commands::export_file_read_base64,
            commands::platform_info,
        ])
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用失败")
        .run(|_app, event| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
                ipc::on_gui_exit();
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                let _ = event;
            }
        });
}
