//! settings.rs 模型单测：平台熔断阈值覆盖解析 + SchedulingBreakerSettings 有效阈值
//! （原 models.rs `middleware_model_tests` 中的 breaker 相关用例）。
//!
//! 引用的 `parse_breaker` / `merge_breaker_into_extra` / `Platform` 等经 models::* 重导出。

use super::super::{
    merge_breaker_into_extra, parse_breaker, Platform, PlatformBreaker, PlatformModels, PlatformStatus, Protocol,
    ProxyClientSettings, SchedulingBreakerSettings,
};

/// 最小 Platform，仅设 extra 用于 breaker 解析测试。
fn platform_with_extra(extra: &str) -> Platform {
    Platform {
        id: 1,
        name: "p".into(),
        platform_type: Protocol::Anthropic,
        base_url: String::new(),
        api_key: String::new(),
        extra: extra.into(),
        models: PlatformModels::default(),
        available_models: vec![],
        endpoints: vec![],
        enabled: true,
        status: PlatformStatus::Enabled,
        auto_disabled_until: 0,
        auto_disable_strikes: 0,
        created_at: 0,
        updated_at: 0,
        deleted_at: 0,
        est_balance_remaining: 0.0,
        est_coding_plan: String::new(),
        last_real_query_at: 0,
        estimate_count: 0,
        show_in_tray: false,
        tray_display: String::new(),
        sort_order: 0,
        manual_budgets: vec![],
        balance_level: String::new(),
    }
}

#[test]
fn parse_merge_breaker_roundtrip() {
    // 空 / 非法 / 无 breaker 键 → 全 0。
    assert_eq!(parse_breaker("").failure_threshold, 0);
    assert_eq!(parse_breaker("not json").open_secs, 0);
    assert_eq!(parse_breaker(r#"{"mock":{}}"#).half_open_max, 0);

    // merge 写入 → 再解析一致，且保留 extra 其余键。
    let merged = merge_breaker_into_extra(
        r#"{"mock":{"x":1}}"#,
        &PlatformBreaker { failure_threshold: 4, open_secs: 90, half_open_max: 2 },
    );
    let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
    assert_eq!(v["mock"]["x"], 1, "保留 extra 其余键");
    let b = parse_breaker(&merged);
    assert_eq!((b.failure_threshold, b.open_secs, b.half_open_max), (4, 90, 2));

    // 全 0 → 移除 breaker 键（无覆盖=继承全局）。
    let cleared = merge_breaker_into_extra(&merged, &PlatformBreaker::default());
    let v2: serde_json::Value = serde_json::from_str(&cleared).unwrap();
    assert!(v2.get("breaker").is_none(), "全 0 移除 breaker 键");
    assert_eq!(v2["mock"]["x"], 1, "清 breaker 不动其余键");
}

#[test]
fn effective_thresholds_extra_override_and_inherit() {
    let global = SchedulingBreakerSettings::default(); // (5, 60, 2)

    // 缺 extra.breaker → 全继承全局默认。
    let p_none = platform_with_extra("{}");
    assert_eq!(global.effective_thresholds(&p_none), (5, 60, 2));

    // extra.breaker 全覆盖。
    let p_all = platform_with_extra(&merge_breaker_into_extra(
        "{}",
        &PlatformBreaker { failure_threshold: 9, open_secs: 120, half_open_max: 4 },
    ));
    assert_eq!(global.effective_thresholds(&p_all), (9, 120, 4));

    // 单键覆盖（failure_threshold），其余继承全局；open_secs/half_open_max=0 → 用全局。
    let p_partial = platform_with_extra(&merge_breaker_into_extra(
        "{}",
        &PlatformBreaker { failure_threshold: 8, open_secs: 0, half_open_max: 0 },
    ));
    assert_eq!(global.effective_thresholds(&p_partial), (8, 60, 2));
}

// ── ProxyClientSettings::to_reqwest_proxy ──

#[test]
fn to_reqwest_proxy_disabled_returns_none() {
    let s = ProxyClientSettings {
        enabled: false,
        proxy_type: "socks5".into(),
        host: "127.0.0.1".into(),
        port: 7890,
        username: "".into(),
        password: "".into(),
        dns_over_proxy: false,
    };
    assert!(s.to_reqwest_proxy().is_none());
}

#[test]
fn to_reqwest_proxy_enabled_socks5_returns_some() {
    let s = ProxyClientSettings {
        enabled: true,
        proxy_type: "socks5".into(),
        host: "127.0.0.1".into(),
        port: 7890,
        username: "".into(),
        password: "".into(),
        dns_over_proxy: false,
    };
    assert!(s.to_reqwest_proxy().is_some());
}

#[test]
fn to_reqwest_proxy_socks5h_dns_over_proxy() {
    let s = ProxyClientSettings {
        enabled: true,
        proxy_type: "socks5".into(),
        host: "127.0.0.1".into(),
        port: 7890,
        username: "".into(),
        password: "".into(),
        dns_over_proxy: true,
    };
    assert!(s.to_reqwest_proxy().is_some());
}

#[test]
fn to_reqwest_proxy_http_type() {
    let s = ProxyClientSettings {
        enabled: true,
        proxy_type: "http".into(),
        host: "127.0.0.1".into(),
        port: 8080,
        username: "".into(),
        password: "".into(),
        dns_over_proxy: false,
    };
    assert!(s.to_reqwest_proxy().is_some());
}

#[test]
fn to_reqwest_proxy_https_type() {
    let s = ProxyClientSettings {
        enabled: true,
        proxy_type: "https".into(),
        host: "127.0.0.1".into(),
        port: 8080,
        username: "user".into(),
        password: "pass".into(),
        dns_over_proxy: false,
    };
    assert!(s.to_reqwest_proxy().is_some());
}

#[test]
fn to_reqwest_proxy_with_auth() {
    let s = ProxyClientSettings {
        enabled: true,
        proxy_type: "socks5".into(),
        host: "127.0.0.1".into(),
        port: 7890,
        username: "alice".into(),
        password: "secret".into(),
        dns_over_proxy: false,
    };
    assert!(s.to_reqwest_proxy().is_some());
}
