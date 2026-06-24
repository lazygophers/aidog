use super::*;
use rusqlite::{params, Result as SqlResult};

use crate::gateway::models::{
    CreateMiddlewareRule, MatchType, MiddlewareRule, RuleAction, RuleScope, RuleType,
    UpdateMiddlewareRule,
};

/// middleware_rule 全列序（INSERT 列子集 + SELECT 共用，与表定义列序一致）。
const MIDDLEWARE_RULE_COLUMNS: &str =
    "id, name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at";

/// 从查询行构造 MiddlewareRule。未知 rule_type 不会出现在结果（行被 list 过滤前已按 from_db_str 处理）。
/// 此处 rule_type 用 from_db_str → 未知值兜底为 RequestFilter 会误导，故 list 时遇未知直接跳过（见 list_middleware_rules）。
fn row_to_middleware_rule(row: &rusqlite::Row) -> SqlResult<MiddlewareRule> {
    let rule_type_str: String = row.get(3)?;
    let scope_str: String = row.get(4)?;
    let match_type_str: String = row.get(6)?;
    let action_str: String = row.get(8)?;
    Ok(MiddlewareRule {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        // 未知 rule_type 极少（仅手改 DB）；兜底为 RequestFilter 不影响引擎（引擎按 from_db_str 分桶时同样会跳过未知）。
        rule_type: RuleType::from_db_str(&rule_type_str).unwrap_or(RuleType::RequestFilter),
        scope: RuleScope::from_db_str(&scope_str),
        scope_ref: row.get(5)?,
        match_type: MatchType::from_db_str(&match_type_str),
        pattern: row.get(7)?,
        action: RuleAction::from_db_str(&action_str),
        config: row.get(9)?,
        priority: row.get(10)?,
        enabled: row.get::<_, i64>(11)? == 1,
        is_builtin: row.get::<_, i64>(12)? == 1,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
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
    let ts = now();
    let rule_type = input.rule_type.as_str().to_string();
    let scope = input.scope.as_str().to_string();
    let match_type = input.match_type.as_str().to_string();
    let action = input.action.as_str().to_string();
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO middleware_rule
                   (name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
                params![
                    input.name,
                    input.description,
                    rule_type,
                    scope,
                    input.scope_ref,
                    match_type,
                    input.pattern,
                    action,
                    input.config,
                    input.priority,
                    if input.enabled { 1 } else { 0 },
                    if input.is_builtin { 1 } else { 0 },
                    ts,
                ],
            )?;
            let id = conn.last_insert_rowid();
            let mut stmt = conn.prepare(
                "SELECT id, name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at FROM middleware_rule WHERE id = ?1",
            )?;
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
    let ts = now();
    let rule_type = input.rule_type.as_str().to_string();
    let scope = input.scope.as_str().to_string();
    let match_type = input.match_type.as_str().to_string();
    let action = input.action.as_str().to_string();
    db
        .call_traced(None, __db_caller, move |conn| {
            let affected = conn.execute(
                "UPDATE middleware_rule SET
                   name = ?2, description = ?3, rule_type = ?4, scope = ?5, scope_ref = ?6,
                   match_type = ?7, pattern = ?8, action = ?9, config = ?10, priority = ?11,
                   enabled = ?12, is_builtin = ?13, updated_at = ?14
                 WHERE id = ?1",
                params![
                    input.id,
                    input.name,
                    input.description,
                    rule_type,
                    scope,
                    input.scope_ref,
                    match_type,
                    input.pattern,
                    action,
                    input.config,
                    input.priority,
                    if input.enabled { 1 } else { 0 },
                    if input.is_builtin { 1 } else { 0 },
                    ts,
                ],
            )?;
            if affected == 0 {
                return Err(tokio_rusqlite::Error::Other(
                    format!("middleware rule {} not found", input.id).into(),
                ));
            }
            let mut stmt = conn.prepare(
                "SELECT id, name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at FROM middleware_rule WHERE id = ?1",
            )?;
            stmt.query_row(params![input.id], row_to_middleware_rule)
                .map_err(tokio_rusqlite::Error::from)
        })
        .await
        .map_err(|e| format!("update middleware rule: {e}"))
    }
}

#[track_caller]
pub fn delete_middleware_rule(db: &Db, id: i64) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM middleware_rule WHERE id = ?1", params![id])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete middleware rule: {e}"))
    }
}

/// 读取中间件总设置（settings scope="middleware" key="settings"）。
/// 无记录或解析失败 → Default（总开关 ON，各类型默认启用）。C2/C3 执行层调用。
pub async fn get_middleware_settings(db: &Db) -> crate::gateway::models::MiddlewareSettings {
    match get_setting(db, "middleware", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => crate::gateway::models::MiddlewareSettings::default(),
    }
}

/// 全局调度 + 熔断默认设置（settings scope=`scheduling`, key=`settings`）。
/// 缺省 / 解析失败 → 默认值（5/1800/2，enabled=true，load_balance）。
pub async fn get_scheduling_settings(db: &Db) -> crate::gateway::models::SchedulingBreakerSettings {
    match get_setting(db, "scheduling", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => crate::gateway::models::SchedulingBreakerSettings::default(),
    }
}

// ─── Notification（N1 — 系统通知模块）──────────────────────

/// 通知设置（settings scope=`notification`, key=`settings`）。缺省 / 解析失败 → 默认（全开 CrossPlatform）。
pub async fn get_notification_settings(db: &Db) -> crate::gateway::models::NotificationSettings {
    match get_setting(db, "notification", "settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => crate::gateway::models::NotificationSettings::default(),
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
        .call_traced(None, __db_caller, move |conn| {
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
) -> impl std::future::Future<Output = Result<Vec<crate::gateway::models::Notification>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, notif_type, title, body, created_at FROM notification ORDER BY created_at DESC, id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(crate::gateway::models::Notification {
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
        .call_traced(None, __db_caller, |conn| {
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
        .call_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM notification WHERE created_at < ?1", params![cutoff])?;
            incremental_vacuum_conn(conn, 100);
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup notifications: {e}"))
    }
}

// ─── ProxyLog CRUD ─────────────────────────────────────────

