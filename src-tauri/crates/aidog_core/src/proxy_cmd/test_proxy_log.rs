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

// ─── scheduled_cleanup_pass（桌面 / 内核共享清理链单轮）────────────

/// 可观察行为：超 3 天的软删行被硬删（桌面壳与内核两份循环代码合并后的回归护栏）。
#[tokio::test]
async fn scheduled_cleanup_pass_purges_old_soft_deleted() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let old = chrono::Utc::now().timestamp_millis() - 4 * 24 * 3600 * 1000_i64;
    db.call_proxy_log_traced(None, std::panic::Location::caller(), move |c| {
        c.execute(
            &format!(
                "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
                 status_code, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, \
                 created_at, deleted_at) VALUES ('cleanup-pass', 0, '', 'm', 'anthropic', \
                 200, 1, 1, 0, 0, 0, {old}, {old})"
            ),
            [],
        )
    })
    .await
    .unwrap();
    super::scheduled_cleanup_pass(&db, 3 * 24 * 3600).await;
    let left: i64 = db
        .call_read_proxy_log_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT COUNT(*) FROM proxy_log WHERE id = 'cleanup-pass'",
                [],
                |r| r.get(0),
            )
        })
        .await
        .unwrap();
    assert_eq!(
        left, 0,
        "old soft-deleted row should be purged, found {left}"
    );
}

/// T1 平台详情：`compute_platform_health_detail` 冷却剩余秒现算（now_ms 注入）、
/// breaker_open / breaker_fails 透传、高峰禁用态从 extra 现算、平台行缺失全零。
#[tokio::test]
async fn platform_health_detail_computes_cooldowns_and_peak() {
    use aidog_db::health_state::{PlatformHealthStateRow, upsert_platform_health_state};

    let db = test_db().await;
    let p = aidog_db::test_support::insert_test_platform(&db, "HP").await;
    let now_ms = chrono::Utc::now().timestamp_millis();
    upsert_platform_health_state(
        &db,
        PlatformHealthStateRow {
            platform_id: p as i64,
            breaker_state: "open".into(),
            breaker_until_ms: now_ms + 60_000,
            breaker_fails: 3,
            breaker_last_fail_ms: now_ms - 1000,
            quota_cooldown_until_ms: now_ms + 30_000,
            quota_verdict_until_ms: now_ms - 5_000,
            auth_cooldown_until_ms: 0,
            balance_cooldown_until_ms: now_ms + 90_500,
            last_connect_fail_ms: now_ms - 60_000,
            updated_at: now_ms,
        },
    )
    .await
    .unwrap();

    // 无 disable_during_peak → peak_disabled false，冷却秒现算。
    let d = super::compute_platform_health_detail(&db, p, now_ms).await.unwrap();
    assert!(d.breaker_open);
    assert_eq!(d.breaker_fails, 3);
    assert_eq!(d.quota_cooldown_secs, 30);
    assert_eq!(d.quota_verdict_secs, 0, "过期冷却 = 0");
    assert_eq!(d.auth_cooldown_secs, 0);
    assert_eq!(d.balance_cooldown_secs, 90);
    assert_eq!(d.last_connect_fail_ms, now_ms - 60_000);
    assert!(!d.peak_disabled);

    // 开 disable_during_peak + 全天 peak 窗口 → peak_disabled true。
    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "UPDATE platform SET extra = ?1 WHERE id = ?2",
            rusqlite::params![
                r#"{"peak":[{"start_hour":0,"end_hour":24,"multiplier":2.0}],"disable_during_peak":true}"#,
                p as i64
            ],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    let d = super::compute_platform_health_detail(&db, p, now_ms).await.unwrap();
    assert!(d.peak_disabled, "disable_during_peak + 命中全天窗口");

    // 平台行缺失（已删 / 不存在）→ 全零 + peak_disabled false，不报错。
    let d = super::compute_platform_health_detail(&db, 999_999, now_ms).await.unwrap();
    assert!(!d.breaker_open);
    assert_eq!(d.breaker_fails, 0);
    assert_eq!(d.quota_cooldown_secs, 0);
    assert!(!d.peak_disabled);
}

#[tokio::test]
async fn platform_quota_window_levels_computes_water() {
    let db = test_db().await;
    let p = aidog_db::test_support::insert_test_platform(&db, "QW").await;
    let now_ms = 1_700_000_000_000i64;
    let extra = r#"{"quota_windows":[
        {"window":3600,"budget_count":2},
        {"window":7200,"budget_usd":5,"reset_mode":"calendar_hour","enabled":false}
    ]}"#;
    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute("UPDATE platform SET extra = ?1 WHERE id = ?2", rusqlite::params![extra, p as i64])?;
        // 写一行真实用量：count 窗口已用 1/2
        conn.execute(
            "INSERT INTO quota_window_usage (platform_id, window_key, window_start_at, consumed) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![p as i64, "3600|count|2", now_ms, 1.0],
        )?;
        Ok(())
    })
    .await
    .unwrap();

    let rows = super::compute_platform_quota_window_levels(&db, p, now_ms).await.unwrap();
    assert_eq!(rows.len(), 1, "disabled 窗口不出水位");
    let r = &rows[0];
    assert_eq!(r.key, "3600|count|2");
    assert_eq!(r.dimension, "count");
    assert_eq!(r.reset_mode, "sliding");
    assert_eq!(r.used, 1.0);
    assert_eq!(r.budget, 2.0);
    assert!((r.remaining_ratio - 0.5).abs() < 1e-9);
    assert_eq!(r.resets_at_ms, now_ms + 3_600_000);

    // 平台行缺失 → 空列表不报错
    let none = super::compute_platform_quota_window_levels(&db, 999_999, now_ms).await.unwrap();
    assert!(none.is_empty());
}
