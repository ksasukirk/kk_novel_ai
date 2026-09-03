//! 外部大模型 / 调试用 CLI（整合进主程序 kk_novel_ai）
//! 代码路径: kk_novel_ai/src-tauri/src/cli.rs
//!
//! - 无参数：启动 GUI
//! - 有子命令 / `--cli`：CLI 模式（JSON stdout / NDJSON RPC）

use crate::api::{dispatch_rpc, writing_run_stream_full};
use crate::error::AppError;
use crate::writing::WritingRequest;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "kk_novel_ai")]
#[command(about = "Kk Novel Ai — GUI 无参数启动；带子命令时进入 CLI（供外部大模型调试）")]
#[command(version)]
struct Cli {
    /// 人类可读输出（默认 JSON）
    #[arg(long, global = true)]
    human: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 连通性
    Ping,
    /// NDJSON RPC：每行一个 {"cmd":...}，适合外部 Agent 长会话
    Rpc,
    /// 打印可调用 cmd 列表（给外部模型看的工具清单）
    Tools,
    Settings {
        #[command(subcommand)]
        action: SettingsCmd,
    },
    Llm {
        #[command(subcommand)]
        action: LlmCmd,
    },
    Project {
        #[command(subcommand)]
        action: ProjectCmd,
    },
    Chapter {
        #[command(subcommand)]
        action: ChapterCmd,
    },
    Lore {
        #[command(subcommand)]
        action: LoreCmd,
    },
    Writing {
        #[command(subcommand)]
        action: WritingCmd,
    },
    Story {
        #[command(subcommand)]
        action: StoryCmd,
    },
    Export {
        #[command(subcommand)]
        action: ExportCmd,
    },
    Import {
        #[command(subcommand)]
        action: ImportCmd,
    },
    Kb {
        #[command(subcommand)]
        action: KbCmd,
    },
    Stats {
        #[command(subcommand)]
        action: StatsCmd,
    },
    Rag {
        #[command(subcommand)]
        action: RagCmd,
    },
    GenLog {
        #[arg(long, default_value_t = 50)]
        limit: u64,
    },
}

#[derive(Subcommand, Debug)]
enum SettingsCmd {
    Get,
    /// 用 JSON 字符串覆盖保存设置
    Set {
        json: String,
    },
    /// 只改个别字段
    Patch {
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long)]
        api_key: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        analysis_model: Option<String>,
        #[arg(long)]
        embedding_model: Option<String>,
        #[arg(long)]
        temperature: Option<f32>,
        #[arg(long)]
        analysis_temperature: Option<f32>,
        #[arg(long)]
        max_tokens: Option<u32>,
        #[arg(long)]
        writing_target_chars: Option<u32>,
        #[arg(long)]
        context_budget: Option<u32>,
        #[arg(long)]
        recent_window_chars: Option<usize>,
        #[arg(long)]
        frequency_penalty: Option<f32>,
        #[arg(long)]
        presence_penalty: Option<f32>,
        #[arg(long)]
        llm_timeout_secs: Option<u64>,
        #[arg(long)]
        writing_retry_on_loop: Option<bool>,
        #[arg(long)]
        writing_model_fallback: Option<bool>,
    },
}

#[derive(Subcommand, Debug)]
enum LlmCmd {
    Health,
    Models,
    Chat {
        prompt: String,
        #[arg(long)]
        system: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectCmd {
    Create {
        root: String,
        #[arg(long, default_value = "未命名小说")]
        title: String,
    },
    Open {
        root: String,
    },
    Get {
        root: String,
    },
    /// 仅从最近作品列表移除（不删磁盘）
    Forget {
        root: String,
    },
    /// 从最近列表移除；加 --purge 则删除含 project.json 的作品目录
    Delete {
        root: String,
        #[arg(long, default_value_t = false)]
        purge: bool,
    },
    /// 清空全部最近小说作品；加 --purge 删除各作品目录
    ForgetAll {
        #[arg(long, default_value_t = false)]
        purge: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ChapterCmd {
    List {
        root: String,
    },
    Read {
        root: String,
        chapter_id: String,
    },
    Write {
        root: String,
        chapter_id: String,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        file: Option<String>,
    },
    Create {
        root: String,
        title: String,
        #[arg(long, default_value = "")]
        summary: String,
    },
    Delete {
        root: String,
        chapter_id: String,
    },
    Update {
        root: String,
        chapter_id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum LoreCmd {
    List {
        root: String,
    },
    Upsert {
        root: String,
        json: String,
    },
    Delete {
        root: String,
        lore_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum StoryCmd {
    Plot {
        #[command(subcommand)]
        action: StoryPlotCmd,
    },
    Timeline {
        #[command(subcommand)]
        action: StoryTlCmd,
    },
    Canon {
        #[command(subcommand)]
        action: StoryCanonCmd,
    },
    Relations {
        #[command(subcommand)]
        action: StoryRelCmd,
    },
    Dashboard {
        root: String,
    },
    ApplyPatch {
        root: String,
        json: String,
    },
}

#[derive(Subcommand, Debug)]
enum StoryPlotCmd {
    Get { root: String },
    Save { root: String, json: String },
}

#[derive(Subcommand, Debug)]
enum StoryTlCmd {
    Get { root: String },
    Save { root: String, json: String },
}

#[derive(Subcommand, Debug)]
enum StoryCanonCmd {
    Get { root: String },
    Save { root: String, json: String },
}

#[derive(Subcommand, Debug)]
enum StoryRelCmd {
    Get { root: String },
    Save { root: String, json: String },
}

#[derive(Subcommand, Debug)]
enum WritingCmd {
    Run {
        root: String,
        chapter_id: String,
        task: String,
        #[arg(long, default_value = "")]
        instruction: String,
        #[arg(long, default_value = "")]
        selection: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        temperature: Option<f32>,
        #[arg(long)]
        max_tokens: Option<u32>,
        #[arg(long)]
        frequency_penalty: Option<f32>,
        #[arg(long)]
        presence_penalty: Option<f32>,
        /// 主模型失败时回退的模型 id
        #[arg(long)]
        fallback_model: Option<String>,
        /// 关闭复读截断后的自动重试
        #[arg(long, default_value_t = false)]
        no_retry_on_loop: bool,
        #[arg(long)]
        stream_stderr: bool,
        /// 旁路 GUI，直接调 LM Studio（不更新界面预览）
        #[arg(long)]
        offline: bool,
        /// 生成后应用到章节：append | replace | none
        #[arg(long, default_value = "none")]
        apply: String,
    },
}

#[derive(Subcommand, Debug)]
enum ExportCmd {
    Txt {
        root: String,
        output: String,
    },
    Epub {
        root: String,
        output: String,
    },
    Pdf {
        root: String,
        output: String,
    },
}

#[derive(Subcommand, Debug)]
enum ImportCmd {
    /// 从 TXT 导入作品（===标题=== 或第N章）
    Txt {
        root: String,
        #[arg(long)]
        file: String,
        #[arg(long, default_value = "未命名小说")]
        title: String,
    },
    /// 按章蒸馏知识库（实体/关系/Canon/总谱）
    Distill {
        root: String,
        #[arg(long, default_value_t = 1)]
        from: u64,
        #[arg(long, default_value_t = 20)]
        to: u64,
        /// none | auto
        #[arg(long, default_value = "none")]
        apply: String,
        #[arg(long, default_value_t = false)]
        resume: bool,
        #[arg(long)]
        job_id: Option<String>,
        #[arg(long, default_value = "")]
        instruction: String,
    },
    /// 将 distill job 的 pending 应用到作品
    Apply {
        root: String,
        job_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum KbCmd {
    List,
    Universal,
    ImportTxt {
        root: String,
        #[arg(long)]
        file: String,
        #[arg(long, default_value = "未命名知识库")]
        title: String,
    },
    Distill {
        root: String,
        #[arg(long, default_value_t = 1)]
        from: u64,
        #[arg(long, default_value_t = 20)]
        to: u64,
        #[arg(long, default_value = "none")]
        apply: String,
        #[arg(long, default_value_t = false)]
        resume: bool,
        #[arg(long)]
        job_id: Option<String>,
    },
    Sync {
        root: String,
    },
    SyncAll,
    Migrate {
        root: String,
        #[arg(long)]
        source_file: Option<String>,
        #[arg(long, default_value_t = true)]
        sync: bool,
    },
    UniversalDashboard,
}

#[derive(Subcommand, Debug)]
enum StatsCmd {
    Get {
        root: String,
    },
    SetGoal {
        root: String,
        goal_chars: u64,
    },
}

#[derive(Subcommand, Debug)]
enum RagCmd {
    Rebuild {
        root: String,
    },
}

fn is_cli_verb(s: &str) -> bool {
    matches!(
        s,
        "ping"
            | "rpc"
            | "tools"
            | "settings"
            | "llm"
            | "project"
            | "chapter"
            | "lore"
            | "writing"
            | "story"
            | "export"
            | "import"
            | "kb"
            | "stats"
            | "rag"
            | "gen-log"
            | "help"
    )
}

/// 是否应以 CLI 模式启动（无业务参数则走 GUI）
pub fn should_run_cli() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return false;
    }
    let mut iter = args.iter().map(|s| s.as_str());
    while let Some(a) = iter.next() {
        if a == "--cli" {
            return true;
        }
        if a == "-h" || a == "--help" || a == "-V" || a == "--version" {
            return true;
        }
        if is_cli_verb(a) {
            return true;
        }
        if a == "--human" {
            continue;
        }
        if a.starts_with('-') {
            continue;
        }
        // 未知位置参数：不抢 GUI（避免误进 CLI）
        return false;
    }
    false
}

fn cli_argv() -> Vec<std::ffi::OsString> {
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    let mut args = std::env::args_os();
    if let Some(bin) = args.next() {
        out.push(bin);
    }
    for a in args {
        if a == "--cli" {
            continue;
        }
        out.push(a);
    }
    out
}

/// Windows GUI 子系统（release）下挂接父进程控制台，保证 CLI 能打印。
/// debug 构建本身带控制台，不要 freopen，否则会弄丢 stdout。
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    use std::ffi::CString;
    use std::os::raw::c_void;

    type BOOL = i32;
    type DWORD = u32;
    extern "system" {
        fn AttachConsole(dw_process_id: DWORD) -> BOOL;
        fn AllocConsole() -> BOOL;
        fn SetConsoleOutputCP(w_code_page_id: DWORD) -> BOOL;
        fn SetConsoleCP(w_code_page_id: DWORD) -> BOOL;
    }
    extern "C" {
        fn freopen(filename: *const i8, mode: *const i8, stream: *mut c_void) -> *mut c_void;
        fn __acrt_iob_func(index: u32) -> *mut c_void;
    }
    const ATTACH_PARENT_PROCESS: DWORD = 0xFFFFFFFF;
    const CP_UTF8: DWORD = 65001;
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            let _ = AllocConsole();
        }
        let _ = SetConsoleOutputCP(CP_UTF8);
        let _ = SetConsoleCP(CP_UTF8);
        let conin = CString::new("CONIN$").unwrap();
        let conout = CString::new("CONOUT$").unwrap();
        let r = CString::new("r").unwrap();
        let w = CString::new("w").unwrap();
        let stdin = __acrt_iob_func(0);
        let stdout = __acrt_iob_func(1);
        let stderr = __acrt_iob_func(2);
        let _ = freopen(conin.as_ptr(), r.as_ptr(), stdin);
        let _ = freopen(conout.as_ptr(), w.as_ptr(), stdout);
        let _ = freopen(conout.as_ptr(), w.as_ptr(), stderr);
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() {}

/// 运行 CLI，返回进程退出码
pub fn run() -> i32 {
    attach_parent_console();
    let cli = Cli::parse_from(cli_argv());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build();
    match rt {
        Ok(rt) => rt.block_on(run_cmd(cli)),
        Err(e) => {
            print_err(false, e);
            1
        }
    }
}

fn print_out(human: bool, value: &Value) {
    if human {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        );
    } else {
        println!("{}", value);
    }
}

fn print_err(human: bool, err: impl std::fmt::Display) {
    let v = json!({ "ok": false, "error": err.to_string() });
    print_out(human, &v);
}

async fn run_cmd(cli: Cli) -> i32 {
    let human = cli.human;
    let result = match cli.command {
        Commands::Ping => dispatch_rpc(json!({ "cmd": "ping" })).await,
        Commands::Tools => Ok(tools_manifest()),
        Commands::Rpc => {
            return run_rpc_loop(human).await;
        }
        Commands::Settings { action } => match action {
            SettingsCmd::Get => dispatch_rpc(json!({ "cmd": "settings_get" })).await,
            SettingsCmd::Set { json: raw } => {
                let settings: Value = match serde_json::from_str(&raw) {
                    Ok(v) => v,
                    Err(e) => {
                        print_err(human, e);
                        return 1;
                    }
                };
                dispatch_rpc(json!({ "cmd": "settings_save", "settings": settings })).await
            }
            SettingsCmd::Patch {
                base_url,
                api_key,
                model,
                analysis_model,
                embedding_model,
                temperature,
                analysis_temperature,
                max_tokens,
                writing_target_chars,
                context_budget,
                recent_window_chars,
                frequency_penalty,
                presence_penalty,
                llm_timeout_secs,
                writing_retry_on_loop,
                writing_model_fallback,
            } => {
                let cur = match dispatch_rpc(json!({ "cmd": "settings_get" })).await {
                    Ok(v) => v,
                    Err(e) => {
                        print_err(human, e);
                        return 1;
                    }
                };
                let mut s = cur["settings"].clone();
                if let Some(v) = base_url {
                    s["base_url"] = json!(v);
                }
                if let Some(v) = api_key {
                    s["api_key"] = json!(v);
                }
                if let Some(v) = model {
                    s["model"] = json!(v);
                }
                if let Some(v) = analysis_model {
                    s["analysis_model"] = json!(v);
                }
                if let Some(v) = embedding_model {
                    s["embedding_model"] = json!(v);
                }
                if let Some(v) = temperature {
                    s["temperature"] = json!(v);
                }
                if let Some(v) = analysis_temperature {
                    s["analysis_temperature"] = json!(v);
                }
                if let Some(v) = writing_target_chars {
                    s["writing_target_chars"] = json!(v);
                } else if let Some(v) = max_tokens {
                    // 只改 max_tokens 时同步规定字数，避免二者脱节
                    s["writing_target_chars"] = json!(v);
                }
                if let Some(v) = max_tokens {
                    s["max_tokens"] = json!(v);
                }
                if let Some(v) = context_budget {
                    s["context_budget"] = json!(v);
                }
                if let Some(v) = recent_window_chars {
                    s["recent_window_chars"] = json!(v);
                }
                if let Some(v) = frequency_penalty {
                    s["frequency_penalty"] = json!(v);
                }
                if let Some(v) = presence_penalty {
                    s["presence_penalty"] = json!(v);
                }
                if let Some(v) = llm_timeout_secs {
                    s["llm_timeout_secs"] = json!(v);
                }
                if let Some(v) = writing_retry_on_loop {
                    s["writing_retry_on_loop"] = json!(v);
                }
                if let Some(v) = writing_model_fallback {
                    s["writing_model_fallback"] = json!(v);
                }
                dispatch_rpc(json!({ "cmd": "settings_save", "settings": s })).await
            }
        },
        Commands::Llm { action } => match action {
            LlmCmd::Health => dispatch_rpc(json!({ "cmd": "llm_health" })).await,
            LlmCmd::Models => dispatch_rpc(json!({ "cmd": "llm_list_models" })).await,
            LlmCmd::Chat {
                prompt,
                system,
                model,
            } => {
                let mut messages = vec![];
                if let Some(sys) = system {
                    messages.push(json!({ "role": "system", "content": sys }));
                }
                messages.push(json!({ "role": "user", "content": prompt }));
                let mut options = json!({});
                if let Some(m) = model {
                    options["model"] = json!(m);
                }
                dispatch_rpc(json!({
                    "cmd": "llm_chat",
                    "messages": messages,
                    "options": options
                }))
                .await
            }
        },
        Commands::Project { action } => match action {
            ProjectCmd::Create { root, title } => {
                dispatch_rpc(json!({ "cmd": "project_create", "root": root, "title": title })).await
            }
            ProjectCmd::Open { root } => {
                dispatch_rpc(json!({ "cmd": "project_open", "root": root })).await
            }
            ProjectCmd::Get { root } => {
                dispatch_rpc(json!({ "cmd": "project_get", "root": root })).await
            }
            ProjectCmd::Forget { root } => {
                dispatch_rpc(json!({ "cmd": "project_forget_recent", "root": root })).await
            }
            ProjectCmd::Delete { root, purge } => {
                dispatch_rpc(json!({ "cmd": "project_delete", "root": root, "purge": purge })).await
            }
            ProjectCmd::ForgetAll { purge } => {
                dispatch_rpc(json!({ "cmd": "project_forget_all_novels", "purge": purge })).await
            }
        },
        Commands::Chapter { action } => match action {
            ChapterCmd::List { root } => {
                let v = dispatch_rpc(json!({ "cmd": "project_get", "root": root })).await;
                match v {
                    Ok(p) => Ok(json!({
                        "ok": true,
                        "chapters": p["project"]["chapters"]
                    })),
                    Err(e) => Err(e),
                }
            }
            ChapterCmd::Read { root, chapter_id } => {
                dispatch_rpc(json!({
                    "cmd": "chapter_read",
                    "root": root,
                    "chapter_id": chapter_id
                }))
                .await
            }
            ChapterCmd::Write {
                root,
                chapter_id,
                content,
                file,
            } => {
                let body = if let Some(path) = file {
                    match std::fs::read_to_string(path) {
                        Ok(t) => t,
                        Err(e) => {
                            print_err(human, e);
                            return 1;
                        }
                    }
                } else {
                    content.unwrap_or_default()
                };
                dispatch_rpc(json!({
                    "cmd": "chapter_write",
                    "root": root,
                    "chapter_id": chapter_id,
                    "content": body
                }))
                .await
            }
            ChapterCmd::Create {
                root,
                title,
                summary,
            } => {
                dispatch_rpc(json!({
                    "cmd": "chapter_create",
                    "root": root,
                    "title": title,
                    "summary": summary
                }))
                .await
            }
            ChapterCmd::Delete { root, chapter_id } => {
                dispatch_rpc(json!({
                    "cmd": "chapter_delete",
                    "root": root,
                    "chapter_id": chapter_id
                }))
                .await
            }
            ChapterCmd::Update {
                root,
                chapter_id,
                title,
                summary,
                status,
            } => {
                let mut payload = json!({
                    "cmd": "chapter_update_meta",
                    "root": root,
                    "chapter_id": chapter_id
                });
                if let Some(t) = title {
                    payload["title"] = json!(t);
                }
                if let Some(s) = summary {
                    payload["summary"] = json!(s);
                }
                if let Some(st) = status {
                    payload["status"] = json!(st);
                }
                dispatch_rpc(payload).await
            }
        },
        Commands::Lore { action } => match action {
            LoreCmd::List { root } => dispatch_rpc(json!({ "cmd": "lore_list", "root": root })).await,
            LoreCmd::Upsert { root, json: raw } => {
                let entry: Value = match serde_json::from_str(&raw) {
                    Ok(v) => v,
                    Err(e) => {
                        print_err(human, e);
                        return 1;
                    }
                };
                dispatch_rpc(json!({ "cmd": "lore_upsert", "root": root, "entry": entry })).await
            }
            LoreCmd::Delete { root, lore_id } => {
                dispatch_rpc(json!({ "cmd": "lore_delete", "root": root, "lore_id": lore_id })).await
            }
        },
        Commands::Writing { action } => match action {
            WritingCmd::Run {
                root,
                chapter_id,
                task,
                instruction,
                selection,
                model,
                temperature,
                max_tokens,
                frequency_penalty,
                presence_penalty,
                fallback_model,
                no_retry_on_loop,
                stream_stderr,
                offline,
                apply,
            } => {
                let mut request = json!({
                    "project_root": root,
                    "chapter_id": chapter_id,
                    "task": task,
                    "instruction": instruction,
                    "selection": selection
                });
                if let Some(m) = model {
                    request["model"] = json!(m);
                }
                if let Some(v) = temperature {
                    request["temperature"] = json!(v);
                }
                if let Some(v) = max_tokens {
                    request["max_tokens"] = json!(v);
                }
                if let Some(v) = frequency_penalty {
                    request["frequency_penalty"] = json!(v);
                }
                if let Some(v) = presence_penalty {
                    request["presence_penalty"] = json!(v);
                }
                if let Some(v) = fallback_model {
                    request["fallback_model"] = json!(v);
                }
                if no_retry_on_loop {
                    request["retry_on_loop"] = json!(false);
                }
                writing_run_cli(request, stream_stderr, offline, apply).await
            }
        },
        Commands::Story { action } => match action {
            StoryCmd::Plot { action } => match action {
                StoryPlotCmd::Get { root } => {
                    dispatch_rpc(json!({ "cmd": "story_plot_get", "root": root })).await
                }
                StoryPlotCmd::Save { root, json: raw } => {
                    let plot: Value = match serde_json::from_str(&raw) {
                        Ok(v) => v,
                        Err(e) => {
                            print_err(human, e);
                            return 1;
                        }
                    };
                    dispatch_rpc(json!({ "cmd": "story_plot_save", "root": root, "plot": plot })).await
                }
            },
            StoryCmd::Timeline { action } => match action {
                StoryTlCmd::Get { root } => {
                    dispatch_rpc(json!({ "cmd": "story_timeline_get", "root": root })).await
                }
                StoryTlCmd::Save { root, json: raw } => {
                    let timeline: Value = match serde_json::from_str(&raw) {
                        Ok(v) => v,
                        Err(e) => {
                            print_err(human, e);
                            return 1;
                        }
                    };
                    dispatch_rpc(json!({
                        "cmd": "story_timeline_save",
                        "root": root,
                        "timeline": timeline
                    }))
                    .await
                }
            },
            StoryCmd::Canon { action } => match action {
                StoryCanonCmd::Get { root } => {
                    dispatch_rpc(json!({ "cmd": "story_canon_get", "root": root })).await
                }
                StoryCanonCmd::Save { root, json: raw } => {
                    let canon: Value = match serde_json::from_str(&raw) {
                        Ok(v) => v,
                        Err(e) => {
                            print_err(human, e);
                            return 1;
                        }
                    };
                    dispatch_rpc(json!({ "cmd": "story_canon_save", "root": root, "canon": canon })).await
                }
            },
            StoryCmd::Relations { action } => match action {
                StoryRelCmd::Get { root } => {
                    dispatch_rpc(json!({ "cmd": "story_relations_get", "root": root })).await
                }
                StoryRelCmd::Save { root, json: raw } => {
                    let relations: Value = match serde_json::from_str(&raw) {
                        Ok(v) => v,
                        Err(e) => {
                            print_err(human, e);
                            return 1;
                        }
                    };
                    dispatch_rpc(json!({
                        "cmd": "story_relations_save",
                        "root": root,
                        "relations": relations
                    }))
                    .await
                }
            },
            StoryCmd::Dashboard { root } => {
                dispatch_rpc(json!({ "cmd": "story_dashboard", "root": root })).await
            }
            StoryCmd::ApplyPatch { root, json: raw } => {
                let patch: Value = match serde_json::from_str(&raw) {
                    Ok(v) => v,
                    Err(e) => {
                        print_err(human, e);
                        return 1;
                    }
                };
                dispatch_rpc(json!({ "cmd": "story_apply_patch", "root": root, "patch": patch })).await
            }
        },
        Commands::Export { action } => match action {
            ExportCmd::Txt { root, output } => {
                dispatch_rpc(json!({ "cmd": "export_txt", "root": root, "output": output })).await
            }
            ExportCmd::Epub { root, output } => {
                dispatch_rpc(json!({ "cmd": "export_epub", "root": root, "output": output })).await
            }
            ExportCmd::Pdf { root, output } => {
                dispatch_rpc(json!({ "cmd": "export_pdf", "root": root, "output": output })).await
            }
        },
        Commands::Import { action } => match action {
            ImportCmd::Txt { root, file, title } => {
                dispatch_rpc(json!({
                    "cmd": "import_txt",
                    "root": root,
                    "file": file,
                    "title": title
                }))
                .await
            }
            ImportCmd::Distill {
                root,
                from,
                to,
                apply,
                resume,
                job_id,
                instruction,
            } => {
                let mut payload = json!({
                    "cmd": "import_distill",
                    "root": root,
                    "from": from,
                    "to": to,
                    "apply": apply,
                    "resume": resume,
                    "instruction": instruction
                });
                if let Some(jid) = job_id {
                    payload["job_id"] = json!(jid);
                }
                dispatch_rpc(payload).await
            }
            ImportCmd::Apply { root, job_id } => {
                dispatch_rpc(json!({
                    "cmd": "import_apply_pending",
                    "root": root,
                    "job_id": job_id
                }))
                .await
            }
        },
        Commands::Kb { action } => match action {
            KbCmd::List => dispatch_rpc(json!({ "cmd": "kb_registry_list" })).await,
            KbCmd::Universal => dispatch_rpc(json!({ "cmd": "kb_universal_open" })).await,
            KbCmd::ImportTxt { root, file, title } => {
                dispatch_rpc(json!({
                    "cmd": "import_txt",
                    "root": root,
                    "file": file,
                    "title": title
                }))
                .await
            }
            KbCmd::Distill {
                root,
                from,
                to,
                apply,
                resume,
                job_id,
            } => {
                let mut payload = json!({
                    "cmd": "import_distill",
                    "root": root,
                    "from": from,
                    "to": to,
                    "apply": apply,
                    "resume": resume
                });
                if let Some(jid) = job_id {
                    payload["job_id"] = json!(jid);
                }
                dispatch_rpc(payload).await
            }
            KbCmd::Sync { root } => {
                dispatch_rpc(json!({ "cmd": "kb_sync", "root": root })).await
            }
            KbCmd::SyncAll => dispatch_rpc(json!({ "cmd": "kb_sync_all" })).await,
            KbCmd::Migrate {
                root,
                source_file,
                sync,
            } => {
                let mut payload = json!({
                    "cmd": "kb_migrate",
                    "root": root,
                    "sync": sync
                });
                if let Some(sf) = source_file {
                    payload["source_file"] = json!(sf);
                }
                dispatch_rpc(payload).await
            }
            KbCmd::UniversalDashboard => {
                match dispatch_rpc(json!({ "cmd": "kb_universal_open" })).await {
                    Ok(uni) => {
                        let root = uni
                            .get("root")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        dispatch_rpc(json!({ "cmd": "story_dashboard", "root": root })).await
                    }
                    Err(e) => Err(e),
                }
            }
        },
        Commands::Stats { action } => match action {
            StatsCmd::Get { root } => dispatch_rpc(json!({ "cmd": "stats_get", "root": root })).await,
            StatsCmd::SetGoal { root, goal_chars } => {
                dispatch_rpc(json!({ "cmd": "stats_set_goal", "root": root, "goal_chars": goal_chars }))
                    .await
            }
        },
        Commands::Rag { action } => match action {
            RagCmd::Rebuild { root } => {
                dispatch_rpc(json!({ "cmd": "rag_rebuild", "root": root })).await
            }
        },
        Commands::GenLog { limit } => {
            dispatch_rpc(json!({ "cmd": "gen_log_list", "limit": limit })).await
        }
    };

    match result {
        Ok(v) => {
            print_out(human, &v);
            if v.get("ok").and_then(|x| x.as_bool()) == Some(false) {
                1
            } else {
                0
            }
        }
        Err(e) => {
            print_err(human, e);
            1
        }
    }
}

async fn writing_run_cli(
    request: Value,
    stream_stderr: bool,
    offline: bool,
    apply: String,
) -> Result<Value, AppError> {
    if !offline {
        match crate::ipc::read_endpoint() {
            Ok(_) => {
                let ipc_req = json!({
                    "cmd": "writing_run",
                    "request": request,
                    "apply": apply,
                    "stream_chunks": stream_stderr
                });
                return crate::ipc::cli_request(ipc_req, |delta| {
                    if stream_stderr {
                        eprint!("{delta}");
                        let _ = std::io::stderr().flush();
                    }
                })
                .await
                .map(|v| {
                    if stream_stderr {
                        eprintln!();
                    }
                    v
                });
            }
            Err(e) => {
                return Err(AppError::msg(format!(
                    "{e} 若只要旁路生成，请加 --offline。"
                )));
            }
        }
    }

    // offline：直接调模型；可选本地 apply
    let req: WritingRequest = serde_json::from_value(request.clone())?;
    let cancel = Arc::new(AtomicBool::new(false));
    let outcome = if stream_stderr {
        let out = writing_run_stream_full(
            req.clone(),
            cancel,
            |delta| {
                eprint!("{delta}");
                let _ = std::io::stderr().flush();
            },
            "cli-offline",
        )
        .await?;
        eprintln!();
        out
    } else {
        writing_run_stream_full(req.clone(), cancel, |_| {}, "cli-offline").await?
    };
    let text = outcome.text.clone();

    let mut applied = "none".to_string();
    if apply == "append" || apply == "replace" {
        let (_meta, content) =
            crate::project::read_chapter(std::path::Path::new(&req.project_root), &req.chapter_id)?;
        let new_content = if apply == "replace" {
            if req.selection.is_empty() {
                return Err(AppError::msg("replace 需要 --selection"));
            }
            content.replacen(&req.selection, &text, 1)
        } else {
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
        };
        crate::project::write_chapter(
            std::path::Path::new(&req.project_root),
            &req.chapter_id,
            &new_content,
        )?;
        applied = apply;
    }

    Ok(json!({
        "ok": true,
        "text": text,
        "raw_text": outcome.raw_text,
        "via": "offline",
        "applied": applied,
        "model_used": outcome.model_used,
        "fallback_from": outcome.fallback_from,
        "truncated": outcome.truncated,
        "loop_retried": outcome.loop_retried
    }))
}

async fn run_rpc_loop(human: bool) -> i32 {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    eprintln!(
        "kk_novel_ai rpc ready (NDJSON). Send {{\"cmd\":\"...\"}} per line. Empty line or EOF to exit."
    );
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                print_err(human, e);
                return 1;
            }
        };
        let line = line.trim();
        if line.is_empty() || line == "exit" || line == "quit" {
            break;
        }
        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let v = json!({ "ok": false, "error": format!("JSON 解析失败: {e}") });
                let _ = writeln!(stdout, "{}", v);
                let _ = stdout.flush();
                continue;
            }
        };
        let resp = match dispatch_rpc(req).await {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e.to_string() }),
        };
        if human {
            let _ = writeln!(
                stdout,
                "{}",
                serde_json::to_string_pretty(&resp).unwrap_or_else(|_| resp.to_string())
            );
        } else {
            let _ = writeln!(stdout, "{}", resp);
        }
        let _ = stdout.flush();
    }
    0
}

fn tools_manifest() -> Value {
    json!({
        "ok": true,
        "name": "kk_novel_ai",
        "protocol": "kk_novel_ai <subcommand> | kk_novel_ai --cli <subcommand> | rpc NDJSON | GUI IPC",
        "commands": [
            {"cmd": "ping", "desc": "心跳"},
            {"cmd": "settings_get", "desc": "读取全局设置"},
            {"cmd": "settings_save", "args": ["settings"], "desc": "保存全局设置"},
            {"cmd": "llm_health", "desc": "检测 LM Studio"},
            {"cmd": "llm_list_models", "desc": "列出模型"},
            {"cmd": "llm_chat", "args": ["messages", "options?"], "desc": "非流式对话"},
            {"cmd": "project_create", "args": ["root", "title?"], "desc": "新建作品目录"},
            {"cmd": "project_open", "args": ["root"], "desc": "打开作品"},
            {"cmd": "project_get", "args": ["root"], "desc": "读取作品元数据"},
            {"cmd": "project_forget_recent", "args": ["root"], "desc": "从最近列表移除"},
            {"cmd": "project_delete", "args": ["root", "purge?"], "desc": "移除最近项；purge 时删含 project.json 的目录"},
            {"cmd": "project_forget_all_novels", "args": ["purge?"], "desc": "清空全部最近小说；可 purge 删盘"},
            {"cmd": "project_save_meta", "args": ["root", "project"], "desc": "保存元数据"},
            {"cmd": "project_suggest_title", "args": ["root"], "desc": "AI 根据内容建议书名"},
            {"cmd": "project_apply_title", "args": ["root", "title"], "desc": "写入书名并刷新最近列表"},
            {"cmd": "chapter_read", "args": ["root", "chapter_id"], "desc": "读章节"},
            {"cmd": "chapter_write", "args": ["root", "chapter_id", "content"], "desc": "写章节"},
            {"cmd": "chapter_create", "args": ["root", "title", "summary?"], "desc": "新建章节"},
            {"cmd": "chapter_delete", "args": ["root", "chapter_id"], "desc": "删除章节"},
            {"cmd": "chapter_update_meta", "args": ["root", "chapter_id", "title?", "summary?", "status?"], "desc": "更新章纲"},
            {"cmd": "lore_list", "args": ["root"], "desc": "列出设定"},
            {"cmd": "lore_list_scoped", "args": ["root"], "desc": "本篇+全局角色仓分栏列表"},
            {"cmd": "character_roster_ensure", "desc": "确保全局角色仓存在"},
            {"cmd": "project_ensure_characters_link", "args": ["root"], "desc": "作品挂接 @characters"},
            {"cmd": "lore_upsert", "args": ["root", "entry"], "desc": "写入设定（含 unique）"},
            {"cmd": "memory_upsert_block_note", "args": ["root", "chapter_id", "block_key", "summary"], "desc": "写入/覆盖块记忆"},
            {"cmd": "memory_remove_block_note", "args": ["root", "chapter_id", "block_key"], "desc": "删除块记忆"},
            {"cmd": "lore_delete", "args": ["root", "lore_id"], "desc": "删除设定"},
            {"cmd": "writing_run", "args": ["request"], "desc": "写作任务；默认走 GUI IPC，--offline 旁路"},
            {"cmd": "story_plot_get", "args": ["root"], "desc": "读故事线"},
            {"cmd": "story_plot_save", "args": ["root", "plot"], "desc": "写故事线"},
            {"cmd": "story_timeline_get", "args": ["root"], "desc": "读时间线"},
            {"cmd": "story_canon_get", "args": ["root"], "desc": "读 Canon"},
            {"cmd": "story_relations_get", "args": ["root"], "desc": "读关系图"},
            {"cmd": "story_apply_patch", "args": ["root", "patch"], "desc": "应用总谱 patch"},
            {"cmd": "story_dashboard", "args": ["root"], "desc": "叙事仪表盘"},
            {"cmd": "export_txt", "args": ["root", "output"], "desc": "导出 TXT"},
            {"cmd": "export_epub", "args": ["root", "output"], "desc": "导出 EPUB"},
            {"cmd": "export_pdf", "args": ["root", "output"], "desc": "导出 PDF"},
            {"cmd": "import_txt", "args": ["root", "file", "title?"], "desc": "导入 TXT 为知识库（kind=knowledge_base）"},
            {"cmd": "import_distill", "args": ["root", "from?", "to?", "apply?", "resume?", "job_id?", "instruction?"], "desc": "按章蒸馏知识库"},
            {"cmd": "import_apply_pending", "args": ["root", "job_id"], "desc": "应用 distill pending"},
            {"cmd": "kb_registry_list", "desc": "列出小说知识库 + 通用库"},
            {"cmd": "kb_universal_open", "desc": "打开/初始化通用知识库"},
            {"cmd": "kb_sync", "args": ["root"], "desc": "同步单书库到通用库"},
            {"cmd": "kb_sync_all", "desc": "同步全部登记库"},
            {"cmd": "kb_migrate", "args": ["root", "source_file?", "sync?"], "desc": "迁移为 knowledge_base"},
            {"cmd": "kb_universal_rebuild_rag", "desc": "重建通用库 embedding"},
            {"cmd": "stats_get", "args": ["root"], "desc": "码字统计"},
            {"cmd": "stats_set_goal", "args": ["root", "goal_chars"], "desc": "设置日目标字数"},
            {"cmd": "rag_rebuild", "args": ["root"], "desc": "重建 embedding 索引"},
            {"cmd": "gen_log_list", "args": ["limit?"], "desc": "全局生成日志"},
            {"cmd": "project_gen_log_list", "args": ["root", "limit?"], "desc": "作品目录内生成/保存履历"},
            {"cmd": "usage_summary", "args": ["root?"], "desc": "token/费用累计摘要"},
            {"cmd": "provider_balance", "args": [], "desc": "DeepSeek 账户余额（其它提供商无接口）"}
        ],
        "examples": [
            "kk_novel_ai tools",
            "kk_novel_cli kb list",
            "kk_novel_cli kb import-txt D:/kb/wendao --file test_files/x.txt --title 问道红尘",
            "kk_novel_cli kb distill D:/kb/wendao --from 1 --to 20 --apply auto",
            "kk_novel_cli kb sync D:/kb/wendao",
            "kk_novel_cli kb migrate D:/kb/old --sync",
            "kk_novel_ai rpc"
        ]
    })
}
