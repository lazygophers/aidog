//! MITM 明文流量的旁路 / 观测分流（cc-sub-mitm 票 10，spec §3.1 表格）。
//!
//! 在 serve_plaintext 注入门（票 08）之上再分四类：
//! - **Core**：AI API（`/v1/messages` 等）+ hello / models 端点 → 维持现状灌
//!   `handle_proxy_core`（含票 08 归属注入门，AI 请求全量记账进 proxy_log）。
//! - **TokenObserve**：`platform.claude.com/v1/oauth/token` → 透明转发 + mitm_log 观测行
//!   （状态码 / 耗时 / 字节数），**body 永不落库**（含 access_token / refresh_token 长期
//!   凭证）；失败（status ≥ 400 含上游错误 502）由 `oauth_token_refresh_failures` 聚合，
//!   票 12 UI 警示消费（open bug #91703：refresh 过代理可能挂）。
//! - **UsageSample**：`api.anthropic.com GET /api/oauth/usage` → 蹭自然流量响应采样进
//!   `oauth_usage_sample`（5h/7d 窗口利用率），**绝不主动请求**（30-60s 轮询即 429）。
//! - **Bypass**：其余全部（`/api/oauth/*` 元数据观测、Datadog 遥测、mcp-proxy、网页流量等）
//!   → mitm_log 元数据行，body 仅 `log_upstream_request` 开启时记。
//!
//! 旁路流量**不再**灌 handle_proxy_core 的「未匹配」桶 passthrough——那会落 proxy_log
//! 污染统计（票 10 目标：旁路进 mitm_log 不进 proxy_log）。core 的 fallback passthrough
//! 仍保留给非 MITM 的直连请求（handler.rs resolve_group 落空路径）。
//!
//! host 清单依据：research/02-cc-outbound-hosts.md（票 02 取证）。哪些 host 会被 MITM
//! 解密由 whitelist 决定（aidog_mitm::whitelist，默认 AI host 清单）；本模块只对已解密
//! 明文请求按 host/path 分流，Datadog / github 等未入 whitelist 的 host 走 P1 盲转，
//! 根本不进这里。

use super::*;
use aidog_logs::MitmLogInsert;
use axum::http::{HeaderMap, Method};

/// MITM 明文请求分流结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MitmRoute {
    /// AI API / hello / models 端点：维持现状，灌 handle_proxy_core（含票 08 归属注入门）。
    Core,
    /// `platform.claude.com/v1/oauth/token`：OAuth token 交换/刷新。观测行 body 恒空。
    TokenObserve,
    /// `api.anthropic.com GET /api/oauth/usage`：蹭响应采样进 oauth_usage_sample。
    UsageSample,
    /// 其余全部旁路：mitm_log 元数据行，body 受 `log_upstream_request` gate。
    Bypass,
}

/// 按 host + path（+ usage 的 method）分流。单一真值源：serve_plaintext 与测试共用。
///
/// `is_api_endpoint` / `is_hello_endpoint` / `is_models_endpoint` 命中一律 Core——这些
/// 端点 core 有专门 handler（hello/models 不碰上游，API 走完整记账管线），分流到旁路
/// 会改变既有行为。
pub(crate) fn classify_mitm_route(host: &str, path: &str, method: &Method) -> MitmRoute {
    if is_api_endpoint(path) || is_hello_endpoint(path) || is_models_endpoint(path) {
        return MitmRoute::Core;
    }
    match host {
        "platform.claude.com" if path == "/v1/oauth/token" => MitmRoute::TokenObserve,
        "api.anthropic.com" if path == "/api/oauth/usage" && method == Method::GET => {
            MitmRoute::UsageSample
        }
        _ => MitmRoute::Bypass,
    }
}

/// 旁路 / 观测请求处理：透明转发到 `url` + mitm_log / oauth_usage_sample 落库。
///
/// `url` 为完整目标 URL——生产调用方（serve_plaintext）传 `https://{CONNECT host}{path?query}`；
/// 测试传 stub 上游 http URL（同 test_e2e_mitm 的 stub_upstream idiom，聚焦分流与落库
/// 断言，上游 TLS 段已由 e2e 覆盖）。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_mitm_observed(
    state: &Arc<ProxyState>,
    route: MitmRoute,
    url: String,
    host: &str,
    path: &str,
    method: Method,
    orig_headers: HeaderMap,
    bytes: Bytes,
    group_name: &str,
    log_settings: &ProxyLogSettings,
) -> Response {
    let start = std::time::Instant::now();
    // body gate（同 proxy_log from_log 语义）：master switch + log_upstream_request 双开才记；
    // token 路径恒不记（含长期凭证）。mitm_log 无 header 列，[REDACTED] 头脱敏机制在此
    // 无落点（headers 本就不入库）。
    let record_body =
        log_settings.enabled && log_settings.log_upstream_request && route != MitmRoute::TokenObserve;
    // `/api/oauth/*`（usage 除外——有 UsageSample 专门采样）body 恒 [REDACTED]（spec §3.2：
    // OAuth 元数据端点可能含凭证，开关开了也不存正文）。
    let oauth_meta = path.starts_with("/api/oauth/") && path != "/api/oauth/usage";
    let req_body_str = if oauth_meta {
        "[REDACTED]".to_string()
    } else if record_body {
        cap_nonstream_body(&bytes)
    } else {
        String::new()
    };
    let req_bytes = bytes.len() as i64;

    // 转发 client：与 relay_passthrough 同款（禁总超时保 SSE 流；connect_timeout 保护连接期）。
    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 {
        system_timeout.connect_timeout_secs
    } else {
        10
    };
    let client =
        http_client::build_http_client(&proxy_client, 0, conn_timeout, None, None).await;

    // 原样转发 header（剥 hop-by-hop + Proxy-* 协商头，与 forward_passthrough_to_orig_host 同款）。
    let mut fwd_headers = passthrough_headers(&orig_headers);
    for name in [
        "proxy-authorization",
        "proxy-connection",
        "proxy-authenticate",
    ] {
        fwd_headers.remove(name);
    }
    let fwd_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let resp = match client
        .request(fwd_method, &url)
        .headers(fwd_headers)
        .body(bytes.to_vec())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "mitm bypass upstream failed (502)");
            if log_settings.enabled {
                let row = MitmLogInsert {
                    group_name: group_name.to_string(),
                    host: host.to_string(),
                    path: path.to_string(),
                    status_code: 502,
                    req_bytes,
                    resp_bytes: 0,
                    decrypted: true,
                    request_body: req_body_str.clone(), // oauth_meta 时 [REDACTED]，否则空（只记元数据）
                    response_body: String::new(),
                    created_at: aidog_db::now(),
                };
                if let Err(e) = aidog_logs::insert_mitm_log(&state.db, row).await {
                    tracing::warn!(error = %e, "mitm bypass log insert failed (non-fatal)");
                }
            }
            let mut r = (
                StatusCode::BAD_GATEWAY,
                format!("mitm bypass upstream error: {e}"),
            )
                .into_response();
            inject_trace_header(&mut r);
            return r;
        }
    };

    let status = resp.status();
    // 捕获上游响应头（原样照搬给客户端；剔黑名单头，理由同 relay_passthrough 的
    // content-encoding 注释——reqwest 解压后转 Encoding 头会失真）。
    let mut resp_header_map = axum::http::HeaderMap::new();
    for (k, v) in resp.headers() {
        let name = k.as_str();
        if RESP_HEADER_BLACKLIST
            .iter()
            .any(|b| name.eq_ignore_ascii_case(b))
        {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            axum::http::HeaderName::from_bytes(name.as_bytes()),
            axum::http::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            resp_header_map.insert(hn, hv);
        }
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let is_stream = content_type.contains("text/event-stream")
        || resp
            .headers()
            .get(reqwest::header::TRANSFER_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.contains("chunked"))
            .unwrap_or(false);
    let resp_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    // ── 非流式：buffered relay + 落库 + usage 采样 ──
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        let resp_bytes = body.len() as i64;
        let resp_body_str = if oauth_meta {
            "[REDACTED]".to_string()
        } else if record_body {
            cap_nonstream_body(&body)
        } else {
            String::new()
        };
        if route == MitmRoute::UsageSample && status.is_success() {
            sample_oauth_usage(state, group_name, &body).await;
        }
        if log_settings.enabled {
            let row = MitmLogInsert {
                group_name: group_name.to_string(),
                host: host.to_string(),
                path: path.to_string(),
                status_code: status.as_u16() as i32,
                req_bytes,
                resp_bytes,
                decrypted: true,
                request_body: req_body_str,
                response_body: resp_body_str,
                created_at: aidog_db::now(),
            };
            if let Err(e) = aidog_logs::insert_mitm_log(&state.db, row).await {
                tracing::warn!(error = %e, "mitm bypass log insert failed (non-fatal)");
            }
        }
        tracing::info!(host = %host, path = %path, status = status.as_u16(), duration_ms = start.elapsed().as_millis() as i64, "mitm bypass forwarded");
        let mut response = (resp_status, body.to_vec()).into_response();
        *response.headers_mut() = resp_header_map;
        inject_trace_header(&mut response);
        return response;
    }

    // ── 流式：透传 SSE bytes，计数 + 流结束（Drop 兜底）落 mitm_log 行 ──
    // 响应 body 不捕获（claude.ai 网页 SSE 属旁路，非记账对象，YAGNI）；只记元数据。
    let db = state.db.clone();
    let row = MitmLogInsert {
        group_name: group_name.to_string(),
        host: host.to_string(),
        path: path.to_string(),
        status_code: status.as_u16() as i32,
        req_bytes,
        resp_bytes: 0, // 由计数器在流结束时填充
        decrypted: true,
        request_body: req_body_str,
        response_body: if oauth_meta { "[REDACTED]".to_string() } else { String::new() },
        created_at: aidog_db::now(),
    };
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let guard = MitmStreamGuard {
        db,
        row,
        counter: counter.clone(),
        log_enabled: log_settings.enabled,
    };
    let stream = resp.bytes_stream().map(move |chunk| {
        let chunk = chunk.unwrap_or_default();
        counter.fetch_add(chunk.len() as u64, std::sync::atomic::Ordering::Relaxed);
        Ok::<Bytes, std::io::Error>(chunk)
    });
    // guard 被闭包持有、随 stream Drop（客户端断连 / 流耗尽）触发落行——与 relay_passthrough
    // 的哨兵 idiom 对齐（poll_fn 恒返 Ready(None) 不产 item，只承载 Drop 时机）。
    let stream = stream.chain(futures::stream::poll_fn(move |_| {
        let _ = &guard;
        std::task::Poll::Ready(None)
    }));
    let mut response = Response::builder()
        .status(resp_status)
        .body(axum::body::Body::from_stream(stream))
        .expect("static response build");
    *response.headers_mut() = resp_header_map;
    inject_trace_header(&mut response);
    response
}

/// 解析 `/api/oauth/usage` 响应并落 oauth_usage_sample（蹭流量采样，spec §3.2 表 2）。
/// 响应形状 `{"five_hour":{"utilization":<pct>},"seven_day":{"utilization":<pct>}}`，
/// utilization 为 0-100 百分数（gridbash usage.rs 直接按百分数消费，无换算）。
/// 认不出任一窗口 → 不落行（不把「没有」记成 0）。
async fn sample_oauth_usage(state: &Arc<ProxyState>, group_name: &str, body: &[u8]) {
    let Ok(v) = serde_json::from_slice::<Value>(body) else {
        tracing::debug!("mitm usage sample: non-JSON response, skip");
        return;
    };
    let five = v.pointer("/five_hour/utilization").and_then(Value::as_f64);
    let seven = v.pointer("/seven_day/utilization").and_then(Value::as_f64);
    let (Some(five), Some(seven)) = (five, seven) else {
        tracing::debug!("mitm usage sample: missing utilization keys, skip");
        return;
    };
    let raw = String::from_utf8_lossy(body).to_string();
    if let Err(e) =
        aidog_logs::insert_oauth_usage_sample(&state.db, group_name, five, seven, &raw).await
    {
        tracing::warn!(error = %e, "oauth usage sample insert failed (non-fatal)");
    }
}

/// 流式旁路的 Drop 兜底 guard（StreamLogGuard 的 mitm_log 版）：流结束 / 客户端断连时
/// 把计数完的 resp_bytes 连同元数据 INSERT 进 mitm_log。Drop 内不能 await → spawn。
struct MitmStreamGuard {
    db: Arc<Db>,
    row: MitmLogInsert,
    counter: Arc<std::sync::atomic::AtomicU64>,
    log_enabled: bool,
}

impl Drop for MitmStreamGuard {
    fn drop(&mut self) {
        if !self.log_enabled {
            return;
        }
        let db = self.db.clone();
        let mut row = std::mem::take(&mut self.row);
        row.resp_bytes = self
            .counter
            .load(std::sync::atomic::Ordering::Relaxed) as i64;
        row.created_at = aidog_db::now();
        tokio::spawn(async move {
            if let Err(e) = aidog_logs::insert_mitm_log(&db, row).await {
                tracing::warn!(error = %e, "mitm bypass stream log insert failed (non-fatal)");
            }
        });
    }
}
