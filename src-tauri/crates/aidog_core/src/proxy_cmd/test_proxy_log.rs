#![cfg(test)]
use super::*;
use crate::gateway::db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State 走 command 包装层，直测 command 转发的 gateway:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn list_count_get_clear_flow() {
    let db = test_db().await;

    assert_eq!(gateway::db::count_proxy_logs(&db).await.unwrap(), 0);
    assert!(gateway::db::list_proxy_logs(&db, 10, 0).await.unwrap().is_empty());
    assert!(gateway::db::get_proxy_log(&db, "none").await.unwrap().is_none());

    let filter = ProxyLogFilter::default();
    assert!(gateway::db::filtered_list_proxy_logs(&db, &filter, 10, 0).await.unwrap().items.is_empty());
    assert_eq!(gateway::db::filtered_count_proxy_logs(&db, &filter).await.unwrap(), 0);

    gateway::db::clear_proxy_logs(&db).await.unwrap();
}

#[tokio::test]
async fn distinct_models_empty() {
    let db = test_db().await;

    let filter = ProxyLogFilter { exclude_sources: Some(vec!["test".into(), "quota".into()]), ..Default::default() };
    assert!(gateway::db::distinct_models_proxy_log(&db, &filter, false, 200).await.unwrap().is_empty());
    assert!(gateway::db::distinct_models_proxy_log(&db, &filter, true, 200).await.unwrap().is_empty());
}

#[tokio::test]
async fn usage_stats_endpoints() {
    let db = test_db().await;

    let _ = gateway::db::get_platform_usage_stats(&db, 1).await.unwrap();
    let _ = gateway::db::get_group_usage_stats(&db, "gk").await.unwrap();
    assert!(gateway::db::get_all_group_usage_stats(&db).await.unwrap().is_empty());
    assert!(gateway::db::platform_usage_stats_all(&db).await.unwrap().is_empty());
    assert!(gateway::db::get_last_test_result(&db, 1).await.unwrap().is_none());
}

#[tokio::test]
async fn log_settings_roundtrip() {
    let db = test_db().await;

    // default
    let _: ProxyLogSettings = gateway::db::get_setting(&db, "proxy", "logging").await
        .ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    // set with retention cleanup branches exercised
    let settings = ProxyLogSettings {
        retention_days: 30,
        user_request_retention_days: 7,
        upstream_request_retention_days: 7,
        ..Default::default()
    };
    let value = serde_json::to_value(&settings).unwrap();
    gateway::db::set_setting(&db, gateway::models::SetSettingInput {
        scope: "proxy".into(), key: "logging".into(), value,
    }).await.unwrap();
    run_retention_cleanup(&db, &settings).await;
    let got: ProxyLogSettings = gateway::db::get_setting(&db, "proxy", "logging").await
        .ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    assert_eq!(got.retention_days, 30);
}
