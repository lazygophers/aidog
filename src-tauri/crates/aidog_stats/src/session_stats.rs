//! 会话归组聚合查询（magpie 票 08）：按 `proxy_log.session_key` 聚合近 N 天
//! 请求数 / token / est_cost，标题 = 会话首条有 request_body 的请求前 60 字符。
//! proxy_log 在 log.db：读一律 `call_read_proxy_log_traced`（db-pool-routing.md）。
//! issue #66：可选 clients 过滤；会话 client = 请求数最多者，session_id 跟随该客户端。

use aidog_db::models::{SessionStats, SESSION_TITLE_MAX_CHARS};
use aidog_db::Db;
use std::collections::HashMap;

/// 窗口内聚合行数上限（按最近活动排序；防超长会话列表撑爆 UI）。
const SESSION_STATS_LIMIT: usize = 200;

/// 默认统计窗口（天）
const DEFAULT_DAYS: u32 = 7;

#[track_caller]
pub fn get_session_stats(
    db: &Db,
    days: Option<u32>,
    clients: Option<Vec<String>>,
) -> impl std::future::Future<Output = Result<Vec<SessionStats>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    let days = days.unwrap_or(DEFAULT_DAYS).clamp(1, 90);
    async move {
        // cutoff 毫秒（proxy_log.created_at 单位 = 毫秒）
        let cutoff = aidog_db::now() - days as i64 * 86_400_000;
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            // 客户端过滤（issue #66）：空 / None = 全部。SQL IN 下推到三段查询。
            let clients = clients.filter(|c| !c.is_empty());
            let client_where = clients
                .as_ref()
                .map(|c| {
                    let placeholders: Vec<String> =
                        (0..c.len()).map(|i| format!("?{}", i + 2)).collect();
                    format!(" AND client IN ({})", placeholders.join(","))
                })
                .unwrap_or_default();
            let cw = client_where;
            // LIMIT 占位下标：?1=cutoff，client 占位从 ?2 起，LIMIT 排在其后。
            let limit_idx = clients.as_ref().map_or(2, |c| c.len() + 2);
            // ① 按 session_key 聚合（空键不参与，spec D4）；最近活动优先
            let base_sql = format!(
                "SELECT session_key, COUNT(*), SUM(input_tokens), SUM(output_tokens), \
                 SUM(cache_tokens), SUM(est_cost), MIN(created_at), MAX(created_at) \
                 FROM proxy_log \
                 WHERE deleted_at = 0 AND session_key != '' AND created_at >= ?1{cw} \
                 GROUP BY session_key \
                 ORDER BY MAX(created_at) DESC \
                 LIMIT ?{limit_idx}",
            );
            let mut stmt = conn.prepare_cached(&base_sql)?;
            let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(
                2 + clients.as_ref().map_or(0, |c| c.len()),
            );
            params.push(rusqlite::types::Value::Integer(cutoff));
            if let Some(ref c) = clients {
                params.extend(c.iter().map(|v| rusqlite::types::Value::Text(v.clone())));
            }
            params.push(rusqlite::types::Value::Integer(SESSION_STATS_LIMIT as i64));
            let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;
            let mut stats: Vec<SessionStats> = Vec::new();
            while let Some(r) = rows.next()? {
                let session_key: String = r.get(0)?;
                let requests: i64 = r.get(1)?;
                let input_tokens: i64 = r.get::<_, Option<i64>>(2)?.unwrap_or(0);
                let output_tokens: i64 = r.get::<_, Option<i64>>(3)?.unwrap_or(0);
                let cache_tokens: i64 = r.get::<_, Option<i64>>(4)?.unwrap_or(0);
                let est_cost: f64 = r.get::<_, Option<f64>>(5)?.unwrap_or(0.0);
                let first_created_at: i64 = r.get(6)?;
                let last_created_at: i64 = r.get(7)?;
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
                    group_key: String::new(),
                    client: String::new(),
                    session_id: String::new(),
                });
            }
            drop(rows);
            drop(stmt);
            let keys: Vec<String> = stats.iter().map(|s| s.session_key.clone()).collect();
            if keys.is_empty() {
                return Ok(stats);
            }
            // ② 行级维度（issue #66）：按 created_at ASC 遍历，逐会话累计
            //    每客户端请求数（dominant = 最多者，并列取末条请求的客户端）、
            //    该客户端最新非空 session_id、末条 group_key。
            {
                let placeholders: Vec<String> =
                    (0..keys.len()).map(|i| format!("?{}", i + 1)).collect();
                let client_idx = keys.len() + 1;
                let cw = clients
                    .as_ref()
                    .map(|c| {
                        let ph: Vec<String> =
                            (0..c.len()).map(|i| format!("?{}", client_idx + 1 + i)).collect();
                        format!(" AND client IN ({})", ph.join(","))
                    })
                    .unwrap_or_default();
                let sql = format!(
                    "SELECT session_key, group_key, client, session_id FROM proxy_log \
                     WHERE deleted_at = 0 AND session_key IN ({}) AND created_at >= ?{client_idx}{cw} \
                     ORDER BY created_at ASC",
                    placeholders.join(",")
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(
                    keys.len() + 1 + clients.as_ref().map_or(0, |c| c.len()),
                );
                for k in &keys {
                    params.push(rusqlite::types::Value::Text(k.clone()));
                }
                params.push(rusqlite::types::Value::Integer(cutoff));
                if let Some(ref c) = clients {
                    params.extend(c.iter().map(|v| rusqlite::types::Value::Text(v.clone())));
                }
                let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;
                #[derive(Default)]
                struct Cc {
                    count: i64,
                    seq: usize,
                    sid: String, // 该客户端最新非空 session_id
                }
                let mut seq = 0usize;
                let mut by_session: HashMap<String, HashMap<String, Cc>> = HashMap::new();
                let mut last_group: HashMap<String, String> = HashMap::new();
                let mut any_sid: HashMap<String, String> = HashMap::new();
                while let Some(row) = rows.next()? {
                    let key: String = row.get(0)?;
                    let gk: String = row.get(1)?;
                    let cl: String = row.get(2)?;
                    let sid: String = row.get(3)?;
                    seq += 1;
                    last_group.insert(key.clone(), gk);
                    let cc = by_session
                        .entry(key.clone())
                        .or_default()
                        .entry(cl.clone())
                        .or_default();
                    cc.count += 1;
                    cc.seq = seq;
                    if !sid.is_empty() {
                        cc.sid = sid.clone();
                        any_sid.insert(key.clone(), sid);
                    }
                }
                for s in stats.iter_mut() {
                    let Some(cmap) = by_session.get(&s.session_key) else { continue };
                    // dominant：请求数最多；并列取末条请求所属客户端（seq 最大）
                    if let Some((dominant, cc)) = cmap
                        .iter()
                        .max_by(|a, b| a.1.count.cmp(&b.1.count).then(a.1.seq.cmp(&b.1.seq)))
                    {
                        s.client = dominant.clone();
                        s.session_id = if cc.sid.is_empty() {
                            any_sid.get(&s.session_key).cloned().unwrap_or_default()
                        } else {
                            cc.sid.clone()
                        };
                    }
                    if let Some(gk) = last_group.get(&s.session_key) {
                        s.group_key = gk.clone();
                    }
                }
            }
            // ③ 标题：每会话首条 request_body != '' 的请求，取前 60 字符
            {
                let placeholders: Vec<String> =
                    (0..keys.len()).map(|i| format!("?{}", i + 1)).collect();
                let client_idx = keys.len() + 1;
                let cw = clients
                    .as_ref()
                    .map(|c| {
                        let ph: Vec<String> =
                            (0..c.len()).map(|i| format!("?{}", client_idx + 1 + i)).collect();
                        format!(" AND client IN ({})", ph.join(","))
                    })
                    .unwrap_or_default();
                let sql = format!(
                    "SELECT session_key, request_body FROM proxy_log \
                     WHERE deleted_at = 0 AND session_key IN ({}) AND created_at >= ?{client_idx} \
                     AND request_body != ''{cw} \
                     ORDER BY created_at ASC",
                    placeholders.join(",")
                );
                let mut stmt = conn.prepare_cached(&sql)?;
                let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(
                    keys.len() + 1 + clients.as_ref().map_or(0, |c| c.len()),
                );
                for k in &keys {
                    params.push(rusqlite::types::Value::Text((*k).to_string()));
                }
                params.push(rusqlite::types::Value::Integer(cutoff));
                if let Some(ref c) = clients {
                    params.extend(c.iter().map(|v| rusqlite::types::Value::Text(v.clone())));
                }
                let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;
                // first-match wins：created_at ASC + 遍历顺序，每会话第一条即标题
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
