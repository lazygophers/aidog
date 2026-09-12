//! 无查询 API 平台的静态配额锚点（spec B2）。
//!
//! bailian_coding 官方只给控制台，无 quota 查询 API；registry `platform.json` 的
//! `plan_quotas`（官方公布的套餐绝对额度）直接作 EstTier 锚点：has_base=true +
//! limit=amount + unit=prompt_count，per-request 增量 `+100/limit` 精确预估，无需真查。

use super::model::EstTier;

/// registry `plan_quotas[].budgets[]` 的最小解析形状（其余字段忽略）。
#[derive(serde::Deserialize)]
struct PlanBudget {
    unit: String,
    amount: f64,
    #[serde(default)]
    window_hours: f64,
    #[serde(default)]
    window_unit: Option<String>,
}

/// registry `plan_quotas[]` 一档（官方套餐档位）。
#[derive(serde::Deserialize)]
struct PlanTier {
    #[serde(default)]
    budgets: Vec<PlanBudget>,
}

/// 取某协议 `plan_quotas` 首档的 count 型额度 → EstTier 锚点。
/// 多档（Lite/Pro/Max）时无法从 registry 判断用户买的哪档，取首档近似；
/// 仅 `unit == "count"` 的 budget 生成锚点（token 型走 tokens 兜底，不在此造锚点）。
/// tier name 按 window 映射既有周期名：5h → five_hour、周 → weekly_limit、
/// 月 → monthly（无已知周期 → 配色退中性，不影响 per-request 增量）。
pub fn anchors_in(doc: &serde_json::Value, protocol: &str) -> Vec<EstTier> {
    let Some(arr) = doc
        .get("protocols")
        .and_then(|p| p.get(protocol))
        .and_then(|t| t.get("plan_quotas"))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    let Some(tier) = arr
        .iter()
        .filter_map(|t| serde_json::from_value::<PlanTier>(t.clone()).ok())
        .find(|t| !t.budgets.is_empty())
    else {
        return Vec::new();
    };
    tier.budgets
        .iter()
        .filter(|b| b.unit == "count" && b.amount > 0.0)
        .filter_map(|b| {
            let name = match b.window_unit.as_deref() {
                Some("hour") if b.window_hours == 5.0 => "five_hour",
                Some("week") => "weekly_limit",
                Some("month") => "monthly",
                _ => return None,
            };
            Some(EstTier {
                name: name.to_string(),
                has_base: true,
                limit: b.amount,
                unit: "prompt_count".to_string(),
                ..Default::default()
            })
        })
        .collect()
}

/// [`anchors_in`] 的生效入口（DB 同步值优先，与 `peak::default_peak` 同源同 idiom）。
pub fn default_plan_quota_anchors(protocol: &str) -> Vec<EstTier> {
    anchors_in(&aidog_db::registry::effective_presets(), protocol)
}

#[cfg(test)]
#[path = "test_anchors.rs"]
mod test_anchors;
