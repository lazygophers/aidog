//! 散点直方图（chart-engine D2 / #33）：proxy_log 逐行 `(duration_ms, est_cost)` 的
//! 服务端 bin 化，输出 `(duration_bin × cost_bin)` 计数矩阵，前端 ScatterChart 直接渲染。
//!
//! 独立 command（非 stats_query 扩展）：散点要的是逐请求联合分布，
//! stats_agg_hourly 只有桶内 SUM/AVG，不保留联合分布，故必须扫 proxy_log 原始行。

use aidog_db::models::*;
use aidog_db::{Db, coding_plan_id_set, load_auto_from_map, resolve_eff_pid};
use std::collections::{HashMap, HashSet};

/// duration 目标 bin 数（实际条数按 nice 步长对齐后允许略多一格）。
pub const DURATION_BIN_TARGET: usize = 24;
/// est_cost 目标 bin 数。
pub const COST_BIN_TARGET: usize = 16;

#[track_caller]
pub fn scatter_histogram<'a>(
    db: &'a Db,
    query: &'a ScatterHistogramQuery,
) -> impl std::future::Future<Output = Result<ScatterHistogram, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let query = query.clone();
        // 跨库预查（同 query_stats minute 路径）：auto_map / coding_ids 在 platform 库，
        // proxy_log 在 log.db，读闭包内不能现取。
        let auto_map = db
            .call_read_platform_traced(None, __db_caller, |conn| {
                load_auto_from_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into()))
            })
            .await
            .map_err(|e| format!("scatter_histogram load auto_map: {e}"))?;
        let coding_ids = coding_id_filter(db, &query.filter_coding_plan, __db_caller).await?;
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            scatter_hist_inner(conn, &query, &auto_map, &coding_ids)
                .map_err(|e| tokio_rusqlite::Error::Other(e.into()))
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// filter_coding_plan == Some(true) 时从 platform 库预查 coding plan 平台 id 集。
/// None = 不过滤。（同 query_stats 的懒查模式。）
async fn coding_id_filter(
    db: &Db,
    want: &Option<bool>,
    caller: &'static std::panic::Location<'static>,
) -> Result<Option<HashSet<i64>>, String> {
    if *want != Some(true) {
        return Ok(None);
    }
    db.call_read_platform_traced(None, caller, |conn| {
        coding_plan_id_set(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into()))
    })
    .await
    .map(Some)
    .map_err(|e| format!("scatter_histogram load coding_ids: {e}"))
}

/// log.db 读闭包内的实际逻辑：SQL 下推 time/group/model 过滤，eff_pid 过滤搬内存
/// （auto 分组 platform_id=0 回溯，同 minute 路径），随后纯内存 bin 化。
/// ponytail: 全窗 proxy_log 内存扫描（与 minute 路径同模式），90d retention 下量级
/// ~1e5 行可接受；日志量到 1e7 再考虑 SQL 两遍扫（min/max 预查 + 计数）。
fn scatter_hist_inner(
    conn: &rusqlite::Connection,
    query: &ScatterHistogramQuery,
    auto_map: &HashMap<String, i64>,
    coding_ids: &Option<HashSet<i64>>,
) -> Result<ScatterHistogram, String> {
    let end = query
        .end
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let start = query
        .start
        .unwrap_or_else(|| (chrono::Utc::now() - chrono::Duration::days(7)).timestamp_millis());

    let mut where_parts = vec![
        "deleted_at = 0".to_string(),
        "created_at >= ?1".to_string(),
        "created_at <= ?2".to_string(),
    ];
    if query.filter_group.is_some() {
        where_parts.push("group_key = ?3".to_string());
    }
    if query.filter_model.is_some() {
        let idx = 3 + query.filter_group.is_some() as usize;
        where_parts.push(format!("(model = ?{idx} OR actual_model = ?{idx})"));
    }
    let where_sql = where_parts.join(" AND ");
    let mut binds: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(start), Box::new(end)];
    if let Some(ref g) = query.filter_group {
        binds.push(Box::new(g.clone()));
    }
    if let Some(ref m) = query.filter_model {
        binds.push(Box::new(m.clone()));
    }
    let refs: Vec<&dyn rusqlite::types::ToSql> = binds.iter().map(|b| b.as_ref()).collect();

    let want_pid: Option<i64> = query
        .filter_platform
        .as_ref()
        .and_then(|s| s.parse::<i64>().ok());
    let coding_set = (query.filter_coding_plan == Some(true))
        .then_some(coding_ids.as_ref())
        .flatten();
    let keep = |eff_pid: i64| {
        want_pid.is_none_or(|w| eff_pid == w)
            && coding_set.is_none_or(|cs| cs.contains(&eff_pid))
    };

    let mut points: Vec<(f64, f64)> = Vec::new();
    {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT COALESCE(duration_ms,0), COALESCE(est_cost,0.0), platform_id, group_key \
                 FROM proxy_log WHERE {where_sql}"
            ))
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query(refs.as_slice())
            .map_err(|e| format!("scatter rows: {e}"))?;
        while let Some(r) = rows.next().map_err(|e| format!("scatter next: {e}"))? {
            let platform_id: i64 = r.get(2).map_err(|e| e.to_string())?;
            let group_key: String = r.get(3).map_err(|e| e.to_string())?;
            let eff_pid = resolve_eff_pid(platform_id, group_key.as_str(), auto_map);
            if keep(eff_pid) {
                let dur: f64 = r.get::<_, i64>(0).map_err(|e| e.to_string())? as f64;
                let cost: f64 = r.get(1).map_err(|e| e.to_string())?;
                points.push((dur, cost));
            }
        }
    }

    Ok(build_histogram(&points, DURATION_BIN_TARGET, COST_BIN_TARGET))
}

/// 1/2/5 量级取整：取 ≥ raw 的最小「好数」（1/2/5 × 10^k）；raw 非正 → 1.0。
pub(crate) fn nice_step(raw: f64) -> f64 {
    if raw.is_finite() && raw > 0.0 {
        let exp = raw.log10().floor() as i32;
        let frac = raw / 10f64.powi(exp);
        let nice = if frac <= 1.0 {
            1.0
        } else if frac <= 2.0 {
            2.0
        } else if frac <= 5.0 {
            5.0
        } else {
            10.0
        };
        nice * 10f64.powi(exp)
    } else {
        1.0
    }
}

/// nice 边界推算：步长 = nice_step((max-min)/target)，起点 = min 向下取整到 step 整数倍，
/// bin 数 = ceil((max-起点)/step)（≥ 1，允许比 target 略多一格盖住 max）。
/// max == min（含全 0）→ 单 bin。
pub(crate) fn bin_edges(min: f64, max: f64, target: usize) -> Vec<f64> {
    let step = nice_step((max - min) / target.max(1) as f64);
    let start = (min / step).floor() * step;
    let span = (max - start).max(0.0);
    let n = ((span / step).ceil() as usize).max(1);
    (0..=n).map(|i| start + i as f64 * step).collect()
}

/// 落点：值 → bin 下标（值 == 上边界归下一 bin，== max 归最后一个 bin）。
/// 负差（fp 误差致 start 略大于值）按 0 处理（负数 as usize 饱和到 0）；越界 clamp 到 n-1。
fn bin_index(v: f64, start: f64, step: f64, n: usize) -> usize {
    let idx = ((v - start) / step) as usize;
    idx.min(n - 1)
}

/// 纯函数：点集 → 直方图。空点集 → 全空数组（空矩阵不报错）。
/// counts[i][j]：i = duration bin 下标（行），j = cost bin 下标（列）。
pub fn build_histogram(
    points: &[(f64, f64)],
    dur_target: usize,
    cost_target: usize,
) -> ScatterHistogram {
    if points.is_empty() {
        return ScatterHistogram {
            duration_bins: Vec::new(),
            cost_bins: Vec::new(),
            counts: Vec::new(),
        };
    }
    let (mut d_min, mut d_max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut c_min, mut c_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(d, c) in points {
        d_min = d_min.min(d);
        d_max = d_max.max(d);
        c_min = c_min.min(c);
        c_max = c_max.max(c);
    }
    let duration_bins = bin_edges(d_min, d_max, dur_target);
    let cost_bins = bin_edges(c_min, c_max, cost_target);
    let (d_n, c_n) = (duration_bins.len() - 1, cost_bins.len() - 1);
    let d_step = duration_bins[1] - duration_bins[0];
    let c_step = cost_bins[1] - cost_bins[0];
    let mut counts = vec![vec![0u64; c_n]; d_n];
    for &(d, c) in points {
        let i = bin_index(d, duration_bins[0], d_step, d_n);
        let j = bin_index(c, cost_bins[0], c_step, c_n);
        counts[i][j] += 1;
    }
    ScatterHistogram {
        duration_bins,
        cost_bins,
        counts,
    }
}
