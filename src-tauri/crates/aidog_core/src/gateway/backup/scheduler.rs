//! throttle 检查 / 执行备份 / 常驻调度 loop / 失败通知。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::Manager;

use super::cleanup::{backup_dir, cleanup_expired, now_millis, timestamp_name_fragment};
use super::{BackupSettings, ALL_SCOPES, BACKUP_EXT};
use crate::gateway::db::Db;
use crate::gateway::import_export;

/// 重入防护: 防启动检查与定时器唤醒同帧并发跑两次 backup。
static BACKUP_RUNNING: AtomicBool = AtomicBool::new(false);

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

/// 启动常驻调度 loop: 每轮读 settings (即时生效), 到点 → maybe_backup。
///
/// tick = min(interval, 60s), 平衡响应性与唤醒开销。app 生命周期内常驻。
pub fn spawn_scheduler(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        // 启动不立即跑（用户要求「启动不做定时操作」）；先 sleep 一个 tick 再进入循环，
        // 周期触发照旧。关机错过的补偿交由周期内 maybe_backup 的到点判定自然吸收。
        loop {
            let db = app.state::<Db>();
            let s = BackupSettings::load(&db).await.sanitized();
            // 下一轮 tick: 不超过 60s, 不超过 interval。
            let tick_secs = (s.interval_hours * 3600).clamp(1, 60) as u64;
            tokio::time::sleep(Duration::from_secs(tick_secs)).await;
            if let Err(e) = maybe_backup(&db).await {
                tracing::warn!(error = %e, "backup: scheduler maybe_backup failed");
            }
        }
    });
}

/// 备份失败通知 (复用 notification::dispatch; 用户关通知则静默)。
async fn notify_failure(db: &Db, error: &str) {
    let vars = std::collections::HashMap::new();
    let db_arc = std::sync::Arc::new(db.clone());
    let _ = crate::gateway::notification::dispatch(
        &db_arc,
        None,
        None,
        "error",
        Some(error),
        &vars,
    )
    .await;
}

#[cfg(test)]
#[path = "test_scheduler.rs"]
mod test_scheduler;
