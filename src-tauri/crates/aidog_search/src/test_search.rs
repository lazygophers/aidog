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

fn base(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn deepseek_bearer_and_parse() {
    let port = spawn_stub(Arc::new(|method, path, headers, body| {
        assert_eq!(method, "POST");
        assert_eq!(path, "/v1/tools/search");
        assert!(headers.iter().any(|h| h.starts_with("authorization: bearer ds-k1")), "headers={headers:?}");
        assert!(body.contains("\"query\":\"q\"") && body.contains("\"count\":7"), "body={body}");
        (
            200,
            "application/json".into(),
            r#"{"results":[{"title":"D1","url":"https://d.example/1","snippet":"sn","summary":"long","date":"2026-06-01T00:00:00+00:00"}]}"#.into(),
        )
    }))
    .await;
    let hits = deepseek_search(&http_client().unwrap(), &base(port), "ds-k1", "q", Some(7)).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!((hits[0].title.as_str(), hits[0].url.as_str(), hits[0].snippet.as_str()), ("D1", "https://d.example/1", "sn"));
    assert_eq!(hits[0].page_age.as_deref(), Some("2026-06-01T00:00:00+00:00"));
}

#[tokio::test]
async fn youcom_header_and_parse() {
    let port = spawn_stub(Arc::new(|method, path, headers, body| {
        assert_eq!(method, "POST");
        assert_eq!(path, "/v1/search");
        assert!(headers.iter().any(|h| h.starts_with("x-api-key: yc-k1")), "headers={headers:?}");
        assert!(body.contains("\"count\":3"), "body={body}");
        (
            200,
            "application/json".into(),
            r#"{"results":{"web":[{"title":"Y1","url":"https://y.example/1","snippets":["s0","s1"],"description":"dd","page_age":"2024-12-18T00:00:00"},{"title":"Y2","url":"https://y.example/2","description":"only desc"}]}}"#.into(),
        )
    }))
    .await;
    let hits = youcom_search(&http_client().unwrap(), &base(port), "yc-k1", "q", Some(3)).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].snippet, "s0", "snippets[0] 优先");
    assert_eq!(hits[1].snippet, "only desc", "无 snippets 回落 description");
    assert_eq!(hits[0].page_age.as_deref(), Some("2024-12-18T00:00:00"));
}

#[tokio::test]
async fn doubao_bearer_and_parse() {
    let port = spawn_stub(Arc::new(|method, path, headers, body| {
        assert_eq!(method, "POST");
        assert_eq!(path, "/search_api/web_search");
        assert!(headers.iter().any(|h| h.starts_with("authorization: bearer db-k1")), "headers={headers:?}");
        assert!(body.contains("\"Query\":\"q\"") && body.contains("\"SearchType\":\"web\"") && body.contains("\"Count\":5"), "body={body}");
        (
            200,
            "application/json".into(),
            r#"{"ResponseMetadata":{"RequestId":"r"},"Result":{"ResultCount":2,"WebResults":[{"Title":"B1","Url":"https://b.example/1","Summary":"sum","Snippet":"snip","PublishTime":"2025-05-30T19:35:24+08:00"},{"Title":"B2","Url":"https://b.example/2","Snippet":"snip only"}]}}"#.into(),
        )
    }))
    .await;
    let hits = doubao_search(&http_client().unwrap(), &base(port), "db-k1", "q", Some(5)).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].snippet, "sum", "Summary 优先");
    assert_eq!(hits[1].snippet, "snip only", "无 Summary 回落 Snippet");
    assert_eq!(hits[0].page_age.as_deref(), Some("2025-05-30T19:35:24+08:00"));
}

#[tokio::test]
async fn doubao_http_200_with_error_is_failure() {
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| {
        (
            200,
            "application/json".into(),
            r#"{"ResponseMetadata":{"RequestId":"r","Error":{"CodeN":10406,"Code":"10406","Message":"free quota used up"}},"Result":null}"#.into(),
        )
    }))
    .await;
    let err = doubao_search(&http_client().unwrap(), &base(port), "k", "q", None).await.unwrap_err();
    assert!(err.contains("10406") && err.contains("free quota used up"), "err={err}");
}

fn tencent_body(pages: &[&str]) -> String {
    serde_json::json!({ "Response": { "Pages": pages } }).to_string()
}

#[tokio::test]
async fn tencent_yuanbao_bearer_and_pages_double_parse() {
    let p1 = r#"{"passage":"正文一","score":0.9,"date":"2024-06-07 19:00:51","title":"T1","url":"https://t.example/1","site":"网易","images":[],"favicon":"x"}"#;
    let p2 = r#"{"passage":"正文二","title":"T2","url":"https://t.example/2"}"#;
    let body = tencent_body(&[p1, "not json", p2]);
    let port = spawn_stub(Arc::new(move |method, path, headers, req| {
        assert_eq!(method, "POST");
        assert_eq!(path, "/SearchPro");
        assert!(headers.iter().any(|h| h.starts_with("authorization: bearer tc-k1")), "headers={headers:?}");
        assert!(req.contains("\"Query\":\"q\"") && req.contains("\"Cnt\":20"), "req={req}");
        (200, "application/json".into(), body.clone())
    }))
    .await;
    // count=15 -> Cnt 向上取整到 20（官方枚举 10/20/30/40/50），结果再截到 15
    let hits = tencent_yuanbao_search(&http_client().unwrap(), &base(port), "tc-k1", "q", Some(15)).await.unwrap();
    assert_eq!(hits.len(), 2, "无法解析的单项跳过");
    assert_eq!((hits[0].title.as_str(), hits[0].url.as_str(), hits[0].snippet.as_str()), ("T1", "https://t.example/1", "正文一"));
    assert_eq!(hits[0].page_age.as_deref(), Some("2024-06-07 19:00:51"));
    assert_eq!(hits[1].title, "T2");
}

#[tokio::test]
async fn tencent_yuanbao_all_pages_unparsable_is_error() {
    let body = tencent_body(&["garbage", "{bad"]);
    let port = spawn_stub(Arc::new(move |_m, _p, _h, _b| (200, "application/json".into(), body.clone()))).await;
    let err = tencent_yuanbao_search(&http_client().unwrap(), &base(port), "k", "q", None).await.unwrap_err();
    assert!(err.contains("Pages"), "err={err}");
}

#[tokio::test]
async fn tencent_yuanbao_response_error_is_failure() {
    let port = spawn_stub(Arc::new(|_m, _p, _h, _b| {
        (200, "application/json".into(), r#"{"Response":{"Error":{"Code":"AuthFailure","Message":"bad key"}}}"#.into())
    }))
    .await;
    let err = tencent_yuanbao_search(&http_client().unwrap(), &base(port), "k", "q", None).await.unwrap_err();
    assert!(err.contains("AuthFailure") && err.contains("bad key"), "err={err}");
}

#[tokio::test]
async fn new_keyed_providers_rotate_keys_via_env_bases() {
    // 经 web_search_detailed + SearchEnv.bases：首 key 401 -> 换第二个 key 成功（四家共用 search_with_keys）。
    for (p, ok_body) in [
        ("deepseek", r#"{"results":[{"title":"t","url":"https://e/1","snippet":"s"}]}"#),
        ("youcom", r#"{"results":{"web":[{"title":"t","url":"https://e/1","description":"s"}]}}"#),
        ("doubao", r#"{"ResponseMetadata":{},"Result":{"WebResults":[{"Title":"t","Url":"https://e/1","Snippet":"s"}]}}"#),
        ("tencent_yuanbao", r#"{"Response":{"Pages":["{\"title\":\"t\",\"url\":\"https://e/1\",\"passage\":\"s\"}"]}}"#),
    ] {
        let ok = ok_body.to_string();
        let port = spawn_stub(Arc::new(move |_m, _p, headers, _b| {
            if headers.iter().any(|h| h.ends_with("good")) { (200, "application/json".into(), ok.clone()) } else { (401, "application/json".into(), "no".into()) }
        }))
        .await;
        let env = SearchEnv { bases: [(p.to_string(), base(port))].into(), ..Default::default() };
        let out = web_search_detailed(&cfg(p, &["bad", "good"]), "q", None, &env).await;
        assert!(out.error.is_none(), "provider {p}: {:?}", out.error);
        assert_eq!(out.hits.len(), 1, "provider {p}");
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
