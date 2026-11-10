//! 票 02：其余 7 个免 key 引擎（ddg_lite / yandex / sogou / so360 / baidu / crossref / pubmed）+ searx.space 手动刷新。
//! 解析器对 Python 脚本 websearch.py 在同一夹具上产出的基准；跳转解析 / 刷新走本地假上游，禁真实外呼。
//!
//! 夹具来源：sogou / so360 / baidu / crossref / pubmed_* / searx_space = 真实抓取（2026-11-09，searx_space 裁成 5 个实例）；
//! yandex_captcha_captured = 真实抓取（对非浏览器 IP 返回验证码页，0 条）；ddg_lite_handmade / yandex_handmade = 手写
//! （DDG Lite 对抓取返回 HTTP 202 质询页，Yandex 同被验证码拦，按脚本解析器选择器构造）。

use super::*;
use crate::engines::*;
use crate::searx_space::*;
use crate::test_search::{Handler, spawn_stub};
use std::sync::Arc;

fn fx(name: &str) -> String {
    std::fs::read_to_string(format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn assert_matches_oracle(hits: &[SearchHit], expected_file: &str) {
    let want: Vec<serde_json::Value> = serde_json::from_str(&fx(expected_file)).unwrap();
    assert_eq!(hits.len(), want.len(), "{expected_file}: 条数");
    for (i, (h, w)) in hits.iter().zip(&want).enumerate() {
        assert_eq!(h.url, w["url"].as_str().unwrap(), "{expected_file}[{i}] url");
        assert_eq!(h.title, w["title"].as_str().unwrap(), "{expected_file}[{i}] title");
        assert_eq!(h.snippet, w["snippet"].as_str().unwrap(), "{expected_file}[{i}] snippet");
    }
}

fn only(id: &str) -> SearchProviderConfig {
    SearchProviderConfig { sources: vec![id.to_string()], ..Default::default() }
}

fn env_for(id: &str, port: u16) -> SearchEnv {
    SearchEnv {
        bases: [(id.to_string(), format!("http://127.0.0.1:{port}"))].into(),
        timeout: Duration::from_secs(5),
    }
}

fn serve(body: String, ctype: &str) -> Handler {
    let ct = ctype.to_string();
    Arc::new(move |_m, _p, _h, _b| (200, ct.clone(), body.clone()))
}

#[test]
fn new_engines_are_registered_default_off() {
    for id in ["ddg_lite", "yandex", "sogou", "so360", "baidu", "crossref", "pubmed"] {
        let e = ENGINES.iter().find(|e| e.id == id).unwrap_or_else(|| panic!("{id} 未注册"));
        assert!(!e.default_on, "{id} 必须默认关");
    }
}

#[test]
fn parse_ddg_lite_matches_oracle() {
    let h = parse_ddg_lite(&fx("ddg_lite_handmade.html"));
    assert_eq!(h.len(), 3, "第 3 条 /y.js 非 http 须跳过");
    assert_matches_oracle(&h, "ddg_lite_handmade.expected.json");
    assert_eq!(h[0].url, "https://www.rust-lang.org/", "uddg 须解包");
}
#[test]
fn parse_yandex_matches_oracle() {
    let h = parse_yandex(&fx("yandex_handmade.html"));
    assert_eq!(h.len(), 2);
    assert_matches_oracle(&h, "yandex_handmade.expected.json");
}
#[test]
fn parse_yandex_captcha_is_empty() {
    assert!(parse_yandex(&fx("yandex_captcha_captured.html")).is_empty());
}
#[test]
fn parse_sogou_matches_oracle_keeping_raw_link_hrefs() {
    let h = parse_sogou(&fx("sogou.html"));
    assert!(h.len() >= 5);
    assert_matches_oracle(&h, "sogou.expected.json");
    assert!(h.iter().any(|x| x.url.starts_with("/link")), "解析层保留 /link 跳转，由引擎解真 URL");
}
#[test]
fn parse_so360_matches_oracle() {
    let h = parse_so360(&fx("so360.html"));
    assert!(!h.is_empty());
    assert_matches_oracle(&h, "so360.expected.json");
    assert!(h.iter().all(|x| !x.url.starts_with("https://www.so.com/link")));
}
#[test]
fn parse_baidu_matches_oracle() {
    let h = parse_baidu(&fx("baidu.html"));
    assert!(!h.is_empty());
    assert_matches_oracle(&h, "baidu.expected.json");
    assert!(h.iter().all(|x| x.snippet.chars().count() <= 250));
}
#[test]
fn parse_crossref_matches_oracle() {
    let h = parse_crossref(&fx("crossref.json"));
    assert_eq!(h.len(), 10);
    assert_matches_oracle(&h, "crossref.expected.json");
}
#[test]
fn parse_pubmed_matches_oracle() {
    let h = parse_pubmed(&fx("pubmed_esearch.json"), &fx("pubmed_esummary.json"));
    assert_eq!(h.len(), 10);
    assert_matches_oracle(&h, "pubmed.expected.json");
    assert!(!h[0].title.ends_with('.'));
}

/// 注入 `Location` 头：假上游 handler 只回 content-type，借此把额外头拼进响应。
fn redirect_to(loc: &str) -> (u16, String, String) {
    (302, format!("text/plain\r\nLocation: {loc}"), String::new())
}

#[tokio::test]
async fn sogou_resolves_link_redirects_via_meta_refresh() {
    let page = serve(fx("sogou.html"), "text/html");
    let port = spawn_stub(Arc::new(move |m, p, h, b| {
        if p.starts_with("/link?url=") {
            return (200, "text/html".into(), "<html><script>window.location.replace(\"https://real.example/x\")</script><noscript><META http-equiv=\"refresh\" content=\"0;URL='https://real.example/x'\"></noscript></html>".into());
        }
        assert!(p.starts_with("/web?query="), "{p}");
        page(m, p, h, b)
    }))
    .await;
    let o = web_search_detailed(&only("sogou"), "rust", None, &env_for("sogou", port)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert!(o.hits.iter().all(|h| h.url.starts_with("http")), "未解出的 /link 须丢弃");
    assert!(o.hits.iter().any(|h| h.url == "https://real.example/x"), "{:?}", o.hits);
    assert!(o.hits.iter().any(|h| h.url.contains("baike.sogou.com")), "已是真链接的原样保留");
}

#[tokio::test]
async fn baidu_resolves_link_redirects_only_for_link_urls_and_caps_at_limit() {
    let page = fx("baidu.html");
    let port = spawn_stub(Arc::new(move |_m, p, _h, _b| {
        if p.starts_with("/link?url=") {
            return redirect_to("https://real.example/baidu");
        }
        assert!(p.starts_with("/s?wd="), "{p}");
        (200, "text/html".into(), page.clone())
    }))
    .await;
    let o = web_search_detailed(&only("baidu"), "rust", Some(4), &env_for("baidu", port)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    // 脚本先 [:limit] 再解跳转；/baidu.php（广告）不是 link? 形态，保持原 URL
    assert!(!o.hits.is_empty() && o.hits.len() <= 4);
    assert!(o.hits.iter().any(|h| h.url == "https://real.example/baidu"), "{:?}", o.hits);
    assert!(o.hits.iter().all(|h| !h.url.contains("baidu.com/link?")), "link? 须全部解出");
}

#[tokio::test]
async fn baidu_unresolvable_link_keeps_original_url_like_script() {
    let page = fx("baidu.html");
    let port = spawn_stub(Arc::new(move |_m, p, _h, _b| {
        if p.starts_with("/link?url=") {
            return (200, "text/html".into(), "no location".into());
        }
        (200, "text/html".into(), page.clone())
    }))
    .await;
    let o = web_search_detailed(&only("baidu"), "rust", Some(3), &env_for("baidu", port)).await;
    // 脚本行为：Location 缺失时保留原跳转 URL（仍是 http），不丢条目
    assert!(o.hits.iter().any(|h| h.url.contains("baidu.com/link?")), "{:?}", o.hits);
}

#[tokio::test]
async fn so360_and_ddg_lite_and_yandex_engines_hit_expected_paths() {
    for (id, body, prefix, ctype) in [
        ("so360", fx("so360.html"), "/s?q=", "text/html"),
        ("ddg_lite", fx("ddg_lite_handmade.html"), "/lite/?q=", "text/html"),
        ("yandex", fx("yandex_handmade.html"), "/search/?text=", "text/html"),
    ] {
        let port = spawn_stub(Arc::new(move |_m, p, _h, _b| {
            assert!(p.starts_with(prefix), "{id}: {p}");
            (200, ctype.into(), body.clone())
        }))
        .await;
        let o = web_search_detailed(&only(id), "rust", None, &env_for(id, port)).await;
        assert!(o.error.is_none() && !o.hits.is_empty(), "{id}: {:?}", o.error);
    }
}

#[tokio::test]
async fn crossref_engine_requests_works_api_and_parses() {
    let port = spawn_stub(Arc::new(|_m, p, _h, _b| {
        assert!(p.starts_with("/works?"), "{p}");
        assert!(p.contains("query=rust") && p.contains("rows=10") && p.contains("select=title"), "{p}");
        (200, "application/json".into(), fx("crossref.json"))
    }))
    .await;
    let o = web_search_detailed(&only("crossref"), "rust", None, &env_for("crossref", port)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.hits.len(), 10);
}

#[tokio::test]
async fn pubmed_engine_chains_esearch_then_esummary() {
    let port = spawn_stub(Arc::new(|_m, p, _h, _b| {
        if p.starts_with("/entrez/eutils/esearch.fcgi?") {
            assert!(p.contains("db=pubmed") && p.contains("term=crispr") && p.contains("retmax=10"), "{p}");
            (200, "application/json".into(), fx("pubmed_esearch.json"))
        } else if p.starts_with("/entrez/eutils/esummary.fcgi?") {
            assert!(p.contains("db=pubmed") && p.contains("id="), "{p}");
            (200, "application/json".into(), fx("pubmed_esummary.json"))
        } else {
            panic!("unexpected {p}")
        }
    }))
    .await;
    let o = web_search_detailed(&only("pubmed"), "crispr", None, &env_for("pubmed", port)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.hits.len(), 10);
    assert!(o.hits[0].url.starts_with("https://pubmed.ncbi.nlm.nih.gov/"));
}

#[tokio::test]
async fn pubmed_empty_idlist_is_empty_not_error() {
    let port = spawn_stub(Arc::new(|_m, p, _h, _b| {
        assert!(p.starts_with("/entrez/eutils/esearch.fcgi?"), "无 id 时不应再调 esummary: {p}");
        (200, "application/json".into(), r#"{"esearchresult":{"idlist":[]}}"#.into())
    }))
    .await;
    let o = web_search_detailed(&only("pubmed"), "zzzz", None, &env_for("pubmed", port)).await;
    assert!(o.hits.is_empty());
}

// ---------- searx.space 手动刷新 ----------

#[test]
fn healthy_instances_match_oracle_order_and_filter() {
    let got: Vec<String> = healthy_instances(&fx("searx_space.json")).into_iter().map(|i| i.url).collect();
    let want: Vec<String> = serde_json::from_str(&fx("searx_space.expected.json")).unwrap();
    assert_eq!(got, want, "https 且 http 200 且搜索成功率>=80%，按耗时升序");
}

#[test]
fn healthy_instances_carry_search_seconds() {
    let v = healthy_instances(&fx("searx_space.json"));
    assert!(v[0].seconds.is_some_and(|s| s > 0.0), "{v:?}");
    let key = |i: &SearxInstance| i.seconds.unwrap_or(9999.0);
    assert!(v.windows(2).all(|w| key(&w[0]) <= key(&w[1])), "未知耗时排最后: {v:?}");
}

#[tokio::test]
async fn fetch_searx_instances_pulls_once_from_given_url() {
    let hits = Arc::new(AtomicUsize::new(0));
    let c = hits.clone();
    let port = spawn_stub(Arc::new(move |_m, p, _h, _b| {
        c.fetch_add(1, Ordering::SeqCst);
        assert_eq!(p, "/data/instances.json");
        (200, "application/json".into(), fx("searx_space.json"))
    }))
    .await;
    let got = fetch_searx_instances(&format!("http://127.0.0.1:{port}/data/instances.json")).await.unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn fetch_searx_instances_errors_on_http_failure_and_empty_healthy() {
    let bad = spawn_stub(Arc::new(|_m, _p, _h, _b| (500, "text/plain".into(), "x".into()))).await;
    let e = fetch_searx_instances(&format!("http://127.0.0.1:{bad}/")).await.unwrap_err();
    assert!(e.contains("500"), "{e}");
    let empty = spawn_stub(Arc::new(|_m, _p, _h, _b| (200, "application/json".into(), r#"{"instances":{}}"#.into()))).await;
    let e = fetch_searx_instances(&format!("http://127.0.0.1:{empty}/")).await.unwrap_err();
    assert!(e.contains("无健康实例"), "{e}");
}

/// 「从不自动拉取」：拉取函数与 searx.space 地址只允许出现在白名单文件（定义处 + 用户点击触发的管理命令）。
/// 任何定时器 / 启动流程里新增调用都会让此测试变红。
#[test]
fn searx_space_is_only_pulled_by_the_manual_command() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let allowed = ["aidog_search/src/searx_space.rs", "aidog_core/src/proxy_cmd/search.rs"];
    let mut offenders = Vec::new();
    let mut stack = vec![crates.clone()];
    while let Some(d) = stack.pop() {
        for ent in std::fs::read_dir(&d).unwrap().flatten() {
            let p = ent.path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if p.is_dir() {
                if name != "target" && name != "node_modules" && name != "fixtures" {
                    stack.push(p);
                }
            } else if name.ends_with(".rs") && !name.starts_with("test_") && !name.ends_with("_test.rs") {
                let rel = p.strip_prefix(&crates).unwrap().to_string_lossy().replace('\\', "/");
                let src = std::fs::read_to_string(&p).unwrap();
                if (src.contains("fetch_searx_instances") || src.contains("searx.space/data"))
                    && !allowed.iter().any(|a| rel.ends_with(a))
                {
                    offenders.push(rel);
                }
            }
        }
    }
    // 本测试文件以 test_ 开头已被排除；lib.rs 仅 `pub mod` 声明不含字面量
    assert!(offenders.is_empty(), "非白名单文件引用了 searx.space 拉取: {offenders:?}");
    let cmd = crates.join("aidog_core/src/proxy_cmd/search.rs");
    assert!(cmd.exists(), "手动刷新命令文件缺失");
}
