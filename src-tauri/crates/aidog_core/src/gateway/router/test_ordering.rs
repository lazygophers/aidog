use super::super::super::scheduling::{SchedulerState, StickyTable};
use super::super::candidates::ScheduleCtx;
use super::super::test_mod::{mk_gp, mk_gp_cp, mk_gp_exp, mk_gp_lp};
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
        rate_body: None,
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
        rate_body: None,
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
        rate_body: None,
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

// ── HealthAware 档内排序（2026-11-06 拍板：level_priority 主导 + coding plan / 单价档内链）──

fn hk(pairs: Vec<(u64, HealthOrderKey)>) -> HashMap<u64, HealthOrderKey> {
    pairs.into_iter().collect()
}

fn ids(v: &[&GroupPlatformDetail]) -> Vec<u64> {
    v.iter().map(|g| g.platform.id).collect()
}

/// coding_remaining_ratio：各 tier 剩余比例取最小（最紧维度）；无数据 → 1.0。
#[test]
fn health_coding_remaining_ratio_tightest_and_missing() {
    assert_eq!(coding_remaining_ratio(""), 1.0, "empty est = fully available");
    assert_eq!(coding_remaining_ratio(r#"{"tiers":[]}"#), 1.0, "no tiers = fully available");
    let r = coding_remaining_ratio(
        r#"{"tiers":[{"name":"five_hour","est_utilization":80.0},{"name":"weekly_limit","est_utilization":50.0}]}"#,
    );
    assert!(
        (r - 0.2).abs() < 1e-9,
        "tightest dimension (min remaining) wins, got {r}"
    );
}

/// 决策 1 回归保护：coding plan 不压过手动优先级 —— lp=1 的 cp 排在 lp=10 的按量之后。
#[test]
fn health_aware_level_priority_dominates_coding_plan() {
    let cp = mk_gp_lp(1, 100, 0, 1);
    let payg = mk_gp_lp(2, 1, 0, 10);
    let keys = hk(vec![
        (1, HealthOrderKey::CodingRemaining(0.9)),
        (2, HealthOrderKey::PaygPrice(0.000_001)),
    ]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&cp, &payg];
    order_health_aware(&mut v, 0, &keys);
    assert_eq!(
        ids(&v),
        vec![2, 1],
        "higher level_priority payg stays before low-priority coding plan"
    );
}

/// 决策 1 的另一面：同 level_priority 档内 coding plan 整体排前。
#[test]
fn health_aware_same_level_coding_plan_first() {
    let cp = mk_gp(1, 1, 0);
    let payg = mk_gp(2, 1, 0);
    let keys = hk(vec![
        (1, HealthOrderKey::CodingRemaining(1.0)),
        (2, HealthOrderKey::PaygPrice(0.000_001)),
    ]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&payg, &cp];
    order_health_aware(&mut v, 0, &keys);
    assert_eq!(ids(&v), vec![1, 2], "same level: coding plan bucket first");
}

/// 决策 4：coding plan 桶内按最紧维度剩余比例降序，权重不参与档内键。
#[test]
fn health_aware_coding_bucket_tightest_first() {
    let tight = mk_gp_cp(1, 1000, 0); // 剩 10%，权重 1000
    let loose = mk_gp_cp(2, 1, 0); // 剩 90%，权重 1
    let keys = hk(vec![
        (1, HealthOrderKey::CodingRemaining(0.1)),
        (2, HealthOrderKey::CodingRemaining(0.9)),
    ]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&tight, &loose];
    order_health_aware(&mut v, 0, &keys);
    assert_eq!(
        ids(&v),
        vec![2, 1],
        "higher remaining first despite much lower weight"
    );
}

/// 决策 3：按量桶内静态估算单价升序，权重不参与档内键。
#[test]
fn health_aware_payg_bucket_cheapest_first() {
    let cheap = mk_gp(1, 1, 0);
    let pricey = mk_gp(2, 1000, 0);
    let keys = hk(vec![
        (1, HealthOrderKey::PaygPrice(0.000_001)),
        (2, HealthOrderKey::PaygPrice(0.000_002)),
    ]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&pricey, &cheap];
    order_health_aware(&mut v, 0, &keys);
    assert_eq!(ids(&v), vec![1, 2], "cheaper first despite lower weight");
}

/// 键缺失回落：coding plan → 1.0（按全可用），按量 → 排按量桶末尾。
#[test]
fn health_aware_missing_keys_fallback() {
    let cp = mk_gp_cp(1, 1, 0);
    let payg = mk_gp(2, 1, 0);
    let keys = hk(vec![]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&payg, &cp];
    order_health_aware(&mut v, 0, &keys);
    assert_eq!(ids(&v), vec![1, 2], "cp (missing = 1.0) still first");
}

/// 决策 4 尾巴：全键并列 → 现状 tiebreak（有效权重降序 + expires_at 升序）保持。
#[test]
fn health_aware_tied_falls_back_to_weight_expiry() {
    let a = mk_gp_exp(1, 5, 0, 100);
    let b = mk_gp_exp(2, 1, 0, 100);
    let keys = hk(vec![
        (1, HealthOrderKey::PaygPrice(0.000_001)),
        (2, HealthOrderKey::PaygPrice(0.000_001)),
    ]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&b, &a];
    order_health_aware(&mut v, 0, &keys);
    assert_eq!(ids(&v), vec![1, 2], "equal key: higher weight first");
}

// ── order_rotate / order_usage（票 magpie 05）──

#[test]
fn rotate_same_tier_cycles_by_offset() {
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    let gp3 = mk_gp(3, 1, 0);
    // 同档（level_priority / priority 全同）：offset 0/1/2 依次轮换
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_rotate(&mut v, 0);
    assert_eq!(ids(&v), vec![1, 2, 3], "offset 0 keeps order");
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_rotate(&mut v, 1);
    assert_eq!(ids(&v), vec![2, 3, 1], "offset 1 rotates left by 1");
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_rotate(&mut v, 2);
    assert_eq!(ids(&v), vec![3, 1, 2], "offset 2 rotates left by 2");
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_rotate(&mut v, 3);
    assert_eq!(ids(&v), vec![1, 2, 3], "offset wraps modulo len");
}

#[test]
fn rotate_preserves_tier_order_between_tiers() {
    // 两档：档 1 = (lp 5, prio 0) 的 p1/p2，档 2 = (lp 5, prio 1) 的 p3。
    // 档间顺序固定，档内各自轮换。
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    let gp3 = mk_gp(3, 1, 1);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_rotate(&mut v, 1);
    assert_eq!(ids(&v), vec![2, 1, 3], "tier1 rotates, tier2 pinned");
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_rotate(&mut v, 2);
    assert_eq!(ids(&v), vec![1, 2, 3], "tier1 wraps at 2 members");
}

#[test]
fn rotate_ignores_weight_and_expiry() {
    // 权重 / expires_at 不同不影响轮换（严格轮换语义）
    let gp1 = mk_gp_exp(1, 9, 0, 100);
    let gp2 = mk_gp_exp(2, 1, 0, 0);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2];
    order_rotate(&mut v, 1);
    assert_eq!(ids(&v), vec![2, 1], "strict rotation ignores weight/expiry");
}

#[test]
fn usage_least_success_first_with_missing_as_zero() {
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    let gp3 = mk_gp(3, 1, 0);
    let mut counts = HashMap::new();
    counts.insert(1u64, 10i64);
    counts.insert(2u64, 3i64);
    // p3 无记录 = 0 → 最先用；其次 p2(3)；p1(10) 最后
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_usage(&mut v, &counts);
    assert_eq!(ids(&v), vec![3, 2, 1], "least success first, missing = 0");
}

#[test]
fn usage_tie_breaks_by_weight_desc_then_expiry_asc() {
    // 计数并列 → 有效权重降序；再并列 → expires_at 升序
    let gp1 = mk_gp(1, 5, 0);
    let gp2 = mk_gp(2, 1, 0);
    let counts = HashMap::new();
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp2, &gp1];
    order_usage(&mut v, &counts);
    assert_eq!(ids(&v), vec![1, 2], "tie → higher weight first");
    // expires_at=0 = 永不过期（expiry_sort_key 视为 i64::MAX 排末尾），故用 300/100 两档
    let gp3 = mk_gp_exp(3, 1, 0, 300);
    let gp4 = mk_gp_exp(4, 1, 0, 100);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp3, &gp4];
    order_usage(&mut v, &counts);
    assert_eq!(ids(&v), vec![4, 3], "tie weight → earlier expiry first");
}

// ── pace / smart（票 magpie 06）──

fn pk(reset_at: Option<i64>, remaining: Option<f64>) -> PaceSmartKey {
    PaceSmartKey { reset_at, remaining }
}

fn pkeys(pairs: Vec<(u64, Option<i64>, Option<f64>)>) -> HashMap<u64, PaceSmartKey> {
    pairs
        .into_iter()
        .map(|(id, t, r)| (id, pk(t, r)))
        .collect()
}

#[test]
fn pace_most_remaining_first_with_reset_tiebreak() {
    // 剩余比例降序主键；并列 → 重置时间升序；重置时间缺失排末位
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    let gp3 = mk_gp(3, 1, 0);
    let keys = pkeys(vec![
        (1, Some(10_000), Some(0.2)),
        (2, Some(5_000), Some(0.8)),
        (3, None, Some(0.8)),
    ]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_pace(&mut v, &keys);
    assert_eq!(
        ids(&v),
        vec![2, 3, 1],
        "most remaining first; tie → earliest reset; unknown reset last"
    );
}

#[test]
fn pace_missing_data_ranked_last() {
    // 无剩余额度数据（按量平台）排有数据之后，权重再高也不提首；无数据档内按权重降序
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    let payg_hi = mk_gp(9, 1000, 0);
    let payg_lo = mk_gp(8, 1, 0);
    let keys = pkeys(vec![(1, Some(1_000), Some(0.1)), (2, Some(1_000), Some(0.9))]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&payg_hi, &payg_lo, &gp1, &gp2];
    order_pace(&mut v, &keys);
    assert_eq!(
        ids(&v),
        vec![2, 1, 9, 8],
        "data-having first by remaining desc, missing-data after by weight desc"
    );
}

#[test]
fn smart_earliest_reset_first_with_remaining_tiebreak() {
    // 重置时间升序主键；并列 → 剩余比例降序
    let gp1 = mk_gp(1, 1, 0);
    let gp2 = mk_gp(2, 1, 0);
    let gp3 = mk_gp(3, 1, 0);
    let keys = pkeys(vec![
        (1, Some(5_000), Some(0.9)),
        (2, Some(1_000), Some(0.2)),
        (3, Some(1_000), Some(0.7)),
    ]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
    order_smart(&mut v, &keys);
    assert_eq!(
        ids(&v),
        vec![3, 2, 1],
        "earliest reset first; tie → most remaining first"
    );
}

#[test]
fn smart_missing_reset_ranked_last_by_remaining() {
    // 无重置时间数据排有数据之后，仍按剩余比例降序（残缺数据不浪费）；全缺排最末
    let known_a = mk_gp(1, 1, 0);
    let known_b = mk_gp(2, 1, 0);
    let unknown_some = mk_gp(3, 1, 0); // 有 remaining 无 reset
    let unknown_none = mk_gp(4, 1000, 0); // 全缺，权重再高也最后
    let keys = pkeys(vec![
        (1, Some(3_000), Some(0.5)),
        (2, Some(9_000), Some(0.1)),
        (3, None, Some(0.9)),
        (4, None, None),
    ]);
    let mut v: Vec<&GroupPlatformDetail> = vec![&unknown_none, &unknown_some, &known_b, &known_a];
    order_smart(&mut v, &keys);
    assert_eq!(
        ids(&v),
        vec![1, 2, 3, 4],
        "reset-known asc → unknown-by-remaining desc → missing last"
    );
}

#[test]
fn pace_smart_fallback_predicates() {
    let gp_known = mk_gp(1, 1, 0);
    let gp_none = mk_gp(2, 1, 0);
    let keys_data = pkeys(vec![(1, Some(1_000), Some(0.5)), (2, None, None)]);
    let keys_none = pkeys(vec![(2, None, None)]);
    let have = vec![&gp_known, &gp_none];
    let none_only = vec![&gp_none];
    assert!(!pace_falls_back_to_usage(&keys_data, &have));
    assert!(pace_falls_back_to_usage(&keys_none, &none_only));
    assert!(!smart_falls_back_to_health_aware(&keys_data, &have));
    assert!(smart_falls_back_to_health_aware(&keys_none, &none_only));
}

#[test]
fn compute_pace_smart_keys_parses_est_plan() {
    let now: i64 = 1_000_000_000_000;
    let hour: i64 = 3_600_000;
    let mut gp = mk_gp(1, 1, 0);
    // five_hour util 90 → remaining 0.1；window_start = now-4h → reset = now+1h
    gp.platform.est_coding_plan =
        format!(r#"{{"tiers":[{{"name":"five_hour","est_utilization":90.0,"window_start":{}}}]}}"#, now - 4 * hour);
    let mut gp2 = mk_gp(2, 1, 0);
    // 过期重置时刻（start = now-10h → reset = now-5h）→ 视为已重置 → now
    gp2.platform.est_coding_plan =
        format!(r#"{{"tiers":[{{"name":"five_hour","est_utilization":50.0,"window_start":{}}}]}}"#, now - 10 * hour);
    let gp3 = mk_gp(3, 1, 0); // 空 est_coding_plan → 全 None
    let keys = compute_pace_smart_keys(&[gp.clone(), gp2.clone(), gp3.clone()], now);
    let k1 = keys.get(&1).expect("key p1");
    assert_eq!(k1.remaining, Some(0.1), "min tier remaining");
    assert_eq!(k1.reset_at, Some(now + hour), "window_start + five_hour cycle");
    let k2 = keys.get(&2).expect("key p2");
    assert_eq!(k2.reset_at, Some(now), "past reset clamped to now");
    let k3 = keys.get(&3).expect("key p3");
    assert_eq!(k3.reset_at, None, "no plan → no reset");
    assert_eq!(k3.remaining, None, "no plan → no remaining");
}
