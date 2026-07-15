//! CLI 代理 provider 探测命令（cpa-standalone-module s3）。
//!
//! 临时用 provider 配置查余额，不落库。按 provider.quota.type 分流查询入口：
//! - `newapi` → NewAPI 专用入口（两步：sub-user / dashscope，extra 透传 balance 配置）
//! - 其他 / `none` / 缺省 → 原生 query_quota（9 平台 base_url 关键词 dispatch）
//!
//! platform_id=0，None-guard 保证零落库。

use std::sync::Arc;

use aidog_core::gateway::{
    db::{self, Db},
    models::parse_quota_type,
    quota::{with_cli_proxy_provider_id, PlatformQuota},
};
use tauri::State;

/// 临时用 provider 配置查余额，不落库（preview）。
/// 按 provider.quota JSON 的 type 字段分流查询入口（cli-proxy-quota-type s1）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_proxy_test(db: State<'_, Db>, id: u64) -> Result<PlatformQuota, String> {
    tracing::debug!(command = "cli_proxy_test", id, "command invoked");
    let provider = db::get_cli_proxy_provider(&db, id)
        .await?
        .ok_or_else(|| format!("cli_proxy_provider {id} 不存在"))?;
    let quota_type = parse_quota_type(&provider.quota);
    tracing::debug!(
        command = "cli_proxy_test",
        base_url = %provider.base_url,
        api_key = "[REDACTED]",
        quota_type = %quota_type,
        "querying quota"
    );
    let db_arc = Arc::new(db.inner().clone());
    // platform_id=0：persist_quota_to_db 的 None-guard 等价直接绕过（见 cpa_import_preview_quota 注释）。
    // with_cli_proxy_provider_id scope 透传 provider id → make_quota_log 填 cli_proxy_provider_id。
    let q = match quota_type.as_str() {
        "newapi" => {
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
        }
        _ => {
            with_cli_proxy_provider_id(
                provider.id as i64,
                aidog_core::gateway::quota::query_quota(
                    Some(&db_arc),
                    &provider.base_url,
                    &provider.api_key,
                    0,
                ),
            )
            .await
        }
    };
    tracing::info!(command = "cli_proxy_test", success = q.success, "quota preview done");
    Ok(q)
}
