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

#[test]
fn toml_table_to_map_string_values_only() {
    let mut m = toml::map::Map::new();
    m.insert("KEY".to_string(), toml::Value::String("val".to_string()));
    m.insert("NUM".to_string(), toml::Value::Integer(42)); // non-string → filtered
    let result = toml_table_to_map(Some(&m));
    assert_eq!(result.get("KEY").unwrap(), "val");
    assert!(!result.contains_key("NUM"), "non-string toml values should be filtered");
}

#[test]
fn toml_table_to_map_none_returns_empty() {
    let result = toml_table_to_map(None);
    assert!(result.is_empty());
}

/// build_codex_entry with empty env omits env subtable.
#[test]
fn build_codex_entry_no_env_omits_env_field() {
    let cfg = McpConfigRaw {
        transport: McpTransport::Stdio,
        command: "cmd".into(),
        args: vec!["a".into(), "b".into()],
        env: BTreeMap::new(), // empty env
        url: String::new(),
        headers: BTreeMap::new(),
    };
    let entry = build_codex_entry(&cfg);
    // No env key in output TOML
    assert!(entry.get("env").is_none(), "empty env should produce no env subtable");
    let back = parse_codex_entry(&entry).expect("roundtrip");
    assert_eq!(back.command, "cmd");
    assert_eq!(back.args, vec!["a", "b"]);
    assert!(back.env.is_empty());
}

/// parse_codex_entry with missing command falls back to empty string.
#[test]
fn parse_codex_entry_no_command() {
    let toml_str = r#"args = ["server"]"#;
    let v: toml::Value = toml::from_str(toml_str).unwrap();
    let cfg = parse_codex_entry(&v).expect("should parse even without command");
    assert_eq!(cfg.command, ""); // default empty
    assert_eq!(cfg.args, vec!["server"]);
}

/// parse_codex_entry returns None for non-table value.
#[test]
fn parse_codex_entry_non_table_returns_none() {
    let v = toml::Value::String("not a table".to_string());
    assert!(parse_codex_entry(&v).is_none());
}

/// read_codex_toml on non-existent file returns empty table.
#[test]
fn read_codex_toml_missing_file_returns_empty() {
    let path = std::path::PathBuf::from("/tmp/nonexistent_aidog_test_12345678.toml");
    let result = read_codex_toml(&path).expect("should return empty table for missing file");
    assert!(result.as_table().unwrap().is_empty());
}

/// read_codex_toml on empty file returns empty table.
#[test]
fn read_codex_toml_empty_file_returns_empty() {
    let path = std::env::temp_dir().join("aidog_test_empty_codex.toml");
    std::fs::write(&path, "").unwrap();
    let result = read_codex_toml(&path).expect("should return empty table for empty file");
    assert!(result.as_table().unwrap().is_empty());
    let _ = std::fs::remove_file(&path);
}

/// write_codex_toml + read_codex_toml roundtrip.
#[test]
fn write_and_read_codex_toml_roundtrip() {
    let path = std::env::temp_dir().join(format!(
        "aidog_test_codex_rt_{}.toml",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    let mut root = toml::map::Map::new();
    root.insert("key".to_string(), toml::Value::String("value".to_string()));
    write_codex_toml(&path, &toml::Value::Table(root)).expect("write should succeed");
    let read_back = read_codex_toml(&path).expect("read should succeed");
    assert_eq!(
        read_back.get("key").and_then(|v| v.as_str()),
        Some("value")
    );
    let _ = std::fs::remove_file(&path);
}
