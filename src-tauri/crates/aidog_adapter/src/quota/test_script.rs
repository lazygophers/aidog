//! JS 自定义查询脚本测试：纯 eval（不触网）+ 固定格式解析。
use super::*;

fn qctx() -> CustomQueryCtx {
    CustomQueryCtx {
        base_url: "https://example.com/v1".into(),
        api_key: "sk-test".into(),
        extra: r#"{"foo":1}"#.into(),
    }
}

#[test]
fn script_returns_balance() {
    let q = eval_script(
        &qctx(),
        r#"
        return {
            success: true,
            balance: { remaining: 1.5, currency: "CNY", is_valid: true },
        };
        "#,
    )
    .unwrap();
    assert!(q.success);
    let b = q.balance.unwrap();
    assert!((b.remaining - 1.5).abs() < 1e-9);
    assert_eq!(b.currency, "CNY");
}

#[test]
fn script_returns_coding_plan_tiers() {
    let q = eval_script(
        &qctx(),
        r#"
        return {
            success: true,
            coding_plan: { level: "pro", tiers: [
                { name: "five_hour", utilization: 42.5 },
                { name: "weekly_limit", utilization: 10.0, limit: 100, remaining: 90 },
            ]},
        };
        "#,
    )
    .unwrap();
    let cp = q.coding_plan.unwrap();
    assert_eq!(cp.level.as_deref(), Some("pro"));
    assert_eq!(cp.tiers.len(), 2);
    assert!((cp.tiers[0].utilization - 42.5).abs() < 1e-9);
    assert_eq!(cp.tiers[1].limit, Some(100.0));
}

#[test]
fn script_can_read_ctx_and_parse_json() {
    let q = eval_script(
        &qctx(),
        r#"
        const extra = JSON.parse(ctx.extra);
        return { success: extra.foo === 1 };
        "#,
    )
    .unwrap();
    assert!(q.success, "ctx.extra 须可 JSON.parse");
}

#[test]
fn script_error_path() {
    // 脚本 throw → err_quota
    let q = eval_script(&qctx(), "throw new Error('boom');").unwrap_err();
    assert!(q.contains("boom"), "须携带脚本错误信息: {q}");

    // success=false + error
    let q = eval_script(
        &qctx(),
        r#"return { success: false, error: "quota exceeded" };"#,
    )
    .unwrap();
    assert!(!q.success);
    assert_eq!(q.error.as_deref(), Some("quota exceeded"));
}

#[test]
fn script_non_object_return_rejected() {
    let q = eval_script(&qctx(), r#"return 42;"#).unwrap();
    assert!(!q.success);
    assert!(q.error.unwrap().contains("must return an object"));
}

#[test]
fn script_http_error_is_catchable() {
    // http.get 对不存在 host 报错 → 脚本可 try/catch 转 success=false
    // （eval 在同步测试线程内跑，block_in_place 需多线程 rt——包 tokio::test）
}

#[tokio::test]
async fn script_http_error_propagates() {
    let q = run_custom_query(
        None,
        qctx(),
        r#"
        try {
            http.get("http://127.0.0.1:1/nope");
            return { success: false, error: "unreachable" };
        } catch (e) {
            return { success: false, error: "caught: " + e.message };
        }
        "#,
        0,
    )
    .await;
    assert!(!q.success);
    assert!(q.error.unwrap().starts_with("caught:"), "脚本须能 catch http 错误");
}
