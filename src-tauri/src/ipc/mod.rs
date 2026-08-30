//! GUI 本机 IPC（TCP NDJSON）
//! 代码路径: kk_novel_ai/src-tauri/src/ipc/mod.rs
//!
//! GUI 进程监听 127.0.0.1，写入 app_data/ipc.json；CLI 连接后转发 writing_run 等命令。

use crate::error::{AppError, AppResult};
use crate::gui_writing;
use crate::llm::stream::CancelRegistry;
use crate::paths;
use crate::project;
use crate::writing::WritingRequest;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcEndpoint {
    pub host: String,
    pub port: u16,
    pub pid: u32,
    pub token: String,
    pub started_at: String,
}

/// 等待前端在 cli-writing-start 后确认已保存脏章节
#[derive(Default)]
pub struct PrepareRegistry {
    waiters: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl PrepareRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, id: &str) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.waiters.lock().insert(id.to_string(), tx);
        rx
    }

    pub fn ack(&self, id: &str) -> bool {
        if let Some(tx) = self.waiters.lock().remove(id) {
            let _ = tx.send(());
            true
        } else {
            false
        }
    }

    pub fn cancel(&self, id: &str) {
        let _ = self.waiters.lock().remove(id);
    }
}

static IPC_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn write_endpoint(ep: &IpcEndpoint) -> AppResult<()> {
    let path = paths::ipc_endpoint_path()?;
    std::fs::write(path, serde_json::to_string_pretty(ep)?)?;
    Ok(())
}

pub fn clear_endpoint() {
    if let Ok(path) = paths::ipc_endpoint_path() {
        let _ = std::fs::remove_file(path);
    }
}

pub fn read_endpoint() -> AppResult<IpcEndpoint> {
    let path = paths::ipc_endpoint_path()?;
    if !path.exists() {
        return Err(AppError::msg(
            "未检测到运行中的 GUI（缺少 ipc.json）。请先启动界面，或加 --offline 旁路直调。",
        ));
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(&path)?)?)
}

pub fn on_gui_exit() {
    clear_endpoint();
}

pub fn prepare_ack(reg: &PrepareRegistry, prepare_id: &str) -> Value {
    json!({ "ok": reg.ack(prepare_id), "prepare_id": prepare_id })
}

/// GUI setup：启动 IPC 服务
pub fn start_ipc_server(
    app: AppHandle,
    cancel_reg: Arc<CancelRegistry>,
    prepare_reg: Arc<PrepareRegistry>,
) {
    if IPC_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_server(app, cancel_reg, prepare_reg).await {
            eprintln!("[ipc] server stopped: {e}");
        }
        IPC_RUNNING.store(false, Ordering::SeqCst);
        clear_endpoint();
    });
}

async fn run_server(
    app: AppHandle,
    cancel_reg: Arc<CancelRegistry>,
    prepare_reg: Arc<PrepareRegistry>,
) -> AppResult<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let token = Uuid::new_v4().to_string();
    write_endpoint(&IpcEndpoint {
        host: "127.0.0.1".into(),
        port,
        pid: std::process::id(),
        token: token.clone(),
        started_at: chrono::Utc::now().to_rfc3339(),
    })?;

    loop {
        let (stream, _) = listener.accept().await?;
        let app2 = app.clone();
        let cancel2 = cancel_reg.clone();
        let prep2 = prepare_reg.clone();
        let token2 = token.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = handle_connection(stream, app2, cancel2, prep2, token2).await {
                eprintln!("[ipc] connection error: {e}");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    app: AppHandle,
    cancel_reg: Arc<CancelRegistry>,
    prepare_reg: Arc<PrepareRegistry>,
    token: String,
) -> AppResult<()> {
    let (reader, writer) = stream.into_split();
    let writer = Arc::new(tokio::sync::Mutex::new(writer));
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_json(&writer, &json!({ "ok": false, "error": format!("JSON: {e}") })).await?;
                continue;
            }
        };
        let id = req
            .get("id")
            .cloned()
            .unwrap_or_else(|| json!(Uuid::new_v4().to_string()));
        let req_token = req.get("token").and_then(|v| v.as_str()).unwrap_or("");
        if req_token != token {
            write_json(
                &writer,
                &json!({ "id": id, "ok": false, "error": "IPC token 无效" }),
            )
            .await?;
            continue;
        }

        let cmd = req.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        let resp = match cmd {
            "ping" | "gui_status" => Ok(json!({
                "id": id,
                "ok": true,
                "pid": std::process::id(),
                "generating": cancel_reg.active_count() > 0,
                "active": cancel_reg.active_count()
            })),
            "llm_cancel" => {
                let rid = req.get("request_id").and_then(|v| v.as_str()).unwrap_or("");
                Ok(json!({ "id": id, "ok": cancel_reg.cancel(rid), "request_id": rid }))
            }
            "project_focus" => {
                let root = req.get("root").and_then(|v| v.as_str()).unwrap_or("");
                let chapter_id = req.get("chapter_id").and_then(|v| v.as_str());
                let _ = app.emit(
                    "project-focus",
                    json!({ "root": root, "chapter_id": chapter_id }),
                );
                Ok(json!({ "id": id, "ok": true }))
            }
            "preview_apply" => preview_apply(&app, &req, &id),
            "writing_run" => {
                writing_run_ipc(&app, &cancel_reg, &prepare_reg, &req, &id, &writer).await
            }
            other => Ok(json!({
                "id": id,
                "ok": false,
                "error": format!("未知 IPC cmd: {other}")
            })),
        };

        match resp {
            Ok(v) => write_json(&writer, &v).await?,
            Err(e) => {
                write_json(
                    &writer,
                    &json!({ "id": id, "ok": false, "error": e.to_string() }),
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn write_json(
    writer: &Arc<tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    v: &Value,
) -> AppResult<()> {
    let mut s = serde_json::to_string(v)?;
    s.push('\n');
    let mut w = writer.lock().await;
    w.write_all(s.as_bytes()).await?;
    w.flush().await?;
    Ok(())
}

fn preview_apply(app: &AppHandle, req: &Value, id: &Value) -> AppResult<Value> {
    let mode = req.get("mode").and_then(|v| v.as_str()).unwrap_or("append");
    let root = req
        .get("project_root")
        .or_else(|| req.get("root"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("缺少 project_root"))?;
    let chapter_id = req
        .get("chapter_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("缺少 chapter_id"))?;
    let text = req
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg("缺少 text"))?;
    let selection = req.get("selection").and_then(|v| v.as_str()).unwrap_or("");

    let (_meta, content) = project::read_chapter(Path::new(root), chapter_id)?;
    let new_content = match mode {
        "replace" => {
            if selection.is_empty() {
                return Err(AppError::msg("replace 需要 selection"));
            }
            content.replacen(selection, text, 1)
        }
        _ => {
            let sep = if content.is_empty() {
                ""
            } else if content.ends_with("\n\n") {
                ""
            } else if content.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{content}{sep}{text}")
        }
    };
    project::write_chapter(Path::new(root), chapter_id, &new_content)?;
    let _ = app.emit(
        "chapter-external-update",
        json!({
            "root": root,
            "chapter_id": chapter_id,
            "content": new_content,
            "saved": true
        }),
    );
    Ok(json!({ "id": id, "ok": true, "mode": mode }))
}

async fn writing_run_ipc(
    app: &AppHandle,
    cancel_reg: &Arc<CancelRegistry>,
    prepare_reg: &Arc<PrepareRegistry>,
    req: &Value,
    id: &Value,
    writer: &Arc<tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>>,
) -> AppResult<Value> {
    let writing_req: WritingRequest = serde_json::from_value(
        req.get("request")
            .cloned()
            .ok_or_else(|| AppError::msg("缺少 request"))?,
    )?;
    let apply = req
        .get("apply")
        .and_then(|v| v.as_str())
        .unwrap_or("none")
        .to_string();
    let stream_chunks = req
        .get("stream_chunks")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let prepare_id = Uuid::new_v4().to_string();
    let rx = prepare_reg.register(&prepare_id);
    let _ = app.emit(
        "project-focus",
        json!({
            "root": writing_req.project_root,
            "chapter_id": writing_req.chapter_id
        }),
    );
    let _ = app.emit(
        "cli-writing-start",
        json!({
            "prepare_id": prepare_id,
            "project_root": writing_req.project_root,
            "chapter_id": writing_req.chapter_id,
            "task": writing_req.task
        }),
    );
    match tokio::time::timeout(Duration::from_secs(5), rx).await {
        Ok(Ok(())) => {}
        _ => {
            prepare_reg.cancel(&prepare_id);
        }
    }

    let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let writer_c = writer.clone();
    let id_c = id.clone();
    let flusher = tauri::async_runtime::spawn(async move {
        while let Some(delta) = chunk_rx.recv().await {
            let _ = write_json(
                &writer_c,
                &json!({
                    "id": id_c,
                    "type": "chunk",
                    "delta": delta
                }),
            )
            .await;
        }
    });

    let result = gui_writing::run_writing_emit(
        app.clone(),
        cancel_reg.clone(),
        writing_req.clone(),
        "gui-ipc",
        move |delta| {
            if stream_chunks {
                let _ = chunk_tx.send(delta.to_string());
            }
        },
    )
    .await;
    // closure 释放后关闭 chunk 通道，等刷完再回最终 JSON
    let _ = flusher.await;
    let result = result?;
    let text = result
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let request_id = result.get("request_id").cloned().unwrap_or(json!(null));

    let mut applied = "none".to_string();
    if apply == "append" || apply == "replace" {
        preview_apply(
            app,
            &json!({
                "mode": apply,
                "project_root": writing_req.project_root,
                "chapter_id": writing_req.chapter_id,
                "text": text,
                "selection": writing_req.selection
            }),
            id,
        )?;
        applied = apply;
    }

    Ok(json!({
        "id": id,
        "ok": true,
        "request_id": request_id,
        "text": text,
        "via": "gui-ipc",
        "applied": applied
    }))
}

/// CLI：向 GUI IPC 发送一条请求，收集最终响应（chunk 行回调 on_chunk）
pub async fn cli_request(req: Value, mut on_chunk: impl FnMut(&str)) -> AppResult<Value> {
    let ep = read_endpoint()?;
    let addr = format!("{}:{}", ep.host, ep.port);
    let stream = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(&addr))
        .await
        .map_err(|_| {
            AppError::msg(format!(
                "连接 GUI IPC 超时 ({addr})。请确认界面已启动，或删除 %APPDATA%/kk_novel_ai/ipc.json 后重试。"
            ))
        })?
        .map_err(|e| {
            AppError::msg(format!(
                "连接 GUI IPC 失败 ({addr}): {e}. 请确认界面已启动，或加 --offline。"
            ))
        })?;
    let mut stream = stream;

    let mut payload = req;
    if payload.get("token").is_none() {
        payload["token"] = json!(ep.token);
    }
    if payload.get("id").is_none() {
        payload["id"] = json!(Uuid::new_v4().to_string());
    }
    let mut line = serde_json::to_string(&payload)?;
    line.push('\n');
    stream.write_all(line.as_bytes()).await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    let mut buf = String::new();
    loop {
        buf.clear();
        let n = reader.read_line(&mut buf).await?;
        if n == 0 {
            return Err(AppError::msg("GUI IPC 连接已关闭"));
        }
        let v: Value = serde_json::from_str(buf.trim())
            .map_err(|e| AppError::msg(format!("IPC 响应 JSON 无效: {e}")))?;
        if v.get("type").and_then(|t| t.as_str()) == Some("chunk") {
            if let Some(d) = v.get("delta").and_then(|x| x.as_str()) {
                on_chunk(d);
            }
            continue;
        }
        return Ok(v);
    }
}
