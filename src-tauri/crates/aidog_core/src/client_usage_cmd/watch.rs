//! 票 03：会话目录实时监听与定时对齐（issue #65）。
//!
//! 监听范围 = 同意开关开启且有会话根的客户端（claude_code / codex；其余读取器票 06 落地后补根）。
//! 文件事件入池去抖 2 秒后按文件增量读取（复用票 02 的 `scan_one_file` 链）；关开关 →
//! `sync_watchers` 摘除监听。定时对齐（5 分钟）比对 file_size/mtime：变化 → 增量读取；
//! 文件删除 → 删明细/进度/空会话壳（`delete_client_file_state` 内含空壳清扫）。
//!
//! 测试注入面：`sync_watchers_with` / `process_due_paths` / `align_once` 均显式收
//! roots 或 db 参数，tempdir 驱动，不碰真实用户目录。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use aidog_db::Db;

/// 同一文件短时多次写只触发一次读取。
const DEBOUNCE_MS: i64 = 2000;
/// 定时对齐周期。
const ALIGN_INTERVAL_SECS: u64 = 300;

/// 去抖池：路径 → 最新事件时刻（同路径覆写为最新，到期批量处理一次）。
static PENDING: OnceLock<Mutex<HashMap<String, i64>>> = OnceLock::new();
/// 当前监听中的根目录。
static WATCHED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
/// 全局监听器（首个 sync 时懒创建）。
static WATCHER: OnceLock<Mutex<Option<notify::RecommendedWatcher>>> = OnceLock::new();
/// 测试可见的读取次数计数（process_due_paths 内每次 scan_one_file +1）。
#[cfg(test)]
static SCAN_COUNT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

fn pending_slot() -> &'static Mutex<HashMap<String, i64>> {
    PENDING.get_or_init(Default::default)
}

fn watched_slot() -> &'static Mutex<HashSet<String>> {
    WATCHED.get_or_init(Default::default)
}

fn watcher_slot() -> &'static Mutex<Option<notify::RecommendedWatcher>> {
    WATCHER.get_or_init(Default::default)
}

/// 测试计数读取。
#[cfg(test)]
pub(super) fn scan_count() -> i64 {
    SCAN_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// 客户端会话根目录（生产路径）。真值在 mod.rs::client_root（codex 根为票 04 补的）。
fn client_root(code: &str) -> Option<PathBuf> {
    super::client_root(code)
}

/// 事件入池：记录路径与最新事件时刻（同路径覆写 = 去抖）。
fn note_event(path: String, now_ms: i64) {
    pending_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(path, now_ms);
}

/// 测试注入：模拟一次文件事件入池（不经真实 FS 事件）。
#[cfg(test)]
pub(super) fn note_event_for_test(path: String, now_ms: i64) {
    note_event(path, now_ms);
}

/// notify 回调：事件路径入池（同步闭包，只碰内存）。
fn forward_event(res: Result<notify::Event, notify::Error>) {
    let Ok(ev) = res else { return };
    for p in ev.paths {
        note_event(p.to_string_lossy().into_owned(), aidog_db::now());
    }
}

/// 取出到期路径（`now_ms - 事件时刻 >= 去抖窗口`），从池中归集移除。
pub(super) fn take_due_paths(now_ms: i64) -> Vec<String> {
    let mut slot = pending_slot().lock().unwrap_or_else(|e| e.into_inner());
    let due: Vec<String> = slot
        .iter()
        .filter(|(_, t)| now_ms >= t.saturating_add(DEBOUNCE_MS))
        .map(|(p, _)| p.clone())
        .collect();
    for p in &due {
        slot.remove(p);
    }
    due
}

/// 同步监听面 = 同意开关开启且有会话根的客户端根目录（生产入口：启动与 consent_set 调用）。
pub async fn sync_watchers(db: &Db) {
    let consent = super::load_consent(db).await;
    let want: Vec<(String, PathBuf)> = super::SUPPORTED_CLIENTS
        .iter()
        .filter(|c| super::consent_allows(&consent, c))
        .filter_map(|c| client_root(c).map(|r| (c.to_string(), r)))
        .collect();
    sync_watchers_with(&want);
}

/// 当前监听中的根目录快照（测试断言用）。
#[cfg(test)]
pub(super) fn watched_snapshot() -> Vec<String> {
    watched_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

/// FSEvents/macOS 会报规范路径（/private/var/...），tempdir 根常是 /var/...；
/// 统一规范化后再监听与匹配，规范化失败（目录不存在）回落原路径。
fn canonical_root(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// 监听面 diff 应用（测试注入面：want 由调用方给定；进程生命周期内复用同一 watcher）。
pub(super) fn sync_watchers_with(want: &[(String, PathBuf)]) {
    let want_roots: Vec<(String, String)> = want
        .iter()
        .map(|(c, r)| (c.clone(), canonical_root(r).to_string_lossy().into_owned()))
        .collect();
    let mut slot = watcher_slot().lock().unwrap_or_else(|e| e.into_inner());
    let w = match slot.as_mut() {
        Some(w) => w,
        None => {
            let w = match notify::recommended_watcher(forward_event) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!(error = %e, "client usage watcher create failed");
                    return;
                }
            };
            slot.insert(w)
        }
    };
    let mut watched = watched_slot().lock().unwrap_or_else(|e| e.into_inner());
    // 摘除：不在 want 里的已监听根目录。
    let want_set: HashSet<&String> = want_roots.iter().map(|(_, r)| r).collect();
    for old in watched.iter().filter(|r| !want_set.contains(r)).cloned().collect::<Vec<_>>() {
        let p = Path::new(&old);
        if let Err(e) = notify::Watcher::unwatch(w, p) {
            tracing::warn!(error = %e, root = %old, "client usage unwatch failed");
        }
        watched.remove(&old);
    }
    // 新增：want 里有但还没监听的根目录（含新建子目录，递归模式覆盖两层结构）。
    let new_roots: Vec<&String> = want_roots
        .iter()
        .map(|(_, r)| r)
        .filter(|r| !watched.contains(*r))
        .collect();
    for root in new_roots {
        let p = Path::new(root);
        if let Err(e) = notify::Watcher::watch(w, p, notify::RecursiveMode::Recursive) {
            tracing::warn!(error = %e, root = %root, "client usage watch failed");
            continue;
        }
        watched.insert(root.clone());
    }
}

/// 归客户端：路径落在哪个启用客户端的根目录下。roots 为测试注入面（生产 = 启用客户端根）。
fn owner_client<'a>(path: &str, roots: &'a [(String, PathBuf)]) -> Option<&'a str> {
    roots
        .iter()
        .find(|(_, r)| {
            let path = Path::new(path);
            path.starts_with(r) || path.starts_with(canonical_root(r))
        })
        .map(|(c, _)| c.as_str())
}

/// 处理到期路径：存在 → 按当前进度增量扫描（截断重建在 `scan_one_file` 内）；
/// 不存在 → 删明细/进度/空会话壳。返回新增明细数。
pub(super) async fn process_due_paths(db: &Db, roots: &[(String, PathBuf)], now_ms: i64) -> i64 {
    let paths = take_due_paths(now_ms);
    let mut total = 0i64;
    for p in paths {
        if Path::new(&p).extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(client) = owner_client(&p, roots) else {
            continue;
        };
        if std::fs::metadata(&p).is_ok() {
            let progress = aidog_db::list_client_file_progress(db, client)
                .await
                .unwrap_or_default();
            let prog = progress.iter().find(|r| r.path == p).cloned();
            let path = PathBuf::from(&p);
            let sessions_root = roots
                .iter()
                .find(|(c, _)| *c == client)
                .map(|(_, r)| r.clone())
                .unwrap_or_default();
            match super::scan_one_file(db, client, &path, prog, &sessions_root).await {
                Ok((n, _)) => {
                    #[cfg(test)]
                    SCAN_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    total += n;
                }
                Err(e) => tracing::warn!(error = %e, path = %p, "client usage watch scan failed"),
            }
        } else {
            if let Err(e) = aidog_db::delete_client_file_state(db, client, &p).await {
                tracing::warn!(error = %e, path = %p, "client usage watch delete failed");
            }
        }
    }
    total
}

/// 启用客户端的根目录（生产去抖消费面）。
async fn enabled_roots(db: &Db) -> Vec<(String, PathBuf)> {
    let consent = super::load_consent(db).await;
    super::SUPPORTED_CLIENTS
        .iter()
        .filter(|c| super::consent_allows(&consent, c))
        .filter_map(|c| client_root(c).map(|r| (c.to_string(), r)))
        .collect()
}

/// 启动事件处理循环（桌面与内核启动路径各调一次；500ms 一拍，去抖到期即处理）。
pub fn start_event_loop() {
    tokio::spawn(async {
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            tick.tick().await;
            let roots = enabled_roots(aidog_ctx::db()).await;
            if roots.is_empty() {
                continue;
            }
            let now = aidog_db::now();
            let inserted = process_due_paths(aidog_ctx::db(), &roots, now).await;
            if inserted > 0 {
                tracing::debug!(inserted, "client usage watch import");
            }
        }
    });
}

/// 定时对齐循环（5 分钟一轮；首轮跳过——启动增量导入已覆盖同一时刻）。
pub async fn align_loop() {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(ALIGN_INTERVAL_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await; // 首轮跳过
    loop {
        interval.tick().await;
        let inserted = align_once(aidog_ctx::db()).await;
        if inserted > 0 {
            tracing::debug!(inserted, "client usage periodic align import");
        }
    }
}

/// 一轮定时对齐：对启用客户端的进度表逐行比对 file_size/mtime——变化 → 增量读取
/// （截断/替换的重建语义在 `scan_one_file` 内），文件删除 → 删明细/进度/空会话壳。
pub(super) async fn align_once(db: &Db) -> i64 {
    let consent = super::load_consent(db).await;
    let mut total = 0i64;
    for code in super::SUPPORTED_CLIENTS {
        if !super::consent_allows(&consent, code) {
            continue;
        }
        let progress = aidog_db::list_client_file_progress(db, code).await.unwrap_or_default();
        for row in progress {
            match std::fs::metadata(&row.path) {
                Err(_) => {
                    if let Err(e) = aidog_db::delete_client_file_state(db, code, &row.path).await {
                        tracing::warn!(error = %e, path = %row.path, "client usage align delete failed");
                    }
                }
                Ok(meta) => {
                    let mtime = meta_mtime_ms(&meta);
                    if (meta.len() as i64) == row.file_size && mtime == row.mtime_ms {
                        continue;
                    }
                    let path = row.path.clone();
                    let sessions_root = client_root(code).unwrap_or_default();
                    match super::scan_one_file(db, code, Path::new(&path), Some(row), &sessions_root).await {
                        Ok((n, _)) => total += n,
                        Err(e) => tracing::warn!(error = %e, path = %path, "client usage align scan failed"),
                    }
                }
            }
        }
    }
    total
}

fn meta_mtime_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
