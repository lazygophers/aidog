use super::*;

/// 外壳装进来的管理接口路由（桌面壳 / 内核都装 `aidog_rpc::management_router()`）。
static MANAGEMENT_ROUTER: std::sync::OnceLock<axum::Router> = std::sync::OnceLock::new();

/// 装管理接口路由（路径相对 `/api`：`/rpc/<命令>`、`/events`、可选 `/ui`）。进程内只装一次。
pub fn install_management_router(router: axum::Router) {
    let _ = MANAGEMENT_ROUTER.set(router);
}

fn json_response(status: StatusCode, body: &str) -> Response {
    let mut r = (status, [("content-type", "application/json")], body.to_string()).into_response();
    inject_trace_header(&mut r);
    r
}

/// `/api/rpc/*`、`/api/events`、`/api/ui/*`：本机限制 → 开关 → 可选凭据，通过后把去掉
/// `/api` 前缀的请求交给外壳装进来的路由。注册在分组解析与转发之前（与 group-info 同层）。
pub(crate) async fn handle_management(
    AxumState(state): AxumState<Arc<ProxyState>>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
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
        let presented = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("")
            .trim();
        if presented != settings.auth_token {
            tracing::warn!(path = %req.uri().path(), "management: bad or missing bearer token");
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
