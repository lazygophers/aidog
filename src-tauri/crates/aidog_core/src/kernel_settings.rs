//! 内核管理面设置（票 08）：端口 + Bearer 凭据。
//!
//! # 管理面**永远只绑 127.0.0.1**
//!
//! `aidog-kernel --ui` 的管理面没有「开放到局域网」的开关，也不可配置监听地址：它一律
//! 监听 `127.0.0.1`。要从别的设备访问界面，请自行在前面架一层反向代理（nginx / caddy），
//! 由反向代理负责 TLS 与鉴权，回连本机的 `127.0.0.1:<port>`。
//!
//! 理由（2026-09-03 审查裁决）：管理面 `/rpc/<命令>` 覆盖全部命令，含改配置、读全部请求
//! 日志（含请求/响应正文）、执行脚本，比代理的转发端口危险一个数量级；而它的鉴权只有一个
//! 静态 Bearer，还带不进浏览器的文档导航与 `EventSource`。把「开放到公网/局域网」这件事
//! 整个交给成熟的反向代理，比在这里自造一套半吊子的开放路径更安全。
//!
//! # 与代理 `bind_lan` 的关系：没有关系
//!
//! 代理端口的 `ProxySettings::bind_lan`（`crate::shared`）是**另一个维度**的开关，管的是
//! **转发端口**：局域网设备能把请求经 aidog 转给上游，防线是每个分组各自的 `group_key`
//! Bearer。它与本模块无关，**禁在任何一侧读另一侧。**
//!
//! # 凭据
//!
//! [`KernelSettings::auth_token`] 非空时，管理面所有请求都要求
//! `Authorization: Bearer <auth_token>`。绑在 127.0.0.1 也仍然有意义：反向代理回连本机时，
//! 同机的其他进程同样够得着这个端口，凭据是这一层的防线。为空 = 不校验。

use aidog_db::{self as db, Db};

/// 内核管理面（`--ui` 形态）的监听设置。
///
/// **没有绑定地址字段**：管理面永远 127.0.0.1（见模块文档）。老库里若残留 `bind_lan`
/// 键，serde 反序列化时按未知字段忽略，不需要迁移。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct KernelSettings {
    /// 管理面监听端口。默认 9891（代理默认 9890 的下一个，避免撞车）。
    #[serde(default = "default_kernel_port")]
    pub port: u16,
    /// Bearer 凭据。空 = 不校验。
    #[serde(default)]
    pub auth_token: String,
}

fn default_kernel_port() -> u16 {
    9891
}

impl Default for KernelSettings {
    fn default() -> Self {
        Self {
            port: default_kernel_port(),
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
    /// 读内核管理面设置。
    pub async fn kernel_settings_get() -> KernelSettings {
        load_kernel_settings(aidog_ctx::db()).await
    }
}

crate::tauri_command! {
    /// 写内核管理面设置（端口 + 凭据）。
    pub async fn kernel_settings_set(settings: KernelSettings) -> Result<(), String> {
        save_kernel_settings(aidog_ctx::db(), &settings).await
    }
}

#[cfg(test)]
#[path = "test_kernel_settings.rs"]
mod test_kernel_settings;
