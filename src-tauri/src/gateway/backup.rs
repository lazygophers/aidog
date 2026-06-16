//! 定时备份子系统。
//!
//! 复用 [`crate::gateway::import_export`] 的 collect + encrypt (AES-256-GCM `.aidogx`),
//! 把全量数据按用户设定间隔导出落盘到 `~/.aidog/backups/`, 超期自动清理。
//!
//! - 设置存 `setting` 表 (scope=`backup`, key=`settings`, value=JSON), 缺省/解析失败 → 默认值。
//! - 触发: setup 启动检查 (throttle) + 常驻 sleep loop (每轮读 settings 即时生效)。
//! - 重入防护: `AtomicBool`。
//! - 失败: 记 `last_backup_error` + 调 [`notification::dispatch`] (若用户开通知)。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::gateway::db::{self, Db};
use crate::gateway::import_export;
use crate::gateway::models::SetSettingInput;

/// 备份相关 setting 在 `setting` 表的 scope。
const SETTING_SCOPE: &str = "backup";
const SETTING_KEY: &str = "settings";

/// 备份文件存放目录名 (相对 `~/.aidog/`)。
const BACKUP_DIR_NAME: &str = "backups";

/// 备份文件扩展名 (与手动导出一致, 复用同一加密容器)。
const BACKUP_EXT: &str = "aidogx";

/// 全量导出的 scope 列表 (等价手动「导出全部」)。
pub const ALL_SCOPES: &[&str] = &[
    import_export::SCOPE_PLATFORM,
    import_export::SCOPE_GROUP,
    import_export::SCOPE_GROUP_PLATFORM,
    import_export::SCOPE_SETTING,
    import_export::SCOPE_CODEX,
    import_export::SCOPE_CLAUDE_CODE,
    import_export::SCOPE_SKILLS,
];

/// 重入防护: 防启动检查与定时器唤醒同帧并发跑两次 backup。
static BACKUP_RUNNING: AtomicBool = AtomicBool::new(false);

/// 定时备份设置 (前后端共享 schema)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSettings {
    /// 总开关。
    #[serde(default)]
    pub enabled: bool,
    /// 间隔 (小时), ≥1。
    #[serde(default = "default_interval_hours")]
    pub interval_hours: i64,
    /// 保留天数, 1..=90。
    #[serde(default = "default_retention_days")]
    pub retention_days: i64,
    /// 上次成功备份 epoch 毫秒 (0=从未), 由后端写。
    #[serde(default)]
    pub last_backup_at: i64,
    /// 上次错误信息 (空=成功), 由后端写。
    #[serde(default)]
    pub last_backup_error: String,
}

fn default_interval_hours() -> i64 {
    24
}
fn default_retention_days() -> i64 {
    7
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_hours: default_interval_hours(),
            retention_days: default_retention_days(),
            last_backup_at: 0,
            last_backup_error: String::new(),
        }
    }
}

impl BackupSettings {
    /// 从 db 读取 (缺省/解析失败 → 默认)。
    pub async fn load(db: &Db) -> Self {
        match db::get_setting(db, SETTING_SCOPE, SETTING_KEY).await {
            Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
            _ => Self::default(),
        }
    }

    /// 写入 db (全字段 upsert)。
    pub async fn save(&self, db: &Db) -> Result<(), String> {
        let value = serde_json::to_value(self).map_err(|e| format!("serialize backup settings: {e}"))?;
        db::set_setting(
            db,
            SetSettingInput {
                scope: SETTING_SCOPE.to_string(),
                key: SETTING_KEY.to_string(),
                value,
            },
        )
        .await
    }

    /// 规范化: 钳制到合法区间, 防前端误传。
    pub fn sanitized(mut self) -> Self {
        if self.interval_hours < 1 {
            self.interval_hours = default_interval_hours();
        }
        if !(1..=90).contains(&self.retention_days) {
            self.retention_days = default_retention_days();
        }
        self
    }
}

/// 备份结果 (立即触发 command 返回前端)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub ok: bool,
    pub path: Option<String>,
    pub error: Option<String>,
    pub timestamp: i64,
}

// ─── 路径 ─────────────────────────────────────────────

/// `~/.aidog/backups/`, 不存在则创建。
fn backup_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "cannot resolve home directory".to_string())?;
    let dir = home.join(".aidog").join(BACKUP_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create backup dir: {e}"))?;
    Ok(dir)
}

/// 当前 UTC 时间 → 文件名片段 `YYYYMMDD-HHMMSS`。
fn timestamp_name_fragment() -> String {
    chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ─── 核心 ─────────────────────────────────────────────

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
pub async fn cleanup_expired_in_dir(dir: &std::path::Path, retention_days: i64) -> Result<u32, String> {
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

/// 启动常驻调度 loop: 每轮读 settings (即时生效), 到点 → maybe_backup。
///
/// tick = min(interval, 60s), 平衡响应性与唤醒开销。app 生命周期内常驻。
pub fn spawn_scheduler(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        // 启动首次检查: 补「关机错过」场景。
        {
            let db = app.state::<Db>();
            if let Err(e) = maybe_backup(&db).await {
                tracing::warn!(error = %e, "backup: startup maybe_backup failed");
            }
            // 启动也清理一次 (防长期未开 backup 后首次启用堆积)。
            let s = BackupSettings::load(&db).await;
            let _ = cleanup_expired(s.retention_days).await;
        }
        loop {
            let db = app.state::<Db>();
            let s = BackupSettings::load(&db).await.sanitized();
            if let Err(e) = maybe_backup(&db).await {
                tracing::warn!(error = %e, "backup: scheduler maybe_backup failed");
            }
            // 下一轮 tick: 不超过 60s, 不超过 interval。
            let tick_secs = (s.interval_hours * 3600).clamp(1, 60) as u64;
            tokio::time::sleep(Duration::from_secs(tick_secs)).await;
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
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip() {
        let s = BackupSettings {
            enabled: true,
            interval_hours: 12,
            retention_days: 14,
            last_backup_at: 1_700_000_000_000,
            last_backup_error: String::new(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: BackupSettings = serde_json::from_str(&json).unwrap();
        assert!(back.enabled);
        assert_eq!(back.interval_hours, 12);
        assert_eq!(back.retention_days, 14);
        assert_eq!(back.last_backup_at, 1_700_000_000_000);
    }

    #[test]
    fn settings_default_when_missing_fields() {
        // 缺字段 → serde 默认填充。
        let json = r#"{"enabled":true}"#;
        let s: BackupSettings = serde_json::from_str(json).unwrap();
        assert!(s.enabled);
        assert_eq!(s.interval_hours, 24); // default
        assert_eq!(s.retention_days, 7); // default
    }

    #[test]
    fn sanitized_clamps_invalid_values() {
        let s = BackupSettings {
            enabled: true,
            interval_hours: 0,    // 非法 → 默认 24
            retention_days: 999,  // 非法 → 默认 7
            last_backup_at: 0,
            last_backup_error: String::new(),
        };
        let s = s.sanitized();
        assert_eq!(s.interval_hours, 24);
        assert_eq!(s.retention_days, 7);
    }

    #[tokio::test]
    async fn cleanup_removes_expired_files() {
        // 唯一临时 dir (无 tempfile 依赖)。
        let dir = std::env::temp_dir().join(format!(
            "aidog-backup-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // 旧文件 (10 天前 mtime) → 应删。
        let old_path = dir.join("aidog-backup-old.aidogx");
        std::fs::write(&old_path, b"x").unwrap();
        set_mtime_days_ago(&old_path, 10);

        // 新文件 (现在) → 保留。
        let new_path = dir.join("aidog-backup-new.aidogx");
        std::fs::write(&new_path, b"y").unwrap();

        // 非 .aidogx → 不动。
        let other = dir.join("notes.txt");
        std::fs::write(&other, b"z").unwrap();

        let removed = cleanup_expired_in_dir(&dir, 7).await.unwrap();
        assert_eq!(removed, 1);
        assert!(!old_path.exists());
        assert!(new_path.exists());
        assert!(other.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 把文件 mtime 设为 `days_ago` 天前 (std FileTimes, Rust 1.75+)。
    fn set_mtime_days_ago(path: &std::path::Path, days_ago: i64) {
        use std::fs::FileTimes;
        let past = SystemTime::now() - Duration::from_secs((days_ago * 86400) as u64);
        let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        let times = FileTimes::new().set_modified(past).set_accessed(past);
        f.set_times(times).unwrap();
    }

    #[tokio::test]
    async fn maybe_backup_skips_when_disabled() {
        let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
        db.init_tables().await.unwrap();
        // 默认 enabled=false → 必跳过 (返回 None), 不会真跑导出。
        let r = maybe_backup(&db).await;
        assert!(r.is_ok(), "maybe_backup disabled should not error: {:?}", r.err());
        assert!(r.unwrap().is_none());
    }

    #[tokio::test]
    async fn maybe_backup_throttles_within_interval() {
        let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
        db.init_tables().await.unwrap();
        // enabled=true + last_backup_at=now → 距上次 < interval → 跳过。
        let s = BackupSettings {
            enabled: true,
            interval_hours: 24,
            retention_days: 7,
            last_backup_at: now_millis(),
            last_backup_error: String::new(),
        };
        s.save(&db).await.unwrap();
        let r = maybe_backup(&db).await.unwrap();
        assert!(r.is_none(), "within-interval should be throttled");
    }

    #[tokio::test]
    async fn backup_settings_load_save_roundtrip() {
        let db = crate::gateway::db::Db::new(":memory:").await.unwrap();
        db.init_tables().await.unwrap();
        // 默认 → save → load 一致 (sanitized 后)。
        let s = BackupSettings {
            enabled: true,
            interval_hours: 6,
            retention_days: 30,
            last_backup_at: 0,
            last_backup_error: String::new(),
        };
        s.save(&db).await.unwrap();
        let loaded = BackupSettings::load(&db).await;
        assert!(loaded.enabled);
        assert_eq!(loaded.interval_hours, 6);
        assert_eq!(loaded.retention_days, 30);
    }
}
