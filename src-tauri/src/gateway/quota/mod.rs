//! 平台余额 & Coding Plan 配额查询服务
//!
//! 移植自 cc-switch，支持:
//!   - 余额查询: DeepSeek, StepFun, SiliconFlow, OpenRouter, Novita
//!   - Coding Plan: Kimi, GLM (智谱), MiniMax
//!   - New API (中转平台): 两步余额查询
//!
//! 对于无法实时获取的平台，前端可通过 proxy_logs 估算用量。

mod balance;
mod coding_plan;
mod http;
mod newapi;

use std::sync::Arc;

use super::db::Db;

// 对外路径保持不变: gateway::quota::{PlatformQuota, BalanceInfo, CodingPlanInfo, QuotaTier,
// query_quota, query_quota_newapi, parse_newapi_extra}。
// allow(unused_imports): cdylib/staticlib crate 下 facade re-export 的部分项仅被 #[cfg(test)]
// 消费 (estimate.rs 测试) 或保留为对外 API, 非 test 构建视作未用 → 误报。
#[allow(unused_imports)]
pub use http::{BalanceInfo, CodingPlanInfo, PlatformQuota, QuotaTier};
#[allow(unused_imports)]
pub use newapi::{parse_newapi_extra, query_quota_newapi};

use balance::{
    query_deepseek_balance, query_novita_balance, query_openrouter_balance,
    query_siliconflow_balance, query_stepfun_balance,
};
use coding_plan::{query_kimi_coding_plan, query_minimax_coding_plan, query_zhipu_coding_plan};
use http::{err_quota, QUOTA_PLATFORM_ID};

// ── 公开入口 ──────────────────────────────────────────────

/// 根据 base_url 自动检测平台并查询余额或 Coding Plan 配额。
/// platform_id 透传给落库日志（task_local scope），让 Logs 页能显示归属平台。
pub async fn query_quota(db: Option<&Arc<Db>>, base_url: &str, api_key: &str, platform_id: i64) -> PlatformQuota {
    QUOTA_PLATFORM_ID.scope(platform_id, query_quota_inner(db, base_url, api_key)).await
}

async fn query_quota_inner(db: Option<&Arc<Db>>, base_url: &str, api_key: &str) -> PlatformQuota {
    if api_key.trim().is_empty() {
        return err_quota("API key is empty");
    }
    let url = base_url.to_lowercase();

    // Coding Plan 查询 (优先检测，这些平台通常同时有 Coding Plan)
    if url.contains("api.kimi.com/coding") {
        return query_kimi_coding_plan(db, api_key).await;
    }
    if url.contains("open.bigmodel.cn") || url.contains("bigmodel.cn") {
        // GLM 可能同时返回 coding plan
        let quota = query_zhipu_coding_plan(db, base_url, api_key).await;
        return quota;
    }
    if url.contains("api.z.ai") {
        return query_zhipu_coding_plan(db, base_url, api_key).await;
    }
    if url.contains("api.minimaxi.com") {
        return query_minimax_coding_plan(db, api_key, true).await;
    }
    if url.contains("api.minimax.io") {
        return query_minimax_coding_plan(db, api_key, false).await;
    }

    // 余额查询
    if url.contains("api.deepseek.com") {
        return query_deepseek_balance(db, api_key).await;
    }
    if url.contains("api.stepfun.com") || url.contains("api.stepfun.ai") {
        return query_stepfun_balance(db, api_key).await;
    }
    if url.contains("api.siliconflow.cn") {
        return query_siliconflow_balance(db, api_key, true).await;
    }
    if url.contains("api.siliconflow.com") {
        return query_siliconflow_balance(db, api_key, false).await;
    }
    if url.contains("openrouter.ai") {
        return query_openrouter_balance(db, api_key).await;
    }
    if url.contains("api.novita.ai") {
        return query_novita_balance(db, api_key).await;
    }

    // 不支持的平台
    err_quota("Unsupported platform for quota query")
}
