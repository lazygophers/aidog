//! 会话归组聚合查询（magpie 票 08）：按 `proxy_log.session_key` 聚合近 N 天
//! 请求数 / token / est_cost，标题 = 会话首条有 request_body 的请求前 60 字符。
//! proxy_log 在 log.db：读一律 `call_read_proxy_log_traced`（db-pool-routing.md）。

use aidog_db::models::{SessionStats, SESSION_TITLE_MAX_CHARS};
use aidog_db::Db;

/// 窗口内聚合行数上限（按最近活动排序；防超长会话列表撑爆 UI）。
const SESSION_STATS_LIMIT: usize = 200;

/// 默认统计窗口（天）
const DEFAULT_DAYS: u32 = 7;

#[track_caller]
pub fn get_session_stats(
    db: &Db,
    days: Option<u32>,
) -> impl std::future::Future<Output = Result<Vec<SessionStats>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    let days = days.unwrap_or(DEFAULT_DAYS).clamp(1, 90);
    async move {
        // cutoff 毫秒（proxy_log.created_at 单位 = 毫秒）
        let cutoff = aidog_db::now() - days as i64 * 86_400_000;
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            // ① 按 session_key 聚合（空键不参与，spec D4）；最近活动优先
            let mut stmt = conn.prepare_cached(
                "SELECT session_key, COUNT(*), SUM(input_tokens), SUM(output_tokens), \
                 SUM(cache_tokens), SUM(est_cost), MIN(created_at), MAX(created_at) \
                 FROM proxy_log \
                 WHERE deleted_at = 0 AND session_key != '' AND created_at >= ?1 \
                 GROUP BY session_key \
                 ORDER BY MAX(created_at) DESC \
                 LIMIT ?2",
            )?;
            let limit = SESSION_STATS_LIMIT as i64;
            let mut rows = stmt.query_map(
                rusqlite::params![cutoff, limit],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                        row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            )?;
            let mut stats: Vec<SessionStats> = Vec::new();
            for r in &mut rows {
                let (session_key, requests, input_tokens, output_tokens, cache_tokens, est_cost, first_created_at, last_created_at) = r?;
                stats.push(SessionStats {
                    session_key,
                    title: String::new(),
                    requests,
                    input_tokens,
                    output_tokens,
                    cache_tokens,
                    est_cost,
                    first_created_at,
                    last_created_at,
                });
            }
            drop(rows);
            drop(stmt);
            // ② 标题：每会话首条 request_body != '' 的请求，取前 60 字符
            let keys: Vec<&str> = stats.iter().map(|s| s.session_key.as_str()).collect();
            if !keys.is_empty() {
                let placeholders: Vec<String> =
                    (0..keys.len()).map(|i| format!("?{}", i + 1)).collect();
                let sql = format!(
                    "SELECT session_key, request_body FROM proxy_log \
                     WHERE deleted_at = 0 AND session_key IN ({}) AND request_body != '' \
                     ORDER BY created_at ASC",
                    placeholders.join(",")
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(keys.len());
                for k in &keys {
                    params.push(rusqlite::types::Value::Text((*k).to_string()));
                }
                let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;
                // first-match wins：created_at ASC + 遍历顺序，每会话第一条即标题
                use std::collections::HashMap;
                let mut titles: HashMap<String, String> = HashMap::new();
                while let Some(row) = rows.next()? {
                    let key: String = row.get(0)?;
                    if titles.contains_key(&key) {
                        continue;
                    }
                    let body: String = row.get(1)?;
                    let title: String = body.chars().take(SESSION_TITLE_MAX_CHARS).collect();
                    titles.insert(key, title);
                }
                for s in stats.iter_mut() {
                    if let Some(t) = titles.get(&s.session_key) {
                        s.title = t.clone();
                    }
                }
            }
            Ok(stats)
        })
        .await
        .map_err(|e| format!("session stats agg: {e}"))
    }
}
