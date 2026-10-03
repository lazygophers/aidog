//! 内置推荐清单门禁：schema 可解析、11 项、七字段无损往返到 McpUpdatePayload。

use crate::recommended::{bundled_mcp_recommended, RecommendedManifest};
use crate::types::McpTransport;
use crate::McpUpdatePayload;

const LOCALES: [&str; 8] = [
    "zh-Hans", "en-US", "ar-SA", "fr-FR", "de-DE", "ru-RU", "ja-JP", "es-ES",
];
const CATEGORIES: [&str; 6] = ["browser", "docs", "code", "search", "service", "tool"];
const EXPECTED_NAMES: [&str; 11] = [
    "playwright",
    "chrome-devtools",
    "context7",
    "deepwiki",
    "memory",
    "sequential-thinking",
    "serena",
    "brave-search",
    "sentry",
    "supabase",
    "time",
];

fn manifest() -> RecommendedManifest {
    serde_json::from_str(bundled_mcp_recommended()).expect("bundled recommended.json parses")
}

#[test]
fn bundled_manifest_shape() {
    let m = manifest();
    assert_eq!(m.schema_version, 1);
    assert!(m.updated_at > 1_700_000_000);
    assert_eq!(m.entries.len(), 11);
    let names: Vec<_> = m.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, EXPECTED_NAMES);
}

#[test]
fn entries_metadata_valid() {
    for e in manifest().entries {
        assert!(
            CATEGORIES.contains(&e.category.as_str()),
            "{}: bad category {}",
            e.name,
            e.category
        );
        assert!(!e.display_name.is_empty(), "{}: empty display_name", e.name);
        assert!(!e.docs_url.is_empty(), "{}: empty docs_url", e.name);
        assert!(!e.description.is_empty(), "{}: empty description", e.name);
        for loc in LOCALES {
            let v = e
                .description
                .get(loc)
                .unwrap_or_else(|| panic!("{}: missing locale {loc}", e.name));
            assert!(!v.trim().is_empty(), "{}: blank {loc}", e.name);
        }
        // required_env_keys 必须已预填进 env（键预填、值留空占位）。
        for k in &e.required_env_keys {
            assert!(
                e.env.contains_key(k),
                "{}: required env key {k} missing from env",
                e.name
            );
        }
        // env 值一律空占位：清单不存敏感真值。
        for v in e.env.values() {
            assert!(v.is_empty(), "{}: env value must be empty placeholder", e.name);
        }
    }
}

#[test]
fn entries_transport_and_payload_roundtrip() {
    for e in manifest().entries {
        let t = McpTransport::parse(&e.transport);
        assert_eq!(t.as_str(), e.transport, "{}: transport not canonical", e.name);
        match t {
            McpTransport::Stdio => assert!(!e.command.is_empty(), "{}: stdio no command", e.name),
            McpTransport::Http | McpTransport::Sse => {
                assert!(!e.url.is_empty(), "{}: http/sse no url", e.name)
            }
        }

        // 七字段无损往返：entry 序列化值 → McpUpdatePayload（McpUpdatePayload 仅 Deserialize，
        // 「回程」直接逐字段与 entry 原值比对）。
        let val = serde_json::to_value(&e).unwrap();
        let payload: McpUpdatePayload = serde_json::from_value(val).unwrap();
        assert_eq!(payload.name, e.name, "{}", e.name);
        assert_eq!(payload.transport, e.transport, "{}", e.name);
        assert_eq!(payload.command, e.command, "{}", e.name);
        assert_eq!(payload.args, e.args, "{}", e.name);
        assert_eq!(payload.env, e.env, "{}", e.name);
        assert_eq!(payload.url, e.url, "{}", e.name);
        assert_eq!(payload.headers, e.headers, "{}", e.name);
    }
}

#[test]
fn manifest_serializes_spec_shape() {
    // 序列化形状锁 spec §1.1 的 snake_case key（远程源 = 仓库同路径文件，按此形状比对）。
    let val = serde_json::to_value(manifest()).unwrap();
    assert!(val.get("schema_version").is_some());
    assert!(val.get("updated_at").is_some());
    assert!(val["entries"][0].get("display_name").is_some());
    assert!(val["entries"][0].get("required_env_keys").is_some());
}
