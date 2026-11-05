//! 管理接口设置验收（ADR 0010）：开关缺省开，凭据可选；设置里没有任何绑定地址 / 端口字段。
use super::*;
use aidog_db::test_support::test_db;

/// 新装（DB 无 `kernel/settings` 记录）读出来是开关开 + 无凭据。
#[tokio::test]
async fn fresh_install_defaults() {
    let db = test_db().await;
    let s = load_kernel_settings(&db).await;
    assert!(s.api_enabled, "管理接口缺省开");
    assert!(!s.has_auth(), "新装不应凭空带凭据");
}

/// 老库里残留的 `bind_lan` 键必须被忽略，而不是让整份设置解析失败退回默认
/// （退回默认会把用户配好的端口/凭据吃掉）。
#[test]
fn legacy_bind_lan_key_is_ignored_not_fatal() {
    let s: KernelSettings =
        serde_json::from_str(r#"{"port":9999,"bind_lan":true,"auth_token":"secret"}"#).unwrap();
    assert!(s.api_enabled, "旧库没有开关键 → 缺省开");
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
        api_enabled: false,
        auth_token: "   ".into(),
    };
    assert!(!s.has_auth());
}

/// 开关与凭据能落库并读回。
#[tokio::test]
async fn settings_round_trip() {
    let db = test_db().await;
    let s = KernelSettings {
        api_enabled: false,
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
            api_enabled: false,
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

// ── 测试面板发送（票 05）：真实回环 HTTP，假上游代替代理 ─────────────────────

type Seen = std::sync::Arc<std::sync::Mutex<Vec<(String, Option<String>, Option<String>, String)>>>;

/// 起本地假「代理」：记录 (路径, x-aidog-source, authorization, body)，回固定状态与正文。
async fn spawn_stub(status: u16, body: &'static str) -> (u16, Seen) {
    use axum::{extract::State, http::HeaderMap, http::Uri, routing::post, Router};
    let seen: Seen = Default::default();
    let app = Router::new()
        .fallback(post(
            move |State(s): State<Seen>, uri: Uri, h: HeaderMap, b: String| async move {
                let g = |n: &str| h.get(n).and_then(|v| v.to_str().ok()).map(String::from);
                s.lock().unwrap().push((uri.path().to_string(), g("x-aidog-source"), g("authorization"), b));
                (axum::http::StatusCode::from_u16(status).unwrap(), body)
            },
        ))
        .with_state(seen.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (port, seen)
}

/// 真实状态码与响应体原样返回（含 4xx），请求带来源头、POST 参数 JSON、路径 /api/rpc/<命令>。
#[tokio::test]
async fn send_returns_real_status_and_body_with_source_header() {
    let (port, seen) = spawn_stub(403, r#"{"error":"forbidden"}"#).await;
    let r = send_management_request(port, "", "backup_export", r#"{"a":1}"#).await.unwrap();
    assert_eq!(r.status, 403);
    assert_eq!(r.body, r#"{"error":"forbidden"}"#);
    let s = seen.lock().unwrap();
    assert_eq!(s[0].0, "/api/rpc/backup_export");
    assert_eq!(s[0].1.as_deref(), Some("settings-test"));
    assert_eq!(s[0].2, None, "凭据空 = 不带 Authorization");
    assert_eq!(s[0].3, r#"{"a":1}"#);
}

/// 凭据非空 → Bearer；参数空白 → 发 `{}`。
#[tokio::test]
async fn send_attaches_bearer_and_defaults_empty_params() {
    let (port, seen) = spawn_stub(200, "[]").await;
    send_management_request(port, "tok", "platform_list", "  ").await.unwrap();
    let s = seen.lock().unwrap();
    assert_eq!(s[0].2.as_deref(), Some("Bearer tok"));
    assert_eq!(s[0].3, "{}");
}

/// 代理没跑（端口无监听）→ 固定错误码 proxy_unreachable，供前端提示。
#[tokio::test]
async fn send_reports_proxy_unreachable() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    let e = send_management_request(port, "", "platform_list", "").await.unwrap_err();
    assert_eq!(e, "proxy_unreachable");
}

/// 命令名只允许标识符字符，防路径注入（../、斜杠、查询串）。
#[tokio::test]
async fn send_rejects_bad_command_name() {
    let e = send_management_request(1, "", "../x?y", "").await.unwrap_err();
    assert_eq!(e, "invalid_command");
}
