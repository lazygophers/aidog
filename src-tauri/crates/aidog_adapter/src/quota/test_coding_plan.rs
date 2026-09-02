//! Coding plan 族等价测试（quota-scripts T4）：glm / kimi / minimax 系 registry 脚本 +
//! 本地 axum stub，逐平台断言 PlatformQuota 字段级一致。ISO 毫秒格式以 t3a 实测样例为准：
//! 1750000000000 → `2025-06-15T15:06:40+00:00`，1750000000123 → `2025-06-15T15:06:40.123+00:00`。
//! 同族协议正文拷贝（glm 4 份 / kimi 2 份 / minimax cn 2 份），各代码点独立跑防漂移。
use super::script::{CustomQueryCtx, run_custom_query};
use super::test_stub::{spawn_capture, spawn_stub};
use aidog_db::registry;

fn script_for(code: &str) -> String {
    registry::quota_scripts_in(registry::presets(), code)
        .into_iter()
        .next()
        .map(|v| v.script)
        .unwrap_or_else(|| panic!("registry {code} 无 quota 脚本"))
}

fn retarget(script: &str, host: &str, stub: &str) -> String {
    let out = script.replace(host, stub);
    assert!(out != *script, "脚本未含 host 常量 {host}，retarget 失效");
    out
}

async fn run(script: &str, base_url: &str) -> crate::quota::PlatformQuota {
    run_custom_query(
        None,
        CustomQueryCtx {
            base_url: base_url.to_string(),
            api_key: "sk-test".into(),
            extra: String::new(),
        },
        script,
        0,
    )
    .await
}

/// glm 族脚本（open.bigmodel.cn / api.z.ai 两个 host 常量都改指 stub）。
async fn glm_stub(code: &str, status: u16, body: &'static str) -> crate::quota::PlatformQuota {
    let stub = spawn_stub(status, body).await;
    let s = retarget(&script_for(code), "https://open.bigmodel.cn", &stub);
    let s = retarget(&s, "https://api.z.ai", &stub);
    run(&s, &stub).await
}

// ── glm（zhipu）────────────────────────────────────────────

#[tokio::test]
async fn glm_all_family_codes_classify_units_and_level() {
    for code in ["glm", "glm_en", "glm_coding", "glm_coding_en"] {
        let q = glm_stub(
            code,
            200,
            r#"{"success":true,"data":{"level":"Max","limits":[
                {"type":"TOKENS_LIMIT","unit":3,"percentage":42.0,"nextResetTime":1750000000000},
                {"type":"TOKENS_LIMIT","unit":6,"percentage":80.0,"nextResetTime":1750000000123},
                {"type":"TIME_LIMIT","percentage":10.0,"usage":"5","remaining":"2","nextResetTime":1750000000000}
            ]}}"#,
        )
        .await;
        assert!(q.success, "{code}: {:?}", q.error);
        let cp = q.coding_plan.as_ref().unwrap();
        assert_eq!(cp.level.as_deref(), Some("Max"));
        let t = &cp.tiers;
        assert_eq!(t.len(), 3, "{code}");
        // 输出固定顺序 five_hour, weekly_limit, mcp_monthly
        assert_eq!(t[0].name, "five_hour");
        assert!((t[0].utilization - 42.0).abs() < 1e-9);
        assert_eq!(t[0].resets_at.as_deref(), Some("2025-06-15T15:06:40+00:00"));
        assert!(
            t[0].limit.is_none() && t[0].remaining.is_none(),
            "five_hour 恒不带绝对量"
        );
        assert_eq!(t[1].name, "weekly_limit");
        assert_eq!(
            t[1].resets_at.as_deref(),
            Some("2025-06-15T15:06:40.123+00:00")
        );
        assert_eq!(t[2].name, "mcp_monthly");
        assert!((t[2].utilization - 10.0).abs() < 1e-9);
        // mcp 绝对量 parse_f64 字符串双兼容
        assert_eq!(t[2].limit, Some(5.0));
        assert_eq!(t[2].remaining, Some(2.0));
    }
}

#[tokio::test]
async fn glm_unclassified_fill_and_mcp_zero_percent() {
    // unit 3 占 five_hour；无 unit 6 → unclassified（None-first 再升序）填 weekly
    let q = glm_stub(
        "glm",
        200,
        r#"{"data":{"level":"pro","limits":[
            {"type":"TOKENS_LIMIT","unit":3,"percentage":10.0,"nextResetTime":100},
            {"type":"TOKENS_LIMIT","unit":9,"percentage":11.0},
            {"type":"TOKENS_LIMIT","unit":4,"percentage":12.0,"nextResetTime":50},
            {"type":"TIME_LIMIT","percentage":0.0,"usage":0,"remaining":0}
        ]}}"#,
    )
    .await;
    assert!(q.success, "{:?}", q.error);
    let t = &q.coding_plan.unwrap().tiers;
    assert_eq!(t.len(), 3);
    assert_eq!(t[0].name, "five_hour");
    assert_eq!(
        t[0].resets_at.as_deref(),
        Some("1970-01-01T00:00:00.100+00:00")
    );
    assert_eq!(t[1].name, "weekly_limit");
    // None-first：unit=9 无 reset 排前占 weekly
    assert_eq!(t[1].utilization, 11.0);
    assert_eq!(t[2].name, "mcp_monthly");
    assert_eq!(t[2].utilization, 0.0);
    assert!(
        t[2].limit.is_none() && t[2].remaining.is_none(),
        "≤0 绝对量不带"
    );
}

#[tokio::test]
async fn glm_error_paths() {
    // success=false → error = msg
    let q = glm_stub("glm", 200, r#"{"success":false,"msg":"quota denied"}"#).await;
    assert!(!q.success);
    assert_eq!(q.error.as_deref(), Some("quota denied"));
    // msg 缺省 Unknown
    let q2 = glm_stub("glm", 200, r#"{"success":false}"#).await;
    assert_eq!(q2.error.as_deref(), Some("Unknown"));
    // data 键缺失
    let q3 = glm_stub("glm", 200, r#"{"success":true}"#).await;
    assert_eq!(q3.error.as_deref(), Some("Missing data field"));
    // data:null / 非对象 → 空 tiers 仍 success（serde Null 语义）
    let q4 = glm_stub("glm", 200, r#"{"success":true,"data":null}"#).await;
    assert!(q4.success, "{:?}", q4.error);
    assert!(q4.coding_plan.unwrap().tiers.is_empty());
    // body 为 JSON null → 缺 data 键 → Missing data field
    let q5 = glm_stub("glm", 200, "null").await;
    assert_eq!(q5.error.as_deref(), Some("Missing data field"));
}

#[tokio::test]
async fn glm_bare_authorization_header() {
    // glm 不加 Bearer 前缀（Rust 原实现裸 Authorization），capture stub 断言
    let (stub, log) = spawn_capture(vec![(
        "/api/monitor/usage/quota/limit",
        200,
        r#"{"data":{"level":null,"limits":[]}}"#,
    )])
    .await;
    let s = retarget(&script_for("glm"), "https://open.bigmodel.cn", &stub);
    let s = retarget(&s, "https://api.z.ai", &stub);
    let q = run(&s, &stub).await;
    assert!(q.success, "{:?}", q.error);
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].path, "/api/monitor/usage/quota/limit");
    assert_eq!(
        log[0].authorization, "sk-test",
        "glm 裸 Authorization 不加 Bearer"
    );
}

// ── kimi / kimi_coding ────────────────────────────────────

async fn kimi_run(code: &str, body: &'static str) -> crate::quota::PlatformQuota {
    let stub = spawn_stub(200, body).await;
    let s = retarget(&script_for(code), "https://api.kimi.com", &stub);
    run(&s, &stub).await
}

#[tokio::test]
async fn kimi_five_hour_and_weekly_formulas() {
    for code in ["kimi", "kimi_coding"] {
        let q = kimi_run(
            code,
            r#"{"limits":[{"detail":{"limit":100,"remaining":40,"resetTime":1750000000000}},
                 {"detail":{"limit":0,"remaining":0,"resetTime":"2026-01-01T00:00:00Z"}}],
                "usage":{"limit":200,"remaining":50,"resetTime":null}}"#,
        )
        .await;
        assert!(q.success, "{code}: {:?}", q.error);
        let cp = q.coding_plan.as_ref().unwrap();
        assert_eq!(cp.level, None, "kimi level 恒 null");
        let t = &cp.tiers;
        assert_eq!(t.len(), 3, "{code}");
        let first = &t[0];
        assert_eq!(first.name, "five_hour");
        assert!((first.utilization - 60.0).abs() < 1e-9, "(100-40)/100");
        assert_eq!(first.limit, Some(100.0), "暴露绝对 limit");
        assert_eq!(first.remaining, Some(40.0));
        assert_eq!(
            first.resets_at.as_deref(),
            Some("2025-06-15T15:06:40+00:00")
        );
        // limit=0 → 除零保护 util=0；字符串 resetTime 原样透传
        let second = &t[1];
        assert_eq!(second.utilization, 0.0);
        assert_eq!(second.resets_at.as_deref(), Some("2026-01-01T00:00:00Z"));
        let weekly = &t[2];
        assert_eq!(weekly.name, "weekly_limit");
        assert!((weekly.utilization - 75.0).abs() < 1e-9);
        assert_eq!(weekly.resets_at, None);
        assert_eq!(weekly.limit, Some(200.0));
    }
}

#[tokio::test]
async fn kimi_default_bucket_and_null_usage() {
    // detail:null → 缺省桶 limit=1.0 remaining=0.0（util=100，= Rust unwrap_or(1.0)）；
    // usage:null（键存在值 null）→ 默认桶 weekly
    let q = kimi_run("kimi", r#"{"limits":[{"detail":null}],"usage":null}"#).await;
    assert!(q.success, "{:?}", q.error);
    let t = &q.coding_plan.unwrap().tiers;
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].limit, Some(1.0));
    assert_eq!(t[0].remaining, Some(0.0));
    assert!(
        (t[0].utilization - 100.0).abs() < 1e-9,
        "缺省 1.0/0.0 → 已用满"
    );
    assert_eq!(t[1].name, "weekly_limit");
    assert_eq!(t[1].limit, Some(1.0));

    // usage 键缺失 → 只有 five_hour 桶
    let q2 = kimi_run("kimi", r#"{"limits":[]}"#).await;
    assert!(q2.coding_plan.unwrap().tiers.is_empty());
}

// ── minimax / minimax_coding / minimax_en ─────────────────

async fn minimax_run(code: &str, host: &str, body: &'static str) -> crate::quota::PlatformQuota {
    let stub = spawn_stub(200, body).await;
    let s = retarget(&script_for(code), host, &stub);
    run(&s, &stub).await
}

#[tokio::test]
async fn minimax_general_bucket_counts() {
    for (code, host) in [
        ("minimax", "https://api.minimaxi.com"),
        ("minimax_coding", "https://api.minimaxi.com"),
        ("minimax_en", "https://api.minimax.io"),
    ] {
        let q = minimax_run(
            code,
            host,
            r#"{"base_resp":{"status_code":0},
                "model_remains":[{"model_name":"general",
                    "current_interval_remaining_percent":70.5,"end_time":1750000000123,
                    "current_weekly_status":1,"current_weekly_remaining_percent":25.0,
                    "weekly_end_time":1750000000000,
                    "current_weekly_total_count":200,"current_weekly_usage_count":50}]}"#,
        )
        .await;
        assert!(q.success, "{code}: {:?}", q.error);
        let t = &q.coding_plan.as_ref().unwrap().tiers;
        assert_eq!(t.len(), 2, "{code}");
        assert_eq!(t[0].name, "five_hour");
        assert!((t[0].utilization - 29.5).abs() < 1e-9, "100-70.5");
        assert_eq!(
            t[0].resets_at.as_deref(),
            Some("2025-06-15T15:06:40.123+00:00")
        );
        assert_eq!(t[1].name, "weekly_limit");
        assert!((t[1].utilization - 75.0).abs() < 1e-9);
        assert_eq!(t[1].limit, Some(200.0), "次数型带绝对 limit");
        assert_eq!(t[1].remaining, Some(150.0));
    }
}

#[tokio::test]
async fn minimax_weekly_status_and_token_type() {
    // status=0 → 跳过 weekly（回归：旧行为不建桶）
    let q = minimax_run(
        "minimax",
        "https://api.minimaxi.com",
        r#"{"model_remains":[{"model_name":"general",
            "current_interval_remaining_percent":80.0,
            "current_weekly_status":0,"current_weekly_remaining_percent":10.0}]}"#,
    )
    .await;
    let t = &q.coding_plan.unwrap().tiers;
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].name, "five_hour");

    // status=2 → 已用满仍显示；token 型（无 counts）→ limit/remaining null
    let q2 = minimax_run(
        "minimax",
        "https://api.minimaxi.com",
        r#"{"model_remains":[{"model_name":"general",
            "current_interval_remaining_percent":80.0,
            "current_weekly_status":2,"current_weekly_remaining_percent":0.0}]}"#,
    )
    .await;
    let t2 = &q2.coding_plan.unwrap().tiers;
    assert_eq!(t2.len(), 2);
    assert!((t2[1].utilization - 100.0).abs() < 1e-9);
    assert!(t2[1].limit.is_none() && t2[1].remaining.is_none());
}

#[tokio::test]
async fn minimax_base_resp_errors_and_empty_remains() {
    // base_resp 非 0 → API error 文案
    let q = minimax_run(
        "minimax",
        "https://api.minimaxi.com",
        r#"{"base_resp":{"status_code":1001,"status_msg":"invalid key"}}"#,
    )
    .await;
    assert!(!q.success);
    assert_eq!(
        q.error.as_deref(),
        Some("API error (code 1001): invalid key")
    );

    // msg 缺省 Unknown
    let q2 = minimax_run(
        "minimax",
        "https://api.minimaxi.com",
        r#"{"base_resp":{"status_code":5}}"#,
    )
    .await;
    assert_eq!(q2.error.as_deref(), Some("API error (code 5): Unknown"));

    // 无 general / 空 model_remains → success 空 tiers 不 panic
    let q3 = minimax_run(
        "minimax",
        "https://api.minimaxi.com",
        r#"{"model_remains":[{"model_name":"abab"}]}"#,
    )
    .await;
    assert!(q3.success, "{:?}", q3.error);
    assert!(q3.coding_plan.unwrap().tiers.is_empty());

    let q4 = minimax_run(
        "minimax_en",
        "https://api.minimax.io",
        r#"{"model_remains":[]}"#,
    )
    .await;
    assert!(q4.success);
    assert!(q4.coding_plan.unwrap().tiers.is_empty());
}
