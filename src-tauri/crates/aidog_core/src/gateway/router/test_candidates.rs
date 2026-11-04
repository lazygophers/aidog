use super::super::super::scheduling::{Admission, BreakerThresholds, SchedulerState, StickyTable};
use super::super::ordering::{apply_coding_plan_priority, is_coding_plan};
use super::super::test_mod::{mk_gp, mk_gp_cp};
use super::*;
use aidog_db as db;
use aidog_db::models::{RouteReason, RouteTag, UpdatePlatform};
use aidog_stats::DbInitTables;

async fn mk_test_db() -> db::Db {
    let db = db::Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    db
}

async fn mk_db_platform(db: &db::Db, name: &str) -> Platform {
    db::create_platform(
        db,
        CreatePlatform {
            name: name.into(),
            platform_type: Protocol::Anthropic,
            base_url: "https://example.invalid".into(),
            api_key: "k".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("create platform")
}

/// 创建 coding plan 平台：带一个 coding_plan=true 的 anthropic 端点（is_coding_plan→true）。
async fn mk_db_platform_cp(db: &db::Db, name: &str) -> Platform {
    db::create_platform(
        db,
        CreatePlatform {
            name: name.into(),
            platform_type: Protocol::Anthropic,
            base_url: "https://example.invalid".into(),
            api_key: "k".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: Some(vec![PlatformEndpoint {
                protocol: Protocol::Anthropic,
                base_url: "https://example.invalid".into(),
                client_type: Default::default(),
                coding_plan: true,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("create coding-plan platform")
}

async fn mk_db_group(db: &db::Db, name: &str, platform_ids: &[u64]) -> Group {
    mk_db_group_mode(db, name, platform_ids, RoutingMode::Failover).await
}

async fn mk_db_group_mode(
    db: &db::Db,
    name: &str,
    platform_ids: &[u64],
    mode: RoutingMode,
) -> Group {
    let g = db::create_group(
        db,
        CreateGroup {
            name: name.into(),
            group_key: Some(name.into()),
            routing_mode: mode,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2,
            model_mappings: vec![],
            env_vars: vec![],
        },
    )
    .await
    .expect("create group");
    let inputs: Vec<GroupPlatformInput> = platform_ids
        .iter()
        .enumerate()
        .map(|(i, &pid)| GroupPlatformInput {
            platform_id: pid,
            priority: Some(i as i32),
            weight: Some(1),
            level_priority: None,
        })
        .collect();
    db::set_group_platforms(db, g.id, &inputs)
        .await
        .expect("set group platforms");
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
    let th = BreakerThresholds {
        failure_threshold: 1,
        open_secs: 1800,
        half_open_max: 2,
    };
    sched.inc_inflight(p.id);
    sched.record_failure(p.id, &th, now);
    assert_eq!(sched.admission(p.id, &th, now, true), Admission::Reject);

    let sticky = StickyTable::new();
    // 总开关开，否则熔断维度不参与
    let settings = SchedulingBreakerSettings {
        enabled: true,
        ..Default::default()
    };
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    // 单平台短路：无视熔断必请求
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("single platform must force request, not Err");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p.id);
    // 单平台组快照：选中条目 target_model 已回填
    assert_eq!(
        set.routing.entries[0].target_model.as_deref(),
        Some(set.candidates[0].target_model.as_str())
    );
}

/// 配额冷却（429 配额耗尽）：冷却中的平台不进候选，但 DB status 保持 enabled（不被标记禁用）；
/// 到点后自动回到候选。
#[tokio::test]
async fn quota_cooldown_excludes_candidate_without_disabling() {
    let db = mk_test_db().await;
    let cooled = mk_db_platform(&db, "GLM").await;
    let other = mk_db_platform(&db, "Kimi").await;
    let g = mk_db_group(&db, "multi", &[cooled.id, other.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default(); // 熔断总开关关：冷却维度独立
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let now = db::now();
    sched.set_quota_cooldown(cooled.id, now + 3_600_000);

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("other platform still available");
    assert!(
        set.candidates.iter().all(|c| c.platform.id != cooled.id),
        "quota-cooled platform must not be scheduled"
    );

    // DB status 未被改成 auto_disabled（UI 仍显示启用）
    let row = db::get_platform(&db, cooled.id)
        .await
        .expect("get platform")
        .expect("platform exists");
    assert_eq!(row.status, PlatformStatus::Enabled);
    assert_eq!(row.auto_disabled_until, 0);
}

/// 单平台分组：唯一平台 auto_disabled（401/403 退避中）时仍必请求。
#[tokio::test]
async fn single_platform_forces_request_when_auto_disabled() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "GLM").await;
    let g = mk_db_group(&db, "single", &[p.id]).await;
    // 置 auto_disabled（退避未到期）
    db::test_support::set_legacy_auto_disabled(&db, p.id, 1, db::now() + 3_600_000).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("single platform auto_disabled must still force request");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p.id);
}

#[tokio::test]
async fn single_platform_censorship_blocked_does_not_bypass() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "censorship").await;
    let g = mk_db_group(&db, "single-censorship", &[p.id]).await;
    db::disable_platform_for_censorship(&db, p.id)
        .await
        .unwrap();

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let result =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
    assert_eq!(
        result.err().as_deref(),
        Some("group's only platform is censorship-blocked")
    );
}

/// 单平台分组：唯一平台手动 Disabled 是显式关停 → 仍 Err（唯一硬停）。
#[tokio::test]
async fn single_platform_manual_disabled_errs() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "GLM").await;
    let g = mk_db_group(&db, "single", &[p.id]).await;
    db::update_platform(
        &db,
        UpdatePlatform {
            id: p.id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: Some(PlatformStatus::Disabled),
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("disable");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let res =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
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
        extra: Some(r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    }).await.expect("set peak-disabled");
    let g = mk_db_group(&db, "single", &[p.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let res =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
    assert!(
        res.is_err(),
        "peak-disabled sole platform must Err (no bypass)"
    );
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
        extra: Some(r#"{"disable_during_peak":true,"peak":[{"start_hour":1,"end_hour":2,"multiplier":1.5}]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    }).await.expect("set peak-disabled off-peak");
    let g = mk_db_group(&db, "single", &[p.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    // 极小概率运行在 01:00-02:00 UTC；选择窗口外的概率 22/24 ≈ 92%
    let now = db::now();
    let (hour, _) = crate::gateway::peak::utc_hour_weekday(now);
    if hour == 1 {
        return; // 偶发跳过
    }
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("off-peak: single platform must still force request");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p.id);
}

/// 多平台分组：所有候选高峰禁用 → 返 Err("peak_disabled")（区别于普通 NoCandidate）。
/// 验证 D5: 整组全被高峰排除时返结构化错误，handler.rs 据此落 proxy_log blocked_reason='peak'。
#[tokio::test]
async fn multi_platform_all_peak_disabled_errs() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "p1").await;
    let p2 = mk_db_platform(&db, "p2").await;
    // 两平台都启用高峰禁用 + 24h 全天命中窗口
    for pid in [p1.id, p2.id] {
        db::update_platform(&db, UpdatePlatform {
            id: pid, name: None, platform_type: None, base_url: None, api_key: None,
            extra: Some(r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#.to_string()),
            models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None,
            join_group_ids: None, expires_at: None, quota_source: None,
        }).await.expect("set peak-disabled");
    }
    let g = mk_db_group(&db, "multi", &[p1.id, p2.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let res =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
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
        extra: Some(r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    }).await.expect("set p1 peak-disabled");
    let g = mk_db_group(&db, "multi", &[p1.id, p2.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("partial peak-disabled: must still have p2 candidate");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(
        set.candidates[0].platform.id, p2.id,
        "only p2 (not peak-disabled) remains"
    );
}

/// 票 06：把 platform.platform_type 直写为枚举外值（模拟降级读新数据 / 脏数据）。
/// UpdatePlatform.platform_type 是 Option<Protocol> 走不了 API，只能 raw SQL。
async fn set_unknown_platform_type(target_db: &db::Db, id: u64) {
    target_db
        .call_platform_traced(None, std::panic::Location::caller(), move |conn| {
            conn.execute(
                "UPDATE platform SET platform_type='no_such_protocol' WHERE id=?1",
                rusqlite::params![id as i64],
            )?;
            Ok(())
        })
        .await
        .expect("set unknown platform_type");
    target_db.invalidate_group_details_cache();
}

/// 单平台分组：唯一平台协议未知（DB 枚举外值 → Protocol::Unknown）→ Err("unknown_protocol")，
/// 不 bypass（照 peak_disabled 先例，caller 落 proxy_log blocked_reason='unknown_protocol'）。
#[tokio::test]
async fn single_platform_unknown_protocol_errs() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "p").await;
    set_unknown_platform_type(&db, p.id).await;
    let g = mk_db_group(&db, "solo-unknown", &[p.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let res =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
    assert!(res.is_err(), "unknown-protocol sole platform must Err");
    if let Err(e) = res {
        assert_eq!(
            e, "unknown_protocol",
            "expected unknown_protocol error, got: {e}"
        );
    }
}

/// 多平台分组：所有候选协议未知 → 返 Err("unknown_protocol")（区别于普通 NoCandidate）。
#[tokio::test]
async fn multi_platform_all_unknown_protocol_errs() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "p1").await;
    let p2 = mk_db_platform(&db, "p2").await;
    set_unknown_platform_type(&db, p1.id).await;
    set_unknown_platform_type(&db, p2.id).await;
    let g = mk_db_group(&db, "multi-unknown", &[p1.id, p2.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let res =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
    assert!(res.is_err(), "all unknown-protocol must Err");
    if let Err(e) = res {
        assert_eq!(
            e, "unknown_protocol",
            "expected unknown_protocol error, got: {e}"
        );
    }
}

/// 路由画布修复 3：任何路径出的快照（成功 / 路由失败）都带 max_retries，前端据此判新格式。
#[tokio::test]
async fn rcfix_snapshot_always_stamps_max_retries() {
    let db = mk_test_db().await;
    let ok = mk_db_platform(&db, "ok").await;
    let g = mk_db_group(&db, "rcfix-ok", &[ok.id]).await;
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", None, RequestKind::Chat).await.unwrap();
    assert_eq!(set.routing.max_retries, Some(g.max_retries), "success snapshot");

    let p1 = mk_db_platform(&db, "u1").await;
    let p2 = mk_db_platform(&db, "u2").await;
    set_unknown_platform_type(&db, p1.id).await;
    set_unknown_platform_type(&db, p2.id).await;
    let g2 = mk_db_group(&db, "rcfix-fail", &[p1.id, p2.id]).await;
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None, rate_body: None };
    let e = select_candidates_ctx(&db, &g2, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await.unwrap_err();
    assert_eq!(e.routing.max_retries, Some(g2.max_retries), "route-fail snapshot");
}

/// 多平台分组：仅部分候选协议未知 → 被排除，其他候选正常纳入（不影响）。
#[tokio::test]
async fn multi_platform_partial_unknown_protocol_skipped() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "p1").await; // 协议未知
    let p2 = mk_db_platform(&db, "p2").await; // 正常
    set_unknown_platform_type(&db, p1.id).await;
    let g = mk_db_group(&db, "multi-partial-unknown", &[p1.id, p2.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("partial unknown-protocol: must still have p2 candidate");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(
        set.candidates[0].platform.id, p2.id,
        "only p2 (known protocol) remains"
    );
}

/// 空平台分组 → Err("group has no platforms").
#[tokio::test]
async fn empty_group_returns_err() {
    let db = mk_test_db().await;
    let g = mk_db_group(&db, "empty", &[]).await;
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let res =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
    assert!(res.is_err(), "empty group should error");
    if let Err(err_msg) = res {
        assert!(
            err_msg.contains("no platforms"),
            "expected 'no platforms' in: {err_msg}"
        );
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
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
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
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: Some("sess-key".to_string()),
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    assert!(!set.candidates.is_empty());
}

/// model_mapping 命中 → 映射目标平台提到最前。
#[tokio::test]
async fn model_mapping_prioritizes_target_platform() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "map-p1").await;
    let p2 = mk_db_platform(&db, "map-p2").await;
    // Create group with model mapping: source "gpt-4o" → target p2
    let g = db::create_group(
        &db,
        CreateGroup {
            name: "map-group".into(),
            group_key: Some("map-group".into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
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
        },
    )
    .await
    .expect("create group");
    let inputs = vec![
        GroupPlatformInput {
            platform_id: p1.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: None,
        },
        GroupPlatformInput {
            platform_id: p2.id,
            priority: Some(1),
            weight: Some(1),
            level_priority: None,
        },
    ];
    db::set_group_platforms(&db, g.id, &inputs)
        .await
        .expect("set");
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "gpt-4o", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    // p2 should be first (mapped target)
    assert_eq!(set.candidates[0].platform.id, p2.id);
    assert_eq!(set.candidates[0].target_model, "gpt-4o-mapped");
    // 快照同条目 target_model 已回填映射目标
    let e = set.routing.entries.iter().find(|e| e.platform_id == p2.id).unwrap();
    assert_eq!(e.target_model.as_deref(), Some("gpt-4o-mapped"));
}

/// All platforms manually disabled → Err("no available platform").
#[tokio::test]
async fn all_platforms_disabled_returns_err() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "dis-p1").await;
    let p2 = mk_db_platform(&db, "dis-p2").await;
    // Disable both
    for pid in [p1.id, p2.id] {
        db::update_platform(
            &db,
            UpdatePlatform {
                id: pid,
                name: None,
                platform_type: None,
                base_url: None,
                api_key: None,
                extra: None,
                models: None,
                available_models: None,
                endpoints: None,
                enabled: None,
                status: Some(PlatformStatus::Disabled),
                manual_budgets: None,
                join_group_ids: None,
                expires_at: None,
                quota_source: None,
            },
        )
        .await
        .expect("disable");
    }
    let g = mk_db_group(&db, "dis-group", &[p1.id, p2.id]).await;
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let res =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
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
    let th = BreakerThresholds {
        failure_threshold: 1,
        open_secs: 1800,
        half_open_max: 2,
    };
    // 仅 p1 熔断 Open，p2 健康
    sched.inc_inflight(p1.id);
    sched.record_failure(p1.id, &th, now);

    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings {
        enabled: true,
        ..Default::default()
    };
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    // 有健康平台 → 只选 p2（坏的被过滤）
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p2.id);

    // p2 也熔断 → 全坏 → 回退透传，两候选都回（不 blackhole）
    sched.inc_inflight(p2.id);
    sched.record_failure(p2.id, &th, now);
    let set2 = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
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
    assert_eq!(
        v.iter().map(|g| g.platform.id).collect::<Vec<_>>(),
        vec![2, 4, 1, 3]
    );
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
    let g = db::create_group(
        &db,
        CreateGroup {
            name: "mix-fo".into(),
            group_key: Some("mix-fo".into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2,
            model_mappings: vec![],
            env_vars: vec![],
        },
    )
    .await
    .expect("create group");
    // 非 cp 给更优 priority(0)，cp 给较差 priority(1) —— 验证 coding plan 偏好覆盖 priority。
    let inputs = vec![
        GroupPlatformInput {
            platform_id: non.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: None,
        },
        GroupPlatformInput {
            platform_id: cp.id,
            priority: Some(1),
            weight: Some(1),
            level_priority: None,
        },
    ];
    db::set_group_platforms(&db, g.id, &inputs)
        .await
        .expect("set");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    assert_eq!(set.candidates.len(), 2);
    assert_eq!(
        set.candidates[0].platform.id, cp.id,
        "coding plan platform must be first despite worse priority"
    );
    assert_eq!(set.candidates[1].platform.id, non.id);
}

/// Failover 同 bucket 内 priority 不变：两个 coding plan 平台仍按 priority 升序。
#[tokio::test]
async fn failover_intra_coding_plan_bucket_keeps_priority() {
    let db = mk_test_db().await;
    let cp_a = mk_db_platform_cp(&db, "cp-a").await;
    let cp_b = mk_db_platform_cp(&db, "cp-b").await;
    let g = db::create_group(
        &db,
        CreateGroup {
            name: "cp-bucket".into(),
            group_key: Some("cp-bucket".into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2,
            model_mappings: vec![],
            env_vars: vec![],
        },
    )
    .await
    .expect("create group");
    // cp_b priority 更优(0) → 应排 cp_a(1) 之前（桶内 priority 升序）
    let inputs = vec![
        GroupPlatformInput {
            platform_id: cp_a.id,
            priority: Some(1),
            weight: Some(1),
            level_priority: None,
        },
        GroupPlatformInput {
            platform_id: cp_b.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: None,
        },
    ];
    db::set_group_platforms(&db, g.id, &inputs)
        .await
        .expect("set");
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    assert_eq!(
        set.candidates[0].platform.id, cp_b.id,
        "intra-bucket priority asc preserved"
    );
    assert_eq!(set.candidates[1].platform.id, cp_a.id);
}

/// LoadBalance 混合分组：coding plan bucket 整体在前；bucket 内加权随机不变。
/// 单 cp + 单非 cp → cp 必首（bucket 大小 1，随机不改变跨桶序）。
#[tokio::test]
async fn load_balance_coding_plan_bucket_first() {
    let db = mk_test_db().await;
    let non = mk_db_platform(&db, "lb-non").await;
    let cp = mk_db_platform_cp(&db, "lb-cp").await;
    let g = db::create_group(
        &db,
        CreateGroup {
            name: "lb-mix".into(),
            group_key: Some("lb-mix".into()),
            routing_mode: RoutingMode::LoadBalance,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2,
            model_mappings: vec![],
            env_vars: vec![],
        },
    )
    .await
    .expect("create group");
    // 给非 cp 更大 weight（加权随机更可能选它）—— 验证 coding plan 桶仍整体在前。
    let inputs = vec![
        GroupPlatformInput {
            platform_id: non.id,
            priority: Some(0),
            weight: Some(100),
            level_priority: None,
        },
        GroupPlatformInput {
            platform_id: cp.id,
            priority: Some(1),
            weight: Some(1),
            level_priority: None,
        },
    ];
    db::set_group_platforms(&db, g.id, &inputs)
        .await
        .expect("set");
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    assert_eq!(
        set.candidates[0].platform.id, cp.id,
        "coding plan bucket must lead regardless of weight"
    );
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
    assert_eq!(
        ordered[0].platform.id, 1,
        "active platform leads probe even if probe is coding plan"
    );
    assert_eq!(ordered[1].platform.id, 2);
}

/// 显式 model_mapping 目标平台仍居首，即便它非 coding plan（coding plan 偏好不得覆盖显式映射）。
#[tokio::test]
async fn explicit_mapping_overrides_coding_plan_preference() {
    let db = mk_test_db().await;
    let non = mk_db_platform(&db, "map-non").await; // 映射目标，非 coding plan
    let cp = mk_db_platform_cp(&db, "map-cp").await; // coding plan，但非映射目标
    let g = db::create_group(
        &db,
        CreateGroup {
            name: "map-cp-group".into(),
            group_key: Some("map-cp-group".into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2,
            model_mappings: vec![ModelMapping {
                source_model: "gpt-4o".to_string(),
                target_model: "gpt-4o-mapped".to_string(),
                target_platform_id: non.id,
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
            }],
            env_vars: vec![],
        },
    )
    .await
    .expect("create group");
    let inputs = vec![
        GroupPlatformInput {
            platform_id: non.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: None,
        },
        GroupPlatformInput {
            platform_id: cp.id,
            priority: Some(1),
            weight: Some(1),
            level_priority: None,
        },
    ];
    db::set_group_platforms(&db, g.id, &inputs)
        .await
        .expect("set");
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "gpt-4o", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    // 映射目标 non（非 coding plan）仍居首，coding plan cp 退居其后
    assert_eq!(
        set.candidates[0].platform.id, non.id,
        "explicit mapping target must stay first over coding plan"
    );
    assert_eq!(set.candidates[0].target_model, "gpt-4o-mapped");
}

// ── [platform-expiry-priority] 同 priority 内按 expires_at 升序优先 ──

/// 创建带 expires_at 的普通平台。
async fn mk_db_platform_exp(db: &db::Db, name: &str, expires_at: i64) -> Platform {
    db::create_platform(
        db,
        CreatePlatform {
            name: name.into(),
            platform_type: Protocol::Anthropic,
            base_url: "https://example.invalid".into(),
            api_key: "k".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: Some(expires_at),
            quota_source: None,
        },
    )
    .await
    .expect("create platform with expires_at")
}

/// Failover 同 priority 候选：未过期的平台中 expires_at 最小者优先调度。
/// 混合 3 平台同 priority=0：近未来 / 远未来 / 永不过期(0) → 期望近→远→永。
#[tokio::test]
async fn failover_prefers_earliest_expiry_within_same_priority() {
    let db = mk_test_db().await;
    let now = db::now();
    let p_near = mk_db_platform_exp(&db, "near", now + 60_000).await; // 1 分钟后过期
    let p_far = mk_db_platform_exp(&db, "far", now + 7 * 86_400_000).await; // 7 天后过期
    let p_forever = mk_db_platform_exp(&db, "forever", 0).await; // 永不过期
    // 三平台均 priority=0（同优先级），故意打乱注册顺序验证排序不受注册序影响
    let g = db::create_group(
        &db,
        CreateGroup {
            name: "exp-group".into(),
            group_key: Some("exp-group".into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2,
            model_mappings: vec![],
            env_vars: vec![],
        },
    )
    .await
    .expect("create group");
    let inputs = vec![
        GroupPlatformInput {
            platform_id: p_forever.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: None,
        },
        GroupPlatformInput {
            platform_id: p_far.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: None,
        },
        GroupPlatformInput {
            platform_id: p_near.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: None,
        },
    ];
    db::set_group_platforms(&db, g.id, &inputs)
        .await
        .expect("set");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    assert_eq!(set.candidates.len(), 3);
    // expires_at 升序：近未来 → 远未来 → 永不过期
    assert_eq!(
        set.candidates[0].platform.id, p_near.id,
        "earliest expiry must be scheduled first"
    );
    assert_eq!(
        set.candidates[1].platform.id, p_far.id,
        "farther expiry second"
    );
    assert_eq!(
        set.candidates[2].platform.id, p_forever.id,
        "never-expiring (0) goes last within same priority"
    );
}

/// expires_at=0（永不过期）排在所有有期限平台之后，即便 priority 更优也不跨 priority。
#[tokio::test]
async fn failover_priority_dominates_over_expiry_in_db() {
    let db = mk_test_db().await;
    let now = db::now();
    // 永不过期但 priority 更优(0) vs 快过期但 priority 较差(1)
    let p_noexp = mk_db_platform_exp(&db, "noexp-p0", 0).await;
    let p_expiring = mk_db_platform_exp(&db, "exp-p1", now + 60_000).await;
    let g = db::create_group(
        &db,
        CreateGroup {
            name: "prio-dom".into(),
            group_key: Some("prio-dom".into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2,
            model_mappings: vec![],
            env_vars: vec![],
        },
    )
    .await
    .expect("create group");
    // p_noexp priority=0（更优）、p_expiring priority=1（较差）
    let inputs = vec![
        GroupPlatformInput {
            platform_id: p_noexp.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: None,
        },
        GroupPlatformInput {
            platform_id: p_expiring.id,
            priority: Some(1),
            weight: Some(1),
            level_priority: None,
        },
    ];
    db::set_group_platforms(&db, g.id, &inputs)
        .await
        .expect("set");

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    // priority 主序：p_noexp(0) 居首，即便永不过期；expires_at 不跨 priority
    assert_eq!(
        set.candidates[0].platform.id, p_noexp.id,
        "priority dominates over expiry"
    );
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
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };
    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("ok");
    // 已过期 p_expired 被过滤，只剩 p_ok
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(
        set.candidates[0].platform.id, p_ok.id,
        "expired platform filtered by candidate_state, not prioritized"
    );
}

// ── 非 Failover 模式 expiry tiebreak 集成（扩展至全部模式） ──

// ── PRD 07-11: preset.models.peak 分支路由层切换 ──────────────────────────────

/// 构造一个 glm_coding 协议的 Platform（带 peak + preset.models.default 填入 platform.models）。
fn glm_coding_platform(extra: &str, platform_models: PlatformModels) -> Platform {
    let mut p = super::super::test_mod::mk_platform(PlatformStatus::Enabled, 0);
    p.platform_type = Protocol::GlmCoding;
    p.extra = extra.into();
    p.models = platform_models;
    p
}

/// 高峰窗口 06:00-10:00 UTC × 3.0（与 platform-presets.json glm_coding.peak[0] 同）。
/// 测试时间戳选 2024-01-01T07:00:00Z（hour=7 落 [6,10)）→ 命中高峰。
const PEAK_MS: i64 = (1_704_067_200 + 7 * 3600) * 1000;
/// 非高峰时间戳 2024-01-01T12:00:00Z（hour=12，[6,10) 外）。
const OFF_MS: i64 = (1_704_067_200 + 12 * 3600) * 1000;

#[test]
fn resolve_effective_models_peak_branch_replaces_default_when_in_peak_window() {
    // preset.glm_coding.models.default 填入 platform.models（创建时常见路径）
    let platform_models = PlatformModels {
        default: vec!["glm-5.2".into()],
        opus: vec!["glm-5.2".into()],
        sonnet: vec!["glm-4.7".into()],
        gpt: vec!["glm-5.2".into()],
        haiku: vec!["glm-4.5".into()],
        jev: vec![],
        ..Default::default()
    };
    // extra 无用户覆盖 → peak_for 回落 bundled preset glm_coding 默认窗口（6-10 ×3.0）
    let p = glm_coding_platform("", platform_models);
    let eff = resolve_effective_models(&p, &[], PEAK_MS, "");
    // 命中高峰 → 切到 preset.models.peak（当前 registry 全档 glm-5.3-flash）
    assert_eq!(
        eff.default,
        vec!["glm-5.3-flash"],
        "peak: default 切 glm-5.3-flash"
    );
    assert_eq!(
        eff.sonnet,
        vec!["glm-5.3-flash"],
        "peak: sonnet 切 glm-5.3-flash"
    );
    assert_eq!(
        eff.haiku,
        vec!["glm-5.3-flash"],
        "peak: haiku 切 glm-5.3-flash"
    );
}

#[test]
fn resolve_effective_models_no_peak_when_off_peak() {
    let platform_models = PlatformModels {
        default: vec!["glm-5.2".into()],
        sonnet: vec!["glm-4.7".into()],
        ..Default::default()
    };
    let p = glm_coding_platform("", platform_models);
    let eff = resolve_effective_models(&p, &[], OFF_MS, "");
    // 非高峰 → 保持 platform.models（不切 peak）
    assert_eq!(
        eff.default,
        vec!["glm-5.2"],
        "off-peak: 保持 default"
    );
    assert_eq!(
        eff.sonnet,
        vec!["glm-4.7"],
        "off-peak: 保持 sonnet"
    );
}

#[test]
fn resolve_effective_models_user_time_windows_overrides_peak_branch() {
    // 用户显式 time_windows 优先级高于 preset.models.peak（time_rules 非空 → 跳过 peak 分支）
    let platform_models = PlatformModels {
        default: vec!["glm-5.2".into()],
        ..Default::default()
    };
    // extra 含 time_windows（全天命中 → models.default=custom-model）
    let extra = serde_json::json!({
        "time_windows": [{
            "windows": [{"start_hour": 0, "end_hour": 24}],
            "models": {"default": "custom-model"}
        }]
    })
    .to_string();
    let p = glm_coding_platform(&extra, platform_models);
    let time_rules = crate::gateway::time_windows::parse_platform_time_windows(&p.extra);
    let eff = resolve_effective_models(&p, &time_rules, PEAK_MS, "");
    // time_windows 命中 → custom-model（不被 preset.models.peak 覆盖）
    assert_eq!(
        eff.default,
        vec!["custom-model"],
        "user time_windows 优先于 preset peak"
    );
}

#[test]
fn resolve_effective_models_protocol_without_peak_branch_unchanged() {
    // 协议无 preset.models.peak 分支（如 anthropic）→ 行为零变更（向后兼容）
    // mk_platform 默认协议即 anthropic
    let mut p = super::super::test_mod::mk_platform(PlatformStatus::Enabled, 0);
    p.models = PlatformModels {
        default: vec!["claude-sonnet-4".into()],
        ..Default::default()
    };
    let eff = resolve_effective_models(&p, &[], PEAK_MS, "");
    assert_eq!(
        eff.default,
        vec!["claude-sonnet-4"],
        "无 peak 分支 → 不动"
    );
}

#[test]
fn resolve_effective_models_user_peak_override_takes_effect() {
    // 用户 extra.peak 覆盖 preset 默认（peak_for 优先返用户值）
    // 此处用户窗口设为非时段（start=end=0 退化 = 全天命中）→ is_in_peak_window=true
    let platform_models = PlatformModels {
        default: vec!["glm-5.2".into()],
        ..Default::default()
    };
    let extra = serde_json::json!({
        "peak": [{"start_hour": 0, "end_hour": 0, "multiplier": 1.5}]
    })
    .to_string();
    let p = glm_coding_platform(&extra, platform_models);
    // OFF_MS（hour=12，正常非高峰）但用户窗口全天命中 → 仍切 peak
    let eff = resolve_effective_models(&p, &[], OFF_MS, "");
    assert_eq!(
        eff.default,
        vec!["glm-5.3-flash"],
        "用户 peak 命中 → 切 peak"
    );
}

// ── sole_platform: 单启用平台分组短路（single-enabled-platform-shortcut）──

/// ① 3 平台仅 1 enabled（其余手动 disabled）→ 走单平台短路路径，唯一 enabled 平台必被选中。
#[tokio::test]
async fn sole_enabled_platform_among_three_shortcuts() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "only-enabled").await;
    let p2 = mk_db_platform(&db, "disabled-2").await;
    let p3 = mk_db_platform(&db, "disabled-3").await;
    for pid in [p2.id, p3.id] {
        db::update_platform(
            &db,
            UpdatePlatform {
                id: pid,
                name: None,
                platform_type: None,
                base_url: None,
                api_key: None,
                extra: None,
                models: None,
                available_models: None,
                endpoints: None,
                enabled: None,
                status: Some(PlatformStatus::Disabled),
                manual_budgets: None,
                join_group_ids: None,
                expires_at: None,
                quota_source: None,
            },
        )
        .await
        .expect("disable");
    }
    let g = mk_db_group(&db, "sole-enabled", &[p1.id, p2.id, p3.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("sole enabled platform must force request via single-platform shortcut");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p1.id);
}

/// ② 2 enabled + 1 auto_disabled（未到期）→ 不走短路，走正常多平台路径（仅 2 个 enabled 候选）。
#[tokio::test]
async fn two_enabled_one_auto_disabled_no_shortcut() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "enabled-1").await;
    let p2 = mk_db_platform(&db, "enabled-2").await;
    let p3 = mk_db_platform(&db, "auto-disabled-3").await;
    db::test_support::set_legacy_auto_disabled(&db, p3.id, 1, db::now() + 3_600_000).await;
    let g = mk_db_group(&db, "two-enabled", &[p1.id, p2.id, p3.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("two enabled platforms: normal multi-platform routing");
    let ids: Vec<u64> = set.candidates.iter().map(|c| c.platform.id).collect();
    assert!(
        ids.contains(&p1.id) && ids.contains(&p2.id),
        "both enabled platforms present: {ids:?}"
    );
    assert!(
        !ids.contains(&p3.id),
        "auto_disabled (not yet due) platform excluded: {ids:?}"
    );
}

/// ③ 0 enabled 且总数 >1（均 auto_disabled 未到期）→ 不走短路（sole_platform 返 None），
/// 全部候选被过滤 → Err（与短路「无视状态必请求」行为相反，验证未误短路）。
#[tokio::test]
async fn zero_enabled_multi_platform_no_shortcut() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "auto-disabled-1").await;
    let p2 = mk_db_platform(&db, "auto-disabled-2").await;
    for pid in [p1.id, p2.id] {
        db::test_support::set_legacy_auto_disabled(&db, pid, 1, db::now() + 3_600_000).await;
    }
    let g = mk_db_group(&db, "zero-enabled", &[p1.id, p2.id]).await;

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let res =
        select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat).await;
    assert!(
        res.is_err(),
        "zero enabled, no shortcut: all auto_disabled-not-due must Err, not force request"
    );
}

/// 失败降权（health-aware，2026-09-29）：降权窗口内的平台沉到候选末尾，
/// 降权平台之间保持原排序；未降权平台在前。排序不踢出候选。
#[tokio::test]
async fn penalty_demotes_platform_to_last() {
    let db = mk_test_db().await;
    let p1 = mk_db_platform(&db, "penalized-1").await;
    let p2 = mk_db_platform(&db, "healthy").await;
    let p3 = mk_db_platform(&db, "penalized-2").await;
    let g = mk_db_group_mode(&db, "lb", &[p1.id, p2.id, p3.id], RoutingMode::LoadBalance).await;

    let sched = SchedulerState::new();
    let now = db::now();
    sched.record_penalty(p1.id, crate::gateway::scheduling::PenaltyTier::Server, now);
    sched.record_penalty(p3.id, crate::gateway::scheduling::PenaltyTier::Client, now);

    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings::default();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("penalty only reorders, never excludes");
    let ids: Vec<u64> = set.candidates.iter().map(|c| c.platform.id).collect();
    // LoadBalance 的加权随机选首用时间种子（order_load_balance(now_ms)，now_ms 来自
    // select_candidates_ctx 内的 db::now()，无注入点）：选首命中 penalized 平台时，两个
    // 降权者沉底后的相对顺序随之翻转。断言取序不变式：healthy 恒在首位，penalized 以任意顺序殿后。
    assert_eq!(ids[0], p2.id, "healthy platform must stay first");
    assert_eq!(
        {
            let mut tail = ids[1..].to_vec();
            tail.sort_unstable();
            tail
        },
        vec![p1.id.min(p3.id), p1.id.max(p3.id)],
        "penalized platforms must fill the tail in any order, got {ids:?}"
    );
}

// ══ jev-decision-proxy：请求类型维度（R2/R3/R5/R6/R8）══

use aidog_db::models::ModelEntry;

/// 建带槽位/available_models 的平台。
#[allow(clippy::too_many_arguments)]
async fn mk_platform_models(
    db: &db::Db,
    name: &str,
    models: PlatformModels,
    available_models: Vec<&str>,
) -> Platform {
    db::create_platform(
        db,
        CreatePlatform {
            name: name.into(),
            platform_type: Protocol::OpenAI,
            base_url: "https://example.invalid/v1".into(),
            api_key: "k".into(),
            extra: String::new(),
            models: Some(models),
            available_models: Some(available_models.into_iter().map(String::from).collect()),
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("create platform")
}

fn slot_jev(v: &str) -> PlatformModels {
    PlatformModels {
        jev: vec![v.into()],
        ..Default::default()
    }
}

fn slot_default(v: &str) -> PlatformModels {
    PlatformModels {
        default: vec![v.into()],
        ..Default::default()
    }
}

fn decision_entry(model_id: &str) -> ModelEntry {
    decision_entry_on("openai", model_id)
}

/// 指定 registry 平台的 decision 能力条目（`mk_platform_models` 建的平台协议是 openai）。
fn decision_entry_on(platform_code: &str, model_id: &str) -> ModelEntry {
    ModelEntry {
        platform_code: platform_code.into(),
        model_id: model_id.into(),
        display_name: model_id.into(),
        canonical_model: model_id.into(),
        family: String::new(),
        version: String::new(),
        predecessor: String::new(),
        capabilities: vec!["decision".into()],
        builtin_tools_excluded: vec![],
        max_input_tokens: None,
        max_output_tokens: None,
        context_window: Some(64000),
        official: true,
        price_data: String::new(),
        updated_at: db::now(),
    }
}

/// R2 来源一：jev 槽位平台可被决策请求选中；上游 model = 槽位值（R8）。
#[tokio::test]
async fn decision_request_selects_jev_slot_platform_and_model() {
    let db = mk_test_db().await;
    let jev_p = mk_platform_models(&db, "ts", slot_jev("jev-latest"), vec![]).await;
    let chat_p = mk_platform_models(&db, "glm", slot_default("glm-5.2"), vec![]).await;
    let g = mk_db_group(&db, "grp", &[jev_p.id, chat_p.id]).await;

    let set = select_candidates_ctx(&db, &g, "anything", None, RequestKind::Decision)
        .await
        .expect("jev platform must be selectable for decision");
    assert_eq!(set.candidates.len(), 1, "chat-only platform filtered out");
    assert_eq!(set.candidates[0].platform.id, jev_p.id);
    assert_eq!(set.candidates[0].target_model, "jev-latest");
}

/// R2 来源二：无 jev 槽位但 available_models 含 registry decision 能力模型 → 推导支持。
#[tokio::test]
async fn decision_request_derives_support_from_available_models() {
    let db = mk_test_db().await;
    db::upsert_model_entries(&db, vec![decision_entry("jev-1.13")])
        .await
        .unwrap();
    let openrouter = mk_platform_models(&db, "or", slot_default("gpt-x"), vec!["jev-1.13"]).await;
    let chat_p = mk_platform_models(&db, "glm", slot_default("glm-5.2"), vec![]).await;
    let g = mk_db_group(&db, "grp", &[openrouter.id, chat_p.id]).await;

    let set = select_candidates_ctx(&db, &g, "unknown-model", None, RequestKind::Decision)
        .await
        .expect("derived platform must be selectable");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, openrouter.id);
    // R8：请求模型不在 available_models → 用列表第一个 decision 模型
    assert_eq!(set.candidates[0].target_model, "jev-1.13");
}

/// R2 按平台判定：同名模型只在别的平台（anthropic）的 registry 条目带 decision → 本平台（openai）不算支持。
#[tokio::test]
async fn decision_capability_is_per_platform_registry_entry() {
    let db = mk_test_db().await;
    db::upsert_model_entries(&db, vec![decision_entry_on("anthropic", "jev-1.13")])
        .await
        .unwrap();
    let p = mk_platform_models(&db, "or", slot_default("gpt-x"), vec!["jev-1.13"]).await;
    let g = mk_db_group(&db, "grp", &[p.id]).await;
    let e = select_candidates_ctx(&db, &g, "jev-1.13", None, RequestKind::Decision)
        .await
        .expect_err("other platform's decision entry must not count");
    assert_eq!(e, "no_decision_platform");

    // 本平台自己的条目补上 → 支持
    db::upsert_model_entries(&db, vec![decision_entry_on("openai", "jev-1.13")])
        .await
        .unwrap();
    let set = select_candidates_ctx(&db, &g, "x", None, RequestKind::Decision)
        .await
        .expect("own registry entry has decision");
    assert_eq!(set.candidates[0].target_model, "jev-1.13");
}

/// R8：推导命中且请求模型在 available_models 里 → 保留请求模型。
#[tokio::test]
async fn decision_model_keeps_requested_when_available() {
    let db = mk_test_db().await;
    db::upsert_model_entries(&db, vec![decision_entry("jev-1.13")])
        .await
        .unwrap();
    let p = mk_platform_models(&db, "or", slot_default("gpt-x"), vec!["gpt-x", "jev-1.13"]).await;
    let g = mk_db_group(&db, "grp", &[p.id]).await;
    let set = select_candidates_ctx(&db, &g, "gpt-x", None, RequestKind::Decision)
        .await
        .expect("single derived platform");
    assert_eq!(set.candidates[0].target_model, "gpt-x");
}

/// R3：纯决策平台被聊天请求剔除；全组只剩纯决策平台 → no_chat_platform。
#[tokio::test]
async fn chat_request_skips_decision_only_platform() {
    let db = mk_test_db().await;
    let only = mk_platform_models(&db, "ts", slot_jev("jev-latest"), vec![]).await;
    let mixed = mk_platform_models(
        &db,
        "or",
        PlatformModels {
            jev: vec!["jev-latest".into()],
            default: vec!["gpt-x".into()],
            ..Default::default()
        },
        vec![],
    )
    .await;
    let g = mk_db_group(&db, "grp", &[only.id, mixed.id]).await;

    let set = select_candidates_ctx(&db, &g, "gpt-x", None, RequestKind::Chat)
        .await
        .expect("mixed platform still serves chat");
    assert_eq!(set.candidates.len(), 1, "decision-only platform skipped");
    assert_eq!(set.candidates[0].platform.id, mixed.id);

    // 全组只剩纯决策平台 → no_chat_platform
    let g2 = mk_db_group(&db, "grp2", &[only.id]).await;
    let err = select_candidates_ctx(&db, &g2, "gpt-x", None, RequestKind::Chat)
        .await
        .expect_err("all decision-only must Err");
    assert_eq!(err, "no_chat_platform");
}

/// R5：单平台分组两方向——决策平台收聊天请求 / 聊天平台收决策请求都 Err。
#[tokio::test]
async fn single_platform_kind_mismatch_errs_both_directions() {
    let db = mk_test_db().await;
    let jev_p = mk_platform_models(&db, "ts", slot_jev("jev-latest"), vec![]).await;
    let g1 = mk_db_group(&db, "grp1", &[jev_p.id]).await;
    let e = select_candidates_ctx(&db, &g1, "m", None, RequestKind::Chat)
        .await
        .err()
        .unwrap();
    assert_eq!(e, "no_chat_platform");

    let chat_p = mk_platform_models(&db, "glm", slot_default("glm-5.2"), vec![]).await;
    let g2 = mk_db_group(&db, "grp2", &[chat_p.id]).await;
    let e = select_candidates_ctx(&db, &g2, "m", None, RequestKind::Decision)
        .await
        .err()
        .unwrap();
    assert_eq!(e, "no_decision_platform");
}

/// R6：model_mappings 显式目标平台不支持该请求类型 → 直接 Err（不忽略映射）。
#[tokio::test]
async fn mapping_target_kind_mismatch_errs() {
    let db = mk_test_db().await;
    let jev_p = mk_platform_models(&db, "ts", slot_jev("jev-latest"), vec![]).await;
    let chat_p = mk_platform_models(&db, "glm", slot_default("glm-5.2"), vec![]).await;
    let g = mk_db_group(&db, "grp", &[jev_p.id, chat_p.id]).await;

    // 聊天请求显式映射到纯决策平台 → no_chat_platform（即使组内有可用聊天平台）
    db::update_group(
        &db,
        db::UpdateGroup {
            id: g.id,
            name: None,
            routing_mode: None,
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: None,
            model_mappings: vec![ModelMapping {
                source_model: "glm-5.2".into(),
                target_platform_id: jev_p.id,
                target_model: "glm-5.2".into(),
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
            }],
            env_vars: vec![],
            is_default: None,
        },
    )
    .await
    .unwrap();
    // select 按 group struct 的 model_mappings 匹配 → 重取更新后的 group
    let g = db::get_group(&db, g.id).await.unwrap().unwrap();
    let e = select_candidates_ctx(&db, &g, "glm-5.2", None, RequestKind::Chat)
        .await
        .expect_err("mapping to decision-only platform must Err");
    assert_eq!(e, "no_chat_platform");
}

// ── 映射后再走槽位匹配（2026-10-01 用户确认：映射只改名，不跳过槽位规则）──

/// 给 group 写一条映射（source → target_platform 的 target_model），返回重取后的 group。
async fn with_mapping(
    db: &db::Db,
    g: &Group,
    source: &str,
    platform_id: u64,
    target: &str,
) -> Group {
    db::update_group(
        db,
        db::UpdateGroup {
            id: g.id,
            name: None,
            routing_mode: None,
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: None,
            model_mappings: vec![ModelMapping {
                source_model: source.into(),
                target_platform_id: platform_id,
                target_model: target.into(),
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
            }],
            env_vars: vec![],
            is_default: None,
        },
    )
    .await
    .unwrap();
    db::get_group(db, g.id).await.unwrap().unwrap()
}

fn slots_opus_default() -> PlatformModels {
    PlatformModels {
        opus: vec!["glm-opus".into()],
        default: vec!["glm-d".into()],
        ..Default::default()
    }
}

/// 映射名含 opus → 命中目标平台 opus 槽（多平台路径；映射目标平台仍居首）。
#[tokio::test]
async fn mapped_name_hits_opus_slot() {
    let db = mk_test_db().await;
    let other = mk_platform_models(&db, "other", slot_default("x-d"), vec![]).await;
    let p = mk_platform_models(&db, "p", slots_opus_default(), vec![]).await;
    let g = mk_db_group(&db, "grp", &[other.id, p.id]).await;
    let g = with_mapping(&db, &g, "src", p.id, "claude-opus-4").await;
    let set = select_candidates_ctx(&db, &g, "src", None, RequestKind::Chat)
        .await
        .unwrap();
    assert_eq!(
        set.candidates[0].platform.id, p.id,
        "mapping target platform first"
    );
    assert_eq!(set.candidates[0].target_model, "glm-opus");
    // 其余候选同样用映射名做槽位匹配（无档位命中 → default）
    assert_eq!(set.candidates[1].target_model, "x-d");
}

/// 映射名无档位词 → 回落 default 槽（单平台路径）。
#[tokio::test]
async fn mapped_name_without_tier_falls_to_default_slot() {
    let db = mk_test_db().await;
    let p = mk_platform_models(&db, "p", slots_opus_default(), vec![]).await;
    let g = mk_db_group(&db, "grp", &[p.id]).await;
    let g = with_mapping(&db, &g, "src", p.id, "plain-model").await;
    let set = select_candidates_ctx(&db, &g, "src", None, RequestKind::Chat)
        .await
        .unwrap();
    assert_eq!(set.candidates[0].target_model, "glm-d");
}

/// 无 default 槽且映射名无档位词 → 透传映射名。
#[tokio::test]
async fn mapped_name_passthrough_without_default_slot() {
    let db = mk_test_db().await;
    let p = mk_platform_models(
        &db,
        "p",
        PlatformModels {
            opus: vec!["glm-opus".into()],
            ..Default::default()
        },
        vec![],
    )
    .await;
    let g = mk_db_group(&db, "grp", &[p.id]).await;
    let g = with_mapping(&db, &g, "src", p.id, "plain-model").await;
    let set = select_candidates_ctx(&db, &g, "src", None, RequestKind::Chat)
        .await
        .unwrap();
    assert_eq!(set.candidates[0].target_model, "plain-model");
}

/// 决策请求：映射命中后仍走 jev 槽（槽位值优先于映射名）。
#[tokio::test]
async fn decision_mapping_still_uses_jev_slot() {
    let db = mk_test_db().await;
    let jev_p = mk_platform_models(&db, "ts", slot_jev("jev-latest"), vec![]).await;
    let g = mk_db_group(&db, "grp", &[jev_p.id]).await;
    let g = with_mapping(&db, &g, "m", jev_p.id, "jev-other").await;
    let set = select_candidates_ctx(&db, &g, "m", None, RequestKind::Decision)
        .await
        .unwrap();
    assert_eq!(set.candidates[0].target_model, "jev-latest");
}

/// 全组无任何决策能力平台 → no_decision_platform。
#[tokio::test]
async fn all_group_lacks_decision_errs() {
    let db = mk_test_db().await;
    let p1 = mk_platform_models(&db, "a", slot_default("m1"), vec![]).await;
    let p2 = mk_platform_models(&db, "b", slot_default("m2"), vec![]).await;
    let g = mk_db_group(&db, "grp", &[p1.id, p2.id]).await;
    let e = select_candidates_ctx(&db, &g, "m", None, RequestKind::Decision)
        .await
        .err()
        .unwrap();
    assert_eq!(e, "no_decision_platform");
}

/// 批量覆盖持久化的空串 jev 槽不算决策能力；空串聊天槽也不让 jev 平台变成「可聊天」。
#[tokio::test]
async fn blank_jev_slot_is_not_decision_capable() {
    let db = mk_test_db().await;
    let blank = mk_platform_models(
        &db,
        "blank",
        PlatformModels {
            default: vec!["gpt-x".into()],
            jev: vec!["".into()],
            ..Default::default()
        },
        vec![],
    )
    .await;
    let g = mk_db_group(&db, "grp", &[blank.id]).await;
    let e = select_candidates_ctx(&db, &g, "m", None, RequestKind::Decision)
        .await
        .expect_err("blank jev must not route decision");
    assert_eq!(e, "no_decision_platform");

    let only = mk_platform_models(
        &db,
        "only",
        PlatformModels {
            jev: vec!["jev-latest".into()],
            default: vec!["".into()],
            sonnet: vec!["".into()],
            ..Default::default()
        },
        vec![],
    )
    .await;
    let g2 = mk_db_group(&db, "grp2", &[only.id]).await;
    let e = select_candidates_ctx(&db, &g2, "m", None, RequestKind::Chat)
        .await
        .expect_err("blank chat slots keep platform decision-only");
    assert_eq!(e, "no_chat_platform");
}

// ── 透传平台路由（ADR 0008）：通配命中 + 绕过状态机 ──

/// 创建透传平台：Protocol::Passthrough，可选 extra（peak/认证配置）与 models 槽位。
async fn mk_db_platform_passthrough(
    db: &db::Db,
    name: &str,
    extra: &str,
    models: Option<db::models::PlatformModels>,
) -> Platform {
    db::create_platform(
        db,
        CreatePlatform {
            name: name.into(),
            platform_type: Protocol::Passthrough,
            base_url: "https://passthrough.invalid".into(),
            api_key: "k".into(),
            extra: extra.into(),
            models,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("create passthrough platform")
}

/// 通配命中：任意 source_model 均可命中透传平台，且不吃模型槽改写
/// （即便平台配了 default 槽，target_model 仍是请求模型名原样——body 原样转发）。
#[tokio::test]
async fn passthrough_wildcard_matches_any_model_no_slot_rewrite() {
    let db = mk_test_db().await;
    let p = mk_db_platform_passthrough(
        &db,
        "pt",
        "",
        Some(db::models::PlatformModels {
            default: vec!["slot-model".into()],
            ..Default::default()
        }),
    )
    .await;
    let g = mk_db_group(&db, "single-pt", &[p.id]).await;

    let set = select_candidates_ctx(&db, &g, "totally-unknown-model", None, RequestKind::Chat)
        .await
        .expect("passthrough platform matches any source model");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, p.id);
    assert_eq!(
        set.candidates[0].target_model, "totally-unknown-model",
        "no slot rewrite: target_model is the request model verbatim"
    );
}

/// 绕过状态机：auto_disabled（退避未到期）+ 熔断 Reject + 配额冷却同时命中，
/// 透传平台仍进候选（永远在线，ADR 0008 决策 2）。
#[tokio::test]
async fn passthrough_bypasses_auto_disabled_breaker_and_cooldown() {
    let db = mk_test_db().await;
    let p = mk_db_platform_passthrough(&db, "pt", "", None).await;
    let other = mk_db_platform(&db, "normal").await;
    let g = mk_db_group(&db, "multi", &[p.id, other.id]).await;
    // 普通平台手动 Disabled：既避免 sole_platform 短路（enabled 恰 1 个会绕过整个过滤），
    // 又作为「正常平台被排除」的对照。
    db::update_platform(
        &db,
        UpdatePlatform {
            id: other.id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: Some(PlatformStatus::Disabled),
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("disable normal platform");

    // auto_disabled（DB 持久态，退避 1h 未到期）
    db::test_support::set_legacy_auto_disabled(&db, p.id, 1, db::now() + 3_600_000).await;
    // 熔断 Open + 配额冷却（内存态）
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let now = db::now();
    let th = BreakerThresholds {
        failure_threshold: 1,
        open_secs: 1800,
        half_open_max: 2,
    };
    sched.inc_inflight(p.id);
    sched.record_failure(p.id, &th, now);
    assert_eq!(sched.admission(p.id, &th, now, true), Admission::Reject);
    // 配额冷却：找一个必然在未来的重置时间
    sched.set_quota_cooldown(p.id, now + 3_600_000);
    let settings = SchedulingBreakerSettings {
        enabled: true,
        ..Default::default()
    };
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let set = select_candidates_ctx(&db, &g, "any-model", Some(&ctx), RequestKind::Chat)
        .await
        .expect("passthrough stays online");
    assert!(
        set.candidates.iter().any(|c| c.platform.id == p.id),
        "passthrough must bypass auto_disabled / breaker / quota cooldown"
    );
}

/// disable_during_peak + 命中窗口：透传平台照常在线（多平台不被排除、单平台组不 peak_disabled 硬停）。
#[tokio::test]
async fn passthrough_bypasses_disable_during_peak() {
    let db = mk_test_db().await;
    // 24h 全天命中窗口
    let extra =
        r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#;
    let p = mk_db_platform_passthrough(&db, "pt", extra, None).await;
    let g = mk_db_group(&db, "single-pt", &[p.id]).await;

    // 单平台组：正常平台此形态返 Err("peak_disabled")，透传平台例外仍 Ok
    let set = select_candidates_ctx(&db, &g, "m", None, RequestKind::Chat)
        .await
        .expect("passthrough sole platform: peak-disabled must not block (always-online)");
    assert_eq!(set.candidates[0].platform.id, p.id);
}

/// 绕过有边界：手动 Disabled（用户显式关停）与过期仍排除——永远在线只覆盖自动摘除维度。
#[tokio::test]
async fn passthrough_manual_disabled_and_expired_still_excluded() {
    let db = mk_test_db().await;
    let p = mk_db_platform_passthrough(&db, "pt", "", None).await;
    let g = mk_db_group(&db, "single-pt", &[p.id]).await;

    // 手动 Disabled → 单平台组 Err
    db::update_platform(
        &db,
        UpdatePlatform {
            id: p.id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: Some(PlatformStatus::Disabled),
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("disable");
    let res = select_candidates_ctx(&db, &g, "m", None, RequestKind::Chat).await;
    assert!(
        res.is_err(),
        "manually disabled passthrough must stay excluded"
    );

    // 恢复 enabled + 置过期 + 加第二个 enabled 平台（避开单平台短路——该路径不查
    // expires_at，与平台类型无关的既有语义）→ 过期平台走多平台过滤仍排除
    db::update_platform(
        &db,
        UpdatePlatform {
            id: p.id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: Some(PlatformStatus::Enabled),
            manual_budgets: None,
            join_group_ids: None,
            expires_at: Some(db::now() - 1),
            quota_source: None,
        },
    )
    .await
    .expect("re-enable but expire");
    let other = mk_db_platform(&db, "normal2").await;
    let g2 = mk_db_group(&db, "multi-pt", &[p.id, other.id]).await;
    let set = select_candidates_ctx(&db, &g2, "m", None, RequestKind::Chat)
        .await
        .expect("non-expired sibling keeps the group routable");
    assert!(
        !set.candidates.iter().any(|c| c.platform.id == p.id),
        "expired passthrough must stay excluded"
    );
}

// ── 票 01：路由决策快照——排除原因标签逐个覆盖 ──

async fn disable_platform(db: &db::Db, id: u64) {
    db::update_platform(
        db,
        aidog_db::models::UpdatePlatform {
            id,
            status: Some(PlatformStatus::Disabled),
                    name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("disable platform");
}

/// 手动禁用：快照带 disabled-manual 标签，选中候选不带走。
/// 注意候选须有两个 Enabled 平台，否则命中 sole_platform 单启用短路（不进 filter）。
#[tokio::test]
async fn snapshot_labels_manual_disabled() {
    let db = mk_test_db().await;
    let live = mk_db_platform(&db, "LIVE").await;
    let live2 = mk_db_platform(&db, "LIVE2").await;
    let off = mk_db_platform(&db, "OFF").await;
    disable_platform(&db, off.id).await;
    let g = mk_db_group(&db, "g-manual", &[live.id, live2.id, off.id]).await;

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", None, RequestKind::Chat)
        .await
        .expect("live candidate available");
    assert_eq!(set.candidates.len(), 2);
    assert_eq!(set.routing.entries.len(), 3);
    let e_live = set.routing.entries.iter().find(|e| e.platform_id == live.id).unwrap();
    let e_off = set.routing.entries.iter().find(|e| e.platform_id == off.id).unwrap();
    assert_eq!(e_live.reason, RouteReason::Selected);
    assert_eq!(e_live.order, 1);
    assert_eq!(e_off.reason, RouteReason::DisabledManual);
    assert_eq!(set.routing.mode, Some(g.routing_mode));
    // 目标模型回填：选中候选 = 解析结果；排除候选算不到 = None
    assert_eq!(
        e_live.target_model.as_deref(),
        Some(set.candidates[0].target_model.as_str())
    );
    assert_eq!(e_off.target_model, None);
}

/// 熔断：Reject 的候选带 breaker 标签（非回退路径）。
#[tokio::test]
async fn snapshot_labels_breaker_rejected() {
    let db = mk_test_db().await;
    let broken = mk_db_platform(&db, "BROKEN").await;
    let live = mk_db_platform(&db, "LIVE").await;
    let g = mk_db_group(&db, "g-breaker", &[broken.id, live.id]).await;

    let sched = SchedulerState::new();
    let now = db::now();
    let th = BreakerThresholds {
        failure_threshold: 1,
        open_secs: 1800,
        half_open_max: 2,
    };
    sched.inc_inflight(broken.id);
    sched.record_failure(broken.id, &th, now);
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings {
        enabled: true,
        ..Default::default()
    };
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: None,
        rate_body: None,
    };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("live candidate available");
    assert_eq!(set.candidates.len(), 1);
    let e_broken = set.routing.entries.iter().find(|e| e.platform_id == broken.id).unwrap();
    assert_eq!(e_broken.reason, RouteReason::Breaker);
}

/// 高峰禁用：开关 on + 窗口命中（0-24 恒命中）→ peak-disabled 标签。
#[tokio::test]
async fn snapshot_labels_peak_disabled() {
    let db = mk_test_db().await;
    let peaky = mk_db_platform(&db, "PEAKY").await;
    let live = mk_db_platform(&db, "LIVE").await;
    db::update_platform(
        &db,
        aidog_db::models::UpdatePlatform {
            id: peaky.id,
            extra: Some(
                r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#
                    .into(),
            ),
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
            status: None,
        },
    )
    .await
    .expect("set peak extra");
    let g = mk_db_group(&db, "g-peak", &[peaky.id, live.id]).await;

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", None, RequestKind::Chat)
        .await
        .expect("live candidate available");
    let e_peaky = set.routing.entries.iter().find(|e| e.platform_id == peaky.id).unwrap();
    assert_eq!(e_peaky.reason, RouteReason::PeakDisabled);
    // 整组未全被高峰禁用 → 不触发 peak_disabled Err（行为不变）
    assert_eq!(set.candidates.len(), 1);
}

/// 协议未知：unknown-protocol 标签；整组皆未知 → Err("unknown_protocol") 且快照随 Err 携带。
#[tokio::test]
async fn snapshot_labels_unknown_protocol_and_err_carries_snapshot() {
    let db = mk_test_db().await;
    let unknown = mk_db_platform_with(&db, "UNK", Protocol::Unknown, "").await;
    let live = mk_db_platform(&db, "LIVE").await;
    let g = mk_db_group(&db, "g-unknown", &[unknown.id, live.id]).await;

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", None, RequestKind::Chat)
        .await
        .expect("live candidate available");
    let e_unknown = set.routing.entries.iter().find(|e| e.platform_id == unknown.id).unwrap();
    assert_eq!(e_unknown.reason, RouteReason::UnknownProtocol);

    let g_all_unknown = mk_db_group(&db, "g-unknown-all", &[unknown.id]).await;
    let err = select_candidates_ctx(&db, &g_all_unknown, "claude-opus-4-8", None, RequestKind::Chat)
        .await
        .expect_err("all unknown must fail");
    assert!(err == "unknown_protocol");
    assert_eq!(err.routing.entries.len(), 1);
    assert_eq!(err.routing.entries[0].reason, RouteReason::UnknownProtocol);
}

/// coding plan 上浮位置标签：cp 平台带 coding-plan 标签。
#[tokio::test]
async fn snapshot_tags_coding_plan_platform() {
    let db = mk_test_db().await;
    let cp = mk_db_platform_cp(&db, "CP").await;
    let plain = mk_db_platform(&db, "PLAIN").await;
    let g = mk_db_group_mode(&db, "g-cp", &[plain.id, cp.id], RoutingMode::Failover).await;

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", None, RequestKind::Chat)
        .await
        .expect("candidates available");
    let e_cp = set.routing.entries.iter().find(|e| e.platform_id == cp.id).unwrap();
    let e_plain = set.routing.entries.iter().find(|e| e.platform_id == plain.id).unwrap();
    assert_eq!(e_cp.tags, vec![RouteTag::CodingPlan]);
    assert!(e_plain.tags.is_empty());
}

/// 单平台组短路：成功路径落 selected 快照；peak 硬停 Err 快照带 peak-disabled。
#[tokio::test]
async fn snapshot_single_platform_selected_and_peak_blocked() {
    let db = mk_test_db().await;
    let only = mk_db_platform(&db, "ONLY").await;
    db::update_platform(
        &db,
        aidog_db::models::UpdatePlatform {
            id: only.id,
            extra: Some(
                r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#
                    .into(),
            ),
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
            status: None,
        },
    )
    .await
    .expect("set peak extra");
    let g = mk_db_group(&db, "g-single", &[only.id]).await;

    let err = select_candidates_ctx(&db, &g, "claude-opus-4-8", None, RequestKind::Chat)
        .await
        .expect_err("single platform peak-disabled is hard stop");
    assert!(err == "peak_disabled");
    assert_eq!(err.routing.entries.len(), 1);
    assert_eq!(err.routing.entries[0].reason, RouteReason::PeakDisabled);
}

async fn mk_db_platform_with(
    db: &db::Db,
    name: &str,
    platform_type: Protocol,
    extra: &str,
) -> Platform {
    db::create_platform(
        db,
        CreatePlatform {
            name: name.into(),
            platform_type,
            base_url: "https://example.invalid".into(),
            api_key: "k".into(),
            extra: extra.into(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("create platform")
}

// ── 直通模型链（直接模型转发票 01）──

/// 三文件库 fixture：直通链测试走 get_group_platforms（platform/group split 表），
/// 内存库三 handle 同连接测不出错路由（db-pool-routing.md），一律用 three_file_db。
async fn mk_three_file_db() -> (db::Db, tempfile::TempDir) {
    aidog_db::test_support::three_file_db().await
}

async fn set_extra(db: &db::Db, id: u64, extra: &str) {
    db::update_platform(
        db,
        UpdatePlatform {
            id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: Some(extra.into()),
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("set extra");
}

/// 直通命中：声明平台唯一候选，转发模型名 = 请求原名（不进槽位匹配）。
#[tokio::test]
async fn direct_model_hits_with_original_name() {
    let (db, _dir) = mk_three_file_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    set_extra(&db, a.id, r#"{"direct_models":["my-custom-model"]}"#).await;
    let g = mk_db_group(&db, "direct", &[a.id, b.id]).await;

    let set = select_candidates_ctx(&db, &g, "my-custom-model", None, RequestKind::Chat)
        .await
        .expect("direct hit");
    assert_eq!(set.candidates.len(), 1, "only the declarer is a candidate");
    assert_eq!(set.candidates[0].platform.id, a.id);
    assert_eq!(
        set.candidates[0].target_model, "my-custom-model",
        "forwarded name stays original (no slot rewrite)"
    );
}

/// 组内 failover：多个声明平台按现有排序（priority）逐个试，均转发原名。
#[tokio::test]
async fn direct_model_failover_across_declarers() {
    let (db, _dir) = mk_three_file_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    set_extra(&db, a.id, r#"{"direct_models":["m1"]}"#).await;
    set_extra(&db, b.id, r#"{"direct_models":["m1"]}"#).await;
    let g = mk_db_group(&db, "direct-fo", &[a.id, b.id]).await;

    let set = select_candidates_ctx(&db, &g, "m1", None, RequestKind::Chat)
        .await
        .expect("direct failover");
    assert_eq!(set.candidates.len(), 2);
    assert_eq!(set.candidates[0].platform.id, a.id);
    assert_eq!(set.candidates[1].platform.id, b.id);
    assert!(set.candidates.iter().all(|c| c.target_model == "m1"));
}

/// 声明平台全不可用（手动 Disabled）→ Err("direct_model_unavailable")，
/// 快照随 Err 携带且排除原因细分。
#[tokio::test]
async fn direct_model_all_unavailable_errs_with_snapshot() {
    let (db, _dir) = mk_three_file_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    set_extra(&db, a.id, r#"{"direct_models":["m1"]}"#).await;
    db::update_platform(
        &db,
        UpdatePlatform {
            id: a.id,
            status: Some(PlatformStatus::Disabled),
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("disable declarer");
    let g = mk_db_group(&db, "direct-dead", &[a.id, b.id]).await;

    let e = select_candidates_ctx(&db, &g, "m1", None, RequestKind::Chat)
        .await
        .expect_err("all declarers unavailable must Err");
    assert_eq!(&*e, "direct_model_unavailable");
    let dead = e
        .routing
        .entries
        .iter()
        .find(|en| en.platform_id == a.id)
        .unwrap();
    assert_eq!(dead.reason, RouteReason::DisabledManual);
}

/// 无人声明该模型 → 默认链原样（候选回到全组，槽位匹配生效）。
#[tokio::test]
async fn direct_model_not_declared_falls_back_default_chain() {
    let (db, _dir) = mk_three_file_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    set_extra(&db, a.id, r#"{"direct_models":["other-model"]}"#).await;
    let g = mk_db_group(&db, "direct-fallback", &[a.id, b.id]).await;

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", None, RequestKind::Chat)
        .await
        .expect("default chain");
    assert_eq!(set.candidates.len(), 2, "whole group via default chain");
    assert_eq!(set.candidates[0].target_model, "claude-opus-4-8");
}

/// 组内无任何 direct_models → 映射行为零回归：目标平台提首 + 槽位匹配照旧。
#[tokio::test]
async fn no_direct_models_group_mapping_unchanged() {
    let (db, _dir) = mk_three_file_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    let g = mk_db_group(&db, "no-direct", &[a.id, b.id]).await;
    let g = with_mapping(&db, &g, "src", b.id, "claude-opus-4").await;

    let set = select_candidates_ctx(&db, &g, "src", None, RequestKind::Chat)
        .await
        .expect("default chain with mapping");
    assert_eq!(
        set.candidates[0].platform.id, b.id,
        "mapping target platform still promoted first"
    );
}

/// 直通链上映射只翻译不锁平台：映射 target_platform_id 不提首，按现有排序；
/// target_model = 映射后的原名（不经槽位）。
#[tokio::test]
async fn direct_model_mapping_translates_without_locking() {
    let (db, _dir) = mk_three_file_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    set_extra(&db, a.id, r#"{"direct_models":["mapped-t"]}"#).await;
    set_extra(&db, b.id, r#"{"direct_models":["mapped-t"]}"#).await;
    let g = mk_db_group(&db, "direct-map", &[a.id, b.id]).await;
    // 默认链会把 target_platform_id=b 提首；直通链必须保持 priority 顺序 a 在前
    let g = with_mapping(&db, &g, "src", b.id, "mapped-t").await;

    let set = select_candidates_ctx(&db, &g, "src", None, RequestKind::Chat)
        .await
        .expect("direct chain via mapped name");
    assert_eq!(set.candidates[0].platform.id, a.id, "no mapping promotion");
    assert_eq!(set.candidates[0].target_model, "mapped-t");
}

// ══ embedding：请求类型维度（槽位 + OpenAI 系 endpoint）══

fn slot_embedding(v: &str) -> PlatformModels {
    PlatformModels {
        embedding: vec![v.into()],
        ..Default::default()
    }
}

fn upd_platform(id: u64) -> UpdatePlatform {
    UpdatePlatform {
        id, name: None, platform_type: None, base_url: None, api_key: None, extra: None,
        models: None, available_models: None, endpoints: None, enabled: None, status: None,
        manual_budgets: None, join_group_ids: None, expires_at: None, quota_source: None,
    }
}

async fn mk_embedding_platform(db: &db::Db, name: &str, proto: Protocol, models: PlatformModels) -> Platform {
    db::create_platform(
        db,
        CreatePlatform {
            name: name.into(),
            platform_type: proto,
            base_url: "https://example.invalid/v1".into(),
            api_key: "k".into(),
            extra: String::new(),
            models: Some(models),
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .expect("create platform")
}

/// 槽位空 / 非 OpenAI 系 → 非候选；槽位值即上游模型（忽略请求名）；纯向量化平台被 Chat 剔除。
#[tokio::test]
async fn embedding_request_filters_and_rewrites() {
    let db = mk_test_db().await;
    let ok = mk_embedding_platform(&db, "ok", Protocol::OpenAI, slot_embedding("emb-x")).await;
    let empty = mk_embedding_platform(&db, "empty", Protocol::OpenAI, slot_default("gpt")).await;
    let anth = mk_embedding_platform(&db, "anth", Protocol::Anthropic, slot_embedding("emb-y")).await;
    let g = mk_db_group(&db, "grp", &[ok.id, empty.id, anth.id]).await;

    let set = select_candidates_ctx(&db, &g, "client-model", None, RequestKind::Embedding)
        .await
        .expect("one embedding platform");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, ok.id);
    assert_eq!(set.candidates[0].target_model, "emb-x");

    // 纯向量化平台被 Chat 剔除，混合平台保留
    let set = select_candidates_ctx(&db, &g, "gpt", None, RequestKind::Chat)
        .await
        .expect("chat platforms remain");
    let ids: Vec<u64> = set.candidates.iter().map(|c| c.platform.id).collect();
    assert!(!ids.contains(&ok.id) && ids.contains(&empty.id));
}

/// 单平台组 与 多平台组退化为 1 个可用平台：槽位为空都报 no_embedding_platform。
#[tokio::test]
async fn embedding_empty_slot_errs_single_and_degraded_group() {
    let db = mk_test_db().await;
    let empty = mk_embedding_platform(&db, "empty", Protocol::OpenAI, slot_default("gpt")).await;
    let emb = mk_embedding_platform(&db, "emb", Protocol::OpenAI, slot_embedding("emb-x")).await;
    // 退化：emb 手动禁用 → 组内只剩 empty 一个 Enabled
    db::update_platform(
        &db,
        UpdatePlatform {
            status: Some(PlatformStatus::Disabled),
            ..upd_platform(emb.id)
        },
    )
    .await
    .expect("disable");
    let single = mk_db_group(&db, "single", &[empty.id]).await;
    let degraded = mk_db_group(&db, "degraded", &[empty.id, emb.id]).await;
    for g in [single, degraded] {
        let e = select_candidates_ctx(&db, &g, "m", None, RequestKind::Embedding)
            .await
            .expect_err("no embedding platform");
        assert_eq!(e, "no_embedding_platform", "group {}", g.name);
    }
}

/// time_windows 规则不含 embedding 槽位也不影响读数（读 platform.models 而非 effective_models）；
/// direct_models 声明与客户端模型名同名也不改变 Embedding 路由。
#[tokio::test]
async fn embedding_ignores_time_windows_and_direct_models() {
    let db = mk_test_db().await;
    let p = mk_embedding_platform(&db, "p", Protocol::OpenAI, slot_embedding("emb-x")).await;
    let q = mk_embedding_platform(&db, "q", Protocol::OpenAI, slot_embedding("emb-q")).await;
    db::update_platform(
        &db,
        UpdatePlatform {
            extra: Some(
                serde_json::json!({
                    "time_windows": [{"start_hour":0,"end_hour":24,"models":{"default":"other"}}],
                    "direct_models": ["client-model"]
                })
                .to_string(),
            ),
            ..upd_platform(p.id)
        },
    )
    .await
    .expect("set extra");
    let g = mk_db_group(&db, "grp", &[p.id, q.id]).await;
    let set = select_candidates_ctx(&db, &g, "client-model", None, RequestKind::Embedding)
        .await
        .expect("embedding routes normally");
    assert_eq!(set.candidates.len(), 2, "direct_models 不把候选收窄到声明平台");
    let p_route = set.candidates.iter().find(|c| c.platform.id == p.id).unwrap();
    assert_eq!(p_route.target_model, "emb-x");
}

// ══ rerank：与 embedding 同构的请求类型维度 ══

fn slot_rerank(v: &str) -> PlatformModels {
    PlatformModels { rerank: vec![v.into()], ..Default::default() }
}

/// 槽位空 / 非 OpenAI 系 / 只配 embedding 槽 → 非候选；槽位值即上游模型；纯重排序平台被 Chat 剔除。
#[tokio::test]
async fn rerank_request_filters_and_rewrites() {
    let db = mk_test_db().await;
    let ok = mk_embedding_platform(&db, "ok", Protocol::OpenAI, slot_rerank("rr-x")).await;
    let empty = mk_embedding_platform(&db, "empty", Protocol::OpenAI, slot_default("gpt")).await;
    let emb = mk_embedding_platform(&db, "emb", Protocol::OpenAI, slot_embedding("emb-y")).await;
    let anth = mk_embedding_platform(&db, "anth", Protocol::Anthropic, slot_rerank("rr-y")).await;
    let g = mk_db_group(&db, "grp", &[ok.id, empty.id, emb.id, anth.id]).await;

    let set = select_candidates_ctx(&db, &g, "client-model", None, RequestKind::Rerank)
        .await
        .expect("one rerank platform");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, ok.id);
    assert_eq!(set.candidates[0].target_model, "rr-x");

    let set = select_candidates_ctx(&db, &g, "gpt", None, RequestKind::Chat)
        .await
        .expect("chat platforms remain");
    let ids: Vec<u64> = set.candidates.iter().map(|c| c.platform.id).collect();
    assert!(!ids.contains(&ok.id) && ids.contains(&empty.id));
}

/// 单平台组 与 多平台组退化为 1 个可用平台：槽位为空都报 no_rerank_platform。
#[tokio::test]
async fn rerank_empty_slot_errs_single_and_degraded_group() {
    let db = mk_test_db().await;
    let empty = mk_embedding_platform(&db, "empty", Protocol::OpenAI, slot_default("gpt")).await;
    let rr = mk_embedding_platform(&db, "rr", Protocol::OpenAI, slot_rerank("rr-x")).await;
    db::update_platform(
        &db,
        UpdatePlatform { status: Some(PlatformStatus::Disabled), ..upd_platform(rr.id) },
    )
    .await
    .expect("disable");
    let single = mk_db_group(&db, "single", &[empty.id]).await;
    let degraded = mk_db_group(&db, "degraded", &[empty.id, rr.id]).await;
    for g in [single, degraded] {
        let e = select_candidates_ctx(&db, &g, "m", None, RequestKind::Rerank)
            .await
            .expect_err("no rerank platform");
        assert_eq!(e, "no_rerank_platform", "group {}", g.name);
    }
}

/// time_windows 不含 rerank 槽位不影响读数；direct_models 同名不收窄候选。
#[tokio::test]
async fn rerank_ignores_time_windows_and_direct_models() {
    let db = mk_test_db().await;
    let p = mk_embedding_platform(&db, "p", Protocol::OpenAI, slot_rerank("rr-x")).await;
    let q = mk_embedding_platform(&db, "q", Protocol::OpenAI, slot_rerank("rr-q")).await;
    db::update_platform(
        &db,
        UpdatePlatform {
            extra: Some(
                serde_json::json!({
                    "time_windows": [{"start_hour":0,"end_hour":24,"models":{"default":"other"}}],
                    "direct_models": ["client-model"]
                })
                .to_string(),
            ),
            ..upd_platform(p.id)
        },
    )
    .await
    .expect("set extra");
    let g = mk_db_group(&db, "grp", &[p.id, q.id]).await;
    let set = select_candidates_ctx(&db, &g, "client-model", None, RequestKind::Rerank)
        .await
        .expect("rerank routes normally");
    assert_eq!(set.candidates.len(), 2);
    let p_route = set.candidates.iter().find(|c| c.platform.id == p.id).unwrap();
    assert_eq!(p_route.target_model, "rr-x");
}

// ── 路由快照「模型选择来源」model_via ──

#[test]
fn model_via_peak_branch_then_slot() {
    let pm = PlatformModels {
        default: vec!["glm-5.2".into()],
        sonnet: vec!["glm-4.7".into()],
        ..Default::default()
    };
    let p = glm_coding_platform("", pm);
    // 高峰窗口：preset peak 分支替换 → [PresetPeak, SlotSonnet]
    let hit = model_via_for(&p, None, RequestKind::Chat, PEAK_MS, "", &Some("claude-sonnet-4".into()));
    assert_eq!(hit, vec![ModelVia::Mapped, ModelVia::PresetPeak, ModelVia::SlotSonnet]);
    // 非高峰 + 映射改名 → [Mapped, SlotSonnet]
    let off = model_via_for(&p, None, RequestKind::Chat, OFF_MS, "x", &Some("claude-sonnet-4".into()));
    assert_eq!(off, vec![ModelVia::Mapped, ModelVia::SlotSonnet]);
    // 无档位词 → default 槽
    let def = model_via_for(&p, None, RequestKind::Chat, OFF_MS, "foo", &None);
    assert_eq!(def, vec![ModelVia::SlotDefault]);
}

#[test]
fn model_via_no_slot_is_passthrough_and_tools_passthrough() {
    let p = glm_coding_platform("", PlatformModels::default());
    assert_eq!(
        model_via_for(&p, None, RequestKind::Chat, OFF_MS, "foo", &None),
        vec![ModelVia::Passthrough]
    );
    assert_eq!(
        model_via_for(&p, None, RequestKind::Tools("t"), OFF_MS, "foo", &None),
        vec![ModelVia::Passthrough]
    );
}

// ── 票 04：客户端白名单（多平台组普通过滤路径）──

const WL_CC: &str = r#"{"allowed_clients":["claude_code"]}"#;
const PEAK_OFF: &str =
    r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#;
const PEAK_OFF_WL: &str = r#"{"allowed_clients":["claude_code"],"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#;

#[derive(Clone, Copy, Debug)]
enum Plat {
    /// 白名单 {claude_code}
    Wl,
    /// 无白名单
    Open,
    /// 高峰禁用（恒命中）
    Peak,
    /// 高峰禁用 + 白名单（状态维度先排除，归 peak）
    PeakWl,
    /// 协议未知
    Unknown,
}

/// 表驱动：白名单与其他排除原因并存时的错误码（curl UA 不在 claude_code 白名单内）。
#[tokio::test]
async fn client_whitelist_error_code_matrix() {
    use Plat::*;
    // (组内平台, 期望 Err reason；None = 选得出候选)
    let cases: &[(&[Plat], Option<&str>)] = &[
        (&[Wl, Wl], Some("client_not_allowed")),
        (&[Wl, Peak], Some("client_not_allowed_mixed")),
        (&[Wl, PeakWl], Some("client_not_allowed_mixed")),
        (&[Wl, Unknown], Some("client_not_allowed_mixed")),
        // 既有优先级不变：全高峰禁用（含配了白名单的）仍报 peak_disabled
        (&[Peak, PeakWl], Some("peak_disabled")),
        (&[PeakWl, PeakWl], Some("peak_disabled")),
        // 全未知协议仍报 unknown_protocol
        (&[Unknown, Unknown], Some("unknown_protocol")),
        // 有未配白名单的存活平台 → 落到它
        (&[Wl, Open], None),
    ];
    for (i, (plats, want)) in cases.iter().enumerate() {
        let db = mk_test_db().await;
        let mut ids = Vec::new();
        for (j, p) in plats.iter().enumerate() {
            let pl = mk_db_platform(&db, &format!("P{i}-{j}")).await;
            match p {
                Wl => set_extra(&db, pl.id, WL_CC).await,
                Open => {}
                Peak => set_extra(&db, pl.id, PEAK_OFF).await,
                PeakWl => set_extra(&db, pl.id, PEAK_OFF_WL).await,
                Unknown => {
                    let pid = pl.id as i64;
                    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
                        conn.execute(
                            "UPDATE platform SET platform_type='no_such_protocol' WHERE id=?1",
                            rusqlite::params![pid],
                        )?;
                        Ok(())
                    })
                    .await
                    .unwrap();
                }
            }
            ids.push(pl.id);
        }
        db.invalidate_group_details_cache();
        let g = mk_db_group(&db, &format!("g-wl-{i}"), &ids).await;
        let gate = ClientGate::On(ClientProbe::from_ua("curl/8.4.0"));
        let got =
            select_candidates_gated(&db, &g, "claude-opus-4-8", None, RequestKind::Chat, gate).await;
        match (want, got) {
            (Some(w), Err(e)) => assert_eq!(e.reason, *w, "case {i} {plats:?}"),
            (None, Ok(set)) => assert_eq!(set.candidates.len(), 1, "case {i} {plats:?}"),
            (w, g) => panic!("case {i} {plats:?}: want {w:?}, got {:?}", g.map(|s| s.candidates.len())),
        }
    }
}

/// 白名单命中 / 大类展开 / 认不出 / Off 不判定，及快照标签。
#[tokio::test]
async fn client_whitelist_hit_matrix_and_snapshot_label() {
    let db = mk_test_db().await;
    let wl = mk_db_platform(&db, "WL").await;
    let open = mk_db_platform(&db, "OPEN").await;
    set_extra(&db, wl.id, r#"{"allowed_clients":["claude_code_vscode"]}"#).await;
    let g = mk_db_group(&db, "g-hit", &[wl.id, open.id]).await;
    let sel = |gate| {
        let (db, g) = (&db, &g);
        async move {
            select_candidates_gated(db, g, "claude-opus-4-8", None, RequestKind::Chat, gate)
                .await
                .unwrap()
        }
    };
    let ua = |s: &str| ClientGate::On(ClientProbe::from_ua(s));
    // 变体精确命中 → 两个平台都在
    assert_eq!(sel(ua("claude-cli/2.0 (external, claude-vscode)")).await.candidates.len(), 2);
    // 只勾变体，识别为 CLI → WL 被拦、快照带标签
    let set = sel(ua("claude-cli/2.0 (external, cli)")).await;
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, open.id);
    let e = set.routing.entries.iter().find(|e| e.platform_id == wl.id).unwrap();
    assert_eq!(e.reason, RouteReason::ClientNotAllowed);
    // 认不出（无 UA）→ WL 被拦
    assert_eq!(sel(ClientGate::On(ClientProbe::from_ua(""))).await.candidates.len(), 1);
    // Off：不判定，两个都在
    assert_eq!(sel(ClientGate::Off).await.candidates.len(), 2);
    // 勾大类 = 全部变体
    set_extra(&db, wl.id, WL_CC).await;
    db.invalidate_group_details_cache();
    assert_eq!(sel(ua("claude-cli/2.0 (external, claude-vscode)")).await.candidates.len(), 2);
}

/// 熔断全空回退：被白名单拦下的平台不会被熔断回退「捞回来」。
#[tokio::test]
async fn client_whitelist_not_revived_by_breaker_bypass() {
    let db = mk_test_db().await;
    let wl = mk_db_platform(&db, "WL").await;
    let broken = mk_db_platform(&db, "BROKEN").await;
    set_extra(&db, wl.id, WL_CC).await;
    let g = mk_db_group(&db, "g-bb", &[wl.id, broken.id]).await;
    let sched = SchedulerState::new();
    let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
    sched.inc_inflight(broken.id);
    sched.record_failure(broken.id, &th, db::now());
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings { enabled: true, ..Default::default() };
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None, rate_body: None };
    let gate = ClientGate::On(ClientProbe::from_ua("curl/8.4.0"));
    let set = select_candidates_gated(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat, gate)
        .await
        .expect("breaker bypass revives only the non-whitelisted platform");
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].platform.id, broken.id);
}

// ── 票 04：模型链（备选）──

fn chain(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn chat_chain(p: &Platform, eff: &PlatformModels, name: &str) -> Vec<String> {
    resolve_target_chain(RequestKind::Chat, eff, p, &DecisionModels::default(), name)
}

/// 槽位选择规则不变，命中槽位返回完整有序链（去空去重保序）；请求名透传 / 无槽位 → 单元素链。
#[test]
fn chain_slot_selection_and_passthrough() {
    let pm = PlatformModels {
        default: chain(&["d1", "d2"]),
        opus: chain(&["o1", " ", "o2", "o1"]),
        ..Default::default()
    };
    let p = glm_coding_platform("", pm.clone());
    assert_eq!(chat_chain(&p, &pm, "claude-opus-4"), chain(&["o1", "o2"]));
    assert_eq!(chat_chain(&p, &pm, "foo"), chain(&["d1", "d2"]));
    let empty = PlatformModels::default();
    assert_eq!(chat_chain(&p, &empty, "foo[1m]"), chain(&["foo"]));
}

/// 三层级联各层多模型：time_windows / preset peak / platform.models 的链顺序都原样输出。
#[test]
fn chain_three_layers_keep_order() {
    // 层 3：platform.models
    let pm = PlatformModels { default: chain(&["a", "b", "c"]), ..Default::default() };
    let p = glm_coding_platform("", pm);
    let eff = resolve_effective_models(&p, &[], OFF_MS, "");
    assert_eq!(chat_chain(&p, &eff, "foo"), chain(&["a", "b", "c"]));
    // 层 1：time_windows 数组形态
    let extra = serde_json::json!({"time_windows": [{
        "windows": [{"start_hour": 0, "end_hour": 24}],
        "models": {"default": ["t1", "t2"]}
    }]})
    .to_string();
    let p = glm_coding_platform(&extra, PlatformModels { default: chain(&["a"]), ..Default::default() });
    let rules = crate::gateway::time_windows::parse_platform_time_windows(&p.extra);
    let eff = resolve_effective_models(&p, &rules, PEAK_MS, "");
    assert_eq!(chat_chain(&p, &eff, "foo"), chain(&["t1", "t2"]));
    // 层 2：preset peak 分支（registry 里是单值，断言链 == preset peak default 槽的归一化链）
    let (eff, via) = resolve_effective_models_tagged(&p, &[], PEAK_MS, "");
    assert_eq!(via, Some(ModelVia::PresetPeak));
    let expect = slot_chain(&crate::gateway::peak::default_peak_models("glm_coding").unwrap().default);
    assert_eq!(chat_chain(&p, &eff, "foo"), expect);
}

/// 完整链路：RouteResult 链首 + fallback_models、快照 model_chain；链长 1 快照不带 model_chain。
#[tokio::test]
async fn route_result_carries_fallbacks_and_snapshot_chain() {
    let db = mk_test_db().await;
    let multi = mk_platform_models(
        &db,
        "multi",
        PlatformModels { default: chain(&["m1", "m2", "m3"]), ..Default::default() },
        vec![],
    )
    .await;
    let single = mk_platform_models(
        &db,
        "single",
        PlatformModels { default: chain(&["s1"]), ..Default::default() },
        vec![],
    )
    .await;
    let g = mk_db_group(&db, "grp", &[multi.id, single.id]).await;
    let set = select_candidates_ctx(&db, &g, "foo", None, RequestKind::Chat).await.expect("route");
    let m = set.candidates.iter().find(|c| c.platform.id == multi.id).unwrap();
    assert_eq!(m.target_model, "m1");
    assert_eq!(m.fallback_models, chain(&["m2", "m3"]));
    let s = set.candidates.iter().find(|c| c.platform.id == single.id).unwrap();
    assert_eq!(s.target_model, "s1");
    assert!(s.fallback_models.is_empty());
    let me = set.routing.entries.iter().find(|e| e.platform_id == multi.id).unwrap();
    assert_eq!(me.target_model.as_deref(), Some("m1"));
    assert_eq!(me.model_chain, chain(&["m1", "m2", "m3"]));
    let se = set.routing.entries.iter().find(|e| e.platform_id == single.id).unwrap();
    assert!(se.model_chain.is_empty());
}

/// 单平台组（bypass 路径）同样携带备选。
#[tokio::test]
async fn single_platform_group_carries_fallbacks() {
    let db = mk_test_db().await;
    let p = mk_platform_models(
        &db,
        "p",
        PlatformModels { default: chain(&["m1", "m2"]), ..Default::default() },
        vec![],
    )
    .await;
    let g = mk_db_group(&db, "solo", &[p.id]).await;
    let set = select_candidates_ctx(&db, &g, "foo", None, RequestKind::Chat).await.expect("route");
    assert_eq!(set.candidates[0].fallback_models, chain(&["m2"]));
    assert_eq!(set.routing.entries[0].model_chain, chain(&["m1", "m2"]));
}

/// 直通模型链：备选恒空（即使平台槽位有多模型）。
#[tokio::test]
async fn direct_model_chain_has_no_fallbacks() {
    let db = mk_test_db().await;
    let p = mk_platform_models(
        &db,
        "p",
        PlatformModels { default: chain(&["m1", "m2"]), ..Default::default() },
        vec![],
    )
    .await;
    let q = mk_platform_models(&db, "q", PlatformModels::default(), vec![]).await;
    db::update_platform(
        &db,
        UpdatePlatform {
            extra: Some(serde_json::json!({"direct_models": ["client-model"]}).to_string()),
            ..upd_platform(p.id)
        },
    )
    .await
    .expect("set extra");
    let g = mk_db_group(&db, "grp", &[p.id, q.id]).await;
    let set = select_candidates_ctx(&db, &g, "client-model", None, RequestKind::Chat)
        .await
        .expect("direct route");
    assert_eq!(set.candidates[0].target_model, "client-model");
    assert!(set.candidates[0].fallback_models.is_empty());
    assert!(set.routing.entries.iter().all(|e| e.model_chain.is_empty()));
}

/// 透传平台：备选恒空。
#[test]
fn passthrough_platform_chain_is_single() {
    let mut p = super::super::test_mod::mk_platform(PlatformStatus::Enabled, 0);
    p.platform_type = Protocol::Passthrough;
    let pm = PlatformModels { default: chain(&["x", "y"]), ..Default::default() };
    assert_eq!(chat_chain(&p, &pm, "foo"), chain(&["foo"]));
}

/// 专用槽（embedding / jev）同样出链；rerank 槽空 → 单值。
#[test]
fn dedicated_slot_chains() {
    let pm = PlatformModels {
        embedding: chain(&["e1", "e2"]),
        jev: chain(&["j1", "j2"]),
        ..Default::default()
    };
    let p = glm_coding_platform("", pm.clone());
    let dm = DecisionModels::default();
    assert_eq!(resolve_target_chain(RequestKind::Embedding, &pm, &p, &dm, "x"), chain(&["e1", "e2"]));
    assert_eq!(resolve_target_chain(RequestKind::Decision, &pm, &p, &dm, "x"), chain(&["j1", "j2"]));
    assert_eq!(resolve_target_chain(RequestKind::Rerank, &pm, &p, &dm, "x"), chain(&["x"]));
}

/// `/models` 与 pi 清单展平含备选且跨平台去重。
#[test]
fn model_list_flattens_fallbacks_deduped() {
    let a = PlatformModels { default: chain(&["a", "b"]), opus: chain(&["b", "c"]), ..Default::default() };
    let b = PlatformModels { sonnet: chain(&["c", "d"]), ..Default::default() };
    assert_eq!(
        crate::gateway::proxy::merge_group_model_names(&[], &[a, b]),
        chain(&["a", "b", "c", "d"])
    );
}

// ── 路由画布 01：路由阶段补记快照 ──

fn open_breaker(sched: &SchedulerState, pid: u64, now: i64) {
    let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
    sched.inc_inflight(pid);
    sched.record_failure(pid, &th, now);
}

/// 字面单平台组：短路标记 literal=true，列出被绕过的熔断 / auto_disabled。
#[tokio::test]
async fn rc01_literal_single_shortcut_lists_bypassed_states() {
    let db = mk_test_db().await;
    let p = mk_db_platform(&db, "GLM").await;
    let g = mk_db_group(&db, "single", &[p.id]).await;
    db::test_support::set_legacy_auto_disabled(&db, p.id, 1, db::now() + 3_600_000).await;
    let sched = SchedulerState::new();
    open_breaker(&sched, p.id, db::now());
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings { enabled: true, ..Default::default() };
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None, rate_body: None };

    let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx), RequestKind::Chat)
        .await
        .expect("force request");
    let sc = set.routing.shortcut.expect("shortcut recorded");
    assert!(sc.literal);
    assert!(sc.bypassed.contains(&aidog_db::models::ShortcutBypass::AutoDisabled));
    assert!(sc.bypassed.contains(&aidog_db::models::ShortcutBypass::Breaker));
}

/// 等效单平台（多平台组仅 1 个 Enabled）：literal=false；多平台正常路径无 shortcut。
#[tokio::test]
async fn rc01_equivalent_single_shortcut_and_multi_none() {
    let db = mk_test_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    let c = mk_db_platform(&db, "C").await;
    let g_multi = mk_db_group(&db, "multi", &[a.id, b.id]).await;
    let set = select_candidates_ctx(&db, &g_multi, "m", None, RequestKind::Chat).await.unwrap();
    assert!(set.routing.shortcut.is_none());

    db::test_support::set_legacy_auto_disabled(&db, b.id, 1, db::now() + 3_600_000).await;
    let g_eq = mk_db_group(&db, "equiv", &[a.id, b.id, c.id]).await;
    db::test_support::set_legacy_auto_disabled(&db, c.id, 1, db::now() + 3_600_000).await;
    let set = select_candidates_ctx(&db, &g_eq, "m", None, RequestKind::Chat).await.unwrap();
    let sc = set.routing.shortcut.expect("equivalent shortcut");
    assert!(!sc.literal);
    assert!(sc.bypassed.is_empty(), "sole platform itself is healthy");
}

/// auto_disabled 退避期满的探测放行带 Probe 标记。
#[tokio::test]
async fn rc01_probe_candidate_tagged() {
    let db = mk_test_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    let c = mk_db_platform(&db, "C").await;
    db::test_support::set_legacy_auto_disabled(&db, c.id, 1, db::now() - 1000).await;
    let g = mk_db_group(&db, "probe", &[a.id, b.id, c.id]).await;
    let set = select_candidates_ctx(&db, &g, "m", None, RequestKind::Chat).await.unwrap();
    let tag_of = |id: u64| set.routing.entries.iter().find(|e| e.platform_id == id).unwrap().tags.clone();
    assert!(tag_of(c.id).contains(&RouteTag::Probe));
    assert!(!tag_of(a.id).contains(&RouteTag::Probe));
}

/// 熔断全空回填：回填候选带 BreakerBackfill，reason 仍为 selected。
#[tokio::test]
async fn rc01_breaker_backfill_tagged() {
    let db = mk_test_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    let g = mk_db_group(&db, "bk", &[a.id, b.id]).await;
    let sched = SchedulerState::new();
    let now = db::now();
    open_breaker(&sched, a.id, now);
    open_breaker(&sched, b.id, now);
    let sticky = StickyTable::new();
    let settings = SchedulingBreakerSettings { enabled: true, ..Default::default() };
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None, rate_body: None };
    let set = select_candidates_ctx(&db, &g, "m", Some(&ctx), RequestKind::Chat).await.unwrap();
    assert_eq!(set.routing.entries.len(), 2);
    for e in &set.routing.entries {
        assert_eq!(e.reason, RouteReason::Selected);
        assert!(e.tags.contains(&RouteTag::BreakerBackfill), "{:?}", e.tags);
    }
}

/// 直通模式：未声明该模型的平台以 not-declared 原因出现。
#[tokio::test]
async fn rc01_direct_model_undeclared_platform_listed() {
    let (db, _dir) = mk_three_file_db().await;
    let a = mk_db_platform(&db, "A").await;
    let b = mk_db_platform(&db, "B").await;
    set_extra(&db, a.id, r#"{"direct_models":["m1"]}"#).await;
    let g = mk_db_group(&db, "direct-nd", &[a.id, b.id]).await;
    let set = select_candidates_ctx(&db, &g, "m1", None, RequestKind::Chat).await.unwrap();
    let eb = set.routing.entries.iter().find(|e| e.platform_id == b.id).expect("B listed");
    assert_eq!(eb.reason, RouteReason::NotDeclared);
    let ea = set.routing.entries.iter().find(|e| e.platform_id == a.id).unwrap();
    assert_eq!(ea.reason, RouteReason::Selected);
}

/// max_retries 截断：order > max_retries+1 的入选候选标 retry-limit，max_retries 可读。
#[test]
fn rc01_mark_retry_limit_truncates_tail() {
    let mk = |order: u32| aidog_db::models::RouteCandidate {
        order,
        platform: format!("p{order}"),
        platform_id: order as u64,
        level_priority: 5,
        priority: 0,
        reason: RouteReason::Selected,
        target_model: None,
        tags: vec![],
        model_via: vec![],
        model_chain: vec![],
    };
    let mut r = RoutingSnapshot { entries: vec![mk(1), mk(2), mk(3), mk(4)], ..Default::default() };
    r.mark_retry_limit(2);
    assert_eq!(r.max_retries, Some(2));
    let reasons: Vec<_> = r.entries.iter().map(|e| e.reason).collect();
    assert_eq!(
        reasons,
        vec![RouteReason::Selected, RouteReason::Selected, RouteReason::Selected, RouteReason::RetryLimit]
    );
}
