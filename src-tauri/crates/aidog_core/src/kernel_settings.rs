//! 内核管理面设置（票 08）：端口 + **只管内核端口的绑定开关** + Bearer 凭据。
//!
//! # 与代理 `bind_lan` 的关系：没有关系
//!
//! 代理端口的 `ProxySettings::bind_lan`（`crate::shared`）与本模块的
//! [`KernelSettings::bind_lan`] 是**两个互不读取的开关**，存在 DB 的两把不同 key 下
//! （`proxy/settings` vs `kernel/settings`）。理由：两者开放的东西完全不同 ——
//!
//! - 代理的 `bind_lan` 开放**转发端口**：局域网设备能把请求经 aidog 转给上游，
//!   防线是每个分组各自的 `group_key` Bearer。
//! - 内核的 `bind_lan` 开放**管理接口**：`/rpc/<命令>` 覆盖全部 207 个命令，含改配置、
//!   读全部请求日志（含请求/响应正文）、执行脚本。它比转发端口危险一个数量级。
//!
//! 所以「我要局域网转发」不得顺带把管理面也开出去，反之亦然。**禁在任何一侧读另一侧。**
//!
//! # 开启的硬前提：先配凭据
//!
//! [`kernel_set_bind_lan`]`(true)` 在 [`KernelSettings::auth_token`] 为空时**拒绝**
//! （返回 `Err`，不落库）。凭据形态沿用 `/api/*` 的 Bearer 语义：请求带
//! `Authorization: Bearer <auth_token>`。127.0.0.1 形态下凭据为空时不强制鉴权
//! （本机单用户，等价于桌面版的 IPC）；一旦配了凭据就一律校验，与绑定地址无关。

use aidog_db::{self as db, Db};

/// 内核管理面（`--ui` 形态）的监听设置。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct KernelSettings {
    /// 管理面监听端口。默认 9891（代理默认 9890 的下一个，避免撞车）。
    #[serde(default = "default_kernel_port")]
    pub port: u16,
    /// 管理面绑定地址：true=0.0.0.0（局域网可访问）/ false=127.0.0.1（仅本机）。
    /// **默认关**。与 `ProxySettings::bind_lan` 无任何关系（见模块文档）。
    #[serde(default)]
    pub bind_lan: bool,
    /// Bearer 凭据。空 = 未配置 → 此时 `bind_lan` 不允许开。
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
            bind_lan: false,
            auth_token: String::new(),
        }
    }
}

impl KernelSettings {
    /// 是否已配置鉴权凭据（纯空白串不算）。
    pub fn has_auth(&self) -> bool {
        !self.auth_token.trim().is_empty()
    }

    /// 实际绑定 IP。开=0.0.0.0，关=127.0.0.1。
    pub fn bind_ip(&self) -> std::net::IpAddr {
        if self.bind_lan {
            std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
        } else {
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        }
    }

    /// 校验「开 bind_lan 必须已配凭据」。返回 `Err` 时调用方不得落库 / 不得监听 0.0.0.0。
    ///
    /// 错误消息是 i18n key（前端翻译），与仓库既有 command 错误串风格一致。
    pub fn check_bind_precondition(&self) -> Result<(), String> {
        if self.bind_lan && !self.has_auth() {
            return Err("kernel.bindLanRequiresAuth".to_string());
        }
        Ok(())
    }
}

/// 读设置。无记录 / 解析失败 → 默认值（关 + 无凭据），不 panic。
pub async fn load_kernel_settings(db: &Db) -> KernelSettings {
    match db::get_setting(db, "kernel", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "parse kernel settings failed, using defaults");
            KernelSettings::default()
        }),
        _ => KernelSettings::default(),
    }
}

/// 写设置。**入口即校验**前提条件，绕不过去（命令层与内核启动层共用这一条路）。
pub async fn save_kernel_settings(db: &Db, s: &KernelSettings) -> Result<(), String> {
    s.check_bind_precondition()?;
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
    /// 写内核管理面设置（端口 / 凭据 / 绑定开关一起提交）。
    ///
    /// 未配凭据却要开 `bind_lan` → `Err("kernel.bindLanRequiresAuth")`，整笔不落库。
    pub async fn kernel_settings_set(settings: KernelSettings) -> Result<(), String> {
        save_kernel_settings(aidog_ctx::db(), &settings).await
    }
}

crate::tauri_command! {
    /// 单独切换内核绑定开关（设置页那个 Switch 用）。
    ///
    /// 开启前提未满足时拒绝并返回原因 key，DB 里的值保持原样。
    pub async fn kernel_set_bind_lan(enabled: bool) -> Result<(), String> {
        let db = aidog_ctx::db();
        let mut s = load_kernel_settings(db).await;
        s.bind_lan = enabled;
        save_kernel_settings(db, &s).await
    }
}

#[cfg(test)]
#[path = "test_kernel_settings.rs"]
mod test_kernel_settings;
