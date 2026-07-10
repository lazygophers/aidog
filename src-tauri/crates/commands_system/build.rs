use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // about_info env! 需 AIDOG_GIT_COMMIT / AIDOG_BUILD_TIME 编译期注入。
    // 与 root build.rs 同口径（C6 about.rs 迁入本 crate，env! 跨 crate 不传递，需本 crate 独立注入）。
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=AIDOG_GIT_COMMIT={commit}");

    let build_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!("cargo:rustc-env=AIDOG_BUILD_TIME={build_secs}");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../../.git/HEAD");
}
