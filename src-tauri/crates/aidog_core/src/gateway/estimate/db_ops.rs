//! DB 集成：余额原子自减 / coding plan read-modify-write / 校准覆盖 / 请求后预估入口

use rusqlite::params;

use super::algo::{apply_tier_delta, balance_cost, calibrate_tier, should_calibrate};
use super::model::EstCodingPlan;
use crate::gateway::quota::PlatformQuota;
use aidog_db::{Db, now};

/// 读取平台校准状态（短持锁）
pub async fn read_estimate_state(db: &Db, platform_id: u64) -> Result<(i64, i64), String> {
    db.platform_write_conn()
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
pub use aidog_db::apply_balance_delta;

/// coding plan 预估：一次闭包内 SELECT→修改→UPDATE（read-modify-write 串行，避免并发覆盖）。
/// 同时 estimate_count+1。`requests` 是本次触发预估的请求数（正常 1；unit==prompt_count
/// 的 tier 按它扣，token 只喂 tokens 兜底口径）。
pub async fn apply_coding_plan_delta(
    db: &Db,
    platform_id: u64,
    requests: i64,
    tokens: f64,
) -> Result<(), String> {
    db.platform_write_conn()
        .call(move |conn| {
            let json: String = conn.query_row(
                "SELECT est_coding_plan FROM platform WHERE id = ?1",
                params![platform_id as i64],
                |r| r.get(0),
            )?;
            let mut plan = EstCodingPlan::from_json(&json);
            for tier in plan.tiers.iter_mut() {
                apply_tier_delta(tier, requests, tokens);
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

/// 速率限制快照落库（覆盖写，只保留最新一条）。与 coding plan 预估无关，不碰
/// `estimate_count`：它是「离上次真查多久」的计数器，速率限制不走真查那条链。
pub async fn write_rate_limit(db: &Db, platform_id: u64, rl: &super::RateLimit) {
    let json = rl.to_json();
    if json.is_empty() {
        return;
    }
    let r = db
        .platform_write_conn()
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET rate_limit = ?1 WHERE id = ?2",
                params![json, platform_id as i64],
            )?;
            Ok(())
        })
        .await;
    if let Err(e) = r {
        tracing::warn!(platform_id, error = %e, "write_rate_limit failed");
        return;
    }
    db.invalidate_group_details_cache();
}

/// 静态锚点播种（spec B2）：无查询 API 平台（bailian_coding）用 registry `plan_quotas`
/// 的绝对额度作 has_base 锚点。仅当 est_coding_plan 为空时写（幂等，不覆盖已有预估/校准）。
pub async fn seed_plan_anchor_if_empty(db: &Db, platform_id: u64, protocol: &str) -> bool {
    let anchors = super::anchors::default_plan_quota_anchors(protocol);
    if anchors.is_empty() {
        return false;
    }
    let json: String = db
        .platform_write_conn()
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
    if !EstCodingPlan::from_json(&json).tiers.is_empty() {
        return false;
    }
    let plan = EstCodingPlan { tiers: anchors, level: None };
    write_real_quota(db, platform_id, 0.0, &plan.to_json(), now())
        .await
        .is_ok()
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
    db.platform_write_conn()
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
            calibrate_tier(
                &prev_tier,
                &t.name,
                t.utilization,
                has_base,
                t.limit,
                t.resets_at.as_deref(),
                now(),
                // unit 透传（spec B1）：脚本声明 prompt_count/mcp_time/tokens，
                // 缺失 = tokens 兜底
                t.unit.as_deref().unwrap_or("tokens"),
            )
        })
        .collect();
    EstCodingPlan {
        tiers,
        level: cp.level.clone(),
    }
}

/// 用一次真查结果对齐 est（严格覆盖）：est_balance/est_coding_plan = 真实值，
/// 重置 last_real_query_at + estimate_count，并（方案 B）拟合 coef。
/// 供 GUI 手动真查 + 冷启动初始化复用——确保真查发生时 est 立即严格对齐真实，
/// 避免 raw CodingPlanInfo JSON 直写 est_coding_plan（字段 utilization≠est_utilization）导致 est 显 0/偏差。
/// 一次短读拿 prev coding plan（用于拟合）→ 锁外纯计算 → write_real_quota 短持锁覆盖。
pub async fn calibrate_from_quota(
    db: &Db,
    platform_id: u64,
    quota: &PlatformQuota,
    is_coding_plan: bool,
) {
    if !quota.success {
        tracing::warn!(platform_id, is_coding_plan, error = ?quota.error, "calibrate_from_quota skipped: upstream quota query failed, keeping estimates");
        return;
    }
    let prev_json: String = db
        .platform_write_conn()
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
    // chart-engine T4 / #34：真实余额查询成功 → 顺手落一条 quota_snapshot（事件式，无定时器）。
    // coding plan 无按量余额（est_balance 强制 0），不落快照避免趋势被 0 污染。
    if !is_coding_plan
        && let Some(b) = &quota.balance
        && let Err(e) = aidog_stats::insert_quota_snapshot(db, platform_id, b.remaining).await
    {
        tracing::warn!(platform_id, error = %e, "insert quota snapshot failed");
    }
    tracing::info!(platform_id, is_coding_plan, coding_json_len = coding_json.len(), result = ?result, "calibrate_from_quota done");
    schedule_reset_refresh(db, platform_id, quota).await;
}

/// 已排定的重置刷新（platform_id → 目标时刻 unix ms）。同一平台同一时刻只排一次定时。
/// 内存态：进程重启即空，由冷启动真查重新排（见 `cold_start_init_estimates`）。
static RESET_REFRESH: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<u64, i64>>,
> = std::sync::OnceLock::new();

/// 排入去重表：已有一个「未到点且时刻相差 <60s」的定时 → 返回 false（不重复排）。
fn claim_refresh_slot(platform_id: u64, at_ms: i64, now_ms: i64) -> bool {
    let map = RESET_REFRESH.get_or_init(Default::default);
    let Ok(mut g) = map.lock() else { return false };
    if let Some(&existing) = g.get(&platform_id)
        && existing > now_ms
        && (existing - at_ms).abs() < 60_000
    {
        return false;
    }
    g.insert(platform_id, at_ms);
    true
}

/// coding plan 各档 `resets_at` 里最早的一个「足够靠后的未来时刻」（unix ms）。
/// 30s 下限防抖：上游返回过去/临界时刻时不排（否则真查→再排→再真查自激）。
/// 24h 上限与 `parse_quota_reset_at` 同口径：更远的时刻由下次真查再排。
fn earliest_reset_ms(quota: &PlatformQuota, now_ms: i64) -> Option<i64> {
    quota
        .coding_plan
        .as_ref()?
        .tiers
        .iter()
        .filter_map(|t| super::algo::parse_resets_to_ms(t.resets_at.as_deref()?))
        .filter(|&ms| ms > now_ms + 30_000 && ms < now_ms + 24 * 3_600_000)
        .min()
}

/// coding plan 窗口重置时刻主动真查（不等下一个请求、不等 429）。
/// 真查结果里每档带 `resets_at` → 排到最早那个时刻；到点刷新回来又带下一窗口的
/// `resets_at` → 自续，无需周期轮询。非 coding plan（余额平台）无 `resets_at` → 不排。
async fn schedule_reset_refresh(db: &Db, platform_id: u64, quota: &PlatformQuota) {
    let Some(at_ms) = earliest_reset_ms(quota, now()) else {
        return;
    };
    let Ok(Some(p)) = aidog_db::get_platform(db, platform_id).await else {
        return;
    };
    spawn_refresh_at(
        std::sync::Arc::new(db.clone()),
        platform_id,
        p.base_url,
        p.api_key,
        at_ms,
    );
}

/// 配额重置时刻主动真查一次并校准（不等下一个请求）。
///
/// 冷却期内平台不被调度 → 没有请求驱动的校准，`est_coding_plan` 会一直停在耗尽值。
/// 排一个定时任务到重置时刻（+2s 让上游窗口确实翻篇），真查回来即对齐真实额度并刷托盘。
/// coding plan / 余额平台由真查结果自身判定（`coding_plan.is_some()`，同 persist_quota_to_db）。
/// 内存态：进程重启丢失该定时，冷启动真查会重新排（`cold_start_init_estimates`）。
/// 同平台同时刻去重（`claim_refresh_slot`）：429 冷却与真查自续可能排到同一时刻，只留一个。
pub fn spawn_refresh_at(
    db: std::sync::Arc<Db>,
    platform_id: u64,
    base_url: String,
    api_key: String,
    at_ms: i64,
) {
    if !claim_refresh_slot(platform_id, at_ms, now()) {
        return;
    }
    let delay_ms = (at_ms - now()).max(0) as u64 + 2_000;
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        let quota =
            crate::gateway::quota::query_quota(Some(&db), &base_url, &api_key, platform_id as i64)
                .await;
        let is_coding_plan = quota.coding_plan.is_some();
        calibrate_from_quota(&db, platform_id, &quota, is_coding_plan).await;
        aidog_ctx::emit_unit("tray-refresh");
        tracing::info!(platform_id, "quota refreshed at upstream reset time");
    });
}

/// 后台校准编排：锁外 await query_quota → 锁内覆盖。失败保留预估（不重置）。
/// query_quota 按平台行协议路由 registry 脚本（quota-scripts T4）：newapi 两步查询 /
/// devin ACU / 11 平台族统一覆盖（旧行为缺口：devin 未特判 → base_url 启发式打不中
/// → "Unsupported"，现随统一脚本路径顺带消除）。
async fn run_calibration(
    db: &Db,
    platform_id: u64,
    base_url: &str,
    api_key: &str,
    is_coding_plan: bool,
) {
    // 锁外 async 真查（构造 Arc<Db> 供 http_client 读系统代理设置）
    let db_arc = std::sync::Arc::new(db.clone());
    let quota =
        crate::gateway::quota::query_quota(Some(&db_arc), base_url, api_key, platform_id as i64)
            .await;
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
    // peak 窗口先算：`is_peak` 决定条目里的 peak 绝对价是否生效（票 T4）。
    // 与 calc_est_cost（proxy/log.rs → billing.rs）同一口径，估算链此前漏乘（既存 bug）。
    let windows = crate::gateway::peak::peak_for(extra, platform_type);
    let is_peak = crate::gateway::peak::is_in_peak_window(&windows, now(), model);
    // resolve_price 单次解析，余额扣减（balance delta）与手动预算 est_cost 复用同一结果
    // （同一 (platform_type, model, input_tokens)，结果等价），避免对余额平台重复解析两次。
    let resolved = aidog_db::resolve_price(
        db,
        platform_type,
        model,
        0.0,
        0.0,
        input_tokens,
        now(),
        is_peak,
    )
    .await
    .ok();
    // 高峰绝对价已含涨价 → 倍率压成 1.0，否则乘平台 peak 倍率（避免双重计价）。
    let raw_mult = crate::gateway::peak::resolve_multiplier(&windows, now(), model);
    let peak_mult = resolved
        .as_ref()
        .map_or(raw_mult, |r| r.multiplier(raw_mult));
    let resolved_price = resolved.map(|r| r.price);

    // 1. 增量预估
    if is_coding_plan {
        let total = (input_tokens + output_tokens + cache_tokens) as f64;
        let _ = apply_coding_plan_delta(db, platform_id, 1, total).await;
    } else if let Some(ref price) = resolved_price {
        // 按量平台扣金额
        let cost = balance_cost(
            input_tokens,
            output_tokens,
            cache_tokens,
            price.input_cost_per_token,
            price.output_cost_per_token,
            price.cache_read_input_token_cost,
        ) * peak_mult;
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
                ) * peak_mult
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
    if let Ok((last_real, count)) = read_estimate_state(db, platform_id).await
        && should_calibrate(now(), last_real, count)
    {
        run_calibration(db, platform_id, base_url, api_key, is_coding_plan).await;
    }
}

#[cfg(test)]
#[path = "test_db_ops.rs"]
mod test_db_ops;
