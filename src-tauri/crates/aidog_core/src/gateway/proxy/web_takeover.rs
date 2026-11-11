//! 网关代执行服务端网页工具（网关-网页搜索 票 04，ADR 0012 决策 1 / 2）。
//!
//! 请求 tools 含 `web_search_*` / `web_fetch_*` 服务端工具、且尝试选中的平台不是 Anthropic 官方时，
//! 不调模型：执行一次工具（`tool_exec`，写工具日志，不写代理日志），按官方结果块格式应答。
//!
//! 形状出处（第 1 类，官方文档）：
//! - web_search：<https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool>
//!   （`server_tool_use` / `web_search_tool_result` / `web_search_tool_result_error` / 错误码 / 流式事件 / `usage.server_tool_use`）
//! - web_fetch：<https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-fetch-tool>
//! - 域名过滤规则：<https://platform.claude.com/docs/en/agents-and-tools/tool-use/server-tools#domain-filtering>
//!
//! 逆向，版本 2.1.319（本机 Claude Code 二进制，非官方出处；ADR 写 2.1.318）：WebSearch 子请求只读应答里的
//! `server_tool_use`（计次并冲刷前文）、`web_search_tool_result`（`content` 为数组时取每项 `title` / `url`，
//! 否则读 `content.error_code`）与 `text` 块；`encrypted_content` / `page_age` 不被读取。

use super::headers::is_official_anthropic_host;
use super::*;
use crate::tool_exec::{TRIGGER_GATEWAY, ToolCtx, run_tool, run_web_search_hits};
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Search,
    Fetch,
}

/// 请求里声明的服务端网页工具。
#[derive(Debug)]
pub(crate) struct WebTool {
    pub(crate) kind: Kind,
    allowed: Vec<String>,
    blocked: Vec<String>,
}

fn kind_of(tool: &Value) -> Option<Kind> {
    let t = tool.get("type").and_then(Value::as_str)?;
    if t.starts_with("web_search_") {
        Some(Kind::Search)
    } else if t.starts_with("web_fetch_") {
        Some(Kind::Fetch)
    } else {
        None
    }
}

/// body 的 tools 是否全是服务端网页工具（混有其它工具则不接管，照常转发模型；入口早期判定：代执行的请求不写代理日志，见 handler）。
pub(crate) fn has_web_server_tool(body: Option<&Value>) -> bool {
    body.and_then(|b| b.get("tools")).and_then(Value::as_array).is_some_and(|ts| !ts.is_empty() && ts.iter().all(|t| kind_of(t).is_some()))
}

/// 取请求声明的服务端网页工具；`tool_choice` 指名其一时取该工具，否则取第一个。
pub(crate) fn detect(body: &Value) -> Option<WebTool> {
    let tools = body.get("tools")?.as_array()?;
    if !tools.iter().all(|t| kind_of(t).is_some()) {
        return None;
    }
    let forced = body["tool_choice"]["name"].as_str();
    let tool = tools
        .iter()
        .filter(|t| kind_of(t).is_some())
        .find(|t| forced.is_some() && t["name"].as_str() == forced)
        .or_else(|| tools.iter().find(|t| kind_of(t).is_some()))?;
    let list = |k: &str| -> Vec<String> {
        tool[k].as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect()
    };
    Some(WebTool { kind: kind_of(tool)?, allowed: list("allowed_domains"), blocked: list("blocked_domains") })
}

/// 本次尝试选中的平台是否由网关接管：官方 Anthropic 原样转发，其余接管。
pub(crate) fn takes_over(platform_base_url: &str) -> bool {
    !is_official_anthropic_host(platform_base_url)
}

/// 最后一条用户消息的文本（字符串或 text 块拼接）。
fn last_user_text(body: &Value) -> String {
    let Some(m) = body["messages"].as_array().and_then(|ms| ms.iter().rev().find(|m| m["role"] == "user")) else {
        return String::new();
    };
    match &m["content"] {
        Value::String(s) => s.clone(),
        Value::Array(bs) => bs.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// 搜索词：Claude Code 形如 `Perform a web search for the query: <词>`（逆向，2.1.319），其余取全文。
fn extract_query(text: &str) -> String {
    const PREFIX: &str = "Perform a web search for the query:";
    text.split_once(PREFIX).map_or(text, |(_, q)| q).trim().to_string()
}

/// 消息里的第一个 http(s) URL。
fn extract_url(text: &str) -> Option<String> {
    let start = text.find("https://").into_iter().chain(text.find("http://")).min()?;
    let tail = &text[start..];
    let end = tail.find(|c: char| c.is_whitespace() || "\"'<>`".contains(c)).unwrap_or(tail.len());
    let u = tail[..end].trim_end_matches(['.', ',', ';', ')', ']']);
    Some(u.to_string())
}

/// 域名过滤单条匹配：`example.com` 含子域，`docs.example.com` 只含该子域；
/// 带路径（`example.com/blog`、`*` 仅出现在路径里）时 web_search 按路径前缀匹配，web_fetch 带路径的条目恒不匹配。
fn entry_matches(url: &url::Url, entry: &str, with_path: bool) -> bool {
    let (dom, path) = entry.split_once('/').map_or((entry, None), |(d, p)| (d, Some(p)));
    let host = url.host_str().unwrap_or_default().to_lowercase();
    if !(host == dom || host.ends_with(&format!(".{dom}"))) {
        return false;
    }
    match path {
        None => true,
        Some(_) if !with_path => false,
        Some(p) => url.path().starts_with(&format!("/{}", p.split('*').next().unwrap_or_default())),
    }
}

impl WebTool {
    /// 过滤规则下该 URL 是否保留（白名单优先；两者都空 = 全保留；URL 解析不了 = 丢弃）。
    fn allows(&self, url: &str) -> bool {
        let Ok(u) = url::Url::parse(url) else { return false };
        let with_path = self.kind == Kind::Search;
        if !self.allowed.is_empty() {
            return self.allowed.iter().any(|e| entry_matches(&u, e, with_path));
        }
        !self.blocked.iter().any(|e| entry_matches(&u, e, with_path))
    }

    fn filtered(&self) -> bool {
        !self.allowed.is_empty() || !self.blocked.is_empty()
    }
}

fn error_content(kind: Kind, code: &str) -> Value {
    let t = if kind == Kind::Search { "web_search_tool_result_error" } else { "web_fetch_tool_result_error" };
    json!({"type": t, "error_code": code})
}

/// 执行一次工具，返回 (server_tool_use 块, 结果块, 简短文字)。
async fn execute(
    state: &Arc<ProxyState>,
    group: &Group,
    platform: &str,
    tool: &WebTool,
    body: &Value,
) -> (Value, Value, String) {
    let id = format!("srvtoolu_{}", uuid::Uuid::new_v4().simple());
    let text = last_user_text(body);
    let settings = crate::mcp_tools::load_mcp_tools_settings(&state.db).await;
    let ctx = ToolCtx { db: &state.db, trigger: TRIGGER_GATEWAY, group: &group.name, platform };
    let (name, input, result_type, content, summary) = match tool.kind {
        Kind::Search => {
            let query = extract_query(&text);
            let (content, summary) = if query.is_empty() {
                (error_content(Kind::Search, "invalid_tool_input"), "Web search failed: no query found in the last user message.".to_string())
            } else {
                match run_web_search_hits(&ctx, &query, &settings, |h| tool.allows(&h.url)).await {
                    Ok(hits) if hits.is_empty() => {
                        let why = if tool.filtered() { " after applying allowed_domains / blocked_domains" } else { "" };
                        (json!([]), format!("No web search results for \"{query}\"{why}."))
                    }
                    Ok(hits) => (
                        json!(hits.iter().map(|h| json!({"type":"web_search_result","title":h.title,"url":h.url,"page_age":h.page_age})).collect::<Vec<_>>()),
                        format!("Web search results for \"{query}\":\n\n{}", aidog_search::format_hits(&hits)),
                    ),
                    Err(e) => {
                        tracing::warn!(error = %e, "gateway web_search: all engines failed");
                        (error_content(Kind::Search, "unavailable"), "Web search is unavailable: all search engines failed.".to_string())
                    }
                }
            };
            ("web_search", json!({"query": query}), "web_search_tool_result", content, summary)
        }
        Kind::Fetch => {
            let url = extract_url(&text).unwrap_or_default();
            let (content, summary) = if url.is_empty() {
                (error_content(Kind::Fetch, "invalid_tool_input"), "Web fetch failed: no URL found in the last user message.".to_string())
            } else if !tool.allows(&url) {
                (error_content(Kind::Fetch, "url_not_allowed"), format!("Web fetch blocked by domain filters: {url}"))
            } else {
                match run_tool(&ctx, "web_fetch", &json!({"url": url}), &settings).await {
                    Ok(page) => {
                        let media_type = match settings.fetch.format {
                            aidog_search::FetchFormat::Markdown => "text/markdown",
                            aidog_search::FetchFormat::Html => "text/html",
                            aidog_search::FetchFormat::Text => "text/plain",
                        };
                        (
                        json!({"type":"web_fetch_result","url":url,
                            "content":{"type":"document","source":{"type":"text","media_type":media_type,"data":page},"title":url},
                            "retrieved_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)}),
                        format!("Fetched {url}."),
                        )
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "gateway web_fetch failed");
                        (error_content(Kind::Fetch, "url_not_accessible"), format!("Web fetch failed: {url}"))
                    }
                }
            };
            ("web_fetch", json!({"url": url}), "web_fetch_tool_result", content, summary)
        }
    };
    let use_block = json!({"type":"server_tool_use","id":id,"name":name,"input":input});
    let result_block = json!({"type":result_type,"tool_use_id":id,"content":content});
    (use_block, result_block, summary)
}

fn sse(event: &str, data: &Value) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

/// 流式：Anthropic SSE 事件序列（server_tool_use → 结果块 → 文字 → message_delta / message_stop）。
fn sse_body(msg_id: &str, model: &str, use_block: &Value, result_block: &Value, text: &str, usage_key: &str) -> String {
    let mut empty_use = use_block.clone();
    empty_use["input"] = json!({});
    let mut out = sse(
        "message_start",
        &json!({"type":"message_start","message":{"id":msg_id,"type":"message","role":"assistant","model":model,"content":[],
            "stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":0,"output_tokens":0}}}),
    );
    let start = |idx: usize, block: &Value, out: &mut String| {
        out.push_str(&sse("content_block_start", &json!({"type":"content_block_start","index":idx,"content_block":block})));
    };
    start(0, &empty_use, &mut out);
    out.push_str(&sse(
        "content_block_delta",
        &json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":use_block["input"].to_string()}}),
    ));
    out.push_str(&sse("content_block_stop", &json!({"type":"content_block_stop","index":0})));
    start(1, result_block, &mut out);
    out.push_str(&sse("content_block_stop", &json!({"type":"content_block_stop","index":1})));
    start(2, &json!({"type":"text","text":""}), &mut out);
    out.push_str(&sse(
        "content_block_delta",
        &json!({"type":"content_block_delta","index":2,"delta":{"type":"text_delta","text":text}}),
    ));
    out.push_str(&sse("content_block_stop", &json!({"type":"content_block_stop","index":2})));
    out.push_str(&sse(
        "message_delta",
        &json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},
            "usage":{"output_tokens":0,"server_tool_use":{usage_key:1}}}),
    ));
    out.push_str(&sse("message_stop", &json!({"type":"message_stop"})));
    out
}

/// 代执行并应答（非流式 JSON / 流式 SSE）。不碰上游、不写代理日志。
pub(crate) async fn respond(
    state: &Arc<ProxyState>,
    group: &Group,
    platform: &str,
    tool: &WebTool,
    body: &Value,
    model: &str,
    is_stream: bool,
) -> Response {
    let (use_block, result_block, text) = execute(state, group, platform, tool, body).await;
    let msg_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    let usage_key = if tool.kind == Kind::Search { "web_search_requests" } else { "web_fetch_requests" };
    let (ctype, payload) = if is_stream {
        ("text/event-stream", sse_body(&msg_id, model, &use_block, &result_block, &text, usage_key))
    } else {
        (
            "application/json",
            json!({"id":msg_id,"type":"message","role":"assistant","model":model,
                "content":[use_block, result_block, {"type":"text","text":text}],
                "stop_reason":"end_turn","stop_sequence":null,
                "usage":{"input_tokens":0,"output_tokens":0,"server_tool_use":{usage_key:1}}})
            .to_string(),
        )
    };
    let mut r = (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, ctype)], payload).into_response();
    inject_trace_header(&mut r);
    r
}

#[cfg(test)]
#[path = "test_web_takeover.rs"]
mod test_web_takeover;
