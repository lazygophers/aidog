//! 管理接口设置（ADR 0010）：开关 + 可选 Bearer 凭据。
//!
//! 管理接口（`POST /api/rpc/<命令>`、`GET /api/events`、内核 `--ui` 的 `/api/ui/`）挂在
//! **代理端口**上，桌面版与内核版一致；只接受本机连接（复用 `/api/*` 既有的非回环拒绝，
//! 与代理的 `bind_lan` 无关）。设置键仍叫 `kernel/settings`（持久化兼容）。
//!
//! - [`KernelSettings::api_enabled`]：统一启停，缺省开；关着时三类路径返回「未启用」而不落入转发。
//! - [`KernelSettings::auth_token`]：非空时三类路径都要求 `Authorization: Bearer <auth_token>`；
//!   空 = 不校验。反向代理回连本机时，同机其他进程同样够得着代理端口，凭据是这一层的防线。

use crate::gateway::proxy::{SETTINGS_TEST_SOURCE, SOURCE_HEADER};
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

/// 测试面板发送结果：真实状态码、耗时、响应正文。
#[derive(serde::Serialize, Clone, Debug, ts_rs::TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ApiTestResult {
    pub status: u16,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub body: String,
}

/// 向本机代理端口上的 `POST /api/rpc/<命令>` 发真实 HTTP 请求（webview 跨源 fetch 读不到
/// 无 CORS 头的响应，故走 Rust）。带 `X-Aidog-Source: settings-test` 以便调用记录区分来源；
/// 凭据非空带 Bearer。错误码：`invalid_command` / `proxy_unreachable`。
pub async fn send_management_request(
    port: u16,
    token: &str,
    command: &str,
    params: &str,
) -> Result<ApiTestResult, String> {
    if command.is_empty() || !command.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("invalid_command".into());
    }
    let body = if params.trim().is_empty() { "{}" } else { params };
    let mut req = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("http://127.0.0.1:{port}/api/rpc/{command}"))
        .header("content-type", "application/json")
        .header(SOURCE_HEADER, SETTINGS_TEST_SOURCE)
        .body(body.to_string());
    if !token.trim().is_empty() {
        req = req.bearer_auth(token.trim());
    }
    let started = std::time::Instant::now();
    let resp = req.send().await.map_err(|e| {
        if e.is_connect() { "proxy_unreachable".to_string() } else { e.to_string() }
    })?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    Ok(ApiTestResult { status, duration_ms: started.elapsed().as_millis() as u64, body })
}

crate::tauri_command! {
    /// 设置页测试面板：经代理端口真实调用一个管理接口命令（走开关 / 本机限制 / 凭据 / 遮挡）。
    pub async fn api_test_send(command: String, params: String) -> Result<ApiTestResult, String> {
        let db = aidog_ctx::db();
        let port = crate::shared::load_proxy_settings(db).await?.port;
        let token = load_kernel_settings(db).await.auth_token;
        send_management_request(port, &token, &command, &params).await
    }
}

#[cfg(test)]
#[path = "test_kernel_settings.rs"]
mod test_kernel_settings;
