//! list 提速的 SWR 缓存（进程内 + 磁盘 `~/.aidog/skills-cache.json`）。
//!
//! 双层：进程内 `SKILLS_CACHE`（首访从磁盘 lazy load）+ 磁盘文件。
//! - `list_cached(scope)` → 立即返回缓存（命中即 0 子进程）；冷启动无缓存 → 空 + stale=true。
//! - `list_refresh(scope)` → 强制跑 npx、更新内存+磁盘、返回 fresh（stale=false）。
//! - 写操作后 `invalidate(scope)` 失效对应 scope（内存 + 磁盘），下次 refresh 重填。
//!   容错：磁盘损坏 / 缺失 → 当冷启动（空缓存）。原子写（temp + rename）防半文件。

use super::list::{enrich_with_sources, list_installed};
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
            // 向后兼容：旧缓存 items 无 source 字段（source-grouping task 前写入）。
            // 命中缓存后 enrich_with_sources 读锁文件补 source（0 npx，cheap）。
            // 旧 None + 锁文件有 → 补；已有 source → 幂等重赋；第三方 symlink → 保持 None。
            let mut items = entry.items.clone();
            enrich_with_sources(&mut items, scope);
            CachedSkills { items, stale: false }
        }
        None => CachedSkills {
            items: Vec::new(),
            stale: true,
        },
    }
}

/// 强制跑 npx 取最新，更新内存+磁盘缓存，返回 fresh（stale=false）。
///
/// SWR 的 "revalidate" 半。npx 失败 → 返回空 fresh（不污染已有缓存？这里仍写空覆盖，
/// 与直跑 `list_installed` 失败语义一致：上游列表真为空 vs 命令失败不可区分，保持简单）。
pub fn list_refresh(scope: &SkillScope, proxy_url: Option<&str>) -> CachedSkills {
    let items = list_installed(scope, proxy_url);
    write_cache(scope, items.clone());
    CachedSkills { items, stale: false }
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
