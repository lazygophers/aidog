use super::*;
use rusqlite::{Connection};

struct QueryParams {
    start: i64,
    end: i64,
    filter_group: Option<String>,
    filter_model: Option<String>,
    filter_platform: Option<String>,
}

impl QueryParams {
    fn to_sql_params(&self) -> Vec<Box<dyn rusqlite::types::ToSql>> {
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(self.start),
            Box::new(self.end),
        ];
        if let Some(ref v) = self.filter_group { p.push(Box::new(v.clone())); }
        if let Some(ref v) = self.filter_model { p.push(Box::new(v.clone())); }
        if let Some(ref v) = self.filter_platform { p.push(Box::new(v.clone())); }
        p
    }
}

/// 时间分桶 SQL 表达式（select 列）。粒度决定分桶宽度：
/// - `minute` → 每分钟一桶 `%Y-%m-%d %H:%M`（不带秒：前端 x 轴标注取末 5 字符须为 HH:MM）
/// - `5min`   → 每 5 分钟一桶；strftime 无原生 floor，先把 epoch 秒整除 300 再 *300 向下取整到 5min 边界
/// - `hourly` → 每小时一桶 `%Y-%m-%d %H:00`
/// - 其余（含 `daily`/None）→ 每天一桶 `%Y-%m-%d`
///
/// 时区：必须带 `'localtime'`，使分桶按本地日界/小时切分，与 `today_stats`/`today_platform_stats`
/// 的 `chrono::Local` 00:00 语义一致。缺 `'localtime'` 时 strftime 默认按 UTC 切桶，东八区会把本地
/// 同一天的请求拆到相邻 UTC 日/小时桶（曲线按日错位、跨日双峰），属时区 bug。
pub(crate) fn bucket_time_expr(granularity: Option<&str>) -> String {
    match granularity {
        Some("minute") => {
            "strftime('%Y-%m-%d %H:%M', created_at/1000, 'unixepoch', 'localtime')".to_string()
        }
        // epoch 秒 floor 到 300s 边界后再格式化为分钟（桶 key 形如 "2026-06-16 10:05"）
        Some("5min") => {
            "strftime('%Y-%m-%d %H:%M', (created_at/1000/300)*300, 'unixepoch', 'localtime')"
                .to_string()
        }
        Some("hourly") => {
            "strftime('%Y-%m-%d %H:00', created_at/1000, 'unixepoch', 'localtime')".to_string()
        }
        _ => "strftime('%Y-%m-%d', created_at/1000, 'unixepoch', 'localtime')".to_string(),
    }
}

#[track_caller]
pub fn query_stats<'a>(db: &'a Db, query: &'a StatsQuery) -> impl std::future::Future<Output = Result<StatsResult, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let query = query.clone();
    db
        .call_read_traced(None, __db_caller, move |conn| {
            query_stats_inner(conn, &query)
                .map_err(|e| tokio_rusqlite::Error::Other(e.into()))
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 批量统计查询：一次 IPC + 一次连接借用内串行跑 N 个 `query_stats_inner`，
/// 结果顺序与入参 `queries` 一一对应。供浮窗 N 卡聚合，消除每卡独立 IPC 往返。
///
/// 单卡值与 `query_stats`（逐卡）完全一致：复用同一 `query_stats_inner`，不合并/不丢维度。
#[track_caller]
pub fn query_stats_batch(db: &Db, queries: Vec<StatsQuery>) -> impl std::future::Future<Output = Result<Vec<StatsResult>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut out = Vec::with_capacity(queries.len());
            for q in &queries {
                out.push(
                    query_stats_inner(conn, q)
                        .map_err(|e| tokio_rusqlite::Error::Other(e.into()))?,
                );
            }
            Ok(out)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// UTC ms → 本地小时桶文本键 "YYYY-MM-DD HH:00:00"，用于与 stats_agg_hourly.time_hour 字典序比较。
pub(crate) fn utc_ms_to_local_hour_key(ms: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:00:00").to_string())
        .unwrap_or_default()
}

/// 从聚合表 stats_agg_hourly 跑统计查询（hourly/daily 粒度 + 任意 filter/group_by）。
/// 时间范围按本地小时桶字典序比较；daily 桶 = substr(time_hour,1,10)，hourly 桶 = time_hour。
fn query_stats_inner_agg(
    conn: &Connection,
    query: &StatsQuery,
    start: i64,
    end: i64,
) -> Result<StatsResult, String> {
    // start 向下取整到所属本地小时桶；end 同理（time_hour <= end_hour 含 end 所在整点桶）。
    let start_key = utc_ms_to_local_hour_key(start);
    let end_key = utc_ms_to_local_hour_key(end);

    // WHERE：基础时间范围 + 可选 filter。占位符 ?1=start_key ?2=end_key，filter 依次 ?3..。
    let mut where_parts = vec![
        "deleted_at = 0".to_string(),
        "time_hour >= ?1".to_string(),
        "time_hour <= ?2".to_string(),
    ];
    let mut binds: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
        Box::new(start_key.clone()),
        Box::new(end_key.clone()),
    ];
    if let Some(ref g) = query.filter_group {
        where_parts.push(format!("group_key = ?{}", binds.len() + 1));
        binds.push(Box::new(g.clone()));
    }
    if let Some(ref m) = query.filter_model {
        // 聚合表 model 列已是 actual_model 优先值，单列等值即可。
        where_parts.push(format!("model = ?{}", binds.len() + 1));
        binds.push(Box::new(m.clone()));
    }
    if let Some(ref p) = query.filter_platform {
        // 聚合表 platform_id 已是 eff_pid，直接整数等值。
        where_parts.push(format!("platform_id = CAST(?{} AS INTEGER)", binds.len() + 1));
        binds.push(Box::new(p.clone()));
    }
    let where_sql = where_parts.join(" AND ");
    // platform 维度 LEFT JOIN platform p 时，stats 表别名 s；platform 表也有 deleted_at 列，
    // 裸 deleted_at 歧义 → 用 s. 前缀版 where（其余列名 platform 表无，不歧义）。
    let where_sql_s: String = where_parts
        .iter()
        .map(|p| p.replace("deleted_at = 0", "s.deleted_at = 0"))
        .collect::<Vec<_>>()
        .join(" AND ");
    let refs: Vec<&dyn rusqlite::types::ToSql> = binds.iter().map(|b| b.as_ref()).collect();

    // ── Overview ──
    let overview = conn
        .prepare(&format!(
            "SELECT COALESCE(SUM(request_count),0), COALESCE(SUM(success_count),0), \
             COALESCE(SUM(sum_input_tokens),0), COALESCE(SUM(sum_output_tokens),0), COALESCE(SUM(sum_cache_tokens),0), \
             COALESCE(SUM(sum_duration_ms),0), COALESCE(SUM(sum_est_cost),0.0) \
             FROM stats_agg_hourly WHERE {where_sql}"
        ))
        .map_err(|e| e.to_string())?
        .query_row(refs.as_slice(), |row| {
            let total: i64 = row.get(0).unwrap_or(0);
            let success: i64 = row.get(1).unwrap_or(0);
            let inp: i64 = row.get(2).unwrap_or(0);
            let cache: i64 = row.get(4).unwrap_or(0);
            let sum_dur: i64 = row.get(5).unwrap_or(0);
            Ok(StatsOverview {
                total_requests: total as i32,
                success_rate: if total > 0 { success as f64 / total as f64 * 100.0 } else { 0.0 },
                total_input_tokens: inp,
                total_output_tokens: row.get(3).unwrap_or(0),
                total_cache_tokens: cache,
                cache_rate: if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 },
                avg_duration_ms: if total > 0 { sum_dur as f64 / total as f64 } else { 0.0 },
                total_cost: row.get(6).unwrap_or(0.0),
            })
        })
        .map_err(|e| format!("agg overview: {e}"))?;

    // ── Time buckets ──
    // hourly → 整个 time_hour 桶；daily → 取前 10 字符（YYYY-MM-DD）。
    let bucket_expr = match query.granularity.as_deref() {
        Some("hourly") => "time_hour",
        _ => "substr(time_hour, 1, 10)",
    };
    let buckets: Vec<StatsBucket> = conn
        .prepare(&format!(
            "SELECT {bucket_expr} AS b, COALESCE(SUM(request_count),0), COALESCE(SUM(success_count),0), \
             COALESCE(SUM(error_count),0), COALESCE(SUM(sum_input_tokens),0), COALESCE(SUM(sum_output_tokens),0), \
             COALESCE(SUM(sum_cache_tokens),0), COALESCE(SUM(sum_duration_ms),0), \
             COALESCE(SUM(sum_est_cost),0.0) \
             FROM stats_agg_hourly WHERE {where_sql} GROUP BY b ORDER BY b"
        ))
        .map_err(|e| e.to_string())?
        .query_map(refs.as_slice(), |row| {
            let req: i64 = row.get(1).unwrap_or(0);
            let sum_dur: i64 = row.get(7).unwrap_or(0);
            Ok(StatsBucket {
                time_bucket: row.get(0).unwrap_or_default(),
                total_requests: row.get(1).unwrap_or(0),
                success_count: row.get(2).unwrap_or(0),
                error_count: row.get(3).unwrap_or(0),
                input_tokens: row.get(4).unwrap_or(0),
                output_tokens: row.get(5).unwrap_or(0),
                cache_tokens: row.get(6).unwrap_or(0),
                avg_duration_ms: if req > 0 { sum_dur as f64 / req as f64 } else { 0.0 },
                total_cost: row.get(8).unwrap_or(0.0),
            })
        })
        .map_err(|e| format!("agg buckets: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // ── Dimension breakdown ──
    let dimension_data: Vec<DimensionEntry> = if let Some(ref gb) = query.group_by {
        let dim_sql = if gb == "platform" {
            // platform_id 已是 eff_pid，LEFT JOIN platform 取真名（含软删平台名仍可显示）。
            format!(
                "SELECT COALESCE(p.name, '未知') AS dim, COALESCE(SUM(s.request_count),0), COALESCE(SUM(s.success_count),0), \
                 COALESCE(SUM(s.sum_input_tokens),0), COALESCE(SUM(s.sum_output_tokens),0), COALESCE(SUM(s.sum_cache_tokens),0), \
                 COALESCE(SUM(s.sum_duration_ms),0), COALESCE(SUM(s.sum_est_cost),0.0) \
                 FROM stats_agg_hourly s LEFT JOIN platform p ON p.id = s.platform_id \
                 WHERE {where_sql_s} GROUP BY s.platform_id ORDER BY 2 DESC LIMIT 50"
            )
        } else {
            let dim_col = match gb.as_str() {
                "model" => "model",
                _ => "group_key",
            };
            format!(
                "SELECT {dim_col} AS dim, COALESCE(SUM(request_count),0), COALESCE(SUM(success_count),0), \
                 COALESCE(SUM(sum_input_tokens),0), COALESCE(SUM(sum_output_tokens),0), COALESCE(SUM(sum_cache_tokens),0), \
                 COALESCE(SUM(sum_duration_ms),0), COALESCE(SUM(sum_est_cost),0.0) \
                 FROM stats_agg_hourly WHERE {where_sql} GROUP BY {dim_col} ORDER BY 2 DESC LIMIT 50"
            )
        };
        conn.prepare(&dim_sql)
            .map_err(|e| e.to_string())?
            .query_map(refs.as_slice(), |row| {
                let req: i64 = row.get(1).unwrap_or(0);
                let sum_dur: i64 = row.get(6).unwrap_or(0);
                Ok(DimensionEntry {
                    name: row.get(0).unwrap_or_default(),
                    total_requests: row.get(1).unwrap_or(0),
                    success_count: row.get(2).unwrap_or(0),
                    input_tokens: row.get(3).unwrap_or(0),
                    output_tokens: row.get(4).unwrap_or(0),
                    cache_tokens: row.get(5).unwrap_or(0),
                    avg_duration_ms: if req > 0 { sum_dur as f64 / req as f64 } else { 0.0 },
                    total_cost: row.get(7).unwrap_or(0.0),
                })
            })
            .map_err(|e| format!("agg dimension: {e}"))?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };

    // ── available_models（当前时间范围 + group + platform 过滤内的模型集，不含 filter_model）──
    let mut am_parts = vec![
        "deleted_at = 0".to_string(),
        "time_hour >= ?1".to_string(),
        "time_hour <= ?2".to_string(),
    ];
    let mut am_binds: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(start_key), Box::new(end_key)];
    if let Some(ref g) = query.filter_group {
        am_parts.push(format!("group_key = ?{}", am_binds.len() + 1));
        am_binds.push(Box::new(g.clone()));
    }
    if let Some(ref p) = query.filter_platform {
        am_parts.push(format!("platform_id = CAST(?{} AS INTEGER)", am_binds.len() + 1));
        am_binds.push(Box::new(p.clone()));
    }
    let am_where = am_parts.join(" AND ");
    let am_refs: Vec<&dyn rusqlite::types::ToSql> = am_binds.iter().map(|b| b.as_ref()).collect();
    let available_models: Vec<String> = conn
        .prepare(&format!(
            "SELECT DISTINCT model FROM stats_agg_hourly WHERE {am_where} AND model != '' ORDER BY model"
        ))
        .map_err(|e| e.to_string())?
        .query_map(am_refs.as_slice(), |row| row.get::<_, String>(0))
        .map_err(|e| format!("agg available_models: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(StatsResult { overview, buckets, dimension_data, available_models })
}

pub(crate) fn query_stats_inner(conn: &Connection, query: &StatsQuery) -> Result<StatsResult, String> {
    let end = query.end.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let start = query.start.unwrap_or_else(|| {
        (chrono::Utc::now() - chrono::Duration::days(7)).timestamp_millis()
    });

    let qp = QueryParams {
        start,
        end,
        filter_group: query.filter_group.clone(),
        filter_model: query.filter_model.clone(),
        filter_platform: query.filter_platform.clone(),
    };

    // hourly / daily（含 None→daily 默认）粒度从聚合表 stats_agg_hourly 查；
    // minute / 5min 粒度聚合表不覆盖（hourly 桶无法下钻到分钟），仍走下方 proxy_log 原路径。
    match query.granularity.as_deref() {
        Some("minute") | Some("5min") => {} // fall through to proxy_log path
        _ => return query_stats_inner_agg(conn, query, start, end),
    }

    // 有效 platform_id 表达式：原 platform_id，auto 分组（platform_id=0）经
    // group.auto_from_platform 回溯到源平台（与 get_platform_usage_stats 同语义）。
    const EFF_PID: &str = "\
CASE WHEN proxy_log.platform_id = 0 THEN COALESCE(\
(SELECT CAST(g.auto_from_platform AS INTEGER) FROM \"group\" g \
 WHERE g.group_key = proxy_log.group_key AND g.auto_from_platform != '' AND g.deleted_at = 0 LIMIT 1), 0)\
ELSE proxy_log.platform_id END";

    // Build WHERE clause（列名一律 proxy_log. 前缀：dimension platform 分支 LEFT JOIN platform 后，
    // deleted_at / created_at 等列两表皆有，裸列名会触发 ambiguous column 错误）
    let mut where_parts = vec!["proxy_log.created_at >= ?1".to_string(), "proxy_log.created_at <= ?2".to_string()];
    if qp.filter_group.is_some() {
        where_parts.push("proxy_log.group_key = ?3".to_string());
    }
    if qp.filter_model.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize;
        where_parts.push(format!("(proxy_log.model = ?{idx} OR proxy_log.actual_model = ?{idx})"));
    }
    if qp.filter_platform.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize + qp.filter_model.is_some() as usize;
        // value = platform_id 十进制字符串；按有效平台 id（含 auto 分组回溯）匹配
        where_parts.push(format!("({EFF_PID}) = CAST(?{idx} AS INTEGER)"));
    }
    let where_sql = where_parts.join(" AND ");

    let bucket_expr = bucket_time_expr(query.granularity.as_deref());

    // ── Overview ──
    let overview_sql = format!(
        "SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
         COALESCE(SUM(est_cost), 0.0) \
         FROM proxy_log WHERE deleted_at = 0 AND {where_sql}"
    );
    let p = qp.to_sql_params();
    let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
    let overview = conn.prepare(&overview_sql)
        .map_err(|e| e.to_string())?
        .query_row(refs.as_slice(), |row| {
            let total: i32 = row.get(0).unwrap_or(0);
            let success: i32 = row.get(1).unwrap_or(0);
            Ok(StatsOverview {
                total_requests: total,
                success_rate: if total > 0 { success as f64 / total as f64 * 100.0 } else { 0.0 },
                total_input_tokens: row.get(2).unwrap_or(0),
                total_output_tokens: row.get(3).unwrap_or(0),
                total_cache_tokens: row.get(4).unwrap_or(0),
                cache_rate: {
                    let inp: i64 = row.get(2).unwrap_or(0);
                    let cache: i64 = row.get(4).unwrap_or(0);
                    if inp + cache > 0 { cache as f64 / (inp + cache) as f64 * 100.0 } else { 0.0 }
                },
                avg_duration_ms: row.get(5).unwrap_or(0.0),
                total_cost: row.get(6).unwrap_or(0.0),
            })
        }).map_err(|e| format!("overview: {e}"))?;

    // ── Time buckets ──
    let bucket_sql = format!(
        "SELECT {bucket_expr}, COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
         COALESCE(SUM(est_cost), 0.0) \
         FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 1"
    );
    let p = qp.to_sql_params();
    let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
    let buckets: Vec<StatsBucket> = conn.prepare(&bucket_sql)
        .map_err(|e| e.to_string())?
        .query_map(refs.as_slice(), |row| {
            Ok(StatsBucket {
                time_bucket: row.get(0).unwrap_or_default(),
                total_requests: row.get(1).unwrap_or(0),
                success_count: row.get(2).unwrap_or(0),
                error_count: row.get(3).unwrap_or(0),
                input_tokens: row.get(4).unwrap_or(0),
                output_tokens: row.get(5).unwrap_or(0),
                cache_tokens: row.get(6).unwrap_or(0),
                avg_duration_ms: row.get(7).unwrap_or(0.0),
                total_cost: row.get(8).unwrap_or(0.0),
            })
        }).map_err(|e| format!("buckets: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // ── Dimension breakdown ──
    let dimension_data = if let Some(ref gb) = query.group_by {
        // platform 维度按有效 platform_id（含 auto 分组回溯）聚合，JOIN platform 取真名
        let dim_sql = if gb == "platform" {
            format!(
                "SELECT COALESCE(p.name, '未知'), COUNT(*), \
                 SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
                 SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
                 COALESCE(SUM(est_cost), 0.0) \
                 FROM proxy_log LEFT JOIN platform p ON p.id = ({EFF_PID}) \
                 WHERE proxy_log.deleted_at = 0 AND {where_sql} GROUP BY ({EFF_PID}) ORDER BY 2 DESC LIMIT 50"
            )
        } else {
            let dim_col = match gb.as_str() {
                "model" => "actual_model",
                _ => "group_key",
            };
            format!(
                "SELECT {dim_col}, COUNT(*), \
                 SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
                 SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms), \
                 COALESCE(SUM(est_cost), 0.0) \
                 FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 2 DESC LIMIT 50"
            )
        };
        let p = qp.to_sql_params();
        let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
        conn.prepare(&dim_sql)
            .map_err(|e| e.to_string())?
            .query_map(refs.as_slice(), |row| {
                Ok(DimensionEntry {
                    name: row.get(0).unwrap_or_default(),
                    total_requests: row.get(1).unwrap_or(0),
                    success_count: row.get(2).unwrap_or(0),
                    input_tokens: row.get(3).unwrap_or(0),
                    output_tokens: row.get(4).unwrap_or(0),
                    cache_tokens: row.get(5).unwrap_or(0),
                    avg_duration_ms: row.get(6).unwrap_or(0.0),
                    total_cost: row.get(7).unwrap_or(0.0),
                })
            }).map_err(|e| format!("dimension: {e}"))?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };

    // available_models：当前筛选范围（date + group + platform，不含 filter_model）内
    // 实际有记录的模型名。列表达式与 filter_model 行为一致（actual_model 优先，回退 model），
    // 使下拉项与筛选语义对齐——选中某项必能命中。
    let am_where = {
        let mut parts = vec![
            "proxy_log.created_at >= ?1".to_string(),
            "proxy_log.created_at <= ?2".to_string(),
        ];
        if qp.filter_group.is_some() {
            parts.push("proxy_log.group_key = ?3".to_string());
        }
        if qp.filter_platform.is_some() {
            let idx = 3 + qp.filter_group.is_some() as usize;
            parts.push(format!("({EFF_PID}) = CAST(?{idx} AS INTEGER)"));
        }
        parts.join(" AND ")
    };
    let am_refs: Vec<&dyn rusqlite::ToSql> = {
        let mut v: Vec<&dyn rusqlite::ToSql> = vec![&start, &end];
        if let Some(ref g) = qp.filter_group { v.push(g); }
        if let Some(ref p) = qp.filter_platform { v.push(p); }
        v
    };
    let available_models: Vec<String> = conn
        .prepare(&format!(
            "SELECT DISTINCT CASE WHEN proxy_log.actual_model != '' THEN proxy_log.actual_model ELSE proxy_log.model END AS m \
             FROM proxy_log WHERE {am_where} ORDER BY m"
        ))
        .map_err(|e| e.to_string())?
        .query_map(am_refs.as_slice(), |row| row.get::<_, String>(0))
        .map_err(|e| format!("available_models: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(StatsResult { overview, buckets, dimension_data, available_models })
}

// ─── Model Price CRUD ──────────────────────────────────────

