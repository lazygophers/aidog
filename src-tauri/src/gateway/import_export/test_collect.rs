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

// ── 平台导出三层清洗（PRD 07-01-export-extra-cleanup） ──

/// 直插一个含给定 extra 的 platform（绕过 create_platform 默认值清洗），返回 id。
/// platform_type 在 DB 中存为 JSON 序列化字符串（`serde_json::to_string`），故需带引号框。
async fn insert_platform_with_extra(db: &crate::gateway::db::Db, name: &str, extra: &str) -> i64 {
    let name = name.to_string();
    let extra = extra.to_string();
    db.write_conn().call(move |conn| {
        conn.execute(
            "INSERT INTO platform (name, platform_type, base_url, api_key, extra, created_at, updated_at)
             VALUES (?1, '\"anthropic\"', 'https://x.example.com', 'sk', ?2, 0, 0)",
            rusqlite::params![name, extra],
        )?;
        Ok(conn.last_insert_rowid())
    })
    .await
    .unwrap()
}

/// 导出清洗：空 extra (`{}` / `""`) 平台无 extra 字段；无运行时字段；无 status / enabled。
#[tokio::test]
async fn collect_platform_strips_empty_extra_and_runtime() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    insert_platform_with_extra(&db, "empty-braces", "{}").await;
    insert_platform_with_extra(&db, "empty-string", "").await;

    let p = collect::collect(&db, &[SCOPE_PLATFORM.to_string()]).await.unwrap();
    assert_eq!(p.platform.len(), 2, "应收集 2 个平台");

    for plat in &p.platform {
        let obj = plat.as_object().expect("platform 是 obj");
        // 空 extra → 字段省略。
        assert!(obj.get("extra").is_none(), "空 extra 应省略: {plat}");
        // 运行时不导出。
        for k in [
            "auto_disabled_until",
            "auto_disable_strikes",
            "expires_at",
            "deleted_at",
            "est_balance_remaining",
            "est_coding_plan",
            "last_real_query_at",
            "last_error",
            "last_error_at",
        ] {
            assert!(!obj.contains_key(k), "运行时字段 {k} 不应导出: {plat}");
        }
        // status / enabled 不导出（分享不带原用户启用意图）。
        assert!(!obj.contains_key("status"), "status 不应导出: {plat}");
        assert!(!obj.contains_key("enabled"), "enabled 不应导出: {plat}");
        // 配置空值省略（models / available_models / endpoints 缺省）。
        assert!(!obj.contains_key("models"), "空 models 应省略: {plat}");
        assert!(
            !obj.contains_key("available_models"),
            "空 available_models 应省略: {plat}"
        );
        assert!(!obj.contains_key("endpoints"), "空 endpoints 应省略: {plat}");
        // 核心配置字段保留。
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("platform_type"));
        assert!(obj.contains_key("base_url"));
        assert!(obj.contains_key("api_key"));
    }
}

/// 导出清洗：非空 extra 序列化为 JSON object（非裸 string）。
#[tokio::test]
async fn collect_platform_extra_as_object() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    insert_platform_with_extra(
        &db,
        "with-extra",
        r#"{"breaker":{"failure_threshold":5,"open_secs":30,"half_open_max":2}}"#,
    )
    .await;

    let p = collect::collect(&db, &[SCOPE_PLATFORM.to_string()]).await.unwrap();
    assert_eq!(p.platform.len(), 1);
    let plat = &p.platform[0];
    let extra = plat.get("extra").expect("非空 extra 应保留");
    assert!(
        extra.is_object(),
        "extra 应为 JSON object 非 string: {extra}"
    );
    assert_eq!(extra["breaker"]["failure_threshold"], 5);
    assert_eq!(extra["breaker"]["open_secs"], 30);
    assert_eq!(extra["breaker"]["half_open_max"], 2);
}

/// 导出清洗：非法 extra JSON → 兜底省略（design 决策）。
#[tokio::test]
async fn collect_platform_invalid_extra_falls_back_to_omit() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    insert_platform_with_extra(&db, "bad-json", "not-valid-json").await;

    let p = collect::collect(&db, &[SCOPE_PLATFORM.to_string()]).await.unwrap();
    assert_eq!(p.platform.len(), 1);
    let plat = &p.platform[0];
    assert!(
        plat.get("extra").is_none(),
        "非法 extra 应兜底省略: {plat}"
    );
}

/// 导出清洗：仅空白字符的 extra → 省略。
#[tokio::test]
async fn collect_platform_whitespace_only_extra_omitted() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    insert_platform_with_extra(&db, "ws-only", "   ").await;

    let p = collect::collect(&db, &[SCOPE_PLATFORM.to_string()]).await.unwrap();
    assert_eq!(p.platform.len(), 1);
    assert!(p.platform[0].get("extra").is_none());
}
