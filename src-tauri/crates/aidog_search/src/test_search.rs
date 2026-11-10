//! search 模块单元测试（magpie 票 11）：全 provider 打本地假上游，禁真实外呼
//! （test-no-real-calls.md）。断言请求形状（path / 鉴权头 / body 字段）与响应解析。

use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 假上游 handler：收 (method, path, 小写头列表, body)，回 (status, content-type, body)。
pub(crate) type Handler =
    Arc<dyn Fn(String, String, Vec<String>, String) -> (u16, String, String) + Send + Sync>;

/// 起本地假上游，返回端口。请求按「一次读尽（测试请求都小）→ 回一包 → 关连接」处理。
pub(crate) async fn spawn_stub(handler: Handler) -> u16 {
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

fn cfg(provider: &str, keys: &[&str]) -> SearchProviderConfig {
    SearchProviderConfig {
        sources: vec![provider.into()],
        services: [(provider.to_string(), ServiceConfig { keys: keys.iter().map(|k| k.to_string()).collect() })].into(),
        mode: SearchMode::Sequential,
        ..Default::default()
    }
}

#[tokio::test]
async fn tavily_request_shape_and_parse() {
    let port = spawn_stub(Arc::new(|method, path, headers, body| {
        assert_eq!(method, "POST");
        assert_eq!(path, "/search");
        assert!(body.contains("\"query\":\"rust async\""), "body={body}");
        assert!(body.contains("\"api_key\":\"tvly-k1\""), "api_key in body, body={body}");
        assert!(headers.iter().any(|h| h.starts_with("content-type: application/json")));
        (
            200,
            "application/json".into(),
            r#"{"results":[{"title":"Rust","url":"https://rust-lang.org","content":"systems lang"},{"title":"Tokio","url":"https://tokio.rs","content":"async runtime"}]}"#.into(),
        )
    }))
    .await;
    let hits = tavily_search(&http_client().unwrap(), &format!("http://127.0.0.1:{port}"), "tvly-k1", "rust async", Some(5)).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].title, "Rust");
    assert_eq!(hits[0].url, "https://rust-lang.org");
    assert_eq!(hits[0].snippet, "systems lang");
}

#[tokio::test]
async fn brave_header_and_parse() {
    let port = spawn_stub(Arc::new(|method, path, headers, _body| {
        assert_eq!(method, "GET");
        assert!(path.starts_with("/res/v1/web/search?q=rust+async"), "path={path}");
        assert!(headers.iter().any(|h| h.starts_with("x-subscription-token: brave-k1")), "headers={headers:?}");
        (
            200,
            "application/json".into(),
            r#"{"web":{"results":[{"title":"B1","url":"https://b.example/1","description":"desc one"}]}}"#.into(),
        )
    }))
    .await;
    let hits = brave_search(&http_client().unwrap(), &format!("http://127.0.0.1:{port}"), "brave-k1", "rust async", None).await.unwrap();
    assert_eq!(hits[0].title, "B1");
    assert_eq!(hits[0].snippet, "desc one");
}

#[tokio::test]
async fn exa_header_and_parse() {
    let port = spawn_stub(Arc::new(|method, path, headers, body| {
        assert_eq!(method, "POST");
        assert_eq!(path, "/search");
        assert!(headers.iter().any(|h| h.starts_with("x-api-key: exa-k1")));
        assert!(body.contains("\"numResults\":3"), "body={body}");
        (
            200,
            "application/json".into(),
            r#"{"results":[{"title":"E1","url":"https://e.example/1","text":"body text"},{"title":"E2","url":"https://e.example/2"}]}"#.into(),
        )
    }))
    .await;
    let hits = exa_search(&http_client().unwrap(), &format!("http://127.0.0.1:{port}"), "exa-k1", "q", Some(3)).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].snippet, "body text");
    assert_eq!(hits[1].snippet, "", "无 text 字段时 snippet 留空");
}

#[tokio::test]
async fn firecrawl_bearer_and_parse() {
    let port = spawn_stub(Arc::new(|method, path, headers, body| {
        assert_eq!(method, "POST");
        assert_eq!(path, "/v1/search");
        assert!(headers.iter().any(|h| h.starts_with("authorization: bearer fc-k1")));
        assert!(body.contains("\"limit\":4"), "body={body}");
        (
            200,
            "application/json".into(),
            r##"{"success":true,"data":[{"title":"F1","url":"https://f.example/1","description":"d1"},{"title":"F2","url":"https://f.example/2","markdown":"# md only"}]}"##.into(),
        )
    }))
    .await;
    let hits = firecrawl_search(&http_client().unwrap(), &format!("http://127.0.0.1:{port}"), "fc-k1", "q", Some(4)).await.unwrap();
    assert_eq!(hits[0].snippet, "d1");
    assert_eq!(hits[1].snippet, "# md only", "description 缺失时回落 markdown");
}

#[tokio::test]
async fn rotation_tries_all_keys_until_success() {
    let tries = Arc::new(AtomicUsize::new(0));
    let t2 = tries.clone();
    let port = spawn_stub(Arc::new(move |_m, _p, headers, body| {
        t2.fetch_add(1, Ordering::SeqCst);
        let good = body.contains("\"api_key\":\"good\"") || headers.iter().any(|h| h.ends_with("good"));
        if good { (200, "application/json".into(), r#"{"results":[]}"#.into()) } else { (401, "application/json".into(), "unauthorized".into()) }
    }))
    .await;
    let hits = search_with_keys("tavily", &format!("http://127.0.0.1:{port}"), vec!["a1".into(), "a2".into(), "good".into()], "q", None).await;
    assert!(hits.is_ok(), "轮换到好 key 应成功");
    // 轮换起点是进程级游标，与同二进制里其他用到 search_with_keys 的测试并行时起点不定：
    // 好 key 在第 1..=3 次必被试到，精确次数不assert（聚合路径由 all_keys_fail 测试覆盖）。
    let n = tries.load(Ordering::SeqCst);
    assert!((1..=3).contains(&n), "轮换应逐 key 尝试直至命中，实得 {n} 次");
}

#[tokio::test]
async fn rotation_aggregates_errors_when_all_keys_fail() {
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| (401, "application/json".into(), "unauthorized".into()))).await;
    let err = search_with_keys("tavily", &format!("http://127.0.0.1:{port}"), vec!["k1".into(), "k2".into()], "q", None).await.unwrap_err();
    assert!(err.contains("全部 2 个 key 失败"), "err={err}");
    assert!(err.contains("key#1"), "err={err}");
    assert!(err.contains("key#2"), "err={err}");
    assert!(err.contains("401"), "err={err}");
}

#[tokio::test]
async fn unverified_providers_return_explicit_error() {
    for p in ["youcom", "deepseek", "doubao", "tencent_yuanbao"] {
        let sc = cfg(p, &["k"]);
        let err = web_search(&sc, "q", None).await.unwrap_err();
        assert!(err.contains("unverified"), "provider {p}: err={err}");
    }
}

#[tokio::test]
async fn nothing_configured_and_all_engines_off_is_explicit() {
    let sc = SearchProviderConfig { sources: vec![], ..Default::default() };
    let err = web_search(&sc, "q", None).await.unwrap_err();
    assert!(err.contains("未启用任何搜索来源"), "err={err}");
}

#[tokio::test]
async fn fetch_converts_html_to_text() {
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| {
        (
            200,
            "text/html; charset=utf-8".into(),
            "<html><head><style>.x{color:red}</style></head><body><h1>Title&nbsp;A</h1><script>alert(1)</script><p>Hello &amp; welcome to &lt;aidog&gt;</p><br/>line2</body></html>".into(),
        )
    }))
    .await;
    let text = fetch_url_text(&format!("http://127.0.0.1:{port}/page")).await.unwrap();
    assert!(text.contains("Title A"), "text={text}");
    assert!(text.contains("Hello & welcome to <aidog>"), "text={text}");
    assert!(text.contains("line2"), "text={text}");
    assert!(!text.contains("alert(1)"), "script 内容必须剥掉: {text}");
    assert!(!text.contains("color:red"), "style 内容必须剥掉: {text}");
    // 原生标签剥干净（解码出的 &lt;aidog&gt; 合法含 '<'，不在此列）
    assert!(!text.contains("<p>") && !text.contains("<h1>") && !text.contains("<br"), "标签必须剥干净: {text}");
}

#[tokio::test]
async fn fetch_truncates_over_1mb() {
    let big = format!("<p>{}</p>", "x".repeat(2 * 1024 * 1024));
    let port = spawn_stub(Arc::new(move |_m, _p, _h, _b| (200, "text/html".into(), big.clone()))).await;
    let text = fetch_url_text(&format!("http://127.0.0.1:{port}/big")).await.unwrap();
    assert!(text.chars().count() < 2 * 1024 * 1024, "必须截断");
    assert!(text.contains("[内容超过 1MB，已截断]"), "text 尾部={}", &text[text.len().saturating_sub(120)..]);
}

#[tokio::test]
async fn fetch_rejects_non_http_scheme_and_http_error() {
    assert!(fetch_url_text("ftp://x/file").await.unwrap_err().contains("http/https"));
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| (404, "text/plain".into(), "nope".into()))).await;
    assert!(fetch_url_text(&format!("http://127.0.0.1:{port}/x")).await.unwrap_err().contains("404"));
}

#[test]
fn html_to_text_handles_numeric_entities_and_plain_text() {
    assert_eq!(html_to_text("<p>&#39;x&#x27;</p>"), "'x'");
    assert_eq!(html_to_text("plain"), "plain");
    assert_eq!(html_to_text(""), "");
}

#[test]
fn format_shapes_hits_and_empty() {
    assert_eq!(format_hits(&[]), "（无结果）");
    let s = format_hits(&[SearchHit { title: "T".into(), url: "https://u".into(), snippet: "s".into(), page_age: None }]);
    assert!(s.contains("1. T") && s.contains("https://u") && s.contains('s'), "s={s}");
}
