//! 工具执行 + 工具日志（网关-网页搜索 票 03）。
//!
//! 触发方：`mcp`（HTTP MCP 的 tools/call）与 `gateway`（网关代执行，票 04）共用本入口，
//! 每次执行写一行 `tool_log`（log.db），不进 proxy_log。隐私：搜索词 / URL / 结果内容受
//! `ProxyLogSettings.log_user_request` 控制，关时写空串。

use crate::mcp_tools::McpToolsSettings;
use aidog_db::Db;
use aidog_db::models::{ProxyLogSettings, ToolLog};
use serde_json::Value;

pub const TRIGGER_GATEWAY: &str = "gateway";
pub const TRIGGER_MCP: &str = "mcp";

/// 一次工具执行的归属信息。
pub struct ToolCtx<'a> {
    pub db: &'a Db,
    pub trigger: &'static str,
    /// 分组名（Group.name）。
    pub group: &'a str,
    /// 代执行时被绕开的平台名；MCP 触发传空串。
    pub platform: &'a str,
}

/// 结果正文入库上限（字符）：超长（如 data URL 图片）截断，防撑库。
const RESULT_LOG_MAX_CHARS: usize = 20_000;

/// 执行工具并落工具日志。失败返回 `Err(文本)`（调用方决定映射成 isError / 错误块）。
pub async fn run_tool(
    ctx: &ToolCtx<'_>,
    name: &str,
    args: &Value,
    settings: &McpToolsSettings,
) -> Result<String, String> {
    use aidog_search as search;
    let started = std::time::Instant::now();
    let mut engines: Vec<search::EngineReport> = Vec::new();
    let mut count = 0usize;
    let subject = match name {
        "web_fetch" => args["url"].as_str(),
        "web_search" => args["query"].as_str(),
        _ => args["prompt"].as_str(),
    }
    .unwrap_or_default()
    .to_string();
    let outcome: Result<String, String> = match name {
        "web_search" => {
            let n = args["count"].as_u64().map(|v| v as usize);
            let o = search::web_search_detailed(&settings.search, &subject, n, &search::SearchEnv::default()).await;
            engines = o.engines;
            count = o.hits.len();
            match o.error {
                Some(e) => Err(e),
                None => Ok(search::format_hits(&o.hits)),
            }
        }
        "web_fetch" if !settings.fetch.enabled => Err("web_fetch 已停用（「启用抓网页」已关闭）".into()),
        "web_fetch" => search::fetch_url_text(&subject, settings.fetch.format).await,
        "generate_image" => {
            crate::media::generate_image(settings.image.as_ref(), &subject, args["size"].as_str()).await
        }
        _ => Err(format!("未知工具：`{name}`")),
    };
    if name != "web_search" && outcome.is_ok() {
        count = 1;
    }
    write_log(ctx, name, subject, count, &engines, started, &outcome).await;
    outcome
}

/// 网关代执行的搜索（票 04）：与 [`run_tool`] 的 `web_search` 同一引擎链与日志写入，但返回结构化命中，
/// 并按 `keep` 过滤（域名白/黑名单）；日志的结果条数 / 正文取过滤后。引擎全失败 = `Err`。
pub async fn run_web_search_hits(
    ctx: &ToolCtx<'_>,
    query: &str,
    settings: &McpToolsSettings,
    keep: impl Fn(&aidog_search::SearchHit) -> bool,
) -> Result<Vec<aidog_search::SearchHit>, String> {
    use aidog_search as search;
    let started = std::time::Instant::now();
    let o = search::web_search_detailed(&settings.search, query, None, &search::SearchEnv::default()).await;
    let outcome = match o.error {
        Some(e) => Err(e),
        None => Ok(o.hits.into_iter().filter(|h| keep(h)).collect::<Vec<_>>()),
    };
    let logged = outcome.as_ref().map(|h| search::format_hits(h)).map_err(Clone::clone);
    let count = outcome.as_ref().map_or(0, Vec::len);
    write_log(ctx, "web_search", query.to_string(), count, &o.engines, started, &logged).await;
    outcome
}

async fn write_log(
    ctx: &ToolCtx<'_>,
    name: &str,
    subject: String,
    count: usize,
    engines: &[aidog_search::EngineReport],
    started: std::time::Instant,
    outcome: &Result<String, String>,
) {
    let privacy_on = log_user_request(ctx.db).await;
    let row = ToolLog {
        tool: name.to_string(),
        trigger: ctx.trigger.to_string(),
        group_name: ctx.group.to_string(),
        platform: ctx.platform.to_string(),
        query: if privacy_on { subject } else { String::new() },
        result_count: count as i64,
        engines: engines_json(engines),
        duration_ms: started.elapsed().as_millis() as i64,
        success: outcome.is_ok(),
        error: outcome.as_ref().err().cloned().unwrap_or_default(),
        result: match (outcome, privacy_on) {
            (Ok(t), true) => aidog_search::truncate_str(t, RESULT_LOG_MAX_CHARS).to_string(),
            _ => String::new(),
        },
        ..Default::default()
    };
    if let Err(e) = aidog_db::insert_tool_log(ctx.db, &row).await {
        tracing::warn!(error = %e, tool = name, "write tool_log failed");
    }
}

fn engines_json(engines: &[aidog_search::EngineReport]) -> String {
    let v: Vec<Value> = engines
        .iter()
        .map(|e| serde_json::json!({"engine": e.engine, "count": e.count, "latency_ms": e.latency_ms, "error": e.error}))
        .collect();
    serde_json::to_string(&v).unwrap_or_else(|_| "[]".into())
}

/// 隐私开关：与 proxy_log 同一个 `log_user_request`（缺省关）。
async fn log_user_request(db: &Db) -> bool {
    aidog_db::get_setting(db, "proxy", "logging")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value::<ProxyLogSettings>(v).ok())
        .is_some_and(|s| s.log_user_request)
}
