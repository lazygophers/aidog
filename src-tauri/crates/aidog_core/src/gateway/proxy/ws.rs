//! WebSocket 透传（ADR 0008 / 票 03）：连接级字节泵。
//!
//! 客户端 GET + `Upgrade: websocket` 打到代理 → Bearer/x-api-key/`?api_key=` 定组 →
//! 候选里第一个透传平台 → 与上游（base_url http→ws / https→wss + 原始 path_and_query）
//! 建 ws 连接（认证头按平台配置剥除+注入，与 HTTP 透传同一套纯函数）→ 双向原样转发帧，
//! 不解析内容（无 ws 内 token 统计，tokens=0 / est_cost=0 落库）。
//!
//! 非 passthrough 平台无法服务 WS（wire 层无从转换）→ 候选里没有透传平台时 400
//! `no_passthrough_platform`（proxy_log blocked_by=router 审计，与 peak_disabled 同型）。
//!
//! ponytail: 帧转换走 String/Vec 重建而非类型直传（axum 与本 crate 的 tungstenite 版本
//! 各自带 Utf8Bytes/Bytes，类型不保证同一来源）——泵上多一次小分配，远好过版本耦合。

use super::*;
use axum::extract::FromRequest;
use axum::extract::ws::{Message as AxMessage, WebSocket, WebSocketUpgrade};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{
    tungstenite::{self, client::IntoClientRequest},
    MaybeTlsStream, WebSocketStream,
};

/// WS 握手不透传的 header（连接协商 / 传输层由各自端点自管；host 由上游客户端按 URL 生成）。
const WS_HANDSHAKE_STRIP: &[&str] = &[
    "host",
    "connection",
    "upgrade",
    "sec-websocket-key",
    "sec-websocket-version",
    "sec-websocket-extensions",
    "sec-websocket-protocol",
    "sec-websocket-accept",
    "content-length",
    "transfer-encoding",
    "proxy-connection",
    "proxy-authorization",
];

/// 入口判定：GET + `Upgrade: websocket`（值大小写不敏感）。纯函数。
pub(crate) fn is_ws_upgrade(method: &axum::http::Method, headers: &axum::http::HeaderMap) -> bool {
    if method != axum::http::Method::GET {
        return false;
    }
    headers
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
}

/// http(s) URL → ws(s)（透传上游 scheme 跟随 base_url；已带 ws/wss 的原样返回）。纯函数。
pub(crate) fn to_ws_url(http_url: &str) -> String {
    if let Some(rest) = http_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = http_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        http_url.to_string()
    }
}

/// WS 握手转上游的头集合：剥协商/传输头 + 认证剥除注入（与 HTTP 透传同一纯函数）。纯函数。
pub(crate) fn ws_upstream_handshake_headers(
    orig: &axum::http::HeaderMap,
    inject: &Option<(String, String)>,
) -> axum::http::HeaderMap {
    let mut hm = axum::http::HeaderMap::new();
    for (k, v) in orig.iter() {
        if WS_HANDSHAKE_STRIP.contains(&k.as_str()) {
            continue;
        }
        hm.insert(k.clone(), v.clone());
    }
    super::passthrough::apply_platform_auth_headers(&mut hm, inject);
    hm
}

/// axum 帧 → tungstenite 帧（String/Vec 重建，见模块注释）。
fn axum_to_tung(m: AxMessage) -> tungstenite::Message {
    match m {
        AxMessage::Text(t) => tungstenite::Message::Text(t.as_str().to_owned().into()),
        AxMessage::Binary(b) => tungstenite::Message::Binary(b.to_vec().into()),
        AxMessage::Ping(p) => {
            tungstenite::Message::Ping(p.to_vec().into())
        }
        AxMessage::Pong(p) => {
            tungstenite::Message::Pong(p.to_vec().into())
        }
        AxMessage::Close(_) => tungstenite::Message::Close(None),
    }
}

/// tungstenite 帧 → axum 帧（同上重建；Close 帧丢弃细节只传关闭语义；Frame 透传给运行时自管）。
fn tung_to_axum(m: tungstenite::Message) -> AxMessage {
    match m {
        tungstenite::Message::Text(t) => AxMessage::Text(t.as_str().to_owned().into()),
        tungstenite::Message::Binary(b) => AxMessage::Binary(axum::body::Bytes::from(b.to_vec())),
        tungstenite::Message::Ping(p) => AxMessage::Ping(axum::body::Bytes::from(p.to_vec())),
        tungstenite::Message::Pong(p) => AxMessage::Pong(axum::body::Bytes::from(p.to_vec())),
        tungstenite::Message::Close(_) => AxMessage::Close(None),
        tungstenite::Message::Frame(_) => AxMessage::Pong(Vec::new().into()),
    }
}

/// 双向帧泵：任一方向 Close/Err 即收尾（另一任务 abort，socket Drop 触发关闭）。
async fn ws_pump(client: WebSocket, upstream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>) {
    let (mut c_sink, mut c_src) = client.split();
    let (mut u_sink, mut u_src) = upstream.split();
    let mut c2u = tokio::spawn(async move {
        while let Some(Ok(m)) = c_src.next().await {
            let close = matches!(m, AxMessage::Close(_));
            if u_sink.send(axum_to_tung(m)).await.is_err() || close {
                break;
            }
        }
    });
    let mut u2c = tokio::spawn(async move {
        while let Some(Ok(m)) = u_src.next().await {
            let close = matches!(m, tungstenite::Message::Close(_));
            if c_sink.send(tung_to_axum(m)).await.is_err() || close {
                break;
            }
        }
    });
    tokio::select! {
        _ = &mut c2u => u2c.abort(),
        _ = &mut u2c => c2u.abort(),
    }
}

/// WS 透传主入口（`handle_proxy_inner` CONNECT 分流之后调用）。
pub(crate) async fn handle_ws_passthrough(
    state: Arc<ProxyState>,
    req: Request,
    request_id: String,
) -> Response {
    let start = std::time::Instant::now();
    let created_at = aidog_db::now();
    let log_settings = state.settings_cache.read().await.log_settings.clone();

    let mut log = ProxyLog {
        id: request_id,
        created_at,
        updated_at: created_at,
        ..Default::default()
    };

    // token 提取与 handle_proxy_core 同链：Bearer → x-api-key → ?api_key=（ADR 0008 决策 3）。
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            req.headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| req.uri().query().and_then(query_api_key));

    let orig_uri = req.uri().clone();
    let orig_headers = req.headers().clone();
    log.request_url = orig_uri.to_string();
    // WS 无请求体无 model 字段；path 入 model 供统计/日志页辨认端点。
    log.model = orig_uri.path().to_string();
    log.is_stream = true;

    let group = match resolve_group(&state.db, token.as_deref()).await {
        Some(g) => g,
        None => {
            log.response_body = "no matching group".to_string();
            log.status_code = 404;
            log.done = true;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::NOT_FOUND, "no matching group").into_response();
        }
    };
    log.group_key = group.group_key.clone();
    upsert_log(&state, &log, &log_settings).await;

    // 候选：无模型名（WS 无 body），RequestKind::Chat；候选里第一个透传平台服务本连接。
    let candidate_set = match select_candidates_ctx(&state.db, &group, "", None, RequestKind::Chat)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            log.response_body = format!("route error: {e}");
            log.status_code = 400;
            log.done = true;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_REQUEST, log.response_body.clone()).into_response();
        }
    };
    let platform = candidate_set
        .candidates
        .iter()
        .find(|r| matches!(r.platform.platform_type, Protocol::Passthrough))
        .map(|r| r.platform.clone());
    let Some(platform) = platform else {
        log.blocked_by = "router".into();
        log.blocked_reason = "no_passthrough_platform".into();
        log.response_body = "no passthrough platform in group".to_string();
        log.status_code = 400;
        log.done = true;
        log.duration_ms = start.elapsed().as_millis() as i32;
        upsert_log(&state, &log, &log_settings).await;
        return (StatusCode::BAD_REQUEST, log.response_body.clone()).into_response();
    };
    log.platform_id = platform.id;

    // 上游 URL：base_url（http→ws / https→wss）+ 客户端原始 path_and_query（共识稿 §3/§7）。
    let base = super::passthrough::passthrough_base_url(&platform);
    let ws_url = to_ws_url(&super::passthrough::build_passthrough_url(&base, &orig_uri));

    // 认证：与 HTTP 透传同一解析（extra.auth_header/auth_template + api_key）。
    let inject = super::passthrough::parse_passthrough_auth(&platform.extra, &platform.api_key);
    let hs_headers = ws_upstream_handshake_headers(&orig_headers, &inject);
    // 上游握手头落日志：固定敏感清单 + 用户自定义认证头名 redact（与 passthrough.rs 同型）。
    {
        let mut h = serde_json::Map::new();
        for (k, v) in &hs_headers {
            let name = k.as_str();
            let custom_auth = inject
                .as_ref()
                .is_some_and(|(n, _)| name.eq_ignore_ascii_case(n));
            if is_sensitive_auth_header(name) || custom_auth {
                h.insert(name.to_string(), Value::String("[REDACTED]".into()));
            } else if let Ok(s) = v.to_str() {
                h.insert(name.to_string(), Value::String(s.to_string()));
            }
        }
        log.upstream_request_headers = Value::Object(h).to_string();
    }

    // 先连上游（失败 502，不进 101），再升级客户端。
    let mut hs_req = match ws_url
        .as_str()
        .into_client_request()
    {
        Ok(r) => r,
        Err(e) => {
            log.response_body = format!("invalid upstream ws url: {e}");
            log.status_code = 502;
            log.done = true;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_GATEWAY, log.response_body.clone()).into_response();
        }
    };
    for (k, v) in hs_headers.iter() {
        hs_req.headers_mut().insert(k.clone(), v.clone());
    }

    let (up_ws, _up_resp) = match tokio_tungstenite::connect_async(hs_req).await {
        Ok(x) => x,
        Err(e) => {
            log.response_body = format!("upstream ws connect failed: {e}");
            log.status_code = 502;
            log.done = true;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_GATEWAY, log.response_body.clone()).into_response();
        }
    };
    log.upstream_status_code = 101;
    log.status_code = 101;
    upsert_log(&state, &log, &log_settings).await;

    let upgrader = match WebSocketUpgrade::from_request(req, &()).await {
        Ok(u) => u,
        Err(_) => {
            log.response_body = "invalid websocket upgrade request".to_string();
            log.status_code = 400;
            log.upstream_status_code = 0;
            log.done = true;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_REQUEST, log.response_body.clone()).into_response();
        }
    };

    // 泵收尾补终态：连接时长 = 101 到任一方向关闭；est_cost 0 / tokens 0 已是默认。
    let state2 = state.clone();
    let log_settings2 = log_settings.clone();
    let start2 = start;
    upgrader.on_upgrade(move |client_ws| async move {
        ws_pump(client_ws, up_ws).await;
        let mut l = log;
        l.status_code = 200;
        l.upstream_status_code = 101;
        l.done = true;
        l.duration_ms = start2.elapsed().as_millis() as i32;
        upsert_log(&state2, &l, &log_settings2).await;
    })
}

#[cfg(test)]
#[path = "test_ws.rs"]
mod test_ws;
