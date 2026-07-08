//! DB 集成：余额原子自减 / coding plan read-modify-write / 校准覆盖 / 请求后预估入口

use rusqlite::params;

use super::algo::{apply_tier_delta, balance_cost, calibrate_tier, should_calibrate};
use super::model::EstCodingPlan;
use crate::gateway::db::{now, Db};
use crate::gateway::quota::PlatformQuota;

/// 读取平台校准状态（短持锁）
pub async fn read_estimate_state(db: &Db, platform_id: u64) -> Result<(i64, i64), String> {
    db.write_conn()
        .call(move |conn| {
            Ok(conn.query_row(
                "SELECT last_real_query_at, estimate_count FROM platform WHERE id = ?1",
                params![platform_id as i64],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// 余额原子自减 + estimate_count+1（单条 SQL，闭包原子，无 read-modify-write 间隙）
pub async fn apply_balance_delta(db: &Db, platform_id: u64, cost: f64) -> Result<(), String> {
    db.write_conn()
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET est_balance_remaining = est_balance_remaining - ?1, estimate_count = estimate_count + 1 WHERE id = ?2",
                params![cost, platform_id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
    // est_balance_remaining 内嵌于 GroupDetail.platforms；list_group_details 非代理热路径，失效廉价。
    db.invalidate_group_details_cache();
    Ok(())
}

/// coding plan 预估：一次闭包内 SELECT→修改→UPDATE（read-modify-write 串行，避免并发覆盖）。
/// 同时 estimate_count+1。
pub async fn apply_coding_plan_delta(db: &Db, platform_id: u64, tokens: f64) -> Result<(), String> {
    db.write_conn()
        .call(move |conn| {
            let json: String = conn.query_row(
                "SELECT est_coding_plan FROM platform WHERE id = ?1",
                params![platform_id as i64],
                |r| r.get(0),
            )?;
            let mut plan = EstCodingPlan::from_json(&json);
            for tier in plan.tiers.iter_mut() {
                apply_tier_delta(tier, tokens);
            }
            conn.execute(
                "UPDATE platform SET est_coding_plan = ?1, estimate_count = estimate_count + 1 WHERE id = ?2",
                params![plan.to_json(), platform_id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
    db.invalidate_group_details_cache();
    Ok(())
}

/// 校准覆盖（短写）：用真值覆盖 est_balance_remaining + est_coding_plan，
/// 重置 last_real_query_at + estimate_count。coding plan 在闭包外已拟合好。
pub async fn write_real_quota(
    db: &Db,
    platform_id: u64,
    est_balance: f64,
    est_coding_plan_json: &str,
    now_ms: i64,
) -> Result<(), String> {
    let est_coding_plan_json = est_coding_plan_json.to_string();
    db.write_conn()
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET est_balance_remaining = ?1, est_coding_plan = ?2, last_real_query_at = ?3, estimate_count = 0 WHERE id = ?4",
                params![est_balance, est_coding_plan_json, now_ms, platform_id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
    db.invalidate_group_details_cache();
    Ok(())
}

/// 根据真查结果 + 上一窗口预估状态，构造校准后的 est_coding_plan JSON（纯计算 + 一次短读拿 prev）。
pub fn build_calibrated_coding_plan(prev: &EstCodingPlan, quota: &PlatformQuota) -> EstCodingPlan {
    let cp = match &quota.coding_plan {
        Some(c) => c,
        None => return EstCodingPlan::default(),
    };
    let tiers = cp
        .tiers
        .iter()
        .map(|t| {
            let has_base = t.limit.is_some();
            let prev_tier = prev
                .tiers
                .iter()
                .find(|p| p.name == t.name)
                .cloned()
                .unwrap_or_default();
            calibrate_tier(&prev_tier, &t.name, t.utilization, has_base, t.limit, t.resets_at.as_deref(), now())
        })
        .collect();
    EstCodingPlan { tiers, level: cp.level.clone() }
}

/// 用一次真查结果对齐 est（严格覆盖）：est_balance/est_coding_plan = 真实值，
/// 重置 last_real_query_at + estimate_count，并（方案 B）拟合 coef。
/// 供 GUI 手动真查 + 冷启动初始化复用——确保真查发生时 est 立即严格对齐真实，
/// 避免 raw CodingPlanInfo JSON 直写 est_coding_plan（字段 utilization≠est_utilization）导致 est 显 0/偏差。
/// 一次短读拿 prev coding plan（用于拟合）→ 锁外纯计算 → write_real_quota 短持锁覆盖。
pub async fn calibrate_from_quota(db: &Db, platform_id: u64, quota: &PlatformQuota, is_coding_plan: bool) {
    if !quota.success {
        tracing::warn!(platform_id, is_coding_plan, error = ?quota.error, "calibrate_from_quota skipped: upstream quota query failed, keeping estimates");
        return;
    }
    let prev_json: String = db
        .write_conn()
        .call(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT est_coding_plan FROM platform WHERE id = ?1",
                    params![platform_id as i64],
                    |r| r.get(0),
                )
                .unwrap_or_default())
        })
        .await
        .unwrap_or_default();
    let prev = EstCodingPlan::from_json(&prev_json);
    let est_balance = if is_coding_plan {
        0.0
    } else {
        quota.balance.as_ref().map(|b| b.remaining).unwrap_or(0.0)
    };
    let coding_json = if is_coding_plan {
        build_calibrated_coding_plan(&prev, quota).to_json()
    } else {
        String::new()
    };
    let result = write_real_quota(db, platform_id, est_balance, &coding_json, now()).await;
    tracing::info!(platform_id, is_coding_plan, coding_json_len = coding_json.len(), result = ?result, "calibrate_from_quota done");
}

/// 后台校准编排：锁外 await query_quota → 锁内覆盖。失败保留预估（不重置）。
/// NewApi 平台走专用两步查询（query_quota_newapi），与 lib.rs 手动查询/冷启动一致；
/// 否则 query_quota 按 base_url 子串分派对 newapi 自定义实例返 "Unsupported" → est 永不主动刷新。
async fn run_calibration(
    db: &Db,
    platform_id: u64,
    platform_type: &str,
    base_url: &str,
    api_key: &str,
    extra: &str,
    is_coding_plan: bool,
) {
    // 锁外 async 真查（构造 Arc<Db> 供 http_client 读系统代理设置）
    let db_arc = std::sync::Arc::new(db.clone());
    let quota = if platform_type == "newapi" {
        crate::gateway::quota::query_quota_newapi(Some(&db_arc), base_url, api_key, extra, platform_id as i64).await
    } else {
        crate::gateway::quota::query_quota(Some(&db_arc), base_url, api_key, platform_id as i64).await
    };
    // 失败时 calibrate_from_quota 自身 early-return（保留预估值，不重置计数/时间，下次请求再试）。
    calibrate_from_quota(db, platform_id, &quota, is_coding_plan).await;
}

/// 单次请求后的预估入口（在 proxy 后台 tokio::spawn 中调用）。
/// - 余额平台（非 coding plan）：扣金额。
/// - coding plan 平台：更新 utilization。
/// - 命中校准阈值 → 触发真查覆盖（锁外 await）。
#[allow(clippy::too_many_arguments)]
pub async fn estimate_after_request(
    db: &Db,
    platform_id: u64,
    platform_type: &str,
    base_url: &str,
    api_key: &str,
    model: &str,
    extra: &str,
    input_tokens: i64,
    output_tokens: i64,
    cache_tokens: i64,
    is_coding_plan: bool,
) {
    // resolve_price 单次解析，余额扣减（balance delta）与手动预算 est_cost 复用同一 ResolvedPrice
    // （同一 (model, platform_type, input_tokens)，结果等价），避免对余额平台重复解析两次。
    let resolved_price =
        crate::gateway::db::resolve_price(db, model, platform_type, 0.0, 0.0, input_tokens)
            .await
            .ok();

    // 1. 增量预估
    if is_coding_plan {
        let total = (input_tokens + output_tokens + cache_tokens) as f64;
        let _ = apply_coding_plan_delta(db, platform_id, total).await;
    } else if let Some(ref price) = resolved_price {
        // 按量平台扣金额
        let cost = balance_cost(
            input_tokens,
            output_tokens,
            cache_tokens,
            price.input_cost_per_token,
            price.output_cost_per_token,
            price.cache_read_input_token_cost,
        );
        let _ = apply_balance_delta(db, platform_id, cost).await;
    }

    // 1b. 手动预算扣减（独立于上游 quota 的并行机制；无 manual_budgets 则 no-op）。
    //     est_cost 走 resolve_price（与按量平台一致，含默认价回退）；token 扣总 token。
    {
        let total_tokens = (input_tokens + output_tokens + cache_tokens) as f64;
        let est_cost = resolved_price
            .as_ref()
            .map(|price| {
                balance_cost(
                    input_tokens,
                    output_tokens,
                    cache_tokens,
                    price.input_cost_per_token,
                    price.output_cost_per_token,
                    price.cache_read_input_token_cost,
                )
            })
            .unwrap_or(0.0);
        let _ = crate::gateway::manual_budget::apply_manual_budgets(
            db,
            platform_id,
            est_cost,
            total_tokens,
            now(),
        )
        .await;
    }

    // 2. 校准判定（短读，锁外 await）
    if let Ok((last_real, count)) = read_estimate_state(db, platform_id).await {
        if should_calibrate(now(), last_real, count) {
            run_calibration(db, platform_id, platform_type, base_url, api_key, extra, is_coding_plan).await;
        }
    }
}

#[cfg(test)]
#[path = "test_db_ops.rs"]
mod test_db_ops;
