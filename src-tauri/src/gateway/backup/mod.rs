//! 定时备份子系统。
//!
//! 复用 [`crate::gateway::import_export`] 的 collect + encrypt (AES-256-GCM `.aidogx`),
//! 把全量数据按用户设定间隔导出落盘到 `~/.aidog/backups/`, 超期自动清理。
//!
//! - 设置存 `setting` 表 (scope=`backup`, key=`settings`, value=JSON), 缺省/解析失败 → 默认值。
//! - 触发: setup 启动检查 (throttle) + 常驻 sleep loop (每轮读 settings 即时生效)。
//! - 重入防护: `AtomicBool`。
//! - 失败: 记 `last_backup_error` + 调 [`notification::dispatch`] (若用户开通知)。
//!
//! 子模块划分:
//! - 本模块: 共享类型 / 常量 ([`BackupSettings`] / [`BackupResult`])。
//! - [`cleanup`]: 路径 / 时间 helper + 超期清理。
//! - [`scheduler`]: throttle 检查 / 执行备份 / 常驻调度 loop / 失败通知。

mod cleanup;
mod scheduler;

pub use scheduler::{run_backup, spawn_scheduler};

use serde::{Deserialize, Serialize};

use crate::gateway::db::{self, Db};
use crate::gateway::import_export;
use crate::gateway::models::SetSettingInput;

/// 备份相关 setting 在 `setting` 表的 scope。
const SETTING_SCOPE: &str = "backup";
const SETTING_KEY: &str = "settings";

/// 当前默认值版本号。
///
/// - 新装用户走 [`BackupSettings::default`] 直接拿到此版本。
/// - 老数据 (无 `defaults_version` 字段) 反序列化为 0 → [`BackupSettings::load`] 一次性迁移:
///   version<CURRENT 且 `enabled` 仍是旧默认 false → 翻 true (视为「从未手动确认」)。
/// - 走过 [`crate::backup_settings_set`] (UI 保存入口) 即写为此版本, 标记「已手动确认」,
///   此后存储值永久尊重 (即使用户关 enabled 也不重开)。
pub const CURRENT_DEFAULTS_VERSION: u32 = 1;

/// 备份文件存放目录名 (相对 `~/.aidog/`)。
pub(crate) const BACKUP_DIR_NAME: &str = "backups";

/// 备份文件扩展名 (与手动导出一致, 复用同一加密容器)。
pub(crate) const BACKUP_EXT: &str = "aidogx";

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
    /// 默认值版本号: 0 = 老数据/从未手动确认; <[`CURRENT_DEFAULTS_VERSION`] = 待迁移。
    ///
    /// 用户经 UI 保存一次后写为 [`CURRENT_DEFAULTS_VERSION`] (标记「已手动确认」, 此后尊重存储值)。
    #[serde(default)]
    pub defaults_version: u32,
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
            enabled: true,
            interval_hours: default_interval_hours(),
            retention_days: default_retention_days(),
            last_backup_at: 0,
            last_backup_error: String::new(),
            defaults_version: CURRENT_DEFAULTS_VERSION,
        }
    }
}

impl BackupSettings {
    /// 从 db 读取 (缺省/解析失败 → 默认)。
    ///
    /// 内嵌一次性版本迁移: 若 `defaults_version` 老于 [`CURRENT_DEFAULTS_VERSION`]:
    ///   - 旧默认值 `enabled=false` (无 version 字段的老数据) → 翻 true (视为「从未手动确认」)。
    ///   - 迁移结果落库 (幂等: 第二次 load version 已=CURRENT, 不再触发)。
    ///   - `save` 失败不阻断 (返回内存迁移态, 下次启动再试)。
    pub async fn load(db: &Db) -> Self {
        let mut s = match db::get_setting(db, SETTING_SCOPE, SETTING_KEY).await {
            Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
            _ => Self::default(),
        };
        if s.defaults_version < CURRENT_DEFAULTS_VERSION {
            if !s.enabled {
                s.enabled = true;
            }
            s.defaults_version = CURRENT_DEFAULTS_VERSION;
            let _ = s.save(db).await;
        }
        s
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

#[cfg(test)]
mod test_mod;
