//! 免 key 引擎测试：解析器对离线夹具（期望值由用户脚本 websearch.py 的 Python 解析器在同一夹具上产出，
//! 即移植对照基准）+ 并行 / 超时 / 去重 / 优先级编排（本地假引擎，禁真实外呼）。
//!
//! 夹具来源：ddg / bing / github / wikipedia_zh / arxiv = 真实抓取（wreq Chrome 指纹，2026-11-09）；
//! google_jswall_captured = 真实抓取（Google 对非浏览器返回 JS 墙，0 条）；
//! google_handmade / searxng = 手写（按解析器选择器 / SearXNG JSON 形状构造）。

use super::*;
use crate::engines::*;
use crate::test_search::{Handler, spawn_stub};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fx(name: &str) -> String {
    std::fs::read_to_string(format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// 把解析结果与 Python 基准 JSON 逐条比对（title / url / snippet）。
fn assert_matches_oracle(hits: &[SearchHit], expected_file: &str) {
    let want: Vec<serde_json::Value> = serde_json::from_str(&fx(expected_file)).unwrap();
    assert_eq!(hits.len(), want.len(), "{expected_file}: 条数");
    for (i, (h, w)) in hits.iter().zip(&want).enumerate() {
        assert_eq!(h.url, w["url"].as_str().unwrap(), "{expected_file}[{i}] url");
        assert_eq!(h.title, w["title"].as_str().unwrap(), "{expected_file}[{i}] title");
        assert_eq!(h.snippet, w["snippet"].as_str().unwrap(), "{expected_file}[{i}] snippet");
    }
}

#[test]
fn parse_ddg_matches_oracle() {
    let h = parse_ddg(&fx("ddg.html"));
    assert!(!h.is_empty());
    assert_matches_oracle(&h, "ddg.expected.json");
}
#[test]
fn parse_bing_matches_oracle() {
    assert_matches_oracle(&parse_bing(&fx("bing.html")), "bing.expected.json");
}
#[test]
fn parse_google_handmade_matches_oracle_and_skips_google_and_relative() {
    let h = parse_google(&fx("google_handmade.html"));
    assert_matches_oracle(&h, "google_handmade.expected.json");
    assert_eq!(h.len(), 2);
}
#[test]
fn parse_google_js_wall_is_empty() {
    assert!(parse_google(&fx("google_jswall_captured.html")).is_empty());
}
#[test]
fn parse_github_matches_oracle() {
    assert_matches_oracle(&parse_github(&fx("github.html")), "github.expected.json");
}
#[test]
fn parse_wikipedia_matches_oracle() {
    assert_matches_oracle(&parse_wikipedia(&fx("wikipedia_zh.json"), "zh"), "wikipedia_zh.expected.json");
}
#[test]
fn parse_arxiv_matches_oracle() {
    assert_matches_oracle(&parse_arxiv(&fx("arxiv.xml")), "arxiv.expected.json");
}
#[test]
fn parse_searx_matches_oracle() {
    assert_matches_oracle(&parse_searx(&fx("searxng.json")), "searxng.expected.json");
}

// ---------- 编排 ----------

/// 只开指定引擎，其余全关（防止测试误打真实引擎）。
fn only(ids: &[&str]) -> SearchProviderConfig {
    SearchProviderConfig { sources: ids.iter().map(|s| s.to_string()).collect(), mode: SearchMode::Parallel, ..Default::default() }
}

fn env_for(bases: &[(&str, u16)]) -> SearchEnv {
    SearchEnv {
        bases: bases.iter().map(|(k, p)| (k.to_string(), format!("http://127.0.0.1:{p}"))).collect(),
        timeout: Duration::from_secs(5),
    }
}

fn serve(body: String, ctype: &str) -> Handler {
    let ct = ctype.to_string();
    Arc::new(move |_m, _p, _h, _b| (200, ct.clone(), body.clone()))
}

#[test]
fn default_engines_are_the_script_defaults() {
    let on: Vec<&str> = ENGINES.iter().filter(|e| e.default_on).map(|e| e.id).collect();
    let mut on_sorted = on.clone();
    on_sorted.sort();
    assert_eq!(on_sorted, ["arxiv", "bing", "ddg", "github", "google", "searxng", "wikipedia"]);
}

#[tokio::test]
async fn parallel_merge_dedup_cap_and_reports() {
    let p1 = spawn_stub(serve(fx("ddg.html"), "text/html")).await;
    let p2 = spawn_stub(serve(fx("bing.html"), "text/html")).await;
    let o = web_search_detailed(&only(&["ddg", "bing"]), "rust", None, &env_for(&[("ddg", p1), ("bing", p2)])).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.source, "parallel");
    assert_eq!(o.hits.len(), MERGE_CAP);
    let mut urls: Vec<_> = o.hits.iter().map(|h| h.url.clone()).collect();
    urls.sort();
    urls.dedup();
    assert_eq!(urls.len(), MERGE_CAP, "URL 必须去重");
    assert_eq!(o.engines.len(), 2);
    assert!(o.engines.iter().all(|r| r.error.is_none() && r.count == 10));
    // 轮询交织：前两条分别来自两个引擎，不被首个引擎占满
    assert_ne!(o.hits[0].url, o.hits[1].url);
    let ddg_first = parse_ddg(&fx("ddg.html"))[0].url.clone();
    let bing_first = parse_bing(&fx("bing.html"))[0].url.clone();
    assert_eq!(o.hits[0].url, ddg_first);
    assert_eq!(o.hits[1].url, bing_first);
}

#[tokio::test]
async fn count_caps_result_length() {
    let p1 = spawn_stub(serve(fx("ddg.html"), "text/html")).await;
    let o = web_search_detailed(&only(&["ddg"]), "rust", Some(3), &env_for(&[("ddg", p1)])).await;
    assert_eq!(o.hits.len(), 3);
}

#[tokio::test]
async fn failing_engine_is_skipped_and_recorded() {
    let ok = spawn_stub(serve(fx("ddg.html"), "text/html")).await;
    let bad = spawn_stub(Arc::new(|_m, _p, _h, _b| (500, "text/plain".into(), "boom".into()))).await;
    let o = web_search_detailed(&only(&["ddg", "bing"]), "rust", None, &env_for(&[("ddg", ok), ("bing", bad)])).await;
    assert!(o.error.is_none(), "有一个引擎成功即整体成功");
    assert_eq!(o.hits.len(), 10);
    let bing = o.engines.iter().find(|r| r.engine == "bing").unwrap();
    assert!(bing.error.as_deref().unwrap().contains("500"), "{bing:?}");
    assert_eq!(bing.count, 0);
}

#[tokio::test]
async fn all_engines_failing_is_an_error_with_reports() {
    let bad = spawn_stub(Arc::new(|_m, _p, _h, _b| (503, "text/plain".into(), "x".into()))).await;
    let o = web_search_detailed(&only(&["ddg", "bing"]), "rust", None, &env_for(&[("ddg", bad), ("bing", bad)])).await;
    let e = o.error.expect("全失败必须有 error");
    assert!(e.contains("ddg") && e.contains("bing"), "{e}");
    assert_eq!(o.engines.len(), 2);
}

#[tokio::test]
async fn engine_timeout_is_enforced() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((s, _)) = l.accept().await {
            held.push(s); // 接住连接但永不应答
        }
    });
    let mut env = env_for(&[("ddg", port)]);
    env.timeout = Duration::from_millis(300);
    let t = std::time::Instant::now();
    let o = web_search_detailed(&only(&["ddg"]), "rust", None, &env).await;
    assert!(t.elapsed() < Duration::from_secs(3));
    assert!(o.error.is_some());
    assert!(o.engines[0].error.as_deref().unwrap().contains("timeout"), "{:?}", o.engines);
}

#[tokio::test]
async fn no_engine_enabled_is_explicit_unconfigured_error() {
    let o = web_search_detailed(&only(&[]), "rust", None, &SearchEnv::default()).await;
    assert!(o.error.unwrap().contains("未启用任何搜索来源"));
}

#[tokio::test]
async fn wikipedia_falls_back_to_en_when_zh_empty() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let body = fx("wikipedia_zh.json");
    let port = spawn_stub(Arc::new(move |_m, _p, _h, _b| {
        if c.fetch_add(1, Ordering::SeqCst) == 0 {
            (200, "application/json".into(), r#"{"query":{"search":[]}}"#.into())
        } else {
            (200, "application/json".into(), body.clone())
        }
    }))
    .await;
    let o = web_search_detailed(&only(&["wikipedia"]), "Rust", None, &env_for(&[("wikipedia", port)])).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(o.hits.len(), 10);
    assert!(o.hits[0].url.contains("en.wikipedia.org"), "回退 en 时链接用 en 域: {}", o.hits[0].url);
}

#[tokio::test]
async fn searxng_engine_uses_user_instances_and_tries_next_on_failure() {
    let bad = spawn_stub(Arc::new(|_m, _p, _h, _b| (500, "text/plain".into(), "x".into()))).await;
    let good = spawn_stub(Arc::new(|_m, path, _h, _b| {
        assert!(path.starts_with("/search?q=hi&format=json"), "path={path}");
        (200, "application/json".into(), fx("searxng.json"))
    }))
    .await;
    let mut cfg = only(&["searxng"]);
    cfg.searxng_base_urls = vec![format!("http://127.0.0.1:{bad}"), format!("http://127.0.0.1:{good}/")];
    let o = web_search_detailed(&cfg, "hi", None, &SearchEnv::default()).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.hits.len(), 2);
}

#[tokio::test]
async fn searxng_without_instances_reports_error() {
    let o = web_search_detailed(&only(&["searxng"]), "hi", None, &SearchEnv::default()).await;
    assert!(o.error.unwrap().contains("searxng"));
}

#[tokio::test]
async fn keyed_provider_wins_over_engines() {
    let key_port = spawn_stub(Arc::new(|_m, _p, _h, _b| {
        (200, "application/json".into(), r#"{"results":[{"title":"T","url":"https://k.example/","content":"c"}]}"#.into())
    }))
    .await;
    let mut cfg = only(&["tavily", "ddg"]);
    cfg.mode = SearchMode::Sequential;
    cfg.services.insert("tavily".into(), ServiceConfig { keys: vec!["k".into()] });
    // ddg 未注入 base：若误走引擎路径会真实外呼，因此排首位的 key 服务成功后必须停
    let o = web_search_detailed(&cfg, "q", None, &env_for(&[("tavily", key_port)])).await;
    assert_eq!(o.source, "sequential");
    assert_eq!(o.hits.len(), 1);
    assert_eq!(o.engines.len(), 1);
    assert_eq!(o.engines[0].engine, "tavily");
}

#[tokio::test]
async fn unlisted_keyed_service_is_not_called() {
    let p = spawn_stub(serve(fx("ddg.html"), "text/html")).await;
    let mut cfg = only(&["ddg"]);
    // 配了 key 但不在 sources：不调用（tavily 未注入 base，误调会真实外呼）
    cfg.services.insert("tavily".into(), ServiceConfig { keys: vec!["k".into()] });
    let o = web_search_detailed(&cfg, "q", None, &env_for(&[("ddg", p)])).await;
    assert_eq!(o.engines.len(), 1);
    assert_eq!(o.source, "parallel");
    assert_eq!(o.hits.len(), 10);
}

#[test]
fn quote_plus_matches_python() {
    assert_eq!(quote_plus("a b&c/é~*"), "a+b%26c%2F%C3%A9~%2A");
}

#[test]
fn default_config_is_enabled() {
    assert!(SearchProviderConfig::default().enabled, "Default 必须与 serde 缺省一致（true）");
}
