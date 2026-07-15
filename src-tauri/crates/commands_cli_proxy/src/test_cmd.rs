//! CLI 代理 provider 探测命令（cpa-standalone-module s3）。
//!
//! 临时用 provider 配置查余额，不落库。复用 `gateway::quota::query_quota`（对齐旧
//! `cpa_import_preview_quota` idiom）。provider platform_id=0，None-guard 保证零落库。

use std::sync::Arc;

use aidog_core::gateway::{
    db::{self, Db},
    quota::{with_cli_proxy_provider_id, PlatformQuota},
};
use tauri::State;

/// 临时用 provider 配置查余额，不落库（preview）。
/// 9 原生 provider 支持（DeepSeek/OpenRouter/GLM/Kimi/MiniMax/SiliconFlow/StepFun/Novita）；
/// NewAPI 中转 base_url 不匹配原生 dispatch → Unsupported 时 fallback 试 NewAPI 入口
/// （provider.extra 透传 balance_api_key 等配置）。不支持者 PlatformQuota.success=false 前端显「—」。
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
    // with_cli_proxy_provider_id scope 透传 provider id → make_quota_log 填 cli_proxy_provider_id。
    let q = with_cli_proxy_provider_id(
        provider.id as i64,
        aidog_core::gateway::quota::query_quota(
            Some(&db_arc),
            &provider.base_url,
            &provider.api_key,
            0,
        ),
    )
    .await;
    // query_quota 只覆盖 9 原生平台（dispatch 按 base_url 关键词）；NewAPI 中转域名不匹配
    // → Unsupported（同步返回，未发 HTTP）。fallback 试 NewAPI 入口（extra 透传 balance 配置）。
    let q = if !q.success && q.error.as_deref() == Some("Unsupported platform for quota query") {
        with_cli_proxy_provider_id(
            provider.id as i64,
            aidog_core::gateway::quota::query_quota_newapi(
                Some(&db_arc),
                &provider.base_url,
                &provider.api_key,
                &provider.extra,
                0,
            ),
        )
        .await
    } else {
        q
    };
    tracing::info!(command = "cli_proxy_test", success = q.success, "quota preview done");
    Ok(q)
}
