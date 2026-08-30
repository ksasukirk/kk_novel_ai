//! 控制台 CLI 入口（无 windows_subsystem，stdout 稳定）
//! 代码路径: kk_novel_ai/src-tauri/src/bin/kk_novel_cli.rs

fn main() {
    let code = kk_novel_ai_lib::cli::run();
    std::process::exit(code);
}
