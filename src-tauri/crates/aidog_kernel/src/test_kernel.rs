//! 票 08 验收测试。
//!
//! 接缝按 SPEC「Testing Decisions / seam 二」的现成形态：起真实 axum router、发真实请求、
//! 断言状态码与响应体。不断言私有函数被调了几次。

use super::*;
use aidog_core::kernel_settings::KernelSettings;

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

/// `--port` 已删：给明确提示（指向代理端口），而不是当成未知参数。
#[test]
fn port_flag_is_rejected_with_a_hint() {
    for args in [vec!["--port", "9891"], vec!["--ui", "--port", "0"], vec!["--port"]] {
        match parse_args(args.into_iter().map(String::from)) {
            ParseOutcome::Error(e) => assert!(e.contains("proxy port"), "提示要指向代理端口，实得 {e:?}"),
            _ => panic!("--port 必须被拒"),
        }
    }
}

// ─── 验收：--ui 与管理接口开关 ─────────────────────────────────────────────

/// 开关关 + `--ui` → 启动失败并说明原因；开关开 / 没带 `--ui` 都放行。
#[test]
fn ui_requires_the_management_api_switch() {
    let ui = Options { ui: true, ui_dir: None };
    let pure = Options { ui: false, ui_dir: None };
    let off = KernelSettings { api_enabled: false, ..Default::default() };
    let on = KernelSettings::default();
    let err = check_ui_allowed(&ui, &off).unwrap_err();
    assert!(err.contains("disabled"), "{err}");
    assert!(check_ui_allowed(&ui, &on).is_ok());
    assert!(check_ui_allowed(&pure, &off).is_ok(), "纯内核不依赖 --ui 检查");
}

// ─── 单实例锁 ──────────────────────────────────────────────────────────────

/// 第一个进程抢到锁，第二个抢不到 —— 这正是共存期里旧壳与新壳同装时要挡的场景。
#[test]
fn second_instance_cannot_acquire_the_lock() {
    let dir = std::env::temp_dir().join(format!("aidog-lock-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let first = acquire_single_instance_lock(&dir).unwrap();
    let held = match first {
        LockOutcome::Acquired(f) => f,
        LockOutcome::AlreadyRunning(_) => panic!("干净目录里第一次必须抢到锁"),
    };

    match acquire_single_instance_lock(&dir).unwrap() {
        LockOutcome::AlreadyRunning(addr) => {
            assert!(addr.is_empty(), "还没写过地址时读出来应该是空的，实得 {addr:?}")
        }
        LockOutcome::Acquired(_) => panic!("锁还被持有时第二次不得抢到"),
    }

    // 释放后下一个进程能接上 —— 否则进程崩一次就永远起不来了。
    drop(held);
    assert!(matches!(
        acquire_single_instance_lock(&dir).unwrap(),
        LockOutcome::Acquired(_)
    ));

    std::fs::remove_dir_all(&dir).ok();
}

/// 抢到锁的实例把监听地址写进锁文件，抢不到的那个原样读回来。
#[test]
fn lock_file_carries_the_listen_address_to_the_next_instance() {
    let dir = std::env::temp_dir().join(format!("aidog-lock-addr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let held = match acquire_single_instance_lock(&dir).unwrap() {
        LockOutcome::Acquired(f) => f,
        LockOutcome::AlreadyRunning(_) => panic!("干净目录里第一次必须抢到锁"),
    };
    let line = format!("{LISTEN_LINE_PREFIX}http://127.0.0.1:19999");
    record_listen_addr(&held, &line);

    match acquire_single_instance_lock(&dir).unwrap() {
        LockOutcome::AlreadyRunning(addr) => assert_eq!(addr, line),
        LockOutcome::Acquired(_) => panic!("锁还被持有时第二次不得抢到"),
    }

    // 覆写必须截断：短地址盖长地址时不能留下前一次的尾巴。
    let shorter = format!("{LISTEN_LINE_PREFIX}http://127.0.0.1:81");
    record_listen_addr(&held, &shorter);
    match acquire_single_instance_lock(&dir).unwrap() {
        LockOutcome::AlreadyRunning(addr) => assert_eq!(addr, shorter, "覆写没截断，留下了旧内容"),
        LockOutcome::Acquired(_) => panic!("锁还被持有时第二次不得抢到"),
    }

    drop(held);
    std::fs::remove_dir_all(&dir).ok();
}

// ─── 路由表 ────────────────────────────────────────────────────────────────

/// `/rpc` 表与注册表同规模且无重名（重名会让 axum 直接 panic，这里提前断言）。
#[test]
fn rpc_table_has_no_duplicates() {
    let mut sorted: Vec<&str> = aidog_rpc::RPC_COMMAND_NAMES.to_vec();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(before, sorted.len(), "/rpc 路由表里有重复命令名");
    // 阈值 150：只作「宏没展开全」的下限护栏，不追命令总数。
    assert!(before > 150, "路由表规模异常（{before}），可能是宏没展开全");
}

// ─── 集成：代理端口上的 /api/rpc 与 /api/events ────────────────────────────

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

/// 起一个真代理（随机端口、临时库、本地回环），管理接口装的是真路由表。
async fn spawn_test_proxy(
    settings: KernelSettings,
) -> (String, std::sync::Arc<ctx::HeadlessCtx>) {
    let db = aidog_db::test_support::test_db().await;
    aidog_core::kernel_settings::save_kernel_settings(&db, &settings)
        .await
        .unwrap();
    aidog_core::gateway::proxy::install_management_router(aidog_rpc::management_router());
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mw = std::sync::Arc::new(aidog_middleware::MiddlewareEngine::new());
    let c = std::sync::Arc::new(ctx::HeadlessCtx::new(db.clone(), mw.clone()));
    aidog_core::gateway::proxy::start_proxy(std::sync::Arc::new(db), port, mw, false)
        .await
        .unwrap();
    (format!("http://127.0.0.1:{port}"), c)
}

fn with_token(t: &str) -> KernelSettings {
    KernelSettings { auth_token: t.into(), ..Default::default() }
}

/// 验收：`/api/rpc/<已知命令>` 返回与 `invoke` 相同的 JSON。
///
/// 用 `about_info` 是因为它不吃参数也不碰 DB，断言的是「HTTP 形态返回的就是命令返回值本身」。
#[tokio::test]
async fn rpc_known_command_returns_command_json() {
    let (base, _ctx) = spawn_test_proxy(KernelSettings::default()).await;
    let resp = client()
        .post(format!("{base}/api/rpc/about_info"))
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

/// 验收：未知命令 404。
#[tokio::test]
async fn rpc_unknown_command_returns_404() {
    let (base, _ctx) = spawn_test_proxy(KernelSettings::default()).await;
    let resp = client()
        .post(format!("{base}/api/rpc/definitely_not_a_command"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

/// notification_rule_validate 曾漏登记：合法表达式 200，非法表达式非 2xx。
#[tokio::test]
async fn rpc_notification_rule_validate_registered() {
    let (base, _ctx) = spawn_test_proxy(KernelSettings::default()).await;
    let call = |expr: &'static str| {
        let url = format!("{base}/api/rpc/notification_rule_validate");
        async move {
            client()
                .post(url)
                .json(&serde_json::json!({ "expr": expr }))
                .send()
                .await
                .unwrap()
        }
    };
    assert_eq!(call("a == 1").await.status(), 200);
    let bad = call("severity == ").await;
    assert!(!bad.status().is_success() && bad.status() != 404, "非法表达式应走命令 Err，实际 {}", bad.status());
}

/// 命令自身失败时落非 2xx，body 就是 reject 值（对齐 `invoke` 的 reject 语义）。
#[tokio::test]
async fn rpc_command_error_maps_to_non_2xx() {
    let (base, _ctx) = spawn_test_proxy(KernelSettings::default()).await;
    // 缺必填参数 → 参数解析失败 → 400。
    let resp = client()
        .post(format!("{base}/api/rpc/skill_read_file"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

/// 开关关：未启用的明确 404，不落入转发。
#[tokio::test]
async fn disabled_switch_returns_not_enabled() {
    let (base, _ctx) =
        spawn_test_proxy(KernelSettings { api_enabled: false, ..Default::default() }).await;
    let resp = client()
        .post(format!("{base}/api/rpc/about_info"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(v["error"], "management_api_disabled");
}

/// 配了凭据后，不带 / 错 Bearer 一律 401；带对的放行。401 body 必须是合法 JSON
/// （前端传输层统一 `JSON.parse`，裸文本会把「未授权」掩盖成「解析失败」）。
#[tokio::test]
async fn management_requires_bearer_when_credential_configured() {
    let (base, _ctx) = spawn_test_proxy(with_token("s3cret")).await;
    let url = format!("{base}/api/rpc/about_info");
    let body = serde_json::json!({});

    let no_auth = client().post(&url).json(&body).send().await.unwrap();
    assert_eq!(no_auth.status(), 401, "无凭据必须被拒");
    assert_eq!(
        no_auth.json::<serde_json::Value>().await.unwrap().as_str(),
        Some("unauthorized"),
        "401 body 必须是可 JSON.parse 的 \"unauthorized\""
    );
    let wrong = client().post(&url).bearer_auth("nope").json(&body).send().await.unwrap();
    assert_eq!(wrong.status(), 401, "错凭据必须被拒");
    let ok = client().post(&url).bearer_auth("s3cret").json(&body).send().await.unwrap();
    assert_eq!(ok.status(), 200);
}

/// 验收：`/api/events` 能建立 SSE 连接，emit 后收到事件，且 payload 形状不变
/// （`proxy-log-updated` 的 payload 是**数字** platform_id）。
#[tokio::test]
async fn events_stream_delivers_proxy_log_updated() {
    use aidog_ctx::AppCtx;
    use futures::StreamExt;

    let (base, c) = spawn_test_proxy(KernelSettings::default()).await;
    let resp = client().get(format!("{base}/api/events")).send().await.unwrap();
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
    tokio::spawn(async move {
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            c.emit("proxy-log-updated", serde_json::json!(7u64));
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

/// `--ui`：`/ui/` 出 index、静态文件原样、前缀内未知路径回落 index；前缀外不回落。
#[tokio::test]
async fn ui_router_serves_assets_with_spa_fallback_inside_prefix_only() {
    let dir = std::env::temp_dir().join(format!("aidog-ui-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), "INDEX").unwrap();
    std::fs::write(dir.join("a.js"), "JS").unwrap();
    let app = with_ui(axum::Router::new(), Some(dir.clone()));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.ok() });

    let get = |p: &str| {
        let u = format!("{base}{p}");
        async move { client().get(u).send().await.unwrap() }
    };
    assert_eq!(get("/ui/").await.text().await.unwrap(), "INDEX");
    assert_eq!(get("/ui/a.js").await.text().await.unwrap(), "JS");
    assert_eq!(get("/ui/some/spa/route").await.text().await.unwrap(), "INDEX");
    assert_eq!(get("/other").await.status(), 404, "回落只在 /ui 前缀内");
    std::fs::remove_dir_all(&dir).ok();
}

/// 验收（票 I12）：内核没有桌面会话，系统弹窗必须**转发给外壳**而不是被丢掉。
///
/// 丢掉的后果不是「少个弹窗」：`aidog_notification::dispatch` 的整条 Popup 通道
/// （额度告警、任务完成……）在 Flutter 形态下会全哑。外壳侧的订阅方是
/// `flutter/lib/platform.dart::bindKernelPopups`，事件名与 payload 键名两边必须一字不差。
#[tokio::test]
async fn show_popup_forwards_to_shell_instead_of_dropping() {
    use aidog_ctx::AppCtx;

    let db = aidog_db::test_support::test_db().await;
    let c = ctx::HeadlessCtx::new(
        db,
        std::sync::Arc::new(aidog_middleware::MiddlewareEngine::new()),
    );
    let mut rx = c.subscribe();

    assert!(
        c.show_popup("标题", "正文"),
        "请求已交给外壳 —— 返回 false 会让 DispatchResult.popup 一律报「没弹」"
    );

    // 事件通道是进程级单例，并行的别的测试也会往里 emit：只认弹窗事件。
    let ev = loop {
        let ev = rx.try_recv().expect("show_popup 必须广播出一个事件");
        if ev.name == aidog_notification::NOTIF_POPUP {
            break ev;
        }
    };
    assert_eq!(
        ev.name, "notif-popup",
        "事件名是跨语言契约，改名要连 Dart 一起改"
    );
    assert_eq!(ev.payload["title"], "标题");
    assert_eq!(ev.payload["body"], "正文");
}

// ─── 密钥信息只写不读（API 票 02）──────────────────────────────────────────

/// 一个进程只能装一次 `AppCtx`，所以密钥相关的 HTTP 行为合在一个测试里按序断言。
/// 全程只打本地回环代理与临时库，无外呼。
#[tokio::test]
async fn secrets_are_masked_over_http_and_restored_on_write() {
    let (base, ctx) = spawn_test_proxy(KernelSettings::default()).await;
    aidog_ctx::install(ctx.clone());
    let rpc = |name: &str, body: serde_json::Value| {
        let url = format!("{base}/api/rpc/{name}");
        async move { client().post(url).json(&body).send().await.unwrap() }
    };

    // 新建平台带密钥成功；创建响应里密钥已遮。
    let r = rpc("platform_create", serde_json::json!({"input": {
        "name": "p1", "platform_type": "openai", "base_url": "http://127.0.0.1:1/v1",
        "api_key": "sk-secret-123", "extra": "{\"auth_header\":\"x-my-key\"}"
    }})).await;
    assert_eq!(r.status(), 200);
    let created: serde_json::Value = r.json().await.unwrap();
    assert_eq!(created["api_key"], "***");
    let id = created["id"].as_u64().unwrap();
    let db = aidog_ctx::db();
    assert_eq!(aidog_db::get_platform(db, id).await.unwrap().unwrap().api_key, "sk-secret-123", "库里是原值");

    // 列表 / 详情经 HTTP 都是 ***，原文不出现；IPC 路径（命令函数直调）仍是原值。
    for (name, body) in [("platform_list", serde_json::json!({})), ("platform_get", serde_json::json!({"id": id}))] {
        let text = rpc(name, body).await.text().await.unwrap();
        assert!(!text.contains("sk-secret-123") && !text.contains("x-my-key"), "{name} 泄漏: {text}");
        assert!(text.contains("***"), "{name}: {text}");
    }
    let ipc = aidog_core::platform_cmd::platform::platform_list().await.unwrap();
    assert!(ipc.iter().any(|p| p.api_key == "sk-secret-123"), "桌面 IPC 行为不变");

    // 更新平台回传 *** → 库中密钥与透传认证头不变；回传新值 → 照常生效。
    let r = rpc("platform_update", serde_json::json!({"input": {
        "id": id, "name": "p1-renamed", "api_key": "***", "extra": "{\"auth_header\":\"***\"}"
    }})).await;
    assert_eq!(r.status(), 200);
    let p = aidog_db::get_platform(db, id).await.unwrap().unwrap();
    assert_eq!((p.name.as_str(), p.api_key.as_str()), ("p1-renamed", "sk-secret-123"));
    assert!(p.extra.contains("x-my-key") && !p.extra.contains("***"), "extra: {}", p.extra);
    rpc("platform_update", serde_json::json!({"input": {"id": id, "api_key": "sk-new-456"}})).await;
    assert_eq!(aidog_db::get_platform(db, id).await.unwrap().unwrap().api_key, "sk-new-456");

    // 通知渠道：token / webhook / 口令写入生效，读取遮蔽，回传 *** 保持原值。
    let settings = serde_json::json!({"settings": {"channels": {
        "telegram": {"bot_token": "123:tg-secret", "chat_id": "9"},
        "email": {"smtp_host": "h", "password": "smtp-pw", "from": "a@b", "to": "c@d"},
        "webhook": {"url": "http://127.0.0.1:1/hook?t=wh-secret"},
        "bark": {"url": "http://127.0.0.1:1/bark-secret"}
    }}});
    assert_eq!(rpc("notification_settings_set", settings).await.status(), 200);
    let text = rpc("notification_settings_get", serde_json::json!({})).await.text().await.unwrap();
    for leak in ["tg-secret", "smtp-pw", "wh-secret", "bark-secret"] {
        assert!(!text.contains(leak), "通知设置泄漏 {leak}: {text}");
    }
    let echoed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(echoed["channels"]["webhook"]["url"], "***");
    let r = rpc("notification_settings_set", serde_json::json!({"settings": echoed})).await;
    assert_eq!(r.status(), 200);
    let kept = aidog_db::get_notification_settings(db).await.channels;
    assert_eq!(kept.telegram.unwrap().bot_token, "123:tg-secret");
    assert_eq!(kept.email.unwrap().password, "smtp-pw");
    assert_eq!(kept.webhook.unwrap().url, "http://127.0.0.1:1/hook?t=wh-secret");
    assert_eq!(kept.bark.unwrap().url, "http://127.0.0.1:1/bark-secret");

    // exports_secret 命令整条 403，且不执行。
    for name in ["platform_share_export", "export_to_file", "cc_proxy_export", "mcp_share_export"] {
        let r = rpc(name, serde_json::json!({})).await;
        assert_eq!(r.status(), 403, "{name}");
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["error"], "exports_secret_forbidden", "{name}");
    }

    // 其余带密钥的写命令同样「回传 *** = 保持原值」（Spec1）。AppCtx 进程内只能装一次，故并入本测试。
    let rpc_auth = |name: &str, body: serde_json::Value, token: &str| {
        let url = format!("{base}/api/rpc/{name}");
        let token = token.to_string();
        async move { client().post(url).bearer_auth(token).json(&body).send().await.unwrap() }
    };
    let setting = |scope: &'static str, key: &'static str| async move {
        aidog_db::get_setting(aidog_ctx::db(), scope, key).await.unwrap().unwrap()
    };

    // proxy_client_set_settings：出站代理口令。
    assert_eq!(
        rpc("proxy_client_set_settings", serde_json::json!({"settings": {"enabled": false, "username": "u", "password": "pc-pw"}})).await.status(),
        200
    );
    let echoed: serde_json::Value = rpc("proxy_client_get_settings", serde_json::json!({})).await.json().await.unwrap();
    assert_eq!(echoed["password"], "***");
    assert_eq!(rpc("proxy_client_set_settings", serde_json::json!({"settings": echoed})).await.status(), 200);
    assert_eq!(setting("proxy", "proxy_client").await["password"], "pc-pw", "proxy_client_set_settings 回传 *** 必须保持原口令");

    // settings_set(scope=kernel)：管理接口凭据。写入后即要求 Bearer。
    let r = rpc("settings_set", serde_json::json!({"input": {"scope": "kernel", "key": "settings", "value": {"api_enabled": true, "auth_token": "tok-1"}}})).await;
    assert_eq!(r.status(), 200);
    let echoed: serde_json::Value = rpc_auth("settings_get", serde_json::json!({"scope": "kernel", "key": "settings"}), "tok-1").await.json().await.unwrap();
    assert_eq!(echoed["auth_token"], "***");
    let r = rpc_auth("settings_set", serde_json::json!({"input": {"scope": "kernel", "key": "settings", "value": echoed}}), "tok-1").await;
    assert_eq!(r.status(), 200);
    assert_eq!(setting("kernel", "settings").await["auth_token"], "tok-1", "settings_set(kernel) 回传 *** 必须保持原凭据");
    assert_eq!(rpc_auth("about_info", serde_json::json!({}), "***").await.status(), 401, "Bearer *** 不得通过管理接口");
    assert_eq!(rpc_auth("about_info", serde_json::json!({}), "tok-1").await.status(), 200);

    // kernel_settings_set：换新凭据照常生效；回传 *** 保持原值。
    let r = rpc_auth("kernel_settings_set", serde_json::json!({"settings": {"api_enabled": true, "auth_token": "tok-2"}}), "tok-1").await;
    assert_eq!(r.status(), 200);
    let echoed: serde_json::Value = rpc_auth("kernel_settings_get", serde_json::json!({}), "tok-2").await.json().await.unwrap();
    assert_eq!(echoed["auth_token"], "***");
    assert_eq!(rpc_auth("kernel_settings_set", serde_json::json!({"settings": echoed}), "tok-2").await.status(), 200);
    assert_eq!(setting("kernel", "settings").await["auth_token"], "tok-2", "kernel_settings_set 回传 *** 必须保持原凭据");
    assert_eq!(rpc_auth("about_info", serde_json::json!({}), "***").await.status(), 401, "Bearer *** 不得通过管理接口");
    assert_eq!(rpc_auth("about_info", serde_json::json!({}), "tok-2").await.status(), 200);
}
