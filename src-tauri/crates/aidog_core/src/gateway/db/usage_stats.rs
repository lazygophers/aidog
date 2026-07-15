use super::*;
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};

/// stats_agg_hourly 累计聚合列块（列序固定，row.get 依赖此序）：
/// SUM(request_count), SUM(success_count), SUM(input/output/cache tokens), SUM(est_cost)。
/// get_platform_usage_stats / get_group_usage_stats / get_all_group_usage_stats /
/// platform_usage_stats_all 4 处逐字共用。
const AGG_TOTAL_COLS: &str = "COALESCE(SUM(request_count), 0), \
     COALESCE(SUM(success_count), 0), \
     COALESCE(SUM(sum_input_tokens), 0), COALESCE(SUM(sum_output_tokens), 0), COALESCE(SUM(sum_cache_tokens), 0), \
     COALESCE(SUM(sum_est_cost), 0.0)";

/// 单平台最近 5 条请求健康度（recent_total / recent_failures），仍裸查 proxy_log：
/// 聚合表 stats_agg_hourly 丢失请求级顺序无法重建近 5 条。LIMIT 5 走索引、便宜。
///
/// 过滤须与 stats_agg_hourly 的 eff_pid 归属一致（对齐改造前 where_clause 回溯语义）：
/// 直挂日志 `platform_id = ?1`；自动分组日志 `platform_id = 0` 经 group.auto_from_platform
/// （十进制字符串）回溯，按 group.group_key 匹配 proxy_log.group_key（gk_<hex>，非显示名 g.name；
/// 见 migration 024 / group-name-group-key-split）。
///
/// proxy-log-db-split s3：`"group"` 表在主库，`proxy_log` 在 proxy_log.db。`auto_keys` 由调用方
/// 在主库闭包内预查后传入（跨库禁在一个连接内同时查两库）。
fn recent_health_single(conn: &Connection, platform_id: u64, auto_keys: &[String]) -> (i64, i64) {
    let pid = platform_id as i64;
    // proxy_log 裸单表查近 5 条：platform_id 直挂 OR (platform_id=0 AND group_key IN(动态列表))。
    // 动态构造 ?2,?3,... 占位（无子查询）；auto_keys 为空时退化为纯 platform_id 匹配。
    let mut binds: Vec<&dyn rusqlite::types::ToSql> = vec![&pid];
    let group_clause = if auto_keys.is_empty() {
        String::new()
    } else {
        let placeholders: Vec<String> = auto_keys
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect();
        for k in auto_keys {
            binds.push(k);
        }
        format!(" OR (platform_id = 0 AND group_key IN ({}))", placeholders.join(", "))
    };
    let where_clause =
        format!("deleted_at = 0 AND (platform_id = ?1{group_clause})");
    conn.query_row(
        &format!("SELECT COUNT(*), SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END) \
         FROM (SELECT status_code FROM proxy_log WHERE {where_clause} ORDER BY created_at DESC LIMIT 5)"),
        binds.as_slice(),
        |row| Ok((row.get(1).unwrap_or(0), row.get(0).unwrap_or(0))),
    )
    .unwrap_or((0, 0))
}

#[track_caller]
pub fn get_platform_usage_stats(db: &Db, platform_id: u64) -> impl std::future::Future<Output = Result<crate::gateway::models::PlatformUsageStats, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let today_key = local_today_hour_key();
    // proxy-log-db-split s3：`"group"` 表在主库，stats_agg_hourly / proxy_log 在 proxy_log.db。
    // 先主库预查该 platform 作为 auto_from_platform 源的 group_key 列表（recent_health 回溯用）。
    let pid_str = platform_id.to_string();
    let auto_keys: Vec<String> = db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT group_key FROM \"group\" WHERE auto_from_platform = ?1 AND deleted_at = 0",
            )?;
            let rows = stmt.query_map(params![pid_str], |r| r.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })
        .await
        .map_err(|e| format!("platform usage stats load auto_keys: {e}"))?;
    // proxy_log 闭包：stats_agg_hourly 累计/今日 + proxy_log 近 5 条健康度。
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            // 累计/今日从聚合表查。stats_agg_hourly.platform_id 写入时已按 group.auto_from_platform
            // 回溯（upsert_stats_agg），故直接 `platform_id = ?1`，无需 proxy_log 那套子查询回溯。
            let pid = platform_id as i64;
            let mut stmt = conn.prepare_cached(
                &format!("SELECT {AGG_TOTAL_COLS}, \
                 COALESCE(SUM(CASE WHEN time_hour >= ?2 THEN sum_input_tokens + sum_output_tokens ELSE 0 END), 0), \
                 COALESCE(SUM(CASE WHEN time_hour >= ?2 THEN sum_est_cost ELSE 0.0 END), 0.0) \
                 FROM stats_agg_hourly WHERE platform_id = ?1 AND deleted_at = 0"),
            )?;
            let mut stats = stmt.query_row(params![pid, today_key], |row| {
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
            })?;
            // 最近 5 条健康度：聚合表无法重建，裸查 proxy_log（LIMIT 5 走索引）。
            let (recent_failures, recent_total) = recent_health_single(conn, platform_id, &auto_keys);
            stats.recent_failures = recent_failures;
            stats.recent_total = recent_total;
            Ok(stats)
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
    // proxy_log 在 proxy_log.db（proxy-log-db-split s3），走专用读池。
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let pid = platform_id as i64;
            let mut stmt = conn.prepare_cached(
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
                    // 正文截断 ~4000 字符，供前端 JSON 解析结构化展示（成功/失败均带）。
                    let body_full: String = response_body.chars().take(4000).collect();
                    Ok(Some(crate::gateway::models::LastTestResult {
                        success,
                        status_code,
                        duration_ms,
                        created_at,
                        error,
                        response_body: body_full,
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
    // stats_agg_hourly 在 proxy_log.db（proxy-log-db-split s3），走专用读池。
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            // 从聚合表查单组累计 + 今日。recent_failures/recent_total 聚合表无法重建（需逐请求近 5 条），
            // 置 0（Groups 页不渲染该健康点；与批量版 get_all_group_usage_stats 一致）。
            let mut stmt = conn.prepare_cached(
                &format!("SELECT {AGG_TOTAL_COLS}, \
                 COALESCE(SUM(CASE WHEN time_hour >= ?2 THEN sum_input_tokens + sum_output_tokens ELSE 0 END), 0), \
                 COALESCE(SUM(CASE WHEN time_hour >= ?2 THEN sum_est_cost ELSE 0.0 END), 0.0) \
                 FROM stats_agg_hourly WHERE group_key = ?1 AND deleted_at = 0"),
            )?;
            let stats = stmt.query_row(
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
    // stats_agg_hourly 在 proxy_log.db（proxy-log-db-split s3），走专用读池。
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare_cached(
                &format!("SELECT group_key, {AGG_TOTAL_COLS} \
                 FROM stats_agg_hourly WHERE deleted_at = 0 AND group_key <> '' \
                 GROUP BY group_key"),
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

/// 批量：单查 `GROUP BY platform_id` 返回所有平台 → 聚合 map（platform_id → stats）。
/// 替代前端逐平台调 `get_platform_usage_stats`（N 次往返 / 2N 次 SQL → 1 次往返 / 2 次 SQL）。
///
/// 累计/今日聚合数据源 = `stats_agg_hourly`，其 `platform_id` 列写入时已按
/// `group.auto_from_platform` 回溯（upsert_stats_agg），即已是 eff_pid——故直接
/// `GROUP BY platform_id`，无需 proxy_log 那套子查询回溯。回溯不到的自动分组日志
/// 归 platform_id=0（写入时即落 0），此处与改造前一致跳过（不归任何平台卡片）。
///
/// recent_total/recent_failures 仍按每平台最近 5 条（created_at DESC）裸查 proxy_log：
/// 聚合表丢请求级顺序无法重建近 5 条。窗口函数 ROW_NUMBER 单查取每 eff_pid 末 5 条，
/// 避免逐平台 5 行子查询往返。eff_pid 派生子查询保留（proxy_log.platform_id=0 回溯）。
/// cache_rate 按 inp/cache 算。
#[track_caller]
pub fn platform_usage_stats_all(
    db: &Db,
) -> impl std::future::Future<Output = Result<std::collections::HashMap<u64, crate::gateway::models::PlatformUsageStats>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let today_key = local_today_hour_key();
    // proxy-log-db-split s3：`"group"` 表在主库，stats_agg_hourly / proxy_log 在 proxy_log.db。
    // 先主库预查 auto_map（recent 的 eff_pid 内存回溯需要），再入 proxy_log 闭包跑两阶段聚合。
    let auto_map = db
        .call_read_traced(None, __db_caller, |conn| load_auto_from_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into())))
        .await
        .map_err(|e| format!("all platform usage stats load auto_map: {e}"))?;
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            // ① 全量聚合（每 platform_id 的 total/success/tokens/cost + 今日 tokens/cost），
            // 直接从 stats_agg_hourly GROUP BY platform_id（已是 eff_pid，无需回溯）。
            let mut stmt = conn.prepare_cached(
                &format!("SELECT platform_id, {AGG_TOTAL_COLS}, \
                 COALESCE(SUM(CASE WHEN time_hour >= ?1 THEN sum_input_tokens + sum_output_tokens ELSE 0 END), 0), \
                 COALESCE(SUM(CASE WHEN time_hour >= ?1 THEN sum_est_cost ELSE 0.0 END), 0.0) \
                 FROM stats_agg_hourly WHERE deleted_at = 0 \
                 GROUP BY platform_id"),
            )?;
            let rows = stmt.query_map(params![today_key], |row| {
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
                    continue; // platform_id=0 = 未知平台（回溯不到），不归任何平台卡片
                }
                map.insert(eff_pid as u64, stats);
            }

            // ② 每平台最近 5 条健康度（recent_total/recent_failures）仍裸查 proxy_log：
            // 聚合表无法重建请求级顺序。去 eff_pid 标量子查询/窗口函数：单表取
            // (platform_id, group_key, status_code) 按 created_at DESC，内存逐行回溯 eff_pid，
            // 每 eff_pid 取前 5 条（已按时间降序），统计 total/failures（与旧 ROW_NUMBER rn<=5 等价）。
            let mut recent_stmt = conn.prepare(
                "SELECT platform_id, group_key, status_code FROM proxy_log \
                 WHERE deleted_at = 0 ORDER BY created_at DESC",
            )?;
            // eff_pid → (取到的近 5 条计数, 其中失败数)
            let mut recent: std::collections::HashMap<i64, (i64, i64)> =
                std::collections::HashMap::new();
            let rows_iter = recent_stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,    // platform_id
                    row.get::<_, String>(1)?, // group_key
                    row.get::<_, i64>(2)?,    // status_code
                ))
            })?;
            for r in rows_iter {
                let (platform_id, group_key, status_code) = r?;
                let eff_pid = resolve_eff_pid(platform_id, &group_key, &auto_map);
                if eff_pid <= 0 {
                    continue;
                }
                let entry = recent.entry(eff_pid).or_insert((0, 0));
                if entry.0 >= 5 {
                    continue; // 该 eff_pid 已收满近 5 条（行已按 created_at DESC）
                }
                entry.0 += 1;
                if !(200..300).contains(&(status_code as i32)) {
                    entry.1 += 1;
                }
            }
            for (eff_pid, (recent_total, recent_failures)) in recent {
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
    // stats_agg_hourly 在 proxy_log.db（proxy-log-db-split s3），走专用读池。
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
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
    // stats_agg_hourly 在 proxy_log.db（proxy-log-db-split s3），走专用读池。
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let pid = platform_id as i64;
            Ok(hourly_rate_inner(conn, now_ms, &window_key, "platform_id = ?2", &[&pid])?)
        })
        .await
        .map_err(|e| format!("platform hourly rate: {e}"))
    }
}

