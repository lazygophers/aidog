use std::sync::Arc;
use aidog_db::Db;
use crate::quota::http::{
    err_quota_platform, millis_to_iso8601, now_millis, parse_f64_field, quota_get_json,
    CodingPlanInfo, PlatformQuota, QuotaTier,
};
use crate::quota::capability::QuotaCapability;

fn coding_plan_ok(tiers: Vec<QuotaTier>, level: Option<String>) -> PlatformQuota {
    PlatformQuota {
        success: true,
        error: None,
        queried_at: now_millis(),
        balance: None,
        coding_plan: Some(CodingPlanInfo { tiers, level }),
        newapi_user_id: None,
    }
}
// ── Coding Plan: Kimi ─────────────────────────────────────
// GET https://api.kimi.com/coding/v1/usages

pub async fn query_kimi_coding_plan(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(
        db,
        "https://api.kimi.com/coding/v1/usages",
        &[("Authorization", format!("Bearer {api_key}"))],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("kimi", &e),
    };
    parse_kimi_coding_plan(&body)
}

pub fn parse_kimi_coding_plan(body: &serde_json::Value) -> PlatformQuota {
    let mut tiers = Vec::new();
    // 5h 窗口
    if let Some(limits) = body.get("limits").and_then(|v| v.as_array()) {
        for item in limits {
            if let Some(detail) = item.get("detail") {
                let limit = parse_f64_field(detail, "limit").unwrap_or(1.0);
                let remaining = parse_f64_field(detail, "remaining").unwrap_or(0.0);
                let used = (limit - remaining).max(0.0);
                let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
                let resets_at = detail.get("resetTime").and_then(|v| {
                    v.as_str()
                        .map(String::from)
                        .or_else(|| v.as_i64().and_then(millis_to_iso8601))
                });
                // Kimi 暴露绝对 limit/remaining → 保留供精确预估基数
                tiers.push(QuotaTier {
                    name: "five_hour".into(),
                    utilization,
                    resets_at,
                    limit: Some(limit),
                    remaining: Some(remaining),
                });
            }
        }
    }
    // 周限额
    if let Some(usage) = body.get("usage") {
        let limit = parse_f64_field(usage, "limit").unwrap_or(1.0);
        let remaining = parse_f64_field(usage, "remaining").unwrap_or(0.0);
        let used = (limit - remaining).max(0.0);
        let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
        let resets_at = usage.get("resetTime").and_then(|v| {
            v.as_str()
                .map(String::from)
                .or_else(|| v.as_i64().and_then(millis_to_iso8601))
        });
        tiers.push(QuotaTier {
            name: "weekly_limit".into(),
            utilization,
            resets_at,
            limit: Some(limit),
            remaining: Some(remaining),
        });
    }
    coding_plan_ok(tiers, None)
}


/// 平台 quota 能力配置（三函数模式之一：无参静态配置）
pub fn quota_config() -> QuotaCapability {
    QuotaCapability { supports_coding_plan: true, tier_names: vec!["five_hour", "weekly_limit"].into_iter().map(String::from).collect(), supports_balance: false, supports_mcp_query: false, ..Default::default() }.with_custom()
}
