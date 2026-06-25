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

/// F1: list_refresh npx 失败时（如 project path 不存在 → npx cwd 失败）保留旧缓存 + load_failed=true。
/// 验证写空覆盖 bug 已修：失败时缓存不被空 vec 覆盖，前端可显示「加载失败，显示上次列表」。
#[test]
fn list_refresh_npx_failure_preserves_old_cache() {
    // 用不存在的 project path 触发 npx 失败（cwd 不存在 → run_npx_in_scope 返 success=false）。
    // 注意：project scope cache_key 含 path，每个测试用唯一 path 避免与其他测试串扰。
    let scope = SkillScope::Project {
        path: format!("/nonexistent/test_cache_fail_{}", std::process::id()),
    };

    // 1. 先写入一个旧缓存条目（模拟历史成功 list 的结果）。
    {
        let key = scope.cache_key();
        let mut guard = cache_store().lock().unwrap();
        guard.scopes.insert(
            key,
            ScopeCacheEntry {
                cached_at: 100,
                items: vec![SkillInfo {
                    name: "old-skill".to_string(),
                    enabled_agents: vec![SkillAgent::Claude],
                    scope: scope.clone(),
                    installed_path: Some("/p/old".to_string()),
                    description: None,
                    source: None,
                }],
            },
        );
    }

    // 2. list_refresh 触发 npx 失败 → 应回旧 items + stale=false（有缓存） + load_failed=true。
    let result = list_refresh(&scope, None);
    assert!(
        result.load_failed,
        "list_refresh npx 失败时应返 load_failed=true"
    );
    assert_eq!(
        result.items.len(),
        1,
        "应保留旧缓存 items（不被空 vec 覆盖）"
    );
    assert_eq!(result.items[0].name, "old-skill");
    assert!(
        !result.stale,
        "有旧缓存时 stale=false（前端可直接渲染 items）"
    );

    // 3. 验证缓存本身未被空 vec 覆盖（内存中仍是 old-skill）。
    let cached = list_cached(&scope);
    assert_eq!(cached.items.len(), 1);
    assert_eq!(cached.items[0].name, "old-skill");

    // 清理本测试写入的缓存。
    invalidate(&scope);
}

/// F1: list_refresh npx 失败 + 无旧缓存（首次进页即失败）→ 返空 items + stale=true + load_failed=true。
/// 前端应显加载态 + 失败提示（而非假空列表）。
#[test]
fn list_refresh_npx_failure_no_cache_returns_stale_load_failed() {
    let scope = SkillScope::Project {
        path: format!("/nonexistent/test_cache_fail_nocache_{}", std::process::id()),
    };

    // 确保无缓存。
    invalidate(&scope);

    let result = list_refresh(&scope, None);
    assert!(result.load_failed, "npx 失败应 load_failed=true");
    assert!(result.items.is_empty(), "无旧缓存时 items 空");
    assert!(
        result.stale,
        "无旧缓存时 stale=true（前端应显加载态/失败提示而非空列表）"
    );
}
