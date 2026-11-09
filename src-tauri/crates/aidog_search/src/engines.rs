//! 免 key 搜索引擎（网关-网页搜索 票 01）：解析器逐个对照用户脚本
//! `/Users/luoxin/persons/scripts/lib/websearch.py` 移植，出站走浏览器指纹客户端（wreq）。
//!
//! 引擎注册表 [`ENGINES`] 是唯一清单：加引擎 = 写一个 `e_xxx` + 一条注册项，设置 / 并行 / 合并无需改。

use crate::SearchHit;
use base64::Engine as _;
use percent_encoding::percent_decode_str;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::sync::OnceLock;
use url::Url;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::time::Duration;

/// 引擎运行环境：指纹客户端、测试注入的 base 覆盖、用户的 SearXNG 实例、单引擎超时。
pub struct EngineCtx {
    pub bases: HashMap<String, String>,
    pub searxng_urls: Vec<String>,
    pub timeout: Duration,
}

pub type EngineFn =
    for<'a> fn(&'a EngineCtx, &'a str, usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>>;

/// 一个免 key 引擎的注册项。
pub struct EngineDef {
    pub id: &'static str,
    pub default_on: bool,
    pub run: EngineFn,
}

/// 免 key 引擎注册表（顺序即合并时的轮询顺序，对照脚本 ENGINES）。`default_on` 对照脚本 DEFAULT_ENGINES。
pub const ENGINES: &[EngineDef] = &[
    EngineDef { id: "ddg", default_on: true, run: e_ddg },
    EngineDef { id: "bing", default_on: true, run: e_bing },
    EngineDef { id: "google", default_on: true, run: e_google },
    EngineDef { id: "arxiv", default_on: true, run: e_arxiv },
    EngineDef { id: "searxng", default_on: true, run: e_searxng },
    EngineDef { id: "wikipedia", default_on: true, run: e_wikipedia },
    EngineDef { id: "github", default_on: true, run: e_github },
];

impl EngineCtx {
    /// 引擎 base：测试注入优先，否则取默认（含 scheme+host，无尾斜杠）。
    fn base_or(&self, id: &str, default: impl FnOnce() -> String) -> String {
        match self.bases.get(id) {
            Some(b) => b.trim_end_matches('/').to_string(),
            None => default(),
        }
    }
}

/// 脚本 `quote_plus`：保留 `[A-Za-z0-9_.-~]`，空格转 `+`，其余 %XX 大写。
pub fn quote_plus(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ---------- 出站（浏览器指纹） ----------

/// 浏览器指纹客户端（仅免 key 引擎抓取用；代理转发不经此）。
fn client() -> Result<&'static wreq::Client, String> {
    static C: OnceLock<Result<wreq::Client, String>> = OnceLock::new();
    C.get_or_init(|| {
        wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome149)
            .build()
            .map_err(|e| format!("build fingerprint client: {e}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

const HTML_HEADERS: [(&str, &str); 2] = [
    ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    ("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8"),
];

async fn get_text(url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
    let mut req = client()?.get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let resp = req.send().await.map_err(|e| format!("request: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    resp.text().await.map_err(|e| format!("read body: {e}"))
}

// ---------- 解析器（对照 websearch.py parse_*） ----------

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("static selector")
}

/// 脚本 `_text`：各文本节点 strip 后以空格连接，再压空白。
fn text_of(el: ElementRef) -> String {
    el.text()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn norm_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hit(url: String, title: String, snippet: String) -> SearchHit {
    SearchHit { title, url, snippet, page_age: None }
}

/// DDG 结果链接是 //duckduckgo.com/l/?uddg=<编码真链接>，解包（脚本对 uddg 另做一次 unquote）。
fn unwrap_ddg(href: &str) -> String {
    let h = if href.starts_with("//") { format!("https:{href}") } else { href.to_string() };
    if let Ok(u) = Url::parse("https://duckduckgo.com").and_then(|b| b.join(&h))
        && u.path() == "/l/"
        && let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "uddg")
        && !v.is_empty()
    {
        return percent_decode_str(&v).decode_utf8_lossy().into_owned();
    }
    h
}

/// Bing 结果链接 bing.com/ck/a?...&u=a1<base64>，解出真 URL。
fn unwrap_bing(href: &str) -> String {
    if let Ok(u) = Url::parse(href)
        && u.host_str().is_some_and(|h| h.ends_with("bing.com"))
        && u.path().starts_with("/ck/")
        && let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "u")
        && let Some(body) = v.strip_prefix("a1")
        && let Ok(raw) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(body.trim_end_matches('='))
    {
        let real = String::from_utf8_lossy(&raw).into_owned();
        if real.starts_with("http") {
            return real;
        }
    }
    href.to_string()
}

pub fn parse_ddg(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let (s_res, s_a, s_sn) = (sel(".result"), sel(".result__a"), sel(".result__snippet"));
    let mut out = Vec::new();
    for res in doc.select(&s_res) {
        let Some(a) = res.select(&s_a).next() else { continue };
        let href = a.value().attr("href").unwrap_or_default();
        if href.is_empty() {
            continue;
        }
        let url = unwrap_ddg(href);
        if !url.starts_with("http") {
            continue;
        }
        let snippet = res.select(&s_sn).next().map(text_of).unwrap_or_default();
        out.push(hit(url, text_of(a), snippet));
    }
    out
}

pub fn parse_bing(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let (s_li, s_a, s_cap) = (sel("li.b_algo"), sel("h2 a"), sel(".b_caption p, .b_caption"));
    let mut out = Vec::new();
    for li in doc.select(&s_li) {
        let Some(a) = li.select(&s_a).next() else { continue };
        let href = a.value().attr("href").unwrap_or_default();
        if href.is_empty() {
            continue;
        }
        let url = unwrap_bing(href);
        if !url.starts_with("http") {
            continue;
        }
        let snippet = li.select(&s_cap).next().map(text_of).unwrap_or_default();
        out.push(hit(url, text_of(a), snippet));
    }
    out
}

/// Google：锚点是「<a href> 里套 <h3>」；摘要取不到就空，不猜（同脚本）。
pub fn parse_google(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let mut out = Vec::new();
    for h3 in doc.select(&sel("h3")) {
        let Some(a) = h3
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find(|e| e.value().name() == "a" && e.value().attr("href").is_some())
        else {
            continue;
        };
        let href = a.value().attr("href").unwrap_or_default();
        let Ok(u) = Url::parse(href) else { continue };
        let host = u.host_str().unwrap_or_default().to_lowercase();
        if host.is_empty() || host.ends_with("google.com") {
            continue;
        }
        let title = text_of(h3);
        let mut snippet = String::new();
        if let Some(container) = a.ancestors().filter_map(ElementRef::wrap).find(|e| e.value().name() == "div") {
            for div in container.children().filter_map(ElementRef::wrap).filter(|e| e.value().name() == "div") {
                let txt = text_of(div);
                if !txt.is_empty() && txt != title {
                    snippet = txt;
                    break;
                }
            }
        }
        out.push(hit(href.to_string(), title, snippet));
    }
    out
}

pub fn parse_github(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let (s_list, s_a, s_cand) = (
        sel("div[data-testid='results-list']"),
        sel("div.search-title a[href]"),
        sel("span.line-clamp-2, .search-match"),
    );
    let mut out = Vec::new();
    let Some(lst) = doc.select(&s_list).next() else { return out };
    for item in lst.children().filter_map(ElementRef::wrap).filter(|e| e.value().name() == "div") {
        let Some(a) = item.select(&s_a).next() else { continue };
        let href = a.value().attr("href").unwrap_or_default();
        let url = if href.starts_with('/') { format!("https://github.com{href}") } else { href.to_string() };
        let title = text_of(a);
        let snippet = item
            .select(&s_cand)
            .map(text_of)
            .find(|t| !t.is_empty() && *t != title)
            .unwrap_or_default();
        out.push(hit(url, title, snippet));
    }
    out
}

pub fn parse_wikipedia(json: &str, lang: &str) -> Vec<SearchHit> {
    static TAG: OnceLock<Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| Regex::new(r"<[^>]+>").unwrap());
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    v["query"]["search"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|h| {
                    let title = h["title"].as_str().unwrap_or_default();
                    let snippet = tag.replace_all(h["snippet"].as_str().unwrap_or_default(), "").trim().to_string();
                    hit(
                        format!("https://{lang}.wikipedia.org/wiki/{}", quote_plus(&title.replace(' ', "_"))),
                        title.to_string(),
                        snippet,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_arxiv(xml: &str) -> Vec<SearchHit> {
    static ENTRY: OnceLock<Regex> = OnceLock::new();
    let entry_re = ENTRY.get_or_init(|| Regex::new(r"(?s)<entry>(.*?)</entry>").unwrap());
    let tag = |entry: &str, name: &str| -> String {
        Regex::new(&format!(r"(?s)<{name}[^>]*>(.*?)</{name}>"))
            .ok()
            .and_then(|re| re.captures(entry).map(|c| norm_ws(&c[1])))
            .unwrap_or_default()
    };
    entry_re
        .captures_iter(xml)
        .filter_map(|c| {
            let e = &c[1];
            let url = tag(e, "id");
            url.starts_with("http")
                .then(|| hit(url, tag(e, "title"), tag(e, "summary").chars().take(300).collect()))
        })
        .collect()
}

pub fn parse_searx(json: &str) -> Vec<SearchHit> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    v["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|r| r["url"].as_str().is_some_and(|u| u.starts_with("http")))
                .map(|r| {
                    hit(
                        r["url"].as_str().unwrap_or_default().to_string(),
                        norm_ws(r["title"].as_str().unwrap_or_default()),
                        norm_ws(r["content"].as_str().unwrap_or_default()),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------- 引擎（对照 websearch.py _e_*） ----------

fn e_ddg<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("ddg", || "https://html.duckduckgo.com".into());
        Ok(parse_ddg(&get_text(&format!("{b}/html/?q={}", quote_plus(q)), &HTML_HEADERS).await?))
    })
}

fn e_bing<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("bing", || "https://www.bing.com".into());
        Ok(parse_bing(&get_text(&format!("{b}/search?q={}", quote_plus(q)), &HTML_HEADERS).await?))
    })
}

/// 注意：Google 按出口 IP 风控，被标记的 IP 拿到 JS 墙 / reCAPTCHA 页（解析 0 条，等同跳过）。
fn e_google<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("google", || "https://www.google.com".into());
        Ok(parse_google(&get_text(&format!("{b}/search?q={}", quote_plus(q)), &HTML_HEADERS).await?))
    })
}

fn e_github<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("github", || "https://github.com".into());
        Ok(parse_github(
            &get_text(&format!("{b}/search?type=repositories&q={}", quote_plus(q)), &HTML_HEADERS).await?,
        ))
    })
}

/// Wikipedia 免 key 官方 API；zh 查空回退 en。
fn e_wikipedia<'a>(ctx: &'a EngineCtx, q: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        for lang in ["zh", "en"] {
            let b = ctx.base_or("wikipedia", || format!("https://{lang}.wikipedia.org"));
            let url = format!(
                "{b}/w/api.php?action=query&list=search&format=json&srlimit={limit}&sroffset=0&srsearch={}",
                quote_plus(q)
            );
            let items = parse_wikipedia(&get_text(&url, &HTML_HEADERS).await?, lang);
            if !items.is_empty() {
                return Ok(items);
            }
        }
        Ok(Vec::new())
    })
}

/// arXiv 官方免 key API（Atom XML）。
fn e_arxiv<'a>(ctx: &'a EngineCtx, q: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("arxiv", || "https://export.arxiv.org".into());
        let url = format!("{b}/api/query?search_query={}&max_results={limit}&start=0", quote_plus(&format!("all:{q}")));
        Ok(parse_arxiv(&get_text(&url, &[]).await?))
    })
}

/// SearXNG：实例 = 用户自填 / 手动从 searx.space 列表多选（本引擎不自动拉列表），逐个试到出结果。
fn e_searxng<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        if ctx.searxng_urls.is_empty() {
            return Err("searxng: 未配置实例（在「工具」页填写或从 searx.space 列表选择）".into());
        }
        let per = ctx.timeout.min(Duration::from_secs(6));
        let mut errors = Vec::new();
        for base in ctx.searxng_urls.iter().take(SEARX_MAX_ATTEMPTS) {
            let url = format!(
                "{}/search?q={}&format=json&language=auto&pageno=1",
                base.trim_end_matches('/'),
                quote_plus(q)
            );
            match tokio::time::timeout(per, get_text(&url, &[("Accept", "application/json")])).await {
                Ok(Ok(body)) => {
                    let items = parse_searx(&body);
                    if !items.is_empty() {
                        return Ok(items);
                    }
                }
                Ok(Err(e)) => errors.push(format!("{base}: {e}")),
                Err(_) => errors.push(format!("{base}: timeout")),
            }
        }
        let tail = errors[errors.len().saturating_sub(2)..].join("; ");
        Err(if tail.is_empty() { "无可用实例".into() } else { tail })
    })
}

/// 单次搜索最多试几个 SearXNG 实例（公益实例省着用，同脚本）。
const SEARX_MAX_ATTEMPTS: usize = 12;
