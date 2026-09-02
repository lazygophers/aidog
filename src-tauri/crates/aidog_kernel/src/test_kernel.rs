//! 票 08 验收测试。
//!
//! 接缝按 SPEC「Testing Decisions / seam 二」的现成形态：起真实 axum router、发真实请求、
//! 断言状态码与响应体。不断言私有函数被调了几次。

use super::*;
use aidog_core::kernel_settings::KernelSettings;
use std::net::{IpAddr, Ipv4Addr};

// ─── 命令行形态 ────────────────────────────────────────────────────────────

#[test]
fn no_args_means_pure_kernel() {
    let o = match parse_args(Vec::<String>::new()) {
        ParseOutcome::Run(o) => o,
        _ => panic!("无参数必须是正常启动"),
    };
    assert!(!o.ui, "不带参数 = 纯内核，不开管理面");
}

#[test]
fn ui_flag_enables_management() {
    let o = match parse_args(vec!["--ui".to_string()]) {
        ParseOutcome::Run(o) => o,
        _ => panic!("--ui 必须是正常启动"),
    };
    assert!(o.ui);
}

#[test]
fn unknown_argument_is_an_error_not_a_silent_default() {
    assert!(matches!(
        parse_args(vec!["--wat".to_string()]),
        ParseOutcome::Error(_)
    ));
}

// ─── 验收：纯内核形态下无任何 HTTP 管理面在听 ──────────────────────────────

/// `management_bind_addr` 是全进程唯一决定「开不开管理面监听」的地方（`run` 里只有这一处
/// 调 `serve_management`）。不带 `--ui` 时它返回 `None`，即一个 socket 都不绑 ——
/// 且这与内核设置里怎么配无关（哪怕开关开着、凭据也配了）。
#[test]
fn pure_kernel_never_binds_a_management_socket() {
    let opts = Options {
        ui: false,
        ui_dir: None,
    };
    let wide_open = KernelSettings {
        port: 9891,
        bind_lan: true,
        auth_token: "secret".into(),
    };
    assert_eq!(management_bind_addr(&opts, &KernelSettings::default()), None);
    assert_eq!(
        management_bind_addr(&opts, &wide_open),
        None,
        "纯内核形态下即使设置里开着绑定开关也不得开管理面"
    );
}

// ─── 验收：绑定开关默认关 / 开启需凭据 / 与代理 bind_lan 无关 ─────────────

/// 默认（开关关）时只绑 127.0.0.1。
#[test]
fn switch_off_binds_loopback_only() {
    let opts = Options {
        ui: true,
        ui_dir: None,
    };
    let addr = management_bind_addr(&opts, &KernelSettings::default()).unwrap();
    assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(addr.port(), 9891, "默认端口 9891，与代理的 9890 分开");
}

/// 开关开 + 已配凭据 → 0.0.0.0。
#[test]
fn switch_on_with_credentials_binds_all_interfaces() {
    let opts = Options {
        ui: true,
        ui_dir: None,
    };
    let s = KernelSettings {
        port: 9891,
        bind_lan: true,
        auth_token: "secret".into(),
    };
    assert_eq!(
        management_bind_addr(&opts, &s).unwrap().ip(),
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    );
}

/// 开关开但没凭据（库被手改 / 从别的机器拷来）→ 拒绝开放，降级回环。
#[test]
fn switch_on_without_credentials_refuses_to_expose() {
    let opts = Options {
        ui: true,
        ui_dir: None,
    };
    let s = KernelSettings {
        port: 9891,
        bind_lan: true,
        auth_token: String::new(),
    };
    assert_eq!(
        management_bind_addr(&opts, &s).unwrap().ip(),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        "未配凭据时绝不能监听 0.0.0.0"
    );
}

// ─── 路由表 ────────────────────────────────────────────────────────────────

/// `/rpc` 表与注册表同规模且无重名（重名会让 axum 直接 panic，这里提前断言）。
#[test]
fn rpc_table_has_no_duplicates() {
    let mut sorted: Vec<&str> = rpc::RPC_COMMAND_NAMES.to_vec();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(before, sorted.len(), "/rpc 路由表里有重复命令名");
    assert!(
        before > 200,
        "路由表规模异常（{before}），可能是宏没展开全"
    );
}

// ─── 集成：/rpc 与 /events ────────────────────────────────────────────────

/// 起一个只挂 `/rpc/*` + `/events` 的管理面（不带静态资源），返回 base URL 与 ctx。
async fn spawn_test_management(
    auth_token: &str,
) -> (String, std::sync::Arc<ctx::HeadlessCtx>, std::net::SocketAddr) {
    let db = aidog_db::test_support::test_db().await;
    let c = std::sync::Arc::new(ctx::HeadlessCtx::new(
        db,
        std::sync::Arc::new(aidog_middleware::MiddlewareEngine::new()),
    ));
    let state = server::ManagementState::new(c.clone(), auth_token.to_string());
    let app = server::management_router(state, None);
    let (addr, _handle) = server::serve_management(
        app,
        std::net::SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    )
    .await
    .unwrap();
    (format!("http://{addr}"), c, addr)
}

/// 验收：`/rpc/<已知命令>` 返回与 `invoke` 相同的 JSON。
///
/// 用 `about_info` 是因为它不吃参数也不碰 DB，断言的是「HTTP 形态返回的就是命令返回值本身」
/// —— 与桌面版 `invoke("about_info")` 拿到的是同一个 struct 的同一份 serde 序列化。
#[tokio::test]
async fn rpc_known_command_returns_command_json() {
    let (base, _ctx, _addr) = spawn_test_management("").await;
    let resp = reqwest::Client::new()
        .post(format!("{base}/rpc/about_info"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["app_version"].as_str(),
        Some(env!("CARGO_PKG_VERSION")),
        "响应体必须就是命令返回值本身（不是包一层信封）"
    );
    assert!(body["os"].is_string());
}

/// 验收：`/rpc/<未知命令>` 返回 404。
#[tokio::test]
async fn rpc_unknown_command_returns_404() {
    let (base, _ctx, _addr) = spawn_test_management("").await;
    let resp = reqwest::Client::new()
        .post(format!("{base}/rpc/definitely_not_a_command"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

/// 命令自身返回 Err 时落非 2xx，body 就是 reject 值（对齐 `invoke` 的 reject 语义）。
#[tokio::test]
async fn rpc_command_error_maps_to_non_2xx() {
    let (base, _ctx, _addr) = spawn_test_management("").await;
    // 缺必填参数 → 参数解析失败 → 400。
    let resp = reqwest::Client::new()
        .post(format!("{base}/rpc/skill_read_file"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

/// 验收：管理面只在 127.0.0.1 可达（开关关时）。绑定地址即证据 —— 0.0.0.0 与 127.0.0.1
/// 在 socket 层是两种不同的绑定，前者才会接受来自其它网卡的连接。
#[tokio::test]
async fn management_listens_on_loopback_only_by_default() {
    let (_base, _ctx, addr) = spawn_test_management("").await;
    assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
}

/// 配了凭据后，不带 Bearer 的管理请求一律 401；带对的放行。
#[tokio::test]
async fn management_requires_bearer_when_credential_configured() {
    let (base, _ctx, _addr) = spawn_test_management("s3cret").await;
    let client = reqwest::Client::new();

    let no_auth = client
        .post(format!("{base}/rpc/about_info"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(no_auth.status(), 401, "无凭据必须被拒");

    let wrong = client
        .post(format!("{base}/rpc/about_info"))
        .header("Authorization", "Bearer nope")
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401, "错凭据必须被拒");

    let ok = client
        .post(format!("{base}/rpc/about_info"))
        .header("Authorization", "Bearer s3cret")
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
}

/// 验收：`/events` 能建立 SSE 连接，emit 后收到事件，且 payload 形状不变
/// （`proxy-log-updated` 的 payload 是**数字** platform_id）。
///
/// 事件源用 `AppCtx::emit` ——生产路径上唯一的 producer 是
/// `aidog_core::gateway::proxy::log::emit_log_events`，它就是这一句：
/// `aidog_ctx::emit("proxy-log-updated", platform_id.into())`。
#[tokio::test]
async fn events_stream_delivers_proxy_log_updated() {
    use aidog_ctx::AppCtx;
    use futures::StreamExt;

    let (base, c, _addr) = spawn_test_management("").await;
    let resp = reqwest::Client::new()
        .get(format!("{base}/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .starts_with("text/event-stream"),
        "必须是 SSE 流"
    );
    let mut stream = resp.bytes_stream();

    // 连接已建立后再 emit（broadcast 只发给已订阅者）。给 axum 一点时间完成订阅。
    let ctx_for_emit = c.clone();
    tokio::spawn(async move {
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            ctx_for_emit.emit("proxy-log-updated", serde_json::json!(7u64));
        }
    });

    let mut seen = String::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        let Ok(Some(Ok(chunk))) =
            tokio::time::timeout(std::time::Duration::from_secs(2), stream.next()).await
        else {
            continue;
        };
        seen.push_str(&String::from_utf8_lossy(&chunk));
        if seen.contains("event: proxy-log-updated") {
            break;
        }
    }
    assert!(
        seen.contains("event: proxy-log-updated"),
        "10 秒内没收到 proxy-log-updated 事件，实际收到: {seen:?}"
    );
    assert!(
        seen.contains("data: 7"),
        "payload 形状必须不变（platform_id 是数字，不是字符串/对象），实际: {seen:?}"
    );
}
