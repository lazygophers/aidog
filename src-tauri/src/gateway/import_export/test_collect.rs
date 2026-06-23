//! 导出收集器覆盖：各 scope 从 DB + FS 组装 Payload。HOME 隔离保证 codex/claude_code/skills 读 tempdir。
use super::*;
use crate::gateway::db::test_support::{sample_platform, test_db, HomeGuard};

fn all_scopes() -> Vec<String> {
    [
        SCOPE_PLATFORM,
        SCOPE_GROUP,
        SCOPE_GROUP_PLATFORM,
        SCOPE_SETTING,
        SCOPE_CODEX,
        SCOPE_CLAUDE_CODE,
        SCOPE_SKILLS,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[tokio::test]
async fn collect_empty_scopes_yields_empty_payload() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    let p = collect::collect(&db, &[]).await.unwrap();
    assert!(p.platform.is_empty());
    assert!(p.group.is_empty());
    assert_eq!(p.manifest.format_version, 1);
    assert!(!p.manifest.aidog_version.is_empty());
}

#[tokio::test]
async fn collect_platform_and_group_and_pairs() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    let plat = crate::gateway::db::create_platform(&db, sample_platform("p"))
        .await
        .unwrap();
    let grp = crate::gateway::db::create_group(
        &db,
        crate::gateway::db::test_support::sample_group("g", vec![]),
    )
    .await
    .unwrap();
    crate::gateway::db::set_group_platforms(
        &db,
        grp.id,
        &[crate::gateway::models::GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();

    let p = collect::collect(&db, &all_scopes()).await.unwrap();
    assert_eq!(p.platform.len(), 1);
    assert_eq!(p.group.len(), 1);
    assert_eq!(p.group_platform.len(), 1);
    // skills scope (无 ~/.claude/skills) → 空
    assert!(p.skills.is_empty());
    assert_eq!(p.manifest.scopes.len(), 7);
}

#[tokio::test]
async fn collect_settings_scope() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    crate::gateway::db::set_setting(
        &db,
        crate::gateway::models::SetSettingInput {
            scope: "app".into(),
            key: "locale".into(),
            value: serde_json::json!({"locale": "zh-CN"}),
        },
    )
    .await
    .unwrap();
    let p = collect::collect(&db, &[SCOPE_SETTING.to_string()])
        .await
        .unwrap();
    assert!(p.setting.iter().any(|[s, k, _]| s == "app" && k == "locale"));
}

#[tokio::test]
async fn collect_claude_code_global_reads_file() {
    let h = HomeGuard::new();
    let db = test_db().await;
    let claude_dir = h.home().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), r#"{"model":"opus"}"#).unwrap();

    let p = collect::collect(&db, &[SCOPE_CLAUDE_CODE.to_string()])
        .await
        .unwrap();
    assert!(p.claude_code_global.as_deref().unwrap().contains("opus"));
}
