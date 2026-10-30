use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "src/preset_filter.rs"]
mod preset_filter;

/// 枚举 `defaults/<sub>/*.json`（各 kind 预设目录），按 `GITHUB_ACTIONS` 过滤后返回 `include_str!` 清单源码。
fn gen_presets(sub: &str, static_name: &str) -> String {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../defaults")
        .join(sub)
        .canonicalize()
        .unwrap_or_else(|e| panic!("defaults/{sub} dir: {e}"));
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read defaults/{sub}: {e}"))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
    }
    let stem = |p: &PathBuf| p.file_stem().unwrap().to_string_lossy().into_owned();
    let only_oss = std::env::var("GITHUB_ACTIONS").is_ok_and(|v| v == "true");
    let ids = preset_filter::select_ids(files.iter().map(stem).collect(), only_oss)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let mut src = format!("pub static {static_name}: &[(&str, &str)] = &[\n");
    for f in files.iter().filter(|f| ids.contains(&stem(f))) {
        src.push_str(&format!("    ({:?}, include_str!({f:?})),\n", stem(f)));
    }
    src.push_str("];\n");
    src
}

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_ACTIONS");
    let src = gen_presets("settings", "CLAUDE_PRESETS")
        + &gen_presets("codex", "CODEX_PRESETS")
        + &gen_presets("pi", "PI_PRESETS");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("claude_presets.rs");
    std::fs::write(out, src).expect("write claude_presets.rs");

    // git 短 commit（失败回退 "unknown"）—— about.rs 用，同 root build.rs 逻辑（C3 迁入后本 crate 自持）。
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
    println!("cargo:rerun-if-changed=../../../.git/HEAD");
}
