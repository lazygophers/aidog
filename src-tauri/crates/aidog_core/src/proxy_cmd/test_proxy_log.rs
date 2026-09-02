#![cfg(test)]
use super::*;
use aidog_db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State 走 command 包装层，直测 command 转发的 gateway:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn list_count_get_clear_flow() {
    let db = test_db().await;

    assert_eq!(aidog_db::count_proxy_logs(&db).await.unwrap(), 0);
    assert!(
        aidog_logs::list_proxy_logs(&db, 10, 0)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        aidog_logs::get_proxy_log(&db, "none")
            .await
            .unwrap()
            .is_none()
    );

    let filter = ProxyLogFilter::default();
    assert!(
        aidog_logs::filtered_list_proxy_logs(&db, &filter, 10, 0)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        aidog_logs::filtered_count_proxy_logs(&db, &filter)
            .await
            .unwrap(),
        0
    );

    aidog_logs::clear_proxy_logs(&db).await.unwrap();
}

#[tokio::test]
async fn distinct_models_empty() {
    let db = test_db().await;

    let filter = ProxyLogFilter {
        exclude_sources: Some(vec!["test".into(), "quota".into()]),
        ..Default::default()
    };
    assert!(
        aidog_logs::distinct_models_proxy_log(&db, &filter, false, 200)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        aidog_logs::distinct_models_proxy_log(&db, &filter, true, 200)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn usage_stats_endpoints() {
    let db = test_db().await;

    let _ = aidog_stats::get_platform_usage_stats(&db, 1).await.unwrap();
    let _ = aidog_stats::get_group_usage_stats(&db, "gk").await.unwrap();
    assert!(
        aidog_stats::get_all_group_usage_stats(&db)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        aidog_stats::platform_usage_stats_all(&db)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        aidog_stats::get_last_test_result(&db, 1)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn log_settings_roundtrip() {
    let db = test_db().await;

    // default
    let _: ProxyLogSettings = aidog_db::get_setting(&db, "proxy", "logging")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    // set with retention cleanup branches exercised
    let settings = ProxyLogSettings {
        retention_days: 30,
        user_request_retention_days: 7,
        upstream_request_retention_days: 7,
        ..Default::default()
    };
    let value = serde_json::to_value(&settings).unwrap();
    aidog_db::set_setting(
        &db,
        gateway::models::SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value,
        },
    )
    .await
    .unwrap();
    run_retention_cleanup(&db, &settings).await;
    let got: ProxyLogSettings = aidog_db::get_setting(&db, "proxy", "logging")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    assert_eq!(got.retention_days, 30);
}

/// 构造三档同值同单位的设置，只为调度周期测试服务。
fn settings_all(value: u32, unit: aidog_db::models::RetentionUnit) -> ProxyLogSettings {
    ProxyLogSettings {
        user_request_retention_days: value,
        upstream_request_retention_days: value,
        retention_days: value,
        user_request_retention_unit: unit,
        upstream_request_retention_unit: unit,
        retention_unit: unit,
        ..Default::default()
    }
}

async fn save_log_settings(db: &aidog_db::Db, settings: &ProxyLogSettings) {
    aidog_db::set_setting(
        db,
        gateway::models::SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::to_value(settings).unwrap(),
        },
    )
    .await
    .unwrap();
}

/// 设置变更后周期跟着重算（无需重启）：改保留期 → 再读一次 `retention_cleanup_interval`
/// 直接得到新周期。调度循环每轮都调它，故「改了要重启才生效」不可能发生。
#[tokio::test]
async fn retention_interval_recomputes_after_settings_change() {
    let db = test_db().await;

    // 设置缺失 → 走 Default（三档 6h）→ 6h/4 = 90min
    assert_eq!(
        retention_cleanup_interval(&db).await,
        std::time::Duration::from_secs(5400)
    );

    // 用户改成三档全 1 小时 → 周期立刻变 15 分钟
    save_log_settings(&db, &settings_all(1, aidog_db::models::RetentionUnit::Hour)).await;
    assert_eq!(
        retention_cleanup_interval(&db).await,
        std::time::Duration::from_secs(900),
        "1h retention must yield a 15min cycle without restart"
    );

    // 再改成 90 天 → 夹到 24 小时上限
    save_log_settings(&db, &settings_all(90, aidog_db::models::RetentionUnit::Day)).await;
    assert_eq!(
        retention_cleanup_interval(&db).await,
        std::time::Duration::from_secs(24 * 3600)
    );
}

/// 设置保存唤醒调度器：`wake_retention_scheduler` 后，正在等 1 小时的循环立刻返回
/// `true`（= 重算周期），不必等满周期、更不必重启。
#[tokio::test]
async fn wake_returns_from_long_sleep_immediately() {
    wake_retention_scheduler();
    let woken = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        wait_next_retention_cycle(std::time::Duration::from_secs(3600)),
    )
    .await
    .expect("wake must cut the 1h sleep short");
    assert!(
        woken,
        "returning true means: recompute interval, skip cleanup"
    );
}
