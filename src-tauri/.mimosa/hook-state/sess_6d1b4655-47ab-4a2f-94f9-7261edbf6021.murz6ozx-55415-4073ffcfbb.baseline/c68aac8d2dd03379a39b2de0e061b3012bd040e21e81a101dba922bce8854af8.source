//! 成本预算闸门的窗口花费聚合（票 06）。
//!
//! 数据源是 `stats_agg_hourly` 而不是 `proxy_log`，三个理由：
//! ① 它是预聚合表（一个自然月最多几百到几千行，proxy_log 是每请求一行），
//!    走 `idx_stats_agg_time` 的 `time_hour >= ?` 范围扫，绝不全表扫；
//! ② 它**无条件写入**（不受 proxy 日志三级开关影响），关日志后预算仍然准；
//! ③ 它不被 `retention_days`（默认 90 天）删行，proxy_log 会被删——预算不能依赖日志保留期。
//!
//! 窗口 = 自然月（本地时区），与 `time_hour` 的本地小时桶同口径。

use aidog_db::Db;
use aidog_db::models::AppliesTo;
use rusqlite::types::ToSql;

/// 本地时区当前自然月起点，格式与 `stats_agg_hourly.time_hour` 一致（本地小时桶）。
pub fn local_month_start_key() -> String {
    chrono::Local::now().format("%Y-%m-01 00:00:00").to_string()
}

/// 构造 `IN (?n, ?n+1, ...)` 占位串；空列表返回 None（调用方不加该谓词）。
fn in_clause(col: &str, len: usize, next_idx: &mut usize) -> Option<String> {
    if len == 0 {
        return None;
    }
    let ph: Vec<String> = (0..len)
        .map(|_| {
            let s = format!("?{next_idx}");
            *next_idx += 1;
            s
        })
        .collect();
    Some(format!(" AND {col} IN ({})", ph.join(", ")))
}

/// 构造聚合 SQL（`?1` = window_start，三维过滤占位从 `?2` 起顺次编号）。
///
/// 单独抽出来是为了让索引红线测试 EXPLAIN **生产查询本身**，而不是一份手抄的副本
/// （评审 F8：抄一份的话，这里改成全表扫测试照样绿）。
pub(crate) fn spend_sql(n_platforms: usize, n_groups: usize, n_models: usize) -> String {
    let mut idx = 2usize;
    let p_clause = in_clause("platform_id", n_platforms, &mut idx).unwrap_or_default();
    let g_clause = in_clause("group_key", n_groups, &mut idx).unwrap_or_default();
    let m_clause = in_clause("model", n_models, &mut idx).unwrap_or_default();
    format!(
        "SELECT COALESCE(SUM(sum_est_cost), 0.0) FROM stats_agg_hourly \
         WHERE deleted_at = 0 AND time_hour >= ?1{p_clause}{g_clause}{m_clause}"
    )
}

/// 指定窗口起点起、按 `applies_to` 三维过滤后的累计花费（美元）。
///
/// 三维语义与 `CompiledRule::applies` 对称：各维空 = 不限，多值 = 命中任一，维间 AND。
/// 注意 model 维匹配的是 `stats_agg_hourly.model`（写入时取 actual_model，为空才回落
/// 请求模型名）——即**上游实际模型名**。这是预算功能 model 维的唯一口径，规则选中侧
/// （`aidog_middleware::budget`）与前端提示都按它写；配了模型重映射又要按模型限额时，
/// 规则的作用范围必须同时限定平台，否则路由前的挂载点只拿得到客户端请求名。
#[track_caller]
pub fn window_spend<'a>(
    db: &'a Db,
    applies_to: &AppliesTo,
    window_start: String,
) -> impl std::future::Future<Output = Result<f64, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    let platforms = applies_to.platforms.clone();
    let groups = applies_to.groups.clone();
    let models = applies_to.models.clone();
    async move {
        db.call_read_traced(None, __db_caller, move |conn| {
            let sql = spend_sql(platforms.len(), groups.len(), models.len());
            let mut binds: Vec<&dyn ToSql> = vec![&window_start];
            binds.extend(platforms.iter().map(|p| p as &dyn ToSql));
            binds.extend(groups.iter().map(|g| g as &dyn ToSql));
            binds.extend(models.iter().map(|m| m as &dyn ToSql));
            let spent: f64 = conn.query_row(&sql, binds.as_slice(), |r| r.get(0))?;
            Ok(spent)
        })
        .await
        .map_err(|e| format!("budget window spend: {e}"))
    }
}

/// 本自然月累计花费（`window_spend` + `local_month_start_key` 的便捷封装）。
#[track_caller]
pub fn month_spend<'a>(
    db: &'a Db,
    applies_to: &AppliesTo,
) -> impl std::future::Future<Output = Result<f64, String>> + 'a {
    window_spend(db, applies_to, local_month_start_key())
}

#[cfg(test)]
#[path = "test_budget.rs"]
mod test_budget;
