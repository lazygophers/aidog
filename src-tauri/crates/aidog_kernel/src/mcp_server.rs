//! `aidog-kernel mcp` 子命令：stdio 上的 MCP server 骨架（magpie 票 10）。
//!
//! 帧格式 = newline-delimited JSON-RPC（每条消息一行、禁内嵌换行、stdout 禁写非 MCP 消息），
//! 出处：MCP 规范 2025-06-18「Transports → stdio」
//! https://modelcontextprotocol.io/specification/2025-06-18/basic/transports
//!
//! 骨架只做协议层：initialize 握手 + tools/list（4 工具 schema）+ tools/call 一律回
//! 「未实现」（isError）；搜索 / 生图 / 生视频 provider 的实现留票 11/12。不碰
//! aidog_mcp（那是第三方 MCP server 的管理层，语义不同）；桌面版不 spawn 本进程，
//! 由用户的 agent（Claude Code / Codex）按 mcpServers 配置自己拉起。

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
pub fn handle_request(msg: &Value) -> Option<Value> {
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
            // 骨架期一律「未实现」：按 MCP 规范走 CallToolResult.isError=true
            // （工具执行失败属于结果，不是协议层 error）。
            let result = json!({
                "content": [{"type": "text", "text": format!("未实现：工具 `{name}` 尚未接入 provider（票 11/12）")}],
                "isError": true
            });
            Some(response_of(result))
        }
        // 未知方法按 JSON-RPC -32601（notification 已在上面的 id 过滤处被忽略）。
        _ => Some(error_of(-32601, "Method not found", Some(json!(method)))),
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
        if let Some(resp) = handle_request(&msg) {
            write_msg(&mut stdout, &resp).await?;
        }
    }
}

#[cfg(test)]
#[path = "test_mcp_server.rs"]
mod test_mcp_server;
