use super::*;

#[test]
fn agent_slug_roundtrip() {
    assert_eq!(McpAgent::ClaudeCode.slug(), "claude-code");
    assert_eq!(McpAgent::Codex.slug(), "codex");
    assert_eq!(
        McpAgent::from_slug("claude-code"),
        Some(McpAgent::ClaudeCode)
    );
    assert_eq!(McpAgent::from_slug("codex"), Some(McpAgent::Codex));
    assert_eq!(McpAgent::from_slug("claude"), None); // 非 "claude"
    assert_eq!(McpAgent::from_slug("unknown"), None);
}

#[test]
fn transport_parse_and_support() {
    assert_eq!(McpTransport::parse("stdio"), McpTransport::Stdio);
    assert_eq!(McpTransport::parse("HTTP"), McpTransport::Http);
    assert_eq!(McpTransport::parse("Sse"), McpTransport::Sse);
    assert_eq!(McpTransport::parse(""), McpTransport::Stdio);
    // codex 仅 stdio
    assert!(McpTransport::Stdio.supported_by(McpAgent::Codex));
    assert!(!McpTransport::Http.supported_by(McpAgent::Codex));
    assert!(!McpTransport::Sse.supported_by(McpAgent::Codex));
    // claude 全支持
    assert!(McpTransport::Http.supported_by(McpAgent::ClaudeCode));
}

#[test]
fn row_enabled_set_parse() {
    let row = McpServerRow {
        id: 1,
        name: "foo".into(),
        transport: "stdio".into(),
        command: "".into(),
        args_json: "[]".into(),
        env_json: "{}".into(),
        url: "".into(),
        headers_json: "{}".into(),
        enabled_agents: "claude-code,codex".into(),
        created_at: 0,
        updated_at: 0,
    };
    let set = row.enabled_set();
    assert_eq!(set.len(), 2);
    assert!(set.contains(&McpAgent::ClaudeCode));
    assert!(set.contains(&McpAgent::Codex));
}

#[test]
fn info_masks_sensitive_env() {
    let row = McpServerRow {
        id: 1,
        name: "foo".into(),
        transport: "stdio".into(),
        command: "npx".into(),
        args_json: "[]".into(),
        env_json: r#"{"API_KEY":"secret","DEBUG":"1"}"#.into(),
        url: "".into(),
        headers_json: "{}".into(),
        enabled_agents: "claude-code".into(),
        created_at: 0,
        updated_at: 0,
    };
    let info = McpServerInfo::from(row);
    assert_eq!(info.env.get("API_KEY").unwrap(), "***");
    assert_eq!(info.env.get("DEBUG").unwrap(), "1");
}
