//! 管理接口设置（ADR 0010）：开关 + 可选 Bearer 凭据。
//!
//! 管理接口（`POST /api/rpc/<命令>`、`GET /api/events`、内核 `--ui` 的 `/api/ui/`）挂在
//! **代理端口**上，桌面版与内核版一致；只接受本机连接（复用 `/api/*` 既有的非回环拒绝，
//! 与代理的 `bind_lan` 无关）。设置键仍叫 `kernel/settings`（持久化兼容）。
//!
//! - [`KernelSettings::api_enabled`]：统一启停，缺省开；关着时三类路径返回「未启用」而不落入转发。
//! - [`KernelSettings::auth_token`]：非空时三类路径都要求 `Authorization: Bearer <auth_token>`；
//!   空 = 不校验。反向代理回连本机时，同机其他进程同样够得着代理端口，凭据是这一层的防线。

use aidog_db::{self as db, Db};

/// 管理接口设置。
///
/// 老库里残留的 `port` / `bind_lan` 键按未知字段忽略，不需要迁移。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct KernelSettings {
    /// 管理接口开关。缺省 = 开。
    #[serde(default = "default_api_enabled")]
    pub api_enabled: bool,
    /// Bearer 凭据。空 = 不校验。
    #[serde(default)]
    pub auth_token: String,
}

fn default_api_enabled() -> bool {
    true
}

impl Default for KernelSettings {
    fn default() -> Self {
        Self {
            api_enabled: default_api_enabled(),
            auth_token: String::new(),
        }
    }
}

impl KernelSettings {
    /// 是否已配置鉴权凭据（纯空白串不算）。
    pub fn has_auth(&self) -> bool {
        !self.auth_token.trim().is_empty()
    }
}

/// 读设置。无记录 / 解析失败 → 默认值，不 panic。
pub async fn load_kernel_settings(db: &Db) -> KernelSettings {
    match db::get_setting(db, "kernel", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "parse kernel settings failed, using defaults");
            KernelSettings::default()
        }),
        _ => KernelSettings::default(),
    }
}

/// 写设置。
pub async fn save_kernel_settings(db: &Db, s: &KernelSettings) -> Result<(), String> {
    let value = serde_json::to_value(s).map_err(|e| format!("serialize kernel settings: {e}"))?;
    db::set_setting(
        db,
        crate::gateway::models::SetSettingInput {
            scope: "kernel".to_string(),
            key: "settings".to_string(),
            value,
        },
    )
    .await
}

crate::tauri_command! {
    /// 读管理接口设置。
    pub async fn kernel_settings_get() -> KernelSettings {
        load_kernel_settings(aidog_ctx::db()).await
    }
}

crate::tauri_command! {
    /// 写管理接口设置（开关 + 凭据）。
    pub async fn kernel_settings_set(settings: KernelSettings) -> Result<(), String> {
        save_kernel_settings(aidog_ctx::db(), &settings).await
    }
}

crate::tauri_command! {
    /// 最近的管理接口调用记录（只含元数据，保留 7 天）。
    pub async fn api_call_log_list(
        limit: Option<i64>) -> Result<Vec<aidog_db::models::ApiCallLog>, String> {
        aidog_db::list_api_call_logs(aidog_ctx::db(), limit.unwrap_or(200).clamp(1, 1000)).await
    }
}

#[cfg(test)]
#[path = "test_kernel_settings.rs"]
mod test_kernel_settings;
