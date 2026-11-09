//! HTTP MCP（`POST /mcp`）的 JSON-RPC 协议层与工具定义（网关-网页搜索 票 03）。
//!
//! 传输 = Streamable HTTP 的最小实现（MCP 规范 2025-06-18「Transports → Streamable HTTP」
//! https://modelcontextprotocol.io/specification/2025-06-18/basic/transports）：
//! 每个 POST 一条 JSON-RPC 消息，请求回 `application/json`，通知回 202。HTTP 入口见
//! `gateway/proxy/mcp_http.rs`；工具执行与工具日志见 `tool_exec.rs`。
//! 不碰 aidog_mcp（那是第三方 MCP server 的管理层，语义不同）。

use crate::tool_exec::{TRIGGER_MCP, ToolCtx, run_tool};
use aidog_db::Db;
use serde_json::{Value, json};

/// 支持的最高协议版本（协商失败 / 客户端版本不受支持时的回落值）。
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

/// 历史版本协商表：客户端报这些版本时原样回执（MCP lifecycle：版本一致才继续）。
const SUPPORTED_VERSIONS: [&str; 2] = ["2025-06-18", "2025-03-26"];

/// 4 个占位工具的 JSON Schema（per MCP spec `Tool.inputSchema`）。实现留票 11/12。
fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "web_search",
            "description": "联网搜索：按查询词返回网页结果列表（标题、链接、摘要）。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "搜索查询词"},
                    "count": {"type": "integer", "description": "返回结果条数（缺省由 provider 决定）"}
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "web_fetch",
            "description": "抓取一个网页 / URL 的正文内容并转为文本返回。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "要抓取的完整 URL"}
                },
                "required": ["url"]
            }
        }),
        json!({
            "name": "generate_image",
            "description": "文生图：按提示词生成图片，返回图片（URL/base64）。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "图片提示词"},
                    "size": {"type": "string", "description": "图片尺寸，如 1024x1024（缺省由 provider 决定）"}
                },
                "required": ["prompt"]
            }
        }),
        json!({
            "name": "generate_video",
            "description": "文生视频：按提示词生成视频（首版未实现，配置视频 provider 后开放）。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "视频提示词"},
                    "duration_seconds": {"type": "number", "description": "视频时长（秒，缺省由 provider 决定）"}
                },
                "required": ["prompt"]
            }
        }),
    ]
}

/// 一条 JSON-RPC 消息 → 应答（`None` = 不应答：notification / 无关消息 / response）。
pub async fn handle_request(msg: &Value, db: &Db, group: &str) -> Option<Value> {
    // 无 id = notification，按 JSON-RPC 不回包；带 id 但无 method = response，忽略。
    let id = msg.get("id")?;
    let method = msg.get("method")?.as_str()?;

    let response_of = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result});
    let error_of = |code: i64, message: &str, data: Option<Value>| {
        let mut e = json!({"code": code, "message": message});
        if let Some(d) = data {
            e["data"] = d;
        }
        json!({"jsonrpc": "2.0", "id": id, "error": e})
    };

    match method {
        "initialize" => {
            // 版本协商：客户端报受支持的版本 → 原样回执；否则回我们支持的最高版本
            // （MCP lifecycle：server responds with the version it supports）。
            let client_version = msg["params"]["protocolVersion"].as_str();
            let agreed = client_version
                .filter(|v| SUPPORTED_VERSIONS.contains(v))
                .unwrap_or(LATEST_PROTOCOL_VERSION);
            let result = json!({
                "protocolVersion": agreed,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "aidog", "version": env!("CARGO_PKG_VERSION")}
            });
            Some(response_of(result))
        }
        "ping" => Some(response_of(json!({}))),
        "tools/list" => {
            let result = json!({"tools": tool_definitions()});
            Some(response_of(result))
        }
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or_default();
            if !tool_definitions().iter().any(|t| t["name"] == name) {
                return Some(error_of(-32602, "Unknown tool", Some(json!(name))));
            }
            let settings = crate::mcp_tools::load_mcp_tools_settings(db).await;
            let ctx = ToolCtx { db, trigger: TRIGGER_MCP, group, platform: "" };
            let result = handle_tools_call(&ctx, name, &msg["params"]["arguments"], &settings).await;
            Some(response_of(result))
        }
        // 未知方法按 JSON-RPC -32601（notification 已在上面的 id 过滤处被忽略）。
        _ => Some(error_of(-32601, "Method not found", Some(json!(method)))),
    }
}

/// tools/call 的工具执行（经 [`run_tool`]，写工具日志）。
/// 工具执行失败按 MCP 规范走 CallToolResult.isError=true，不是协议层 error。
pub async fn handle_tools_call(
    ctx: &ToolCtx<'_>,
    name: &str,
    args: &Value,
    settings: &crate::mcp_tools::McpToolsSettings,
) -> Value {
    match run_tool(ctx, name, args, settings).await {
        Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

#[cfg(test)]
#[path = "test_mcp_server.rs"]
mod test_mcp_server;
