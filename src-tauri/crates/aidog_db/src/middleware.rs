use super::*;
use rusqlite::{params, Result as SqlResult};

use crate::models::{CreateMiddlewareRule, MiddlewareRule, UpdateMiddlewareRule};

/// middleware_rule 全列序（INSERT 列子集 + SELECT 共用，与表定义列序一致）。
const MIDDLEWARE_RULE_COLUMNS: &str =
    "id, name, description, conditions, actions, applies_to, priority, enabled, is_builtin, failed, created_at, updated_at";

/// 从查询行构造 MiddlewareRule。JSON 列解析失败不在此兜底（迁移后 schema 保证可解析）；
/// 若手改 DB 产生坏行，list 时 serde 失败由 unwrap_or 兜底为 failed 标记（前端引导手删）。
fn row_to_middleware_rule(row: &rusqlite::Row) -> SqlResult<MiddlewareRule> {
    let conditions_json: String = row.get(3)?;
    let actions_json: String = row.get(4)?;
    let applies_json: String = row.get(5)?;
    let (conditions, json_failed) = serde_json::from_str(&conditions_json)
        .map(|c| (c, false))
        .unwrap_or_else(|e| {
            tracing::warn!("middleware rule {} bad conditions JSON: {e}", row.get::<_, i64>(0).unwrap_or(0));
            (default_failed_conditions(), true)
        });
    Ok(MiddlewareRule {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        conditions,
        actions: serde_json::from_str(&actions_json).unwrap_or_default(),
        applies_to: serde_json::from_str(&applies_json).unwrap_or_default(),
        priority: row.get(6)?,
        enabled: row.get::<_, i64>(7)? == 1,
        is_builtin: row.get::<_, i64>(8)? == 1,
        failed: row.get::<_, i64>(9)? == 1 || json_failed,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

/// 坏 JSON 行兜底条件树（Failed Rule：恒不命中，前端引导手删）。
fn default_failed_conditions() -> crate::models::ConditionNode {
    crate::models::ConditionNode::Leaf(crate::models::ConditionLeaf {
        target: crate::models::Target::RequestBody,
        field: String::new(),
        match_type: crate::models::MatchType::Exact,
        pattern: "\u{0}unparseable-rule".to_string(),
    })
}

/// 列出全部中间件规则（按 priority 升序，再 id 升序）。引擎 reload 与前端列表共用。
#[track_caller]
pub fn list_middleware_rules(db: &Db) -> impl std::future::Future<Output = Result<Vec<MiddlewareRule>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let sql = format!(
        "SELECT {MIDDLEWARE_RULE_COLUMNS} FROM middleware_rule ORDER BY priority ASC, id ASC"
    );
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], row_to_middleware_rule)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[track_caller]
pub fn create_middleware_rule(
    db: &Db,
    input: CreateMiddlewareRule,
) -> impl std::future::Future<Output = Result<MiddlewareRule, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    crate::models::validate_rule_phases(&input.conditions)?;
    let ts = now();
    let conditions = serde_json::to_string(&input.conditions).map_err(|e| e.to_string())?;
    let actions = serde_json::to_string(&input.actions).map_err(|e| e.to_string())?;
    let applies_to = serde_json::to_string(&input.applies_to).map_err(|e| e.to_string())?;
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO middleware_rule
                   (name, description, conditions, actions, applies_to, priority, enabled, is_builtin, failed, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?9)",
                params![
                    input.name,
                    input.description,
                    conditions,
                    actions,
                    applies_to,
                    input.priority,
                    if input.enabled { 1 } else { 0 },
                    if input.is_builtin { 1 } else { 0 },
                    ts,
                ],
            )?;
            let id = conn.last_insert_rowid();
            let mut stmt = conn.prepare(&format!(
                "SELECT {MIDDLEWARE_RULE_COLUMNS} FROM middleware_rule WHERE id = ?1"
            ))?;
            stmt.query_row(params![id], row_to_middleware_rule)
                .map_err(tokio_rusqlite::Error::from)
        })
        .await
        .map_err(|e| format!("create middleware rule: {e}"))
    }
}

#[track_caller]
pub fn update_middleware_rule(
    db: &Db,
    input: UpdateMiddlewareRule,
) -> impl std::future::Future<Output = Result<MiddlewareRule, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    crate::models::validate_rule_phases(&input.conditions)?;
    let ts = now();
    let conditions = serde_json::to_string(&input.conditions).map_err(|e| e.to_string())?;
    let actions = serde_json::to_string(&input.actions).map_err(|e| e.to_string())?;
    let applies_to = serde_json::to_string(&input.applies_to).map_err(|e| e.to_string())?;
    db
        .call_traced(None, __db_caller, move |conn| {
            // 内置规则只允许启停（票 02）：整体改写会破坏 seed 强制覆盖语义。
            let (is_builtin, old_name, old_desc, old_priority): (i64, String, String, i64) = conn.query_row(
                "SELECT is_builtin, name, description, priority FROM middleware_rule WHERE id = ?1",
                params![input.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
            if is_builtin == 1 {
                if old_name != input.name || old_desc != input.description || old_priority != input.priority {
                    return Err(tokio_rusqlite::Error::Other(
                        "builtin middleware rule only supports enable/disable (content managed by seed)".into(),
                    ));
                }
                let affected = conn.execute(
                    "UPDATE middleware_rule SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
                    params![input.id, if input.enabled { 1 } else { 0 }, ts],
                )?;
                if affected == 0 {
                    return Err(tokio_rusqlite::Error::Other(
                        format!("middleware rule {} not found", input.id).into(),
                    ));
                }
                let mut stmt = conn.prepare(&format!(
                    "SELECT {MIDDLEWARE_RULE_COLUMNS} FROM middleware_rule WHERE id = ?1"
                ))?;
                return stmt
                    .query_row(params![input.id], row_to_middleware_rule)
                    .map_err(tokio_rusqlite::Error::from);
            }
            let affected = conn.execute(
                "UPDATE middleware_rule SET
                   name = ?2, description = ?3, conditions = ?4, actions = ?5, applies_to = ?6,
                   priority = ?7, enabled = ?8, updated_at = ?9
                 WHERE id = ?1",
                params![
                    input.id,
                    input.name,
                    input.description,
                    conditions,
                    actions,
                    applies_to,
                    input.priority,
                    if input.enabled { 1 } else { 0 },
                    ts,
                ],
            )?;
            if affected == 0 {
                return Err(tokio_rusqlite::Error::Other(
                    format!("middleware rule {} not found", input.id).into(),
                ));
            }
            let mut stmt = conn.prepare(&format!(
                "SELECT {MIDDLEWARE_RULE_COLUMNS} FROM middleware_rule WHERE id = ?1"
            ))?;
            stmt.query_row(params![input.id], row_to_middleware_rule)
                .map_err(tokio_rusqlite::Error::from)
        })
        .await
        .map_err(|e| format!("update middleware rule: {e}"))
    }
}

/// 删除规则。内置规则不可删（只允许启停）；failed 内置行除外（失效残留，允许清走）。
#[track_caller]
pub fn delete_middleware_rule(db: &Db, id: i64) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            let (is_builtin, failed): (i64, i64) = conn.query_row(
                "SELECT is_builtin, failed FROM middleware_rule WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            ).map_err(tokio_rusqlite::Error::from)?;
            if is_builtin == 1 && failed == 0 {
                return Err(tokio_rusqlite::Error::Other(
                    "builtin middleware rule cannot be deleted (toggle enabled instead)".into(),
                ));
            }
            conn.execute("DELETE FROM middleware_rule WHERE id = ?1", params![id])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete middleware rule: {e}"))
    }
}

/// 读取中间件总设置（settings scope="middleware" key="settings"）。
/// 无记录或解析失败 → Default（总开关 ON）。C2/C3 执行层调用。
pub async fn get_middleware_settings(db: &Db) -> crate::models::MiddlewareSettings {
    match get_setting(db, "middleware", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => crate::models::MiddlewareSettings::default(),
    }
}

/// 全局调度 + 熔断默认设置（settings scope=`scheduling`, key=`settings`）。
/// 缺省 / 解析失败 → 默认值（5/1800/2，enabled=true，load_balance）。
pub async fn get_scheduling_settings(db: &Db) -> crate::models::SchedulingBreakerSettings {
    match get_setting(db, "scheduling", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => crate::models::SchedulingBreakerSettings::default(),
    }
}

// ─── Notification（N1 — 系统通知模块）──────────────────────

/// 通知设置（settings scope=`notification`, key=`settings`）。缺省 / 解析失败 → 默认（全开 CrossPlatform）。
pub async fn get_notification_settings(db: &Db) -> crate::models::NotificationSettings {
    match get_setting(db, "notification", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => crate::models::NotificationSettings::default(),
    }
}

/// 插入收件箱通知，返回新行 id。
#[track_caller]
pub fn insert_notification<'a>(
    db: &'a Db,
    notif_type: &'a str,
    title: &'a str,
    body: &'a str,
) -> impl std::future::Future<Output = Result<i64, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let notif_type = notif_type.to_string();
    let title = title.to_string();
    let body = body.to_string();
    let ts = now();
    db
        .call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO notification (notif_type, title, body, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![notif_type, title, body, ts],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .map_err(|e| format!("insert notification: {e}"))
    }
}

/// 列收件箱（按 created_at 倒序），limit 上限。
#[track_caller]
pub fn list_notifications(
    db: &Db,
    limit: i64,
) -> impl std::future::Future<Output = Result<Vec<crate::models::Notification>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, notif_type, title, body, created_at FROM notification ORDER BY created_at DESC, id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(crate::models::Notification {
                    id: row.get(0)?,
                    notif_type: row.get(1)?,
                    title: row.get(2)?,
                    body: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 清空收件箱（删全部行）。
#[track_caller]
pub fn clear_notifications(db: &Db) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_proxy_log_traced(None, __db_caller, |conn| {
            conn.execute("DELETE FROM notification", [])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("clear notifications: {e}"))
    }
}

/// 删除 N 天前的收件箱通知行。`retention_days == 0` → 跳过（永不清理）。
///
/// 硬删（`DELETE FROM`），非软删：notification 表无 deleted_at / tombstone 概念，
/// 抄 proxy_log retention 模式避 SQLite 体积单调增长（见记忆 db-volume-soft-delete-no-vacuum）。
/// 硬删后 `incremental_vacuum(100)` 回收 free pages（auto_vacuum != INCREMENTAL 时 no-op）。
#[track_caller]
pub fn cleanup_notifications(db: &Db, retention_days: u32) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db
        .call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM notification WHERE created_at < ?1", params![cutoff])?;
            incremental_vacuum_conn(conn, 100);
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup notifications: {e}"))
    }
}

// ─── ProxyLog CRUD ─────────────────────────────────────────

