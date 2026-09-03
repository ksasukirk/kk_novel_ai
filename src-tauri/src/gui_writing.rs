//! GUI / IPC 共用的流式写作（emit llm-*）
//! 代码路径: kk_novel_ai/src-tauri/src/gui_writing.rs

use crate::api;
use crate::error::AppResult;
use crate::llm::stream::CancelRegistry;
use crate::writing::WritingRequest;
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

/// 运行写作任务：向前端 emit llm-chunk/done/error，并把增量交给 extra_delta（如 IPC 回传 CLI）。
pub async fn run_writing_emit(
    app: AppHandle,
    cancel_reg: Arc<CancelRegistry>,
    request: WritingRequest,
    source: &str,
    mut extra_delta: impl FnMut(&str),
) -> AppResult<Value> {
    let request_id = Uuid::new_v4().to_string();
    let cancel = cancel_reg.register(&request_id);
    let settings = crate::settings::load_settings().unwrap_or_default();
    let peak_notice = settings.deepseek_peak_notice();
    let app2 = app.clone();
    let rid = request_id.clone();
    let task_label = request.task.clone();
    let task_for_chunk = task_label.clone();
    // 先发 start，前端才能在首包 token 前取消
    let _ = app.emit(
        "llm-start",
        json!({
            "request_id": request_id,
            "task": task_label,
            "source": source,
            "deepseek_peak": peak_notice.is_some(),
            "deepseek_peak_notice": peak_notice,
        }),
    );
    let result = api::writing_run_stream_full(
        request,
        cancel,
        move |delta| {
            let _ = app2.emit(
                "llm-chunk",
                json!({ "request_id": rid, "delta": delta, "task": task_for_chunk }),
            );
            extra_delta(delta);
        },
        source,
    )
    .await;
    cancel_reg.remove(&request_id);
    match result {
        Ok(out) => {
            let _ = app.emit(
                "llm-done",
                json!({
                    "request_id": request_id,
                    "task": task_label,
                    "text": out.text,
                    "raw_text": out.raw_text,
                    "model_used": out.model_used,
                    "fallback_from": out.fallback_from,
                    "truncated": out.truncated,
                    "loop_retried": out.loop_retried,
                    "usage": out.usage,
                    "log_id": out.log_id,
                    "cost_cny": crate::usage::calc_cost_cny(
                        &out.usage,
                        &crate::settings::load_settings().unwrap_or_default(),
                        &out.model_used,
                    ),
                    "context_sources": out.context_sources,
                }),
            );
            Ok(json!({
                "ok": true,
                "request_id": request_id,
                "task": task_label,
                "text": out.text,
                "raw_text": out.raw_text,
                "model_used": out.model_used,
                "fallback_from": out.fallback_from,
                "truncated": out.truncated,
                "loop_retried": out.loop_retried,
                "usage": out.usage,
                "log_id": out.log_id,
                "context_sources": out.context_sources,
            }))
        }
        Err(e) => {
            let _ = app.emit(
                "llm-error",
                json!({ "request_id": request_id, "task": task_label, "error": e.to_string() }),
            );
            Err(e)
        }
    }
}
