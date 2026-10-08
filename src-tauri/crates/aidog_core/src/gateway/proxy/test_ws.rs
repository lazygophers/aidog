// ws.rs 纯函数层单测（票 03）：入口判定 / scheme 换算 / 握手头剥除注入。
// 泵与上游连接的 e2e 属真机验收（见 spec memory 遗留），不在此层覆盖。
use super::*;

fn headers(pairs: &[(&str, &str)]) -> axum::http::HeaderMap {
    let mut h = axum::http::HeaderMap::new();
    for (k, v) in pairs {
        h.insert(
            axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            axum::http::HeaderValue::from_str(v).unwrap(),
        );
    }
    h
}

#[test]
fn ws_upgrade_detection_requires_get_and_upgrade_header() {
    let get = axum::http::Method::GET;
    let post = axum::http::Method::POST;
    let ws = headers(&[("upgrade", "websocket")]);
    let ws_mixed = headers(&[("upgrade", "WebSocket")]);
    let plain = headers(&[("accept", "text/event-stream")]);
    assert!(is_ws_upgrade(&get, &ws));
    assert!(is_ws_upgrade(&get, &ws_mixed));
    assert!(!is_ws_upgrade(&post, &ws), "非 GET 不算 WS");
    assert!(!is_ws_upgrade(&get, &plain), "无 Upgrade 头不算 WS");
}

#[test]
fn http_url_converts_to_ws_scheme() {
    assert_eq!(to_ws_url("https://api.example.com"), "wss://api.example.com");
    assert_eq!(to_ws_url("http://127.0.0.1:9890/v1"), "ws://127.0.0.1:9890/v1");
    assert_eq!(to_ws_url("wss://api.example.com/x"), "wss://api.example.com/x");
    assert_eq!(to_ws_url("ws://h/p"), "ws://h/p");
}

#[test]
fn handshake_headers_strip_negotiation_and_swap_auth() {
    let orig = headers(&[
        ("host", "proxy.local"),
        ("connection", "Upgrade"),
        ("upgrade", "websocket"),
        ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
        ("sec-websocket-version", "13"),
        ("authorization", "Bearer group-secret"),
        ("x-api-key", "group-secret"),
        ("x-custom-trace", "t1"),
    ]);
    let inject = Some(("Authorization".to_string(), "Bearer upstream-key".to_string()));
    let hm = ws_upstream_handshake_headers(&orig, &inject);

    assert!(hm.get("host").is_none(), "host 剥除（上游按 URL 自生成）");
    assert!(hm.get("connection").is_none());
    assert!(hm.get("upgrade").is_none());
    assert!(hm.get("sec-websocket-key").is_none());
    assert!(hm.get("x-api-key").is_none(), "入站认证头剥除");
    assert_eq!(
        hm.get("authorization").unwrap(),
        "Bearer upstream-key",
        "注入平台配置认证头"
    );
    assert_eq!(hm.get("x-custom-trace").unwrap(), "t1", "非协商头保留");
}

#[test]
fn handshake_headers_without_inject_still_strip_inbound_auth() {
    let orig = headers(&[
        ("authorization", "Bearer group-secret"),
        ("x-goog-api-key", "group-secret"),
        ("accept-language", "zh"),
    ]);
    let hm = ws_upstream_handshake_headers(&orig, &None);
    assert!(hm.get("authorization").is_none());
    assert!(hm.get("x-goog-api-key").is_none(), "客户端 group key 不外泄上游");
    assert_eq!(hm.get("accept-language").unwrap(), "zh");
}
