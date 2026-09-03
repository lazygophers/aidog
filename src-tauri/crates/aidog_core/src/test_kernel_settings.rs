//! 内核管理面设置验收（票 08，2026-09-03 审查后收窄）：
//! 管理面永远只绑 127.0.0.1，设置里没有任何能改这件事的字段；凭据仍可配。
use super::*;
use aidog_db::test_support::test_db;

/// 新装（DB 无 `kernel/settings` 记录）读出来是默认端口 + 无凭据。
#[tokio::test]
async fn fresh_install_defaults() {
    let db = test_db().await;
    let s = load_kernel_settings(&db).await;
    assert_eq!(s.port, 9891, "默认端口 9891，与代理的 9890 分开");
    assert!(!s.has_auth(), "新装不应凭空带凭据");
}

/// 老库里残留的 `bind_lan` 键必须被忽略，而不是让整份设置解析失败退回默认
/// （退回默认会把用户配好的端口/凭据吃掉）。
#[test]
fn legacy_bind_lan_key_is_ignored_not_fatal() {
    let s: KernelSettings =
        serde_json::from_str(r#"{"port":9999,"bind_lan":true,"auth_token":"secret"}"#).unwrap();
    assert_eq!(s.port, 9999);
    assert_eq!(s.auth_token, "secret");
}

/// 序列化后的设置里**不得**再出现绑定地址字段：管理面绑哪已不是配置项。
#[test]
fn serialized_settings_have_no_bind_field() {
    let v = serde_json::to_value(KernelSettings::default()).unwrap();
    assert!(
        v.get("bind_lan").is_none(),
        "管理面永远 127.0.0.1，不得再有 bind_lan 字段"
    );
}

/// 纯空白凭据不算「已配置」。
#[test]
fn whitespace_only_credential_does_not_count() {
    let s = KernelSettings {
        port: 9891,
        auth_token: "   ".into(),
    };
    assert!(!s.has_auth());
}

/// 端口与凭据能落库并读回。
#[tokio::test]
async fn settings_round_trip() {
    let db = test_db().await;
    let s = KernelSettings {
        port: 9891,
        auth_token: "secret".into(),
    };
    save_kernel_settings(&db, &s).await.unwrap();
    assert_eq!(load_kernel_settings(&db).await, s);
}

/// 内核设置与代理设置各存各的 key，互不覆盖：改内核侧后代理的 `bind_lan` 原样不动。
#[tokio::test]
async fn kernel_settings_are_independent_from_proxy_settings() {
    let db = test_db().await;

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

    save_kernel_settings(
        &db,
        &KernelSettings {
            port: 9891,
            auth_token: "secret".into(),
        },
    )
    .await
    .unwrap();

    let p = crate::shared::load_proxy_settings(&db).await.unwrap();
    assert!(p.bind_lan, "代理侧的既有设置不得被内核设置改动");
    assert_eq!(p.port, 9890);

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
