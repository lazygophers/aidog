use super::*;
use rusqlite::{params, Result as SqlResult};

/// 今日统计摘要（供托盘预览使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodayStats {
    /// 今日总 token 数（input + output）
    pub tokens: i64,
    /// 今日 input token 数
    pub input_tokens: i64,
    /// 今日 output token 数
    pub output_tokens: i64,
    /// 今日 cache token 数
    pub cache_tokens: i64,
    /// 今日 cache 命中率（cache_tokens / input_tokens * 100）
    pub cache_rate: f64,
    /// 今日预估花费（$），基于 model_price 定价
    pub cost: f64,
    /// 今日总请求数
    pub total_requests: i64,
}

/// 本地「今日 00:00」对应的小时桶文本键 "YYYY-MM-DD 00:00:00"，用于与 stats_agg_hourly.time_hour 做
/// 字典序 >= 比较（time_hour 已是本地时区桶，文本可比）。
pub(crate) fn local_today_hour_key() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d 00:00:00").to_string()
}

/// 获取今日统计（本地时区 00:00 起，从聚合表 stats_agg_hourly 查）。
#[track_caller]
pub fn today_stats(db: &Db) -> impl std::future::Future<Output = Result<TodayStats, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let today_key = local_today_hour_key();

    db
        .call_read_traced(None, __db_caller, move |conn| {
            // 基础统计（从聚合表：request_count 即请求数，sum_* 即各 token，sum_est_cost 即花费）。
            let (input_tokens, output_tokens, cache_tokens, total_requests, cost): (i64, i64, i64, i64, f64) = conn
                .query_row(
                    "SELECT COALESCE(SUM(sum_input_tokens), 0), \
                     COALESCE(SUM(sum_output_tokens), 0), \
                     COALESCE(SUM(sum_cache_tokens), 0), \
                     COALESCE(SUM(request_count), 0), \
                     COALESCE(SUM(sum_est_cost), 0.0) \
                     FROM stats_agg_hourly WHERE time_hour >= ?1 AND deleted_at = 0",
                    params![today_key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                )?;

            let tokens = input_tokens + output_tokens;
            let cache_rate = if input_tokens + cache_tokens > 0 {
                cache_tokens as f64 / (input_tokens + cache_tokens) as f64 * 100.0
            } else {
                0.0
            };

            Ok(TodayStats {
                tokens,
                input_tokens,
                output_tokens,
                cache_tokens,
                cache_rate,
                cost,
                total_requests,
            })
        })
        .await
        .map_err(|e| format!("today stats: {e}"))
    }
}

/// 单平台当日使用统计（供 popover「各平台当日」展示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodayPlatformStat {
    /// 归属平台 id（platform_id=0 自动分组日志已回溯到源平台）。
    pub platform_id: u64,
    /// 平台名（回溯失败 / 平台已删则为空，前端归「未知平台」）。
    pub platform_name: String,
    /// 当日 token 总量（input + output）。
    pub tokens: i64,
    /// 当日预估花费（$）。
    pub cost: f64,
    /// 当日请求数。
    pub requests: i64,
}

/// 各平台当日使用（本地时区 00:00 起，未删除日志），只返回有用量（已用）的平台。
///
/// platform_id=0 的自动分组日志经 `group.auto_from_platform` 回溯到源平台后归并，
/// 回溯不到（auto 分组已删 / 非 auto 分组的 platform_id=0）则归 platform_id=0（前端显「未知平台」）。
/// 平台名 JOIN platform 表（含已软删平台，名仍可显示；查不到则空字符串）。
#[track_caller]
pub fn today_platform_stats(db: &Db) -> impl std::future::Future<Output = Result<Vec<TodayPlatformStat>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let today_key = local_today_hour_key();

    db
        .call_read_traced(None, __db_caller, move |conn| {
            // stats_agg_hourly.platform_id 已是 eff_pid（回溯后源平台 id），直接 GROUP BY 即可，
            // 无需再跑 auto 回溯子查询。GROUP BY 天然只含当日有用量的平台。
            let sql = "
                SELECT platform_id AS eff_pid,
                       COALESCE(SUM(sum_input_tokens + sum_output_tokens), 0) AS tokens,
                       COALESCE(SUM(sum_est_cost), 0.0) AS cost,
                       COALESCE(SUM(request_count), 0) AS reqs
                FROM stats_agg_hourly
                WHERE time_hour >= ?1 AND deleted_at = 0
                GROUP BY platform_id
                ORDER BY cost DESC, tokens DESC";
            let mut stmt = conn.prepare_cached(sql)?;
            let rows = stmt
                .query_map(params![today_key], |row| {
                    let pid: i64 = row.get(0)?;
                    Ok((pid, row.get::<_, i64>(1)?, row.get::<_, f64>(2)?, row.get::<_, i64>(3)?))
                })?
                .collect::<SqlResult<Vec<_>>>()?;

            // 平台名映射（含软删平台，名仍可显示）。
            let mut name_stmt = conn.prepare_cached("SELECT id, name FROM platform")?;
            let names: std::collections::HashMap<i64, String> = name_stmt
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
                .collect::<SqlResult<Vec<_>>>()?
                .into_iter()
                .collect();

            Ok(rows
                .into_iter()
                .map(|(pid, tokens, cost, reqs)| TodayPlatformStat {
                    platform_id: pid.max(0) as u64,
                    platform_name: names.get(&pid).cloned().unwrap_or_default(),
                    tokens,
                    cost,
                    requests: reqs,
                })
                .collect())
        })
        .await
        .map_err(|e| format!("today platform stats: {e}"))
    }
}

// ─── Popover Config (settings: scope="popover", key="config") ─

/// 读取 PopoverConfig。无配置 / 损坏 → 默认配置（不持久化，按需懒生成）。
pub async fn get_popover_config(db: &Db) -> Result<crate::gateway::models::PopoverConfig, String> {
    if let Some(v) = get_setting(db, "popover", "config").await? {
        if !v.is_null() {
            let cfg: crate::gateway::models::PopoverConfig =
                serde_json::from_value(v).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "popover config JSON is corrupt, falling back to default");
                    crate::gateway::models::PopoverConfig::default()
                });
            return Ok(cfg);
        }
    }
    Ok(crate::gateway::models::PopoverConfig::default())
}

/// 写入 PopoverConfig 到 settings。
pub async fn set_popover_config(db: &Db, cfg: &crate::gateway::models::PopoverConfig) -> Result<(), String> {
    let value = serde_json::to_value(cfg).map_err(|e| format!("serialize popover config: {e}"))?;
    set_setting(db, SetSettingInput {
        scope: "popover".to_string(),
        key: "config".to_string(),
        value,
    })
    .await
}

/// 根据 model_price 定价计算单次请求预估花费（$），含 peak_hours 倍率调整。
///
/// 复用 `resolve_price` 的回退链（pricing[platform_type] > top_level >
/// default_platform > fallback 默认价），与 preview 命令 `model_price_resolve` 行为一致：
/// 无模型价 / 价为 0 时回退到 `PriceSyncSettings` 的 fallback 默认价（默认 3.0 $/M），不再返回 0。
///
/// peak_hours（高峰/低峰倍率）混合源（PRD 决策 B）：
/// 1. `platform.extra.peak_hours`（用户覆盖，非空 → 用之）
/// 2. `default_peak_hours(platform_type)`（bundled preset 默认）
/// 3. 1.0（无调整）
///
/// 倍率 × base cost 落 `est_cost`（无新列；审计凭 time + platform_id 可重建窗口命中）。
///
/// 锁安全：本函数不持有 `db.0.lock()`；`get_sync_settings` / `resolve_price`
/// （内部 `get_model_price`）/ `get_platform` 各自获取并释放 db 锁，不会重入死锁。
///
/// `platform_type` 传入平台主类型的 serde 裸名（如 `"deepseek"`）以启用 pricing override；
/// 传 `""` 时 override 不命中，但回退链仍保证非 0。`platform_id`=0（自动分组日志无源平台）
/// / `created_at_ms`=0（缺失）时 peak_hours 不生效（multiplier=1.0）。
#[allow(clippy::too_many_arguments)]
pub async fn calc_est_cost(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    platform_id: i64,
    created_at_ms: i64,
) -> f64 {
    let settings = crate::gateway::price_sync::get_sync_settings(db).await;
    let rp = resolve_price(
        db,
        model_name,
        platform_type,
        settings.fallback_input_price,
        settings.fallback_output_price,
        input_tokens as i64,
    )
    .await
    .unwrap_or_else(|_| crate::gateway::models::ResolvedPrice {
        // 安全默认：直接用 fallback 默认价（$/M → $/token），保证非 0、不 panic
        input_cost_per_token: settings.fallback_input_price / 1_000_000.0,
        output_cost_per_token: settings.fallback_output_price / 1_000_000.0,
        cache_read_input_token_cost: 0.0,
        source: "fallback".to_string(),
    });

    let base = input_tokens as f64 * rp.input_cost_per_token
        + output_tokens as f64 * rp.output_cost_per_token
        + cache_tokens as f64 * rp.cache_read_input_token_cost;

    // peak_hours 倍率：仅当有真实平台 + 时间戳才查（mock / 隧道 / 缺失上下文 → 1.0）。
    if platform_id <= 0 || created_at_ms <= 0 {
        return base;
    }
    let windows = platform_peak_hours(db, platform_id, platform_type).await;
    base * crate::gateway::peak_hours::resolve_multiplier(&windows, created_at_ms)
}

/// 取某平台的 peak_hours 窗口：用户 `extra.peak_hours` 覆盖优先；空/缺 → bundled preset 默认。
/// 失败安全：拿不到平台（已删 / 不存在）→ preset 默认；preset 缺 → 空（caller multiplier=1.0）。
async fn platform_peak_hours(db: &Db, platform_id: i64, platform_type: &str) -> Vec<crate::gateway::peak_hours::PeakWindow> {
    if let Ok(Some(p)) = get_platform(db, platform_id as u64).await {
        let user = crate::gateway::peak_hours::parse_platform_peak_hours(&p.extra);
        if !user.is_empty() {
            return user;
        }
    }
    crate::gateway::peak_hours::default_peak_hours(platform_type)
}

// ─── Group CRUD ────────────────────────────────────────────

