//! UI 态持久化（_ui_* 键）：单键读改写 extra JSON。
//!
//! 前端把 UI 态（卡片展开/折叠等）写到 platform/group 的 `extra` JSON 的 `_ui_*` 键，
//! 后端业务解析（peak_hours_for / parse_disable_during_peak / parse_breaker 等）用
//! serde_json 读己键天然忽略未知键，故 _ui_* 与业务键共存无副作用。
//!
//! 白名单：`platform` / `group`（两表均有 extra 列，schema_late migration 044 起 group 也带）。
use super::*;
use rusqlite::params;

/// 允许操作 extra 键的表名白名单（表名无法参数化绑定，须拼 SQL 前显式校验防注入）。
const EXTRA_TABLES: &[&str] = &["platform", "group"];

/// 读 extra JSON → set 单键 → 写回（单 SQL UPDATE）。
/// - `table`：白名单内的表名（当前仅 "platform"）
/// - `id`：行主键
/// - `key`：JSON 键名（推荐 `_ui_` 前缀避免与业务键冲突）
/// - `value`：任意 JSON 值
///
/// 空串 extra 视作 `{}`。原 extra 非合法 JSON 时返 Err（不静默覆盖防数据丢失）。
#[track_caller]
pub fn update_extra_key<'a>(
    db: &'a Db,
    table: &'a str,
    id: u64,
    key: &'a str,
    value: serde_json::Value,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        if !EXTRA_TABLES.contains(&table) {
            return Err(format!(
                "update_extra_key: unsupported table '{table}' (allowed: {EXTRA_TABLES:?})"
            ));
        }
        let key = key.to_string();
        let sql_read = format!("SELECT extra FROM {table} WHERE id = ?1");
        let sql_write = format!("UPDATE {table} SET extra = ?1 WHERE id = ?2");
        db.call_traced(None, __db_caller, move |conn| {
            // 闭包返回 tokio_rusqlite::Result<u64>（行计数），String 包装在外层 await 后做。
            let raw: String = conn.query_row(&sql_read, params![id as i64], |r| r.get(0))?;
            let mut root: serde_json::Value = if raw.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&raw).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
            };
            match root.as_object_mut() {
                Some(obj) => {
                    obj.insert(key.clone(), value.clone());
                }
                None => {
                    // extra 不是 JSON 对象（历史脏数据 / 数组等）→ 包成对象保留原值 + 新键。
                    let mut obj = serde_json::Map::new();
                    obj.insert("_original".to_string(), root.clone());
                    obj.insert(key.clone(), value.clone());
                    root = serde_json::Value::Object(obj);
                }
            }
            let new_str = serde_json::to_string(&root).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
            })?;
            let n = conn.execute(&sql_write, params![new_str, id as i64])?;
            Ok(n)
        })
        .await
        .map_err(|e| format!("update_extra_key: {e}"))?;
        // extra 改动影响 group_details 缓存（platform.extra 被内嵌）+ groups 缓存（group.extra 内嵌于 Group）。
        // 两失效一并调：platform 写只污染 group_details（groups 缓存不含 platform.extra），
        // group 写两缓存都污染（Group.extra 直接进 groups 列表 + group_details）。
        // ponytail: 不分支按 table 挑调用——invalidate_groups_cache 内部已联动清 group_details，
        // 多调一次 group_details 失效是无害的空写（RwLock 写清 None）。
        db.invalidate_groups_cache();
        Ok(())
    }
}
