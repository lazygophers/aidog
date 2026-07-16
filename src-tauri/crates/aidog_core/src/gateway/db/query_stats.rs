use super::*;
use rusqlite::{Connection};

/// minute/5min 路径的过滤参数（hourly/daily 走聚合表另算，不用本结构）。
struct QueryParams {
    filter_group: Option<String>,
    filter_model: Option<String>,
    filter_platform: Option<String>,
}

#[track_caller]
pub fn query_stats<'a>(db: &'a Db, query: &'a StatsQuery) -> impl std::future::Future<Output = Result<StatsResult, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let query = query.clone();
    // 跨库预查（proxy-log-db-split s3）：stats_agg_hourly / proxy_log 在 log.db，
    // `"group"` / `platform` 表在主库 → 预查 auto_map + platform_names 移入 proxy_log 闭包。
    let auto_map = db
        .call_read_platform_traced(None, __db_caller, |conn| load_auto_from_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into())))
        .await
        .map_err(|e| format!("query_stats load auto_map: {e}"))?;
    let platform_names = db
        .call_read_platform_traced(None, __db_caller, |conn| platform_id_name_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into())))
        .await
        .map_err(|e| format!("query_stats load platform_names: {e}"))?;
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            query_stats_inner(conn, &query, &auto_map, &platform_names)
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
    // 跨库预查（同 query_stats）：auto_map + platform_names 在主库，预查移入 proxy_log 闭包。
    let auto_map = db
        .call_read_platform_traced(None, __db_caller, |conn| load_auto_from_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into())))
        .await
        .map_err(|e| format!("query_stats_batch load auto_map: {e}"))?;
    let platform_names = db
        .call_read_platform_traced(None, __db_caller, |conn| platform_id_name_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into())))
        .await
        .map_err(|e| format!("query_stats_batch load platform_names: {e}"))?;
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let mut out = Vec::with_capacity(queries.len());
            for q in &queries {
                out.push(
                    query_stats_inner(conn, q, &auto_map, &platform_names)
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

/// UTC ms → 本地分钟桶文本键，复刻 minute/5min 的 SQL strftime 分桶（含 'localtime'）：
/// - `minute` → `%Y-%m-%d %H:%M`（逐分钟）
/// - `5min`   → epoch 秒先 floor 到 300s 边界再格式化（与 `(created_at/1000/300)*300` 等价）
///
/// 其余粒度不走本函数（minute/5min 专用，proxy_log 内存路径）。
fn utc_ms_to_local_minute_key(ms: i64, five_min: bool) -> String {
    use chrono::{Local, TimeZone};
    let secs = if five_min {
        // SQL: (created_at/1000/300)*300（整数除法 floor 到 5min 边界），单位秒。
        (ms / 1000 / 300) * 300
    } else {
        ms / 1000
    };
    Local
        .timestamp_opt(secs, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

/// 从聚合表 stats_agg_hourly 跑统计查询（hourly/daily 粒度 + 任意 filter/group_by）。
/// 时间范围按本地小时桶字典序比较；daily 桶 = substr(time_hour,1,10)，hourly 桶 = time_hour。
fn query_stats_inner_agg(
    conn: &Connection,
    query: &StatsQuery,
    start: i64,
    end: i64,
    _auto_map: &HashMap<String, i64>,
    platform_names: &HashMap<i64, String>,
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
        if gb == "platform" {
            // platform_id 已是 eff_pid：单表 GROUP BY platform_id 取聚合（无 JOIN），
            // 平台名走内存 id→name 映射回填（含软删平台名仍可显示），同 today_platform_stats(J6)。
            let dim_sql = format!(
                "SELECT platform_id AS pid, COALESCE(SUM(request_count),0), COALESCE(SUM(success_count),0), \
                 COALESCE(SUM(sum_input_tokens),0), COALESCE(SUM(sum_output_tokens),0), COALESCE(SUM(sum_cache_tokens),0), \
                 COALESCE(SUM(sum_duration_ms),0), COALESCE(SUM(sum_est_cost),0.0) \
                 FROM stats_agg_hourly WHERE {where_sql} GROUP BY platform_id ORDER BY 2 DESC LIMIT 50"
            );
            let rows: Vec<(i64, DimensionEntry)> = conn
                .prepare(&dim_sql)
                .map_err(|e| e.to_string())?
                .query_map(refs.as_slice(), |row| {
                    let pid: i64 = row.get(0).unwrap_or(0);
                    let req: i64 = row.get(1).unwrap_or(0);
                    let sum_dur: i64 = row.get(6).unwrap_or(0);
                    Ok((pid, DimensionEntry {
                        name: String::new(), // 下方按 pid 回填
                        total_requests: row.get(1).unwrap_or(0),
                        success_count: row.get(2).unwrap_or(0),
                        input_tokens: row.get(3).unwrap_or(0),
                        output_tokens: row.get(4).unwrap_or(0),
                        cache_tokens: row.get(5).unwrap_or(0),
                        avg_duration_ms: if req > 0 { sum_dur as f64 / req as f64 } else { 0.0 },
                        total_cost: row.get(7).unwrap_or(0.0),
                    }))
                })
                .map_err(|e| format!("agg dimension: {e}"))?
                .filter_map(|r| r.ok())
                .collect();
            // platform_names 由调用方跨库预查自主库传入（agg 走 proxy_log handle，无 platform 表）。
            rows.into_iter()
                .map(|(pid, mut e)| {
                    e.name = platform_names.get(&pid).cloned().unwrap_or_else(|| "未知".to_string());
                    e
                })
                .collect()
        } else {
            let dim_col = match gb.as_str() {
                "model" => "model",
                _ => "group_key",
            };
            let dim_sql = format!(
                "SELECT {dim_col} AS dim, COALESCE(SUM(request_count),0), COALESCE(SUM(success_count),0), \
                 COALESCE(SUM(sum_input_tokens),0), COALESCE(SUM(sum_output_tokens),0), COALESCE(SUM(sum_cache_tokens),0), \
                 COALESCE(SUM(sum_duration_ms),0), COALESCE(SUM(sum_est_cost),0.0) \
                 FROM stats_agg_hourly WHERE {where_sql} GROUP BY {dim_col} ORDER BY 2 DESC LIMIT 50"
            );
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
        }
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

pub(crate) fn query_stats_inner(conn: &Connection, query: &StatsQuery, auto_map: &HashMap<String, i64>, platform_names: &HashMap<i64, String>) -> Result<StatsResult, String> {
    let end = query.end.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let start = query.start.unwrap_or_else(|| {
        (chrono::Utc::now() - chrono::Duration::days(7)).timestamp_millis()
    });

    let qp = QueryParams {
        filter_group: query.filter_group.clone(),
        filter_model: query.filter_model.clone(),
        filter_platform: query.filter_platform.clone(),
    };

    // hourly / daily（含 None→daily 默认）粒度从聚合表 stats_agg_hourly 查；
    // minute / 5min 粒度聚合表不覆盖（hourly 桶无法下钻到分钟），仍走下方 proxy_log 原路径。
    match query.granularity.as_deref() {
        Some("minute") | Some("5min") => {} // fall through to proxy_log path
        _ => return query_stats_inner_agg(conn, query, start, end, auto_map, platform_names),
    }

    // minute/5min 细粒度走 proxy_log 原始行；eff_pid（auto 分组 platform_id=0 回溯源平台）
    // 不再用 SQL 标量子查询/LEFT JOIN，改为内存预取 group_key→eff_pid 映射逐行回溯 + 内存聚合。
    // 时间/group/model 过滤仍在 SQL（缩小行集），唯 eff_pid(platform) 过滤与平台维度 GROUP BY 搬内存。
    // auto_map + platform_names 由调用方 (query_stats/query_stats_batch) 跨库预查自主库传入
    // （log.db 无 "group"/platform 表，禁在 proxy_log 闭包内现取）。
    let five_min = matches!(query.granularity.as_deref(), Some("5min"));
    // filter_platform value = eff_pid 十进制字符串；解析为整数后内存按行 eff_pid 等值过滤。
    let want_pid: Option<i64> = qp.filter_platform.as_ref().and_then(|s| s.parse::<i64>().ok());

    // SQL 仅下推 time/group/model 过滤（eff_pid 过滤搬内存）。
    let mut where_parts = vec![
        "deleted_at = 0".to_string(),
        "created_at >= ?1".to_string(),
        "created_at <= ?2".to_string(),
    ];
    if qp.filter_group.is_some() {
        where_parts.push("group_key = ?3".to_string());
    }
    if qp.filter_model.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize;
        where_parts.push(format!("(model = ?{idx} OR actual_model = ?{idx})"));
    }
    let where_sql = where_parts.join(" AND ");
    // eff_pid 过滤搬内存后，filter_platform 不再进 SQL：只绑 start/end/group/model。
    let mut binds: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(start), Box::new(end)];
    if let Some(ref g) = qp.filter_group { binds.push(Box::new(g.clone())); }
    if let Some(ref m) = qp.filter_model { binds.push(Box::new(m.clone())); }
    let refs: Vec<&dyn rusqlite::types::ToSql> = binds.iter().map(|b| b.as_ref()).collect();

    // 取全部命中行的原始字段（含 eff_pid 回溯所需 platform_id/group_key）。
    struct Row {
        created_at: i64,
        status_code: i32,
        input: i64,
        output: i64,
        cache: i64,
        duration: i64,
        est_cost: f64,
        eff_pid: i64,
        group_key: String,
        actual_model: String,
    }
    let rows: Vec<Row> = conn
        .prepare(&format!(
            "SELECT created_at, status_code, \
             COALESCE(input_tokens,0), COALESCE(output_tokens,0), COALESCE(cache_tokens,0), \
             COALESCE(duration_ms,0), COALESCE(est_cost,0.0), platform_id, group_key, actual_model \
             FROM proxy_log WHERE {where_sql}"
        ))
        .map_err(|e| e.to_string())?
        .query_map(refs.as_slice(), |r| {
            let platform_id: i64 = r.get(7)?;
            let group_key: String = r.get(8)?;
            let eff_pid = resolve_eff_pid(platform_id, group_key.as_str(), auto_map);
            Ok(Row {
                created_at: r.get(0)?,
                status_code: r.get(1)?,
                input: r.get(2)?,
                output: r.get(3)?,
                cache: r.get(4)?,
                duration: r.get(5)?,
                est_cost: r.get(6)?,
                eff_pid,
                group_key,
                actual_model: r.get(9)?,
            })
        })
        .map_err(|e| format!("minute rows: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let is_2xx = |code: i32| (200..300).contains(&code);

    // ── Overview ──（want_pid 过滤后逐行累加，复刻 SQL COUNT/SUM/AVG 语义）
    let mut ov_total = 0i64;
    let mut ov_success = 0i64;
    let mut ov_input = 0i64;
    let mut ov_output = 0i64;
    let mut ov_cache = 0i64;
    let mut ov_dur = 0i64;
    let mut ov_cost = 0.0f64;
    for r in &rows {
        if let Some(w) = want_pid { if r.eff_pid != w { continue; } }
        ov_total += 1;
        if is_2xx(r.status_code) { ov_success += 1; }
        ov_input += r.input;
        ov_output += r.output;
        ov_cache += r.cache;
        ov_dur += r.duration;
        ov_cost += r.est_cost;
    }
    let overview = StatsOverview {
        total_requests: ov_total as i32,
        success_rate: if ov_total > 0 { ov_success as f64 / ov_total as f64 * 100.0 } else { 0.0 },
        total_input_tokens: ov_input,
        total_output_tokens: ov_output,
        total_cache_tokens: ov_cache,
        cache_rate: if ov_input + ov_cache > 0 { ov_cache as f64 / (ov_input + ov_cache) as f64 * 100.0 } else { 0.0 },
        // SQL AVG(duration_ms) 对所有命中行（含 NULL→0 这里已 COALESCE）求均值。
        avg_duration_ms: if ov_total > 0 { ov_dur as f64 / ov_total as f64 } else { 0.0 },
        total_cost: ov_cost,
    };

    // ── Time buckets ──（按本地分钟桶 key 分组，ORDER BY key 升序）
    #[derive(Default)]
    struct Bkt { req: i64, succ: i64, err: i64, input: i64, output: i64, cache: i64, dur: i64, cost: f64 }
    let mut bmap: std::collections::HashMap<String, Bkt> = std::collections::HashMap::new();
    for r in &rows {
        if let Some(w) = want_pid { if r.eff_pid != w { continue; } }
        let key = utc_ms_to_local_minute_key(r.created_at, five_min);
        let b = bmap.entry(key).or_default();
        b.req += 1;
        if is_2xx(r.status_code) { b.succ += 1; } else { b.err += 1; }
        b.input += r.input;
        b.output += r.output;
        b.cache += r.cache;
        b.dur += r.duration;
        b.cost += r.est_cost;
    }
    let mut bucket_keys: Vec<String> = bmap.keys().cloned().collect();
    bucket_keys.sort();
    let buckets: Vec<StatsBucket> = bucket_keys
        .into_iter()
        .map(|k| {
            let b = &bmap[&k];
            StatsBucket {
                time_bucket: k.clone(),
                total_requests: b.req as i32,
                success_count: b.succ as i32,
                error_count: b.err as i32,
                input_tokens: b.input,
                output_tokens: b.output,
                cache_tokens: b.cache,
                avg_duration_ms: if b.req > 0 { b.dur as f64 / b.req as f64 } else { 0.0 },
                total_cost: b.cost,
            }
        })
        .collect();

    // ── Dimension breakdown ──（platform 维度按 eff_pid 内存分组 + 补名；model/group 按列分组）
    let dimension_data: Vec<DimensionEntry> = if let Some(ref gb) = query.group_by {
        // 维度键 → 累计桶；platform 维度键为 eff_pid 字符串，其余为列值。
        #[derive(Default)]
        struct Dim { req: i64, succ: i64, input: i64, output: i64, cache: i64, dur: i64, cost: f64 }
        let mut dmap: std::collections::HashMap<i64, Dim> = std::collections::HashMap::new();
        let mut smap: std::collections::HashMap<String, Dim> = std::collections::HashMap::new();
        let is_platform = gb == "platform";
        for r in &rows {
            if let Some(w) = want_pid { if r.eff_pid != w { continue; } }
            let d = if is_platform {
                dmap.entry(r.eff_pid).or_default()
            } else {
                // model 维度用 actual_model（与旧 SQL GROUP BY actual_model 一致）；否则 group_key。
                let k = if gb == "model" { r.actual_model.clone() } else { r.group_key.clone() };
                smap.entry(k).or_default()
            };
            d.req += 1;
            if is_2xx(r.status_code) { d.succ += 1; }
            d.input += r.input;
            d.output += r.output;
            d.cache += r.cache;
            d.dur += r.duration;
            d.cost += r.est_cost;
        }
        // 组装 + 排序（总请求数降序）+ LIMIT 50，复刻 ORDER BY 2 DESC LIMIT 50。
        let mut entries: Vec<DimensionEntry> = if is_platform {
            dmap.into_iter()
                .map(|(pid, d)| DimensionEntry {
                    name: platform_names.get(&pid).cloned().unwrap_or_else(|| "未知".to_string()),
                    total_requests: d.req as i32,
                    success_count: d.succ as i32,
                    input_tokens: d.input,
                    output_tokens: d.output,
                    cache_tokens: d.cache,
                    avg_duration_ms: if d.req > 0 { d.dur as f64 / d.req as f64 } else { 0.0 },
                    total_cost: d.cost,
                })
                .collect()
        } else {
            smap.into_iter()
                .map(|(name, d)| DimensionEntry {
                    name,
                    total_requests: d.req as i32,
                    success_count: d.succ as i32,
                    input_tokens: d.input,
                    output_tokens: d.output,
                    cache_tokens: d.cache,
                    avg_duration_ms: if d.req > 0 { d.dur as f64 / d.req as f64 } else { 0.0 },
                    total_cost: d.cost,
                })
                .collect()
        };
        entries.sort_by_key(|e| std::cmp::Reverse(e.total_requests));
        entries.truncate(50);
        entries
    } else {
        vec![]
    };

    // available_models：当前筛选范围（date + group + platform，不含 filter_model）内实际有记录的
    // 模型名（actual_model 优先回退 model）。基于已取行内存去重 + 升序，eff_pid 过滤同上。
    // 注意：available_models 不应受 filter_model 影响——rows 已按 filter_model 过滤，故此处单独从
    // proxy_log 取（不含 model 过滤）以保持原语义。
    let mut am_parts = vec![
        "deleted_at = 0".to_string(),
        "created_at >= ?1".to_string(),
        "created_at <= ?2".to_string(),
    ];
    if qp.filter_group.is_some() {
        am_parts.push("group_key = ?3".to_string());
    }
    let am_where = am_parts.join(" AND ");
    let mut am_binds: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(start), Box::new(end)];
    if let Some(ref g) = qp.filter_group { am_binds.push(Box::new(g.clone())); }
    let am_refs: Vec<&dyn rusqlite::types::ToSql> = am_binds.iter().map(|b| b.as_ref()).collect();
    let mut model_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    conn.prepare(&format!(
            "SELECT platform_id, group_key, actual_model, model FROM proxy_log WHERE {am_where}"
        ))
        .map_err(|e| e.to_string())?
        .query_map(am_refs.as_slice(), |r| {
            let platform_id: i64 = r.get(0)?;
            let group_key: String = r.get(1)?;
            let actual_model: String = r.get(2)?;
            let model: String = r.get(3)?;
            Ok((resolve_eff_pid(platform_id, group_key.as_str(), auto_map),
                if !actual_model.is_empty() { actual_model } else { model }))
        })
        .map_err(|e| format!("available_models: {e}"))?
        .filter_map(|r| r.ok())
        .for_each(|(eff_pid, m)| {
            if let Some(w) = want_pid { if eff_pid != w { return; } }
            model_set.insert(m);
        });
    let available_models: Vec<String> = model_set.into_iter().collect();

    Ok(StatsResult { overview, buckets, dimension_data, available_models })
}

// ─── Model Price CRUD ──────────────────────────────────────

