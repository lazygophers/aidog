use super::super::scheduling::{Admission, BreakerThresholds, SchedulerState};
use super::*;

pub(super) fn mk_platform(status: PlatformStatus, until: i64) -> Platform {
    Platform {
        id: 1,
        name: "p".into(),
        platform_type: Protocol::Anthropic,
        base_url: String::new(),
        api_key: String::new(),
        extra: String::new(),
        models: PlatformModels::default(),
        available_models: vec![],
        endpoints: vec![],
        enabled: status == PlatformStatus::Enabled,
        status,
        auto_disabled_until: until,
        auto_disable_strikes: 0,
        expires_at: 0,
        created_at: 0,
        updated_at: 0,
        deleted_at: 0,
        est_balance_remaining: 0.0,
        est_coding_plan: String::new(),
        last_real_query_at: 0,
        estimate_count: 0,
        show_in_tray: false,
        tray_display: String::new(),
        sort_order: 0,
        manual_budgets: vec![],
        balance_level: String::new(),
        last_error: String::new(),
        last_error_at: 0,
    }
}

pub(super) fn mk_platform_id(id: u64) -> Platform {
    let mut p = mk_platform(PlatformStatus::Enabled, 0);
    p.id = id;
    p
}

pub(super) fn mk_gp(id: u64, weight: i32, priority: i32) -> GroupPlatformDetail {
    GroupPlatformDetail { platform: mk_platform_id(id), priority, weight, level_priority: 5 }
}

pub(super) fn mk_gp_lp(id: u64, weight: i32, priority: i32, level_priority: i32) -> GroupPlatformDetail {
    GroupPlatformDetail { platform: mk_platform_id(id), priority, weight, level_priority }
}

/// 带 expires_at 的候选（用于 [platform-expiry-priority] 排序测试）。
pub(super) fn mk_gp_exp(id: u64, weight: i32, priority: i32, expires_at: i64) -> GroupPlatformDetail {
    let mut p = mk_platform_id(id);
    p.expires_at = expires_at;
    GroupPlatformDetail { platform: p, priority, weight, level_priority: 5 }
}

/// coding plan 候选：platform 带一个 coding_plan=true 的端点（is_coding_plan→true）。
pub(super) fn mk_gp_cp(id: u64, weight: i32, priority: i32) -> GroupPlatformDetail {
    let mut p = mk_platform_id(id);
    p.endpoints = vec![PlatformEndpoint {
        protocol: Protocol::Anthropic,
        base_url: String::new(),
        client_type: Default::default(),
        coding_plan: true,
    }];
    GroupPlatformDetail { platform: p, priority, weight, level_priority: 5 }
}

#[test]
fn breaker_union_autodisabled_admission() {
    // 验证熔断 ∪ auto_disabled 取并集：分别独立判定。
    let sched = SchedulerState::new();
    let now = super::super::db::now();
    let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
    // p1 熔断 Open
    sched.inc_inflight(1);
    sched.record_failure(1, &th, now);
    assert_eq!(sched.admission(1, &th, now, true), Admission::Reject);
    // p2 健康
    assert_eq!(sched.admission(2, &th, now, true), Admission::Allow);
    // auto_disabled 维度独立：candidate_state 判定（不被熔断改写）
    let p_auto = mk_platform(PlatformStatus::AutoDisabled, now + 5000);
    assert_eq!(candidate_state(&p_auto, now, ""), None); // auto_disabled 未到期 → 排除
    // 熔断状态不影响 candidate_state（auto_disabled 维度）
    let p_enabled = mk_platform_id(1);
    assert_eq!(candidate_state(&p_enabled, now, ""), Some(false)); // 仍 enabled（熔断不改 DB status）
}

#[test]
fn breaker_does_not_overwrite_autodisabled() {
    // 熔断与 auto_disabled 状态互不覆盖：record_failure 只动内存 breaker，不动 platform.status。
    let sched = SchedulerState::new();
    let now = super::super::db::now();
    let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
    sched.inc_inflight(1);
    sched.record_failure(1, &th, now);
    // platform.status 仍是 Enabled（熔断不写 DB 三态）
    let p = mk_platform_id(1);
    assert_eq!(p.status, PlatformStatus::Enabled);
    // 内存 breaker 是 Open
    assert!(matches!(sched.breaker_state(1), super::super::scheduling::BreakerState::Open { .. }));
}

#[test]
fn candidate_state_filtering() {
    let now = 1_000_000i64;
    // enabled → 始终纳入（非试探）
    assert_eq!(candidate_state(&mk_platform(PlatformStatus::Enabled, 0), now, ""), Some(false));
    // 用户手动 disabled → 排除
    assert_eq!(candidate_state(&mk_platform(PlatformStatus::Disabled, 0), now, ""), None);
    // auto_disabled 未到退避时间 → 排除
    assert_eq!(candidate_state(&mk_platform(PlatformStatus::AutoDisabled, now + 5000), now, ""), None);
    // auto_disabled 已过退避时间 → 纳入（末尾试探）
    assert_eq!(candidate_state(&mk_platform(PlatformStatus::AutoDisabled, now - 1), now, ""), Some(true));
}

#[test]
fn candidate_state_expires_at_excludes() {
    // 过期是独立维度，与 status 正交：expires_at > 0 且 now >= expires_at → None（排除）。
    let now = 1_000_000i64;
    // 辅助：构造带 expires_at 的 Platform（mk_platform 不暴露 expires_at，直接改字段）。
    let mut p_expired = mk_platform(PlatformStatus::Enabled, 0);
    p_expired.expires_at = now - 1; // 过去 → 已过期
    assert_eq!(candidate_state(&p_expired, now, ""), None, "enabled + 过期 → 排除");
    let mut p_expired_auto = mk_platform(PlatformStatus::AutoDisabled, now - 1);
    p_expired_auto.expires_at = now - 1;
    assert_eq!(candidate_state(&p_expired_auto, now, ""), None, "auto_disabled(已过退避) + 过期 → 仍排除（过期优先）");

    // 未来过期 → 不影响 status 路径（仍按 status 判定）
    let mut p_future = mk_platform(PlatformStatus::Enabled, 0);
    p_future.expires_at = now + 50_000;
    assert_eq!(candidate_state(&p_future, now, ""), Some(false), "未来过期 + enabled → 仍纳入");

    // expires_at == 0 → 永不过期（不影响）
    let p_no_expiry = mk_platform(PlatformStatus::Enabled, 0);
    assert_eq!(candidate_state(&p_no_expiry, now, ""), Some(false), "expires_at=0 → 永不过期");
}

#[test]
fn candidate_state_peak_disabled_dimension() {
    // 高峰禁用是正交维度（不改 status 三态）：开关 on + 命中 peak window → None 排除。
    // 跨天窗口 22-06 命中：2024-01-01T23:00:00Z（Mon）→ hour=23 落在 [22,6) → is_in_peak_window=true。
    let ms_in_peak = chrono::DateTime::<chrono::Utc>::from_timestamp(1704154800, 0).unwrap().timestamp_millis();
    let ms_off_peak = chrono::DateTime::<chrono::Utc>::from_timestamp(1704116400, 0).unwrap().timestamp_millis(); // hour=15

    // 开关 off：不影响（即便 peak window 命中，仍按 status 判定）
    let mut p_off = mk_platform(PlatformStatus::Enabled, 0);
    p_off.extra = r#"{"peak_hours":[{"start_hour":22,"end_hour":6,"multiplier":1.5}]}"#.to_string();
    assert_eq!(candidate_state(&p_off, ms_in_peak, ""), Some(false), "开关 off + peak window → enabled 仍纳入");

    // 开关 on + 非 peak window：仍纳入（与 status 正交）
    let mut p_on_offpeak = mk_platform(PlatformStatus::Enabled, 0);
    p_on_offpeak.extra = r#"{"disable_during_peak":true,"peak_hours":[{"start_hour":22,"end_hour":6,"multiplier":1.5}]}"#.to_string();
    assert_eq!(candidate_state(&p_on_offpeak, ms_off_peak, ""), Some(false), "开关 on + 非 peak window → 仍纳入");

    // 开关 on + peak window → 排除（None）
    let mut p_on_inpeak = mk_platform(PlatformStatus::Enabled, 0);
    p_on_inpeak.extra = r#"{"disable_during_peak":true,"peak_hours":[{"start_hour":22,"end_hour":6,"multiplier":1.5}]}"#.to_string();
    assert_eq!(candidate_state(&p_on_inpeak, ms_in_peak, ""), None, "开关 on + peak window → 排除（与 status 正交）");

    // 开关 on + peak window + AutoDisabled 已过退避 → 仍排除（高峰禁用优先于 status 试探纳入）
    let mut p_auto = mk_platform(PlatformStatus::AutoDisabled, 0);
    p_auto.extra = r#"{"disable_during_peak":true,"peak_hours":[{"start_hour":22,"end_hour":6,"multiplier":1.5}]}"#.to_string();
    assert_eq!(candidate_state(&p_auto, ms_in_peak, ""), None, "auto_disabled(已过退避) + 高峰禁用 → 仍排除");

    // 开关 on + peak window，但 peak_hours 缺失（无窗口）→ 不命中 → 不排除
    let mut p_no_windows = mk_platform(PlatformStatus::Enabled, 0);
    p_no_windows.extra = r#"{"disable_during_peak":true}"#.to_string();
    assert_eq!(candidate_state(&p_no_windows, ms_in_peak, ""), Some(false), "开关 on + 无 peak_hours 窗口 → 不排除");

    // 非 bool 值不误判（"true" 字符串 / 1 数字 → 视为 off）
    let mut p_str = mk_platform(PlatformStatus::Enabled, 0);
    p_str.extra = r#"{"disable_during_peak":"true","peak_hours":[{"start_hour":22,"end_hour":6,"multiplier":1.5}]}"#.to_string();
    assert_eq!(candidate_state(&p_str, ms_in_peak, ""), Some(false), "非布尔 disable_during_peak → 视为 off");
}

#[test]
fn cap_max_tokens_logic() {
    // 超限 → 裁剪到上限
    assert_eq!(cap_max_tokens(Some(100_000), Some(8192)), (Some(8192), true));
    // 未超限 → 原值不变
    assert_eq!(cap_max_tokens(Some(4096), Some(8192)), (Some(4096), false));
    // 恰好等于上限 → 不裁剪
    assert_eq!(cap_max_tokens(Some(8192), Some(8192)), (Some(8192), false));
    // 客户端未传 → 不注入（None 透传）
    assert_eq!(cap_max_tokens(None, Some(8192)), (None, false));
    // 模型无上限记录 → 不裁剪（即便客户端传了巨大值）
    assert_eq!(cap_max_tokens(Some(1_000_000), None), (Some(1_000_000), false));
    // 模型上限为 0（异常数据）→ 视作无限制不裁剪
    assert_eq!(cap_max_tokens(Some(100_000), Some(0)), (Some(100_000), false));
}

#[test]
fn failover_sorts_by_level_priority_desc_then_priority_asc() {
    // level_priority 降序为主：lp=10 先于 lp=5 先于 lp=1，与 priority(拖拽序)无关。
    // p1: lp=5 pri=0 / p2: lp=10 pri=2 / p3: lp=1 pri=1 / p4: lp=10 pri=0
    let gp1 = mk_gp_lp(1, 1, 0, 5);
    let gp2 = mk_gp_lp(2, 1, 2, 10);
    let gp3 = mk_gp_lp(3, 1, 1, 1);
    let gp4 = mk_gp_lp(4, 1, 0, 10);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3, &gp4];
    // 复用 select_candidates_ctx 内的 Failover 排序逻辑
    v.sort_by_key(|gp| (std::cmp::Reverse(gp.level_priority), gp.priority));
    // lp=10 两个在前，其内部按 priority 升序：p4(pri0) < p2(pri2)；再 lp=5(p1)，再 lp=1(p3)
    assert_eq!(v.iter().map(|g| g.platform.id).collect::<Vec<_>>(), vec![4, 2, 1, 3]);
}

#[test]
fn weighted_effective_weight_is_multiplicative() {
    // 有效权重 = weight × level_priority。
    // 默认全 lp=5：等比放大，相对比例不变（兼容现状）。
    let a = mk_gp_lp(1, 3, 0, 5);
    let b = mk_gp_lp(2, 2, 0, 5);
    assert_eq!(effective_weight(&a), 15);
    assert_eq!(effective_weight(&b), 10);
    // 默认下比例 15:10 == 原 weight 3:2，分流比例不变
    assert_eq!(effective_weight(&a) * 2, effective_weight(&b) * 3);
    // 调高 lp 放大该平台有效权重：weight=1 lp=10 → 10 > weight=2 lp=1 → 2
    let hi = mk_gp_lp(3, 1, 0, 10);
    let lo = mk_gp_lp(4, 2, 0, 1);
    assert_eq!(effective_weight(&hi), 10);
    assert_eq!(effective_weight(&lo), 2);
    assert!(effective_weight(&hi) > effective_weight(&lo));
    // clamp：越界 lp 被夹到 [1,10]
    let over = mk_gp_lp(5, 1, 0, 99);
    let under = mk_gp_lp(6, 1, 0, 0);
    assert_eq!(effective_weight(&over), 10);
    assert_eq!(effective_weight(&under), 1);
}
