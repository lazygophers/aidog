use super::*;
use std::collections::BTreeMap;

#[test]
fn parse_claude_entry_stdio() {
    let v = serde_json::json!({
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "foo"],
        "env": {"API_KEY": "x", "DEBUG": "1"}
    });
    let cfg = parse_claude_entry(&v).expect("parse");
    assert_eq!(cfg.transport, McpTransport::Stdio);
    assert_eq!(cfg.command, "npx");
    assert_eq!(cfg.args, vec!["-y".to_string(), "foo".to_string()]);
    assert_eq!(cfg.env.get("API_KEY").unwrap(), "x");
}

#[test]
fn parse_claude_entry_http() {
    let v = serde_json::json!({
        "type": "http",
        "url": "https://mcp.example.com/mcp",
        "headers": {"Authorization": "Bearer x"}
    });
    let cfg = parse_claude_entry(&v).expect("parse");
    assert_eq!(cfg.transport, McpTransport::Http);
    assert_eq!(cfg.url, "https://mcp.example.com/mcp");
    assert_eq!(cfg.headers.get("Authorization").unwrap(), "Bearer x");
    assert!(cfg.command.is_empty());
}

#[test]
fn build_claude_entry_roundtrip() {
    let cfg = McpConfigRaw {
        transport: McpTransport::Stdio,
        command: "uvx".into(),
        args: vec!["duckduckgo-mcp-server".into()],
        env: {
            let mut m = BTreeMap::new();
            m.insert("HTTPS_PROXY".into(), "http://127.0.0.1:7890".into());
            m
        },
        url: String::new(),
        headers: BTreeMap::new(),
    };
    let entry = build_claude_entry(&cfg);
    let back = parse_claude_entry(&entry).expect("roundtrip");
    assert_eq!(back.command, "uvx");
    assert_eq!(back.args, cfg.args);
    assert_eq!(back.env.get("HTTPS_PROXY").unwrap(), "http://127.0.0.1:7890");
}
