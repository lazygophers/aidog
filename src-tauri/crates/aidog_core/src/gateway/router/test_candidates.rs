use super::super::super::db;
use super::super::super::scheduling::{Admission, BreakerThresholds, SchedulerState, StickyTable};
use super::super::ordering::{apply_coding_plan_priority, is_coding_plan};
use super::super::test_mod::{mk_gp, mk_gp_cp};
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
        auto_group: None, join_group_ids: None, default_level_priority: None, expires_at: None,
    }).await.expect("create platform")
}

/// 创建 coding plan 平台：带一个 coding_plan=true 的 anthropic 端点（is_coding_plan→true）。
async fn mk_db_platform_cp(db: &db::Db, name: &str) -> Platform {
    db::create_platform(db, CreatePlatform {
        name: name.into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://example.invalid".into(),
        api_key: "k".into(),
        extra: String::new(),
        models: None, available_models: None,
        endpoints: Some(vec![PlatformEndpoint {
            protocol: Protocol::Anthropic,
            base_url: "https://example.invalid".into(),
            client_type: Default::default(),
            coding_plan: true,
        }]),
        manual_budgets: None,
        auto_group: None, join_group_ids: None, default_level_priority: None, expires_at: None,
    }).await.expect("create coding-plan platform")
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
        max_retries: 2, model_mappings: vec![], env_vars: vec![],    }).await.expect("create group");
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
        join_group_ids: None, expires_at: None,
    }).await.expect("disable");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    let res = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await;
    assert!(res.is_err(), "manually disabled sole platform must Err");
}

/// 单平台分组：唯一平台高峰禁用启用 + 当前在 peak window → 不 bypass status，返 Err("peak_disabled")。
/// 验证 PRD: 高峰禁用优先级高于单平台组 status bypass（status 维度照旧 bypass，本维度不 bypass）。
#[tokio::test]
async fn single_platform_peak_disabled_errs() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "GLM").await;
    // 设置 disable_during_peak=true + 跨天窗口 22-06（always 命中：测一个非边界 hour）
    db::update_platform(&db, UpdatePlatform {
        id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
        extra: Some(r#"{"disable_during_peak":true,"peak_hours":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None,
        join_group_ids: None, expires_at: None,
    }).await.expect("set peak-disabled");
    let g = mk_db_group(&db, "single", &[p.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    let res = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await;
    assert!(res.is_err(), "peak-disabled sole platform must Err (no bypass)");
    if let Err(e) = res {
        assert_eq!(e, "peak_disabled", "expected peak_disabled error, got: {e}");
    }
}

/// 单平台分组：唯一平台高峰禁用启用但**不在** peak window（窗口不覆盖 now）→ 仍 bypass status 必请求。
/// 验证 status bypass 在非高峰照旧生效（两维度独立，高峰禁用仅在窗口命中时覆盖）。
#[tokio::test]
async fn single_platform_peak_disabled_off_peak_still_forces() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "GLM").await;
    // 窗口设为 1-2 点（当前测试在 1-2 点外的概率 ≈ 1；如运行在 1-2 点间会偶发失败，可忽略）
    db::update_platform(&db, UpdatePlatform {
        id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
        extra: Some(r#"{"disable_during_peak":true,"peak_hours":[{"start_hour":1,"end_hour":2,"multiplier":1.5}]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None,
        join_group_ids: None, expires_at: None,
    }).await.expect("set peak-disabled off-peak");
    let g = mk_db_group(&db, "single", &[p.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    // 极小概率运行在 01:00-02:00 UTC；选择窗口外的概率 22/24 ≈ 92%
    let now = db::now();
    let (hour, _) = crate::gateway::peak_hours::utc_hour_weekday(now);
    if hour == 1 {
        return; // 偶发跳过
    }
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await
        .expect("off-peak: single platform must still force request");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p.id);
}

/// 多平台分组：所有候选高峰禁用 → 返 Err("peak_disabled")（区别于普通 NoCandidate）。
/// 验证 D5: 整组全被高峰排除时返结构化错误，handler.rs 据此落 proxy_log blocked_reason='peak_hours'。
#[tokio::test]
async fn multi_platform_all_peak_disabled_errs() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "p1").await;
    let p2 = mk_db_platform(&db, "p2").await;
    // 两平台都启用高峰禁用 + 24h 全天命中窗口
    for pid in [p1.id, p2.id] {
        db::update_platform(&db, UpdatePlatform {
            id: pid, name: None, platform_type: None, base_url: None, api_key: None,
            extra: Some(r#"{"disable_during_peak":true,"peak_hours":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#.to_string()),
            models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None,
            join_group_ids: None, expires_at: None,
        }).await.expect("set peak-disabled");
    }
    let g = mk_db_group(&db, "multi", &[p1.id, p2.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    let res = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await;
    assert!(res.is_err(), "all peak-disabled must Err");
    if let Err(e) = res {
        assert_eq!(e, "peak_disabled", "expected peak_disabled error, got: {e}");
    }
}

/// 多平台分组：仅部分候选高峰禁用 → 被排除，其他候选正常纳入（不影响）。
#[tokio::test]
async fn multi_platform_partial_peak_disabled_skipped() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "p1").await; // 高峰禁用
    let p2 = mk_db_platform(&db, "p2").await; // 正常
    db::update_platform(&db, UpdatePlatform {
        id: p1.id, name: None, platform_type: None, base_url: None, api_key: None,
        extra: Some(r#"{"disable_during_peak":true,"peak_hours":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None,
        join_group_ids: None, expires_at: None,
    }).await.expect("set p1 peak-disabled");
    let g = mk_db_group(&db, "multi", &[p1.id, p2.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await
        .expect("partial peak-disabled: must still have p2 candidate");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p2.id, "only p2 (not peak-disabled) remains");
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
        env_vars: vec![],
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
            join_group_ids: None, expires_at: None,        }).await.expect("disable");
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

// ── coding plan 优先（apply_coding_plan_priority 纯函数 + select_candidates 集成）──

/// apply_coding_plan_priority：coding plan 平台整体上浮到非 coding plan 之前，桶内保持入参顺序（稳定）。
#[test]
fn coding_plan_priority_buckets_stable() {
    // 入参顺序: [非cp1, cp1, 非cp2, cp2]
    let n1 = mk_gp(1, 1, 0);
    let c1 = mk_gp_cp(2, 1, 0);
    let n2 = mk_gp(3, 1, 0);
    let c2 = mk_gp_cp(4, 1, 0);
    let mut v: Vec<&GroupPlatformDetail> = vec![&n1, &c1, &n2, &c2];
    apply_coding_plan_priority(&mut v);
    // coding plan 桶 [cp1, cp2] 在前（保持入参相对序 2 先于 4），非 cp 桶 [1,3] 在后
    assert_eq!(v.iter().map(|g| g.platform.id).collect::<Vec<_>>(), vec![2, 4, 1, 3]);
}

/// is_coding_plan：任一端点 coding_plan=true 即为 coding plan 平台。
#[test]
fn is_coding_plan_detects_endpoint_flag() {
    let cp = mk_gp_cp(1, 1, 0);
    let non = mk_gp(2, 1, 0);
    assert!(is_coding_plan(&cp.platform));
    assert!(!is_coding_plan(&non.platform));
}

/// Failover 混合分组：coding plan 平台排在非 coding plan 之前，
/// 同 bucket 内仍按 level_priority/priority。即便非 cp 平台 priority 更优，cp 仍居首。
#[tokio::test]
async fn failover_prefers_coding_plan_over_priority() {
    let db = mk_test_db().await;
    let non = mk_db_platform(&db, "non-cp").await;
    let cp = mk_db_platform_cp(&db, "cp").await;
    let g = db::create_group(&db, CreateGroup {
        name: "mix-fo".into(), group_key: Some("mix-fo".into()),
        routing_mode: RoutingMode::Failover, auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()), max_retries: 2, model_mappings: vec![], env_vars: vec![],    }).await.expect("create group");
    // 非 cp 给更优 priority(0)，cp 给较差 priority(1) —— 验证 coding plan 偏好覆盖 priority。
    let inputs = vec![
        GroupPlatformInput { platform_id: non.id, priority: Some(0), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: cp.id, priority: Some(1), weight: Some(1), level_priority: None },
    ];
    db::set_group_platforms(&db, g.id, &inputs).await.expect("set");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert_eq!(set.candidates.len(), 2);
    assert_eq!(set.candidates[0].platform.id, cp.id, "coding plan platform must be first despite worse priority");
    assert_eq!(set.candidates[1].platform.id, non.id);
}

/// Failover 同 bucket 内 priority 不变：两个 coding plan 平台仍按 priority 升序。
#[tokio::test]
async fn failover_intra_coding_plan_bucket_keeps_priority() {
    let db = mk_test_db().await;
    let cp_a = mk_db_platform_cp(&db, "cp-a").await;
    let cp_b = mk_db_platform_cp(&db, "cp-b").await;
    let g = db::create_group(&db, CreateGroup {
        name: "cp-bucket".into(), group_key: Some("cp-bucket".into()),
        routing_mode: RoutingMode::Failover, auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()), max_retries: 2, model_mappings: vec![], env_vars: vec![],    }).await.expect("create group");
    // cp_b priority 更优(0) → 应排 cp_a(1) 之前（桶内 priority 升序）
    let inputs = vec![
        GroupPlatformInput { platform_id: cp_a.id, priority: Some(1), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: cp_b.id, priority: Some(0), weight: Some(1), level_priority: None },
    ];
    db::set_group_platforms(&db, g.id, &inputs).await.expect("set");
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert_eq!(set.candidates[0].platform.id, cp_b.id, "intra-bucket priority asc preserved");
    assert_eq!(set.candidates[1].platform.id, cp_a.id);
}

/// LoadBalance 混合分组：coding plan bucket 整体在前；bucket 内加权随机不变。
/// 单 cp + 单非 cp → cp 必首（bucket 大小 1，随机不改变跨桶序）。
#[tokio::test]
async fn load_balance_coding_plan_bucket_first() {
    let db = mk_test_db().await;
    let non = mk_db_platform(&db, "lb-non").await;
    let cp = mk_db_platform_cp(&db, "lb-cp").await;
    let g = db::create_group(&db, CreateGroup {
        name: "lb-mix".into(), group_key: Some("lb-mix".into()),
        routing_mode: RoutingMode::LoadBalance, auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()), max_retries: 2, model_mappings: vec![], env_vars: vec![],    }).await.expect("create group");
    // 给非 cp 更大 weight（加权随机更可能选它）—— 验证 coding plan 桶仍整体在前。
    let inputs = vec![
        GroupPlatformInput { platform_id: non.id, priority: Some(0), weight: Some(100), level_priority: None },
        GroupPlatformInput { platform_id: cp.id, priority: Some(1), weight: Some(1), level_priority: None },
    ];
    db::set_group_platforms(&db, g.id, &inputs).await.expect("set");
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert_eq!(set.candidates[0].platform.id, cp.id, "coding plan bucket must lead regardless of weight");
}

/// probe（auto_disabled 已过退避）的 coding plan 平台不因偏好跨到 active 前。
///
/// 纯函数验证（不走 DB 回拨）：candidates.rs 对 active / probe **各自独立**调
/// apply_coding_plan_priority，合并时 active 桶整体在前。这里直接模拟该结构：
/// active = [非cp]（已 mode 排好）、probe = [cp]，各自分桶后拼接，
/// 断言非 cp active 仍在 cp probe 之前（coding plan 不跨桶上浮）。
#[test]
fn probe_coding_plan_does_not_cross_active() {
    let active_non = mk_gp(1, 1, 0);
    let probe_cp = mk_gp_cp(2, 1, 0);

    // active 桶仅含非 cp 平台；probe 桶仅含 coding plan 平台。
    let mut active: Vec<&GroupPlatformDetail> = vec![&active_non];
    let mut probe: Vec<&GroupPlatformDetail> = vec![&probe_cp];

    // 各自独立应用 coding plan 偏好（与 candidates.rs step2 一致）
    apply_coding_plan_priority(&mut active);
    apply_coding_plan_priority(&mut probe);

    // 合并：active 在前，probe 在后（candidates.rs step3）
    let mut ordered: Vec<&GroupPlatformDetail> = Vec::new();
    ordered.extend(active);
    ordered.extend(probe);

    // 非 cp active 仍居首，cp probe 退其后 —— probe 整体在 active 后，coding plan 不跨桶上浮。
    assert_eq!(ordered[0].platform.id, 1, "active platform leads probe even if probe is coding plan");
    assert_eq!(ordered[1].platform.id, 2);
}

/// 显式 model_mapping 目标平台仍居首，即便它非 coding plan（coding plan 偏好不得覆盖显式映射）。
#[tokio::test]
async fn explicit_mapping_overrides_coding_plan_preference() {
    let db = mk_test_db().await;
    let non = mk_db_platform(&db, "map-non").await; // 映射目标，非 coding plan
    let cp = mk_db_platform_cp(&db, "map-cp").await; // coding plan，但非映射目标
    let g = db::create_group(&db, CreateGroup {
        name: "map-cp-group".into(), group_key: Some("map-cp-group".into()),
        routing_mode: RoutingMode::Failover, auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()), max_retries: 2,
        model_mappings: vec![ModelMapping {
            source_model: "gpt-4o".to_string(),
            target_model: "gpt-4o-mapped".to_string(),
            target_platform_id: non.id,
            request_timeout_secs: 0, connect_timeout_secs: 0,
        }],
        env_vars: vec![],
    }).await.expect("create group");
    let inputs = vec![
        GroupPlatformInput { platform_id: non.id, priority: Some(0), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: cp.id, priority: Some(1), weight: Some(1), level_priority: None },
    ];
    db::set_group_platforms(&db, g.id, &inputs).await.expect("set");
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "gpt-4o", Some(&ctx)).await.expect("ok");
    // 映射目标 non（非 coding plan）仍居首，coding plan cp 退居其后
    assert_eq!(set.candidates[0].platform.id, non.id, "explicit mapping target must stay first over coding plan");
    assert_eq!(set.candidates[0].target_model, "gpt-4o-mapped");
}

// ── [platform-expiry-priority] 同 priority 内按 expires_at 升序优先 ──

/// 创建带 expires_at 的普通平台。
async fn mk_db_platform_exp(db: &db::Db, name: &str, expires_at: i64) -> Platform {
    db::create_platform(db, CreatePlatform {
        name: name.into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://example.invalid".into(),
        api_key: "k".into(),
        extra: String::new(),
        models: None, available_models: None, endpoints: None, manual_budgets: None,
        auto_group: None, join_group_ids: None, default_level_priority: None,
        expires_at: Some(expires_at),
    }).await.expect("create platform with expires_at")
}

/// Failover 同 priority 候选：未过期的平台中 expires_at 最小者优先调度。
/// 混合 3 平台同 priority=0：近未来 / 远未来 / 永不过期(0) → 期望近→远→永。
#[tokio::test]
async fn failover_prefers_earliest_expiry_within_same_priority() {
    let db = mk_test_db().await;
    let now = db::now();
    let p_near = mk_db_platform_exp(&db, "near", now + 60_000).await;       // 1 分钟后过期
    let p_far = mk_db_platform_exp(&db, "far", now + 7 * 86_400_000).await; // 7 天后过期
    let p_forever = mk_db_platform_exp(&db, "forever", 0).await;           // 永不过期
    // 三平台均 priority=0（同优先级），故意打乱注册顺序验证排序不受注册序影响
    let g = db::create_group(&db, CreateGroup {
        name: "exp-group".into(), group_key: Some("exp-group".into()),
        routing_mode: RoutingMode::Failover, auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()), max_retries: 2, model_mappings: vec![], env_vars: vec![],    }).await.expect("create group");
    let inputs = vec![
        GroupPlatformInput { platform_id: p_forever.id, priority: Some(0), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: p_far.id, priority: Some(0), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: p_near.id, priority: Some(0), weight: Some(1), level_priority: None },
    ];
    db::set_group_platforms(&db, g.id, &inputs).await.expect("set");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert_eq!(set.candidates.len(), 3);
    // expires_at 升序：近未来 → 远未来 → 永不过期
    assert_eq!(set.candidates[0].platform.id, p_near.id, "earliest expiry must be scheduled first");
    assert_eq!(set.candidates[1].platform.id, p_far.id, "farther expiry second");
    assert_eq!(set.candidates[2].platform.id, p_forever.id, "never-expiring (0) goes last within same priority");
}

/// expires_at=0（永不过期）排在所有有期限平台之后，即便 priority 更优也不跨 priority。
#[tokio::test]
async fn failover_priority_dominates_over_expiry_in_db() {
    let db = mk_test_db().await;
    let now = db::now();
    // 永不过期但 priority 更优(0) vs 快过期但 priority 较差(1)
    let p_noexp = mk_db_platform_exp(&db, "noexp-p0", 0).await;
    let p_expiring = mk_db_platform_exp(&db, "exp-p1", now + 60_000).await;
    let g = db::create_group(&db, CreateGroup {
        name: "prio-dom".into(), group_key: Some("prio-dom".into()),
        routing_mode: RoutingMode::Failover, auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()), max_retries: 2, model_mappings: vec![], env_vars: vec![],    }).await.expect("create group");
    // p_noexp priority=0（更优）、p_expiring priority=1（较差）
    let inputs = vec![
        GroupPlatformInput { platform_id: p_noexp.id, priority: Some(0), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: p_expiring.id, priority: Some(1), weight: Some(1), level_priority: None },
    ];
    db::set_group_platforms(&db, g.id, &inputs).await.expect("set");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    // priority 主序：p_noexp(0) 居首，即便永不过期；expires_at 不跨 priority
    assert_eq!(set.candidates[0].platform.id, p_noexp.id, "priority dominates over expiry");
    assert_eq!(set.candidates[1].platform.id, p_expiring.id);
}

/// 已过期平台（now >= expires_at）被 candidate_state 过滤，不参与本需求优先调度。
#[tokio::test]
async fn expired_platform_filtered_out_not_prioritized() {
    let db = mk_test_db().await;
    let now = db::now();
    // 已过期（now - 1）+ 永不过期 候选；已过期应被踢，仅永不过期候选返回。
    let p_expired = mk_db_platform_exp(&db, "expired", now - 1).await;
    let p_ok = mk_db_platform_exp(&db, "ok", 0).await;
    let g = mk_db_group(&db, "exp-filter", &[p_expired.id, p_ok.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    // 已过期 p_expired 被过滤，只剩 p_ok
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p_ok.id, "expired platform filtered by candidate_state, not prioritized");
}

// ── 非 Failover 模式 expiry tiebreak 集成（扩展至全部模式） ──

/// LeastLatency 模式：无延迟样本（所有平台 EMA=MAX，同档）→ 同 EMA 档内按 expires_at 升序。
/// 三平台同 priority、无延迟记录：近未来 / 远未来 / 永不过期 → 期望近 → 远 → 永。
#[tokio::test]
async fn least_latency_prefers_earliest_expiry_within_same_ema() {
    let db = mk_test_db().await;
    let now = db::now();
    let p_near = mk_db_platform_exp(&db, "ll-near", now + 60_000).await;
    let p_far = mk_db_platform_exp(&db, "ll-far", now + 7 * 86_400_000).await;
    let p_forever = mk_db_platform_exp(&db, "ll-forever", 0).await;
    let g = db::create_group(&db, CreateGroup {
        name: "ll-exp".into(), group_key: Some("ll-exp".into()),
        routing_mode: RoutingMode::LeastLatency, auto_from_platform: String::new(),
        request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: Some("anthropic".into()), max_retries: 2, model_mappings: vec![], env_vars: vec![],    }).await.expect("create group");
    let inputs = vec![
        GroupPlatformInput { platform_id: p_forever.id, priority: Some(0), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: p_far.id, priority: Some(0), weight: Some(1), level_priority: None },
        GroupPlatformInput { platform_id: p_near.id, priority: Some(0), weight: Some(1), level_priority: None },
    ];
    db::set_group_platforms(&db, g.id, &inputs).await.expect("set");

    // 无延迟样本 → 所有 EMA=MAX 同档，expiry 升序 tiebreak 生效
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
    assert_eq!(set.candidates.len(), 3);
    assert_eq!(set.candidates[0].platform.id, p_near.id, "earliest expiry first within same EMA bucket");
    assert_eq!(set.candidates[1].platform.id, p_far.id, "farther expiry second");
    assert_eq!(set.candidates[2].platform.id, p_forever.id, "never-expiring (0) last within same EMA bucket");
}

// ── PRD 07-11: preset.models.peak 分支路由层切换 ──────────────────────────────

/// 构造一个 glm_coding 协议的 Platform（带 peak_hours + preset.models.default 填入 platform.models）。
fn glm_coding_platform(extra: &str, platform_models: PlatformModels) -> Platform {
    let mut p = super::super::test_mod::mk_platform(PlatformStatus::Enabled, 0);
    p.platform_type = Protocol::GlmCoding;
    p.extra = extra.into();
    p.models = platform_models;
    p
}

/// 高峰窗口 06:00-10:00 UTC × 3.0（与 platform-presets.json glm_coding.peak_hours[0] 同）。
/// 测试时间戳选 2024-01-01T07:00:00Z（hour=7 落 [6,10)）→ 命中高峰。
const PEAK_MS: i64 = (1_704_067_200 + 7 * 3600) * 1000;
/// 非高峰时间戳 2024-01-01T12:00:00Z（hour=12，[6,10) 外）。
const OFF_MS: i64 = (1_704_067_200 + 12 * 3600) * 1000;

#[test]
fn resolve_effective_models_peak_branch_replaces_default_when_in_peak_window() {
    // preset.glm_coding.models.default 填入 platform.models（创建时常见路径）
    let platform_models = PlatformModels {
        default: Some("glm-5.2".into()),
        opus: Some("glm-5.2".into()),
        sonnet: Some("glm-4.7".into()),
        gpt: Some("glm-5.2".into()),
        haiku: Some("glm-4.5".into()),
    };
    // extra 无用户覆盖 → peak_hours_for 回落 bundled preset glm_coding 默认窗口（6-10 ×3.0）
    let p = glm_coding_platform("", platform_models);
    let eff = resolve_effective_models(&p, &[], PEAK_MS, "");
    // 命中高峰 → 切到 preset.models.peak（glm-4.7 / glm-4.6 ...）
    assert_eq!(eff.default.as_deref(), Some("glm-4.7"), "peak: default 切 glm-4.7");
    assert_eq!(eff.sonnet.as_deref(), Some("glm-4.6"), "peak: sonnet 切 glm-4.6");
    assert_eq!(eff.haiku.as_deref(), Some("glm-4.5"), "peak: haiku 不变 glm-4.5");
}

#[test]
fn resolve_effective_models_no_peak_when_off_peak() {
    let platform_models = PlatformModels {
        default: Some("glm-5.2".into()),
        sonnet: Some("glm-4.7".into()),
        ..Default::default()
    };
    let p = glm_coding_platform("", platform_models);
    let eff = resolve_effective_models(&p, &[], OFF_MS, "");
    // 非高峰 → 保持 platform.models（不切 peak）
    assert_eq!(eff.default.as_deref(), Some("glm-5.2"), "off-peak: 保持 default");
    assert_eq!(eff.sonnet.as_deref(), Some("glm-4.7"), "off-peak: 保持 sonnet");
}

#[test]
fn resolve_effective_models_user_time_models_overrides_peak_branch() {
    // 用户显式 time_models 优先级高于 preset.models.peak（time_rules 非空 → 跳过 peak 分支）
    let platform_models = PlatformModels {
        default: Some("glm-5.2".into()),
        ..Default::default()
    };
    // extra 含 time_models（全天命中 → models.default=custom-model）
    let extra = serde_json::json!({
        "time_models": [{
            "windows": [{"start_hour": 0, "end_hour": 24}],
            "models": {"default": "custom-model"}
        }]
    }).to_string();
    let p = glm_coding_platform(&extra, platform_models);
    let time_rules = crate::gateway::time_models::parse_platform_time_models(&p.extra);
    let eff = resolve_effective_models(&p, &time_rules, PEAK_MS, "");
    // time_models 命中 → custom-model（不被 preset.models.peak 覆盖）
    assert_eq!(eff.default.as_deref(), Some("custom-model"), "user time_models 优先于 preset peak");
}

#[test]
fn resolve_effective_models_protocol_without_peak_branch_unchanged() {
    // 协议无 preset.models.peak 分支（如 anthropic）→ 行为零变更（向后兼容）
    // mk_platform 默认协议即 anthropic
    let mut p = super::super::test_mod::mk_platform(PlatformStatus::Enabled, 0);
    p.models = PlatformModels {
        default: Some("claude-sonnet-4".into()),
        ..Default::default()
    };
    let eff = resolve_effective_models(&p, &[], PEAK_MS, "");
    assert_eq!(eff.default.as_deref(), Some("claude-sonnet-4"), "无 peak 分支 → 不动");
}

#[test]
fn resolve_effective_models_user_peak_hours_override_takes_effect() {
    // 用户 extra.peak_hours 覆盖 preset 默认（peak_hours_for 优先返用户值）
    // 此处用户窗口设为非时段（start=end=0 退化 = 全天命中）→ is_in_peak_window=true
    let platform_models = PlatformModels {
        default: Some("glm-5.2".into()),
        ..Default::default()
    };
    let extra = serde_json::json!({
        "peak_hours": [{"start_hour": 0, "end_hour": 0, "multiplier": 1.5}]
    }).to_string();
    let p = glm_coding_platform(&extra, platform_models);
    // OFF_MS（hour=12，正常非高峰）但用户窗口全天命中 → 仍切 peak
    let eff = resolve_effective_models(&p, &[], OFF_MS, "");
    assert_eq!(eff.default.as_deref(), Some("glm-4.7"), "用户 peak_hours 命中 → 切 peak");
}

// ── cpa-standalone-module s2: cli-proxy 平台路由 ──────────────────────────────

/// 创建一个 cli-proxy 平台（extra 指向 provider_id）。
async fn mk_cli_proxy_platform(db: &db::Db, name: &str, provider_id: u64) -> Platform {
    let extra = serde_json::json!({ "cli_proxy_provider_id": provider_id }).to_string();
    db::create_platform(db, CreatePlatform {
        name: name.into(),
        platform_type: Protocol::CliProxy,
        base_url: String::new(),  // cli-proxy 平台 base_url 空由 provider 注入
        api_key: String::new(),
        extra,
        models: None, available_models: None, endpoints: None, manual_budgets: None,
        auto_group: None, join_group_ids: None, default_level_priority: None, expires_at: None,
    }).await.expect("create cli-proxy platform")
}

/// 创建一个 cli_proxy_provider 行。
async fn mk_cli_proxy_provider(db: &db::Db, name: &str, wire: &str, base_url: &str, status: &str) -> CliProxyProvider {
    db::create_cli_proxy_provider(db, CreateCliProxyProvider {
        name: name.into(),
        wire_protocol: wire.into(),
        base_url: base_url.into(),
        api_key: "sk-cli".into(),
        models: vec!["claude-sonnet-4".into(), "claude-opus-4".into()],
        extra: String::new(),
        quota: String::new(),
        status: status.into(),
        group_id: None,
    }).await.expect("create provider")
}

/// cli-proxy 平台路由：provider 配置注入 endpoint + base_url + api_key。
#[tokio::test]
async fn cli_proxy_platform_injects_provider_config() {
    let db = mk_test_db().await;
    let provider = mk_cli_proxy_provider(&db, "p1", "anthropic", "https://upstream.example/v1", "active").await;
    let p = mk_cli_proxy_platform(&db, "cli-plat", provider.id).await;
    let g = mk_db_group(&db, "g", &[p.id]).await;

    let set = select_candidates(&db, &g, "claude-sonnet-4").await.expect("ok");
    assert_eq!(set.candidates.len(), 1);
    let c = &set.candidates[0];
    // endpoint 注入：wire = provider.wire_protocol = anthropic
    assert_eq!(c.platform.endpoints.len(), 1, "cli-proxy injects 1 synthetic endpoint");
    assert_eq!(format!("{:?}", c.platform.endpoints[0].protocol).to_lowercase(), "anthropic");
    // base_url + api_key 由 provider 覆盖
    assert_eq!(c.platform.base_url, "https://upstream.example/v1");
    assert_eq!(c.platform.api_key, "sk-cli");
    // target_model: source_model 在 provider.models 中 → 透传
    assert_eq!(c.target_model, "claude-sonnet-4");
}

/// cli-proxy 平台 target_model 回落：source_model 不在 provider.models → 取首项。
#[tokio::test]
async fn cli_proxy_target_model_falls_back_to_first() {
    let db = mk_test_db().await;
    let provider = mk_cli_proxy_provider(&db, "p1", "anthropic", "https://up.example/v1", "active").await;
    let p = mk_cli_proxy_platform(&db, "cli-plat", provider.id).await;
    let g = mk_db_group(&db, "g", &[p.id]).await;

    let set = select_candidates(&db, &g, "some-unknown-model").await.expect("ok");
    assert_eq!(set.candidates[0].target_model, "claude-sonnet-4", "unknown model falls back to provider.models[0]");
}

/// cli-proxy provider 缺失（id 无效）→ 平台排除；单平台组 → Err。
#[tokio::test]
async fn cli_proxy_missing_provider_excludes() {
    let db = mk_test_db().await;
    // 不创建 provider，直接指向不存在的 id
    let p = mk_cli_proxy_platform(&db, "cli-plat", 9999).await;
    let g = mk_db_group(&db, "g", &[p.id]).await;

    let res = select_candidates(&db, &g, "claude-sonnet-4").await;
    let err = match res {
        Err(e) => e,
        Ok(_) => panic!("missing provider should exclude platform"),
    };
    assert!(err.contains("missing") || err.contains("disabled"), "got: {err}");
}

/// cli-proxy provider status != active → 平台排除。
#[tokio::test]
async fn cli_proxy_disabled_provider_excludes() {
    let db = mk_test_db().await;
    let provider = mk_cli_proxy_provider(&db, "p1", "anthropic", "https://up.example/v1", "disabled").await;
    let p = mk_cli_proxy_platform(&db, "cli-plat", provider.id).await;
    let g = mk_db_group(&db, "g", &[p.id]).await;

    let res = select_candidates(&db, &g, "claude-sonnet-4").await;
    let err = match res {
        Err(e) => e,
        Ok(_) => panic!("disabled provider should exclude platform"),
    };
    assert!(err.contains("missing") || err.contains("disabled"), "got: {err}");
}

/// cli-proxy 平台 extra 缺 cli_proxy_provider_id → 排除。
#[tokio::test]
async fn cli_proxy_missing_provider_id_excludes() {
    let db = mk_test_db().await;
    let p = db::create_platform(&db, CreatePlatform {
        name: "cli-no-id".into(),
        platform_type: Protocol::CliProxy,
        base_url: String::new(),
        api_key: String::new(),
        extra: String::new(),  // 无 cli_proxy_provider_id
        models: None, available_models: None, endpoints: None, manual_budgets: None,
        auto_group: None, join_group_ids: None, default_level_priority: None, expires_at: None,
    }).await.expect("create platform");
    let g = mk_db_group(&db, "g", &[p.id]).await;

    let res = select_candidates(&db, &g, "claude-sonnet-4").await;
    let err = match res {
        Err(e) => e,
        Ok(_) => panic!("missing provider_id should exclude platform"),
    };
    assert!(err.contains("missing") || err.contains("disabled"), "got: {err}");
}

/// 多平台分组：cli-proxy 平台与普通平台混合，cli-proxy 被正确注入，普通平台不受影响。
#[tokio::test]
async fn cli_proxy_mixed_with_normal_platform() {
    let db = mk_test_db().await;
    let provider = mk_cli_proxy_provider(&db, "p1", "openai", "https://up.example/v1", "active").await;
    let p_cli = mk_cli_proxy_platform(&db, "cli-plat", provider.id).await;
    let p_normal = mk_db_platform(&db, "normal-plat").await;
    let g = mk_db_group(&db, "g", &[p_cli.id, p_normal.id]).await;

    let set = select_candidates(&db, &g, "claude-sonnet-4").await.expect("ok");
    assert_eq!(set.candidates.len(), 2);
    let cli_candidate = set.candidates.iter().find(|c| c.platform.id == p_cli.id).expect("cli candidate present");
    assert_eq!(cli_candidate.platform.base_url, "https://up.example/v1", "cli-proxy base_url injected");
    assert_eq!(cli_candidate.platform.endpoints.len(), 1, "cli-proxy endpoint injected");
    let normal = set.candidates.iter().find(|c| c.platform.id == p_normal.id).expect("normal candidate present");
    assert_eq!(normal.platform.base_url, "https://example.invalid", "normal platform base_url untouched");
    assert!(normal.platform.endpoints.is_empty(), "normal platform endpoints untouched");
}

