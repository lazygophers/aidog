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
// ── 余额查询: DeepSeek ───────────────────────────────────
// GET https://api.deepseek.com/user/balance

pub async fn query_deepseek_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(
        db,
        "https://api.deepseek.com/user/balance",
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("deepseek", &e),
    };
    parse_deepseek_balance(&body)
}

pub fn parse_deepseek_balance(body: &serde_json::Value) -> PlatformQuota {
    let is_available = body
        .get("is_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let mut remaining = 0.0_f64;
    if let Some(infos) = body.get("balance_infos").and_then(|v| v.as_array()) {
        for info in infos {
            remaining += parse_f64_field(info, "total_balance").unwrap_or(0.0);
        }
    }
    balance_ok(BalanceInfo {
        remaining,
        total: None,
        used: None,
        currency: "CNY".into(),
        is_valid: is_available,
    })
}


/// 平台 quota 能力配置（三函数模式之一：无参静态配置）
pub fn quota_config() -> QuotaCapability {
    QuotaCapability { supports_balance: true, tier_names: vec![], supports_coding_plan: false, supports_mcp_query: false, ..Default::default() }.with_custom()
}
