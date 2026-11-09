//! `aidog-kernel mcp` 子命令：stdio 上的 MCP server 骨架（magpie 票 10）。
//!
//! 帧格式 = newline-delimited JSON-RPC（每条消息一行、禁内嵌换行、stdout 禁写非 MCP 消息），
//! 出处：MCP 规范 2025-06-18「Transports → stdio」
//! https://modelcontextprotocol.io/specification/2025-06-18/basic/transports
//!
//! 协议层：initialize 握手 + tools/list（4 工具 schema）。tools/call 里
//! web_search / web_fetch 走 [`search`] 的真实现（票 11），generate_image /
//! generate_video 走 [`media`]（票 12：image 为 OpenAI images 兼容实现，video
//! 仅接口 + 明确错误）。provider 设置经环境变量 `AIDOG_MCP_TOOLS_JSON`
//! 注入（整库 SQLCipher，本进程不连库——见 main.rs「不连库不抢锁」；由 aidog 写 agent
//! 的 mcpServers 配置时嵌入）。不碰 aidog_mcp（那是第三方 MCP server 的管理层，语义不同）；
//! 桌面版不 spawn 本进程，由用户的 agent（Claude Code / Codex）按 mcpServers 配置自己拉起。

use crate::media;
use aidog_search as search;
use aidog_core::mcp_tools::McpToolsSettings;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// MCP 进程的 provider 设置来源：`AIDOG_MCP_TOOLS_JSON`（McpToolsSettings 的 JSON）。
/// 缺失或解析失败按未配置处理（诊断写 stderr，stdout 禁非 MCP 消息）。
fn mcp_tools_settings() -> McpToolsSettings {
    match std::env::var("AIDOG_MCP_TOOLS_JSON") {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("aidog-kernel mcp: AIDOG_MCP_TOOLS_JSON parse failed (treated as unconfigured): {e}");
                McpToolsSettings::default()
            }
        },
        Err(_) => McpToolsSettings::default(),
    }
}

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
pub async fn handle_request(msg: &Value) -> Option<Value> {
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
            let settings = mcp_tools_settings();
            let result =
                handle_tools_call(name, &msg["params"]["arguments"], &settings).await;
            Some(response_of(result))
        }
        // 未知方法按 JSON-RPC -32601（notification 已在上面的 id 过滤处被忽略）。
        _ => Some(error_of(-32601, "Method not found", Some(json!(method)))),
    }
}

/// tools/call 的工具执行（票 11：web_search / web_fetch 真实现；image/video 留票 12）。
/// 工具执行失败按 MCP 规范走 CallToolResult.isError=true，不是协议层 error。
pub async fn handle_tools_call(
    name: &str,
    args: &Value,
    settings: &McpToolsSettings,
) -> Value {
    let outcome = match name {
        "web_search" => {
            let query = args["query"].as_str().unwrap_or_default();
            let count = args["count"].as_u64().map(|v| v as usize);
            search::web_search(&settings.search, query, count)
                .await
                .map(|hits| search::format_hits(&hits))
        }
        "web_fetch" => {
            search::fetch_url_text(args["url"].as_str().unwrap_or_default()).await
        }
        "generate_image" => {
            let prompt = args["prompt"].as_str().unwrap_or_default();
            let size = args["size"].as_str();
            media::generate_image(settings.image.as_ref(), prompt, size).await
        }
        "generate_video" => {
            let prompt = args["prompt"].as_str().unwrap_or_default();
            media::generate_video(settings.video.as_ref(), prompt).await
        }
        // handle_request 已按 tool_definitions 校验过名字，此臂正常不可达。
        _ => Err(format!("未知工具：`{name}`")),
    };
    match outcome {
        Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

/// 写一行响应（newline-delimited JSON：单行、无内嵌换行——serde_json 序列化天然满足）。
async fn write_msg<W: tokio::io::AsyncWrite + Unpin>(
    out: &mut W,
    msg: &Value,
) -> Result<(), String> {
    let mut line = serde_json::to_string(msg).map_err(|e| format!("serialize response: {e}"))?;
    line.push('\n');
    out.write_all(line.as_bytes())
        .await
        .map_err(|e| format!("write stdout: {e}"))?;
    out.flush().await.map_err(|e| format!("flush stdout: {e}"))?;
    Ok(())
}

/// stdio 读写循环：逐行读 JSON-RPC、应答，stdin EOF 即退出。
pub async fn run() -> Result<(), String> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(l)) => l,
            Ok(None) => return Ok(()),
            Err(e) => return Err(format!("read stdin: {e}")),
        };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp =
                    json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error", "data": e.to_string()}});
                write_msg(&mut stdout, &resp).await?;
                continue;
            }
        };
        if let Some(resp) = handle_request(&msg).await {
            write_msg(&mut stdout, &resp).await?;
        }
    }
}

#[cfg(test)]
#[path = "test_mcp_server.rs"]
mod test_mcp_server;
