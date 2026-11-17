//! 插件管理单元测试（随 plugins.rs 1:1）。全部落 tempdir，不碰真实 ~/.codex，
//! 不调用真实 codex CLI。
use super::*;

#[test]
fn parse_codex_list_matches_cli_contract() {
    // 本机 codex-cli 0.168.0 `codex plugin list --json` 实测输出（隔离 HOME），
    // 前置 stderr WARNING 行证明容错。
    let raw = concat!(
        "WARNING: proceeding, even though we could not create PATH aliases: Refusing to create helper binaries under temporary dir\n",
        r#"{
  "installed": [
    {
      "pluginId": "hello@testmkt",
      "name": "hello",
      "marketplaceName": "testmkt",
      "version": "0.1.0",
      "installed": true,
      "enabled": true,
      "source": {"source": "local", "path": "/tmp/mkt/hello"},
      "marketplaceSource": {"sourceType": "local", "source": "/tmp/mkt"},
      "installPolicy": "AVAILABLE",
      "authPolicy": "ON_INSTALL"
    }
  ],
  "available": []
}"#,
    );
    let list = parse_codex_list(raw).unwrap();
    assert_eq!(list.len(), 1, "got: {list:?}");
    let p = &list[0];
    assert_eq!(p.plugin_id, "hello@testmkt");
    assert_eq!(p.name, "hello");
    assert_eq!(p.marketplace_name, "testmkt");
    assert_eq!(p.version, "0.1.0");
    assert!(p.installed);
    assert!(p.enabled);

    let empty = parse_codex_list("{\n  \"installed\": [],\n  \"available\": []\n}").unwrap();
    assert!(empty.is_empty());
}

#[test]
fn parse_codex_marketplaces_matches_cli_contract() {
    let raw = concat!(
        "WARNING: proceeding\n",
        r#"{
  "marketplaces": [
    {
      "name": "testmkt",
      "root": "/tmp/mkt",
      "marketplaceSource": {"sourceType": "local", "source": "/tmp/mkt"}
    }
  ]
}"#,
    );
    let list = parse_codex_marketplaces(raw).unwrap();
    assert_eq!(list.len(), 1, "got: {list:?}");
    assert_eq!(list[0].name, "testmkt");
    assert_eq!(list[0].source, "/tmp/mkt");
    assert_eq!(list[0].root, "/tmp/mkt");
}
