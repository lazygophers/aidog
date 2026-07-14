//! CLI 代理 provider 探测命令（cpa-standalone-module s3）。
//!
//! 临时用 provider 配置查余额，不落库。复用 `gateway::quota::query_quota`（对齐旧
//! `cpa_import_preview_quota` idiom）。provider platform_id=0，None-guard 保证零落库。

use std::sync::Arc;

use aidog_core::gateway::{db::{self, Db}, quota::PlatformQuota};
use tauri::State;

/// 临时用 provider 配置查余额，不落库（preview）。
/// 9 provider 支持（DeepSeek/OpenRouter/GLM/Kimi/MiniMax/NewAPI/SiliconFlow/StepFun/Novita），
/// 不支持者 PlatformQuota.success=false 前端显「—」。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_proxy_test(db: State<'_, Db>, id: u64) -> Result<PlatformQuota, String> {
    tracing::debug!(command = "cli_proxy_test", id, "command invoked");
    let provider = db::get_cli_proxy_provider(&db, id)
        .await?
        .ok_or_else(|| format!("cli_proxy_provider {id} 不存在"))?;
    tracing::debug!(
        command = "cli_proxy_test",
        base_url = %provider.base_url,
        api_key = "[REDACTED]",
        "querying quota"
    );
    let db_arc = Arc::new(db.inner().clone());
    // platform_id=0：persist_quota_to_db 的 None-guard 等价直接绕过（见 cpa_import_preview_quota 注释）。
    let q = aidog_core::gateway::quota::query_quota(Some(&db_arc), &provider.base_url, &provider.api_key, 0).await;
    tracing::info!(command = "cli_proxy_test", success = q.success, "quota preview done");
    Ok(q)
}
