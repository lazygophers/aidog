//! web_search / web_fetch 的 provider 适配层（magpie 票 11，spec D5a）。
//!
//! 端点与鉴权均取自一手文档：key 服务 8 家（tavily / brave / exa / firecrawl / youcom /
//! deepseek / doubao / tencent_yuanbao）+ 免 key 引擎，全部实现真实调用。
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
pub const DEEPSEEK_BASE: &str = "https://api.deepseek.com";
pub const YOUCOM_BASE: &str = "https://ydc-index.io";
pub const DOUBAO_BASE: &str = "https://open.feedcoopapi.com";
pub const TENCENT_YUANBAO_BASE: &str = "https://api.wsa.cloud.tencent.com";

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

/// 搜索执行模式。
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    /// 按 `sources` 顺序逐个试：报错 / 超时 / 0 条换下一个，首个非空即停。
    #[default]
    Sequential,
    /// 并发查全部来源（等全部返回），合并 + URL 去重。
    Parallel,
}

/// 单个 key 服务的配置（路径必须以 `keys` 结尾，管理接口脱敏按该后缀匹配）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ServiceConfig {
    /// API keys，多 key 轮换。
    #[serde(default)]
    pub keys: Vec<String>,
}

/// 搜索设置（`mcp_tools/settings` 的 `search` 段；`aidog_core::mcp_tools` 再导出）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct SearchProviderConfig {
    /// 「启用搜索」总开关（缺省 true）；false = 整个搜索停用（含免 key 引擎）。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 有序启用的来源（key 服务 id 或免 key 引擎 id，顺序即优先级）。
    #[serde(default)]
    pub sources: Vec<String>,
    /// 执行模式（缺省顺序）。
    #[serde(default)]
    pub mode: SearchMode,
    /// key 服务 id → 配置（key 本身与是否在 `sources` 里无关，取消勾选不删 key）。
    #[serde(default)]
    pub services: BTreeMap<String, ServiceConfig>,
    /// SearXNG 引擎使用的实例 base_url（用户自填 / 从 searx.space 列表多选）。
    #[serde(default)]
    pub searxng_base_urls: Vec<String>,
}

fn default_enabled() -> bool {
    true
}

/// 全新安装的来源：`default_on` 的免 key 引擎，按 [`engines::ENGINES`] 序。
fn default_sources() -> Vec<String> {
    engines::ENGINES.iter().filter(|e| e.default_on).map(|e| e.id.to_string()).collect()
}

impl Default for SearchProviderConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            sources: default_sources(),
            mode: SearchMode::Parallel,
            services: BTreeMap::new(),
            searxng_base_urls: Vec::new(),
        }
    }
}

/// 旧形状（`provider` / `keys` / `enabled` / `engines`）→ 新形状，原地改写 `search` 段。
/// 返回是否有改动（调用方据此回写）。已含 `sources` 键、非对象 → 不动。幂等。
///
/// 口径（保持旧的实际效果）：旧 `enabled` 只管 key 服务，免 key 引擎恒落；新 `enabled` 管整个搜索，
/// 故旧 enabled=false 但有引擎可用时迁移后 enabled=true、来源只含引擎。key 一律保留在 `services`。
pub fn migrate_search_value(search: &mut serde_json::Value) -> bool {
    let Some(old) = search.as_object() else { return false };
    if old.contains_key("sources") {
        return false;
    }
    let old_enabled = old.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let provider = old.get("provider").and_then(|v| v.as_str()).unwrap_or_default().trim().to_string();
    let keys: Vec<String> = old
        .get("keys")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|k| k.as_str()).map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect())
        .unwrap_or_default();
    let overrides = old.get("engines").and_then(|v| v.as_object());
    let mut sources: Vec<String> = engines::ENGINES
        .iter()
        .filter(|e| overrides.and_then(|m| m.get(e.id)).and_then(|v| v.as_bool()).unwrap_or(e.default_on))
        .map(|e| e.id.to_string())
        .collect();
    let has_keyed = !provider.is_empty() && provider != "searxng" && !keys.is_empty();
    let mut services = serde_json::Map::new();
    if has_keyed {
        services.insert(provider.clone(), serde_json::json!({ "keys": keys }));
    }
    let keyed_first = has_keyed && old_enabled;
    if keyed_first {
        sources.insert(0, provider);
    }
    let enabled = old_enabled || !sources.is_empty();
    let urls = old.get("searxng_base_urls").cloned().unwrap_or_else(|| serde_json::json!([]));
    *search = serde_json::json!({
        "enabled": enabled,
        "sources": sources,
        "mode": if keyed_first { "sequential" } else { "parallel" },
        "services": services,
        "searxng_base_urls": urls,
    });
    true
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
    /// 执行路径：`sequential` / `parallel`；`disabled`（搜索已停用）/ `none`（来源列表为空）。
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

/// 完整搜索：「启用搜索」关 → 直接报停用；否则按 `sources` + `mode` 执行（顺序 / 一起）。
pub async fn web_search_detailed(
    cfg: &SearchProviderConfig,
    query: &str,
    count: Option<usize>,
    env: &SearchEnv,
) -> SearchOutcome {
    let mut out = SearchOutcome::default();
    if !cfg.enabled {
        out.source = "disabled".into();
        out.error = Some("搜索已停用（「启用搜索」已关闭）".into());
        return out;
    }
    let mut ids: Vec<&str> = Vec::new();
    for s in cfg.sources.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if !ids.contains(&s) {
            ids.push(s);
        }
    }
    if ids.is_empty() {
        out.source = "none".into();
        out.error = Some("未启用任何搜索来源（来源列表为空）".into());
        return out;
    }
    let cap = count.unwrap_or(MERGE_CAP).clamp(1, MERGE_CAP);
    let mut lists: Vec<Vec<SearchHit>> = Vec::new();
    match cfg.mode {
        SearchMode::Sequential => {
            out.source = "sequential".into();
            for id in ids {
                let (r, rep) = run_source(cfg, env, id, query, count).await;
                out.engines.push(rep);
                if let Ok(h) = r
                    && !h.is_empty()
                {
                    lists.push(h);
                    break;
                }
            }
        }
        SearchMode::Parallel => {
            out.source = "parallel".into();
            for (r, rep) in futures::future::join_all(ids.into_iter().map(|id| run_source(cfg, env, id, query, count))).await {
                out.engines.push(rep);
                if let Ok(h) = r {
                    lists.push(h);
                }
            }
        }
    }
    out.hits = merge_hits(&lists, cap);
    if out.engines.iter().all(|r| r.error.is_some()) {
        let errs: Vec<String> =
            out.engines.iter().map(|r| format!("{}: {}", r.engine, r.error.as_deref().unwrap_or_default())).collect();
        out.error = Some(format!("所有来源都失败: {}", errs.join("; ")));
    }
    out
}

/// 跑单个来源（key 服务或免 key 引擎），带单来源超时；返回结果与日志明细。
async fn run_source(
    cfg: &SearchProviderConfig,
    env: &SearchEnv,
    id: &str,
    query: &str,
    count: Option<usize>,
) -> (Result<Vec<SearchHit>, String>, EngineReport) {
    let t = std::time::Instant::now();
    let fut = async {
        if KEY_SERVICES.contains(&id) {
            keyed_search(cfg, env, id, query, count).await
        } else if let Some(e) = engines::ENGINES.iter().find(|e| e.id == id) {
            let ctx = engines::EngineCtx {
                bases: env.bases.clone(),
                searxng_urls: cfg.searxng_base_urls.iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect(),
                timeout: env.timeout,
            };
            (e.run)(&ctx, query, MERGE_CAP).await
        } else {
            Err(format!("未知搜索来源 `{id}`"))
        }
    };
    let r = match tokio::time::timeout(env.timeout, fut).await {
        Ok(r) => r,
        Err(_) => Err(format!("timeout ({}ms)", env.timeout.as_millis())),
    };
    let rep = EngineReport {
        engine: id.into(),
        count: r.as_ref().map_or(0, Vec::len),
        latency_ms: t.elapsed().as_millis() as u64,
        error: r.as_ref().err().cloned(),
    };
    (r, rep)
}

async fn keyed_search(
    cfg: &SearchProviderConfig,
    env: &SearchEnv,
    p: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let default_base = match p {
        "tavily" => TAVILY_BASE,
        "brave" => BRAVE_BASE,
        "exa" => EXA_BASE,
        "firecrawl" => FIRECRAWL_BASE,
        "deepseek" => DEEPSEEK_BASE,
        "youcom" => YOUCOM_BASE,
        "doubao" => DOUBAO_BASE,
        "tencent_yuanbao" => TENCENT_YUANBAO_BASE,
        _ => return Err(format!("未知 key 服务 `{p}`")),
    };
    let base = env.bases.get(p).map(String::as_str).unwrap_or(default_base);
    let keys: Vec<String> = cfg
        .services
        .get(p)
        .map(|s| s.keys.iter().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect())
        .unwrap_or_default();
    search_with_keys(p, base, keys, query, count).await
}

/// URL 去重键（同脚本）：百分号解码 + 去尾斜杠。
fn dedup_key(url: &str) -> String {
    percent_encoding::percent_decode_str(url).decode_utf8_lossy().trim_end_matches('/').to_string()
}

/// 各来源轮询交织（避免首个来源占满上限），URL 去重，截到 `cap`。
fn merge_hits(lists: &[Vec<SearchHit>], cap: usize) -> Vec<SearchHit> {
    let mut hits = Vec::new();
    let mut seen = std::collections::HashSet::new();
    'merge: for i in 0..lists.iter().map(Vec::len).max().unwrap_or(0) {
        for l in lists {
            if let Some(h) = l.get(i)
                && seen.insert(dedup_key(&h.url))
            {
                hits.push(h.clone());
                if hits.len() >= cap {
                    break 'merge;
                }
            }
        }
    }
    hits
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
            "deepseek" => deepseek_search(&client, base, &keys[idx], query, count).await,
            "youcom" => youcom_search(&client, base, &keys[idx], query, count).await,
            "doubao" => doubao_search(&client, base, &keys[idx], query, count).await,
            "tencent_yuanbao" => tencent_yuanbao_search(&client, base, &keys[idx], query, count).await,
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

fn str_of(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

/// DeepSeek：POST {base}/v1/tools/search，Bearer（api-docs.deepseek.com/guides/search）；count 1-50。
pub async fn deepseek_search(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut body = serde_json::json!({ "query": query });
    if let Some(n) = count {
        body["count"] = serde_json::json!(n.clamp(1, 50));
    }
    let v = call(client.post(format!("{base}/v1/tools/search")).bearer_auth(key).json(&body)).await?;
    Ok(v["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| SearchHit {
                    title: str_of(&r["title"]),
                    url: str_of(&r["url"]),
                    snippet: str_of(&r["snippet"]),
                    page_age: r["date"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// You.com：POST {base}/v1/search，头 X-API-Key（docs.you.com/api-reference/search/v1-search）；count 1-100。
pub async fn youcom_search(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut body = serde_json::json!({ "query": query });
    if let Some(n) = count {
        body["count"] = serde_json::json!(n.clamp(1, 100));
    }
    let v = call(client.post(format!("{base}/v1/search")).header("X-API-Key", key).json(&body)).await?;
    Ok(v["results"]["web"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| SearchHit {
                    title: str_of(&r["title"]),
                    url: str_of(&r["url"]),
                    snippet: r["snippets"][0].as_str().or_else(|| r["description"].as_str()).unwrap_or_default().to_string(),
                    page_age: r["page_age"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// 豆包（火山联网搜索）：POST {base}/search_api/web_search，Bearer（docs.volcengine.com Networkedsearch-2）。
/// 业务错误是 HTTP 200 + `ResponseMetadata.Error`，必须看 body。
pub async fn doubao_search(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut body = serde_json::json!({ "Query": query, "SearchType": "web" });
    if let Some(n) = count {
        body["Count"] = serde_json::json!(n.clamp(1, 50));
    }
    let v = call(client.post(format!("{base}/search_api/web_search")).bearer_auth(key).json(&body)).await?;
    let err = &v["ResponseMetadata"]["Error"];
    if err.is_object() {
        return Err(format!("{}: {}", str_of(&err["Code"]), str_of(&err["Message"])));
    }
    Ok(v["Result"]["WebResults"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| SearchHit {
                    title: str_of(&r["Title"]),
                    url: str_of(&r["Url"]),
                    snippet: r["Summary"].as_str().filter(|s| !s.is_empty()).or_else(|| r["Snippet"].as_str()).unwrap_or_default().to_string(),
                    page_age: r["PublishTime"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// 腾讯元宝（联网搜索 API Key 接入）：POST {base}/SearchPro，Bearer（cloud.tencent.com/document/product/1806/130615）。
/// `Response.Pages[]` 每项是 JSON 字符串，需二次解析；`Cnt` 只接受 10/20/…/50，故向上取整后再截断。
pub async fn tencent_yuanbao_search(
    client: &reqwest::Client,
    base: &str,
    key: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut body = serde_json::json!({ "Query": query });
    // shortcut: ≤10 条不带 Cnt（极速版不支持该参数，缺省即 10）。
    if let Some(n) = count.filter(|n| *n > 10) {
        body["Cnt"] = serde_json::json!(n.div_ceil(10).min(5) * 10);
    }
    let v = call(client.post(format!("{base}/SearchPro")).bearer_auth(key).json(&body)).await?;
    let resp = &v["Response"];
    if resp["Error"].is_object() {
        return Err(format!("{}: {}", str_of(&resp["Error"]["Code"]), str_of(&resp["Error"]["Message"])));
    }
    let pages = resp["Pages"].as_array().cloned().unwrap_or_default();
    let mut hits: Vec<SearchHit> = pages
        .iter()
        .filter_map(|p| serde_json::from_str::<serde_json::Value>(p.as_str()?).ok())
        .map(|p| SearchHit {
            title: str_of(&p["title"]),
            url: str_of(&p["url"]),
            snippet: str_of(&p["passage"]),
            page_age: p["date"].as_str().map(str::to_string),
        })
        .collect();
    if hits.is_empty() && !pages.is_empty() {
        return Err("Response.Pages 条目均无法解析为 JSON".into());
    }
    if let Some(n) = count {
        hits.truncate(n);
    }
    Ok(hits)
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

#[cfg(test)]
#[path = "test_modes.rs"]
mod test_modes;
