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
// ── 余额查询: SiliconFlow ────────────────────────────────
// GET https://api.siliconflow.cn/v1/user/info

pub async fn query_siliconflow_balance(
    db: Option<&Arc<Db>>,
    api_key: &str,
    is_cn: bool,
) -> PlatformQuota {
    let domain = if is_cn {
        "api.siliconflow.cn"
    } else {
        "api.siliconflow.com"
    };
    let url = format!("https://{domain}/v1/user/info");
    let body = match quota_get_json(db, &url, &[("Authorization", format!("Bearer {api_key}"))]).await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("siliconflow", &e),
    };
    parse_siliconflow_balance(&body, is_cn)
}

pub fn parse_siliconflow_balance(body: &serde_json::Value, is_cn: bool) -> PlatformQuota {
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota_platform("siliconflow", "Missing data field"),
    };
    let total = parse_f64_field(data, "totalBalance").unwrap_or(0.0);
    let unit = if is_cn { "CNY" } else { "USD" };
    balance_ok(BalanceInfo {
        remaining: total,
        total: None,
        used: None,
        currency: unit.into(),
        is_valid: true,
    })
}


/// 平台 quota 能力配置（三函数模式之一：无参静态配置）
pub fn quota_config() -> QuotaCapability {
    QuotaCapability { supports_balance: true, tier_names: vec![], supports_coding_plan: false, supports_mcp_query: false, ..Default::default() }.with_custom()
}
