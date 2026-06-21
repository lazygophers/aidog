use super::*;
use std::collections::BTreeMap;

#[test]
fn parse_codex_entry_stdio() {
    let toml_str = r#"
command = "uvx"
args = ["duckduckgo-mcp-server"]

[env]
ALL_PROXY = "http://127.0.0.1:7890"
"#;
    let v: toml::Value = toml::from_str(toml_str).unwrap();
    let cfg = parse_codex_entry(&v).expect("parse");
    assert_eq!(cfg.transport, McpTransport::Stdio);
    assert_eq!(cfg.command, "uvx");
    assert_eq!(cfg.args, vec!["duckduckgo-mcp-server".to_string()]);
    assert_eq!(cfg.env.get("ALL_PROXY").unwrap(), "http://127.0.0.1:7890");
}

#[test]
fn build_codex_entry_has_env_subtable() {
    let cfg = McpConfigRaw {
        transport: McpTransport::Stdio,
        command: "uvx".into(),
        args: vec!["srv".into()],
        env: {
            let mut m = BTreeMap::new();
            m.insert("KEY".into(), "v".into());
            m
        },
        url: String::new(),
        headers: BTreeMap::new(),
    };
    let entry = build_codex_entry(&cfg);
    let back = parse_codex_entry(&entry).expect("roundtrip");
    assert_eq!(back.command, "uvx");
    assert_eq!(back.env.get("KEY").unwrap(), "v");
}
