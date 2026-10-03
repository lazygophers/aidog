use super::*;
use tracing::Instrument;

/// 健康端点（`GET /` 与 `GET /proxy`）：客户端启动探测命中代理根 URL 时，
/// 既无 Authorization 也无上游请求语义 —— 直接返回 200 + 身份 JSON，
/// 不进 handle_proxy（否则 resolve_group None → 404）也不落 proxy_log（避免污染统计）。
///
/// **B 缺口修复**: 历史版本无 span 包裹, `inject_trace_header` 兜底现场造的 id 从未写过
/// 日志行（无 span → 无 event 在该 span 内 → grep 0 命中）。现包 health span + 一行 info
/// log, 健康端点 trace_id 不再孤儿, header→日志可 grep 直达。
/// 判定 path 是否为 Claude Code 的连接预热探测 `/api/hello`（官方 gateway protocol：
/// 客户端启动时对 `ANTHROPIC_BASE_URL` 发 `HEAD /api/hello`，期望快速 2xx）。
/// base_url 常带前缀（`http://127.0.0.1:9890/proxy`）→ 实际 path 为 `/proxy/api/hello`，
/// 故按尾段匹配（与 is_models_endpoint 同款宽松定位），容尾斜杠。
/// 该探测无 Authorization → 不前置分流会落 resolve_group None → 404 + 污染 proxy_log。
pub(crate) fn is_hello_endpoint(path: &str) -> bool {
    path.trim_end_matches('/').ends_with("/api/hello")
}

pub(crate) async fn handle_root() -> Response {
    let tid = crate::logging::current_trace_id().unwrap_or_else(crate::logging::gen_trace_id);
    let span = tracing::info_span!("health", trace_id = %tid);
    async {
        let mut r = (
            StatusCode::OK,
            Json(serde_json::json!({
                "service": "aidog",
                "ok": true,
            })),
        )
            .into_response();
        inject_trace_header(&mut r);
        tracing::info!("health probe");
        r
    }
    .instrument(span)
    .await
}
