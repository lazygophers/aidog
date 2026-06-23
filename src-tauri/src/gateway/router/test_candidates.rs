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
    mk_db_group_mode(db, name, platform_ids, RoutingMode::Failover).await
}

async fn mk_db_group_mode(db: &db::Db, name: &str, platform_ids: &[u64], mode: RoutingMode) -> Group {
    let g = db::create_group(db, CreateGroup {
        name: name.into(),
        group_key: Some(name.into()),
        routing_mode: mode,
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

/// 空平台分组 → Err("group has no platforms").
#[tokio::test]
async fn empty_group_returns_err() {
    let db = mk_test_db().await;
    let g = mk_db_group(&db, "empty", &[]).await;
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let res = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await;
    assert!(res.is_err(), "empty group should error");
    if let Err(err_msg) = res {
        assert!(err_msg.contains("no platforms"), "expected 'no platforms' in: {err_msg}");
    }
}

/// LoadBalance 路由模式：多平台分组，选出候选 >= 1。
#[tokio::test]
async fn load_balance_mode_returns_candidates() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "lb-p1").await;
    let p2 = mk_db_platform(&db, "lb-p2").await;
    let g = mk_db_group_mode(&db, "lb-group", &[p1.id, p2.id], RoutingMode::LoadBalance).await;
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert!(!set.candidates.is_empty());
}

/// LeastLatency 路由模式：返回候选，不 panic。
#[tokio::test]
async fn least_latency_mode_returns_candidates() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "ll-p1").await;
    let p2 = mk_db_platform(&db, "ll-p2").await;
    let g = mk_db_group_mode(&db, "ll-group", &[p1.id, p2.id], RoutingMode::LeastLatency).await;
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert!(!set.candidates.is_empty());
}

/// Sticky 路由模式：返回候选，不 panic。
#[tokio::test]
async fn sticky_mode_returns_candidates() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "sticky-p1").await;
    let p2 = mk_db_platform(&db, "sticky-p2").await;
    let g = mk_db_group_mode(&db, "sticky-group", &[p1.id, p2.id], RoutingMode::Sticky).await;
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: Some("sess-key".to_string()) };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert!(!set.candidates.is_empty());
}

/// model_mapping 命中 → 映射目标平台提到最前。
#[tokio::test]
async fn model_mapping_prioritizes_target_platform() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "map-p1").await;
    let p2 = mk_db_platform(&db, "map-p2").await;
    // Create group with model mapping: source "gpt-4o" → target p2
    let g = db::create_group(&db, CreateGroup {
        name: "map-group".into(),
        group_key: Some("map-group".into()),
        routing_mode: RoutingMode::Failover,
        auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()),
        max_retries: 2,
        model_mappings: vec![ModelMapping {
            source_model: "gpt-4o".to_string(),
            target_model: "gpt-4o-mapped".to_string(),
            target_platform_id: p2.id,
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
        }],
    }).await.expect("create group");
    let inputs = vec![
        GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: p2.id, priority: Some(1), weight: Some(1), level_priority: None },
    ];
    db::set_group_platforms(&db, g.id, &inputs).await.expect("set");
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "gpt-4o", Some(&ctx)).await.expect("ok");
    // p2 should be first (mapped target)
    assert_eq!(set.candidates[0].platform.id, p2.id);
    assert_eq!(set.candidates[0].target_model, "gpt-4o-mapped");
}

/// All platforms manually disabled → Err("no available platform").
#[tokio::test]
async fn all_platforms_disabled_returns_err() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "dis-p1").await;
    let p2 = mk_db_platform(&db, "dis-p2").await;
    // Disable both
    for pid in [p1.id, p2.id] {
        db::update_platform(&db, UpdatePlatform {
            id: pid, name: None, platform_type: None, base_url: None, api_key: None,
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: Some(PlatformStatus::Disabled), manual_budgets: None,
            join_group_ids: None,
        }).await.expect("disable");
    }
    let g = mk_db_group(&db, "dis-group", &[p1.id, p2.id]).await;
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let res = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await;
    assert!(res.is_err());
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
