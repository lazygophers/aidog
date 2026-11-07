//! mcp_server 单元测试（票 10 协议行为 + 票 11 tools/call 分发）。

use super::*;
use aidog_core::mcp_tools::{McpToolsSettings, SearchProviderConfig};
use serde_json::{Value, json};

/// 带 id 的请求包装。
fn req(id: i64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn result_of(resp: &Value) -> &Value {
    resp.get("result").expect("response should carry result")
}

fn err_code(resp: &Value) -> i64 {
    resp.get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_i64())
        .expect("response should carry error.code")
}

/// handle_request 已 async（票 11 工具执行要出站）：同步测试用 current_thread runtime 包一层。
fn handle(msg: &Value) -> Option<Value> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(handle_request(msg))
}

#[test]
fn initialize_echoes_supported_version_and_advertises_tools() {
    let resp = handle(&req(1, "initialize", json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": {"name": "test-client", "version": "0"}
    })))
    .expect("initialize must produce a response");

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let result = result_of(&resp);
    assert_eq!(result["protocolVersion"], "2025-06-18");
    assert!(result["capabilities"]["tools"].is_object(), "capabilities.tools must be an object");
    assert_eq!(result["serverInfo"]["name"], "aidog");
    assert!(
        result["serverInfo"]["version"].as_str().is_some_and(|v| !v.is_empty()),
        "serverInfo.version must be a non-empty string"
    );
}

#[test]
fn initialize_negotiates_down_for_unsupported_client_version() {
    // 客户端报旧版本且不受支持 → 服务端回自己支持的最高版本（MCP lifecycle 规范）。
    let resp = handle(&req(2, "initialize", json!({
        "protocolVersion": "2024-10-07",
        "capabilities": {}
    })))
    .expect("initialize must produce a response");
    assert_eq!(result_of(&resp)["protocolVersion"], LATEST_PROTOCOL_VERSION);
}

#[test]
fn initialize_negotiates_up_for_older_supported_client_version() {
    // 客户端报 2025-03-26（受支持的旧版本）→ 原样回执，不强行抬版本。
    let resp = handle(&req(3, "initialize", json!({
        "protocolVersion": "2025-03-26",
        "capabilities": {}
    })))
    .expect("initialize must produce a response");
    assert_eq!(result_of(&resp)["protocolVersion"], "2025-03-26");
}

#[test]
fn tools_list_returns_the_four_placeholders_with_schemas() {
    let resp = handle(&req(4, "tools/list", json!({})))
        .expect("tools/list must produce a response");
    let tools = result_of(&resp)["tools"]
        .as_array()
        .expect("tools must be an array");

    let mut names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["generate_image", "generate_video", "web_fetch", "web_search"]
    );

    for t in tools {
        assert!(t["description"].as_str().is_some_and(|d| !d.is_empty()), "每工具要带 description");
        assert!(t["inputSchema"].is_object(), "每工具要带 inputSchema 对象");
        assert_eq!(t["inputSchema"]["type"], "object");
    }

    // web_search.query 必填；web_fetch.url 必填；generate_image/generate_video.prompt 必填。
    let schema_of = |name: &str| -> Value {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .map(|t| t["inputSchema"].clone())
            .expect("tool must exist")
    };
    assert!(schema_of("web_search")["required"]
        .as_array()
        .is_some_and(|r| r.contains(&json!("query"))));
    assert!(schema_of("web_fetch")["required"]
        .as_array()
        .is_some_and(|r| r.contains(&json!("url"))));
    assert!(schema_of("generate_image")["required"]
        .as_array()
        .is_some_and(|r| r.contains(&json!("prompt"))));
    assert!(schema_of("generate_video")["required"]
        .as_array()
        .is_some_and(|r| r.contains(&json!("prompt"))));
}

fn call_tool(name: &str, args: Value, settings: &McpToolsSettings) -> Value {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(handle_tools_call(name, &args, settings))
}

#[test]
fn tools_call_web_search_unconfigured_is_explicit_error() {
    let result = call_tool("web_search", json!({"query": "q"}), &McpToolsSettings::default());
    assert_eq!(result["isError"], true, "未配置必须 isError=true");
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("未配置"), "text={text}");
}

#[test]
fn tools_call_unverified_provider_is_explicit_error() {
    // unverified provider 不打网络：入口处直接显式报错（票面纪律：不猜端点）。
    let settings = McpToolsSettings {
        search: SearchProviderConfig {
            provider: "deepseek".into(),
            keys: vec!["k".into()],
            enabled: true,
            searxng_base_urls: vec![],
        },
        ..Default::default()
    };
    let result = call_tool("web_search", json!({"query": "q"}), &settings);
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"].as_str().unwrap().contains("unverified"),
        "text={}",
        result["content"][0]["text"]
    );
}

#[test]
fn tools_call_generate_image_and_video_still_placeholder() {
    // 票 12 范围：保持「未实现」占位。
    for name in ["generate_image", "generate_video"] {
        let result = call_tool(name, json!({"prompt": "p"}), &McpToolsSettings::default());
        assert_eq!(result["isError"], true, "{name} 必须 isError=true");
        assert!(
            result["content"][0]["text"].as_str().unwrap().contains("未实现"),
            "{name} 的文案要写明未实现"
        );
    }
}

#[test]
fn tools_call_unknown_tool_is_invalid_params() {
    let resp = handle(&req(6, "tools/call", json!({"name": "nope", "arguments": {}})))
        .expect("unknown tool must produce an error response");
    assert_eq!(err_code(&resp), -32602);
}

#[test]
fn unknown_method_is_method_not_found() {
    let resp = handle(&req(7, "resources/list", json!({})))
        .expect("unknown method must produce an error response");
    assert_eq!(err_code(&resp), -32601);
}

#[test]
fn ping_returns_empty_result() {
    let resp = handle(&req(8, "ping", json!({}))).expect("ping must produce a response");
    assert!(result_of(&resp).is_object());
}

#[test]
fn notifications_produce_no_response() {
    assert_eq!(
        handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})),
        None
    );
}

#[test]
fn env_settings_missing_or_invalid_falls_back_to_default() {
    // 不设 env 时 = 未配置（不 panic）。env 读取进程级，这里只验 default 路径的形状。
    let s = McpToolsSettings::default();
    assert_eq!(call_tool("web_search", json!({"query": "q"}), &s)["isError"], true);
}
