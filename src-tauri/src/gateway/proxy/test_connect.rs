//! CONNECT 隧道自检（ponytail：非平凡分支逻辑留一个最小可运行检查）。
//! 覆盖 host 匹配命中 / 未命中 + upsert_connect_log 落行（source_protocol=http-connect）。
use super::*;
use crate::gateway::db::test_support;
use crate::gateway::models::{CreatePlatform, Protocol};

/// match_platform_by_host：主 base_url host 命中 → 返回 platform_id；未命中 → None。
#[tokio::test]
async fn match_platform_by_host_hits_main_base_url() {
    let db = test_support::test_db().await;
    // 平台 base_url host = api.test-connect-hit.example
    let p = crate::gateway::db::create_platform(&db, CreatePlatform {
        name: "conn-hit".into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://api.test-connect-hit.example/v1".into(),
        api_key: "sk-test".into(),
        extra: String::new(),
        models: None,
        available_models: None,
        endpoints: None,
        manual_budgets: None,
        auto_group: None,
        join_group_ids: None,
        default_level_priority: None,
        expires_at: None,
    }).await.expect("create platform");

    let hit = endpoint::match_platform_by_host(&db, "api.test-connect-hit.example").await;
    assert_eq!(hit, Some(p.id), "CONNECT host 命中平台主 base_url 必须返回 platform_id");

    let miss = endpoint::match_platform_by_host(&db, "api.does-not-exist.example").await;
    assert!(miss.is_none(), "未命中任何平台 base_url host 必须返回 None");
}

/// upsert_connect_log：落一行 proxy_log，source_protocol=http-connect + tokens/cost=0。
/// 关键不变量：不走 upsert_log（不污染 stats_agg），字段语义正确。
#[tokio::test]
async fn upsert_connect_log_writes_http_connect_row() {
    let db = test_support::test_db().await;
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(crate::gateway::middleware::MiddlewareEngine::default()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
    });

    log::upsert_connect_log(
        &state, "conn-log-1".into(), String::new(), 0,
        "api.example.com:443".into(), 200, 42,
    ).await;

    let row = crate::gateway::db::get_proxy_log(&state.db, "conn-log-1").await
        .expect("query proxy_log").expect("row must exist");
    assert_eq!(row.source_protocol, "http-connect", "source_protocol 标记隧道");
    assert_eq!(row.target_protocol, "http-connect");
    assert_eq!(row.platform_id, 0, "未命中 → platform_id=0");
    assert_eq!(row.status_code, 200);
    assert_eq!(row.duration_ms, 42);
    assert_eq!(row.input_tokens, 0, "隧道不计费");
    assert_eq!(row.est_cost, 0.0);
    assert_eq!(row.request_url, "api.example.com:443");
}
