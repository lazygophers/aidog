//! 余额族等价测试（quota-scripts T4）：7 平台 registry 脚本 + 本地 axum stub，
//! 逐平台断言 PlatformQuota 字段级一致（脚本正文来自 bundled registry，脚本内固定
//! host 常量替换为 stub URL 实现离线 mock）。
//! 错误文案差异（t3b-handoff）：HTTP 错误带 body（只锁 `HTTP ` 前缀 + status 码）、
//! Parse 锁前缀、Network 锁前缀。
use super::http::make_quota_log_for_script;
use super::script::{CustomQueryCtx, run_custom_query};
use super::test_stub::spawn_stub;
use aidog_db::registry;

/// bundled registry 某协议首条变体脚本正文。
fn script_for(code: &str) -> String {
    registry::quota_scripts_in(registry::presets(), code)
        .into_iter()
        .next()
        .map(|v| v.script)
        .unwrap_or_else(|| panic!("registry {code} 无 quota 脚本"))
}

/// 把脚本内固定 host 常量改指 stub，实现离线 mock（不改脚本逻辑）。
fn retarget(script: &str, host: &str, stub: &str) -> String {
    let out = script.replace(host, stub);
    assert!(out != *script, "脚本未含 host 常量 {host}，retarget 失效");
    out
}

async fn run(script: &str, base_url: &str, extra: &str) -> crate::quota::PlatformQuota {
    run_custom_query(
        None,
        CustomQueryCtx {
            base_url: base_url.to_string(),
            api_key: "sk-test".into(),
            extra: extra.into(),
        },
        script,
        0,
    )
    .await
}

#[tokio::test]
async fn quota_script_log_has_non_empty_id() {
    let log = make_quota_log_for_script("https://example.com/quota", 200, "{}");
    assert!(!log.id.is_empty());
    assert_eq!(log.group_key, "[quota:script]");
    assert_eq!(log.source_protocol, "quota");
}

// ── deepseek ──────────────────────────────────────────────
#[tokio::test]
async fn deepseek_sums_balance_infos() {
    let stub = spawn_stub(
        200,
        r#"{"is_available":true,"balance_infos":[{"total_balance":"10.5","currency":"CNY"},{"total_balance":4.5,"currency":"CNY"}]}"#,
    )
    .await;
    let s = retarget(&script_for("deepseek"), "https://api.deepseek.com", &stub);
    let q = run(&s, &stub, "").await;
    assert!(q.success, "{:?}", q.error);
    let b = q.balance.unwrap();
    assert!((b.remaining - 15.0).abs() < 1e-9);
    assert_eq!(b.total, None);
    assert_eq!(b.used, None);
    assert_eq!(b.currency, "CNY");
    assert!(b.is_valid);
}

#[tokio::test]
async fn deepseek_unavailable_and_missing_fields() {
    let stub = spawn_stub(200, r#"{"is_available":false}"#).await;
    let s = retarget(&script_for("deepseek"), "https://api.deepseek.com", &stub);
    let q = run(&s, &stub, "").await;
    assert!(q.success);
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 0.0);
    assert!(!b.is_valid);

    // 字段全缺 → 求和 0，is_available 缺省 true
    let stub2 = spawn_stub(200, "{}").await;
    let s2 = retarget(&script_for("deepseek"), "https://api.deepseek.com", &stub2);
    let q2 = run(&s2, &stub2, "").await;
    let b2 = q2.balance.unwrap();
    assert_eq!(b2.remaining, 0.0);
    assert!(b2.is_valid);
}

#[tokio::test]
async fn deepseek_http_parse_network_errors() {
    let code = script_for("deepseek");
    // HTTP：文案含 body 且 status 为裸数字，只锁前缀（t3b 差异 1）
    let stub = spawn_stub(401, r#"{"message":"bad key"}"#).await;
    let q = run(
        &retarget(&code, "https://api.deepseek.com", &stub),
        &stub,
        "",
    )
    .await;
    assert!(!q.success);
    let e = q.error.unwrap();
    assert!(e.starts_with("HTTP 401"), "实际: {e}");
    assert!(e.contains("bad key"), "带 body: {e}");

    // Parse：serde 细节串可能不同，锁前缀
    let stub2 = spawn_stub(200, "not json at all").await;
    let q2 = run(
        &retarget(&code, "https://api.deepseek.com", &stub2),
        &stub2,
        "",
    )
    .await;
    assert!(!q2.success);
    assert!(q2.error.unwrap().starts_with("Parse: "));

    // Network：未监听端口直连（NO_PROXY 逗号分隔，reqwest 只认逗号）
    unsafe { std::env::set_var("NO_PROXY", "127.0.0.1,localhost,::1") };
    let q3 = run(
        &retarget(&code, "https://api.deepseek.com", "http://127.0.0.1:1"),
        "http://127.0.0.1:1",
        "",
    )
    .await;
    assert!(!q3.success);
    assert!(q3.error.unwrap().starts_with("Network: "));
}

// ── stepfun / stepfun_en（同族正文拷贝，两份都跑）──────────

#[tokio::test]
async fn stepfun_reads_balance_cn_and_en() {
    for code in ["stepfun", "stepfun_en"] {
        let stub = spawn_stub(200, r#"{"balance":88.0}"#).await;
        let s = retarget(&script_for(code), "https://api.stepfun.com", &stub);
        let q = run(&s, &stub, "").await;
        assert!(q.success, "{code}: {:?}", q.error);
        let b = q.balance.unwrap();
        assert_eq!(b.remaining, 88.0);
        assert_eq!(b.currency, "CNY");
        assert!(b.is_valid);

        // 缺字段 → 0
        let stub2 = spawn_stub(200, "{}").await;
        let s2 = retarget(&script_for(code), "https://api.stepfun.com", &stub2);
        let q2 = run(&s2, &stub2, "").await;
        assert_eq!(q2.balance.unwrap().remaining, 0.0);
    }
}

// ── siliconflow / siliconflow_en ──────────────────────────

#[tokio::test]
async fn siliconflow_cn_cny_and_en_usd() {
    let body = r#"{"data":{"totalBalance":12.34}}"#;
    for (code, host, currency) in [
        ("siliconflow", "https://api.siliconflow.cn", "CNY"),
        ("siliconflow_en", "https://api.siliconflow.com", "USD"),
    ] {
        let stub = spawn_stub(200, body).await;
        let s = retarget(&script_for(code), host, &stub);
        let q = run(&s, &stub, "").await;
        assert!(q.success, "{code}: {:?}", q.error);
        let b = q.balance.unwrap();
        assert!((b.remaining - 12.34).abs() < 1e-9);
        assert_eq!(b.currency, currency);
    }
}

#[tokio::test]
async fn siliconflow_missing_data_errors_null_data_ok() {
    let code = script_for("siliconflow");
    let stub = spawn_stub(200, r#"{"code":20000}"#).await;
    let q = run(
        &retarget(&code, "https://api.siliconflow.cn", &stub),
        &stub,
        "",
    )
    .await;
    assert!(!q.success);
    assert_eq!(q.error.unwrap(), "Missing data field");

    // data:null → 走缺省 0，不报错（Rust Value::Null 语义）
    let stub2 = spawn_stub(200, r#"{"data":null}"#).await;
    let q2 = run(
        &retarget(&code, "https://api.siliconflow.cn", &stub2),
        &stub2,
        "",
    )
    .await;
    assert!(q2.success, "{:?}", q2.error);
    assert_eq!(q2.balance.unwrap().remaining, 0.0);
}

// ── openrouter ────────────────────────────────────────────

#[tokio::test]
async fn openrouter_nested_flat_and_negative() {
    let code = script_for("openrouter");
    // nested data 形态
    let stub = spawn_stub(
        200,
        r#"{"data":{"total_credits":100.0,"total_usage":30.0}}"#,
    )
    .await;
    let q = run(&retarget(&code, "https://openrouter.ai", &stub), &stub, "").await;
    assert!(q.success, "{:?}", q.error);
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 70.0);
    assert_eq!(b.total, Some(100.0));
    assert_eq!(b.used, Some(30.0));
    assert_eq!(b.currency, "USD");
    assert!(b.is_valid);

    // flat body 兼容
    let stub2 = spawn_stub(200, r#"{"total_credits":5.0,"total_usage":7.0}"#).await;
    let q2 = run(
        &retarget(&code, "https://openrouter.ai", &stub2),
        &stub2,
        "",
    )
    .await;
    let b2 = q2.balance.unwrap();
    assert_eq!(b2.remaining, -2.0);
    assert!(!b2.is_valid, "负 remaining → invalid");
}

// ── novita ────────────────────────────────────────────────

#[tokio::test]
async fn novita_scales_by_1e4_and_zero_invalid() {
    let code = script_for("novita");
    let stub = spawn_stub(200, r#"{"availableBalance":123400}"#).await;
    let q = run(&retarget(&code, "https://api.novita.ai", &stub), &stub, "").await;
    assert!(q.success, "{:?}", q.error);
    let b = q.balance.unwrap();
    assert!((b.remaining - 12.34).abs() < 1e-9);
    assert_eq!(b.currency, "USD");
    assert!(b.is_valid);

    let stub2 = spawn_stub(200, r#"{"availableBalance":0}"#).await;
    let q2 = run(
        &retarget(&code, "https://api.novita.ai", &stub2),
        &stub2,
        "",
    )
    .await;
    let b2 = q2.balance.unwrap();
    assert_eq!(b2.remaining, 0.0);
    assert!(!b2.is_valid);
}
