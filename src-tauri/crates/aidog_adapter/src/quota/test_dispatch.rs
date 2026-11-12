//! query_quota 入口 dispatcher 覆盖：空 key / 不支持平台 早退分支。
//! provider 具体查询走 registry 脚本内固定 host，无法 stub（需真实网络），故仅覆盖
//! 可达的 host 路由判定早退。脚本等价断言见 test_balance / test_coding_plan /
//! test_special_scripts（stub + host retarget）。
use super::*;
use aidog_db::models::{CreatePlatform, Protocol};

#[tokio::test]
async fn draft_magpie_account_is_used_without_persisting_platform_settings() {
    let (db, _temp_dir) = aidog_db::test_support::three_file_db().await;
    let (stub, requests) = crate::quota::test_stub::spawn_capture(vec![
        ("/", 200, r#"{"data":[{"name":"draft-account","kind":"balance","balance":"12 USD"}]}"#),
    ])
    .await;
    let platform = aidog_db::create_platform(
        &db,
        CreatePlatform {
            name: "Magpie test".into(),
            platform_type: Protocol::Magpie,
            base_url: stub.clone(),
            api_key: "test-key".into(),
            extra: r#"{"magpie_name":"saved-account"}"#.into(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: Some(false),
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let draft = HashMap::from([("magpie_name".into(), "draft-account".into())]);

    let result = query_quota_with_draft_requires(
        Some(&Arc::new(db.clone())),
        &stub,
        "test-key",
        platform.id as i64,
        Some(&draft),
    )
    .await;

    assert!(result.success, "{:?}", result.error);
    assert_eq!(requests.lock().unwrap()[0].path, "/v1/magpie/quotas?kept=1");
    let saved = aidog_db::get_platform(&db, platform.id).await.unwrap().unwrap();
    let extra: serde_json::Value = serde_json::from_str(&saved.extra).unwrap();
    assert_eq!(extra["magpie_name"], "saved-account");
}

#[tokio::test]
async fn empty_api_key_errors() {
    let q = query_quota(None, "https://api.deepseek.com", "  ", 0).await;
    assert!(!q.success);
    assert!(q.error.as_deref().unwrap().contains("API key"));
}

#[tokio::test]
async fn unsupported_platform_errors() {
    let q = query_quota(None, "https://unknown.example.com/v1", "sk-x", 0).await;
    assert!(!q.success);
    assert!(q.error.as_deref().unwrap().contains("Unsupported"));
}

#[tokio::test]
async fn query_quota_for_unsupported_protocol_falls_back_to_heuristic() {
    use aidog_db::models::Protocol;
    // 无 quota 脚本的协议 → 回落 base_url 启发式 → 无命中关键词 → Unsupported
    let q = query_quota_for(
        None,
        &Protocol::OpenAI,
        "https://unknown.example.com/v1",
        "sk-x",
        0,
    )
    .await;
    assert!(!q.success);
    assert!(q.error.as_deref().unwrap().contains("Unsupported"));
}
