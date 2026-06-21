//! 备份文件路径 / 时间 helper + 超期清理。

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{BACKUP_DIR_NAME, BACKUP_EXT};

// ─── 路径 ─────────────────────────────────────────────

/// `~/.aidog/backups/`, 不存在则创建。
pub(crate) fn backup_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "cannot resolve home directory".to_string())?;
    let dir = home.join(".aidog").join(BACKUP_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create backup dir: {e}"))?;
    Ok(dir)
}

/// 当前 UTC 时间 → 文件名片段 `YYYYMMDD-HHMMSS`。
pub(crate) fn timestamp_name_fragment() -> String {
    chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

pub(crate) fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ─── 清理 ─────────────────────────────────────────────

/// 删除 `~/.aidog/backups/` 下 mtime 早于 `now - retention_days*86400` 的 `.aidogx`。
///
/// 委托 [`cleanup_expired_in_dir`]; 扫不到目录 → 静默 (可能从未备份)。
pub async fn cleanup_expired(retention_days: i64) -> Result<u32, String> {
    let dir = match backup_dir() {
        Ok(d) => d,
        Err(_) => return Ok(0),
    };
    cleanup_expired_in_dir(&dir, retention_days).await
}

/// 核心: 对指定 dir 清理超期 `.aidogx` (按文件 mtime 秒精度比较)。
///
/// 拆出便于单测 (注入临时 dir)。返回删除数。
pub(crate) async fn cleanup_expired_in_dir(dir: &std::path::Path, retention_days: i64) -> Result<u32, String> {
    let cutoff_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        - retention_days * 86400;
    let mut removed = 0u32;
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(0),
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some(BACKUP_EXT) {
            continue;
        }
        let mtime_secs = match p.metadata().and_then(|m| m.modified()) {
            Ok(t) => t
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(i64::MAX),
            Err(_) => continue,
        };
        if mtime_secs < cutoff_secs {
            if let Err(e) = std::fs::remove_file(&p) {
                tracing::warn!(path = %p.display(), error = %e, "backup: cleanup remove failed");
            } else {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        tracing::info!(removed, "backup: cleanup expired");
    }
    Ok(removed)
}

#[cfg(test)]
#[path = "test_cleanup.rs"]
mod test_cleanup;
