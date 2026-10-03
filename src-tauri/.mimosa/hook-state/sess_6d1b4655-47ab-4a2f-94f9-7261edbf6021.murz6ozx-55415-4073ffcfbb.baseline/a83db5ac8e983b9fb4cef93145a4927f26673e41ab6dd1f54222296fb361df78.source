use super::super::super::scheduling::{SchedulerState, StickyTable};
use super::super::candidates::ScheduleCtx;
use super::super::test_mod::{mk_gp, mk_gp_exp};
use super::*;

fn mk_settings() -> SchedulingBreakerSettings {
    SchedulingBreakerSettings::default()
}


#[test]
fn sticky_binds_then_falls_back() {
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = mk_settings();
    let now = aidog_db::now();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
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


// ── order_load_balance ──

#[test]
fn load_balance_single_platform_unchanged() {
    let gp1 = mk_gp(1, 5, 0);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1];
    order_load_balance(&mut v, 12345);
    assert_eq!(v[0].platform.id, 1, "single platform stays");
}

#[test]
fn load_balance_zero_total_weight_keeps_order() {
    // weight=0 → effective_weight is 0 for both
    let gp1 = mk_gp(1, 0, 0);
    let gp2 = mk_gp(2, 0, 0);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2];
    order_load_balance(&mut v, 99);
    // zero total weight → returns early after sort, order may vary but no panic
    assert_eq!(v.len(), 2);
}

#[test]
fn load_balance_two_equal_weight_both_pickable() {
    // mk_gp uses level_priority=5, so effective_weight = weight * clamp(5,1..10) = 1*5 = 5
    // total = 10 per two platforms with weight=1 each
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    // seed 0: rand_val=0%10=0; p1: 0-5=-5<0 → pick=0, no swap → p1 first
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2];
    order_load_balance(&mut v, 0);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].platform.id, 1, "seed=0 should pick p1");
    // seed 5: rand_val=5%10=5; p1: 5-5=0 (not<0); p2: 0-5=-5<0 → pick=1 → swap → p2 first
    let mut v2: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2];
    order_load_balance(&mut v2, 5);
    assert_eq!(v2.len(), 2);
    assert_eq!(v2[0].platform.id, 2, "seed=5 should pick p2");
    assert_ne!(
        v[0].platform.id, v2[0].platform.id,
        "different seeds pick different first platform"
    );
}

#[test]
fn load_balance_higher_weight_preferred() {
    // p1 weight=1, p2 weight=10 → p2 more likely to be first
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 10, 0);
    // total=11, seeds 0..10 → rand_val in [0,10]
    // p1 wins only when rand_val < 1, i.e., rand_val=0
    // seed=0 → 0%11=0 → p1 picked (rand_val=0 → 0-1=-1<0 → pick=0)
    // seed=1 → 1%11=1 → p1 (1-1=0 not <0) → p2 (0-10=-10<0 → pick=1, swap)
    let mut v = vec![&gp1, &gp2];
    // Sort before call: sort by weight desc → [p2(10), p1(1)]
    order_load_balance(&mut v, 1);
    // After sort desc by weight: p2=10 comes first; seed=1 → rand=1 → 1-10=-9<0 at pick=0, no swap → p2 stays first
    assert_eq!(
        v[0].platform.id, 2,
        "higher weight platform p2 should be first for seed=1"
    );
}


#[test]
fn apply_sticky_no_ctx_does_nothing() {
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2];
    apply_sticky(&mut v, None, 0);
    assert_eq!(v[0].platform.id, 1, "no ctx → unchanged");
}

#[test]
fn apply_sticky_empty_candidates_no_panic() {
    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = mk_settings();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: Some("key".to_string()),
    };
    let mut v: Vec<&GroupPlatformDetail> = vec![];
    apply_sticky(&mut v, Some(&ctx), 0); // should not panic
    assert!(v.is_empty());
}

// ── expiry_sort_key / [platform-expiry-priority] ──

#[test]
fn expiry_sort_key_zero_maps_to_max() {
    // expires_at=0（永不过期）→ i64::MAX（排末尾）
    assert_eq!(expiry_sort_key(0), i64::MAX);
    // expires_at>0 → 原值（升序排）
    assert_eq!(expiry_sort_key(1_000_000), 1_000_000);
    assert_eq!(expiry_sort_key(i64::MAX), i64::MAX);
}

#[test]
fn failover_sorts_by_expiry_asc_within_same_priority() {
    // 同 level_priority / priority 的三平台，仅 expires_at 不同：
    // p1: 永不过期(0) / p2: 远未来 / p3: 近未来（快过期）
    // 期望升序：p3（近）→ p2（远）→ p1（永不过期，末尾）
    let gp1 = mk_gp_exp(1, 1, 0, 0);
    let gp2 = mk_gp_exp(2, 1, 0, 10_000_000_000);
    let gp3 = mk_gp_exp(3, 1, 0, 1_000_000_000);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    v.sort_by_key(|gp| {
        (
            std::cmp::Reverse(gp.level_priority),
            gp.priority,
            expiry_sort_key(gp.platform.expires_at),
        )
    });
    assert_eq!(
        v.iter().map(|g| g.platform.id).collect::<Vec<_>>(),
        vec![3, 2, 1],
        "expires_at 升序：快过期先（3），远未来次（2），永不过期末（1）"
    );
}

#[test]
fn failover_priority_dominates_expiry() {
    // priority 主序不变：priority 更优(0) 的平台即便永不过期，仍排在 priority 较差(1) 但快过期平台之前。
    // 即 expires_at 仅在同 priority 内生效（prd 边界决策 3）。
    let gp_p0_noexp = mk_gp_exp(1, 1, 0, 0); // priority 0, 永不过期
    let gp_p1_expiring = mk_gp_exp(2, 1, 1, 1_000); // priority 1, 快过期
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp_p1_expiring, &gp_p0_noexp];
    v.sort_by_key(|gp| {
        (
            std::cmp::Reverse(gp.level_priority),
            gp.priority,
            expiry_sort_key(gp.platform.expires_at),
        )
    });
    assert_eq!(
        v.iter().map(|g| g.platform.id).collect::<Vec<_>>(),
        vec![1, 2],
        "priority 主序：p0 永不过期仍先于 p1 快过期（expires_at 不跨 priority）"
    );
}

#[test]
fn failover_same_expiry_falls_through_to_stable_order() {
    // 同 priority + 同 expires_at → 排序键全平局，Rust sort 稳定 → 保持入参相对序。
    let gp_a = mk_gp_exp(1, 1, 0, 5_000);
    let gp_b = mk_gp_exp(2, 1, 0, 5_000);
    let gp_c = mk_gp_exp(3, 1, 0, 5_000);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp_a, &gp_b, &gp_c];
    v.sort_by_key(|gp| {
        (
            std::cmp::Reverse(gp.level_priority),
            gp.priority,
            expiry_sort_key(gp.platform.expires_at),
        )
    });
    assert_eq!(
        v.iter().map(|g| g.platform.id).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "同 priority + 同 expires_at → 稳定保持入参序（prd 边界决策 4）"
    );
}

#[test]
fn failover_mixed_expiry_zero_at_end_within_priority() {
    // 混合场景：同 priority 内，有期限平台（不论快慢）均排在 expires_at=0 之前。
    let gp_noexp = mk_gp_exp(1, 1, 5, 0); // 永不过期
    let gp_far = mk_gp_exp(2, 1, 5, 99_999_999_999); // 远未来
    let gp_near = mk_gp_exp(3, 1, 5, 1_111); // 近未来
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp_noexp, &gp_far, &gp_near];
    v.sort_by_key(|gp| {
        (
            std::cmp::Reverse(gp.level_priority),
            gp.priority,
            expiry_sort_key(gp.platform.expires_at),
        )
    });
    assert_eq!(
        v.iter().map(|g| g.platform.id).collect::<Vec<_>>(),
        vec![3, 2, 1],
        "有期限 升序（3 近 → 2 远）→ 永不过期末（1）"
    );
}

// ── LoadBalance 同权重档内 expiry tiebreak（基础重试序，不影响加权随机选首） ──

#[test]
fn load_balance_expiry_tiebreak_within_same_weight() {
    // 三平台同 weight=1（同 effective_weight 档）；仅 expires_at 不同。
    // seed=0 → 加权随机选首仍按 weight（落 pick=0），随后整体保持基础排序：
    // 同权重档内 expires_at 升序：近 → 远 → 永不过期。
    // 期望基础重试序 [近, 远, 永不过期]，加权随机不改变（pick=0 无 swap）。
    let gp_noexp = mk_gp_exp(1, 1, 0, 0);
    let gp_far = mk_gp_exp(2, 1, 0, 10_000_000_000);
    let gp_near = mk_gp_exp(3, 1, 0, 1_000_000_000);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp_noexp, &gp_far, &gp_near];
    // seed=0 → rand_val=0 → 首平台(near, weight=1) 0-5=-5<0 → pick=0 无 swap
    order_load_balance(&mut v, 0);
    assert_eq!(
        v.iter().map(|g| g.platform.id).collect::<Vec<_>>(),
        vec![3, 2, 1],
        "同权重档内 expires_at 升序：近(3) → 远(2) → 永不过期(1)；seed=0 加权随机不改变基础序"
    );
}

#[test]
fn load_balance_expiry_does_not_affect_weighted_pick() {
    // 加权随机选首仍只基于 weight：高权重平台 expires_at=0（永不过期），
    // 低权重平台快过期；高权重平台凭 weight 被随机选首，不被 expiry 提前/推后。
    // p1: weight=10, 永不过期；p2: weight=1, 快过期。
    let gp_hi = mk_gp_exp(1, 10, 0, 0);
    let gp_lo = mk_gp_exp(2, 1, 0, 1_000);
    // effective_weight = weight * clamp(level_priority=5,1..10) → p1=50, p2=5, total=55
    // seed=0 → rand_val=0 → 基础排序后首平台(p1 weight 50) 0-50=-50<0 → pick=0
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp_lo, &gp_hi];
    order_load_balance(&mut v, 0);
    // 基础排序按 Reverse(weight)：p1(50) 先于 p2(5)，expiry 仅在同权重档生效（此处权重不同）。
    assert_eq!(
        v.iter().map(|g| g.platform.id).collect::<Vec<_>>(),
        vec![1, 2],
        "权重不同时 expiry 不参与排序：高权重 p1 居首，加权随机选首仅基于 weight"
    );
}

// ── Sticky 经 order_load_balance：同权重档 expiry tiebreak 随之生效 ──

#[test]
fn sticky_inherits_load_balance_expiry_tiebreak() {
    // Sticky 模式复用 order_load_balance 作基础排序；无绑定时 apply_sticky 把当前首选写绑定。
    // 同权重三平台仅 expires_at 不同：先 order_load_balance（同权重档 expiry 升序），
    // 再 apply_sticky 无既有绑定 → 写绑定为首选（最早过期者），并保持顺序。
    let gp_noexp = mk_gp_exp(1, 1, 0, 0);
    let gp_far = mk_gp_exp(2, 1, 0, 10_000_000_000);
    let gp_near = mk_gp_exp(3, 1, 0, 1_000_000_000);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp_noexp, &gp_far, &gp_near];
    order_load_balance(&mut v, 0); // seed=0 → pick=0 无 swap，基础序保持
    assert_eq!(
        v.iter().map(|g| g.platform.id).collect::<Vec<_>>(),
        vec![3, 2, 1],
        "Sticky 基础排序经 order_load_balance：同权重档 expiry 升序"
    );

    let sched = SchedulerState::new();
    let sticky = StickyTable::new();
    let settings = mk_settings();
    let now = aidog_db::now();
    let ctx = ScheduleCtx {
        scheduler: &sched,
        sticky: &sticky,
        settings: &settings,
        sticky_key: Some("grpB|client1".to_string()),
    };
    apply_sticky(&mut v, Some(&ctx), now);
    // 无既有绑定 → 写绑定为当前首选（最早过期 p3）
    assert_eq!(
        sticky.get("grpB|client1", now),
        Some(3),
        "Sticky 绑定到最早过期首选 p3"
    );
    assert_eq!(v[0].platform.id, 3, "首选仍为最早过期平台");
}
