use super::*;

/// statusline 段消费的本地预估信息（只读，不上游真查）
#[derive(serde::Serialize)]
struct GroupInfoResp {
    applicable: bool,
    balance: f64,
    /// 累计预估花费（$ / 平台币种），基于 est_cost 聚合
    spent: f64,
    coding_plan: Vec<CodingTierResp>,
    requests: i64,
    /// 成功率（0-100）
    success_rate: f64,
    /// 缓存命中率（0-100）
    cache_rate: f64,
    total_tokens: i64,
    currency: String,
    /// 余额可用天数 = balance / 动态窗口日均花费；无花费 / 无余额 → null。
    /// statusline 余额段据此上色（<1 红 / <3 黄 / 否则绿）。
    balance_days_remaining: Option<f64>,
    /// 余额使用速率配色级别（usage_color 唯一阈值源）："red"|"yellow"|"green"|"neutral"。
    /// statusline / 前端只消费此 level 不重算阈值。
    balance_level: String,
}

#[derive(serde::Serialize)]
struct CodingTierResp {
    name: String,
    /// 利用率（0-100）
    utilization: f64,
    /// 预期消耗速率分级："fast" | "normal" | "busy"（旧字段，保留兼容；新配色走 level）。
    pace: String,
    /// 使用速率配色级别（usage_color 唯一阈值源）："red"|"yellow"|"green"|"neutral"。
    /// statusline / 前端只消费此 level 不重算阈值。
    level: String,
    /// 预期重置 unix 秒；无可靠来源时 null（statusline 红色时择机展示）。
    reset_at: Option<i64>,
}

/// 分组信息端点 — 仅单平台分组返回本地预估值。
/// 鉴权：`Authorization: Bearer <group_key>`，localhost-only 端点。
/// 多平台 / 无平台分组返回 `{ applicable:false, ... }`（200）。
pub(crate) async fn handle_group_info(
    state: AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    // 每次 group-info 调用生成独立 trace id（statusline 周期拉取，无上游请求关联），
    // span 内所有日志自动带 group_info{trace_id=xxxxxxxx} 前缀。
    let span = tracing::info_span!("group_info", trace_id = %crate::logging::new_trace_id());
    handle_group_info_inner(state, headers).instrument(span).await
}

async fn handle_group_info_inner(
    AxumState(state): AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let empty = || GroupInfoResp {
        applicable: false,
        balance: 0.0,
        spent: 0.0,
        coding_plan: Vec::new(),
        requests: 0,
        success_rate: 0.0,
        cache_rate: 0.0,
        total_tokens: 0,
        currency: String::new(),
        balance_days_remaining: None,
        balance_level: super::usage_color::UsageLevel::Neutral.as_str().to_string(),
    };

    // 从 Authorization: Bearer <token> 提取 group_key
    let group_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let group_key = match group_key {
        Some(n) if !n.is_empty() => n,
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };

    // 定位分组
    let groups = match super::db::list_groups(&state.db).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "group-info: list_groups failed, returning not-applicable");
            return (StatusCode::OK, Json(empty())).into_response();
        }
    };
    let group = match groups.iter().find(|g| g.group_key == group_key) {
        Some(g) => g,
        None => {
            tracing::debug!(group = %group_key, "group-info: group not found, not-applicable");
            return (StatusCode::OK, Json(empty())).into_response();
        }
    };

    // 关联平台 —— 恰好 1 个才适用
    let platforms = match super::db::get_group_platforms(&state.db, group.id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(group = %group_key, error = %e, "group-info: get_group_platforms failed, not-applicable");
            return (StatusCode::OK, Json(empty())).into_response();
        }
    };
    if platforms.len() != 1 {
        return (StatusCode::OK, Json(empty())).into_response();
    }
    let platform = &platforms[0].platform;

    // usage 统计（复用现有 db 查询，只读）
    let stats = super::db::get_group_usage_stats(&state.db, &group.group_key).await.unwrap_or(
        super::models::PlatformUsageStats {
            total_requests: 0,
            success_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_tokens: 0,
            cache_rate: 0.0,
            recent_failures: 0,
            recent_total: 0,
            total_cost: 0.0,
            today_tokens: 0,
            today_cost: 0.0,
        }
    );

    let success_rate = if stats.total_requests > 0 {
        stats.success_count as f64 / stats.total_requests as f64 * 100.0
    } else {
        0.0
    };
    let total_tokens =
        stats.total_input_tokens + stats.total_output_tokens + stats.total_cache_tokens;

    // coding plan tiers（补 pace + level + reset_at）
    // level 走 usage_color（按 window_start + cycle 推算剩余可用时间%）；
    // reset_at = window_start + cycle（预估侧推算的本周期重置点，无 window_start 时 None）。
    let now_ms = super::db::now();
    let mut coding_plan: Vec<CodingTierResp> = super::estimate::EstCodingPlan::from_json(&platform.est_coding_plan)
        .tiers
        .into_iter()
        .map(|t| {
            let pace = super::estimate::tier_pace(&t).as_str().to_string();
            let level = super::estimate::tier_usage_level(&t, now_ms).as_str().to_string();
            let reset_at = super::usage_color::cycle_ms_for_tier(&t.name)
                .filter(|_| t.window_start > 0)
                .map(|cycle| (t.window_start + cycle) / 1000);
            CodingTierResp {
                name: t.name,
                utilization: t.est_utilization,
                pace,
                level,
                reset_at,
            }
        })
        .collect();

    // 追加 manual budgets 为 coding_plan tiers（让 statusline 显示各窗口预算利用率）
    // 只追加窗口类预算（rolling/fixed/daily），"total" 由 balance 段单独展示。
    for b in platform.manual_budgets.iter().filter(|b| b.enabled && b.kind != "total") {
        let util = if b.amount > 0.0 {
            (b.consumed / b.amount * 100.0).min(100.0)
        } else {
            0.0
        };
        let label = match b.kind.as_str() {
            "total" => "total".to_string(),
            _ => {
                let w = b.window_hours.unwrap_or(1.0);
                let short = match b.window_unit {
                    super::models::WindowUnit::Minute => "m",
                    super::models::WindowUnit::Hour => "h",
                    super::models::WindowUnit::Day => "d",
                    super::models::WindowUnit::Week => "w",
                    super::models::WindowUnit::Month => "mo",
                };
                let w_int = w.fract() == 0.0;
                if w_int { format!("{}{}", w as i64, short) } else { format!("{}{}", w, short) }
            }
        };
        let pace = if util > 80.0 { "fast" } else if util > 50.0 { "normal" } else { "busy" }.to_string();
        // level 走 usage_color：按窗口剩余时间 + 利用率算剩余可用时间%。
        // 窗口预算的 cycle = window_duration_ms，remain = window_start_at + dur - now；
        // 无窗口起点 / total 类 → 中性。
        let level = {
            let dur = super::manual_budget::window_duration_ms(b);
            match (dur, b.window_start_at) {
                (Some(dur), Some(start)) => {
                    let remain = start + dur - now_ms;
                    super::usage_color::coding_tier_level(util, Some(remain), Some(dur))
                }
                _ => super::usage_color::UsageLevel::Neutral,
            }
        }
        .as_str()
        .to_string();
        coding_plan.push(CodingTierResp {
            name: label,
            utilization: util,
            pace,
            level,
            reset_at: None,
        });
    }

    // 余额 = max(est_balance_remaining, manual "total" budget remaining)
    // 只取 kind="total" 的手动预算作为余额来源；rolling/fixed/daily 是窗口限速，不是余额。
    let manual_total_remaining: f64 = platform.manual_budgets
        .iter()
        .filter(|b| b.enabled && b.kind == "total")
        .map(super::manual_budget::remaining)
        .sum::<f64>();
    let balance = platform.est_balance_remaining.max(manual_total_remaining);

    // 余额可用天数：动态窗口日速率（rate_per_hour，prd B）→ days = (balance / rate_per_hour) / 24。
    // 无用量数据 / 无余额 → null（配色中性，不报警）。
    let balance_days_remaining = {
        let rate_per_hour = super::db::get_group_hourly_rate(&state.db, &group.group_key).await.unwrap_or(None);
        match rate_per_hour {
            Some(rate) if rate > 0.0 && balance > 0.0 => Some((balance / rate) / 24.0),
            _ => None,
        }
    };
    let balance_level = super::usage_color::balance_level(balance_days_remaining).as_str().to_string();

    let resp = GroupInfoResp {
        applicable: true,
        balance,
        spent: stats.total_cost,
        coding_plan,
        requests: stats.total_requests,
        success_rate,
        cache_rate: stats.cache_rate,
        total_tokens,
        currency: String::new(),
        balance_days_remaining,
        balance_level,
    };

    (StatusCode::OK, Json(resp)).into_response()
}
