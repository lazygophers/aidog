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
    EngineDef { id: "ddg_lite", default_on: false, run: e_ddg_lite },
    EngineDef { id: "bing", default_on: true, run: e_bing },
    EngineDef { id: "google", default_on: true, run: e_google },
    EngineDef { id: "yandex", default_on: false, run: e_yandex },
    EngineDef { id: "sogou", default_on: false, run: e_sogou },
    EngineDef { id: "baidu", default_on: false, run: e_baidu },
    EngineDef { id: "so360", default_on: false, run: e_so360 },
    EngineDef { id: "arxiv", default_on: true, run: e_arxiv },
    EngineDef { id: "crossref", default_on: false, run: e_crossref },
    EngineDef { id: "pubmed", default_on: false, run: e_pubmed },
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

/// 不跟随跳转的指纹客户端（百度 / 搜狗的 /link 跳转要读 Location / meta refresh）。
fn client_noredir() -> Result<&'static wreq::Client, String> {
    static C: OnceLock<Result<wreq::Client, String>> = OnceLock::new();
    C.get_or_init(|| {
        wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome149)
            .redirect(wreq::redirect::Policy::none())
            .build()
            .map_err(|e| format!("build fingerprint client: {e}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// 朴素客户端（无指纹）：NCBI eutils 对 Chrome TLS 指纹直接 reset（同脚本 _e_pubmed）。
fn client_plain() -> Result<&'static wreq::Client, String> {
    static C: OnceLock<Result<wreq::Client, String>> = OnceLock::new();
    C.get_or_init(|| wreq::Client::builder().build().map_err(|e| format!("build client: {e}")))
        .as_ref()
        .map_err(Clone::clone)
}

const HTML_HEADERS: [(&str, &str); 2] = [
    ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    ("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8"),
];

async fn get_text(url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
    get_text_with(client()?, url, headers).await
}

async fn get_text_with(c: &wreq::Client, url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
    let mut req = c.get(url);
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

/// DDG Lite：纯 table 布局，摘要在链接所在行之后的第二个 `<tr>`（脚本如此，不猜）。
pub fn parse_ddg_lite(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let (s_a, s_td) = (sel("a.result-link"), sel("td"));
    fn next_tr(e: ElementRef<'_>) -> Option<ElementRef<'_>> {
        e.next_siblings().filter_map(ElementRef::wrap).find(|x| x.value().name() == "tr")
    }
    let mut out = Vec::new();
    for a in doc.select(&s_a) {
        let url = unwrap_ddg(a.value().attr("href").unwrap_or_default());
        if !url.starts_with("http") {
            continue;
        }
        let row = a.ancestors().filter_map(ElementRef::wrap).find(|x| x.value().name() == "tr");
        let snippet = row
            .and_then(next_tr)
            .and_then(next_tr)
            .and_then(|r| r.select(&s_td).next())
            .map(text_of)
            .unwrap_or_default();
        out.push(hit(url, text_of(a), snippet));
    }
    out
}

/// Yandex：li.serp-item，链接已是真 URL；被验证码拦时解析为 0 条。
pub fn parse_yandex(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let (s_li, s_a, s_h2, s_sn) = (
        sel("li.serp-item"),
        sel("a.Link[href]"),
        sel("h2"),
        sel(".OrganicTextContentSpan, .TextContainer, .organic__content-wrapper"),
    );
    let mut out = Vec::new();
    for li in doc.select(&s_li) {
        let Some(a) = li.select(&s_a).next() else { continue };
        let href = a.value().attr("href").unwrap_or_default();
        if !href.starts_with("http") {
            continue;
        }
        let h2 = li.select(&s_h2).next().map(text_of).unwrap_or_default();
        let title = if h2.is_empty() { text_of(a) } else { h2 };
        let snippet = li.select(&s_sn).next().map(text_of).unwrap_or_default();
        out.push(hit(href.to_string(), title, snippet));
    }
    out
}

/// 搜狗：vrwrap/rb 块；部分链接只有 `/link?url=` 跳转，保留原样由引擎解真 URL。
pub fn parse_sogou(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let (s_b, s_a, s_sn) = (sel("div.vrwrap, div.rb"), sel("h3 a[href]"), sel(".text-layout"));
    let mut out = Vec::new();
    for b in doc.select(&s_b) {
        let Some(a) = b.select(&s_a).next() else { continue };
        let snippet = b.select(&s_sn).next().map(text_of).unwrap_or_default();
        out.push(hit(a.value().attr("href").unwrap_or_default().to_string(), text_of(a), snippet));
    }
    out
}

/// 360：真 URL 在 h3 a 的 data-mdurl；href 退到会话跳转链 so.com/link 时丢弃。
pub fn parse_so360(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let (s_li, s_a, s_sn) = (sel("li.res-list"), sel("h3 a"), sel("div.res-rich"));
    let mut out = Vec::new();
    for li in doc.select(&s_li) {
        let Some(a) = li.select(&s_a).next() else { continue };
        let v = a.value();
        let url = v.attr("data-mdurl").filter(|u| !u.is_empty()).or(v.attr("href")).unwrap_or_default();
        if !url.starts_with("http") || url.starts_with("https://www.so.com/link") {
            continue;
        }
        let snippet = li.select(&s_sn).next().map(text_of).unwrap_or_default();
        out.push(hit(url.to_string(), text_of(a), snippet));
    }
    out
}

/// 百度：链接全是 baidu.com/link?url= 会话跳转（由引擎逐条解）；摘要容器 class 乱变，取整块文本去掉标题前缀。
pub fn parse_baidu(html: &str) -> Vec<SearchHit> {
    let doc = Html::parse_document(html);
    let (s_b, s_a) = (sel("div.result, div.c-container"), sel("h3 a[href]"));
    let mut out = Vec::new();
    for b in doc.select(&s_b) {
        let Some(a) = b.select(&s_a).next() else { continue };
        let title = text_of(a);
        let body = text_of(b);
        let rest = body.strip_prefix(title.as_str()).map_or(body.as_str(), str::trim);
        let snippet: String = rest.chars().take(250).collect();
        out.push(hit(a.value().attr("href").unwrap_or_default().to_string(), title, snippet));
    }
    out
}

/// Crossref works 响应：items[].title[0] / URL / abstract（JATS 片段去标签，截 300 字）。
pub fn parse_crossref(json: &str) -> Vec<SearchHit> {
    static TAG: OnceLock<Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| Regex::new(r"<[^>]+>").unwrap());
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    v["message"]["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|it| it["URL"].as_str().is_some_and(|u| u.starts_with("http")))
                .map(|it| {
                    let abs = tag.replace_all(it["abstract"].as_str().unwrap_or_default(), "");
                    hit(
                        it["URL"].as_str().unwrap_or_default().to_string(),
                        it["title"][0].as_str().unwrap_or_default().to_string(),
                        abs.trim().chars().take(300).collect(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// PubMed：esearch 的 id 列表 + esummary 的标题 / 来源。
pub fn parse_pubmed(esearch: &str, esummary: &str) -> Vec<SearchHit> {
    let s: serde_json::Value = serde_json::from_str(esearch).unwrap_or_default();
    let r: serde_json::Value = serde_json::from_str(esummary).unwrap_or_default();
    s["esearchresult"]["idlist"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str())
                .filter_map(|id| {
                    let item = &r["result"][id];
                    let title = norm_ws(item["title"].as_str().unwrap_or_default());
                    (!title.is_empty()).then(|| {
                        hit(
                            format!("https://pubmed.ncbi.nlm.nih.gov/{id}/"),
                            title.trim_end_matches('.').to_string(),
                            item["source"].as_str().unwrap_or_default().to_string(),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------- 其余引擎 ----------

fn e_ddg_lite<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("ddg_lite", || "https://lite.duckduckgo.com".into());
        Ok(parse_ddg_lite(&get_text(&format!("{b}/lite/?q={}", quote_plus(q)), &HTML_HEADERS).await?))
    })
}

fn e_yandex<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("yandex", || "https://yandex.com".into());
        Ok(parse_yandex(&get_text(&format!("{b}/search/?text={}", quote_plus(q)), &HTML_HEADERS).await?))
    })
}

fn e_so360<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("so360", || "https://www.so.com".into());
        Ok(parse_so360(&get_text(&format!("{b}/s?q={}", quote_plus(q)), &HTML_HEADERS).await?))
    })
}

/// 搜狗：`/link` 跳转页是 meta refresh，GET 一次取真 URL；解不出的丢弃。
fn e_sogou<'a>(ctx: &'a EngineCtx, q: &'a str, _limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        static META: OnceLock<Regex> = OnceLock::new();
        let meta = META.get_or_init(|| Regex::new(r#"URL='?([^'">]+)"#).unwrap());
        let b = ctx.base_or("sogou", || "https://www.sogou.com".into());
        let items = parse_sogou(&get_text(&format!("{b}/web?query={}", quote_plus(q)), &HTML_HEADERS).await?);
        let resolved = futures::future::join_all(items.into_iter().map(|mut it| {
            let b = b.clone();
            async move {
                if it.url.starts_with("/link") {
                    let hdr = [("Referer", "https://www.sogou.com/")];
                    let real = async {
                        let r = client_noredir().ok()?.get(format!("{b}{}", it.url)).header(hdr[0].0, hdr[0].1).send().await.ok()?;
                        let loc = r.headers().get("location").and_then(|v| v.to_str().ok()).map(String::from);
                        let body: String = r.text().await.ok()?.chars().take(500).collect();
                        loc.or_else(|| meta.captures(&body).map(|c| c[1].to_string()))
                    }
                    .await;
                    if let Some(u) = real.filter(|u| u.starts_with("http")) {
                        it.url = u;
                    }
                }
                it
            }
        }))
        .await;
        Ok(resolved.into_iter().filter(|h| h.url.starts_with("http")).collect())
    })
}

/// 百度：先取前 limit 条，再逐条 302 解 `baidu.com/link?` 真 URL（广告 baidu.php 不解，同脚本）。
fn e_baidu<'a>(ctx: &'a EngineCtx, q: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("baidu", || "https://www.baidu.com".into());
        let items: Vec<_> =
            parse_baidu(&get_text(&format!("{b}/s?wd={}", quote_plus(q)), &HTML_HEADERS).await?).into_iter().take(limit).collect();
        let resolved = futures::future::join_all(items.into_iter().map(|mut it| {
            let (b, override_base) = (b.clone(), ctx.bases.contains_key("baidu"));
            async move {
                if it.url.contains("baidu.com/link?") {
                    // 测试注入 base 时把 host 换成假上游，生产路径原样请求
                    let target = match (override_base, Url::parse(&it.url)) {
                        (true, Ok(u)) => format!("{b}{}?{}", u.path(), u.query().unwrap_or_default()),
                        _ => it.url.clone(),
                    };
                    let loc = async {
                        let r = client_noredir().ok()?.get(target).send().await.ok()?;
                        r.headers().get("location").and_then(|v| v.to_str().ok()).map(String::from)
                    }
                    .await;
                    if let Some(u) = loc.filter(|u| u.starts_with("http")) {
                        it.url = u;
                    }
                }
                it
            }
        }))
        .await;
        Ok(resolved.into_iter().filter(|h| h.url.starts_with("http")).collect())
    })
}

/// Crossref 官方免 key API（跨库论文 DOI 元数据）。
fn e_crossref<'a>(ctx: &'a EngineCtx, q: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("crossref", || "https://api.crossref.org".into());
        let url = format!("{b}/works?query={}&rows={limit}&offset=0&select=title%2CURL%2Cabstract", quote_plus(q));
        Ok(parse_crossref(&get_text(&url, &[]).await?))
    })
}

/// PubMed 官方免 key API：esearch 拿 id → esummary 拿标题；朴素客户端（eutils 拒 Chrome 指纹）。
fn e_pubmed<'a>(ctx: &'a EngineCtx, q: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>, String>> {
    Box::pin(async move {
        let b = ctx.base_or("pubmed", || "https://eutils.ncbi.nlm.nih.gov".into());
        let c = client_plain()?;
        let search = get_text_with(
            c,
            &format!("{b}/entrez/eutils/esearch.fcgi?db=pubmed&term={}&retmode=json&retmax={limit}&retstart=0", quote_plus(q)),
            &[],
        )
        .await?;
        let v: serde_json::Value = serde_json::from_str(&search).unwrap_or_default();
        let ids: Vec<&str> = v["esearchresult"]["idlist"].as_array().into_iter().flatten().filter_map(|i| i.as_str()).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let summary = get_text_with(
            c,
            &format!("{b}/entrez/eutils/esummary.fcgi?db=pubmed&id={}&retmode=json", ids.join(",")),
            &[],
        )
        .await?;
        Ok(parse_pubmed(&search, &summary))
    })
}
