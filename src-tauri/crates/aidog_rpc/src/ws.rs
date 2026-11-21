//! 统一 WebSocket 事件通道（2026-11-21 日志统计-会话列表 票）：`GET /api/ws`，与
//! `/api/rpc`、`/api/events` 同层挂在代理端口管理面；本机限制 / api_enabled / auth_token
//! 闸门由 `handle_management` 在路由之前统一执行，本模块不重复防线。
//!
//! 协议：客户端连上后发 JSON 控制消息 `{"subscribe":"<topic>"}` / `{"unsubscribe":"<topic>"}`
//! （一条消息可同时带两键）；服务端按已订阅 topic 推 JSON `{"topic":"<t>","data":<payload>}`。
//! topic = `aidog_ctx` 事件名（与 `/api/events` SSE 同一广播源 [`aidog_ctx::subscribe_events`]），
//! 当前承载 `sessions`（会话级终态事件，`gateway/proxy/log.rs::emit_log_events` 发出）。
//! 旧 `/api/events` 不迁移（遗留已记）。
//!
//! 消费语义与 SSE 一致：订阅者跟不上（`Lagged`）丢事件不断连（刷新语义幂等）。

use std::collections::HashSet;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;

/// `GET /ws`（管理面路径相对 `/api`）：升级为 WebSocket。
pub async fn ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    let mut topics: HashSet<String> = HashSet::new();
    let mut rx = aidog_ctx::subscribe_events();
    loop {
        tokio::select! {
            // 客户端控制消息；关闭 / 协议错误 → 断开
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => apply_control(&text, &mut topics),
                    Some(Ok(_)) => {}
                    _ => break,
                }
            }
            // 服务端事件：只推已订阅 topic
            ev = rx.recv() => {
                match ev {
                    Ok(ev) if topics.contains(&ev.name) => {
                        let payload = serde_json::json!({"topic": ev.name, "data": ev.payload});
                        if socket.send(Message::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {} // 未订阅 topic 的事件直接丢弃
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {} // 跟不上丢事件不断连
                    Err(_) => break,
                }
            }
        }
    }
}

/// 解析一条控制消息，更新订阅集。非法 JSON / 键值非字符串静默忽略（客户端自负）。
fn apply_control(text: &str, topics: &mut HashSet<String>) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    if let Some(t) = v.get("subscribe").and_then(|x| x.as_str()) {
        topics.insert(t.to_string());
    }
    if let Some(t) = v.get("unsubscribe").and_then(|x| x.as_str()) {
        topics.remove(t);
    }
}

#[cfg(test)]
mod test_ws;
