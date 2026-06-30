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
async fn collect_new_scopes_roundtrip_and_key_consistency() {
    use crate::gateway::mcp::McpServerRow;
    use crate::gateway::models::CreateMiddlewareRule;

    let _h = HomeGuard::new();
    let db = test_db().await;

    // mcp
    crate::gateway::db::upsert_mcp_server(
        &db,
        &McpServerRow {
            id: 0,
            name: "ctx7".into(),
            transport: "stdio".into(),
            command: "npx".into(),
            args_json: "[]".into(),
            env_json: "{}".into(),
            url: String::new(),
            headers_json: "{}".into(),
            enabled_agents: "claude-code".into(),
            created_at: 1,
            updated_at: 1,
        },
    )
    .await
    .unwrap();

    // middleware
    crate::gateway::db::create_middleware_rule(
        &db,
        CreateMiddlewareRule {
            name: "blockfoo".into(),
            description: "d".into(),
            rule_type: crate::gateway::models::RuleType::RequestFilter,
            scope: crate::gateway::models::RuleScope::Global,
            scope_ref: String::new(),
            match_type: crate::gateway::models::MatchType::Contains,
            pattern: "foo".into(),
            action: crate::gateway::models::RuleAction::Warn,
            config: "{}".into(),
            priority: 0,
            enabled: true,
            is_builtin: false,
        },
    )
    .await
    .unwrap();

    // model_price
    crate::gateway::db::upsert_model_price(
        &db,
        "gpt-test",
        "manual",
        r#"{"input_cost_per_token":0.000001}"#,
        Some(1000),
        Some(2000),
        Some(8000),
        )
    .await
    .unwrap();

    let scopes: Vec<String> = [SCOPE_MCP, SCOPE_MIDDLEWARE, SCOPE_MODEL_PRICE]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let p = collect::collect(&db, &scopes).await.unwrap();
    let has_model = |pl: &super::Payload, name: &str| {
        pl.model_price
            .iter()
            .any(|m| m.get("model_name").and_then(|v| v.as_str()) == Some(name))
    };
    let has_mw = |pl: &super::Payload, name: &str| {
        pl.middleware
            .iter()
            .any(|m| m.get("name").and_then(|v| v.as_str()) == Some(name))
    };
    assert_eq!(p.mcp.len(), 1);
    assert!(has_mw(&p, "blockfoo"));
    assert!(has_model(&p, "gpt-test"));

    // 序列化往返一致（含新字段，校验 checksum）
    let mut p2 = p.clone();
    let bytes = p2.serialize_with_checksum().unwrap();
    let back = super::Payload::from_bytes_verified(&bytes).unwrap();
    assert_eq!(back.mcp.len(), 1);
    assert!(has_mw(&back, "blockfoo"));
    assert!(has_model(&back, "gpt-test"));

    // build_items 造的 (scope,key) 必含新 scope 稳定键
    let items = super::apply::export_items(&p);
    let has = |scope: &str, key: &str| items.iter().any(|i| i.scope == scope && i.key == key);
    assert!(has(SCOPE_MCP, "idx:0"));
    assert!(has(SCOPE_MIDDLEWARE, "idx:0"));
    assert!(has(SCOPE_MODEL_PRICE, "model:gpt-test"));

    // filter_payload selection 命中新 scope key → 仅保留选中项
    let mut sel = std::collections::BTreeSet::new();
    sel.insert((SCOPE_MCP.to_string(), "idx:0".to_string()));
    let mut filtered = p.clone();
    super::apply::filter_payload(&mut filtered, Some(&sel));
    assert_eq!(filtered.mcp.len(), 1);
    assert_eq!(filtered.middleware.len(), 0);
    assert_eq!(filtered.model_price.len(), 0);
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
