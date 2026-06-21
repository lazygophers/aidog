use super::*;
use rusqlite::{params, OptionalExtension, Result as SqlResult};

#[track_caller]
pub fn get_setting<'a>(
    db: &'a Db,
    scope: &'a str,
    key: &'a str,
) -> impl std::future::Future<Output = Result<Option<serde_json::Value>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    // 缓存命中：热路径（log_settings/lang/sync_settings 每请求多次读）走内存，绕过后台线程往返。
    // 命中路径零分配：借 `(&str, &str)` 经 `dyn KeyPair` 探测 map，不构造 `(String, String)`。
    {
        let probe: &dyn KeyPair = &(scope, key);
        if let Ok(g) = db.1.settings.read() {
            if let Some(hit) = g.get(probe) {
                return Ok(hit.clone());
            }
        }
    }
    let scope = scope.to_string();
    let key = key.to_string();
    let result = db
        
        .call_traced(None, __db_caller, {
            let scope = scope.clone();
            let key = key.clone();
            move |conn| {
                let mut stmt = conn.prepare("SELECT value FROM setting WHERE scope = ?1 AND key = ?2 AND deleted_at = 0")?;
                stmt.query_row(params![scope, key], |row| {
                    let v: String = row.get(0)?;
                    Ok(serde_json::from_str(&v).unwrap_or_else(|e| {
                        tracing::warn!(scope = %scope, key = %key, error = %e, "stored setting value is not valid JSON, returning Null");
                        serde_json::Value::Null
                    }))
                })
                .optional()
                .map_err(tokio_rusqlite::Error::from)
            }
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Ok(mut g) = db.1.settings.write() {
        g.insert((scope, key), result.clone());
    }
    Ok(result)
    }
}

#[track_caller]
pub fn set_setting(db: &Db, input: SetSettingInput) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    let value_str =
        serde_json::to_string(&input.value).map_err(|e| format!("serialize setting: {e}"))?;
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO setting (scope, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(scope, key) DO UPDATE SET value = ?3, updated_at = ?4, deleted_at = 0",
                params![input.scope, input.key, value_str, ts],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert setting: {e}"))?;
    db.invalidate_settings_cache();
    Ok(())
    }
}

#[track_caller]
pub fn delete_setting<'a>(db: &'a Db, scope: &'a str, key: &'a str) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let scope = scope.to_string();
    let key = key.to_string();
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE setting SET deleted_at = ?1 WHERE scope = ?2 AND key = ?3",
                params![now(), scope, key],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete setting: {e}"))?;
    db.invalidate_settings_cache();
    Ok(())
    }
}

#[track_caller]
pub fn list_setting_keys<'a>(db: &'a Db, scope: &'a str) -> impl std::future::Future<Output = Result<Vec<String>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let scope = scope.to_string();
    db
        .call_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare("SELECT key FROM setting WHERE scope = ?1 AND deleted_at = 0 ORDER BY key")?;
            let rows = stmt.query_map(params![scope], |row| row.get(0))?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 导入导出用：列出全部未删除 setting 原始行（scope, key, value_json）。
#[track_caller]
pub fn list_all_settings_raw(db: &Db) -> impl std::future::Future<Output = Result<Vec<(String, String, String)>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT scope, key, value FROM setting WHERE deleted_at = 0 ORDER BY scope, key",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 导入导出用：列出 group→platform 全部关联（按名称解析，跨机迁移友好）。
#[track_caller]
pub fn list_all_group_platform_pairs(
    db: &Db,
) -> impl std::future::Future<Output = Result<Vec<(String, String)>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT g.name, p.name FROM group_platform gp
                 JOIN \"group\" g ON g.id = gp.group_id
                 JOIN platform p ON p.id = gp.platform_id
                 WHERE gp.deleted_at = 0 ORDER BY g.name, p.name",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

// ─── Middleware Rule CRUD (C1 基座) ────────────────────────

