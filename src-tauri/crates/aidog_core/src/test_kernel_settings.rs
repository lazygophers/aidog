//! 票 08 验收：内核绑定开关（默认关 / 开启需凭据 / 与代理 `bind_lan` 互不影响）。
use super::*;
use aidog_db::test_support::test_db;

/// 验收「内核绑定开关默认关，关时只在 127.0.0.1 可达」的前半段：新装（DB 无
/// `kernel/settings` 记录）读出来必须是关，且解析成的监听 IP 是回环。
#[tokio::test]
async fn fresh_install_defaults_to_loopback() {
    let db = test_db().await;
    let s = load_kernel_settings(&db).await;
    assert!(!s.bind_lan, "内核绑定开关必须默认关");
    assert_eq!(
        s.bind_ip(),
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        "关的时候必须只绑 127.0.0.1"
    );
    assert!(!s.has_auth(), "新装不应凭空带凭据");
}

/// 字段缺失（旧记录 / 手写 JSON）同样走默认关，而不是被 serde 当成 true。
#[test]
fn missing_field_defaults_to_off() {
    let s: KernelSettings = serde_json::from_str(r#"{"port":9891}"#).unwrap();
    assert!(!s.bind_lan);
    assert!(!s.has_auth());
}

/// 开启后监听 0.0.0.0（开关语义本身）。
#[test]
fn enabled_binds_all_interfaces() {
    let s = KernelSettings {
        port: 9891,
        bind_lan: true,
        auth_token: "secret".into(),
    };
    assert_eq!(
        s.bind_ip(),
        std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
    );
}

/// 验收「未配置鉴权凭据时开启开关被拒绝并给出原因」：
/// 命令返回 Err(原因 key)，且 DB 里的值**没有**被改成 true。
#[tokio::test]
async fn enabling_without_credentials_is_refused_and_not_persisted() {
    let db = test_db().await;
    let mut s = load_kernel_settings(&db).await;
    s.bind_lan = true;

    let err = save_kernel_settings(&db, &s)
        .await
        .expect_err("未配凭据时开启必须被拒绝");
    assert_eq!(
        err, "kernel.bindLanRequiresAuth",
        "必须给出可翻译的拒绝原因，而不是静默失败"
    );

    let after = load_kernel_settings(&db).await;
    assert!(!after.bind_lan, "被拒绝的开启不得落库");
}

/// 纯空白凭据不算「已配置」（否则一个空格就能绕过硬前提）。
#[tokio::test]
async fn whitespace_only_credential_does_not_count() {
    let db = test_db().await;
    let s = KernelSettings {
        port: 9891,
        bind_lan: true,
        auth_token: "   ".into(),
    };
    assert!(save_kernel_settings(&db, &s).await.is_err());
}

/// 配了凭据后允许开启，并且能读回。
#[tokio::test]
async fn enabling_with_credentials_persists() {
    let db = test_db().await;
    let s = KernelSettings {
        port: 9891,
        bind_lan: true,
        auth_token: "secret".into(),
    };
    save_kernel_settings(&db, &s).await.unwrap();
    let after = load_kernel_settings(&db).await;
    assert_eq!(after, s);
}

/// 验收「开关不读取也不影响代理的 `bind_lan`」：两侧各自落在不同 setting key 上，
/// 改内核那侧后代理侧原样不动，反之亦然。
#[tokio::test]
async fn kernel_switch_is_independent_from_proxy_bind_lan() {
    let db = test_db().await;

    // 代理侧：用户既有的局域网转发开着。
    crate::shared::save_proxy_settings_to_db(
        &db,
        &crate::shared::ProxySettings {
            port: 9890,
            autostart: true,
            silent_launch: false,
            bind_lan: true,
        },
    )
    .await
    .unwrap();

    // 内核侧仍必须是关的 —— 不得从代理那侧继承 true。
    let k = load_kernel_settings(&db).await;
    assert!(!k.bind_lan, "内核开关不得读取代理的 bind_lan");

    // 反向：开内核开关（带凭据）后，代理侧的值不变。
    save_kernel_settings(
        &db,
        &KernelSettings {
            port: 9891,
            bind_lan: true,
            auth_token: "secret".into(),
        },
    )
    .await
    .unwrap();
    let p = crate::shared::load_proxy_settings(&db).await.unwrap();
    assert!(p.bind_lan, "代理侧的既有设置不得被内核开关改动");
    assert_eq!(p.port, 9890);

    // 两把 key 各存各的，互不覆盖。
    assert!(
        aidog_db::get_setting(&db, "kernel", "settings")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        aidog_db::get_setting(&db, "proxy", "settings")
            .await
            .unwrap()
            .is_some()
    );
}
