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
// ── Coding Plan: GLM (智谱) ──────────────────────────────
// GET {base}/api/monitor/usage/quota/limit

pub async fn query_zhipu_coding_plan(
    db: Option<&Arc<Db>>,
    base_url: &str,
    api_key: &str,
) -> PlatformQuota {
    let base = if base_url.to_lowercase().contains("bigmodel.cn") {
        "https://open.bigmodel.cn"
    } else {
        "https://api.z.ai"
    };
    let url = format!("{base}/api/monitor/usage/quota/limit");
    let body = match quota_get_json(
        db,
        &url,
        &[
            ("Authorization", api_key.to_string()), // 智谱不加 Bearer
            ("Content-Type", "application/json".to_string()),
        ],
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return err_quota_platform("zhipu", &e),
    };
    parse_zhipu_coding_plan(&body)
}

pub fn parse_zhipu_coding_plan(body: &serde_json::Value) -> PlatformQuota {
    if body.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let msg = body.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown");
        return err_quota_platform("zhipu", msg);
    }
    let data = match body.get("data") {
        Some(d) => d,
        None => return err_quota_platform("zhipu", "Missing data field"),
    };
    let level = data.get("level").and_then(|v| v.as_str()).map(String::from);
    let mut tiers = Vec::new();
    if let Some(limits) = data.get("limits").and_then(|v| v.as_array()) {
        // Phase 1: 按 unit 字段分类 TOKENS_LIMIT（unit=3→5h, unit=6→weekly）
        type Entry = (Option<i64>, f64, Option<String>);
        let mut five_hour: Option<Entry> = None;
        let mut weekly: Option<Entry> = None;
        let mut unclassified: Vec<Entry> = Vec::new();

        for item in limits {
            let limit_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let pct = item.get("percentage").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let reset_ms = item.get("nextResetTime").and_then(|v| v.as_i64());
            let reset_iso = reset_ms.and_then(millis_to_iso8601);

            if limit_type.eq_ignore_ascii_case("TOKENS_LIMIT") {
                let entry = (reset_ms, pct, reset_iso);
                match item.get("unit").and_then(|v| v.as_i64()) {
                    Some(3) if five_hour.is_none() => five_hour = Some(entry),
                    Some(6) if weekly.is_none() => weekly = Some(entry),
                    _ => unclassified.push(entry),
                }
            } else if limit_type.eq_ignore_ascii_case("TIME_LIMIT") {
                // MCP 月用量。utilization（已用%）直接取 `percentage` 字段，与 TOKENS_LIMIT
                // 同口径（GLM 上游内部一致：percentage = 已用%）。
                // 历史 bug: 旧实现用 currentValue/usage 推算 utilization，字段语义与上游相反
                // （usage 实为剩余量、currentValue 实为额度），导致 0% 已用被算成 100% 已用
                // → statusline 显 mcp 剩余 0%（实际剩 100%）。
                // 绝对量字段（limit/remaining）仅保留供精确预估基数，不参与 utilization。
                let utilization = pct;
                let limit = parse_f64_field(item, "usage").filter(|v| *v > 0.0);
                let remaining = parse_f64_field(item, "remaining").filter(|v| *v > 0.0);
                tiers.push(QuotaTier {
                    name: "mcp_monthly".into(),
                    utilization,
                    resets_at: reset_iso,
                    limit,
                    remaining,
                });
            }
        }

        // 未分类条目按 reset 升序填入空槽（兜底启发式）
        unclassified.sort_by_key(|(reset, _, _)| (reset.is_some(), reset.unwrap_or(i64::MIN)));
        for entry in unclassified {
            if five_hour.is_none() {
                five_hour = Some(entry);
            } else if weekly.is_none() {
                weekly = Some(entry);
            }
        }

        // 按固定顺序输出 token tiers
        if let Some((_, pct, resets_at)) = five_hour {
            tiers.insert(
                0,
                QuotaTier {
                    name: "five_hour".into(),
                    utilization: pct,
                    resets_at,
                    limit: None,
                    remaining: None,
                },
            );
        }
        if let Some((_, pct, resets_at)) = weekly {
            // 插入到 five_hour 之后、mcp_monthly 之前
            let pos = tiers
                .iter()
                .position(|t| t.name == "mcp_monthly")
                .unwrap_or(tiers.len());
            tiers.insert(
                pos,
                QuotaTier {
                    name: "weekly_limit".into(),
                    utilization: pct,
                    resets_at,
                    limit: None,
                    remaining: None,
                },
            );
        }
    }
    coding_plan_ok(tiers, level)
}


/// 平台 quota 能力配置（三函数模式之一：无参静态配置）
pub fn quota_config() -> QuotaCapability {
    QuotaCapability { supports_coding_plan: true, tier_names: vec!["five_hour", "weekly_limit", "mcp_monthly"].into_iter().map(String::from).collect(), supports_balance: false, supports_mcp_query: true, ..Default::default() }.with_custom()
}
