#![cfg(test)]
use super::*;

#[test]
fn extract_version_plain() {
    assert_eq!(extract_version("2.1.156"), Some("2.1.156".to_string()));
}

#[test]
fn extract_version_with_label() {
    assert_eq!(
        extract_version("claude 2.1.156 (build abc)"),
        Some("2.1.156".to_string())
    );
}

#[test]
fn extract_version_prerelease() {
    assert_eq!(
        extract_version("2.1.156-beta.1"),
        Some("2.1.156-beta.1".to_string())
    );
}

#[test]
fn extract_version_codex_timestamp_patch() {
    // codex 时间戳式 patch（参考 research cli-install-tech.md）
    assert_eq!(
        extract_version("0.1.2505172116"),
        Some("0.1.2505172116".to_string())
    );
}

#[test]
fn extract_version_missing_dots_not_matched() {
    // 只有一个 dot 不算版本号
    assert_eq!(extract_version("v1.2"), None);
}

#[test]
fn extract_version_none() {
    assert_eq!(extract_version("no version here"), None);
}

#[test]
fn infer_source_nvm() {
    assert_eq!(
        infer_source("/home/u/.nvm/versions/node/v20/bin/claude"),
        "nvm"
    );
}

#[test]
fn infer_source_homebrew_cellar() {
    // Homebrew Cellar 真身须先于通用规则命中
    assert_eq!(
        infer_source("/opt/homebrew/Cellar/claude/1.0/bin/claude"),
        "homebrew"
    );
}

#[test]
fn infer_source_homebrew_bin() {
    assert_eq!(infer_source("/opt/homebrew/bin/claude"), "homebrew");
}

#[test]
fn infer_source_volta() {
    assert_eq!(infer_source("/Users/u/.volta/bin/claude"), "volta");
}

#[test]
fn infer_source_native_installer() {
    assert_eq!(
        infer_source("/Users/u/.local/share/claude/versions/2.1.156/claude"),
        "native"
    );
}

#[test]
fn infer_source_system_default() {
    assert_eq!(infer_source("/usr/local/bin/claude"), "system");
}

#[test]
fn infer_source_windows_backslash() {
    assert_eq!(
        infer_source(r"C:\Users\u\scoop\shims\claude.exe"),
        "scoop"
    );
}

#[test]
fn is_conflicting_single_install() {
    let installs = vec![CliInstallation {
        path: "/a".into(),
        version: Some("1.0.0".into()),
        runnable: true,
        source: "system".into(),
        is_path_default: true,
    }];
    assert!(!is_conflicting(&installs));
}

#[test]
fn is_conflicting_two_same_version_both_runnable_no_conflict() {
    // 同版本装两份且都能跑不算冲突（不打扰用户）
    let installs = vec![
        CliInstallation {
            path: "/a".into(),
            version: Some("1.0.0".into()),
            runnable: true,
            source: "system".into(),
            is_path_default: true,
        },
        CliInstallation {
            path: "/b".into(),
            version: Some("1.0.0".into()),
            runnable: true,
            source: "nvm".into(),
            is_path_default: false,
        },
    ];
    assert!(!is_conflicting(&installs));
}

#[test]
fn is_conflicting_version_divergence() {
    let installs = vec![
        CliInstallation {
            path: "/a".into(),
            version: Some("1.0.0".into()),
            runnable: true,
            source: "system".into(),
            is_path_default: true,
        },
        CliInstallation {
            path: "/b".into(),
            version: Some("2.0.0".into()),
            runnable: true,
            source: "nvm".into(),
            is_path_default: false,
        },
    ];
    assert!(is_conflicting(&installs));
}

#[test]
fn is_conflicting_runnable_mixed() {
    let installs = vec![
        CliInstallation {
            path: "/a".into(),
            version: Some("1.0.0".into()),
            runnable: true,
            source: "system".into(),
            is_path_default: true,
        },
        CliInstallation {
            path: "/b".into(),
            version: None,
            runnable: false,
            source: "nvm".into(),
            is_path_default: false,
        },
    ];
    assert!(is_conflicting(&installs));
}

#[test]
fn tools_only_claude_and_codex() {
    // MVP 范围裁剪：仅 claude + codex
    assert_eq!(TOOLS, &["claude", "codex"]);
}
