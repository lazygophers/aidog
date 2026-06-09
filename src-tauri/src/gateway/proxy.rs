use axum::{
    body::Body,
    extract::{Request, State as AxumState},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::adapter::{self, ChatRequest, ChatStreamEvent};
use super::db::Db;
use super::models::Group;
use super::router::select_platform;

/// 代理服务器共享状态
pub struct ProxyState {
    pub db: std::sync::Mutex<Db>,
    pub port: u16,
}

/// 启动代理服务器，返回 shutdown handle
pub async fn start_proxy(
    db: std::sync::Mutex<Db>,
    port: u16,
) -> Result<tokio::task::JoinHandle<()>, String> {
    let state = Arc::new(ProxyState { db, port });

    let app = Router::new()
        .fallback(handle_proxy)
        .with_state(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("bind failed: {e}"))?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok(handle)
}

/// 主代理处理函数
async fn handle_proxy(
    AxumState(state): AxumState<Arc<ProxyState>>,
    req: Request,
) -> Response {
    // 解析路径匹配分组
    let path = req.uri().path().to_string();

    let group = {
        let db = state.db.lock().map_err(|e| e.to_string());
        match db {
            Ok(db) => match find_group_by_path(&db, &path) {
                Some(g) => g,
                None => return (StatusCode::NOT_FOUND, "no matching group").into_response(),
            },
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        }
    };

    // 读取请求体
    let (parts, body) = req.into_parts();
    let bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("read body: {e}")).into_response(),
    };

    let mut chat_req: ChatRequest = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("parse request: {e}")).into_response(),
    };

    let is_stream = chat_req.stream.unwrap_or(false);

    // 路由选择平台 + 模型映射
    let route = {
        let db = state.db.lock().map_err(|e| e.to_string());
        match db {
            Ok(db) => select_platform(&db, &group, &chat_req.model),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        }
    };
    let route = match route {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("route: {e}")).into_response(),
    };

    // 替换模型名
    chat_req.model = route.target_model;

    // 协议转换
    let (req_body, api_path) = adapter::convert_request(&chat_req, &route.platform.protocol);

    // 构建目标 URL
    let base_url = route.platform.base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);

    // 转发请求
    let client = Client::new();
    let mut req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", route.platform.api_key))
        .body(serde_json::to_string(&req_body).unwrap_or_default());

    // Anthropic 协议需要额外 header
    if matches!(route.platform.protocol, super::models::Protocol::Anthropic) {
        req_builder = req_builder
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", &route.platform.api_key);
    }

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("upstream: {e}")).into_response(),
    };

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY), body)
            .into_response();
    }

    // 非流式：直接透传 JSON
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body.to_vec(),
        )
            .into_response();
    }

    // 流式：转换 SSE 格式为 Anthropic 格式返回
    let protocol = route.platform.protocol;
    let stream = resp.bytes_stream().map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => return Ok::<_, std::io::Error>(format!("event: error\ndata: {{\"error\":\"{e}\"}}\n\n")),
        };

        let text = String::from_utf8_lossy(&chunk);
        let mut output = String::new();

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    // OpenAI 结束标记 → 转为 Anthropic stop
                    output.push_str(&adapter::to_anthropic_sse(&ChatStreamEvent::Stop {
                        finish_reason: Some("end_turn".to_string()),
                    }).unwrap_or_default());
                    continue;
                }

                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    if let Some(event) = adapter::parse_sse(&json, &protocol) {
                        if let Some(sse) = adapter::to_anthropic_sse(&event) {
                            output.push_str(&sse);
                        }
                    }
                }
            }
        }

        Ok(output)
    });

    let body = Body::from_stream(stream);

    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/event-stream"),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
            (axum::http::header::CONNECTION, "keep-alive"),
        ],
        body,
    )
        .into_response()
}

/// 根据 path 前缀匹配分组
fn find_group_by_path(db: &Db, request_path: &str) -> Option<Group> {
    let groups = super::db::list_groups(db).ok()?;
    groups.into_iter().find(|g| request_path.starts_with(&g.path))
}
