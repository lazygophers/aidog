//! 平台余额 & Coding Plan 配额查询服务（拆自 aidog_core::gateway::quota，2026-08-16）。
//!
//! 三函数模式（每个有 quota 能力的平台目录挂 `quota.rs`）：
//!   1. `<platform>::quota::quota_config() -> QuotaCapability` —— 静态能力配置
//!      （是否支持 coding plan / 余额 / MCP / 周·月·五小时限制层级）
//!   2. `<platform>::quota::query_quota(...) -> PlatformQuota` —— 完整查询结果
//!   3. 本模块 `run_custom_query(...)` —— JS 脚本自定义查询
//!      （内置 http 请求 / JSON 解析等，返回固定格式 PlatformQuota）
//!
//! dispatch 保留旧 `query_quota`（base_url 自动检测）签名，调用方零改动；
//! 另提供按 Protocol 的类型化入口。

pub mod capability;
pub mod http;
pub mod script;

pub use capability::{QuotaCapability, capability_for_variant};
pub use http::{BalanceInfo, CodingPlanInfo, PlatformQuota, QuotaTier};

use std::sync::Arc;

use aidog_db::models::Protocol;
use aidog_db::Db;

use http::{err_quota, QUOTA_PLATFORM_ID};

// ── 入口 1: 能力配置（Protocol → 平台 config 函数）────────

/// 按平台协议返回该平台的 quota 能力配置。
/// 无 quota 实现的平台返回空能力（全 false），custom_query 恒可用。
pub fn quota_config_for(protocol: &Protocol) -> QuotaCapability {
    use Protocol::*;
    match protocol {
        Glm | GlmCoding | GlmEn | GlmCodingEn => crate::glm::quota::quota_config(),
        Kimi | KimiCoding => crate::kimi::quota::quota_config(),
        MiniMax | MiniMaxEn | MinimaxCoding => crate::minimax::quota::quota_config(),
        DeepSeek => crate::deepseek::quota::quota_config(),
        StepFun | StepFunEn => crate::stepfun::quota::quota_config(),
        SiliconFlow | SiliconFlowEn => crate::siliconflow::quota::quota_config(),
        OpenRouter => crate::openrouter::quota::quota_config(),
        Novita => crate::novita::quota::quota_config(),
        Devin => crate::devin::quota::quota_config(),
        _ => QuotaCapability {
            custom_query_supported: true,
            ..Default::default()
        },
    }
}

// ── 入口 2: 完整查询结果 ─────────────────────────────────

/// 根据 base_url 自动检测平台并查询余额或 Coding Plan 配额（旧签名，调用方零改动）。
/// platform_id 透传给落库日志（task_local scope），让 Logs 页能显示归属平台。
pub async fn query_quota(db: Option<&Arc<Db>>, base_url: &str, api_key: &str, platform_id: i64) -> PlatformQuota {
    QUOTA_PLATFORM_ID.scope(platform_id, query_quota_inner(db, base_url, api_key)).await
}

/// 按 Protocol 类型化入口（平台注册即用，无需 base_url 启发式）。
pub async fn query_quota_for(
    db: Option<&Arc<Db>>,
    protocol: &Protocol,
    base_url: &str,
    api_key: &str,
    platform_id: i64,
) -> PlatformQuota {
    QUOTA_PLATFORM_ID.scope(platform_id, query_for_inner(db, protocol, base_url, api_key)).await
}

async fn query_for_inner(db: Option<&Arc<Db>>, protocol: &Protocol, base_url: &str, api_key: &str) -> PlatformQuota {
    use Protocol::*;
    if api_key.trim().is_empty() {
        return err_quota("API key is empty");
    }
    match protocol {
        Glm | GlmCoding | GlmEn | GlmCodingEn => crate::glm::quota::query_zhipu_coding_plan(db, base_url, api_key).await,
        Kimi | KimiCoding => crate::kimi::quota::query_kimi_coding_plan(db, api_key).await,
        MiniMax | MinimaxCoding => crate::minimax::quota::query_minimax_coding_plan(db, api_key, true).await,
        MiniMaxEn => crate::minimax::quota::query_minimax_coding_plan(db, api_key, false).await,
        DeepSeek => crate::deepseek::quota::query_deepseek_balance(db, api_key).await,
        StepFun | StepFunEn => crate::stepfun::quota::query_stepfun_balance(db, api_key).await,
        SiliconFlow => crate::siliconflow::quota::query_siliconflow_balance(db, api_key, true).await,
        SiliconFlowEn => crate::siliconflow::quota::query_siliconflow_balance(db, api_key, false).await,
        OpenRouter => crate::openrouter::quota::query_openrouter_balance(db, api_key).await,
        Novita => crate::novita::quota::query_novita_balance(db, api_key).await,
        // 未注册平台回落 base_url 启发式（New API 系中转按 URL 判定）
        _ => query_quota_inner(db, base_url, api_key).await,
    }
}

/// base_url 启发式 dispatch（原 query_quota_inner 逻辑原样迁入）。
async fn query_quota_inner(db: Option<&Arc<Db>>, base_url: &str, api_key: &str) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("API key is empty");
    }
    let url = base_url.to_lowercase();

    // Coding Plan 查询 (优先检测，这些平台通常同时有 Coding Plan)
    if url.contains("api.kimi.com/coding") {
        return crate::kimi::quota::query_kimi_coding_plan(db, api_key).await;
    }
    if url.contains("bigmodel.cn") {
        return crate::glm::quota::query_zhipu_coding_plan(db, base_url, api_key).await;
    }
    if url.contains("api.z.ai") {
        return crate::glm::quota::query_zhipu_coding_plan(db, base_url, api_key).await;
    }
    if url.contains("api.minimaxi.com") {
        return crate::minimax::quota::query_minimax_coding_plan(db, api_key, true).await;
    }
    if url.contains("api.minimax.io") {
        return crate::minimax::quota::query_minimax_coding_plan(db, api_key, false).await;
    }

    // 余额查询
    if url.contains("api.deepseek.com") {
        return crate::deepseek::quota::query_deepseek_balance(db, api_key).await;
    }
    if url.contains("api.stepfun.com") || url.contains("api.stepfun.ai") {
        return crate::stepfun::quota::query_stepfun_balance(db, api_key).await;
    }
    if url.contains("api.siliconflow.cn") {
        return crate::siliconflow::quota::query_siliconflow_balance(db, api_key, true).await;
    }
    if url.contains("api.siliconflow.com") {
        return crate::siliconflow::quota::query_siliconflow_balance(db, api_key, false).await;
    }
    if url.contains("openrouter.ai") {
        return crate::openrouter::quota::query_openrouter_balance(db, api_key).await;
    }
    if url.contains("api.novita.ai") {
        return crate::novita::quota::query_novita_balance(db, api_key).await;
    }

    err_quota(&format!("Unsupported base_url for quota query: {base_url}"))
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
