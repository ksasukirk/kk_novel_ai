// release 构建时隐藏 Windows 控制台窗口（GUI）
// CLI 模式会在 cli::run 内 AttachConsole 挂接父终端
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! 主入口：无参数启动 GUI；带子命令或 `--cli` 进入 CLI
//! 代码路径: kk_novel_ai/src-tauri/src/main.rs

fn main() {
    if kk_novel_ai_lib::cli::should_run_cli() {
        let code = kk_novel_ai_lib::cli::run();
        std::process::exit(code);
    }
    kk_novel_ai_lib::run();
}
