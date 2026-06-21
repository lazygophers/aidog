use super::super::candidates::ScheduleCtx;
use super::super::super::scheduling::{SchedulerState, StickyTable};
use super::super::test_mod::{mk_gp, mk_gp_lp};
use super::*;

fn mk_settings() -> SchedulingBreakerSettings {
    SchedulingBreakerSettings::default()
}

#[test]
fn least_latency_orders_by_ema_ascending() {
    let sched = SchedulerState::new();
    // p1 EMA=300, p2 EMA=100, p3 无样本(MAX)
    sched.inc_inflight(1); sched.record_success(1, 300);
    sched.inc_inflight(2); sched.record_success(2, 100);
    let sticky = StickyTable::new();
    let settings = mk_settings();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    let gp3 = mk_gp(3, 1, 0);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_least_latency(&mut v, Some(&ctx));
    // 升序: p2(100) < p1(300) < p3(MAX)
    assert_eq!(v[0].platform.id, 2);
    assert_eq!(v[1].platform.id, 1);
    assert_eq!(v[2].platform.id, 3);
}

#[test]
fn sticky_binds_then_falls_back() {
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = mk_settings();
    let now = super::super::super::db::now();
    let ctx = ScheduleCtx {
        scheduler: &sched, sticky: &sticky, settings: &settings,
        sticky_key: Some("grpA|client1".to_string()),
    };
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);

    // 首次：无绑定 → 写绑定为首选 p1
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2];
    apply_sticky(&mut v, Some(&ctx), now);
    assert_eq!(sticky.get("grpA|client1", now), Some(1));

    // 再次：绑定 p1 健康（在集中），无论入参顺序如何，p1 提首位
    let mut v2: Vec<&GroupPlatformDetail> = vec![&gp2, &gp1];
    apply_sticky(&mut v2, Some(&ctx), now);
    assert_eq!(v2[0].platform.id, 1);

    // 绑定平台 p1 不在候选集（熔断/失效）→ 回退首选并重绑 p2
    let gp3 = mk_gp(3, 1, 0);
    let mut v3: Vec<&GroupPlatformDetail> = vec![&gp2, &gp3];
    apply_sticky(&mut v3, Some(&ctx), now);
    assert_eq!(sticky.get("grpA|client1", now), Some(2)); // 重绑为新首选
}

#[test]
fn least_latency_level_priority_tiebreak() {
    // 同延迟档时 level_priority 高者先；延迟主导不被 level_priority 推翻。
    let sched = SchedulerState::new();
    // p1,p2 同延迟 EMA=100；p3 延迟 200（更慢）
    sched.inc_inflight(1); sched.record_success(1, 100);
    sched.inc_inflight(2); sched.record_success(2, 100);
    sched.inc_inflight(3); sched.record_success(3, 200);
    let sticky = StickyTable::new();
    let settings = mk_settings();
    let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

    // p1 lp=5, p2 lp=10（同延迟，p2 应先）; p3 lp=10 但延迟更高（仍排末尾，延迟主导）
    let gp1 = mk_gp_lp(1, 1, 0, 5);
    let gp2 = mk_gp_lp(2, 1, 0, 10);
    let gp3 = mk_gp_lp(3, 1, 0, 10);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_least_latency(&mut v, Some(&ctx));
    // 同延迟 100 档：p2(lp10) 先于 p1(lp5)；p3(延迟200) 末尾（延迟主导，不被高 lp 提前）
    assert_eq!(v.iter().map(|g| g.platform.id).collect::<Vec<_>>(), vec![2, 1, 3]);
}
