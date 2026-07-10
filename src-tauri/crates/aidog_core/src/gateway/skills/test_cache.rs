use super::*;
use crate::gateway::db::test_support::HomeGuard;
use crate::gateway::skills::types::{SkillInfo, SkillScope};
use std::fs;
use tempfile::TempDir;

/// 构造一个所有字段均 default 的 SkillInfo（含新增锁文件字段，测试用）。
fn make_skill(name: &str, scope: SkillScope) -> SkillInfo {
    SkillInfo {
        name: name.to_string(),
        enabled_agents: vec![],
        scope,
        installed_path: None,
        description: None,
        source: None,
        source_type: None,
        source_url: None,
        skill_folder_hash: None,
        plugin_name: None,
        installed_at: None,
        updated_at: None,
    }
}

#[test]
fn cache_file_roundtrip_serde() {
    // 缓存文件结构可序列化/反序列化往返。
    let mut file = SkillsCacheFile::default();
    let scope = SkillScope::Global;
    file.scopes.insert(
        "global".to_string(),
        ScopeCacheEntry {
            cached_at: 123,
            items: vec![make_skill("foo", scope.clone())],
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
    // HomeGuard：cache_file_path() 会 create_dir_all(~/.aidog) 并读 skills-cache.json，
    // 持 guard 重定向 HOME 到 tempdir，避免触碰真实 ~/.aidog。
    let _h = HomeGuard::new();
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
    // HomeGuard：invalidate → persist_cache_to_disk 会写 ~/.aidog/skills-cache.json，隔离到 tempdir。
    let _h = HomeGuard::new();
    let scope = SkillScope::Project {
        path: format!("/tmp/test_cache_invalidate_{}", std::process::id()),
    };
    // Should not panic even if no entry exists.
    invalidate(&scope);
}

/// list_cached / invalidate / list_cached roundtrip: write something, read it, invalidate, verify stale.
#[test]
fn write_read_invalidate_cycle() {
    // HomeGuard：write_cache / invalidate → persist_cache_to_disk 落 ~/.aidog/skills-cache.json，
    // 隔离到 tempdir，避免污染真实文件（实测会改其 mtime）。
    let _h = HomeGuard::new();
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
                items: vec![make_skill("test-skill", scope.clone())],
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

/// F1: list_refresh 锁文件加载失败时（project scope 下写一份损坏锁文件）保留旧缓存 + load_failed=true。
/// 验证写空覆盖 bug 已修：失败时缓存不被空 vec 覆盖，前端可显示「加载失败，显示上次列表」。
#[test]
fn list_refresh_lockfile_failure_preserves_old_cache() {
    // HomeGuard：list_refresh / invalidate 经 cache_store/persist 触 ~/.aidog 写盘，隔离到 tempdir。
    let _h = HomeGuard::new();
    // project path tempdir + 写一份损坏锁文件触发 ok=false。
    // 注意：project scope cache_key 含 path，每个测试用唯一 tempdir 避免与其他测试串扰。
    let tmp = TempDir::new().unwrap();
    let scope = SkillScope::Project {
        path: tmp.path().to_string_lossy().into_owned(),
    };
    let agents_dir = tmp.path().join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    // 损坏 JSON 触发 list_installed 返 ok=false。
    fs::write(agents_dir.join(".skill-lock.json"), "not json {{{").unwrap();

    // 1. 先写入一个旧缓存条目（模拟历史成功 list 的结果）。
    {
        let key = scope.cache_key();
        let mut guard = cache_store().lock().unwrap();
        guard.scopes.insert(
            key,
            ScopeCacheEntry {
                cached_at: 100,
                items: vec![make_skill("old-skill", scope.clone())],
            },
        );
    }

    // 2. list_refresh 触发锁文件失败 → 应回旧 items + stale=false（有缓存） + load_failed=true。
    let result = list_refresh(&scope, None);
    assert!(
        result.load_failed,
        "list_refresh 锁文件失败时应返 load_failed=true"
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

/// F1: list_refresh 锁文件失败 + 无旧缓存（首次进页即失败）→ 返空 items + stale=true + load_failed=true。
/// 前端应显加载态 + 失败提示（而非假空列表）。
#[test]
fn list_refresh_lockfile_failure_no_cache_returns_stale_load_failed() {
    // HomeGuard：list_refresh / invalidate 经 cache_store/persist 触 ~/.aidog 写盘，隔离到 tempdir。
    let _h = HomeGuard::new();
    let tmp = TempDir::new().unwrap();
    let scope = SkillScope::Project {
        path: tmp.path().to_string_lossy().into_owned(),
    };
    let agents_dir = tmp.path().join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join(".skill-lock.json"), "not json {{{").unwrap();

    // 确保无缓存。
    invalidate(&scope);

    let result = list_refresh(&scope, None);
    assert!(result.load_failed, "锁文件失败应 load_failed=true");
    assert!(result.items.is_empty(), "无旧缓存时 items 空");
    assert!(
        result.stale,
        "无旧缓存时 stale=true（前端应显加载态/失败提示而非空列表）"
    );
}
