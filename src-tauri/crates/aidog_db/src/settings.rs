use crate::cache::KeyPair;
use crate::models::SetSettingInput;
use crate::{Db, now, platform_id_name_map};
use rusqlite::{OptionalExtension, Result as SqlResult, params};

/// db-field-encryption 票 03：纯 cc（订阅透传）组 HTTPS_PROXY 占位密码的 KV scope
/// （key = group_key，value = 密码串）。该 scope 的**值**静态加密落库（`enc1:` 前缀，
/// 键保持明文，其它 scope 一律不碰），收口在下方 set/get；单一真值源，
/// aidog_core 经 `aidog_db::CC_PROXY_AUTH_SCOPE` 引用。
pub const CC_PROXY_AUTH_SCOPE: &str = "cc_proxy_auth";

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
            if let Ok(g) = db.1.settings.read()
                && let Some(hit) = g.get(probe)
            {
                return Ok(hit.clone());
            }
        }
        let scope = scope.to_string();
        let key = key.to_string();
        let result = db
        
        .call_read_traced(None, __db_caller, {
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
        // db-field-encryption 票 03：该 scope 值透明解密（仅密文前缀命中才解，存量明文兼容；
        // 缓存 insert 在解密后，set/delete 已 invalidate，密文不会入缓存）。
        let result = match result {
            Some(serde_json::Value::String(s))
                if scope == CC_PROXY_AUTH_SCOPE && s.starts_with(crate::crypt::ENC_PREFIX) =>
            {
                let plain = crate::crypt::decrypt_field(&s)
                    .map_err(|e| format!("decrypt cc_proxy_auth setting: {e}"))?;
                Some(serde_json::Value::String(plain))
            }
            other => other,
        };
        if let Ok(mut g) = db.1.settings.write() {
            g.insert((scope, key), result.clone());
        }
        Ok(result)
    }
}

#[track_caller]
pub fn set_setting(
    db: &Db,
    input: SetSettingInput,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let ts = now();
        // db-field-encryption 票 03：cc_proxy_auth scope 值加密落库（键明文不动，其它 scope 不碰；
        // 空串保持空、已是密文形态防双重加密，见 encrypt_field）。
        let value = match (input.scope.as_str(), input.value.as_str()) {
            (CC_PROXY_AUTH_SCOPE, Some(s)) => serde_json::Value::String(
                crate::crypt::encrypt_field(s)
                    .map_err(|e| format!("encrypt cc_proxy_auth setting: {e}"))?,
            ),
            _ => input.value.clone(),
        };
        let value_str =
            serde_json::to_string(&value).map_err(|e| format!("serialize setting: {e}"))?;
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
pub fn delete_setting<'a>(
    db: &'a Db,
    scope: &'a str,
    key: &'a str,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let scope = scope.to_string();
        let key = key.to_string();
        db.call_traced(None, __db_caller, move |conn| {
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
pub fn list_setting_keys<'a>(
    db: &'a Db,
    scope: &'a str,
) -> impl std::future::Future<Output = Result<Vec<String>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let scope = scope.to_string();
        db.call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT key FROM setting WHERE scope = ?1 AND deleted_at = 0 ORDER BY key",
            )?;
            let rows = stmt.query_map(params![scope], |row| row.get(0))?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 导入导出用：列出全部未删除 setting 原始行（scope, key, value_json）。
#[track_caller]
pub fn list_all_settings_raw(
    db: &Db,
) -> impl std::future::Future<Output = Result<Vec<(String, String, String)>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT scope, key, value FROM setting WHERE deleted_at = 0 ORDER BY scope, key",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
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
        db.call_read_platform_traced(None, __db_caller, move |conn| {
            // 去双 JOIN：分别取 group/platform 的 id→name 全表映射 + group_platform 关联，
            // 内存按 group_id/platform_id 解析名称（任一端缺失则丢弃，等价旧 inner JOIN）。
            let group_names: std::collections::HashMap<i64, String> = {
                let mut stmt = conn.prepare("SELECT id, name FROM \"group\"")?;
                let rows = stmt
                    .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                    .collect::<SqlResult<Vec<_>>>()?;
                rows.into_iter().collect()
            };
            let platform_names = platform_id_name_map(conn)?;
            let mut stmt = conn
                .prepare("SELECT group_id, platform_id FROM group_platform WHERE deleted_at = 0")?;
            let pairs = stmt
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
                .collect::<SqlResult<Vec<_>>>()?;
            let mut out: Vec<(String, String)> = pairs
                .into_iter()
                .filter_map(
                    |(gid, pid)| match (group_names.get(&gid), platform_names.get(&pid)) {
                        (Some(g), Some(p)) => Some((g.clone(), p.clone())),
                        _ => None,
                    },
                )
                .collect();
            out.sort();
            Ok(out)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

// ─── Middleware Rule CRUD (C1 基座) ────────────────────────

/// db-field-encryption 票 03：存量明文 cc_proxy_auth 值迁移（init_tables_raw Phase 1 主库
/// conn 内调用，每次启动跑）。模式照抄票 01 `migrate_platform_api_keys`：幂等（密文行
/// 不再命中）、单行失败 warn 后继续明文保留下次启动重试、主密钥不可得整体跳过
/// （读侧明文兼容兜底，key 可得后的下次启动再迁）。
pub fn migrate_cc_proxy_auth(conn: &rusqlite::Connection) {
    let Ok(key) = crate::crypt::master_key() else {
        tracing::warn!("cc_proxy_auth migration skipped: master key unavailable");
        return;
    };
    // 列值 = JSON 串包裹（`"pw"`）；密文形态 `\"enc1:…\"` 与空串 `\"\"` 均跳过（后者无密可加，
    // 过滤掉可免每启动一次无意义改写）。
    let keys: Vec<String> = match conn
        .prepare(&format!(
            "SELECT key FROM setting WHERE scope = '{CC_PROXY_AUTH_SCOPE}' \
             AND value NOT LIKE '\"{}%' AND value != '\"\"' AND deleted_at = 0",
            crate::crypt::ENC_PREFIX
        ))
        .and_then(|mut s| s.query_map([], |r| r.get::<_, String>(0))?.collect())
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "cc_proxy_auth migration: select failed");
            return;
        }
    };
    for key_name in keys {
        let row: Result<(), String> = (|| {
            let raw: String = conn
                .query_row(
                    "SELECT value FROM setting WHERE scope = ?1 AND key = ?2",
                    params![CC_PROXY_AUTH_SCOPE, key_name],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            // 非 JSON 字符串形态（坏行 / 历史非串值）→ 解析失败，明文保留下次重试
            let plain: String = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            let enc = crate::crypt::encrypt_with(key, &plain)?;
            let value_str = serde_json::to_string(&enc).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE setting SET value = ?1 WHERE scope = ?2 AND key = ?3",
                params![value_str, CC_PROXY_AUTH_SCOPE, key_name],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(e) = row {
            tracing::warn!(key = %key_name, error = %e, "cc_proxy_auth migration row failed, plaintext kept for next startup");
        }
    }
}
