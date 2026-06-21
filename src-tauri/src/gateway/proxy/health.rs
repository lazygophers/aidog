use super::*;

/// 健康端点（`GET /` 与 `GET /proxy`）：客户端启动探测命中代理根 URL 时，
/// 既无 Authorization 也无上游请求语义 —— 直接返回 200 + 身份 JSON，
/// 不进 handle_proxy（否则 resolve_group None → 404）也不落 proxy_log（避免污染统计）。
pub(crate) async fn handle_root() -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "service": "aidog",
            "ok": true,
        })),
    )
        .into_response()
}
