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
// ── 余额查询: Novita AI ──────────────────────────────────
// GET https://api.novita.ai/v3/user/balance

pub async fn query_novita_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(
        db,
        "https://api.novita.ai/v3/user/balance",
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("novita", &e),
    };
    parse_novita_balance(&body)
}

pub fn parse_novita_balance(body: &serde_json::Value) -> PlatformQuota {
    // Novita 金额单位 0.0001 USD
    let available = parse_f64_field(body, "availableBalance").unwrap_or(0.0) / 10000.0;
    balance_ok(BalanceInfo {
        remaining: available,
        total: None,
        used: None,
        currency: "USD".into(),
        is_valid: available > 0.0,
    })
}


/// 平台 quota 能力配置（三函数模式之一：无参静态配置）
pub fn quota_config() -> QuotaCapability {
    QuotaCapability { supports_balance: true, tier_names: vec![], supports_coding_plan: false, supports_mcp_query: false, ..Default::default() }.with_custom()
}
