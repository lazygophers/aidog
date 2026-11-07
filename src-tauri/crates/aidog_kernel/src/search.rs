//! web_search / web_fetch 的 provider 适配层（magpie 票 11，spec D5a）。
//!
//! 端点与鉴权均取自一手文档（票 ## Answer 有清单）：已验证 5 家（tavily / brave / exa /
//! firecrawl / searxng）实现真实调用；未能从一手文档确认的 4 家（youcom / deepseek /
//! doubao / tencent_yuanbao）按票面纪律**不猜端点**，调用一律返回明确的 unverified 错误。
//!
//! 统一出参 [`SearchHit`]（title / url / snippet），provider 特有字段丢弃；多 key 轮换
//! （调用间递进起点），全 key 失败聚合报错。测试全打本地假上游（test-no-real-calls.md）。

use aidog_core::mcp_tools::SearchProviderConfig;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 官方 API base（测试经同名适配函数注入本地假上游 base，不在设置里放可覆盖字段）。
pub const TAVILY_BASE: &str = "https://api.tavily.com";
pub const BRAVE_BASE: &str = "https://api.search.brave.com";
pub const EXA_BASE: &str = "https://api.exa.ai";
pub const FIRECRAWL_BASE: &str = "https://api.firecrawl.dev";

/// 轮换游标：跨调用递进，起点 = 游标 % key 数，均摊分摊各 key 的用量。
pub static ROTATION: AtomicUsize = AtomicUsize::new(0);

/// 搜索结果统一出参（provider 特有字段丢弃，spec D5a）。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// web_fetch 的大小上限（票面「如 1MB」）：超过即截断，不报错。
pub const FETCH_MAX_BYTES: usize = 1024 * 1024;
/// web_fetch / web_search 单次出站超时。
pub const FETCH_TIMEOUT_SECS: u64 = 30;

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("build http client: {e}"))
}

/// `web_search` 工具入口：按设置分发到 provider 适配器。
pub async fn web_search(
    cfg: &SearchProviderConfig,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    if cfg.provider.is_empty() {
        return Err("未配置搜索 provider（在 aidog 设置里选择并填 API key）".into());
    }
    if !cfg.enabled {
        return Err(format!("搜索 provider `{}` 已禁用", cfg.provider));
    }
    let keys: Vec<String> = cfg
        .keys
        .iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    match cfg.provider.as_str() {
        "tavily" => search_with_keys("tavily", TAVILY_BASE, keys, query, count).await,
        "brave" => search_with_keys("brave", BRAVE_BASE, keys, query, count).await,
        "exa" => search_with_keys("exa", EXA_BASE, keys, query, count).await,
        "firecrawl" => search_with_keys("firecrawl", FIRECRAWL_BASE, keys, query, count).await,
        "searxng" => {
            let urls: Vec<String> = cfg
                .searxng_base_urls
                .iter()
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty())
                .collect();
            if urls.is_empty() {
                return Err("SearXNG 未配置实例 URL（searxng_base_urls）".into());
            }
            // 多实例按序尝试，首个成功实例的结果生效（实例本身无 key）。
            let client = http_client()?;
            let mut errs = Vec::new();
            for u in &urls {
                match searxng_search(&client, u, query, count).await {
                    Ok(h) => return Ok(h),
                    Err(e) => errs.push(format!("{u}: {e}")),
                }
            }
            Err(format!("全部 {} 个 SearXNG 实例失败: {}", urls.len(), errs.join("; ")))
        }
        // 未验证 provider：不猜端点，显式报错（票面纪律）。
        p => Err(format!(
            "provider `{p}` 的公开搜索 API 未能从一手文档确认（unverified），未实现"
        )),
    }
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

async fn call(
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

fn truncate_str(s: &str, max: usize) -> &str {
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
                })
                .collect()
        })
        .unwrap_or_default())
}

/// SearXNG：GET {base}/search?q=…&format=json（实例需开启 JSON format，searxng webapp.py:640/696）。
pub async fn searxng_search(
    client: &reqwest::Client,
    base_url: &str,
    query: &str,
    count: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let mut req = client
        .get(format!("{base_url}/search"))
        .query(&[("q", query), ("format", "json")]);
    if let Some(n) = count {
        req = req.query(&[("count", n.to_string())]);
    }
    let v = call(req).await?;
    Ok(v["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| SearchHit {
                    title: r["title"].as_str().unwrap_or_default().to_string(),
                    url: r["url"].as_str().unwrap_or_default().to_string(),
                    snippet: r["content"].as_str().unwrap_or_default().to_string(),
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
