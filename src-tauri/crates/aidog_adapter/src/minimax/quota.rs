use std::sync::Arc;
use aidog_db::Db;
use crate::quota::http::{
    err_quota_platform, millis_to_iso8601, now_millis, quota_get_json,
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
// ── Coding Plan: MiniMax ─────────────────────────────────
// GET https://{domain}/v1/api/openplatform/coding_plan/remains
//   domain = is_cn ? "api.minimaxi.com" : "api.minimax.io"

pub async fn query_minimax_coding_plan(
    db: Option<&Arc<Db>>,
    api_key: &str,
    is_cn: bool,
) -> PlatformQuota {
    let domain = if is_cn {
        "api.minimaxi.com"
    } else {
        "api.minimax.io"
    };
    let url = format!("https://{domain}/v1/api/openplatform/coding_plan/remains");
    let body = match quota_get_json(
        db,
        &url,
        &[
            ("Authorization", format!("Bearer {api_key}")),
            ("Content-Type", "application/json".to_string()),
        ],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("minimax", &e),
    };
    parse_minimax_coding_plan(&body)
}

pub fn parse_minimax_coding_plan(body: &serde_json::Value) -> PlatformQuota {
    if let Some(base_resp) = body.get("base_resp") {
        let code = base_resp
            .get("status_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        if code != 0 {
            let msg = base_resp
                .get("status_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            return err_quota_platform("minimax", &format!("API error (code {code}): {msg}"));
        }
    }
    let mut tiers = Vec::new();
    if let Some(model_remains) = body.get("model_remains").and_then(|v| v.as_array()) {
        let item = model_remains.iter().find(|i| {
            i.get("model_name")
                .and_then(|v| v.as_str())
                .map(|s| s == "general")
                .unwrap_or(false)
        });
        if let Some(item) = item {
            // 5h 桶
            if let Some(remain_pct) = item
                .get("current_interval_remaining_percent")
                .and_then(|v| v.as_f64())
            {
                let resets_at = item
                    .get("end_time")
                    .and_then(|v| v.as_i64())
                    .and_then(millis_to_iso8601);
                tiers.push(QuotaTier {
                    name: "five_hour".into(),
                    utilization: 100.0 - remain_pct,
                    resets_at,
                    limit: None,
                    remaining: None,
                });
            }
            // 周桶：status 1=有剩余 / 2=已用满，均代表存在周计划窗口 → 建桶；
            // 0/缺失=无周计划 → 跳过。旧实现仅认 status==1，把「周上限已用满(status=2)」
            // 的模型整个丢掉，导致周上限最该展示时反而不显示（general 实测 status=2）。
            let weekly_status = item.get("current_weekly_status").and_then(|v| v.as_i64());
            if matches!(weekly_status, Some(1) | Some(2))
                && let Some(remain_pct) = item
                    .get("current_weekly_remaining_percent")
                    .and_then(|v| v.as_f64())
                {
                    let resets_at = item
                        .get("weekly_end_time")
                        .and_then(|v| v.as_i64())
                        .and_then(millis_to_iso8601);
                    // 次数型模型（current_weekly_total_count>0，如 video）暴露绝对周上限，
                    // 供精确预估基数（has_base）；token 型（general，count=0）仅保留百分比。
                    let limit = item
                        .get("current_weekly_total_count")
                        .and_then(|v| v.as_f64())
                        .filter(|v| *v > 0.0);
                    let remaining = limit.and_then(|t| {
                        item.get("current_weekly_usage_count")
                            .and_then(|v| v.as_f64())
                            .map(|used| (t - used).max(0.0))
                    });
                    tiers.push(QuotaTier {
                        name: "weekly_limit".into(),
                        utilization: 100.0 - remain_pct,
                        resets_at,
                        limit,
                        remaining,
                    });
                }
        }
    }
    coding_plan_ok(tiers, None)
}


/// 平台 quota 能力配置（三函数模式之一：无参静态配置）
pub fn quota_config() -> QuotaCapability {
    QuotaCapability { supports_coding_plan: true, tier_names: vec!["five_hour", "weekly_limit"].into_iter().map(String::from).collect(), supports_balance: false, supports_mcp_query: false, ..Default::default() }.with_custom()
}
