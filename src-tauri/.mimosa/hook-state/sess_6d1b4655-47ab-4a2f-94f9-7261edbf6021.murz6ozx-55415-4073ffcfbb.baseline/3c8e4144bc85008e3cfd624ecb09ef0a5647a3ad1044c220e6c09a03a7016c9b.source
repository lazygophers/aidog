//! `platform_health_state` 表读写：熔断 Open / quota / auth 冷却截止 / connect 失败标记
//! 的持久化（routing-health-optim R4，2026-09-28）。代理重启时由 aidog_core 调度器加载
//! 恢复，过期即弃——重启后死站不再回满血候选。写穿（write-through）由调度器状态变更点
//! fire-and-forget 触发，本模块只管表行。

use crate::*;
use rusqlite::params;

/// 单平台持久化健康态行。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlatformHealthStateRow {
    pub platform_id: i64,
    /// "open"（熔断打开）| "closed"。HalfOpen 不落盘：重启视作已过期回 Closed。
    pub breaker_state: String,
    pub breaker_until_ms: i64,
    pub quota_cooldown_until_ms: i64,
    pub auth_cooldown_until_ms: i64,
    pub last_connect_fail_ms: i64,
    pub updated_at: i64,
}

/// 写穿一行（INSERT OR REPLACE，整行覆盖；状态清除时同样用本函数写回全零）。
#[track_caller]
pub fn upsert_platform_health_state(
    db: &Db,
    row: PlatformHealthStateRow,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_platform_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO platform_health_state \
                 (platform_id, breaker_state, breaker_until_ms, quota_cooldown_until_ms, \
                  auth_cooldown_until_ms, last_connect_fail_ms, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(platform_id) DO UPDATE SET \
                 breaker_state = excluded.breaker_state, \
                 breaker_until_ms = excluded.breaker_until_ms, \
                 quota_cooldown_until_ms = excluded.quota_cooldown_until_ms, \
                 auth_cooldown_until_ms = excluded.auth_cooldown_until_ms, \
                 last_connect_fail_ms = excluded.last_connect_fail_ms, \
                 updated_at = excluded.updated_at",
                params![
                    row.platform_id,
                    row.breaker_state,
                    row.breaker_until_ms,
                    row.quota_cooldown_until_ms,
                    row.auth_cooldown_until_ms,
                    row.last_connect_fail_ms,
                    row.updated_at
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert platform_health_state: {e}"))
    }
}

/// 读全部持久化行（启动恢复用；行数 = 平台数量级，全量拉取）。
#[track_caller]
pub fn load_platform_health_states(
    db: &Db,
) -> impl std::future::Future<Output = Result<Vec<PlatformHealthStateRow>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_platform_traced(None, __db_caller, |conn| {
            let mut stmt = conn.prepare(
                "SELECT platform_id, breaker_state, breaker_until_ms, quota_cooldown_until_ms, \
                 auth_cooldown_until_ms, last_connect_fail_ms, updated_at \
                 FROM platform_health_state",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(PlatformHealthStateRow {
                    platform_id: r.get(0)?,
                    breaker_state: r.get(1)?,
                    breaker_until_ms: r.get(2)?,
                    quota_cooldown_until_ms: r.get(3)?,
                    auth_cooldown_until_ms: r.get(4)?,
                    last_connect_fail_ms: r.get(5)?,
                    updated_at: r.get(6)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
        .await
        .map_err(|e| format!("load platform_health_state: {e}"))
    }
}
