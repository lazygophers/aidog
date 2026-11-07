//! media 模块单元测试（magpie 票 12）：全走本地假上游，禁真实外呼
//! （test-no-real-calls.md）。断言请求形状（path / 鉴权头 / body 字段）、
//! 出参 markdown 形态与显式错误路径。

use super::*;
use aidog_core::mcp_tools::MediaProviderConfig;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 假上游 handler：收 (method, path, 小写头列表, body)，回 (status, content-type, body)。
type Handler =
    Arc<dyn Fn(String, String, Vec<String>, String) -> (u16, String, String) + Send + Sync>;

/// 起本地假上游，返回端口。请求按「一次读尽（测试请求都小）→ 回一包 → 关连接」处理。
async fn spawn_stub(handler: Handler) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(x) => x,
                Err(_) => return,
            };
            let h = handler.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 65536];
                let n = match sock.read(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => return,
                };
                let text = String::from_utf8_lossy(&buf[..n]);
                let mut parts = text.split("\r\n\r\n");
                let head = parts.next().unwrap_or("").to_string();
                let body = parts.next().unwrap_or("").to_string();
                let mut lines = head.lines();
                let mut it = lines.next().unwrap_or("").split_whitespace();
                let method = it.next().unwrap_or("").to_string();
                let path = it.next().unwrap_or("").to_string();
                let headers: Vec<String> = lines.map(|l| l.to_ascii_lowercase()).collect();
                let (status, ctype, resp_body) = h(method, path, headers, body);
                let resp = format!(
                    "HTTP/1.1 {status} T\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    resp_body.len(),
                    resp_body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    port
}

fn media_cfg(base: &str) -> MediaProviderConfig {
    MediaProviderConfig {
        base_url: base.into(),
        api_key: "sk-img-k1".into(),
        model: "img-model".into(),
    }
}

#[tokio::test]
async fn image_request_shape_and_markdown_url() {
    let port = spawn_stub(Arc::new(|method, path, headers, body| {
        assert_eq!(method, "POST");
        assert_eq!(path, "/images/generations");
        assert!(
            headers.iter().any(|h| h.starts_with("authorization: bearer sk-img-k1")),
            "headers={headers:?}"
        );
        assert!(body.contains("\"model\":\"img-model\""), "body={body}");
        assert!(body.contains("\"prompt\":\"a cat\""), "body={body}");
        assert!(body.contains("\"n\":1"), "body={body}");
        assert!(body.contains("\"size\":\"1024x1024\""), "body={body}");
        (
            200,
            "application/json".into(),
            r#"{"created":1,"data":[{"url":"https://img.example/x.png"}]}"#.into(),
        )
    }))
    .await;
    let out = generate_image(
        Some(&media_cfg(&format!("http://127.0.0.1:{port}"))),
        "a cat",
        Some("1024x1024"),
    )
    .await
    .unwrap();
    assert_eq!(out, "![generated image](https://img.example/x.png)");
}

#[tokio::test]
async fn image_b64_becomes_data_url_markdown() {
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| {
        (
            200,
            "application/json".into(),
            r#"{"data":[{"b64_json":"QUJD"}]}"#.into(),
        )
    }))
    .await;
    let out = generate_image(Some(&media_cfg(&format!("http://127.0.0.1:{port}"))), "p", None)
        .await
        .unwrap();
    assert_eq!(out, "![generated image](data:image/png;base64,QUJD)");
}

#[tokio::test]
async fn image_trailing_slash_base_url_not_duplicated_and_size_omitted() {
    let port = spawn_stub(Arc::new(|_m, path, _h, body| {
        assert_eq!(path, "/images/generations", "尾斜杠不得产生 //images");
        assert!(!body.contains("\"size\""), "未传 size 不得发送该字段, body={body}");
        (
            200,
            "application/json".into(),
            r#"{"data":[{"url":"https://i.example/1.png"}]}"#.into(),
        )
    }))
    .await;
    let mut cfg = media_cfg(&format!("http://127.0.0.1:{port}/"));
    cfg.base_url = cfg.base_url.trim_end_matches('/').to_string() + "/";
    let out = generate_image(Some(&cfg), "p", None).await.unwrap();
    assert!(out.contains("https://i.example/1.png"), "out={out}");
}

#[tokio::test]
async fn image_unconfigured_is_explicit_error() {
    let e = generate_image(None, "p", None).await.unwrap_err();
    assert!(e.contains("未配置图片 provider"), "e={e}");
    // 半配置（空白 key）同样算未配置，不发出站请求。
    let cfg = MediaProviderConfig {
        base_url: "https://x.example/v1".into(),
        api_key: "  ".into(),
        model: "m".into(),
    };
    let e = generate_image(Some(&cfg), "p", None).await.unwrap_err();
    assert!(e.contains("未配置图片 provider"), "e={e}");
}

#[tokio::test]
async fn image_http_error_surfaces_status_and_body() {
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| {
        (
            401,
            "application/json".into(),
            r#"{"error":{"message":"bad key"}}"#.into(),
        )
    }))
    .await;
    let e = generate_image(Some(&media_cfg(&format!("http://127.0.0.1:{port}"))), "p", None)
        .await
        .unwrap_err();
    assert!(e.contains("HTTP 401"), "e={e}");
    assert!(e.contains("bad key"), "e={e}");
}

#[tokio::test]
async fn image_missing_data_array_is_explicit_error() {
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| (200, "application/json".into(), "{}".into())))
        .await;
    let e = generate_image(Some(&media_cfg(&format!("http://127.0.0.1:{port}"))), "p", None)
        .await
        .unwrap_err();
    assert!(e.contains("data"), "e={e}");
}

#[tokio::test]
async fn image_item_without_url_or_b64_is_error() {
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| {
        (200, "application/json".into(), r#"{"data":[{"foo":1}]}"#.into())
    }))
    .await;
    let e = generate_image(Some(&media_cfg(&format!("http://127.0.0.1:{port}"))), "p", None)
        .await
        .unwrap_err();
    assert!(e.contains("url") && e.contains("b64_json"), "e={e}");
}

#[tokio::test]
async fn video_unconfigured_is_explicit_error() {
    let e = generate_video(None, "p").await.unwrap_err();
    assert!(e.contains("未配置视频 provider"), "e={e}");
    let e = generate_video(Some(&MediaProviderConfig::default()), "p").await.unwrap_err();
    assert!(e.contains("未配置视频 provider"), "全空配置也按未配置, e={e}");
}

#[tokio::test]
async fn video_configured_is_not_implemented_error() {
    let cfg = media_cfg("https://v.example/v1");
    let e = generate_video(Some(&cfg), "p").await.unwrap_err();
    assert!(e.contains("未实现"), "e={e}");
    assert!(!e.contains("未配置"), "已配置不得报未配置, e={e}");
}
