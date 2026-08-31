//! 平台余额 & Coding Plan 配额查询服务（拆自 aidog_core::gateway::quota，2026-08-16）。
//!
//! quota-scripts spec（T4/T5）：查询执行统一走 registry JS 脚本（`run_quota_script` →
//! `script::run_custom_query`），脚本正文来自 platform 行的物化列（空则回落 registry
//! 选中/首条变体，见 `aidog_db::registry::resolve_quota_script`）。旧 per-platform
//! Rust 查询实现已随 T5 删除。
//!
//! dispatch 保留旧 `query_quota`（base_url 自动检测）签名，调用方零改动；
//! 另提供按 Protocol 的类型化入口。

pub mod capability;
pub mod http;
pub mod script;

pub use capability::{QuotaCapability, capability_for_variant};
pub use http::{BalanceInfo, CodingPlanInfo, PlatformQuota, QuotaTier};
pub use script::CustomQueryCtx;

use std::sync::Arc;

use aidog_db::models::{Platform, Protocol};
use aidog_db::Db;

use http::{err_quota, QUOTA_PLATFORM_ID};
use script::run_custom_query;

// ── 入口 1: 能力配置（选中变体 returns 派生）──────────────

/// 按平台协议返回该平台的 quota 能力配置（quota-scripts spec：由 registry 首条变体的
/// `returns` 声明派生，替代旧的 per-platform 硬编码）。
/// 无 quota 脚本的平台返回空能力（全 false），custom_query 恒可用。
pub fn quota_config_for(protocol: &Protocol) -> QuotaCapability {
    let variants = aidog_db::registry::quota_scripts_in(
        &aidog_db::registry::effective_presets(),
        &protocol.wire_str(),
    );
    match variants.first() {
        Some(v) => capability_for_variant(v),
        None => QuotaCapability {
            custom_query_supported: true,
            ..Default::default()
        },
    }
}

// ── 入口 2: 完整查询结果 ─────────────────────────────────

/// 统一脚本执行入口：解析生效脚本（物化列 → `extra.quota_custom_script` → 选中/首条
/// 变体，`registry::resolve_quota_script`）后 `run_custom_query`。
/// 返回 None = 该协议无任何脚本（调用方回落 base_url 启发式或维持 Unsupported err）。
pub async fn run_quota_script(
    db: Option<&Arc<Db>>,
    protocol_code: &str,
    base_url: &str,
    api_key: &str,
    extra: &str,
    platform_id: i64,
) -> Option<PlatformQuota> {
    let materialized = platform_row(db, platform_id)
        .await
        .map(|p| p.quota_script)
        .unwrap_or_default();
    run_script_at(db, protocol_code, base_url, api_key, extra, &materialized, platform_id).await
}

async fn run_script_at(
    db: Option<&Arc<Db>>,
    protocol_code: &str,
    base_url: &str,
    api_key: &str,
    extra: &str,
    materialized: &str,
    platform_id: i64,
) -> Option<PlatformQuota> {
    let script =
        aidog_db::registry::resolve_quota_script(protocol_code, extra, materialized)?;
    Some(
        run_custom_query(
            db,
            CustomQueryCtx {
                base_url: base_url.to_string(),
                api_key: api_key.to_string(),
                extra: extra.to_string(),
            },
            &script,
            platform_id,
        )
        .await,
    )
}

/// platform_id > 0 且 db 可用时读平台行（协议路由 + extra + 物化脚本列）；
/// 否则 None（base_url 启发式路径，如 cli_proxy provider 探测）。
async fn platform_row(db: Option<&Arc<Db>>, platform_id: i64) -> Option<Platform> {
    if platform_id <= 0 {
        return None;
    }
    let db = db?;
    aidog_db::get_platform(db, platform_id as u64)
        .await
        .ok()
        .flatten()
}

/// 根据 base_url 自动检测平台并查询余额或 Coding Plan 配额（旧签名，调用方零改动）。
/// platform_id 透传给落库日志（task_local scope），让 Logs 页能显示归属平台。
pub async fn query_quota(db: Option<&Arc<Db>>, base_url: &str, api_key: &str, platform_id: i64) -> PlatformQuota {
    let row = platform_row(db, platform_id).await;
    QUOTA_PLATFORM_ID.scope(platform_id, query_quota_with_row(db, base_url, api_key, row)).await
}

/// 行在 → 按行协议走脚本（协议是权威：newapi 两步查询 / devin ACU / 11 平台族全覆盖，
/// 含 base_url 启发式打不中的自定义网关域名）；行协议无脚本再回落 base_url 启发式。
async fn query_quota_with_row(
    db: Option<&Arc<Db>>,
    base_url: &str,
    api_key: &str,
    row: Option<Platform>,
) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("API key is empty");
    }
    if let Some(p) = &row
        && let Some(q) = run_script_at(
            db,
            &p.platform_type.wire_str(),
            base_url,
            api_key,
            &p.extra,
            &p.quota_script,
            p.id as i64,
        )
        .await
    {
        return q;
    }
    query_quota_inner(db, base_url, api_key).await
}

/// 按 Protocol 类型化入口（平台注册即用，无需 base_url 启发式）。
pub async fn query_quota_for(
    db: Option<&Arc<Db>>,
    protocol: &Protocol,
    base_url: &str,
    api_key: &str,
    platform_id: i64,
) -> PlatformQuota {
    let row = platform_row(db, platform_id).await;
    QUOTA_PLATFORM_ID.scope(platform_id, query_for_inner(db, protocol, base_url, api_key, row)).await
}

async fn query_for_inner(
    db: Option<&Arc<Db>>,
    protocol: &Protocol,
    base_url: &str,
    api_key: &str,
    row: Option<Platform>,
) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("API key is empty");
    }
    let (extra, materialized, platform_id) = match &row {
        Some(p) => (p.extra.as_str(), p.quota_script.as_str(), p.id as i64),
        None => ("", "", 0),
    };
    if let Some(q) = run_script_at(db, &protocol.wire_str(), base_url, api_key, extra, materialized, platform_id).await {
        return q;
    }
    // 未注册平台回落 base_url 启发式（New API 系中转按 URL 判定）
    query_quota_inner(db, base_url, api_key).await
}

/// base_url 启发式 dispatch：URL 关键词 → registry 协议 code → 脚本执行。
/// 无命中 / 脚本缺失 → 维持原 `Unsupported base_url` err 文案。
async fn query_quota_inner(db: Option<&Arc<Db>>, base_url: &str, api_key: &str) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("API key is empty");
    }
    let url = base_url.to_lowercase();
    let unsupported = || err_quota(&format!("Unsupported base_url for quota query: {base_url}"));

    // Coding Plan 查询 (优先检测，这些平台通常同时有 Coding Plan)
    let code = if url.contains("api.kimi.com/coding") {
        "kimi"
    } else if url.contains("bigmodel.cn") || url.contains("api.z.ai") {
        // glm 族脚本按 ctx.baseUrl 自派生 open.bigmodel.cn / api.z.ai，两分支同一正文
        "glm"
    } else if url.contains("api.minimaxi.com") {
        "minimax"
    } else if url.contains("api.minimax.io") {
        "minimax_en"
    } else if url.contains("api.deepseek.com") {
        "deepseek"
    } else if url.contains("api.stepfun.com") || url.contains("api.stepfun.ai") {
        "stepfun"
    } else if url.contains("api.siliconflow.cn") {
        "siliconflow"
    } else if url.contains("api.siliconflow.com") {
        "siliconflow_en"
    } else if url.contains("openrouter.ai") {
        "openrouter"
    } else if url.contains("api.novita.ai") {
        "novita"
    } else {
        return unsupported();
    };
    // 启发式路径无平台行（extra/物化列空），零配置走 registry 首条变体
    run_script_at(db, code, base_url, api_key, "", "", 0)
        .await
        .unwrap_or_else(unsupported)
}

#[cfg(test)]
#[path = "test_balance.rs"]
mod test_balance;
#[cfg(test)]
#[path = "test_coding_plan.rs"]
mod test_coding_plan;
#[cfg(test)]
#[path = "test_dispatch.rs"]
mod test_dispatch;
#[cfg(test)]
#[path = "test_special_scripts.rs"]
mod test_special_scripts;
#[cfg(test)]
#[path = "test_stub.rs"]
mod test_stub;
