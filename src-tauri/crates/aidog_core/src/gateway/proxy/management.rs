use super::*;

/// 外壳装进来的管理接口路由（桌面壳 / 内核都装 `aidog_rpc::management_router()`）。
static MANAGEMENT_ROUTER: std::sync::OnceLock<axum::Router> = std::sync::OnceLock::new();

/// 装管理接口路由（路径相对 `/api`：`/rpc/<命令>`、`/events`、可选 `/ui`）。进程内只装一次。
pub fn install_management_router(router: axum::Router) {
    let _ = MANAGEMENT_ROUTER.set(router);
}

/// 用「路径 → (字节, MIME)」查表函数托管 `/ui/*`（桌面版：Tauri 打进二进制的前端资源，与 webview 同一份 dist）。
/// 查不到的路径回落 index.html（SPA）；index 也没有 → 404。与 `management_router()` `merge` 后一起装入。
pub fn ui_router_from_assets<F>(lookup: F) -> axum::Router
where
    F: Fn(&str) -> Option<(Vec<u8>, String)> + Clone + Send + Sync + 'static,
{
    let handler = move |uri: axum::http::Uri| {
        let lookup = lookup.clone();
        async move {
            let rest = uri.path().strip_prefix("/ui").unwrap_or("").trim_start_matches('/');
            let hit = if rest.is_empty() { None } else { lookup(rest) };
            match hit.or_else(|| lookup("index.html")) {
                Some((bytes, mime)) => {
                    ([(axum::http::header::CONTENT_TYPE, mime)], bytes).into_response()
                }
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    };
    axum::Router::new()
        .route("/ui/", axum::routing::get(handler.clone()))
        .route("/ui/{*rest}", axum::routing::get(handler))
}

pub(super) fn json_response(status: StatusCode, body: &str) -> Response {
    let mut r = (status, [("content-type", "application/json")], body.to_string()).into_response();
    inject_trace_header(&mut r);
    r
}

/// 设置页测试面板发请求时带的来源标识头（值 [`SETTINGS_TEST_SOURCE`]）；缺省 = 第三方。
pub const SOURCE_HEADER: &str = "x-aidog-source";
pub const SETTINGS_TEST_SOURCE: &str = "settings-test";

/// 调用记录里的命令名：`/api/rpc/<命令>` 取末段，`/api/events` 记 `events`；
/// `/ui*` 是静态资源，不记。
fn call_log_command(path: &str) -> Option<String> {
    if let Some(cmd) = path.strip_prefix("/api/rpc/") {
        // 命令名来自请求路径，入库截断防超长串撑库。
        return Some(cmd.chars().take(64).collect());
    }
    (path == "/api/events").then(|| "events".to_string())
}

fn req_header(req: &Request, name: &str) -> String {
    req.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or("").to_string()
}

const SESSION_COOKIE: &str = "aidog_ui_session";

/// 登录 Cookie 的值 = HMAC-SHA256(key = 口令, "aidog-ui-session") 的十六进制：不含明文口令，
/// 口令一换旧 Cookie 自动失效。同时作为定长摘要用于常数时间比较。
fn session_value(token: &str) -> String {
    use hmac::{Hmac, KeyInit, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(token.as_bytes()).expect("hmac accepts any key length");
    mac.update(b"aidog-ui-session");
    mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// 常数时间比较：两边都先过 HMAC 变成等长摘要，再逐字节异或累积。
fn token_eq(presented: &str, token: &str) -> bool {
    let (a, b) = (session_value(presented), session_value(token));
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Bearer 或登录 Cookie 任一通过即放行（浏览器的 fetch / EventSource 同源带 Cookie，不带 Bearer）。
fn is_authorized(req: &Request, token: &str) -> bool {
    let bearer = req_header(req, "authorization");
    if let Some(p) = bearer.strip_prefix("Bearer ")
        && token_eq(p.trim(), token)
    {
        return true;
    }
    let want = format!("{SESSION_COOKIE}=");
    req_header(req, "cookie")
        .split(';')
        .filter_map(|kv| kv.trim().strip_prefix(want.as_str()))
        .any(|v| v == session_value(token))
}

fn login_page(status: StatusCode, failed: bool) -> Response {
    let err = if failed { "<p style=\"color:#c0392b\">口令不正确</p>" } else { "" };
    let html = format!(
        "<!doctype html><html lang=\"zh\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>AiDog</title></head>\
<body style=\"font-family:system-ui,sans-serif;display:flex;justify-content:center;margin-top:20vh\">\
<form method=\"post\" action=\"/ui/login\" style=\"display:flex;flex-direction:column;gap:12px;width:280px\">\
<h3>AiDog 管理界面</h3><label>访问口令</label>\
<input type=\"password\" name=\"token\" autofocus autocomplete=\"current-password\" style=\"padding:8px\">{err}\
<button type=\"submit\" style=\"padding:8px\">登录</button></form></body></html>"
    );
    (status, [("content-type", "text/html; charset=utf-8"), ("cache-control", "no-store")], html).into_response()
}

/// `POST /ui/login`：表单里的口令对上就发 Cookie 并跳回 `/ui/`，否则 401 + 登录页。绝不记口令。
async fn handle_login(req: Request, token: &str) -> Response {
    let Ok(body) = axum::body::to_bytes(req.into_body(), 4096).await else {
        return login_page(StatusCode::BAD_REQUEST, true);
    };
    let presented = url::form_urlencoded::parse(&body)
        .find(|(k, _)| k == "token")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default();
    if !token_eq(&presented, token) {
        tracing::warn!("management: ui login rejected");
        return login_page(StatusCode::UNAUTHORIZED, true);
    }
    let cookie = format!("{SESSION_COOKIE}={}; HttpOnly; SameSite=Strict; Path=/", session_value(token));
    (
        StatusCode::SEE_OTHER,
        [(axum::http::header::LOCATION, "/ui/".to_string()), (axum::http::header::SET_COOKIE, cookie)],
    )
        .into_response()
}

/// `/api/rpc/*`、`/api/events`、`/ui*`：本机限制 → 开关 → 可选凭据，通过后把去掉
/// `/api` 前缀的请求交给外壳装进来的路由。注册在分组解析与转发之前（与 group-info 同层）。
/// 每次调用（含被拒的）落一行 `api_call_log` 元数据，绝不记正文。
pub(crate) async fn handle_management(
    AxumState(state): AxumState<Arc<ProxyState>>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    req: Request,
) -> Response {
    let started = std::time::Instant::now();
    // 非回环请求只回 403、不记调用（否则 LAN 上任何人都能刷表）。
    let command = call_log_command(req.uri().path()).filter(|_| addr.ip().is_loopback());
    let user_agent: String = req_header(&req, "user-agent").chars().take(200).collect();
    let source = if req_header(&req, SOURCE_HEADER) == SETTINGS_TEST_SOURCE {
        aidog_db::API_CALL_SOURCE_SETTINGS_TEST
    } else {
        aidog_db::API_CALL_SOURCE_THIRD_PARTY
    };
    let db = state.db.clone();
    let resp = handle_management_inner(state, addr, req).await;
    if let Some(command) = command
        && let Err(e) = aidog_db::insert_api_call_log(
            &db,
            &command,
            resp.status().as_u16() as i32,
            started.elapsed().as_millis() as i64,
            &user_agent,
            source,
        )
        .await
    {
        tracing::warn!(error = %e, "management: write api_call_log failed");
    }
    resp
}

async fn handle_management_inner(
    state: Arc<ProxyState>,
    addr: std::net::SocketAddr,
    mut req: Request,
) -> Response {
    use tower::ServiceExt;
    if !addr.ip().is_loopback() {
        return reject_non_loopback(&addr, "management");
    }
    let settings = crate::kernel_settings::load_kernel_settings(&state.db).await;
    if !settings.api_enabled {
        return json_response(
            StatusCode::NOT_FOUND,
            r#"{"error":"management_api_disabled","message":"管理接口未启用（设置 → API 中打开）"}"#,
        );
    }
    if settings.has_auth() {
        let is_ui = req.uri().path() == "/ui" || req.uri().path().starts_with("/ui/");
        if req.uri().path() == "/ui/login" && req.method() == axum::http::Method::POST {
            return handle_login(req, &settings.auth_token).await;
        }
        if !is_authorized(&req, &settings.auth_token) {
            tracing::warn!(path = %req.uri().path(), "management: bad or missing credential");
            if is_ui {
                return login_page(StatusCode::UNAUTHORIZED, false);
            }
            // 合法 JSON 字符串：前端传输层对响应统一 JSON.parse。
            return json_response(StatusCode::UNAUTHORIZED, "\"unauthorized\"");
        }
    }
    let Some(router) = MANAGEMENT_ROUTER.get() else {
        return json_response(
            StatusCode::NOT_FOUND,
            r#"{"error":"management_api_unavailable","message":"管理接口路由未装载"}"#,
        );
    };
    // `/ui` 无尾斜杠时界面里 vite base "./" 的资源会落到根下，补斜杠（过了闸门才跳）。
    if req.uri().path() == "/ui" {
        let loc = match req.uri().query() {
            Some(q) => format!("/ui/?{q}"),
            None => "/ui/".to_string(),
        };
        return (StatusCode::PERMANENT_REDIRECT, [(axum::http::header::LOCATION, loc)]).into_response();
    }
    // 外层路由已写入 MatchedPath；内层 nest_service（内核静态托管）再写会触发
    // matched_path.rs 的断言 panic，先摘掉。
    req.extensions_mut().remove::<axum::extract::MatchedPath>();
    let stripped = req.uri().path().strip_prefix("/api").unwrap_or(req.uri().path());
    let pq = match req.uri().query() {
        Some(q) => format!("{stripped}?{q}"),
        None => stripped.to_string(),
    };
    match pq.parse() {
        Ok(uri) => *req.uri_mut() = uri,
        Err(_) => return json_response(StatusCode::BAD_REQUEST, "\"bad uri\""),
    }
    router.clone().oneshot(req).await.into_response()
}
