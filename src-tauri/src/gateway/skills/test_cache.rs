use super::*;
use crate::gateway::skills::types::{SkillAgent, SkillInfo, SkillScope};

#[test]
fn cache_file_roundtrip_serde() {
    // 缓存文件结构可序列化/反序列化往返。
    let mut file = SkillsCacheFile::default();
    file.scopes.insert(
        "global".to_string(),
        ScopeCacheEntry {
            cached_at: 123,
            items: vec![SkillInfo {
                name: "foo".to_string(),
                enabled_agents: vec![SkillAgent::Claude],
                scope: SkillScope::Global,
                installed_path: Some("/p/foo".to_string()),
                description: None,
                source: None,
            }],
        },
    );
    let json = serde_json::to_string(&file).unwrap();
    let back: SkillsCacheFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back.scopes.len(), 1);
    let entry = back.scopes.get("global").unwrap();
    assert_eq!(entry.cached_at, 123);
    assert_eq!(entry.items[0].name, "foo");
}

#[test]
fn cache_file_corrupt_json_defaults_empty() {
    // 损坏 JSON → 默认空（当冷启动），不 panic。
    let back: SkillsCacheFile = serde_json::from_str("not json {{{").unwrap_or_default();
    assert!(back.scopes.is_empty());
}
