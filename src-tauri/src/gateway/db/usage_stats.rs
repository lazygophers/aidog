use super::*;
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};

/// 共用使用量聚合：按给定 WHERE 子句统计总量 + 最近 5 次健康度。
/// `where_clause` 不含 `WHERE` 关键字；`params` 须与 `where_clause` 占位符一一对应。
/// 本地当日 00:00 的毫秒 epoch（今日维度聚合的下界）。失败回退 0（=全量当今日，极端容错）。
fn local_today_start_ms() -> i64 {
    use chrono::{Local, TimeZone};
    Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
}

fn usage_stats(
    conn: &Connection,
    where_clause: &str,
    params: &[&dyn rusqlite::types::ToSql],
) -> SqlResult<crate::gateway::models::PlatformUsageStats> {
    // 今日下界绑定到 where_clause 之后的下一个占位符（CASE WHEN 内引用，避免改动调用方 params 顺序）。
    let today_start = local_today_start_ms();
    let mut q_params: Vec<&dyn rusqlite::types::ToSql> = params.to_vec();
    let today_idx = q_params.len() + 1;
    q_params.push(&today_start);
    let stats: crate::gateway::models::PlatformUsageStats = conn.query_row(
        &format!("SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), \
         COALESCE(SUM(est_cost), 0.0), \
         COALESCE(SUM(CASE WHEN created_at >= ?{today_idx} THEN input_tokens + output_tokens ELSE 0 END), 0), \
         COALESCE(SUM(CASE WHEN created_at >= ?{today_idx} THEN est_cost ELSE 0.0 END), 0.0) \
         FROM proxy_log WHERE {where_clause}"),
        q_params.as_slice(),
        |row| {
            let total: i64 = row.get(0).unwrap_or(0);
            let success: i64 = row.get(1).unwrap_or(0);
            let inp: i64 = row.get(2).unwrap_or(0);
            let out: i64 = row.get(3).unwrap_or(0);
            let cache: i64 = row.get(4).unwrap_or(0);
            let cost: f64 = row.get(5).unwrap_or(0.0);
            let today_tokens: i64 = row.get(6).unwrap_or(0);
            let today_cost: f64 = row.get(7).unwrap_or(0.0);
            Ok(crate::gateway::models::PlatformUsageStats {
                total_requests: total,
                success_count: success,
                total_input_tokens: inp,
                total_output_tokens: out,
                total_cache_tokens: cache,
                cache_rate: if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 },
                recent_failures: 0,
                recent_total: 0,
                total_cost: cost,
                today_tokens,
                today_cost,
            })
        },
    )?;

    // Recent 5 requests health
    let (recent_failures, recent_total): (i64, i64) = conn.query_row(
        &format!("SELECT COUNT(*), SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END) \
         FROM (SELECT status_code FROM proxy_log WHERE {where_clause} ORDER BY created_at DESC LIMIT 5)"),
        params,
        |row| Ok((row.get(1).unwrap_or(0), row.get(0).unwrap_or(0))),
    ).unwrap_or((0, 0));

    Ok(crate::gateway::models::PlatformUsageStats {
        recent_failures,
        recent_total,
        ..stats
    })
}

#[track_caller]
pub fn get_platform_usage_stats(db: &Db, platform_id: u64) -> impl std::future::Future<Output = Result<crate::gateway::models::PlatformUsageStats, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            // platform_id 现为整数；自动分组日志可能未带 platform_id（=0），通过 group.auto_from_platform（存十进制字符串）回溯。
            // 回溯按 group.group_key 匹配 proxy_log.group_key（gk_<hex>，非显示名 g.name；见 migration 024 / group-name-group-key-split）。
            let where_clause = "deleted_at = 0 AND (platform_id = ?1 OR (platform_id = 0 AND group_key IN (SELECT group_key FROM \"group\" WHERE auto_from_platform = ?2 AND deleted_at = 0)))";
            let pid = platform_id as i64;
            let pid_str = platform_id.to_string();
            Ok(usage_stats(conn, where_clause, &[&pid, &pid_str])?)
        })
        .await
        .map_err(|e| format!("platform usage stats: {e}"))
    }
}

/// 取某 platform 最近一条 `source_protocol='test'` 的 proxy_log（model_test 落日志时 platform_id 为真实 id，
/// 无需 auto_from_platform 回溯）。返回 None 表示该平台从未测试过。
#[track_caller]
pub fn get_last_test_result(
    db: &Db,
    platform_id: u64,
) -> impl std::future::Future<Output = Result<Option<crate::gateway::models::LastTestResult>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let pid = platform_id as i64;
            let mut stmt = conn.prepare(
                "SELECT status_code, duration_ms, created_at, response_body \
                 FROM proxy_log \
                 WHERE deleted_at = 0 AND platform_id = ?1 AND source_protocol = 'test' \
                 ORDER BY created_at DESC LIMIT 1",
            )?;
            let mut rows = stmt.query_map([&pid], |row| {
                let status_code: i32 = row.get(0).unwrap_or(0);
                let duration_ms: i32 = row.get(1).unwrap_or(0);
                let created_at: i64 = row.get(2).unwrap_or(0);
                let response_body: String = row.get(3).unwrap_or_default();
                Ok((status_code, duration_ms, created_at, response_body))
            })?;
            match rows.next().transpose()? {
                Some((status_code, duration_ms, created_at, response_body)) => {
                    let success = (200..300).contains(&status_code);
                    let error = if success {
                        String::new()
                    } else {
                        response_body.chars().take(200).collect()
                    };
                    Ok(Some(crate::gateway::models::LastTestResult {
                        success,
                        status_code,
                        duration_ms,
                        created_at,
                        error,
                    }))
                }
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| format!("last test result: {e}"))
    }
}

#[track_caller]
pub fn get_group_usage_stats<'a>(db: &'a Db, group_key: &'a str) -> impl std::future::Future<Output = Result<crate::gateway::models::PlatformUsageStats, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let group_key = group_key.to_string();
    let today_key = local_today_hour_key();
    db
        .call_read_traced(None, __db_caller, move |conn| {
            // 从聚合表查单组累计 + 今日。recent_failures/recent_total 聚合表无法重建（需逐请求近 5 条），
            // 置 0（Groups 页不渲染该健康点；与批量版 get_all_group_usage_stats 一致）。
            let stats = conn.query_row(
                "SELECT COALESCE(SUM(request_count), 0), \
                 COALESCE(SUM(success_count), 0), \
                 COALESCE(SUM(sum_input_tokens), 0), COALESCE(SUM(sum_output_tokens), 0), COALESCE(SUM(sum_cache_tokens), 0), \
                 COALESCE(SUM(sum_est_cost), 0.0), \
                 COALESCE(SUM(CASE WHEN time_hour >= ?2 THEN sum_input_tokens + sum_output_tokens ELSE 0 END), 0), \
                 COALESCE(SUM(CASE WHEN time_hour >= ?2 THEN sum_est_cost ELSE 0.0 END), 0.0) \
                 FROM stats_agg_hourly WHERE group_key = ?1 AND deleted_at = 0",
                params![group_key, today_key],
                |row| {
                    let total: i64 = row.get(0)?;
                    let success: i64 = row.get(1)?;
                    let inp: i64 = row.get(2)?;
                    let out: i64 = row.get(3)?;
                    let cache: i64 = row.get(4)?;
                    let cost: f64 = row.get(5)?;
                    let today_tokens: i64 = row.get(6)?;
                    let today_cost: f64 = row.get(7)?;
                    Ok(crate::gateway::models::PlatformUsageStats {
                        total_requests: total,
                        success_count: success,
                        total_input_tokens: inp,
                        total_output_tokens: out,
                        total_cache_tokens: cache,
                        cache_rate: if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 },
                        recent_failures: 0,
                        recent_total: 0,
                        total_cost: cost,
                        today_tokens,
                        today_cost,
                    })
                },
            )?;
            Ok(stats)
        })
        .await
        .map_err(|e| format!("group usage stats: {e}"))
    }
}

/// 批量：单查 `GROUP BY group_key` 返回所有 group → 聚合 map（问题6 N+1 消除）。
/// 替代前端逐 group 调 `get_group_usage_stats`（N 次往返 → 1 次）。
/// `GROUP BY group_key` 天然满足 CLAUDE.md「共享平台不重复计入」：日志按 group_key 归属，
/// 同一平台被多 group 共享时各 group 只统计经本 group 进来的请求，无重复。
/// recent_failures/recent_total/cache_rate 不在批量结果内（Groups 页不渲染，避免每组 5 行子查询）。
#[track_caller]
pub fn get_all_group_usage_stats(
    db: &Db,
) -> impl std::future::Future<Output = Result<std::collections::HashMap<String, crate::gateway::models::PlatformUsageStats>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT group_key, COALESCE(SUM(request_count), 0), \
                 COALESCE(SUM(success_count), 0), \
                 COALESCE(SUM(sum_input_tokens), 0), COALESCE(SUM(sum_output_tokens), 0), COALESCE(SUM(sum_cache_tokens), 0), \
                 COALESCE(SUM(sum_est_cost), 0.0) \
                 FROM stats_agg_hourly WHERE deleted_at = 0 AND group_key <> '' \
                 GROUP BY group_key",
            )?;
            let rows = stmt.query_map([], |row| {
                let group_key: String = row.get(0)?;
                let total: i64 = row.get(1).unwrap_or(0);
                let success: i64 = row.get(2).unwrap_or(0);
                let inp: i64 = row.get(3).unwrap_or(0);
                let out: i64 = row.get(4).unwrap_or(0);
                let cache: i64 = row.get(5).unwrap_or(0);
                let cost: f64 = row.get(6).unwrap_or(0.0);
                Ok((
                    group_key,
                    crate::gateway::models::PlatformUsageStats {
                        total_requests: total,
                        success_count: success,
                        total_input_tokens: inp,
                        total_output_tokens: out,
                        total_cache_tokens: cache,
                        cache_rate: if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 },
                        recent_failures: 0,
                        recent_total: 0,
                        total_cost: cost,
                        today_tokens: 0,
                        today_cost: 0.0,
                    },
                ))
            })?;
            let mut map = std::collections::HashMap::new();
            for r in rows {
                let (name, stats) = r?;
                map.insert(name, stats);
            }
            Ok(map)
        })
        .await
        .map_err(|e| format!("all group usage stats: {e}"))
    }
}

/// 批量：单查 `GROUP BY eff_pid` 返回所有平台 → 聚合 map（platform_id → stats）。
/// 替代前端逐平台调 `get_platform_usage_stats`（N 次往返 / 2N 次 SQL → 1 次往返 / 1 次 SQL）。
///
/// 必须保留 `platform_id = 0` 回溯语义（对齐 `get_platform_usage_stats` / `today_platform_stats`）：
/// 自动分组日志 `platform_id = 0`，通过 `group_key → "group".auto_from_platform`（十进制字符串）
/// 回溯到源平台后归并；回溯不到（auto 分组已删 / 非 auto 分组的 platform_id=0）则归 eff_pid=0。
/// 回溯 join 键为 `g.group_key = proxy_log.group_key`（proxy_log 存 group.group_key=gk_<hex>，
/// 非显示名 g.name；见 migration 024 / 记忆 group-name-group-key-split）。
///
/// recent_total/recent_failures 按每平台最近 5 条（created_at DESC）聚合（语义同
/// `usage_stats` 单平台版），供平台卡片「健康点」按近期成功率配色。窗口函数 ROW_NUMBER
/// 单查取每 eff_pid 末 5 条，避免逐平台 5 行子查询往返。cache_rate 按 inp/cache 算。
#[track_caller]
pub fn platform_usage_stats_all(
    db: &Db,
) -> impl std::future::Future<Output = Result<std::collections::HashMap<u64, crate::gateway::models::PlatformUsageStats>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            // 公共 eff_pid 派生子查询：platform_id=0 经 group.auto_from_platform 回溯到源平台。
            const EFF_PID_SUBQUERY: &str = "\
                SELECT \
                    CASE WHEN platform_id = 0 THEN COALESCE( \
                        (SELECT CAST(g.auto_from_platform AS INTEGER) \
                         FROM \"group\" g \
                         WHERE g.group_key = proxy_log.group_key \
                           AND g.auto_from_platform != '' \
                           AND g.deleted_at = 0 \
                         LIMIT 1), 0) \
                    ELSE platform_id END AS eff_pid, \
                    status_code, input_tokens, output_tokens, cache_tokens, est_cost, created_at \
                FROM proxy_log \
                WHERE deleted_at = 0";

            // ① 全量聚合（每 eff_pid 的 total/success/tokens/cost + 今日 tokens/cost）。
            let today_start = local_today_start_ms();
            let totals_sql = format!(
                "SELECT eff_pid, \
                 COUNT(*), \
                 SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
                 SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), \
                 COALESCE(SUM(est_cost), 0.0), \
                 COALESCE(SUM(CASE WHEN created_at >= ?1 THEN input_tokens + output_tokens ELSE 0 END), 0), \
                 COALESCE(SUM(CASE WHEN created_at >= ?1 THEN est_cost ELSE 0.0 END), 0.0) \
                 FROM ({EFF_PID_SUBQUERY}) \
                 GROUP BY eff_pid"
            );
            let mut stmt = conn.prepare(&totals_sql)?;
            let rows = stmt.query_map([&today_start], |row| {
                let eff_pid: i64 = row.get(0)?;
                let total: i64 = row.get(1).unwrap_or(0);
                let success: i64 = row.get(2).unwrap_or(0);
                let inp: i64 = row.get(3).unwrap_or(0);
                let out: i64 = row.get(4).unwrap_or(0);
                let cache: i64 = row.get(5).unwrap_or(0);
                let cost: f64 = row.get(6).unwrap_or(0.0);
                let today_tokens: i64 = row.get(7).unwrap_or(0);
                let today_cost: f64 = row.get(8).unwrap_or(0.0);
                Ok((
                    eff_pid,
                    crate::gateway::models::PlatformUsageStats {
                        total_requests: total,
                        success_count: success,
                        total_input_tokens: inp,
                        total_output_tokens: out,
                        total_cache_tokens: cache,
                        cache_rate: if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 },
                        recent_failures: 0,
                        recent_total: 0,
                        total_cost: cost,
                        today_tokens,
                        today_cost,
                    },
                ))
            })?;
            let mut map = std::collections::HashMap::new();
            for r in rows {
                let (eff_pid, stats) = r?;
                if eff_pid <= 0 {
                    continue; // eff_pid=0 = 未知平台（回溯不到），不归任何平台卡片
                }
                map.insert(eff_pid as u64, stats);
            }

            // ② 每平台最近 5 条健康度（recent_total/recent_failures），语义同 usage_stats 单平台版。
            // ROW_NUMBER() 按 eff_pid 分区、created_at DESC 排序，取 rn<=5。
            let recent_sql = format!(
                "SELECT eff_pid, \
                 COUNT(*), \
                 SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END) \
                 FROM ( \
                     SELECT eff_pid, status_code, \
                            ROW_NUMBER() OVER (PARTITION BY eff_pid ORDER BY created_at DESC) AS rn \
                     FROM ({EFF_PID_SUBQUERY}) \
                 ) \
                 WHERE rn <= 5 \
                 GROUP BY eff_pid"
            );
            let mut recent_stmt = conn.prepare(&recent_sql)?;
            let recent_rows = recent_stmt.query_map([], |row| {
                let eff_pid: i64 = row.get(0)?;
                let recent_total: i64 = row.get(1).unwrap_or(0);
                let recent_failures: i64 = row.get(2).unwrap_or(0);
                Ok((eff_pid, recent_total, recent_failures))
            })?;
            for r in recent_rows {
                let (eff_pid, recent_total, recent_failures) = r?;
                if eff_pid <= 0 {
                    continue;
                }
                if let Some(stats) = map.get_mut(&(eff_pid as u64)) {
                    stats.recent_total = recent_total;
                    stats.recent_failures = recent_failures;
                }
            }

            Ok(map)
        })
        .await
        .map_err(|e| format!("all platform usage stats: {e}"))
    }
}

/// 动态窗口日速率公共常量。
const RATE_MIN_SPAN_MS: i64 = 5 * 60 * 1000; // 5min
const RATE_MAX_SPAN_MS: i64 = 7 * 24 * 60 * 60 * 1000; // 7d

/// 本地小时桶文本键 "YYYY-MM-DD HH:00:00" 解析回该桶起点的 UTC ms。无法解析时返回 None。
/// 与 `utc_ms_to_local_hour_key` 互逆（同本地时区语义）。
fn local_hour_key_to_utc_ms(key: &str) -> Option<i64> {
    use chrono::{Local, NaiveDateTime, TimeZone};
    // key 形如 "2026-06-21 09:00:00"，分秒恒为 00；用完整 %H:%M:%S 解析（chrono 需分秒占位
    // 才能构成完整 NaiveDateTime，字面 ":00:00" 会解析失败）。
    let naive = NaiveDateTime::parse_from_str(key, "%Y-%m-%d %H:%M:%S").ok()?;
    Local.from_local_datetime(&naive).earliest().map(|dt| dt.timestamp_millis())
}

/// 动态窗口日用量速率核心（同步，锁内调用）。
///
/// 数据源 = `stats_agg_hourly`（聚合表，不受日志开关影响，关日志仍有值）。
/// `window_key` = window_start（now-7d）对应的本地小时桶文本键，`scope_sql` 为附加维度过滤
/// （`group_key = ?` / `platform_id = ?`，agg 表 platform_id 已是回溯后 eff_pid，无需子查询回溯），
/// `scope_params` 从 `?2` 起绑定。span = clamp(now - 最早有花费小时桶起点, 5min, 7d)，
/// `rate_per_hour = SUM(sum_est_cost in span) / span_hours`。无任何用量 → None。
fn hourly_rate_inner(
    conn: &Connection,
    now_ms: i64,
    window_key: &str,
    scope_sql: &str,
    scope_params: &[&dyn rusqlite::types::ToSql],
) -> SqlResult<Option<f64>> {
    let mut binds: Vec<&dyn rusqlite::types::ToSql> = vec![&window_key];
    binds.extend_from_slice(scope_params);
    // 7d 窗口内最早一个有 est_cost(>0) 的小时桶（time_hour 文本桶，字典序 >= 比较）。
    let earliest_sql = format!(
        "SELECT MIN(time_hour) FROM stats_agg_hourly \
         WHERE time_hour >= ?1 AND deleted_at = 0 AND sum_est_cost > 0 AND ({scope_sql})"
    );
    let earliest_key: Option<String> = conn
        .query_row(&earliest_sql, binds.as_slice(), |row| row.get(0))
        .optional()?
        .flatten();
    let earliest_key = match earliest_key {
        Some(k) => k,
        None => return Ok(None), // 无任何用量 → None
    };
    let total_sql = format!(
        "SELECT COALESCE(SUM(sum_est_cost), 0.0) FROM stats_agg_hourly \
         WHERE time_hour >= ?1 AND deleted_at = 0 AND ({scope_sql})"
    );
    let total: f64 = conn.query_row(&total_sql, binds.as_slice(), |row| row.get(0))?;
    if total <= 0.0 {
        return Ok(None);
    }
    // earliest = 最早有花费小时桶的起点 ms；span = clamp(now - earliest, 5min, 7d)。
    // 解析失败兜底为 now（→ span clamp 到 5min 下限），不致 panic。
    let earliest_ms = local_hour_key_to_utc_ms(&earliest_key).unwrap_or(now_ms);
    let span_ms = (now_ms - earliest_ms).clamp(RATE_MIN_SPAN_MS, RATE_MAX_SPAN_MS);
    let span_hours = span_ms as f64 / 3_600_000.0;
    Ok(Some(total / span_hours))
}

/// 分组动态窗口日用量速率（$ / 小时），供 statusline 余额「剩余可用天数」配色。
/// 无任何用量 → None（配色侧视作中性 / 不报警）。短持锁，不跨 await。
#[track_caller]
pub fn get_group_hourly_rate<'a>(db: &'a Db, group_key: &'a str) -> impl std::future::Future<Output = Result<Option<f64>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_key = utc_ms_to_local_hour_key(now_ms - RATE_MAX_SPAN_MS);
    let group_key = group_key.to_string();
    db
        .call_read_traced(None, __db_caller, move |conn| {
            Ok(hourly_rate_inner(conn, now_ms, &window_key, "group_key = ?2", &[&group_key])?)
        })
        .await
        .map_err(|e| format!("group hourly rate: {e}"))
    }
}

/// 单平台动态窗口日用量速率（$ / 小时），供 Platforms 列表页余额按速率配色。
///
/// 数据源 stats_agg_hourly 的 platform_id 列已是回溯后 eff_pid（写入时已按
/// group.auto_from_platform 回溯 platform_id=0 的自动分组日志），故直接 `platform_id = ?`，
/// 无需 proxy_log 那套子查询回溯。无任何用量 → None（前端退中性）。短持锁，不跨 await。
#[track_caller]
pub fn get_platform_hourly_rate(db: &Db, platform_id: u64) -> impl std::future::Future<Output = Result<Option<f64>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_key = utc_ms_to_local_hour_key(now_ms - RATE_MAX_SPAN_MS);
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let pid = platform_id as i64;
            Ok(hourly_rate_inner(conn, now_ms, &window_key, "platform_id = ?2", &[&pid])?)
        })
        .await
        .map_err(|e| format!("platform hourly rate: {e}"))
    }
}

