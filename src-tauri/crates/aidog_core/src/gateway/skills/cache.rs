//! list 提速的 SWR 缓存（进程内 + 磁盘 `~/.aidog/skills-cache.json`）。
//!
//! 双层：进程内 `SKILLS_CACHE`（首访从磁盘 lazy load）+ 磁盘文件。
//! - `list_cached(scope)` → 立即返回缓存（命中即 0 子进程）；冷启动无缓存 → 空 + stale=true。
//! - `list_refresh(scope)` → 强制跑 npx、更新内存+磁盘、返回 fresh（stale=false）。
//! - 写操作后 `invalidate(scope)` 失效对应 scope（内存 + 磁盘），下次 refresh 重填。
//!   容错：磁盘损坏 / 缺失 → 当冷启动（空缓存）。原子写（temp + rename）防半文件。

use super::list::list_installed;
use super::types::{SkillInfo, SkillScope};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// list 缓存返回：数据 + 是否为陈旧/冷启动（true = 调用方应触发 refresh）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSkills {
    /// 缓存的 skill 列表（冷启动为空）。
    pub items: Vec<SkillInfo>,
    /// true = 无缓存命中（冷启动），调用方应显加载态 + 强制 refresh。
    pub stale: bool,
    /// true = `list_refresh` 中 npx 失败 / HOME 缺失，缓存未被更新（保留旧 items）。
    /// 前端应显「加载失败，显示上次列表」提示。`#[serde(default)]` 向后兼容旧前端。
    #[serde(default)]
    pub load_failed: bool,
}

/// 单 scope 的磁盘缓存条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ScopeCacheEntry {
    /// 写入时刻（毫秒 Unix 戳，仅诊断用，不做 TTL 过期）。
    pub(super) cached_at: i64,
    pub(super) items: Vec<SkillInfo>,
}

/// 磁盘缓存根结构（`~/.aidog/skills-cache.json`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct SkillsCacheFile {
    /// per-scope（key = `SkillScope::cache_key`）。
    #[serde(default)]
    pub(super) scopes: HashMap<String, ScopeCacheEntry>,
}

/// 进程内缓存（首访从磁盘 lazy load，之后内存为准 + 写时同步落盘）。
static SKILLS_CACHE: OnceLock<Mutex<SkillsCacheFile>> = OnceLock::new();

/// 磁盘缓存文件路径：`~/.aidog/skills-cache.json`。
/// home 不可解析 → None（降级为纯内存缓存，不落盘）。
fn cache_file_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join(".aidog");
    // best-effort 建目录；失败仍返回路径（写时再失败即降级）。
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("skills-cache.json"))
}

/// 从磁盘读缓存文件。缺失 / 损坏 / 解析失败 → 默认空（当冷启动）。
fn load_cache_from_disk() -> SkillsCacheFile {
    let Some(p) = cache_file_path() else {
        return SkillsCacheFile::default();
    };
    let Ok(text) = std::fs::read_to_string(&p) else {
        return SkillsCacheFile::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// 原子落盘：写临时文件 → rename 覆盖，防并发/中断产生半文件。
/// 任一步失败仅记日志（缓存以内存为准，落盘是优化非必需）。
fn persist_cache_to_disk(cache: &SkillsCacheFile) {
    let Some(p) = cache_file_path() else {
        return;
    };
    let Ok(json) = serde_json::to_string(cache) else {
        return;
    };
    // 同目录临时文件（确保 rename 在同一文件系统，原子生效）。
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, json.as_bytes()).is_err() {
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &p) {
        tracing::warn!(error = %e, "skills cache atomic write failed");
        let _ = std::fs::remove_file(&tmp);
    }
}

/// 取进程内缓存（首访从磁盘 load）。
fn cache_store() -> &'static Mutex<SkillsCacheFile> {
    SKILLS_CACHE.get_or_init(|| Mutex::new(load_cache_from_disk()))
}

/// 立即返回缓存（内存→磁盘，命中即 0 子进程）；无缓存返回空 + stale=true。
///
/// SWR 的 "stale" 半：调用方应立即渲染 `items`，再后台 `list_refresh`。
pub fn list_cached(scope: &SkillScope) -> CachedSkills {
    let key = scope.cache_key();
    let guard = match cache_store().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    match guard.scopes.get(&key) {
        Some(entry) => {
            // 锁文件成为主数据源（2026-06-26 重构）后 source 等 7 字段直接随 SkillInfo 序列化，
            // 旧版 enrich_with_sources 兜底已不需要（cache 直接 clone 透出）。
            // 旧缓存 items（source 为 None / 缺新字段）会在下次 list_refresh 时从锁文件回填。
            let items = entry.items.clone();
            CachedSkills {
                items,
                stale: false,
                load_failed: false,
            }
        }
        None => CachedSkills {
            items: Vec::new(),
            stale: true,
            load_failed: false,
        },
    }
}

/// 强制读锁文件取最新，更新内存+磁盘缓存，返回 fresh（stale=false）。
///
/// SWR 的 "revalidate" 半。锁文件读取失败 / HOME 缺失 / JSON 解析失败 / version 非预期
/// （`list_installed` 返 `ok=false`）→ **不覆盖已有缓存**，返回旧缓存 items + `stale=true` +
/// `load_failed=true`，让前端显「加载失败，显示上次列表」提示而非假空列表。缓存写空是历史
/// bug（缓存被失败的空 vec 覆盖 → UI 显示空，用户误以为 skills 被清理，实际物理文件仍在磁盘）。
pub fn list_refresh(scope: &SkillScope, proxy_url: Option<&str>) -> CachedSkills {
    let (items, ok) = list_installed(scope, proxy_url);
    if !ok {
        // 锁文件读取/解析失败：保留旧缓存（内存 → 磁盘），不写空覆盖。
        // 直接复用 list_cached 取旧 items + 同 stale 语义（有缓存→渲染、无→冷启动），
        // 仅叠加 load_failed=true 让前端显失败提示。
        let cached = list_cached(scope);
        tracing::warn!(
            scope = ?scope,
            old_count = cached.items.len(),
            "list_refresh 锁文件加载失败，保留旧缓存 + load_failed=true"
        );
        return CachedSkills {
            items: cached.items,
            stale: cached.stale,
            load_failed: true,
        };
    }
    write_cache(scope, items.clone());
    CachedSkills {
        items,
        stale: false,
        load_failed: false,
    }
}

/// 写入某 scope 缓存（内存 + 落盘）。
fn write_cache(scope: &SkillScope, items: Vec<SkillInfo>) {
    let key = scope.cache_key();
    let snapshot = {
        let mut guard = match cache_store().lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.scopes.insert(
            key,
            ScopeCacheEntry {
                cached_at: chrono::Utc::now().timestamp_millis(),
                items,
            },
        );
        guard.clone()
    };
    persist_cache_to_disk(&snapshot);
}

/// 失效某 scope 缓存（内存 + 落盘）。写操作成功后调用，下次 refresh 重填。
pub fn invalidate(scope: &SkillScope) {
    let key = scope.cache_key();
    let snapshot = {
        let mut guard = match cache_store().lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.scopes.remove(&key);
        guard.clone()
    };
    persist_cache_to_disk(&snapshot);
}

#[cfg(test)]
#[path = "test_cache.rs"]
mod test_cache;
