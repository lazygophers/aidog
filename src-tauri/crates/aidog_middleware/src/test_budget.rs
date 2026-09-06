//! 票 06 预算闸门单测：未超限放行、刚好超限拒绝、跨月归零、applies_to 三维过滤生效。

use super::test_mod::{chat_req, mk_rule};
use super::*;
use aidog_db::Db;
use aidog_db::models::{
    ActionKind, ActionParams, ActionStep, AppliesTo, ConditionNode, MiddlewareSettings,
};
use aidog_stats::{StatsAggInput, upsert_stats_agg};

const ON: MiddlewareSettings = MiddlewareSettings { enabled: true };

/// 预算闸门动作：本月上限 `budget_usd` 美元。
fn budget_step(budget_usd: f64) -> ActionStep {
    ActionStep {
        kind: ActionKind::BudgetGate,
        params: ActionParams {
            budget_usd,
            ..Default::default()
        },
    }
}

/// 条件树留空（`ALL()` vacuous true）= 范围内每条请求都过闸门，作用范围只由 applies_to 定。
fn always() -> ConditionNode {
    ConditionNode::All { children: vec![] }
}

fn engine_with(budget_usd: f64, applies_to: AppliesTo) -> MiddlewareEngine {
    let mut rule = mk_rule(1, "monthly budget", always(), vec![budget_step(budget_usd)]);
    rule.applies_to = applies_to;
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![rule]);
    e
}

/// 往 stats_agg_hourly 记一条花费（预算聚合的真值源）。
async fn spend(db: &Db, group_key: &str, platform_id: i64, model: &str, cost: f64) {
    upsert_stats_agg(
        db,
        StatsAggInput {
            created_at: chrono::Utc::now().timestamp_millis(),
            model: model.to_string(),
            group_key: group_key.to_string(),
            platform_id,
            status_code: 200,
            input_tokens: 10,
            output_tokens: 20,
            cache_tokens: 0,
            est_cost: cost,
            duration_ms: 100,
        },
    )
    .await
    .unwrap();
}

async fn check(engine: &MiddlewareEngine, db: &Db, group: Option<&str>) -> InboundOutcome {
    engine
        .check_budget(&ON, db, &chat_req("", "hi"), group, None, None)
        .await
}

/// 未超限 → 放行。
#[tokio::test]
async fn under_budget_passes() {
    let db = aidog_db::test_support::test_db().await;
    spend(&db, "ga", 1, "m", 4.99).await;
    let e = engine_with(5.0, AppliesTo::default());
    assert_eq!(check(&e, &db, Some("ga")).await, InboundOutcome::Continue);
}

/// 刚好花到预算 → 拒绝（`>=` 语义），blocked_reason 带可区分前缀。
#[tokio::test]
async fn exactly_at_budget_blocks() {
    let db = aidog_db::test_support::test_db().await;
    spend(&db, "ga", 1, "m", 5.0).await;
    let e = engine_with(5.0, AppliesTo::default());
    match check(&e, &db, Some("ga")).await {
        InboundOutcome::Blocked {
            blocked_by,
            blocked_reason,
        } => {
            assert!(blocked_by.starts_with("rule#1 "), "blocked_by: {blocked_by}");
            assert!(
                blocked_reason.starts_with(BUDGET_BLOCKED_REASON),
                "原因值须可与普通拦截区分: {blocked_reason}"
            );
        }
        other => panic!("expected Blocked, got {other:?}"),
    }
}

/// 超预算 → 拒绝。
#[tokio::test]
async fn over_budget_blocks() {
    let db = aidog_db::test_support::test_db().await;
    spend(&db, "ga", 1, "m", 12.5).await;
    let e = engine_with(10.0, AppliesTo::default());
    assert!(matches!(
        check(&e, &db, Some("ga")).await,
        InboundOutcome::Blocked { .. }
    ));
}

/// 无 budget_gate 动作的规则不触发闸门（也就不查库）。
#[tokio::test]
async fn no_budget_rule_passes() {
    let db = aidog_db::test_support::test_db().await;
    spend(&db, "ga", 1, "m", 999.0).await;
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "noop", always(), vec![])]);
    assert_eq!(check(&e, &db, Some("ga")).await, InboundOutcome::Continue);
}

/// budget_usd ≤ 0 = 未配置，闸门不生效。
#[tokio::test]
async fn zero_budget_is_disabled() {
    let db = aidog_db::test_support::test_db().await;
    spend(&db, "ga", 1, "m", 999.0).await;
    let e = engine_with(0.0, AppliesTo::default());
    assert_eq!(check(&e, &db, Some("ga")).await, InboundOutcome::Continue);
}

/// 跨月归零：上个月的花费不计入本月窗口。
#[tokio::test]
async fn previous_month_spend_not_counted() {
    use chrono::{Datelike, Local, TimeZone};
    let db = aidog_db::test_support::test_db().await;
    // 上个月 15 号中午（本地时区）——窗口起点是本月 1 号，故不该被计入。
    let now = Local::now();
    let (y, m) = if now.month() == 1 {
        (now.year() - 1, 12)
    } else {
        (now.year(), now.month() - 1)
    };
    let prev_ms = Local
        .with_ymd_and_hms(y, m, 15, 12, 0, 0)
        .single()
        .expect("prev month ts")
        .timestamp_millis();
    upsert_stats_agg(
        &db,
        StatsAggInput {
            created_at: prev_ms,
            model: "m".to_string(),
            group_key: "ga".to_string(),
            platform_id: 1,
            status_code: 200,
            input_tokens: 10,
            output_tokens: 20,
            cache_tokens: 0,
            est_cost: 999.0,
            duration_ms: 100,
        },
    )
    .await
    .unwrap();

    let e = engine_with(5.0, AppliesTo::default());
    assert_eq!(
        check(&e, &db, Some("ga")).await,
        InboundOutcome::Continue,
        "上月花费须随自然月归零"
    );
    // 本月再花 5 元即触顶。
    spend(&db, "ga", 1, "m", 5.0).await;
    assert!(matches!(
        check(&e, &db, Some("ga")).await,
        InboundOutcome::Blocked { .. }
    ));
}

/// applies_to 三维过滤生效：范围外的花费不计入，范围外的请求不过闸门。
#[tokio::test]
async fn applies_to_scopes_both_spend_and_request() {
    let db = aidog_db::test_support::test_db().await;
    spend(&db, "ga", 1, "m-one", 9.0).await; // 在范围内
    spend(&db, "gb", 2, "m-two", 100.0).await; // 范围外，不该计入

    // group 维：只统计 ga 的花费 → 9 < 10 放行。
    let e = engine_with(
        10.0,
        AppliesTo {
            groups: vec!["ga".into()],
            ..Default::default()
        },
    );
    assert_eq!(
        check(&e, &db, Some("ga")).await,
        InboundOutcome::Continue,
        "范围外的 100 元不该计入 ga 的预算"
    );
    // 范围外的请求（gb）根本不过这条规则。
    assert_eq!(check(&e, &db, Some("gb")).await, InboundOutcome::Continue);

    // group 维预算收紧到 5 → ga 的 9 元超限，gb 仍不受这条规则约束。
    let e = engine_with(
        5.0,
        AppliesTo {
            groups: vec!["ga".into()],
            ..Default::default()
        },
    );
    assert!(matches!(
        check(&e, &db, Some("ga")).await,
        InboundOutcome::Blocked { .. }
    ));
    assert_eq!(check(&e, &db, Some("gb")).await, InboundOutcome::Continue);

    // platform 维：platform_id=2 的 100 元 → 超限（请求侧带 platform_id 才命中）。
    let e = engine_with(
        50.0,
        AppliesTo {
            platforms: vec![2],
            ..Default::default()
        },
    );
    assert!(matches!(
        e.check_budget(&ON, &db, &chat_req("", "hi"), None, Some(2), None)
            .await,
        InboundOutcome::Blocked { .. }
    ));
    // group 层挂载点（platform_id=None）不命中 platform 维规则。
    assert_eq!(check(&e, &db, Some("ga")).await, InboundOutcome::Continue);

    // model 维：chat_req 的模型是 "test-model"，与作用范围 "m-one" 不符 → 规则不适用。
    let e = engine_with(
        1.0,
        AppliesTo {
            models: vec!["m-one".into()],
            ..Default::default()
        },
    );
    assert_eq!(check(&e, &db, Some("ga")).await, InboundOutcome::Continue);
}

/// 总开关关闭时闸门旁路。
#[tokio::test]
async fn master_switch_off_bypasses() {
    let db = aidog_db::test_support::test_db().await;
    spend(&db, "ga", 1, "m", 999.0).await;
    let e = engine_with(1.0, AppliesTo::default());
    let off = MiddlewareSettings { enabled: false };
    assert_eq!(
        e.check_budget(&off, &db, &chat_req("", "hi"), Some("ga"), None, None)
            .await,
        InboundOutcome::Continue
    );
}

/// budget_states：前端展示用的已用 / 剩余额度。
#[tokio::test]
async fn budget_states_reports_spent_and_remaining() {
    let db = aidog_db::test_support::test_db().await;
    spend(&db, "ga", 1, "m", 3.0).await;
    let e = engine_with(10.0, AppliesTo::default());
    let states = e.budget_states(&db).await;
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].rule_id, 1);
    assert_eq!(states[0].budget_usd, 10.0);
    assert_eq!(states[0].spent_usd, 3.0);
    assert_eq!(states[0].remaining_usd(), 7.0);
    assert!(!states[0].exceeded());
}
