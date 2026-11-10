//! 搜索新配置（sources + mode）：启用开关、顺序 / 一起两模式、超时、去重上限、旧配置迁移。
//! 全部打本地假上游（test-no-real-calls.md）；用 tavily / exa 两个 key 服务当「来源」，免去引擎夹具。

use super::*;
use crate::test_search::spawn_stub;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn body_of(urls: &[&str]) -> String {
    let r: Vec<_> = urls.iter().map(|u| json!({"title": format!("t {u}"), "url": u, "content": "c"})).collect();
    json!({ "results": r }).to_string()
}

/// 固定回 `urls` 的假上游，返回 (端口, 命中计数)。
async fn ok_stub(urls: Vec<String>) -> (u16, Arc<AtomicUsize>) {
    let n = Arc::new(AtomicUsize::new(0));
    let n2 = n.clone();
    let port = spawn_stub(Arc::new(move |_m, _p, _h, _b| {
        n2.fetch_add(1, Ordering::SeqCst);
        let refs: Vec<&str> = urls.iter().map(String::as_str).collect();
        (200, "application/json".into(), body_of(&refs))
    }))
    .await;
    (port, n)
}

async fn err_stub() -> (u16, Arc<AtomicUsize>) {
    let n = Arc::new(AtomicUsize::new(0));
    let n2 = n.clone();
    let port = spawn_stub(Arc::new(move |_m, _p, _h, _b| {
        n2.fetch_add(1, Ordering::SeqCst);
        (500, "text/plain".into(), "boom".into())
    }))
    .await;
    (port, n)
}

/// 接住连接但永不应答（超时用）。
async fn hang_stub() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((s, _)) = l.accept().await {
            held.push(s);
        }
    });
    port
}

/// 门控假上游：收到请求后等到 `gate` 达到 `need`（即多个来源都已连上）才应答；顺序执行会永远等不到。
async fn gated_stub(gate: Arc<AtomicUsize>, need: usize, urls: Vec<String>) -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = l.accept().await {
            let gate = gate.clone();
            let urls = urls.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let _ = s.read(&mut buf).await;
                gate.fetch_add(1, Ordering::SeqCst);
                while gate.load(Ordering::SeqCst) < need {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                let refs: Vec<&str> = urls.iter().map(String::as_str).collect();
                let b = body_of(&refs);
                let resp = format!(
                    "HTTP/1.1 200 T\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
                    b.len()
                );
                let _ = s.write_all(resp.as_bytes()).await;
                let _ = s.shutdown().await;
            });
        }
    });
    port
}

fn cfg(sources: &[&str], mode: SearchMode) -> SearchProviderConfig {
    SearchProviderConfig {
        sources: sources.iter().map(|s| s.to_string()).collect(),
        mode,
        services: sources
            .iter()
            .map(|s| (s.to_string(), ServiceConfig { keys: vec![format!("{s}-key")] }))
            .collect(),
        ..Default::default()
    }
}

fn env(bases: &[(&str, u16)], timeout_ms: u64) -> SearchEnv {
    SearchEnv {
        bases: bases.iter().map(|(k, p)| (k.to_string(), format!("http://127.0.0.1:{p}"))).collect(),
        timeout: Duration::from_millis(timeout_ms),
    }
}

fn urls(n: usize, tag: &str) -> Vec<String> {
    (0..n).map(|i| format!("https://{tag}.example/{i}")).collect()
}

// ---------- 启用搜索 ----------

#[tokio::test]
async fn disabled_search_errors_and_calls_no_source() {
    let (p, n) = ok_stub(urls(1, "a")).await;
    let mut c = cfg(&["tavily"], SearchMode::Sequential);
    c.enabled = false;
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", p)], 2000)).await;
    assert!(o.error.as_deref().unwrap().contains("搜索已停用"), "{:?}", o.error);
    assert_eq!(n.load(Ordering::SeqCst), 0, "停用时不得调任何来源");
    assert!(o.engines.is_empty() && o.hits.is_empty());
}

#[tokio::test]
async fn disabled_search_also_stops_engines() {
    // 旧行为：enabled=false 仍落免 key 引擎。引擎 base 不注入——误调会真实外呼。
    let c = SearchProviderConfig { enabled: false, ..Default::default() };
    let o = web_search_detailed(&c, "q", None, &SearchEnv::default()).await;
    assert!(o.error.unwrap().contains("搜索已停用"));
    assert!(o.engines.is_empty());
}

// ---------- 顺序 ----------

#[tokio::test]
async fn sequential_error_switches_to_next_and_records_attempts() {
    let (bad, nb) = err_stub().await;
    let (good, ng) = ok_stub(urls(2, "e")).await;
    let (third, n3) = ok_stub(urls(1, "z")).await;
    let c = cfg(&["tavily", "exa", "brave"], SearchMode::Sequential);
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", bad), ("exa", good), ("brave", third)], 3000)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.hits.len(), 2);
    assert_eq!((nb.load(Ordering::SeqCst), ng.load(Ordering::SeqCst), n3.load(Ordering::SeqCst)), (1, 1, 0), "首个非空即停");
    let ids: Vec<&str> = o.engines.iter().map(|r| r.engine.as_str()).collect();
    assert_eq!(ids, ["tavily", "exa"], "日志只记实际尝试的来源");
    assert!(o.engines[0].error.as_deref().unwrap().contains("500"));
    assert_eq!(o.engines[1].count, 2);
}

#[tokio::test]
async fn sequential_zero_results_switches_to_next() {
    let (empty, _) = ok_stub(vec![]).await;
    let (good, _) = ok_stub(urls(1, "e")).await;
    let c = cfg(&["tavily", "exa"], SearchMode::Sequential);
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", empty), ("exa", good)], 3000)).await;
    assert_eq!(o.hits.len(), 1);
    assert_eq!(o.engines.len(), 2);
    assert!(o.engines[0].error.is_none() && o.engines[0].count == 0);
}

#[tokio::test]
async fn sequential_timeout_switches_to_next() {
    let hang = hang_stub().await;
    let (good, _) = ok_stub(urls(1, "e")).await;
    let c = cfg(&["tavily", "exa"], SearchMode::Sequential);
    let t = std::time::Instant::now();
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", hang), ("exa", good)], 300)).await;
    assert!(t.elapsed() < Duration::from_secs(3));
    assert_eq!(o.hits.len(), 1);
    assert!(o.engines[0].error.as_deref().unwrap().contains("timeout"), "{:?}", o.engines);
}

#[tokio::test]
async fn sequential_all_fail_aggregates_errors() {
    let (a, _) = err_stub().await;
    let (b, _) = err_stub().await;
    let c = cfg(&["tavily", "exa"], SearchMode::Sequential);
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", a), ("exa", b)], 3000)).await;
    let e = o.error.expect("全败必须有 error");
    assert!(e.contains("tavily") && e.contains("exa"), "{e}");
    assert_eq!(o.engines.len(), 2);
}

#[tokio::test]
async fn sequential_all_empty_is_success_with_zero_hits() {
    let (a, _) = ok_stub(vec![]).await;
    let (b, _) = ok_stub(vec![]).await;
    let c = cfg(&["tavily", "exa"], SearchMode::Sequential);
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", a), ("exa", b)], 3000)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert!(o.hits.is_empty());
    assert_eq!(o.engines.len(), 2);
}

#[tokio::test]
async fn unknown_source_id_is_an_error_report_not_a_panic() {
    let (good, _) = ok_stub(urls(1, "e")).await;
    let c = cfg(&["nope", "exa"], SearchMode::Sequential);
    let o = web_search_detailed(&c, "q", None, &env(&[("exa", good)], 3000)).await;
    assert_eq!(o.hits.len(), 1);
    assert!(o.engines[0].error.as_deref().unwrap().contains("未知搜索来源"));
}

#[tokio::test]
async fn keyed_source_uses_services_keys() {
    let seen = Arc::new(std::sync::Mutex::new(String::new()));
    let s2 = seen.clone();
    let port = spawn_stub(Arc::new(move |_m, _p, _h, body| {
        *s2.lock().unwrap() = body;
        (200, "application/json".into(), body_of(&["https://a.example/"]))
    }))
    .await;
    let mut c = cfg(&["tavily"], SearchMode::Sequential);
    c.services.insert("tavily".into(), ServiceConfig { keys: vec!["  ".into(), " real-key ".into()] });
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", port)], 3000)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert!(seen.lock().unwrap().contains("\"api_key\":\"real-key\""), "{}", seen.lock().unwrap());
}

// ---------- 一起 ----------

#[tokio::test]
async fn parallel_runs_concurrently_merges_and_dedups() {
    let gate = Arc::new(AtomicUsize::new(0));
    // 两个来源都连上才应答：顺序执行会死等到超时
    let shared = "https://same.example/x".to_string();
    let a = gated_stub(gate.clone(), 2, vec![shared.clone(), "https://a.example/1".into()]).await;
    let b = gated_stub(gate.clone(), 2, vec![format!("{shared}/"), "https://b.example/1".into()]).await;
    let c = cfg(&["tavily", "exa"], SearchMode::Parallel);
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", a), ("exa", b)], 5000)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.engines.len(), 2, "{:?}", o.engines);
    let got: Vec<&str> = o.hits.iter().map(|h| h.url.as_str()).collect();
    assert_eq!(got.len(), 3, "同 URL（含尾斜杠差异）只留一条: {got:?}");
    assert_eq!(got[0], shared, "交织：先各取第一条");
}

#[tokio::test]
async fn parallel_caps_at_merge_cap_and_count() {
    let (a, _) = ok_stub(urls(8, "a")).await;
    let (b, _) = ok_stub(urls(8, "b")).await;
    let c = cfg(&["tavily", "exa"], SearchMode::Parallel);
    let e = env(&[("tavily", a), ("exa", b)], 3000);
    assert_eq!(web_search_detailed(&c, "q", None, &e).await.hits.len(), MERGE_CAP);
    assert_eq!(web_search_detailed(&c, "q", Some(3), &e).await.hits.len(), 3);
}

#[tokio::test]
async fn parallel_waits_for_all_and_slow_source_times_out_alone() {
    let hang = hang_stub().await;
    let (good, _) = ok_stub(urls(2, "e")).await;
    let c = cfg(&["tavily", "exa"], SearchMode::Parallel);
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", hang), ("exa", good)], 300)).await;
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.hits.len(), 2);
    let slow = o.engines.iter().find(|r| r.engine == "tavily").unwrap();
    assert!(slow.error.as_deref().unwrap().contains("timeout"));
}

#[tokio::test]
async fn parallel_all_fail_is_error() {
    let (a, _) = err_stub().await;
    let (b, _) = err_stub().await;
    let c = cfg(&["tavily", "exa"], SearchMode::Parallel);
    let o = web_search_detailed(&c, "q", None, &env(&[("tavily", a), ("exa", b)], 3000)).await;
    assert!(o.error.unwrap().contains("所有来源都失败"));
}

#[test]
fn per_source_timeout_default_is_15s() {
    assert_eq!(SearchEnv::default().timeout, Duration::from_secs(15));
}

// ---------- 默认值 / serde ----------

#[test]
fn fresh_default_is_default_on_engines_parallel_enabled() {
    let d = SearchProviderConfig::default();
    assert!(d.enabled);
    assert_eq!(d.mode, SearchMode::Parallel);
    let want: Vec<String> = engines::ENGINES.iter().filter(|e| e.default_on).map(|e| e.id.to_string()).collect();
    assert_eq!(d.sources, want);
}

#[test]
fn serde_mode_snake_case_and_missing_mode_is_sequential() {
    let c: SearchProviderConfig = serde_json::from_value(json!({"sources": ["tavily"]})).unwrap();
    assert_eq!(c.mode, SearchMode::Sequential);
    assert_eq!(serde_json::to_value(SearchMode::Parallel).unwrap(), json!("parallel"));
}

// ---------- 旧配置迁移 ----------

fn migrated(mut v: serde_json::Value) -> (bool, SearchProviderConfig, serde_json::Value) {
    let changed = migrate_search_value(&mut v);
    (changed, serde_json::from_value(v.clone()).unwrap(), v)
}

fn default_on_ids() -> Vec<String> {
    engines::ENGINES.iter().filter(|e| e.default_on).map(|e| e.id.to_string()).collect()
}

#[test]
fn migrate_keyed_provider_goes_first_with_keys_then_engines_sequential() {
    let (changed, c, _) = migrated(json!({
        "provider": "brave", "keys": ["k1", " k2 ", ""], "enabled": true,
        "searxng_base_urls": ["http://s"], "engines": {"bing": false, "baidu": true}
    }));
    assert!(changed);
    assert!(c.enabled);
    assert_eq!(c.mode, SearchMode::Sequential);
    assert_eq!(c.sources[0], "brave");
    let want_engines: Vec<String> = engines::ENGINES
        .iter()
        .filter(|e| match e.id {
            "bing" => false,
            "baidu" => true,
            _ => e.default_on,
        })
        .map(|e| e.id.to_string())
        .collect();
    assert_eq!(c.sources[1..], want_engines[..], "引擎按 ENGINES 原序，覆盖项生效");
    assert_eq!(c.services["brave"].keys, vec!["k1".to_string(), "k2".to_string()]);
    assert_eq!(c.searxng_base_urls, vec!["http://s".to_string()]);
}

#[test]
fn migrate_engines_only_becomes_parallel() {
    let (changed, c, v) = migrated(json!({"provider": "", "keys": [], "enabled": true, "engines": {"ddg": false}}));
    assert!(changed);
    assert_eq!(c.mode, SearchMode::Parallel);
    assert!(!c.sources.contains(&"ddg".to_string()));
    assert!(c.services.is_empty());
    assert!(v.get("provider").is_none() && v.get("keys").is_none() && v.get("engines").is_none(), "旧键不再保留: {v}");
}

#[test]
fn migrate_empty_object_equals_fresh_install() {
    let (changed, c, _) = migrated(json!({}));
    assert!(changed);
    assert_eq!(c.sources, default_on_ids());
    assert_eq!(c.mode, SearchMode::Parallel);
    assert!(c.enabled);
}

#[test]
fn migrate_disabled_with_key_and_engines_enables_search_without_keyed_source() {
    // 旧 enabled=false 只关 key 服务，引擎照落 → 保持实际效果：enabled=true，来源只含引擎，key 保留
    let (_, c, _) = migrated(json!({"provider": "tavily", "keys": ["k"], "enabled": false}));
    assert!(c.enabled);
    assert!(!c.sources.contains(&"tavily".to_string()));
    assert_eq!(c.sources, default_on_ids());
    assert_eq!(c.mode, SearchMode::Parallel);
    assert_eq!(c.services["tavily"].keys, vec!["k".to_string()]);
}

#[test]
fn migrate_disabled_with_no_engines_stays_disabled() {
    let all_off: serde_json::Map<String, serde_json::Value> =
        engines::ENGINES.iter().map(|e| (e.id.to_string(), json!(false))).collect();
    let (_, c, _) = migrated(json!({"provider": "tavily", "keys": ["k"], "enabled": false, "engines": all_off}));
    assert!(!c.enabled, "旧效果本就是不可用，不凭空打开");
    assert!(c.sources.is_empty());
    assert_eq!(c.services["tavily"].keys, vec!["k".to_string()]);
}

#[test]
fn migrate_searxng_provider_is_engine_path_without_service() {
    let (_, c, _) = migrated(json!({"provider": "searxng", "keys": ["k"], "searxng_base_urls": ["http://a"]}));
    assert_eq!(c.sources, default_on_ids());
    assert!(c.services.is_empty());
    assert_eq!(c.mode, SearchMode::Parallel);
    assert_eq!(c.searxng_base_urls, vec!["http://a".to_string()]);
}

#[test]
fn migrate_keyed_without_real_keys_is_not_a_source() {
    let (_, c, _) = migrated(json!({"provider": "exa", "keys": ["  "]}));
    assert!(!c.sources.contains(&"exa".to_string()));
    assert!(c.services.is_empty());
    assert_eq!(c.mode, SearchMode::Parallel);
}

#[test]
fn migrate_is_idempotent_and_skips_when_sources_present() {
    let (_, _, mut v) = migrated(json!({"provider": "brave", "keys": ["k"]}));
    let snapshot = v.clone();
    assert!(!migrate_search_value(&mut v), "第二次不再改动");
    assert_eq!(v, snapshot);
    let mut already = json!({"sources": ["ddg"], "mode": "parallel", "provider": "brave", "keys": ["x"]});
    let before = already.clone();
    assert!(!migrate_search_value(&mut already), "已含 sources 键则跳过");
    assert_eq!(already, before);
}

#[test]
fn migrate_non_object_is_untouched() {
    let mut v = json!("garbage");
    assert!(!migrate_search_value(&mut v));
    assert_eq!(v, json!("garbage"));
}
