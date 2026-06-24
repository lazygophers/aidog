use super::*;
use rusqlite::{params};

/// eff_pid 回溯表达式：platform_id=0 经 group.auto_from_platform 回溯到源平台 id，否则原 platform_id。
/// 与 today_platform_stats / platform_usage_stats_all 同语义；join 键为 g.group_key = proxy_log.group_key。
const AGG_EFF_PID_EXPR: &str = "CASE WHEN platform_id = 0 THEN COALESCE(\
(SELECT CAST(g.auto_from_platform AS INTEGER) FROM \"group\" g \
 WHERE g.group_key = proxy_log.group_key AND g.auto_from_platform != '' AND g.deleted_at = 0 LIMIT 1), 0)\
ELSE platform_id END";

/// 从 proxy_log 重建 stats_agg_hourly 的 UPSERT SQL（rebuild 与回填共用语义）。
/// 不带空表守卫；本地小时桶 + actual_model 优先 + eff_pid 回溯 + deleted_at=0。
/// 冲突键 (time_hour,model,group_key,platform_id) 命中时用 excluded 真值【覆盖】（非累加：
/// SELECT 已是全量 COUNT/SUM 真值，累加会翻倍）；created_at 不写（保留旧行首次创建时间）、deleted_at 不动。
/// proxy_log 已无对应行的旧聚合数据【保留】（不再被清空）。
fn agg_rebuild_insert_sql() -> String {
    format!(
        "INSERT INTO stats_agg_hourly \
         (time_hour, model, group_key, platform_id, \
          request_count, success_count, error_count, \
          sum_input_tokens, sum_output_tokens, sum_cache_tokens, \
          sum_est_cost, sum_duration_ms, created_at, updated_at, deleted_at) \
         SELECT \
           strftime('%Y-%m-%d %H:00:00', created_at/1000, 'unixepoch', 'localtime') AS time_hour, \
           CASE WHEN actual_model != '' THEN actual_model ELSE model END AS model, \
           group_key, \
           {AGG_EFF_PID_EXPR} AS eff_pid, \
           COUNT(*), \
           SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
           SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), \
           COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0), COALESCE(SUM(cache_tokens), 0), \
           COALESCE(SUM(est_cost), 0.0), COALESCE(SUM(duration_ms), 0), \
           ?1, ?1, 0 \
         FROM proxy_log WHERE deleted_at = 0 \
         GROUP BY 1, 2, 3, 4 \
         ON CONFLICT(time_hour, model, group_key, platform_id) DO UPDATE SET \
          request_count = excluded.request_count, \
          success_count = excluded.success_count, \
          error_count = excluded.error_count, \
          sum_input_tokens = excluded.sum_input_tokens, \
          sum_output_tokens = excluded.sum_output_tokens, \
          sum_cache_tokens = excluded.sum_cache_tokens, \
          sum_est_cost = excluded.sum_est_cost, \
          sum_duration_ms = excluded.sum_duration_ms, \
          updated_at = excluded.updated_at"
    )
}

/// 一条终态请求的聚合增量入参（写入 stats_agg_hourly 的 UPSERT 源）。
/// eff_pid（platform_id=0 的 auto 回溯）在 UPSERT SQL 内用 CASE 子查询算，调用方传原始 platform_id。
#[derive(Debug, Clone)]
pub struct StatsAggInput {
    /// 创建时间（UTC ms），用于在 SQL 内算本地小时桶。
    pub created_at: i64,
    /// 模型名（调用方已取 actual_model 非空否则 model）。
    pub model: String,
    pub group_key: String,
    /// 原始 platform_id（=0 时 SQL 内经 group.auto_from_platform 回溯到 eff_pid）。
    pub platform_id: i64,
    pub status_code: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_tokens: i64,
    pub est_cost: f64,
    pub duration_ms: i64,
}

/// 对一条终态请求按 (time_hour,model,group_key,platform_id) UPSERT 进 stats_agg_hourly。
/// 写入【不受日志开关影响】：proxy 终态路径无条件调用。失败非致命（调用方 warn 不中断请求）。
#[track_caller]
pub fn upsert_stats_agg(db: &Db, input: StatsAggInput) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            let now = chrono::Utc::now().timestamp_millis();
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
                  ?2, ?3, \
                  CASE WHEN ?4 = 0 THEN COALESCE( \
                    (SELECT CAST(g.auto_from_platform AS INTEGER) FROM \"group\" g \
                     WHERE g.group_key = ?3 AND g.auto_from_platform != '' AND g.deleted_at = 0 LIMIT 1), 0) \
                  ELSE ?4 END, \
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
                    input.platform_id,
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
#[track_caller]
pub fn rebuild_stats_agg_from_logs(db: &Db) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            let now = chrono::Utc::now().timestamp_millis();
            conn.execute(&agg_rebuild_insert_sql(), params![now])?;
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
    if let Ok(Some(v)) = get_setting(db, "stats", "agg_rebuild_v1").await {
        if v == serde_json::Value::Bool(true) {
            return Ok(false);
        }
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

/// 按 retention_days 硬删过期聚合行（参考 cleanup_proxy_logs；0=永久保留）。
/// 截止时间为 UTC ms；与 time_hour 文本桶比较走 created_at 列（行写入时间）。
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

