//! 请求驱动 quota 预估增量更新（降频）
//!
//! 每次 proxy 请求完成（拿到 token）→ 本地预估增量更新平台余额 + coding plan，
//! 降低对上游 quota API 的查询频率，并在 5min / 100 次时触发真实校准覆盖。
//!
//! 关键约束（见 research）：
//!   - `Db.0` 是 `std::sync::Mutex<Connection>`，**禁持锁跨 .await**；
//!     校准里的 `query_quota` 是 async，须 锁外调用，结果回库时再短持锁。
//!   - 余额预估用**单条 SQL 原子自减**避免多请求并发丢更新。
//!   - coding plan 是 JSON 字段无法 SQL 内自增 → read-modify-write 必须在
//!     同一持锁临界区内完成（一次 lock 内 SELECT+UPDATE）。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::db::{now, Db};
use super::quota::PlatformQuota;

/// 校准阈值：距上次真查超过 5min
pub const CALIBRATE_INTERVAL_MS: i64 = 300_000;
/// 校准阈值：自上次真查以来预估次数
pub const CALIBRATE_COUNT: i64 = 100;

// ── 预估 coding plan JSON 模型 ──────────────────────────────

/// 持久化于 `platform.est_coding_plan` 的预估状态
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstCodingPlan {
    #[serde(default)]
    pub tiers: Vec<EstTier>,
    #[serde(default)]
    pub level: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstTier {
    /// "five_hour" | "weekly_limit"
    pub name: String,
    /// 当前预估利用率 (0-100)
    pub est_utilization: f64,
    /// 方案 B 拟合系数：每 token 增加的利用率百分点（冷启动为 0 = 未知）
    #[serde(default)]
    pub coef_per_token: f64,
    /// 上次真查时的利用率（拟合基线）
    #[serde(default)]
    pub util_at_last_real: f64,
    /// 自上次真查以来累计 token（拟合分母）
    #[serde(default)]
    pub tokens_since_real: f64,
    /// 是否有绝对基数（Kimi limit/remaining → 精确预估）
    #[serde(default)]
    pub has_base: bool,
    /// 绝对配额上限（仅 has_base 时有意义）
    #[serde(default)]
    pub limit: f64,
}

impl EstCodingPlan {
    pub fn from_json(s: &str) -> Self {
        if s.trim().is_empty() {
            return Self::default();
        }
        serde_json::from_str(s).unwrap_or_default()
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ── 纯算法（可单测）──────────────────────────────────────────

/// 校准判定：距上次真查 >5min 或 预估次数 >=100
pub fn should_calibrate(now_ms: i64, last_real_query_at: i64, estimate_count: i64) -> bool {
    now_ms - last_real_query_at > CALIBRATE_INTERVAL_MS || estimate_count >= CALIBRATE_COUNT
}

/// 余额预估增量 cost = in×in_cost + out×out_cost + cache×cache_cost
pub fn balance_cost(
    input_tokens: i64,
    output_tokens: i64,
    cache_tokens: i64,
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_token_cost: f64,
) -> f64 {
    input_tokens as f64 * input_cost_per_token
        + output_tokens as f64 * output_cost_per_token
        + cache_tokens as f64 * cache_read_input_token_cost
}

/// 对单个 tier 应用一次请求的 token 增量（read-modify-write 的纯函数部分）。
///   - Kimi（has_base）：精确，每 token 的 % = 100/limit。
///   - GLM/MiniMax（方案 B）：有 coef → est = util_at_last_real + tokens_since_real×coef；
///     冷启动（coef==0）不预估，est 维持真值（util_at_last_real）。
pub fn apply_tier_delta(tier: &mut EstTier, tokens: f64) {
    if tokens <= 0.0 {
        return;
    }
    if tier.has_base && tier.limit > 0.0 {
        // Kimi 精确增量
        tier.est_utilization += tokens * (100.0 / tier.limit);
        if tier.est_utilization > 100.0 {
            tier.est_utilization = 100.0;
        }
        return;
    }
    // 方案 B
    tier.tokens_since_real += tokens;
    if tier.coef_per_token > 0.0 {
        tier.est_utilization = tier.util_at_last_real + tier.tokens_since_real * tier.coef_per_token;
        if tier.est_utilization > 100.0 {
            tier.est_utilization = 100.0;
        }
    }
    // 冷启动（无 coef）：不预估，est_utilization 保持真值不动
}

/// 单 tier 的预期消耗速率分级（statusline 第 3 行动态色用）。
///
/// 语义：以「窗口内利用率随时间的预期推进」判定该档配额耗尽的快慢——
///   - `Fast`：利用率已偏高 / 预期快于配额时间线（接近耗尽）→ 上游标红。
///   - `Normal`：接近时间线 → 标黄。
///   - `Busy`：利用率低 / 慢于时间线（仍宽裕）→ 标绿。
///
/// 数据不足（无窗口起止时间戳持久化）时退化为按当前 `est_utilization` 阈值估算：
///   >= 80 → Fast；>= 40 → Normal；否则 Busy。
///
/// 拿不到利用率（NaN/负）一律 `Normal` 降级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierPace {
    Fast,
    Normal,
    Busy,
}

impl TierPace {
    pub fn as_str(self) -> &'static str {
        match self {
            TierPace::Fast => "fast",
            TierPace::Normal => "normal",
            TierPace::Busy => "busy",
        }
    }
}

/// 由 tier 利用率估算 pace。无可靠窗口时间线时按利用率阈值降级。
pub fn tier_pace(tier: &EstTier) -> TierPace {
    let util = tier.est_utilization;
    if !util.is_finite() || util < 0.0 {
        return TierPace::Normal;
    }
    if util >= 80.0 {
        TierPace::Fast
    } else if util >= 40.0 {
        TierPace::Normal
    } else {
        TierPace::Busy
    }
}

/// 真查校准：用上游真值覆盖某 tier，并（方案 B）尝试拟合 coef。
///   - has_base（Kimi）：直接记 limit + est_utilization = 真值。
///   - 方案 B：拟合 `coef = (util_real - util_at_last_real) / tokens_since_real`，
///     仅当无跨 reset（util_real >= util_at_last_real）且 tokens_since_real > 0；
///     reset（util_real < util_at_last_real）→ 丢弃本窗口样本，coef 保留；
///     最后重置基线：util_at_last_real = util_real，tokens_since_real = 0，est = 真值。
pub fn calibrate_tier(
    prev: &EstTier,
    name: &str,
    util_real: f64,
    has_base: bool,
    limit: Option<f64>,
) -> EstTier {
    if has_base {
        return EstTier {
            name: name.to_string(),
            est_utilization: util_real,
            coef_per_token: 0.0,
            util_at_last_real: util_real,
            tokens_since_real: 0.0,
            has_base: true,
            limit: limit.unwrap_or(0.0),
        };
    }
    // 方案 B 拟合
    let mut coef = prev.coef_per_token;
    let is_reset = util_real < prev.util_at_last_real;
    if !is_reset && prev.tokens_since_real > 0.0 {
        let fitted = (util_real - prev.util_at_last_real) / prev.tokens_since_real;
        if fitted > 0.0 {
            coef = fitted;
        }
    }
    // reset 时丢弃本窗口样本（coef 保留），仅重置基线
    EstTier {
        name: name.to_string(),
        est_utilization: util_real,
        coef_per_token: coef,
        util_at_last_real: util_real,
        tokens_since_real: 0.0,
        has_base: false,
        limit: 0.0,
    }
}

// ── DB 集成 ─────────────────────────────────────────────────

/// 读取平台校准状态（短持锁）
pub async fn read_estimate_state(db: &Db, platform_id: u64) -> Result<(i64, i64), String> {
    db.0
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
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET est_balance_remaining = est_balance_remaining - ?1, estimate_count = estimate_count + 1 WHERE id = ?2",
                params![cost, platform_id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
}

/// coding plan 预估：一次闭包内 SELECT→修改→UPDATE（read-modify-write 串行，避免并发覆盖）。
/// 同时 estimate_count+1。
pub async fn apply_coding_plan_delta(db: &Db, platform_id: u64, tokens: f64) -> Result<(), String> {
    db.0
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
        .map_err(|e| e.to_string())
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
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET est_balance_remaining = ?1, est_coding_plan = ?2, last_real_query_at = ?3, estimate_count = 0 WHERE id = ?4",
                params![est_balance, est_coding_plan_json, now_ms, platform_id as i64],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
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
            calibrate_tier(&prev_tier, &t.name, t.utilization, has_base, t.limit)
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
        .0
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
async fn run_calibration(
    db: &Db,
    platform_id: u64,
    base_url: &str,
    api_key: &str,
    is_coding_plan: bool,
) {
    // 锁外 async 真查（构造 Arc<Db> 供 http_client 读系统代理设置）
    let db_arc = std::sync::Arc::new(db.clone());
    let quota = super::quota::query_quota(Some(&db_arc), base_url, api_key).await;
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
    input_tokens: i64,
    output_tokens: i64,
    cache_tokens: i64,
    is_coding_plan: bool,
) {
    // 1. 增量预估
    if is_coding_plan {
        let total = (input_tokens + output_tokens + cache_tokens) as f64;
        let _ = apply_coding_plan_delta(db, platform_id, total).await;
    } else {
        // 按量平台扣金额
        if let Ok(price) = super::db::resolve_price(db, model, platform_type, 0.0, 0.0).await {
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
    }

    // 1b. 手动预算扣减（独立于上游 quota 的并行机制；无 manual_budgets 则 no-op）。
    //     est_cost 走 resolve_price（与按量平台一致，含默认价回退）；token 扣总 token。
    {
        let total_tokens = (input_tokens + output_tokens + cache_tokens) as f64;
        let est_cost = super::db::resolve_price(db, model, platform_type, 0.0, 0.0)
            .await
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
        let _ = super::manual_budget::apply_manual_budgets(
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
            run_calibration(db, platform_id, base_url, api_key, is_coding_plan).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::models::*;
    use crate::gateway::quota::{CodingPlanInfo, QuotaTier};

    async fn mem_db() -> Db {
        let db = Db::new(":memory:").await.unwrap();
        db.init_tables().await.unwrap();
        db
    }

    async fn mk_platform(db: &Db, coding: bool) -> u64 {
        let p = super::super::db::create_platform(
            db,
            CreatePlatform {
                name: "p".into(),
                platform_type: if coding { Protocol::Kimi } else { Protocol::DeepSeek },
                base_url: "https://example.com".into(),
                api_key: "sk".into(),
                extra: String::new(),
                models: None,
                available_models: None,
                endpoints: None,
                manual_budgets: None,
            },
        )
        .await
        .unwrap();
        p.id
    }

    // ── 余额原子自减 + cost 计算 ──
    #[tokio::test]
    async fn balance_atomic_decrement() {
        let db = mem_db().await;
        let id = mk_platform(&db, false).await;
        // 先设个初始余额
        write_real_quota(&db, id, 100.0, "", now()).await.unwrap();

        let cost = balance_cost(1000, 500, 200, 0.001, 0.002, 0.0005);
        assert!((cost - (1.0 + 1.0 + 0.1)).abs() < 1e-9, "cost = {cost}");

        apply_balance_delta(&db, id, cost).await.unwrap();
        let p = super::super::db::get_platform(&db, id).await.unwrap().unwrap();
        assert!((p.est_balance_remaining - (100.0 - cost)).abs() < 1e-9);
        assert_eq!(p.estimate_count, 1);

        // 再扣一次，验证累加自减 + count
        apply_balance_delta(&db, id, cost).await.unwrap();
        let p2 = super::super::db::get_platform(&db, id).await.unwrap().unwrap();
        assert!((p2.est_balance_remaining - (100.0 - 2.0 * cost)).abs() < 1e-9);
        assert_eq!(p2.estimate_count, 2);
    }

    // ── Kimi 精确增量：每 token % = 100/limit ──
    #[test]
    fn kimi_precise_increment() {
        let mut tier = EstTier {
            name: "five_hour".into(),
            est_utilization: 40.0,
            coef_per_token: 0.0,
            util_at_last_real: 40.0,
            tokens_since_real: 0.0,
            has_base: true,
            limit: 10_000.0,
        };
        apply_tier_delta(&mut tier, 1000.0); // 1000 × (100/10000) = 10%
        assert!((tier.est_utilization - 50.0).abs() < 1e-9, "got {}", tier.est_utilization);
        // clamp 到 100
        apply_tier_delta(&mut tier, 100_000.0);
        assert!((tier.est_utilization - 100.0).abs() < 1e-9);
    }

    // ── 方案 B 拟合后增量 ──
    #[test]
    fn fitted_increment_with_coef() {
        let mut tier = EstTier {
            name: "five_hour".into(),
            est_utilization: 40.0,
            coef_per_token: 0.0001, // 每 token 0.0001%
            util_at_last_real: 40.0,
            tokens_since_real: 0.0,
            has_base: false,
            limit: 0.0,
        };
        apply_tier_delta(&mut tier, 50_000.0); // 40 + 50000×0.0001 = 45
        assert!((tier.est_utilization - 45.0).abs() < 1e-9, "got {}", tier.est_utilization);
        assert!((tier.tokens_since_real - 50_000.0).abs() < 1e-9);
    }

    // ── 冷启动（无 coef）不预估，只累计 tokens ──
    #[test]
    fn cold_start_no_estimate() {
        let mut tier = EstTier {
            name: "five_hour".into(),
            est_utilization: 40.0,
            coef_per_token: 0.0, // 冷启动
            util_at_last_real: 40.0,
            tokens_since_real: 0.0,
            has_base: false,
            limit: 0.0,
        };
        apply_tier_delta(&mut tier, 50_000.0);
        assert!((tier.est_utilization - 40.0).abs() < 1e-9, "冷启动不应预估，got {}", tier.est_utilization);
        // 但 tokens 仍累计，供下次真查拟合
        assert!((tier.tokens_since_real - 50_000.0).abs() < 1e-9);
    }

    // ── 方案 B 真查拟合 coef ──
    #[test]
    fn calibrate_fits_coef() {
        let prev = EstTier {
            name: "five_hour".into(),
            est_utilization: 45.0,
            coef_per_token: 0.0,
            util_at_last_real: 40.0,
            tokens_since_real: 50_000.0,
            has_base: false,
            limit: 0.0,
        };
        // 真查得 util_real = 50% → coef = (50-40)/50000 = 0.0002
        let cal = calibrate_tier(&prev, "five_hour", 50.0, false, None);
        assert!((cal.coef_per_token - 0.0002).abs() < 1e-12, "coef = {}", cal.coef_per_token);
        assert!((cal.est_utilization - 50.0).abs() < 1e-9);
        assert!((cal.util_at_last_real - 50.0).abs() < 1e-9);
        assert_eq!(cal.tokens_since_real, 0.0);
    }

    // ── reset 检测：util_real < util_at_last_real → 丢样本，coef 保留 ──
    #[test]
    fn calibrate_reset_discards_sample() {
        let prev = EstTier {
            name: "five_hour".into(),
            est_utilization: 90.0,
            coef_per_token: 0.0003, // 上一窗口已拟合
            util_at_last_real: 80.0,
            tokens_since_real: 30_000.0,
            has_base: false,
            limit: 0.0,
        };
        // 窗口 reset，真值跌到 5%（< 80）→ 丢弃本窗口样本，coef 保留旧值
        let cal = calibrate_tier(&prev, "five_hour", 5.0, false, None);
        assert!((cal.coef_per_token - 0.0003).abs() < 1e-12, "reset 应保留旧 coef，got {}", cal.coef_per_token);
        assert!((cal.est_utilization - 5.0).abs() < 1e-9);
        assert!((cal.util_at_last_real - 5.0).abs() < 1e-9);
        assert_eq!(cal.tokens_since_real, 0.0);
    }

    // ── Kimi 校准记 limit ──
    #[test]
    fn calibrate_kimi_records_base() {
        let prev = EstTier::default();
        let cal = calibrate_tier(&prev, "five_hour", 30.0, true, Some(20_000.0));
        assert!(cal.has_base);
        assert!((cal.limit - 20_000.0).abs() < 1e-9);
        assert!((cal.est_utilization - 30.0).abs() < 1e-9);
    }

    // ── 校准阈值触发 ──
    #[test]
    fn calibrate_thresholds() {
        let now_ms = 1_000_000_000_000;
        // 时间未到 + 次数未到 → 不校准
        assert!(!should_calibrate(now_ms, now_ms - 100, 50));
        // 时间超 5min → 校准
        assert!(should_calibrate(now_ms, now_ms - 300_001, 0));
        // 次数 >= 100 → 校准
        assert!(should_calibrate(now_ms, now_ms, 100));
        assert!(should_calibrate(now_ms, now_ms, 150));
    }

    // ── coding plan delta read-modify-write 持久化 ──
    #[tokio::test]
    async fn coding_plan_delta_persists() {
        let db = mem_db().await;
        let id = mk_platform(&db, true).await;
        // 初始化一个 Kimi tier（has_base）
        let plan = EstCodingPlan {
            tiers: vec![EstTier {
                name: "five_hour".into(),
                est_utilization: 0.0,
                coef_per_token: 0.0,
                util_at_last_real: 0.0,
                tokens_since_real: 0.0,
                has_base: true,
                limit: 10_000.0,
            }],
            level: None,
        };
        write_real_quota(&db, id, 0.0, &plan.to_json(), now()).await.unwrap();

        apply_coding_plan_delta(&db, id, 1000.0).await.unwrap(); // +10%
        let p = super::super::db::get_platform(&db, id).await.unwrap().unwrap();
        let stored = EstCodingPlan::from_json(&p.est_coding_plan);
        assert!((stored.tiers[0].est_utilization - 10.0).abs() < 1e-9, "got {}", stored.tiers[0].est_utilization);
        assert_eq!(p.estimate_count, 1);
    }

    // ── 校准覆盖重置 count/time + Kimi 基数写入（端到端经 build_calibrated_coding_plan）──
    #[tokio::test]
    async fn calibration_overwrite_resets() {
        let db = mem_db().await;
        let id = mk_platform(&db, true).await;
        // 先制造预估次数
        write_real_quota(&db, id, 0.0, "", 0).await.unwrap();
        apply_balance_delta(&db, id, 1.0).await.unwrap();
        let before = super::super::db::get_platform(&db, id).await.unwrap().unwrap();
        assert_eq!(before.estimate_count, 1);

        // 模拟真查 coding plan（Kimi 带 limit）
        let quota = PlatformQuota {
            success: true,
            error: None,
            queried_at: now(),
            balance: None,
            coding_plan: Some(CodingPlanInfo {
                tiers: vec![QuotaTier {
                    name: "five_hour".into(),
                    utilization: 30.0,
                    resets_at: None,
                    limit: Some(10_000.0),
                    remaining: Some(7_000.0),
                }],
                level: Some("pro".into()),
            }),
            newapi_user_id: None,
        };
        let prev = EstCodingPlan::from_json(&before.est_coding_plan);
        let calibrated = build_calibrated_coding_plan(&prev, &quota);
        write_real_quota(&db, id, 0.0, &calibrated.to_json(), now()).await.unwrap();

        let after = super::super::db::get_platform(&db, id).await.unwrap().unwrap();
        assert_eq!(after.estimate_count, 0, "校准应重置 count");
        assert!(after.last_real_query_at > 0);
        let stored = EstCodingPlan::from_json(&after.est_coding_plan);
        assert!(stored.tiers[0].has_base);
        assert!((stored.tiers[0].limit - 10_000.0).abs() < 1e-9);
        assert!((stored.tiers[0].est_utilization - 30.0).abs() < 1e-9);
    }

    // ── 真查校准入口 calibrate_from_quota：est 严格对齐真实 + 重置基线/计数（coding plan）──
    #[tokio::test]
    async fn calibrate_from_quota_aligns_coding_plan() {
        let db = mem_db().await;
        let id = mk_platform(&db, true).await;
        // 制造预估漂移：先初始化方案 B tier（非 has_base），再累积 token 让 est 偏离真值。
        let drift = EstCodingPlan {
            tiers: vec![EstTier {
                name: "five_hour".into(),
                est_utilization: 88.0, // 预估漂到 88%
                coef_per_token: 0.0001,
                util_at_last_real: 40.0,
                tokens_since_real: 480_000.0,
                has_base: false,
                limit: 0.0,
            }],
            level: None,
        };
        write_real_quota(&db, id, 0.0, &drift.to_json(), 0).await.unwrap();
        apply_balance_delta(&db, id, 1.0).await.unwrap(); // count=1

        // 真查得 util_real=55%（GLM 方案 B，无 limit）
        let quota = PlatformQuota {
            success: true,
            error: None,
            queried_at: now(),
            balance: None,
            coding_plan: Some(CodingPlanInfo {
                tiers: vec![QuotaTier {
                    name: "five_hour".into(),
                    utilization: 55.0,
                    resets_at: None,
                    limit: None,
                    remaining: None,
                }],
                level: Some("max".into()),
            }),
            newapi_user_id: None,
        };
        calibrate_from_quota(&db, id, &quota, true).await;

        let after = super::super::db::get_platform(&db, id).await.unwrap().unwrap();
        assert_eq!(after.estimate_count, 0, "校准重置 count");
        assert!(after.last_real_query_at > 0, "校准记 last_real_query_at");
        let stored = EstCodingPlan::from_json(&after.est_coding_plan);
        let t = &stored.tiers[0];
        // est 严格对齐真实（不被旧漂移 88% 残留）
        assert!((t.est_utilization - 55.0).abs() < 1e-9, "est 应=真实 55，got {}", t.est_utilization);
        assert!((t.util_at_last_real - 55.0).abs() < 1e-9, "基线应=真实");
        assert_eq!(t.tokens_since_real, 0.0, "累积应清零");
        // 拟合 coef = (55-40)/480000
        assert!((t.coef_per_token - (15.0 / 480_000.0)).abs() < 1e-12, "coef = {}", t.coef_per_token);
    }

    // ── calibrate_from_quota：余额平台 est_balance 严格对齐真实 ──
    #[tokio::test]
    async fn calibrate_from_quota_aligns_balance() {
        let db = mem_db().await;
        let id = mk_platform(&db, false).await;
        // 制造漂移：est 余额扣到很低
        write_real_quota(&db, id, 3.5, "", 0).await.unwrap();
        apply_balance_delta(&db, id, 1.0).await.unwrap();

        let quota = PlatformQuota {
            success: true,
            error: None,
            queried_at: now(),
            balance: Some(crate::gateway::quota::BalanceInfo {
                remaining: 99.9,
                total: None,
                used: None,
                currency: "USD".into(),
                is_valid: true,
            }),
            coding_plan: None,
            newapi_user_id: None,
        };
        calibrate_from_quota(&db, id, &quota, false).await;

        let after = super::super::db::get_platform(&db, id).await.unwrap().unwrap();
        assert!((after.est_balance_remaining - 99.9).abs() < 1e-9, "est_balance 应=真实 99.9，got {}", after.est_balance_remaining);
        assert_eq!(after.estimate_count, 0);
        assert!(after.last_real_query_at > 0);
    }

    // ── 真查失败不重置（保留预估）──
    #[tokio::test]
    async fn calibrate_from_quota_failure_preserves() {
        let db = mem_db().await;
        let id = mk_platform(&db, false).await;
        write_real_quota(&db, id, 50.0, "", 12345).await.unwrap();
        apply_balance_delta(&db, id, 1.0).await.unwrap();

        let quota = PlatformQuota {
            success: false,
            error: Some("boom".into()),
            queried_at: now(),
            balance: None,
            coding_plan: None,
            newapi_user_id: None,
        };
        calibrate_from_quota(&db, id, &quota, false).await;

        let after = super::super::db::get_platform(&db, id).await.unwrap().unwrap();
        // 不重置：count 保留、last_real_query_at 保留、est 不变
        assert_eq!(after.estimate_count, 1, "失败不应重置 count");
        assert_eq!(after.last_real_query_at, 12345, "失败不应改 last_real_query_at");
        assert!((after.est_balance_remaining - (50.0 - 1.0)).abs() < 1e-9);
    }
}
