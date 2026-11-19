//! throttle 检查 / 执行备份 / 常驻调度 loop / 失败通知。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::cleanup::{backup_dir, cleanup_expired, now_millis, timestamp_name_fragment};
use super::{ALL_SCOPES, BACKUP_EXT, BackupSettings};
use aidog_core::gateway::import_export;
use aidog_db::Db;

/// 重入防护: 防启动检查与定时器唤醒同帧并发跑两次 backup。
static BACKUP_RUNNING: AtomicBool = AtomicBool::new(false);

/// 备份调度器唤醒信号（单消费者：scheduler_loop）。
/// `notify_one` 语义：设置保存时若循环在跑备份（无 waiter），permit 存住，下次立即返回。
fn backup_wake() -> &'static tokio::sync::Notify {
    static WAKE: std::sync::OnceLock<tokio::sync::Notify> = std::sync::OnceLock::new();
    WAKE.get_or_init(tokio::sync::Notify::new)
}

/// 唤醒备份调度器：立刻重读 [`BackupSettings`] 重算下次到期（设置保存入口调用）。
pub fn wake_backup_scheduler() {
    backup_wake().notify_one();
}

async fn wait_next_backup(delay: Duration) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        _ = backup_wake().notified() => true,
    }
}

/// throttle 检查: 未启用 / 距上次 < interval → 跳过。
///
/// 供 setup 启动检查 + scheduler loop 共用。
pub async fn maybe_backup(db: &Db) -> Result<Option<PathBuf>, String> {
    let s = BackupSettings::load(db).await.sanitized();
    if !s.enabled {
        return Ok(None);
    }
    let interval_millis = s.interval_hours * 3600 * 1000;
    if s.last_backup_at > 0 && now_millis() - s.last_backup_at < interval_millis {
        return Ok(None);
    }
    run_backup(db).await.map(Some)
}

/// 执行一次备份: collect → encrypt → 落盘 → 更新 last_backup_at → cleanup。
///
/// 重入防护: 已在跑 → 返回 Err。失败: 记 last_backup_error + 通知。
pub async fn run_backup(db: &Db) -> Result<PathBuf, String> {
    if BACKUP_RUNNING.swap(true, Ordering::SeqCst) {
        return Err("backup already running".into());
    }
    let result = run_backup_inner(db).await;
    BACKUP_RUNNING.store(false, Ordering::SeqCst);

    let mut settings = BackupSettings::load(db).await;
    match result {
        Ok(path) => {
            settings.last_backup_at = now_millis();
            settings.last_backup_error.clear();
            let _ = settings.save(db).await;
            // 备份成功后顺带清理超期文件。
            let _ = cleanup_expired(settings.retention_days).await;
            tracing::info!(path = %path.display(), "backup: ok");
            Ok(path)
        }
        Err(e) => {
            settings.last_backup_error = e.clone();
            let _ = settings.save(db).await;
            tracing::error!(error = %e, "backup: failed");
            // 通知用户 (失败, 不阻塞)。
            notify_failure(db, &e).await;
            Err(e)
        }
    }
}

async fn run_backup_inner(db: &Db) -> Result<PathBuf, String> {
    let scopes: Vec<String> = ALL_SCOPES.iter().map(|s| s.to_string()).collect();
    let mut payload = import_export::collect::collect(db, &scopes).await?;
    let bytes = payload.serialize_with_checksum()?;
    let encrypted = import_export::encrypt(&bytes)?;

    let dir = backup_dir()?;
    let filename = format!("aidog-backup-{}.{}", timestamp_name_fragment(), BACKUP_EXT);
    let path = dir.join(filename);
    std::fs::write(&path, &encrypted).map_err(|e| format!("write backup file: {e}"))?;
    Ok(path)
}

/// 睡到下次备份到点的等待时长（取代旧 60 秒轮询，见 scheduler_loop 注释）。
/// - 关闭 → 0（调度循环不启动定时器，只等设置变更唤醒）；
/// - 开启 + 从未备份 → 到期（0）；
/// - 开启 + 距上次 < interval → 睡到 `last_backup_at + interval`；
/// - 已到期 → 0。`first_loop` 时 0 值换成 60 秒启动延迟，保持「启动不立即跑」。
pub fn next_wake_delay(s: &BackupSettings, now_ms: i64, first_loop: bool) -> Duration {
    if !s.enabled {
        return Duration::ZERO;
    }
    let delay = if s.last_backup_at <= 0 {
        Duration::ZERO
    } else {
        let due_at = s
            .last_backup_at
            .saturating_add(s.interval_hours.saturating_mul(3600 * 1000));
        Duration::from_millis(due_at.saturating_sub(now_ms).max(0) as u64)
    };
    if first_loop && delay.is_zero() {
        Duration::from_secs(60)
    } else {
        delay
    }
}

/// 常驻调度 loop: 睡到下次备份到期，或被设置变更唤醒后重算。
/// 关闭时长睡，仅 `backup_settings_set` 的 notify 唤醒，不周期读 DB。
/// 备份失败后等 60 秒（可被设置变更打断）再进下轮；失败不改 `last_backup_at`，
/// 下轮仍判定到期 → 实际重试间隔 ≈ 60 秒，与旧 60 秒轮询同节奏。
///
/// 票 08：由 `pub fn spawn_scheduler()`（内部 `tauri::async_runtime::spawn`）改成裸 async
/// 函数，spawn 交给外壳做 —— 本 crate 因此完全不链 tauri，无界面内核可原样 `tokio::spawn`
/// 同一个 loop。桌面壳照旧 `tauri::async_runtime::spawn(scheduler_loop())`，行为不变。
/// （不能在这里 `tokio::spawn`：桌面壳的调用点在 `setup()` 主线程上，不在 runtime 上下文内。）
pub async fn scheduler_loop() {
    let mut first_loop = true;
    loop {
        let db = aidog_ctx::db();
        let s = BackupSettings::load(db).await.sanitized();
        if !s.enabled {
            first_loop = false;
            backup_wake().notified().await;
            continue;
        }
        let delay = next_wake_delay(&s, now_millis(), first_loop);
        first_loop = false;
        if !wait_next_backup(delay).await
            && let Err(e) = maybe_backup(db).await
        {
            tracing::warn!(error = %e, "backup: scheduler maybe_backup failed");
            let _ = wait_next_backup(Duration::from_secs(60)).await;
        }
    }
}

/// 备份失败通知 (复用 notification::dispatch; 用户关通知则静默)。
async fn notify_failure(db: &Db, error: &str) {
    let vars = std::collections::HashMap::new();
    let db_arc = std::sync::Arc::new(db.clone());
    let _ = aidog_notification::dispatch(&db_arc, None, None, "error", Some(error), &vars).await;
}

#[cfg(test)]
#[path = "test_scheduler.rs"]
mod test_scheduler;
