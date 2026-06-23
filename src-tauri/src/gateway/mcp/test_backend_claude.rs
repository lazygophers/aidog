use super::*;
use std::collections::BTreeMap;

#[test]
fn json_object_to_map_extracts_strings() {
    let v = serde_json::json!({"KEY": "val", "NUM": 42, "BOOL": true});
    let result = json_object_to_map(Some(&v));
    assert_eq!(result.get("KEY").unwrap(), "val");
    assert!(!result.contains_key("NUM"), "non-string values should be filtered");
    assert!(!result.contains_key("BOOL"), "bool values should be filtered");
}

#[test]
fn json_object_to_map_none_returns_empty() {
    let result = json_object_to_map(None);
    assert!(result.is_empty());
}

#[test]
fn build_claude_entry_http_no_headers() {
    let cfg = McpConfigRaw {
        transport: McpTransport::Http,
        command: String::new(),
        args: vec![],
        env: BTreeMap::new(),
        url: "https://api.example.com/mcp".into(),
        headers: BTreeMap::new(), // empty headers
    };
    let entry = build_claude_entry(&cfg);
    assert_eq!(entry.get("type").and_then(|v| v.as_str()), Some("http"));
    assert_eq!(entry.get("url").and_then(|v| v.as_str()), Some("https://api.example.com/mcp"));
    assert!(entry.get("headers").is_none(), "empty headers should not be included");
}

#[test]
fn build_claude_entry_http_with_headers() {
    let cfg = McpConfigRaw {
        transport: McpTransport::Http,
        command: String::new(),
        args: vec![],
        env: BTreeMap::new(),
        url: "https://api.example.com/mcp".into(),
        headers: {
            let mut m = BTreeMap::new();
            m.insert("Authorization".into(), "Bearer tok".into());
            m
        },
    };
    let entry = build_claude_entry(&cfg);
    let headers = entry.get("headers").expect("headers should be present");
    assert_eq!(headers.get("Authorization").and_then(|v| v.as_str()), Some("Bearer tok"));
}

#[test]
fn build_claude_entry_sse_transport() {
    let cfg = McpConfigRaw {
        transport: McpTransport::Sse,
        command: String::new(),
        args: vec![],
        env: BTreeMap::new(),
        url: "https://sse.example.com/events".into(),
        headers: BTreeMap::new(),
    };
    let entry = build_claude_entry(&cfg);
    assert_eq!(entry.get("type").and_then(|v| v.as_str()), Some("sse"));
}

#[test]
fn build_claude_entry_stdio_empty_env_no_env_field() {
    let cfg = McpConfigRaw {
        transport: McpTransport::Stdio,
        command: "cmd".into(),
        args: vec!["arg1".into()],
        env: BTreeMap::new(), // empty env
        url: String::new(),
        headers: BTreeMap::new(),
    };
    let entry = build_claude_entry(&cfg);
    assert!(entry.get("env").is_none(), "empty env should not be included");
    let back = parse_claude_entry(&entry).unwrap();
    assert_eq!(back.args, vec!["arg1"]);
}

/// read_claude_json on missing file returns empty object.
#[test]
fn read_claude_json_missing_file_returns_empty() {
    let path = std::path::PathBuf::from("/tmp/nonexistent_aidog_claude_test_12345678.json");
    let result = read_claude_json(&path).expect("should succeed for missing file");
    assert!(result.as_object().unwrap().is_empty());
}

/// read_claude_json on empty file returns empty object.
#[test]
fn read_claude_json_empty_file_returns_empty() {
    let path = std::env::temp_dir().join("aidog_test_empty_claude.json");
    std::fs::write(&path, "").unwrap();
    let result = read_claude_json(&path).expect("should succeed for empty file");
    assert!(result.as_object().unwrap().is_empty());
    let _ = std::fs::remove_file(&path);
}

/// write_claude_json + read_claude_json roundtrip.
#[test]
fn write_and_read_claude_json_roundtrip() {
    let path = std::env::temp_dir().join(format!(
        "aidog_test_claude_rt_{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    let data = serde_json::json!({"key": "value", "nested": {"a": 1}});
    write_claude_json(&path, &data).expect("write should succeed");
    let back = read_claude_json(&path).expect("read should succeed");
    assert_eq!(back.get("key").and_then(|v| v.as_str()), Some("value"));
    let _ = std::fs::remove_file(&path);
}

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
