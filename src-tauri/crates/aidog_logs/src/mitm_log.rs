//! mitm_log / oauth_usage_sample 表读写（cc-sub-mitm 票 07 建表、票 10 写入层）。
//!
//! mitm_log：MITM 解密旁路流量观测行（host/path/bytes/状态码 + 开关内 body），不进
//! proxy_log（防遥测行污染统计，票 10 目标）。写入方：`aidog_core::gateway::proxy::
//! mitm_bypass`（serve_plaintext 分流）。body gate 在写入方（`enabled &&
//! log_upstream_request`；`platform.claude.com/v1/oauth/token` 路径恒空——含长期凭证，
//! body 永不落库），本模块只管落库 / retention / 观测查询。
//!
//! retention 对齐 proxy_log 语义（spec §3.2）：body 两列按 `upstream_request_retention_days`
//! 对称清空（SET '' 不删行），整行按 `retention_days` 删——挂在
//! `run_retention_cleanup` 同一清理链。

use aidog_db::models::RetentionUnit;
use aidog_db::{Db, incremental_vacuum_conn, now, retention_cutoff_secs};
use rusqlite::params;

/// mitm_log 一行（一次性终态 INSERT，无 upsert——表无 request id 列）。
/// Default 仅为流式 guard 的 `mem::take` 兜底形状，无业务语义。
#[derive(Debug, Clone, Default)]
pub struct MitmLogInsert {
    pub group_name: String,
    pub host: String,
    pub path: String,
    pub status_code: i32,
    pub req_bytes: i64,
    pub resp_bytes: i64,
    pub decrypted: bool,
    pub request_body: String,
    pub response_body: String,
    pub created_at: i64,
}

/// 插入一行 mitm_log（旁路 / OAuth 观测行）。
#[track_caller]
pub fn insert_mitm_log(
    db: &Db,
    row: MitmLogInsert,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO mitm_log (group_name, host, path, status_code, req_bytes, resp_bytes, decrypted, request_body, response_body, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    row.group_name,
                    row.host,
                    row.path,
                    row.status_code,
                    row.req_bytes,
                    row.resp_bytes,
                    row.decrypted as i64,
                    row.request_body,
                    row.response_body,
                    row.created_at,
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("insert mitm_log: {e}"))
    }
}

/// 蹭 `GET /api/oauth/usage` 自然流量采样：插入一条 5h/7d 窗口利用率采样点。
/// `raw` 存响应 JSON 原文（口径核对用）；`five_hour_pct` / `seven_day_pct` 为 0-100 百分数
/// （上游 `utilization` 即百分数，gridbash usage.rs 直接 `100 - used_percent` 无换算）。
#[track_caller]
pub fn insert_oauth_usage_sample<'a>(
    db: &'a Db,
    group_name: &str,
    five_hour_pct: f64,
    seven_day_pct: f64,
    raw: &str,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    let group = group_name.to_string();
    let raw = raw.to_string();
    async move {        db.call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO oauth_usage_sample (group_name, five_hour_pct, seven_day_pct, raw, sampled_at)
                 VALUES (?1,?2,?3,?4,?5)",
                params![group, five_hour_pct, seven_day_pct, raw, now()],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("insert oauth_usage_sample: {e}"))
    }
}

/// OAuth token 刷新观测统计（票 12 UI 警示消费；`platform.claude.com/v1/oauth/token`
/// 的 mitm_log 行按状态码聚合）。
///
/// 返回 `(attempts, failures)`：attempts = since_ms 起该路径全部观测行（含上游错误 502），
/// failures = 其中 status_code >= 400 的行。`since_ms=0` 查全量。
pub fn oauth_token_refresh_failures(
    db: &Db,
    since_ms: i64,
) -> impl std::future::Future<Output = Result<(i64, i64), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_traced(None, __db_caller, move |conn| {
            let (attempts, failures) = conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END), 0)
                 FROM mitm_log
                 WHERE host = 'platform.claude.com' AND path = '/v1/oauth/token' AND created_at >= ?1",
                params![since_ms],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )?;
            Ok((attempts, failures))
        })
        .await
        .map_err(|e| format!("oauth_token_refresh_failures: {e}"))
    }
}

/// 对称清空超期 body 两列（同 `cleanup_upstream_request_fields` 语义：SET '' 不删行）。
pub fn cleanup_mitm_log_bodies(
    db: &Db,
    value: u32,
    unit: RetentionUnit,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let Some(cutoff) = retention_cutoff_secs(unit.secs(value)) else {
            return Ok(());
        };
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE mitm_log SET request_body = '', response_body = ''
                 WHERE created_at < ?1 AND (request_body != '' OR response_body != '')",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup mitm_log bodies: {e}"))
    }
}

/// 删超期整行（同 `cleanup_proxy_logs` 语义：硬删 + incremental vacuum）。
pub fn cleanup_mitm_logs(
    db: &Db,
    value: u32,
    unit: RetentionUnit,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let Some(cutoff) = retention_cutoff_secs(unit.secs(value)) else {
            return Ok(());
        };
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM mitm_log WHERE created_at < ?1", params![cutoff])?;
            incremental_vacuum_conn(conn, 100);
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup mitm_log rows: {e}"))
    }
}
