use std::env;

fn main() {
    let build_date = env::var("BUILD_DATE").unwrap_or_else(|_| "dev-build".to_string());
    let build_version = env::var("BUILD_VERSION").unwrap_or_else(|_| "0.1.0".to_string());

    println!("cargo:rustc-env=BUILD_DATE={build_date}");
    println!("cargo:rustc-env=BUILD_VERSION={build_version}");
    println!("cargo:rerun-if-env-changed=BUILD_DATE");
    println!("cargo:rerun-if-env-changed=BUILD_VERSION");

    tauri_build::build()
}
