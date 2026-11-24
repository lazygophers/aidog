//! `POST /mcp`：HTTP 版 MCP（网关-网页搜索 票 03）。
use super::management::json_response;
use super::*;

pub(crate) async fn handle_mcp(
    AxumState(state): AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    body: axum::body::Bytes,
) -> Response {
    if !addr.ip().is_loopback() {
        return reject_non_loopback(&addr, "mcp");
    }
    // Bearer = 分组 key（与 /api/group-info 同口径）；无令牌 / 未知分组一律 401。
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();
    let group = if token.is_empty() {
        None
    } else {
        aidog_db::list_groups(&state.db)
            .await
            .ok()
            .and_then(|gs| gs.into_iter().find(|g| g.group_key == token))
    };
    let Some(group) = group else {
        let mut r = StatusCode::UNAUTHORIZED.into_response();
        inject_trace_header(&mut r);
        return r;
    };
    let msg: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return json_response(
                StatusCode::OK,
                &serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error","data":e.to_string()}}).to_string(),
            );
        }
    };
    // MCP 生图统一分组（生图-模型类型 票 08）：tools/call generate_image 走调用方分组的
    // 生图模型链（需完整 ProxyState），在协议层之外拦截；其余工具照旧走协议层。
    if msg["method"] == "tools/call" && msg["params"]["name"] == "generate_image" {
        let result =
            super::aux_model::mcp_generate_image(&state, &group, &msg["params"]["arguments"], &headers).await;
        let result = match result {
            Ok(text) => serde_json::json!({"content": [{"type": "text", "text": text}]}),
            Err(e) => serde_json::json!({"content": [{"type": "text", "text": e}], "isError": true}),
        };
        return json_response(
            StatusCode::OK,
            &serde_json::json!({"jsonrpc": "2.0", "id": msg["id"], "result": result}).to_string(),
        );
    }
    match crate::mcp_server::handle_request(&msg, &state.db, &group.name).await {
        Some(resp) => json_response(StatusCode::OK, &resp.to_string()),
        // 通知 / 无关消息：Streamable HTTP 规定回 202 无体。
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// GET /mcp：本实现不提供服务端推送流（Streamable HTTP 允许回 405）。
pub(crate) async fn handle_mcp_get() -> Response {
    StatusCode::METHOD_NOT_ALLOWED.into_response()
}
