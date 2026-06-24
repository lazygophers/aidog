//! Coding Plan 配额查询: Kimi / GLM (智谱) / MiniMax。
//!
//! 每平台拆为 HTTP 壳 + 纯解析 (`parse_X_coding_plan`)。纯解析吃 JSON body，可单测。

use std::sync::Arc;

use crate::gateway::db::Db;

use super::http::{
    err_quota_platform, millis_to_iso8601, now_millis, parse_f64_field, quota_get_json,
    CodingPlanInfo, PlatformQuota, QuotaTier,
};

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

pub(crate) async fn query_kimi_coding_plan(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
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

pub(crate) fn parse_kimi_coding_plan(body: &serde_json::Value) -> PlatformQuota {
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

// ── Coding Plan: GLM (智谱) ──────────────────────────────
// GET {base}/api/monitor/usage/quota/limit

pub(crate) async fn query_zhipu_coding_plan(
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

pub(crate) fn parse_zhipu_coding_plan(body: &serde_json::Value) -> PlatformQuota {
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
                // MCP 月用量（绝对量）
                let total = parse_f64_field(item, "usage").unwrap_or(0.0);
                let used = parse_f64_field(item, "currentValue").unwrap_or(0.0);
                let remaining = parse_f64_field(item, "remaining").unwrap_or(0.0);
                let utilization = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };
                tiers.push(QuotaTier {
                    name: "mcp_monthly".into(),
                    utilization,
                    resets_at: reset_iso,
                    limit: if total > 0.0 { Some(total) } else { None },
                    remaining: if remaining > 0.0 { Some(remaining) } else { None },
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

// ── Coding Plan: MiniMax ─────────────────────────────────
// GET https://{domain}/v1/api/openplatform/coding_plan/remains
//   domain = is_cn ? "api.minimaxi.com" : "api.minimax.io"

pub(crate) async fn query_minimax_coding_plan(
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

pub(crate) fn parse_minimax_coding_plan(body: &serde_json::Value) -> PlatformQuota {
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
            // 周桶 (仅 status=1)
            if item.get("current_weekly_status").and_then(|v| v.as_i64()) == Some(1) {
                if let Some(remain_pct) = item
                    .get("current_weekly_remaining_percent")
                    .and_then(|v| v.as_f64())
                {
                    let resets_at = item
                        .get("weekly_end_time")
                        .and_then(|v| v.as_i64())
                        .and_then(millis_to_iso8601);
                    tiers.push(QuotaTier {
                        name: "weekly_limit".into(),
                        utilization: 100.0 - remain_pct,
                        resets_at,
                        limit: None,
                        remaining: None,
                    });
                }
            }
        }
    }
    coding_plan_ok(tiers, None)
}

#[cfg(test)]
#[path = "test_coding_plan.rs"]
mod test_coding_plan;
