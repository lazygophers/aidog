//! JS 自定义查询脚本测试：纯 eval（不触网）+ 固定格式解析 + 出站注入/落库。
use super::*;

fn qctx() -> CustomQueryCtx {
    CustomQueryCtx {
        base_url: "https://example.com/v1".into(),
        api_key: "sk-test".into(),
        extra: r#"{"foo":1}"#.into(),
    }
}

fn outbound() -> Outbound {
    // 纯 eval 测试不触网，client 仅占位
    Outbound { client: reqwest::Client::new(), db: None }
}

async fn spawn_stub(status: u16, body: &'static str) -> String {
    use axum::routing::any;
    let app = axum::Router::new().fallback(any(move || async move {
        (
            axum::http::StatusCode::from_u16(status).unwrap(),
            [("content-type", "application/json")],
            body,
        )
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    format!("http://{addr}")
}

#[test]
fn script_returns_balance() {
    let q = eval_script(
        &qctx(),
        &outbound(),
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
        &outbound(),
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
fn script_returns_newapi_user_id() {
    let q = eval_script(
        &qctx(),
        &outbound(),
        r#"
        return {
            success: true,
            balance: { remaining: 1.0, currency: "CNY", is_valid: true },
            newapi_user_id: "42",
        };
        "#,
    )
    .unwrap();
    assert!(q.success);
    assert_eq!(q.newapi_user_id.as_deref(), Some("42"), "顶层 newapi_user_id 须透传");

    // 未返回时缺省 None
    let q = eval_script(&qctx(), &outbound(), r#"return { success: true };"#).unwrap();
    assert_eq!(q.newapi_user_id, None);
}

#[test]
fn script_can_read_ctx_and_parse_json() {
    let q = eval_script(
        &qctx(),
        &outbound(),
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
    let q = eval_script(&qctx(), &outbound(), "throw new Error('boom');").unwrap_err();
    assert!(q.contains("boom"), "须携带脚本错误信息: {q}");

    // success=false + error
    let q = eval_script(
        &qctx(),
        &outbound(),
        r#"return { success: false, error: "quota exceeded" };"#,
    )
    .unwrap();
    assert!(!q.success);
    assert_eq!(q.error.as_deref(), Some("quota exceeded"));
}

#[test]
fn script_non_object_return_rejected() {
    let q = eval_script(&qctx(), &outbound(), r#"return 42;"#).unwrap();
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

/// 出站走 CLIENT_BUILDER 注入的 client（app_setup 注系统代理 client 的同一通道），
/// 成功出站落 proxy_log（group_key="[quota:script]"，source_protocol="quota"）。
/// 注入的 builder 返回直连 + 超时 client（与缺省回落一致），经 AtomicBool 标记断言
/// 确被调用——不改变行为，避免污染同进程并行测试。
#[tokio::test]
async fn script_outbound_uses_injected_client_and_persists_log() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::quota::http::set_client_builder;

    static BUILDER_CALLED: AtomicBool = AtomicBool::new(false);
    set_client_builder(Arc::new(|_db| {
        BUILDER_CALLED.store(true, Ordering::SeqCst);
        Box::pin(async {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default()
        })
    }));

    let url = spawn_stub(200, r#"{"ok":true}"#).await;
    let db = Arc::new(aidog_db::test_support::test_db().await);
    let script = format!(r#"var r = http.get("{url}"); return {{ success: true }};"#);
    let q = run_custom_query(Some(&db), qctx(), &script, 0).await;
    assert!(q.success, "脚本须跑通本地 stub: {:?}", q.error);
    assert!(
        BUILDER_CALLED.load(Ordering::SeqCst),
        "出站须走 CLIENT_BUILDER 注入的 client（db=Some 时）"
    );

    let logs = aidog_logs::list_proxy_logs(&db, 100, 0).await.unwrap();
    let hit = logs
        .iter()
        .find(|l| l.group_key == "[quota:script]")
        .expect("成功出站须落 proxy_log");
    assert_eq!(hit.source_protocol, "quota");
    assert_eq!(hit.status_code, 200);
}

/// 非 2xx 出站同样落 proxy_log（upstream_status_code 原样），错误文案可被脚本 catch。
#[tokio::test]
async fn script_outbound_error_persists_log() {
    let url = spawn_stub(500, r#"{"e":"x"}"#).await;
    let db = Arc::new(aidog_db::test_support::test_db().await);
    let script = format!(
        r#"
        try {{
            http.get("{url}");
            return {{ success: false, error: "unreachable" }};
        }} catch (e) {{
            return {{ success: false, error: "caught: " + e.message }};
        }}
        "#,
    );
    let q = run_custom_query(Some(&db), qctx(), &script, 0).await;
    assert!(!q.success);
    assert!(q.error.unwrap().starts_with("caught:"), "脚本须能 catch 非 2xx");

    let logs = aidog_logs::list_proxy_logs(&db, 100, 0).await.unwrap();
    let hit = logs
        .iter()
        .find(|l| l.group_key == "[quota:script]")
        .expect("非 2xx 出站也须落 proxy_log");
    assert_eq!(hit.status_code, 500);
}
