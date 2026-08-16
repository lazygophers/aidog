use std::sync::Arc;
use aidog_db::Db;
use crate::quota::http::{
    err_quota_platform, now_millis, parse_f64_field, quota_get_json, BalanceInfo, PlatformQuota,
};
use crate::quota::capability::QuotaCapability;

fn balance_ok(balance: BalanceInfo) -> PlatformQuota {
    PlatformQuota {
        success: true,
        error: None,
        queried_at: now_millis(),
        balance: Some(balance),
        coding_plan: None,
        newapi_user_id: None,
    }
}
// ── 余额查询: OpenRouter ─────────────────────────────────
// GET https://openrouter.ai/api/v1/credits

pub async fn query_openrouter_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(
        db,
        "https://openrouter.ai/api/v1/credits",
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("openrouter", &e),
    };
    parse_openrouter_balance(&body)
}

pub fn parse_openrouter_balance(body: &serde_json::Value) -> PlatformQuota {
    let data = body.get("data").unwrap_or(body);
    let total_credits = parse_f64_field(data, "total_credits").unwrap_or(0.0);
    let total_usage = parse_f64_field(data, "total_usage").unwrap_or(0.0);
    let remaining = total_credits - total_usage;
    balance_ok(BalanceInfo {
        remaining,
        total: Some(total_credits),
        used: Some(total_usage),
        currency: "USD".into(),
        is_valid: remaining > 0.0,
    })
}


/// 平台 quota 能力配置（三函数模式之一：无参静态配置）
pub fn quota_config() -> QuotaCapability {
    QuotaCapability { supports_balance: true, tier_names: vec![], supports_coding_plan: false, supports_mcp_query: false, ..Default::default() }.with_custom()
}
