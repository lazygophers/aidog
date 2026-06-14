use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // git 短 commit（失败回退 "unknown"）
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=AIDOG_GIT_COMMIT={commit}");

    // 构建时间（epoch 秒，前端格式化；std 无新依赖）
    let build_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!("cargo:rustc-env=AIDOG_BUILD_TIME={build_secs}");

    // commit 变化时重跑（HEAD 移动触发重新注入）
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../.git/HEAD");

    tauri_build::build()
}
