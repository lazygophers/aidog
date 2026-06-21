use super::super::super::db;
use super::super::super::scheduling::{Admission, BreakerThresholds, SchedulerState, StickyTable};
use super::*;

async fn mk_test_db() -> db::Db {
    let db = db::Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    db
}

async fn mk_db_platform(db: &db::Db, name: &str) -> Platform {
    db::create_platform(db, CreatePlatform {
        name: name.into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://example.invalid".into(),
        api_key: "k".into(),
        extra: String::new(),
        models: None, available_models: None, endpoints: None, manual_budgets: None,
        auto_group: None, join_group_ids: None,
    }).await.expect("create platform")
}

async fn mk_db_group(db: &db::Db, name: &str, platform_ids: &[u64]) -> Group {
    let g = db::create_group(db, CreateGroup {
        name: name.into(),
        group_key: Some(name.into()),
        routing_mode: RoutingMode::Failover,
        auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()),
        max_retries: 2, model_mappings: vec![],
    }).await.expect("create group");
    let inputs: Vec<GroupPlatformInput> = platform_ids.iter().enumerate()
        .map(|(i, &pid)| GroupPlatformInput { platform_id: pid, priority: Some(i as i32), weight: Some(1), level_priority: None })
        .collect();
    db::set_group_platforms(db, g.id, &inputs).await.expect("set group platforms");
    g
}

/// 单平台分组：唯一平台熔断 Open 时仍必请求（无视状态），不踢空 blackhole。
#[tokio::test]
async fn single_platform_forces_request_when_circuit_broken() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "GLM").await;
    let g = mk_db_group(&db, "single", &[p.id]).await;

    // 把唯一平台熔断 Open
    let sched = SchedulerState::new();
    let now = db::now();
    let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
    sched.inc_inflight(p.id);
    sched.record_failure(p.id, &th, now);
    assert_eq!(sched.admission(p.id, &th, now, true), Admission::Reject);

    let sticky = StickyTable::new();
    // 总开关开，否则熔断维度不参与
    let settings = SchedulingBreakerSettings { enabled: true, ..Default::default() };
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    // 单平台短路：无视熔断必请求
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await
        .expect("single platform must force request, not Err");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p.id);
}

/// 单平台分组：唯一平台 auto_disabled（401/403 退避中）时仍必请求。
#[tokio::test]
async fn single_platform_forces_request_when_auto_disabled() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "GLM").await;
    let g = mk_db_group(&db, "single", &[p.id]).await;
    // 置 auto_disabled（退避未到期）
    db::set_platform_auto_disabled(&db, p.id).await.expect("set auto_disabled");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await
        .expect("single platform auto_disabled must still force request");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p.id);
}

/// 单平台分组：唯一平台手动 Disabled 是显式关停 → 仍 Err（唯一硬停）。
#[tokio::test]
async fn single_platform_manual_disabled_errs() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "GLM").await;
    let g = mk_db_group(&db, "single", &[p.id]).await;
    db::update_platform(&db, UpdatePlatform {
        id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
        extra: None, models: None, available_models: None, endpoints: None,
        enabled: None, status: Some(PlatformStatus::Disabled), manual_budgets: None,
        join_group_ids: None,
    }).await.expect("disable");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    let res = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await;
    assert!(res.is_err(), "manually disabled sole platform must Err");
}

/// 多平台分组：仍按平台状态过滤（一坏一好 → 只选好的）；全坏熔断 → 回退透传。
#[tokio::test]
async fn multi_platform_respects_status_and_falls_back_when_all_broken() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "GLM").await;
    let p2 = mk_db_platform(&db, "GLM2").await;
    let g = mk_db_group(&db, "multi", &[p1.id, p2.id]).await;

    let sched = SchedulerState::new();
    let now = db::now();
    let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
    // 仅 p1 熔断 Open，p2 健康
    sched.inc_inflight(p1.id);
    sched.record_failure(p1.id, &th, now);

    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings { enabled: true, ..Default::default() };
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    // 有健康平台 → 只选 p2（坏的被过滤）
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p2.id);

    // p2 也熔断 → 全坏 → 回退透传，两候选都回（不 blackhole）
    sched.inc_inflight(p2.id);
    sched.record_failure(p2.id, &th, now);
    let set2 = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await
        .expect("all-broken multi must fall back, not Err");
    assert_eq!(set2.candidates.len(), 2);
}
