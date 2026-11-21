//! 统一 WS 通道测试：订阅 / 退订 / 推送（真 socket + tokio-tungstenite 客户端）与
//! 控制消息解析。闸门（非回环 / token / 开关）在 `handle_management`（aidog_core
//! test_integration.rs），那边覆盖；本文件直连 `management_router` 同款路由。

#![cfg(test)]
use super::*;
use axum::Router;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMessage;

async fn spawn_ws_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let app = Router::new().route("/ws", axum::routing::get(ws_handler));
        axum::serve(listener, app).await.unwrap();
    });
    format!("ws://{addr}/ws")
}

type WsSink = futures::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, WsMessage>;
type WsStream = futures::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>;

async fn connect(url: &str) -> (WsSink, WsStream) {
    let (stream, _resp) = tokio_tungstenite::connect_async(url).await.unwrap();
    stream.split()
}

/// 发控制消息并等服务端消化完（无 ack 协议，用固定短等待；仅测试用）。
async fn send_control(sink: &mut WsSink, json: serde_json::Value) {
    sink.send(WsMessage::Text(json.to_string().into())).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
}

async fn next_msg(stream: &mut WsStream) -> serde_json::Value {
    let msg = tokio::time::timeout(std::time::Duration::from_secs(3), stream.next())
        .await
        .expect("3s 内应收到推送")
        .expect("连接不应关闭")
        .expect("帧读取失败")
        .into_text()
        .expect("文本帧");
    serde_json::from_str(&msg).unwrap()
}

/// 订阅后收到该 topic 推送，形状 {topic, data}。
/// topic 带测试专属后缀：broadcast 通道进程级共享，并行测试互不串扰。
#[tokio::test]
async fn subscribe_then_push_delivers() {
    let url = spawn_ws_server().await;
    let topic = "sessions-t1";
    let (mut sink, mut stream) = connect(&url).await;
    send_control(&mut sink, serde_json::json!({"subscribe": topic})).await;
    aidog_ctx::publish_event(topic, &serde_json::json!({"session_key": "s1", "est_cost": 0.5}));
    let v = next_msg(&mut stream).await;
    assert_eq!(v["topic"], topic);
    assert_eq!(v["data"]["session_key"], "s1");
    assert_eq!(v["data"]["est_cost"], 0.5);
}

/// 未订阅的 topic 不推送。
#[tokio::test]
async fn unsubscribed_topic_not_delivered() {
    let url = spawn_ws_server().await;
    let topic = "sessions-t2";
    let (mut sink, mut stream) = connect(&url).await;
    send_control(&mut sink, serde_json::json!({"subscribe": "other-t2"})).await;
    aidog_ctx::publish_event(topic, &serde_json::json!({"session_key": "s1"}));
    let got = tokio::time::timeout(std::time::Duration::from_millis(300), stream.next()).await;
    assert!(got.is_err(), "未订阅 topic 不得收到推送");
    sink.send(WsMessage::Close(None)).await.ok();
}

/// 退订后停止推送。
#[tokio::test]
async fn unsubscribe_stops_delivery() {
    let url = spawn_ws_server().await;
    let topic = "sessions-t3";
    let (mut sink, mut stream) = connect(&url).await;
    send_control(&mut sink, serde_json::json!({"subscribe": topic})).await;
    aidog_ctx::publish_event(topic, &serde_json::json!({"n": 1}));
    let v = next_msg(&mut stream).await;
    assert_eq!(v["data"]["n"], 1);
    send_control(&mut sink, serde_json::json!({"unsubscribe": topic})).await;
    aidog_ctx::publish_event(topic, &serde_json::json!({"n": 2}));
    let got = tokio::time::timeout(std::time::Duration::from_millis(300), stream.next()).await;
    assert!(got.is_err(), "退订后不得再收到推送");
    sink.send(WsMessage::Close(None)).await.ok();
}

/// apply_control：订阅 / 退订 / 非法 JSON / 键值非字符串。
#[test]
fn apply_control_updates_topic_set() {
    let mut topics = HashSet::new();
    apply_control(r#"{"subscribe":"sessions"}"#, &mut topics);
    assert!(topics.contains("sessions"));
    apply_control(r#"{"unsubscribe":"sessions"}"#, &mut topics);
    assert!(topics.is_empty());
    // 非法 JSON 与非字符串值：静默忽略，不 panic、不改集合
    apply_control("not-json", &mut topics);
    apply_control(r#"{"subscribe":42}"#, &mut topics);
    assert!(topics.is_empty());
    // 一条消息同时订阅两个（单键单值协议；未知键忽略）
    apply_control(r#"{"subscribe":"sessions","unknown":1}"#, &mut topics);
    assert!(topics.contains("sessions"));
}
