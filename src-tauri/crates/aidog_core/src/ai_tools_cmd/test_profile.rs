//! profile 快照单元测试（随源文件 profile.rs 1:1，magpie 票 06）。
//!
//! HOME / CODEX_HOME / PI_CODING_AGENT_DIR 全部指向 tempdir（HomeGuard），
//! 不碰真家目录、不真实外呼。验收线（票面）：save→改乱→apply→逐文件与快照
//! 一致（含权限位）；apply 中途失败不产生半写文件；同名 overwrite 语义。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use aidog_db::test_support::{HomeGuard, sample_group, sample_platform, three_file_db};
use aidog_db::GroupPlatformInput;
use serde_json::json;

use aidog_hooks::SCRIPT_COMPLETE as SCRIPT_NAME;

use super::{apply_profile, delete_profile, list_profiles, save_profile};

fn home() -> PathBuf {
    std::env::var("HOME").unwrap().into()
}

fn codex_home() -> PathBuf {
    std::env::var("CODEX_HOME").unwrap().into()
}

fn pi_dir() -> PathBuf {
    std::env::var("PI_CODING_AGENT_DIR").unwrap().into()
}

fn read_text(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn write_text(p: &Path, s: &str) {
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, s).unwrap();
}

#[cfg(unix)]
fn mode_of(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).unwrap().permissions().mode()
}

#[cfg(unix)]
fn set_mode(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
}

const DOT_JSON: &str = r#"{
  "ANTHROPIC_BASE_URL": "http://127.0.0.1:9890/claude",
  "mcpServers": {
    "aidog": {
      "command": "aidog"
    }
  },
  "userKey": "keep-me"
}"#;

/// 构造「多 agent 配置 + 分组映射」现场并 save 到 `name`，返回快照文件路径。
/// db 由调用方持有（apply 须在同一库上验证分组还原）。
async fn seed_and_save(db: &aidog_db::Db, name: &str) -> PathBuf {
    let h = home();

    // agent 配置文件：claude / codex / pi / aidog 自有面
    write_text(&h.join(".claude.json"), DOT_JSON);
    write_text(
        &h.join(".claude/settings.json"),
        &serde_json::to_string_pretty(&json!({"env": {"CLAUDE_AIDOG": "1"}, "model": "opus"}))
            .unwrap(),
    );
    write_text(
        &codex_home().join("config.toml"),
        "model_provider = \"aidog\"\n\n[model_providers.aidog]\nbase_url = \"http://127.0.0.1:9890/codex\"\n",
    );
    write_text(
        &codex_home().join("team.config.toml"),
        "model_provider = \"aidog\"\n\n[model_providers.aidog]\nbase_url = \"http://127.0.0.1:9890/codex-team\"\n",
    );
    write_text(&pi_dir().join("models.json"), "{\"providers\":{\"aidog-g1\":{}}}");
    let script = h.join(".aidog").join("scripts").join(SCRIPT_NAME);
    write_text(&script, "#!/bin/sh\necho aidog\n");
    set_mode(&script, 0o755);

    // 分组 + 平台 + 映射（平台经 create_platform 走真实写入路径）
    let p = aidog_db::create_platform(db, {
        let mut input = sample_platform("plat-a");
        input.auto_group = Some(false);
        input
    })
    .await
    .unwrap();
    let g = aidog_db::create_group(db, sample_group("g1", Vec::new()))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        db,
        g.id,
        &[GroupPlatformInput {
            platform_id: p.id,
            priority: None,
            weight: None,
            level_priority: None,
        }],
    )
    .await
    .unwrap();

    let meta = save_profile(db, name, false).await.unwrap();
    assert_eq!(meta.name, name);
    home()
        .join(".aidog")
        .join("profiles")
        .join(format!("{name}.json"))
}

/// 同名 save 默认拒绝，显式 overwrite 才覆盖；非法名拒绝；快照带 schema_version。
#[tokio::test]
async fn save_requires_explicit_overwrite() {
    let _h = HomeGuard::new();
    let (db, _dir) = three_file_db().await;

    save_profile(&db, "p1", false).await.unwrap();
    let err = save_profile(&db, "p1", false).await.unwrap_err();
    assert!(err.contains("exists"), "同名默认拒绝，实际: {err}");
    save_profile(&db, "p1", true).await.unwrap();
    assert_eq!(list_profiles().unwrap().len(), 1, "overwrite 后仍是一份");

    for bad in ["", "../evil", "a/b", "a\\b", "."] {
        assert!(
            save_profile(&db, bad, false).await.is_err(),
            "非法名应拒绝: {bad:?}"
        );
    }
    let raw = read_text(&home().join(".aidog/profiles/p1.json"));
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(v["schema_version"], 1, "快照必带 schema_version");
}

/// save → 改乱 → dry-run 报差异不落盘 → apply 逐文件还原（含权限位）+ 分组映射还原。
#[tokio::test]
async fn apply_roundtrip_restores_files_groups_and_permissions() {
    let _h = HomeGuard::new();
    let (db, _dir) = three_file_db().await;
    let _snapshot = seed_and_save(&db, "work").await;
    let h = home();

    // 改乱：改内容、删文件、加多余文件、加多余分组
    write_text(&h.join(".claude.json"), "{\"ANTHROPIC_BASE_URL\":\"broken\",\"junk\":true}");
    std::fs::remove_file(codex_home().join("config.toml")).unwrap();
    let script = h.join(".aidog").join("scripts").join(SCRIPT_NAME);
    std::fs::remove_file(&script).unwrap();
    write_text(&h.join(".aidog/settings.extra.json"), "{\"orphan\":true}");
    let p2 = aidog_db::create_platform(&db, {
        let mut input = sample_platform("plat-b");
        input.auto_group = Some(false);
        input
    })
    .await
    .unwrap();
    let g2 = aidog_db::create_group(&db, sample_group("g2", Vec::new()))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &db,
        g2.id,
        &[GroupPlatformInput {
            platform_id: p2.id,
            priority: None,
            weight: None,
            level_priority: None,
        }],
    )
    .await
    .unwrap();

    // dry-run：报差异且不落盘
    let before_files = snapshot_tree(&h);
    let diff = apply_profile(&db, "work", true).await.unwrap();
    let targets: BTreeSet<String> = diff.iter().map(|d| d.target.clone()).collect();
    assert!(targets.contains("claude_dot_json"), "dry-run 应报 .claude.json 改动");
    assert!(targets.contains("codex_config_toml"), "dry-run 应报 codex config 被删");
    assert!(
        targets.contains(&format!("aidog_script:{SCRIPT_NAME}")),
        "dry-run 应报脚本被删"
    );
    assert!(
        targets.contains("aidog_settings:settings.extra.json"),
        "dry-run 应报多余文件"
    );
    assert!(targets.contains("db:groups"), "dry-run 应报分组差异");
    assert_eq!(snapshot_tree(&h), before_files, "dry-run 不得落盘");

    // apply：逐文件与快照一致（byte 级）+ 权限位一致
    let applied = apply_profile(&db, "work", false).await.unwrap();
    assert!(!applied.is_empty());
    assert_eq!(read_text(&h.join(".claude.json")), DOT_JSON, ".claude.json 应 byte 级还原");
    assert!(codex_home().join("config.toml").exists(), "被删的 codex config 应还原");
    assert!(script.exists(), "被删的 hook 脚本应还原");
    #[cfg(unix)]
    assert_eq!(mode_of(&script) & 0o777, 0o755, "权限位应与快照一致");
    assert!(
        !h.join(".aidog/settings.extra.json").exists(),
        "快照没有的多余文件应删除"
    );

    // 分组映射还原：g1 在且链回 plat-a，g2 消失
    let groups = aidog_db::list_groups(&db).await.unwrap();
    let keys: Vec<&str> = groups.iter().map(|g| g.group_key.as_str()).collect();
    assert!(keys.contains(&"g1"), "快照分组 g1 应在");
    assert!(!keys.contains(&"g2"), "快照外分组 g2 应删除");
    let pairs = aidog_db::list_all_group_platform_pairs(&db).await.unwrap();
    assert!(
        pairs.contains(&("g1".to_string(), "plat-a".to_string())),
        "g1↔plat-a 应还原"
    );
    assert!(!pairs.iter().any(|(g, _)| g == "g2"), "g2 的映射应消失");
}

/// apply 中途单文件失败：报 error、其余文件完整还原、无临时文件残留。
#[tokio::test]
async fn apply_failure_leaves_no_half_written_files() {
    let _h = HomeGuard::new();
    let (db, _dir) = three_file_db().await;
    let _snapshot = seed_and_save(&db, "work").await;
    let h = home();

    // 破坏现场：把 ~/.claude 目录换成普通文件 → claude_settings_json 的
    // create_dir_all 必失败（其余文件路径不受影响）
    std::fs::remove_dir_all(h.join(".claude")).unwrap();
    std::fs::write(h.join(".claude"), "not a dir").unwrap();
    write_text(&h.join(".claude.json"), "{\"broken\":1}");

    let applied = apply_profile(&db, "work", false)
        .await
        .unwrap_or_else(|e| panic!("apply 应 best-effort 完成其余面而非整体失败: {e}"));
    assert!(
        applied
            .iter()
            .any(|d| d.target.contains("claude_settings_json") && d.error.is_some()),
        "失败面应带 error: {applied:?}"
    );
    let dot_json = read_text(&h.join(".claude.json"));
    assert!(dot_json.contains("userKey"), "其它文件应完整还原: {dot_json}");
    assert!(codex_home().join("config.toml").exists());
    let strays: Vec<PathBuf> = walk(&h)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".aidogtmp"))
                .unwrap_or(false)
        })
        .collect();
    assert!(strays.is_empty(), "不应残留临时文件: {strays:?}");
}

/// delete 删除快照文件；apply 不存在的名报错。
#[tokio::test]
async fn delete_removes_profile_and_apply_missing_errors() {
    let _h = HomeGuard::new();
    let (db, _dir) = three_file_db().await;
    save_profile(&db, "gone", false).await.unwrap();
    assert_eq!(list_profiles().unwrap().len(), 1);
    delete_profile("gone").unwrap();
    assert!(list_profiles().unwrap().is_empty());
    assert!(!home().join(".aidog/profiles/gone.json").exists());
    assert!(
        apply_profile(&db, "gone", true).await.is_err(),
        "apply 不存在的方案应报错"
    );
}

// ─── helpers ───────────────────────────────────────────────────

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

fn snapshot_tree(root: &Path) -> BTreeSet<String> {
    walk(root)
        .into_iter()
        .filter_map(|p| {
            let rel = p.strip_prefix(root).ok()?.to_string_lossy().to_string();
            let body = std::fs::read(&p).ok()?;
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                std::fs::metadata(&p).ok()?.permissions().mode() & 0o777
            };
            #[cfg(not(unix))]
            let mode = 0u32;
            Some(format!("{rel}:{mode}:{}", file_sha(&body)))
        })
        .collect()
}

fn file_sha(b: &[u8]) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(b);
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}
