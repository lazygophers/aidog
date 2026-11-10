//! web_search / web_fetch 的 provider 适配层（magpie 票 11，spec D5a）。
//!
//! 端点与鉴权均取自一手文档（票 ## Answer 有清单）：已验证 5 家（tavily / brave / exa /
//! firecrawl / searxng）实现真实调用；未能从一手文档确认的 4 家（youcom / deepseek /
//! doubao / tencent_yuanbao）按票面纪律**不猜端点**，调用一律返回明确的 unverified 错误。
//!
//! 统一出参 [`SearchHit`]（title / url / snippet），provider 特有字段丢弃；多 key 轮换
//! （调用间递进起点），全 key 失败聚合报错。测试全打本地假上游（test-no-real-calls.md）。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

pub mod engines;
pub mod searx_space;

/// 官方 API base（测试经同名适配函数注入本地假上游 base，不在设置里放可覆盖字段）。
pub const TAVILY_BASE: &str = "https://api.tavily.com";
pub const BRAVE_BASE: &str = "https://api.search.brave.com";
pub const EXA_BASE: &str = "https://api.exa.ai";
pub const FIRECRAWL_BASE: &str = "https://api.firecrawl.dev";

/// 轮换游标：跨调用递进，起点 = 游标 % key 数，均摊分摊各 key 的用量。
pub static ROTATION: AtomicUsize = AtomicUsize::new(0);

/// key 服务 id 集合（搜索来源清单的 key 服务部分；单一真值源）。
pub const KEY_SERVICES: &[&str] = &[
    "tavily",
    "brave",
    "exa",
    "firecrawl",
    "youcom",
    "deepseek",
    "doubao",
    "tencent_yuanbao",
];

/// 搜索来源条目（`search_sources` command 返回；显示名走前端 i18n `tools.source.<id>`）。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SearchSource {
    pub id: String,
    /// `keyed`（key 服务）/ `engine`（免 key 引擎）。
    pub kind: &'static str,
    pub needs_key: bool,
    pub default_on: bool,
    /// `per_request`（按次计费）/ `free`；供「一起」模式的计费提示。
    pub billing: &'static str,
}

/// 全部搜索来源：key 服务（按 [`KEY_SERVICES`] 序）在前，免 key 引擎（按 [`engines::ENGINES`] 序）在后。
pub fn search_sources() -> Vec<SearchSource> {
    let keyed = KEY_SERVICES.iter().map(|id| SearchSource {
        id: (*id).into(),
        kind: "keyed",
        needs_key: true,
        default_on: false,
        billing: "per_request",
    });
    let free = engines::ENGINES.iter().map(|e| SearchSource {
        id: e.id.into(),
        kind: "engine",
        needs_key: false,
        default_on: e.default_on,
        billing: "free",
    });
    keyed.chain(free).collect()
}

/// 搜索结果统一出参（provider 特有字段丢弃，spec D5a）。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// 页面年龄 / 发布时间（provider 给了才有，原样字符串）。
    pub page_age: Option<String>,
}

/// 搜索设置（`mcp_tools/settings` 的 `search` 段；`aidog_core::mcp_tools` 再导出）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct SearchProviderConfig {
    /// key 服务 id（词表见 [`KEY_SERVICES`]）。
    #[serde(default)]
    pub provider: String,
    /// API keys，多 key 轮换；至少 1 个非空才算配置了 key 服务。
    #[serde(default)]
    pub keys: Vec<String>,
    /// key 服务启用开关（缺省 true）；false 时视同未配置 key 服务，走免 key 引擎。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// SearXNG 引擎使用的实例 base_url（用户自填 / 从 searx.space 列表多选）。
    #[serde(default)]
    pub searxng_base_urls: Vec<String>,
    /// 免 key 引擎开关覆盖（引擎 id → 开关）；缺项取 [`engines::ENGINES`] 的 `default_on`。
    #[serde(default)]
    pub engines: BTreeMap<String, bool>,
}

fn default_enabled() -> bool {
    true
}

impl Default for SearchProviderConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            keys: Vec::new(),
            enabled: default_enabled(),
            searxng_base_urls: Vec::new(),
            engines: BTreeMap::new(),
        }
    }
}

impl SearchProviderConfig {
    /// 是否已配置到可用（有 provider 且至少一个非空 key）。
    pub fn is_configured(&self) -> bool {
        !self.provider.is_empty() && self.keys.iter().any(|k| !k.trim().is_empty())
    }
}

/// 单引擎 / key 服务一次调用的明细（工具日志消费）。
#[derive(Debug, Clone, PartialEq)]
pub struct EngineReport {
    pub engine: String,
    pub count: usize,
    pub latency_ms: u64,
    pub error: Option<String>,
}

/// 一次搜索的完整结果：命中 + 各引擎明细 + 失败原因。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SearchOutcome {
    pub hits: Vec<SearchHit>,
    pub engines: Vec<EngineReport>,
    /// 实际走的路径：key 服务 id，或 `"engines"`（免 key 引擎并行）。
    pub source: String,
    /// 整体失败原因（None = 成功，可能 0 条）。
    pub error: Option<String>,
}

/// 免 key 引擎并行查询的环境（测试注入本地假引擎 base；生产用 Default）。
#[derive(Debug, Clone)]
pub struct SearchEnv {
    /// 引擎 id → base URL 覆盖（见 `engines::EngineCtx::base_or`）。
    pub bases: std::collections::HashMap<String, String>,
    /// 单引擎超时。
    pub timeout: Duration,
}

impl Default for SearchEnv {
    fn default() -> Self {
        Self { bases: Default::default(), timeout: Duration::from_secs(ENGINE_TIMEOUT_SECS) }
    }
}

/// 单引擎超时秒数。
pub const ENGINE_TIMEOUT_SECS: u64 = 15;
/// 合并后结果上限。
pub const MERGE_CAP: usize = 10;

/// web_fetch 的大小上限（票面「如 1MB」）：超过即截断，不报错。
pub const FETCH_MAX_BYTES: usize = 1024 * 1024;
/// web_fetch / web_search 单次出站超时。
pub const FETCH_TIMEOUT_SECS: u64 = 30;

pub fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("build http client: {e}"))
}

/// `web_search` 工具入口（只要命中列表；明细见 [`web_search_detailed`]）。
pub async fn web_search(
    cfg: &SearchProviderConfig,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let o = web_search_detailed(cfg, query, count, &SearchEnv::default()).await;
    match o.error {
        Some(e) => Err(e),
        None => Ok(o.hits),
    }
}

/// 完整搜索：配了 key 的 key 服务优先（单独成功即返回）；否则并行查询已开的免 key 引擎。
pub async fn web_search_detailed(
    cfg: &SearchProviderConfig,
    query: &str,
    count: Option<usize>,
    env: &SearchEnv,
) -> SearchOutcome {
    if cfg.enabled && cfg.is_configured() && cfg.provider != "searxng" {
        return keyed_search(cfg, query, count, env).await;
    }
    engines_search(cfg, query, count, env).await
}

async fn keyed_search(
    cfg: &SearchProviderConfig,
    query: &str,
    count: Option<usize>,
    env: &SearchEnv,
) -> SearchOutcome {
    let p = cfg.provider.as_str();
    let default_base = match p {
        "tavily" => TAVILY_BASE,
        "brave" => BRAVE_BASE,
        "exa" => EXA_BASE,
        "firecrawl" => FIRECRAWL_BASE,
        // 未验证 provider：不猜端点，显式报错（票面纪律）。
        _ => {
            return SearchOutcome {
                source: p.into(),
                error: Some(format!("provider `{p}` 的公开搜索 API 未能从一手文档确认（unverified），未实现")),
                ..Default::default()
            };
        }
    };
    let base = env.bases.get(p).map(String::as_str).unwrap_or(default_base);
    let keys: Vec<String> = cfg.keys.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect();
    let t = std::time::Instant::now();
    let r = search_with_keys(p, base, keys, query, count).await;
    let latency_ms = t.elapsed().as_millis() as u64;
    let (hits, error) = match r {
        Ok(h) => (h, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    SearchOutcome {
        engines: vec![EngineReport { engine: p.into(), count: hits.len(), latency_ms, error: error.clone() }],
        hits,
        source: p.into(),
        error,
    }
}

/// URL 去重键（同脚本）：百分号解码 + 去尾斜杠。
fn dedup_key(url: &str) -> String {
    percent_encoding::percent_decode_str(url).decode_utf8_lossy().trim_end_matches('/').to_string()
}

async fn engines_search(
    cfg: &SearchProviderConfig,
    query: &str,
    count: Option<usize>,
    env: &SearchEnv,
) -> SearchOutcome {
    let active: Vec<&engines::EngineDef> = engines::ENGINES
        .iter()
        .filter(|e| cfg.engines.get(e.id).copied().unwrap_or(e.default_on))
        .collect();
    let mut out = SearchOutcome { source: "engines".into(), ..Default::default() };
    if active.is_empty() {
        out.error = Some("未配置搜索 provider（填 API key），且没有启用的免 key 引擎".into());
        return out;
    }
    let ctx = engines::EngineCtx {
        bases: env.bases.clone(),
        searxng_urls: cfg.searxng_base_urls.iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect(),
        timeout: env.timeout,
    };
    let runs = active.iter().map(|e| {
        let ctx = &ctx;
        async move {
            let t = std::time::Instant::now();
            let r = match tokio::time::timeout(ctx.timeout, (e.run)(ctx, query, MERGE_CAP)).await {
                Ok(r) => r,
                Err(_) => Err(format!("timeout ({}ms)", ctx.timeout.as_millis())),
            };
            (e.id, r, t.elapsed().as_millis() as u64)
        }
    });
    let results = futures::future::join_all(runs).await;
    let mut lists: Vec<Vec<SearchHit>> = Vec::new();
    for (id, r, latency_ms) in results {
        match r {
            Ok(hits) => {
                out.engines.push(EngineReport { engine: id.into(), count: hits.len(), latency_ms, error: None });
                lists.push(hits);
            }
            Err(e) => out.engines.push(EngineReport { engine: id.into(), count: 0, latency_ms, error: Some(e) }),
        }
    }
    // 各引擎轮询交织（避免首个引擎占满上限），URL 去重。
    let cap = count.unwrap_or(MERGE_CAP).clamp(1, MERGE_CAP);
    let mut seen = std::collections::HashSet::new();
    'merge: for i in 0..lists.iter().map(Vec::len).max().unwrap_or(0) {
        for l in &lists {
            if let Some(h) = l.get(i)
                && seen.insert(dedup_key(&h.url))
            {
                out.hits.push(h.clone());
                if out.hits.len() >= cap {
                    break 'merge;
                }
            }
        }
    }
    if out.engines.iter().all(|r| r.error.is_some()) {
        let errs: Vec<String> =
            out.engines.iter().map(|r| format!("{}: {}", r.engine, r.error.as_deref().unwrap_or_default())).collect();
        out.error = Some(format!("所有引擎都失败: {}", errs.join("; ")));
    }
    out
}

/// 多 key 轮换：起点取自 [`ROTATION`]，逐 key 尝试，全失败聚合报错。
async fn search_with_keys(
    provider: &str,
    base: &str,
    keys: Vec<String>,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    if keys.is_empty() {
        return Err(format!("provider `{provider}` 未配置 API key"));
    }
    let client = http_client()?;
    let start = ROTATION.fetch_add(1, Ordering::Relaxed) % keys.len();
    let mut errs = Vec::new();
    for i in 0..keys.len() {
        let idx = (start + i) % keys.len();
        let attempt = match provider {
            "tavily" => tavily_search(&client, base, &keys[idx], query, count).await,
            "brave" => brave_search(&client, base, &keys[idx], query, count).await,
            "exa" => exa_search(&client, base, &keys[idx], query, count).await,
            "firecrawl" => firecrawl_search(&client, base, &keys[idx], query, count).await,
            other => return Err(format!("unknown provider `{other}`")),
        };
        match attempt {
            Ok(hits) => return Ok(hits),
            Err(e) => errs.push(format!("key#{}: {e}", idx + 1)),
        }
    }
    Err(format!(
        "provider `{provider}` 全部 {} 个 key 失败: {}",
        keys.len(),
        errs.join("; ")
    ))
}

pub async fn call(
    req: reqwest::RequestBuilder,
) -> Result<serde_json::Value, String> {
    let resp = req.send().await.map_err(|e| format!("request: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status.as_u16(), truncate_str(&body, 200)));
    }
    serde_json::from_str(&body).map_err(|e| format!("parse json: {e}: {}", truncate_str(&body, 200)))
}

pub fn truncate_str(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// Tavily：POST {base}/search，body 带 api_key（docs.tavily.com API reference）。
pub async fn tavily_search(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut body = serde_json::json!({ "query": query, "api_key": key });
    if let Some(n) = count {
        body["max_results"] = serde_json::json!(n);
    }
    let v = call(client.post(format!("{base}/search")).json(&body)).await?;
    Ok(v["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| SearchHit {
                    title: r["title"].as_str().unwrap_or_default().to_string(),
                    url: r["url"].as_str().unwrap_or_default().to_string(),
                    snippet: r["content"].as_str().unwrap_or_default().to_string(),
                    page_age: None,
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Brave：GET {base}/res/v1/web/search，头 X-Subscription-Token（官方 dashboard 文档）。
pub async fn brave_search(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut req = client
        .get(format!("{base}/res/v1/web/search"))
        .header("X-Subscription-Token", key)
        .header("Accept", "application/json")
        .query(&[("q", query)]);
    if let Some(n) = count {
        req = req.query(&[("count", n.to_string())]);
    }
    let v = call(req).await?;
    Ok(v["web"]["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| SearchHit {
                    title: r["title"].as_str().unwrap_or_default().to_string(),
                    url: r["url"].as_str().unwrap_or_default().to_string(),
                    snippet: r["description"].as_str().unwrap_or_default().to_string(),
                    page_age: r["page_age"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Exa：POST {base}/search，头 x-api-key（docs.exa.ai reference/search）。
pub async fn exa_search(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut body = serde_json::json!({ "query": query });
    if let Some(n) = count {
        body["numResults"] = serde_json::json!(n);
    }
    let req = client
        .post(format!("{base}/search"))
        .header("x-api-key", key)
        .json(&body);
    let v = call(req).await?;
    Ok(v["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| SearchHit {
                    title: r["title"].as_str().unwrap_or_default().to_string(),
                    url: r["url"].as_str().unwrap_or_default().to_string(),
                    // /search 缺省不带正文，text 仅在请求 contents 时出现；缺省留空。
                    snippet: r["text"].as_str().unwrap_or_default().to_string(),
                    page_age: r["publishedDate"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Firecrawl：POST {base}/v1/search，Bearer（docs.firecrawl.dev api-reference/v1/search）。
pub async fn firecrawl_search(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut body = serde_json::json!({ "query": query });
    if let Some(n) = count {
        body["limit"] = serde_json::json!(n);
    }
    let req = client
        .post(format!("{base}/v1/search"))
        .bearer_auth(key)
        .json(&body);
    let v = call(req).await?;
    Ok(v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| SearchHit {
                    title: r["title"].as_str().unwrap_or_default().to_string(),
                    url: r["url"].as_str().unwrap_or_default().to_string(),
                    snippet: r["description"]
                        .as_str()
                        .or_else(|| r["markdown"].as_str())
                        .unwrap_or_default()
                        .to_string(),
                    page_age: None,
                })
                .collect()
        })
        .unwrap_or_default())
}

/// `web_fetch` 工具入口：直抓 URL，HTML → 纯文本，1MB 截断 + 30s 超时。
pub async fn fetch_url_text(url: &str) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("仅支持 http/https URL，收到 `{url}`"));
    }
    let client = http_client()?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let is_html = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|c| c.to_ascii_lowercase().contains("html"));
    let mut body = Vec::new();
    let mut truncated = false;
    let mut stream = resp;
    while let Some(chunk) = stream.chunk().await.map_err(|e| format!("read body: {e}"))? {
        let room = FETCH_MAX_BYTES.saturating_sub(body.len());
        if room == 0 {
            truncated = true;
            break;
        }
        if chunk.len() > room {
            body.extend_from_slice(&chunk[..room]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&body);
    Ok(if is_html {
        html_to_text(&text)
    } else {
        text.into_owned()
    }
    .trim()
    .to_string()
    + if truncated { "\n\n[内容超过 1MB，已截断]" } else { "" })
}

/// 轻量 HTML → 纯文本：剥 script/style/注释/标签（块级标签转换行），解码常见实体。
/// 手写扫描器，不引解析器依赖（票面「轻量转换（无 headless）」）。
pub fn html_to_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    let block_tags = [
        "br", "p", "div", "li", "tr", "ul", "ol", "table", "h1", "h2", "h3", "h4", "h5", "h6",
        "blockquote", "pre", "section", "article", "header", "footer",
    ];
    while i < bytes.len() {
        if bytes[i] != b'<' {
            // 纯文本段：复制到下一个 '<'
            let end = lower[i..].find('<').map(|p| i + p).unwrap_or(bytes.len());
            out.push_str(&html[i..end]);
            i = end;
            continue;
        }
        // 注释
        if lower[i..].starts_with("<!--") {
            i = match lower[i..].find("-->") {
                Some(p) => i + p + 3,
                None => bytes.len(),
            };
            continue;
        }
        // 标签名
        let name_start = i + 1;
        let name_end = bytes[name_start..]
            .iter()
            .position(|c| !c.is_ascii_alphanumeric())
            .map(|p| name_start + p)
            .unwrap_or(bytes.len());
        let name = &lower[name_start..name_end];
        // script / style：整块丢弃（连内容）
        if name == "script" || name == "style" {
            let close = format!("</{name}");
            i = match lower[i..].find(&close) {
                Some(p) => {
                    let after = i + p + close.len();
                    match lower[after..].find('>') {
                        Some(q) => after + q + 1,
                        None => bytes.len(),
                    }
                }
                None => bytes.len(),
            };
            continue;
        }
        // 标签本体：跳到 '>'
        i = match lower[i..].find('>') {
            Some(p) => i + p + 1,
            None => bytes.len(),
        };
        if block_tags.contains(&name) {
            out.push('\n');
        }
    }
    let decoded = decode_entities(&out);
    // 压缩 3+ 连续换行为空行
    let mut collapsed = String::with_capacity(decoded.len());
    let mut nl = 0;
    for c in decoded.chars() {
        if c == '\n' {
            nl += 1;
            if nl > 2 {
                continue;
            }
        } else {
            nl = 0;
        }
        collapsed.push(c);
    }
    // 块级标签在文本首尾产生的空行剥掉
    collapsed.trim().to_string()
}

/// 解码命名实体（常见 5 个 + nbsp）与数字实体（&#NN; / &#xHH;）。
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let end = rest.find(';');
        let entity = end.map(|e| &rest[1..e]).unwrap_or("");
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16).ok().and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => entity[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match (decoded, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            // 不认识的实体原样保留
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 把搜索结果格式化成 MCP 工具的文本出参。
pub fn format_hits(hits: &[SearchHit]) -> String {
    if hits.is_empty() {
        return "（无结果）".to_string();
    }
    hits.iter()
        .enumerate()
        .map(|(i, h)| {
            let mut s = format!("{}. {}\n   {}", i + 1, h.title, h.url);
            if !h.snippet.is_empty() {
                s.push_str(&format!("\n   {}", h.snippet.replace('\n', " ")));
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
#[path = "test_search.rs"]
mod test_search;

#[cfg(test)]
#[path = "test_engines.rs"]
mod test_engines;

#[cfg(test)]
#[path = "test_engines_more.rs"]
mod test_engines_more;

#[cfg(test)]
#[path = "test_sources.rs"]
mod test_sources;
