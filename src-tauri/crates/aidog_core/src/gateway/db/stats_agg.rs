use super::*;
use rusqlite::{params, Connection, OptionalExtension};

/// 聚合复合键：(本地小时桶, actual_model 优先, group_key, eff_pid)。与 stats_agg_hourly
/// UNIQUE(time_hour,model,group_key,platform_id) 对应。从 proxy_log 重建/回填时内存 GROUP BY 用。
#[derive(PartialEq, Eq, Hash, Clone)]
struct AggKey {
    time_hour: String,
    model: String,
    group_key: String,
    platform_id: i64,
}

/// 单桶累计值（与 stats_agg_hourly 的 request_count/success/error/sum_* 列一一对应）。
#[derive(Default)]
struct AggBucket {
    request_count: i64,
    success_count: i64,
    error_count: i64,
    sum_input_tokens: i64,
    sum_output_tokens: i64,
    sum_cache_tokens: i64,
    sum_est_cost: f64,
    sum_duration_ms: i64,
}

/// 把全部有效 proxy_log 行按 (本地小时桶, actual_model 优先, group_key, eff_pid) 在内存聚合。
///
/// 替代旧 `INSERT ... SELECT ... GROUP BY 1,2,3,4`（含 eff_pid 标量子查询）：
/// 单表读 proxy_log（无 JOIN/子查询）+ `load_auto_from_map` 内存回溯 eff_pid + `resolve_eff_pid`
/// 逐行算，再 HashMap 累加。`time_hour` 用 `utc_ms_to_local_hour_key` 复刻
/// `strftime('%Y-%m-%d %H:00:00', ...,'localtime')`；model 用 actual_model 非空优先（与旧 SELECT 别名一致）。
/// success=2xx，error=终态非 2xx，与旧 SQL CASE 逐字段等价。
fn aggregate_proxy_logs(
    conn: &Connection,
    auto_map: &HashMap<String, i64>,
) -> rusqlite::Result<HashMap<AggKey, AggBucket>> {
    // count_tokens 子端点（/v1/messages/count_tokens）纯计数、不计入 stats_agg 聚合（与增量
    // 写入路径 log.rs first_agg gate 的 is_count_tokens 排除同口径），故回填/重建时一并排除，
    // 避免 count_tokens 行的 input_tokens/est_cost 污染聚合总统计（实测占全库 cost 17.6%）。
    let mut stmt = conn.prepare(
        "SELECT created_at, \
                CASE WHEN actual_model != '' THEN actual_model ELSE model END, \
                group_key, platform_id, status_code, \
                COALESCE(input_tokens, 0), COALESCE(output_tokens, 0), COALESCE(cache_tokens, 0), \
                COALESCE(est_cost, 0.0), COALESCE(duration_ms, 0) \
         FROM proxy_log WHERE deleted_at = 0 \
           AND request_url NOT LIKE '%count_tokens%'",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,    // created_at
            r.get::<_, String>(1)?, // model（actual 优先）
            r.get::<_, String>(2)?, // group_key
            r.get::<_, i64>(3)?,    // platform_id
            r.get::<_, i64>(4)?,    // status_code
            r.get::<_, i64>(5)?,    // input
            r.get::<_, i64>(6)?,    // output
            r.get::<_, i64>(7)?,    // cache
            r.get::<_, f64>(8)?,    // est_cost
            r.get::<_, i64>(9)?,    // duration
        ))
    })?;

    let mut map: HashMap<AggKey, AggBucket> = HashMap::new();
    for r in rows {
        let (created_at, model, group_key, platform_id, status_code, inp, out, cache, cost, dur) = r?;
        let eff_pid = resolve_eff_pid(platform_id, &group_key, auto_map);
        let key = AggKey {
            time_hour: utc_ms_to_local_hour_key(created_at),
            model,
            group_key,
            platform_id: eff_pid,
        };
        let b = map.entry(key).or_default();
        b.request_count += 1;
        let is_2xx = (200..300).contains(&status_code);
        if is_2xx {
            b.success_count += 1;
        } else {
            b.error_count += 1;
        }
        b.sum_input_tokens += inp;
        b.sum_output_tokens += out;
        b.sum_cache_tokens += cache;
        b.sum_est_cost += cost;
        b.sum_duration_ms += dur;
    }
    Ok(map)
}

/// 把内存聚合结果 UPSERT 进 stats_agg_hourly（rebuild 覆盖写 / 回填首建共用）。
/// 冲突键 (time_hour,model,group_key,platform_id) 命中时用真值【覆盖】（非累加：聚合已是全量
/// COUNT/SUM 真值，累加会翻倍）；created_at 仅首建写、命中保留旧值、deleted_at 不动。
/// proxy_log 已无对应行的旧聚合数据【保留】（不在本批 → 不动）。
fn upsert_aggregated(conn: &Connection, agg: &HashMap<AggKey, AggBucket>, now: i64) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO stats_agg_hourly \
         (time_hour, model, group_key, platform_id, \
          request_count, success_count, error_count, \
          sum_input_tokens, sum_output_tokens, sum_cache_tokens, \
          sum_est_cost, sum_duration_ms, created_at, updated_at, deleted_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, 0) \
         ON CONFLICT(time_hour, model, group_key, platform_id) DO UPDATE SET \
          request_count = excluded.request_count, \
          success_count = excluded.success_count, \
          error_count = excluded.error_count, \
          sum_input_tokens = excluded.sum_input_tokens, \
          sum_output_tokens = excluded.sum_output_tokens, \
          sum_cache_tokens = excluded.sum_cache_tokens, \
          sum_est_cost = excluded.sum_est_cost, \
          sum_duration_ms = excluded.sum_duration_ms, \
          updated_at = excluded.updated_at",
    )?;
    for (k, b) in agg {
        stmt.execute(params![
            k.time_hour, k.model, k.group_key, k.platform_id,
            b.request_count, b.success_count, b.error_count,
            b.sum_input_tokens, b.sum_output_tokens, b.sum_cache_tokens,
            b.sum_est_cost, b.sum_duration_ms, now,
        ])?;
    }
    Ok(())
}

/// 存量一次性回填（schema migration 内调用，紧随 stats_agg_hourly 建表 DDL）。
/// 空表守卫在 Rust 内判（`SELECT 1 FROM stats_agg_hourly LIMIT 1`），替代旧 DDL 串内 `NOT EXISTS`。
/// 表非空（已回填/已有增量写入）则跳过，避免重复执行翻倍。
pub(crate) fn backfill_stats_agg_if_empty(
    conn: &Connection,
    auto_map: &HashMap<String, i64>,
) -> rusqlite::Result<()> {
    let exists: bool = conn
        .query_row("SELECT 1 FROM stats_agg_hourly LIMIT 1", [], |_| Ok(true))
        .optional()?
        .unwrap_or(false);
    if exists {
        return Ok(());
    }
    let agg = aggregate_proxy_logs(conn, auto_map)?;
    let now = chrono::Utc::now().timestamp_millis();
    upsert_aggregated(conn, &agg, now)
}

/// 一条终态请求的聚合增量入参（写入 stats_agg_hourly 的 UPSERT 源）。
/// eff_pid（platform_id=0 的 auto 回溯）在 UPSERT 写连接内用 `load_auto_from_map`/`resolve_eff_pid`
/// 内存回溯（写时物化），本结构传【原始】platform_id，函数不再含 SQL 标量子查询。
#[derive(Debug, Clone)]
pub struct StatsAggInput {
    /// 创建时间（UTC ms），用于在 SQL 内算本地小时桶。
    pub created_at: i64,
    /// 模型名（调用方已取 actual_model 非空否则 model）。
    pub model: String,
    pub group_key: String,
    /// 原始 platform_id（=0 时写连接内经 load_auto_from_map 回溯到 eff_pid）。
    pub platform_id: i64,
    pub status_code: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_tokens: i64,
    pub est_cost: f64,
    pub duration_ms: i64,
}

/// 对一条终态请求按 (time_hour,model,group_key,platform_id) UPSERT 进 stats_agg_hourly。
/// eff_pid 写时物化：platform_id!=0 直接用（零查询）；=0 才 `load_auto_from_map` 内存回溯，
/// 替代旧 SQL 内 `CASE WHEN ?4=0 THEN (SELECT ... FROM "group")` 标量子查询。
/// 写入【不受日志开关影响】：proxy 终态路径无条件调用。失败非致命（调用方 warn 不中断请求）。
///
/// stats_agg_hourly 已迁回主库（stats-agg-to-main-db s3）：写入走主库写槽 `call_traced`。
/// auto_map 预查（主库读槽）仍保留——避免在每条请求的写闭包内重复 prepare/load_auto_from_map。
#[track_caller]
pub fn upsert_stats_agg(db: &Db, input: StatsAggInput) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    // platform_id=0（auto 分组）才需 auto_map 回溯；主库读槽预查 move 进写闭包。
    let auto_map = if input.platform_id == 0 {
        db.call_read_platform_traced(None, __db_caller, |conn| load_auto_from_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into())))
            .await
            .map_err(|e| format!("upsert stats agg load auto_map: {e}"))?
    } else {
        HashMap::new()
    };
    db
        .call_traced(None, __db_caller, move |conn| {
            let now = chrono::Utc::now().timestamp_millis();
            let eff_pid = if input.platform_id != 0 {
                input.platform_id
            } else {
                resolve_eff_pid(0, &input.group_key, &auto_map)
            };
            // 本地小时桶由 SQL 内 strftime('...','localtime') 算，与 bucket_time_expr/回填一致。
            let is_2xx = input.status_code >= 200 && input.status_code < 300;
            let success = if is_2xx { 1i64 } else { 0 };
            let error = if is_2xx { 0i64 } else { 1 };
            conn.execute(
                "INSERT INTO stats_agg_hourly \
                 (time_hour, model, group_key, platform_id, \
                  request_count, success_count, error_count, \
                  sum_input_tokens, sum_output_tokens, sum_cache_tokens, \
                  sum_est_cost, sum_duration_ms, created_at, updated_at, deleted_at) \
                 VALUES ( \
                  strftime('%Y-%m-%d %H:00:00', ?1/1000, 'unixepoch', 'localtime'), \
                  ?2, ?3, ?4, \
                  1, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 0) \
                 ON CONFLICT(time_hour, model, group_key, platform_id) DO UPDATE SET \
                  request_count = request_count + 1, \
                  success_count = success_count + ?5, \
                  error_count = error_count + ?6, \
                  sum_input_tokens = sum_input_tokens + ?7, \
                  sum_output_tokens = sum_output_tokens + ?8, \
                  sum_cache_tokens = sum_cache_tokens + ?9, \
                  sum_est_cost = sum_est_cost + ?10, \
                  sum_duration_ms = sum_duration_ms + ?11, \
                  updated_at = ?12",
                params![
                    input.created_at,
                    input.model,
                    input.group_key,
                    eff_pid,
                    success,
                    error,
                    input.input_tokens,
                    input.output_tokens,
                    input.cache_tokens,
                    input.est_cost,
                    input.duration_ms,
                    now,
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert stats agg: {e}"))
    }
}

/// 从 proxy_log upsert 覆盖写 stats_agg_hourly（用户启用 log 后手动修复用）。
/// 存在则按 proxy_log 真值覆盖、不存在才创建；不再清空整表。
/// 关日志期间未落 proxy_log 但已聚合的旧行【保留】（不被抹掉）。
///
/// stats-agg-to-main-db s4：跨库两阶段——proxy_log 在 log.db，stats_agg_hourly 在主库，
/// 禁同闭包跨库读。① log.db 读池跑 `aggregate_proxy_logs` 内存聚合 → Vec；
/// ② 主库写槽 `call_traced` 跑 `upsert_aggregated` 批量写入。
#[track_caller]
pub fn rebuild_stats_agg_from_logs(db: &Db) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let auto_map = db
        .call_read_platform_traced(None, __db_caller, |conn| load_auto_from_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into())))
        .await
        .map_err(|e| format!("rebuild stats agg load auto_map: {e}"))?;
    // ① log.db 读池：proxy_log 内存聚合（无写）。
    let agg = db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            aggregate_proxy_logs(conn, &auto_map).map_err(|e| tokio_rusqlite::Error::Other(e.into()))
        })
        .await
        .map_err(|e| format!("rebuild stats agg aggregate: {e}"))?;
    // ② 主库写槽：批量 UPSERT 进 stats_agg_hourly。
    db
        .call_traced(None, __db_caller, move |conn| {
            let now = chrono::Utc::now().timestamp_millis();
            upsert_aggregated(conn, &agg, now)?;
            Ok(())
        })
        .await
        .map_err(|e| format!("rebuild stats agg: {e}"))
    }
}

/// 一次性纠正：历史 bug（upsert_log 在请求生命周期被多次调用，终态后每次仍对同一请求 +1，
/// 实测 stats_agg_hourly ≈ proxy_log 的 ~8 倍虚高）已污染聚合表。版本门控（参考 default-flip
/// 的 defaults_version 模式 / migrate_auto_vacuum 的 setting 标记）确保仅在下次启动跑一次：
/// 置标记 setting(stats/agg_rebuild_v1)=true 后永不再跑。
///
/// 注意：rebuild 现为 upsert 覆盖写（不再清空整表）——proxy_log 有对应行的桶按真值覆盖修正；
/// **关日志期间未落 proxy_log 但已聚合的旧行保留**（不再被抹掉）。
/// 失败仅返回 Err，调用方（启动 spawn）warn 不置标记，下次启动重试。
pub async fn rebuild_stats_agg_once_if_needed(db: &Db) -> Result<bool, String> {
    if let Ok(Some(v)) = get_setting(db, "stats", "agg_rebuild_v1").await
        && v == serde_json::Value::Bool(true) {
            return Ok(false);
        }
    rebuild_stats_agg_from_logs(db).await?;
    set_setting(
        db,
        SetSettingInput {
            scope: "stats".into(),
            key: "agg_rebuild_v1".into(),
            value: serde_json::Value::Bool(true),
        },
    )
    .await?;
    Ok(true)
}

/// 一次性纠正：历史 count_tokens 计费污染（count_tokens 行的 input_tokens/est_cost 曾计入
/// stats_agg_hourly，实测占全库 cost 17.6%）。aggregate_proxy_logs 现已排除 count_tokens 行，
/// 故此处「覆盖写 + 删孤儿桶」即可把历史聚合纠正到与新规则一致：
///   ① upsert_aggregated 用【不含 count_tokens 的真值】覆盖所有仍有非-count_tokens 行的桶；
///   ② 对【纯 count_tokens 桶】（该 (time_hour,model,group_key,platform_id) 在过滤后聚合中已不存在，
///      但 stats_agg 仍有旧行）→ 删除，否则虚高残留。判定：聚合后存在于 stats_agg 但不在本批 agg key 集合中。
/// 幂等：版本门控 setting(stats/agg_count_tokens_excluded_v1)=true 后永不再跑。
/// proxy_log 历史行不动（保留单行审计可见 input_tokens+est_cost）。
/// 注意：关日志期间未落 proxy_log 的旧桶会被误删——但本项目 P6 已确认日志主开关常开未动，
/// 且先前 agg_rebuild_v1 纠正同样不保留此类桶，口径一致。
pub async fn correct_count_tokens_agg_once_if_needed(db: &Db) -> Result<bool, String> {
    if let Ok(Some(v)) = get_setting(db, "stats", "agg_count_tokens_excluded_v1").await
        && v == serde_json::Value::Bool(true) {
            return Ok(false);
        }
    let __db_caller = std::panic::Location::caller();
    // stats-agg-to-main-db s4：跨库两阶段——proxy_log 在 log.db，stats_agg_hourly 在主库，
    // 禁同闭包跨库读。① log.db 读池跑聚合；② 主库写槽 upsert + 删孤儿。
    let auto_map = db
        .call_read_platform_traced(None, __db_caller, |conn| load_auto_from_map(conn).map_err(|e| tokio_rusqlite::Error::Other(e.into())))
        .await
        .map_err(|e| format!("correct count_tokens agg load auto_map: {e}"))?;
    // ① log.db 读池：不含 count_tokens 的真值聚合（aggregate_proxy_logs 已过滤 count_tokens）。
    let agg = db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            aggregate_proxy_logs(conn, &auto_map).map_err(|e| tokio_rusqlite::Error::Other(e.into()))
        })
        .await
        .map_err(|e| format!("correct count_tokens agg aggregate: {e}"))?;
    // ② 主库写槽：覆盖写 + 删孤儿桶（纯 count_tokens 贡献，扣净后应为 0）。
    db.call_traced(None, __db_caller, move |conn| {
        let now = chrono::Utc::now().timestamp_millis();
        // ① 覆盖写有 backing 行的桶。
        upsert_aggregated(conn, &agg, now)?;
        // ② 删孤儿桶（stats_agg 有但过滤后聚合无 → 纯 count_tokens 贡献，扣净后应为 0）。
        let keep: std::collections::HashSet<(String, String, String, i64)> = agg
            .keys()
            .map(|k| (k.time_hour.clone(), k.model.clone(), k.group_key.clone(), k.platform_id))
            .collect();
        let mut existing: Vec<(String, String, String, i64)> = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT time_hour, model, group_key, platform_id FROM stats_agg_hourly",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?;
            for r in rows {
                existing.push(r?);
            }
        }
        let mut del = conn.prepare(
            "DELETE FROM stats_agg_hourly \
             WHERE time_hour = ?1 AND model = ?2 AND group_key = ?3 AND platform_id = ?4",
        )?;
        for k in &existing {
            if !keep.contains(k) {
                del.execute(params![k.0, k.1, k.2, k.3])?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("correct count_tokens agg: {e}"))?;
    set_setting(
        db,
        SetSettingInput {
            scope: "stats".into(),
            key: "agg_count_tokens_excluded_v1".into(),
            value: serde_json::Value::Bool(true),
        },
    )
    .await?;
    Ok(true)
}

/// 按 retention_days 硬删过期聚合行（参考 cleanup_proxy_logs；0=永久保留）。
/// 截止时间为 UTC ms；与 time_hour 文本桶比较走 created_at 列（行写入时间）。
/// stats_agg_hourly 已迁回主库：走主库写槽 `call_traced`。
#[track_caller]
pub fn cleanup_stats_agg(db: &Db, retention_days: u32) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM stats_agg_hourly WHERE created_at < ?1", params![cutoff])?;
            incremental_vacuum_conn(conn, 100);
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup stats agg: {e}"))
    }
}

#[cfg(test)]
mod test_count_tokens_exclusion {
    use super::aggregate_proxy_logs;
    use rusqlite::Connection;

    /// 建最小 proxy_log + group 表（aggregate_proxy_logs 依赖 load_auto_from_map 读 "group"）。
    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE \"group\" (group_key TEXT, auto_from_platform TEXT, deleted_at INTEGER DEFAULT 0);
             CREATE TABLE proxy_log (
                created_at INTEGER, model TEXT, actual_model TEXT, group_key TEXT,
                platform_id INTEGER, status_code INTEGER, input_tokens INTEGER,
                output_tokens INTEGER, cache_tokens INTEGER, est_cost REAL,
                duration_ms INTEGER, request_url TEXT, deleted_at INTEGER DEFAULT 0
             );",
        )
        .unwrap();
        conn
    }

    fn insert(conn: &Connection, url: &str, cost: f64, input: i64) {
        conn.execute(
            "INSERT INTO proxy_log (created_at, model, actual_model, group_key, platform_id, \
             status_code, input_tokens, output_tokens, cache_tokens, est_cost, duration_ms, request_url) \
             VALUES (?1,'m','m','gk',1,200,?2,0,0,?3,10,?4)",
            rusqlite::params![1_700_000_000_000i64, input, cost, url],
        )
        .unwrap();
    }

    /// 关键 P0：count_tokens 行（request_url LIKE %count_tokens%）的 est_cost/input_tokens
    /// 不进 aggregate_proxy_logs 聚合；普通 chat 行照常计入。
    #[test]
    fn count_tokens_rows_excluded_from_aggregation() {
        let conn = setup();
        insert(&conn, "/glm-auto/v1/messages", 1.5, 100); // 普通对话 → 计入
        insert(&conn, "/glm-auto/v1/messages/count_tokens", 99.0, 9999); // count_tokens → 排除
        insert(&conn, "/proxy/v1/messages/count_tokens/", 50.0, 5000); // 容尾斜杠 → 排除

        let agg = aggregate_proxy_logs(&conn, &std::collections::HashMap::new()).unwrap();
        let total_cost: f64 = agg.values().map(|b| b.sum_est_cost).sum();
        let total_input: i64 = agg.values().map(|b| b.sum_input_tokens).sum();
        let total_reqs: i64 = agg.values().map(|b| b.request_count).sum();

        // 仅普通对话计入：cost=1.5、input=100、1 条请求；99/50 cost 与 9999/5000 tokens 全被滤掉。
        assert_eq!(total_reqs, 1, "只有 1 条普通对话应计入");
        assert!((total_cost - 1.5).abs() < 1e-9, "count_tokens cost 必须被排除");
        assert_eq!(total_input, 100, "count_tokens input_tokens 必须被排除");
    }
}

