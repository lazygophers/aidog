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

/// T2 加载测试（boa 只在 aidog_adapter，求值断言落本 crate；解析函数跨 crate 取
/// aidog_db::registry）：platform.json 的 quota_scripts 解析出变体后，脚本文本
/// 可直接喂 eval_script 空跑求值（不触网）。
#[test]
fn registry_quota_script_parses_and_evaluates() {
    let locale = serde_json::json!({
        "en-US": "Official", "zh-Hans": "官方", "ar-SA": "رسمي", "fr-FR": "Officiel",
        "de-DE": "Offiziell", "ru-RU": "Официальный", "ja-JP": "公式", "es-ES": "Oficial",
    });
    let platform_json = serde_json::json!({
        "last_updated": 1,
        "quota_scripts": [{
            "id": "official",
            "name": locale,
            "requires": [{ "key": "panel_url", "label": locale }],
            "returns": { "balance": true, "tiers": ["monthly"] },
            "script": r#"
                const extra = JSON.parse(ctx.extra);
                return {
                    success: true,
                    balance: { remaining: 1.5, currency: "CNY", is_valid: true },
                    panel: extra.panel_url,
                };
            "#,
        }],
    })
    .to_string();

    let variants = aidog_db::registry::parse_quota_scripts(&platform_json);
    assert_eq!(variants.len(), 1);
    let v = &variants[0];
    assert_eq!(v.id, "official");
    assert_eq!(v.name["zh-Hans"], "官方");
    assert_eq!(v.requires[0].key, "panel_url");
    assert!(v.returns.balance);
    assert_eq!(v.returns.tiers, ["monthly"]);

    let q = eval_script(
        &CustomQueryCtx {
            base_url: "https://example.com/v1".into(),
            api_key: "sk-test".into(),
            extra: r#"{"panel_url":"https://panel.example"}"#.into(),
        },
        &v.script,
    )
    .unwrap();
    assert!(q.success);
    let b = q.balance.expect("balance");
    assert!((b.remaining - 1.5).abs() < 1e-9);
    assert_eq!(b.currency, "CNY");
}
