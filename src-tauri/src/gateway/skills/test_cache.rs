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

/// list_cached: cold start (no entry in scope) → stale=true, empty items.
#[test]
fn list_cached_cold_start_returns_stale() {
    // Use a unique project scope to avoid cross-test contamination with the OnceLock global cache.
    let scope = SkillScope::Project {
        path: format!("/tmp/test_cache_cold_{}", std::process::id()),
    };
    let result = list_cached(&scope);
    // cold start → stale
    assert!(result.stale, "expected stale on cold start");
    assert!(result.items.is_empty(), "expected empty items on cold start");
}

/// invalidate: removing a scope that was never written → no panic.
#[test]
fn invalidate_nonexistent_scope_is_noop() {
    let scope = SkillScope::Project {
        path: format!("/tmp/test_cache_invalidate_{}", std::process::id()),
    };
    // Should not panic even if no entry exists.
    invalidate(&scope);
}

/// list_cached / invalidate / list_cached roundtrip: write something, read it, invalidate, verify stale.
#[test]
fn write_read_invalidate_cycle() {
    // Use a unique scope key per test run.
    let scope = SkillScope::Project {
        path: format!("/tmp/test_cache_cycle_{}", std::process::id()),
    };
    // Write a fake entry directly into cache store.
    {
        let key = scope.cache_key();
        let mut guard = cache_store().lock().unwrap();
        guard.scopes.insert(
            key,
            ScopeCacheEntry {
                cached_at: 42,
                items: vec![SkillInfo {
                    name: "test-skill".to_string(),
                    enabled_agents: vec![],
                    scope: scope.clone(),
                    installed_path: None,
                    description: None,
                    source: None,
                }],
            },
        );
    }
    // Now list_cached should hit (stale=false) with the item.
    let cached = list_cached(&scope);
    assert!(!cached.stale, "should hit after writing");
    assert_eq!(cached.items.len(), 1);
    assert_eq!(cached.items[0].name, "test-skill");

    // Invalidate → back to stale.
    invalidate(&scope);
    let after = list_cached(&scope);
    assert!(after.stale, "should be stale after invalidate");
    assert!(after.items.is_empty());
}
