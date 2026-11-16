//! 特化参数族等价测试（quota-scripts T4）：devin（requires org_id）/ newapi
//! （api-key 脚本默认 + access-token 脚本，互不回退；newapi_user_id）的
//! registry 脚本 + 路由捕获 stub。等价差异（t3c-handoff）：HTTP 错误带 body 只锁
//! 前缀；api_key 空检查在脚本内（专属 command 不经 dispatch 通用检查）。
use super::script::{CustomQueryCtx, run_custom_query};
use super::test_stub::spawn_capture;
use aidog_db::registry;

fn script_for(code: &str) -> String {
    registry::quota_scripts_in(registry::presets(), code)
        .unwrap_or_default()
        .into_iter()
        .next()
        .map(|v| v.script)
        .unwrap_or_else(|| panic!("registry {code} 无 quota 脚本"))
}

async fn run(
    script: &str,
    base_url: &str,
    extra: &str,
    api_key: &str,
) -> crate::quota::PlatformQuota {
    run_custom_query(
        None,
        CustomQueryCtx {
            base_url: base_url.to_string(),
            api_key: api_key.into(),
            extra: extra.into(),
            protocol_code: String::new(),
        },
        script,
        0,
    )
    .await
}

// ── newapi：api-key 脚本（默认）与 access-token 脚本，两者互不回退 ──

/// 按变体 id 取脚本（与运行时 `select_quota_variant` 同语义：id 失效回落首条）。
fn newapi_script(id: &str) -> String {
    let variants = registry::quota_scripts_in(registry::presets(), "newapi").unwrap_or_default();
    registry::select_quota_variant(&variants, Some(id))
        .map(|v| v.script.clone())
        .expect("registry newapi 无 quota 脚本")
}

/// api-key 脚本 token usage 路由 stub；返回 stub 根 URL。
async fn newapi_step1(body: &'static str) -> String {
    spawn_capture(vec![("/api/usage/token/", 200, body)])
        .await
        .0
}

#[test]
fn newapi_default_variant_is_api_key() {
    let variants = registry::quota_scripts_in(registry::presets(), "newapi").unwrap_or_default();
    let ids: Vec<&str> = variants.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(ids, ["api_key", "access_token"]);
    assert!(variants[0].requires.is_empty(), "api-key 脚本无额外参数");
    let keys: Vec<&str> = variants[1].requires.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(keys, ["balance_base_url", "balance_api_key"]);
}

#[tokio::test]
async fn newapi_api_key_limited_scales_by_5e5() {
    let stub = newapi_step1(
        r#"{"data":{"unlimited_quota":false,"total_granted":1500000,"total_used":500000,"total_available":1000000}}"#,
    )
    .await;
    let q = run(&newapi_script("api_key"), &format!("{stub}/v1"), "{}", "sk-n").await;
    assert!(q.success, "{:?}", q.error);
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 2.0);
    assert_eq!(b.total, Some(3.0));
    assert_eq!(b.used, Some(1.0));
    assert_eq!(b.currency, "USD");
    assert!(b.is_valid);
    assert_eq!(q.newapi_user_id, None);
}

#[tokio::test]
async fn newapi_api_key_zero_all() {
    let stub = newapi_step1(r#"{"data":{}}"#).await;
    let q = run(&newapi_script("api_key"), &stub, "{}", "sk-n").await;
    assert!(q.success, "{:?}", q.error);
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 0.0);
    assert_eq!(b.total, None);
    assert_eq!(b.used, None);
    assert!(!b.is_valid);
}

#[tokio::test]
async fn newapi_api_key_url_and_headers() {
    // instance_root 剥最后一段 /v<纯数字>（/openai/v1 → /openai）+ query 双入参 + Bearer
    let (stub, log) =
        spawn_capture(vec![("/openai/api/usage/token/", 200, r#"{"data":{}}"#)]).await;
    let q = run(
        &newapi_script("api_key"),
        &format!("{stub}/openai/v1"),
        "{}",
        "sk my key",
    )
    .await;
    assert!(q.success, "{:?}", q.error);
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(
        log[0].path,
        "/openai/api/usage/token/?key=sk%20my%20key&api_key=sk%20my%20key"
    );
    assert_eq!(log[0].authorization, "Bearer sk my key");
}

#[tokio::test]
async fn newapi_api_key_unlimited_errors_without_user_self_call() {
    let (stub, log) = spawn_capture(vec![
        (
            "/api/usage/token/",
            200,
            r#"{"data":{"unlimited_quota":true}}"#,
        ),
        (
            "/api/user/self",
            200,
            r#"{"success":true,"data":{"id":42,"quota":250000,"used_quota":50000}}"#,
        ),
    ])
    .await;
    // 即使填了 access-token 参数也不回退
    let extra = format!(r#"{{"balance_base_url":"{stub}","balance_api_key":"bk-1"}}"#);
    let q = run(&newapi_script("api_key"), &stub, &extra, "sk-n").await;
    assert!(!q.success);
    let e = q.error.unwrap();
    assert!(e.contains("不限额") && e.contains("access-token"), "实际: {e}");
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1, "只许请求 token usage");
    assert!(log[0].path.starts_with("/api/usage/token/"));
}

#[tokio::test]
async fn newapi_api_key_error_paths() {
    let code = newapi_script("api_key");
    // 空 api_key
    let q = run(&code, "https://x.com", "{}", "  ").await;
    assert!(!q.success);
    assert_eq!(
        q.error.as_deref(),
        Some("New API: api_key required for token usage query")
    );
    // data 键缺失
    let stub = newapi_step1(r#"{"message":"nope"}"#).await;
    let q2 = run(&code, &stub, "{}", "sk-n").await;
    assert_eq!(q2.error.as_deref(), Some("Token usage: Missing data field"));
    // data:null → limited 全 0
    let stub2 = newapi_step1(r#"{"data":null}"#).await;
    let q3 = run(&code, &stub2, "{}", "sk-n").await;
    assert!(q3.success, "{:?}", q3.error);
    assert_eq!(q3.balance.unwrap().remaining, 0.0);
    // HTTP 错误
    let (stub3, _) =
        spawn_capture(vec![("/api/usage/token/", 403, r#"{"error":"forbidden"}"#)]).await;
    let q4 = run(&code, &stub3, "{}", "sk-n").await;
    let e = q4.error.unwrap();
    assert!(e.starts_with("Token usage: HTTP 403"), "实际: {e}");
}

#[tokio::test]
async fn newapi_access_token_user_self_with_user_id() {
    let (stub, log) = spawn_capture(vec![
        (
            "/api/user/self",
            200,
            r#"{"success":true,"data":{"id":42,"quota":250000,"used_quota":50000}}"#,
        ),
        ("/api/usage/token/", 200, r#"{"data":{"unlimited_quota":false}}"#),
    ])
    .await;
    let extra = format!(r#"{{"newapi":{{"balance_base_url":"{stub}","balance_api_key":"bk-1"}}}}"#);
    let q = run(&newapi_script("access_token"), "https://unused.invalid/v1", &extra, "sk-n").await;
    assert!(q.success, "{:?}", q.error);
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 0.5);
    assert_eq!(b.total, Some(0.6), "total = remaining + used");
    assert_eq!(b.used, Some(0.1));
    assert_eq!(q.newapi_user_id.as_deref(), Some("42"));
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1, "不得请求 token usage");
    assert_eq!(log[0].path, "/api/user/self");
    assert_eq!(log[0].authorization, "Bearer bk-1");
}

#[tokio::test]
async fn newapi_access_token_top_level_extra_and_root_fallback() {
    let (stub, log) = spawn_capture(vec![(
        "/api/user/self",
        200,
        r#"{"success":true,"data":{"id":"u-7","quota":0,"used_quota":0}}"#,
    )])
    .await;
    // 顶层 extra（requires 表单写法）；balance_base_url 缺省 → 平台根（剥 /v1）
    let q = run(
        &newapi_script("access_token"),
        &format!("{stub}/v1"),
        r#"{"balance_api_key":"bk-2"}"#,
        "",
    )
    .await;
    assert!(q.success, "{:?}", q.error);
    assert_eq!(q.newapi_user_id.as_deref(), Some("u-7"));
    assert!(!q.balance.unwrap().is_valid, "remaining 0 → invalid");
    assert_eq!(log.lock().unwrap()[0].authorization, "Bearer bk-2");
}

#[tokio::test]
async fn newapi_access_token_missing_token_errors() {
    let (stub, log) = spawn_capture(vec![("/api/user/self", 200, r#"{"success":true,"data":{}}"#)]).await;
    let q = run(&newapi_script("access_token"), &stub, "{}", "sk-n").await;
    assert!(!q.success);
    let e = q.error.unwrap();
    assert!(e.contains("access token required"), "实际: {e}");
    assert!(log.lock().unwrap().is_empty(), "缺 token 不发请求");
}

#[tokio::test]
async fn newapi_access_token_self_failures() {
    let code = newapi_script("access_token");
    let (stub, _) = spawn_capture(vec![(
        "/api/user/self",
        200,
        r#"{"success":false,"message":"denied"}"#,
    )])
    .await;
    let extra = format!(r#"{{"balance_base_url":"{stub}","balance_api_key":"k"}}"#);
    let q = run(&code, &stub, &extra, "").await;
    assert_eq!(q.error.as_deref(), Some("denied"));

    let (stub2, _) = spawn_capture(vec![("/api/user/self", 200, r#"{"success":false}"#)]).await;
    let extra2 = format!(r#"{{"balance_base_url":"{stub2}","balance_api_key":"k"}}"#);
    let q2 = run(&code, &stub2, &extra2, "").await;
    assert_eq!(q2.error.as_deref(), Some("Query failed"));
}

// ── devin：ACU 用量 ───────────────────────────────────────

/// devin 脚本（固定 host 常量改指 stub）。
fn devin_script(stub: &str) -> String {
    let s = script_for("devin");
    let out = s.replace("https://api.devin.ai", stub);
    assert!(out != s, "devin 脚本未含 host 常量，retarget 失效");
    out
}

#[tokio::test]
async fn devin_success_records_total_acus() {
    let (stub, log) = spawn_capture(vec![(
        "/v3/organizations/",
        200,
        r#"{"total_acus":1234.5,"acus_by_product":{"devin":1000.0}}"#,
    )])
    .await;
    let extra = r#"{"devin":{"org_id":"org abc/x"}}"#;
    let q = run(&devin_script(&stub), &stub, extra, "cog-1").await;
    assert!(q.success, "{:?}", q.error);
    let b = q.balance.unwrap();
    assert_eq!(b.used, Some(1234.5));
    assert_eq!(b.remaining, 0.0);
    assert_eq!(b.total, None);
    assert_eq!(b.currency, "ACU");
    assert!(b.is_valid);
    assert_eq!(q.newapi_user_id, None);
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    // org_id encodeURIComponent + Bearer cog key
    assert_eq!(
        log[0].path,
        "/v3/organizations/org%20abc%2Fx/consumption/daily"
    );
    assert_eq!(log[0].authorization, "Bearer cog-1");
}

#[tokio::test]
async fn devin_string_total_acus_and_zero_valid() {
    let (stub, _) =
        spawn_capture(vec![("/v3/organizations/", 200, r#"{"total_acus":"42"}"#)]).await;
    let extra = r#"{"org_id":"org-1"}"#; // 顶层兜底（requires 表单约定）
    let q = run(&devin_script(&stub), &stub, extra, "cog-1").await;
    assert!(q.success, "{:?}", q.error);
    assert_eq!(q.balance.unwrap().used, Some(42.0));

    let (stub2, _) =
        spawn_capture(vec![("/v3/organizations/", 200, r#"{"total_acus":0.0}"#)]).await;
    let q2 = run(&devin_script(&stub2), &stub2, r#"{"org_id":"o"}"#, "cog-1").await;
    assert!(q2.success);
    assert!(q2.balance.unwrap().is_valid, "0 ACU 仍 valid");
}

#[tokio::test]
async fn devin_missing_total_acus_and_null_body() {
    let (stub, _) = spawn_capture(vec![(
        "/v3/organizations/",
        200,
        r#"{"acus_by_product":{}}"#,
    )])
    .await;
    let q = run(&devin_script(&stub), &stub, r#"{"org_id":"o"}"#, "cog-1").await;
    assert!(!q.success);
    assert_eq!(q.error.as_deref(), Some("Missing total_acus field"));

    // body 为 JSON null → 同样 Missing（Value::Null.get() → None）
    let (stub2, _) = spawn_capture(vec![("/v3/organizations/", 200, "null")]).await;
    let q2 = run(&devin_script(&stub2), &stub2, r#"{"org_id":"o"}"#, "cog-1").await;
    assert_eq!(q2.error.as_deref(), Some("Missing total_acus field"));
}

#[tokio::test]
async fn devin_org_id_priority_and_errors() {
    // 空 api_key（脚本内检查，不触网）
    let q = run(&script_for("devin"), "https://x", r#"{"org_id":"o"}"#, " ").await;
    assert!(!q.success);
    assert_eq!(q.error.as_deref(), Some("Devin: api_key required"));

    // org_id 缺失 / bad extra → 逐字符同源码文案（不触网）
    let missing = run(&script_for("devin"), "https://x", "{}", "cog-1").await;
    assert_eq!(
        missing.error.as_deref(),
        Some(r#"Devin: missing org_id in platform.extra (expected {"devin":{"org_id":"<id>"}})"#)
    );
    let bad = run(&script_for("devin"), "https://x", "not json", "cog-1").await;
    assert!(bad.error.unwrap().contains("missing org_id"));

    // 嵌套优先：nested.org_id 存在但空白 → 不回落顶层（与 Rust devin? 命中即不回落一致）
    let stub = spawn_capture(vec![("/v3/organizations/", 200, r#"{"total_acus":1}"#)])
        .await
        .0;
    let nested_blank = run(
        &devin_script(&stub),
        &stub,
        r#"{"devin":{"org_id":"  "},"org_id":"top"}"#,
        "cog-1",
    )
    .await;
    assert!(!nested_blank.success);
    assert!(nested_blank.error.unwrap().contains("missing org_id"));

    // HTTP 错误带 body，锁前缀
    let (stub2, _) = spawn_capture(vec![("/v3/organizations/", 500, r#"{"e":"boom"}"#)]).await;
    let q2 = run(&devin_script(&stub2), &stub2, r#"{"org_id":"o"}"#, "cog-1").await;
    assert!(!q2.success);
    assert!(q2.error.unwrap().starts_with("HTTP 500"));
}

// ── 专属命令入口仍是脚本路径（无 dispatch 通用 key 检查）──

#[tokio::test]
async fn specialized_entries_delegate_to_scripts() {
    // query_quota_newapi / query_quota_devin 签名不变，内部走统一脚本路径：
    // 空 key 时返回脚本自带文案（而非 dispatch 的 "API key is empty"）
    let q = crate::newapi::quota::query_quota_newapi(None, "https://x.com/v1", " ", "{}", 0).await;
    assert_eq!(
        q.error.as_deref(),
        Some("New API: api_key required for token usage query")
    );
    let q2 = crate::devin::quota::query_quota_devin(None, "https://x", " ", "{}", 0).await;
    assert_eq!(q2.error.as_deref(), Some("Devin: api_key required"));
}
