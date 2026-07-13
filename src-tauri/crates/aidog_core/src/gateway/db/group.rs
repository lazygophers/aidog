use super::*;
use rusqlite::{params, OptionalExtension, Result as SqlResult};

/// 序列化 / 反序列化内联 model_mappings
fn serialize_mappings(mappings: &[ModelMapping]) -> String {
    serde_json::to_string(mappings).unwrap_or_else(|_| "[]".to_string())
}

fn parse_mappings(json: &str) -> Vec<ModelMapping> {
    serde_json::from_str(json).unwrap_or_default()
}

/// 序列化 / 反序列化内联 env_vars
fn serialize_env_vars(vars: &[EnvVar]) -> String {
    serde_json::to_string(vars).unwrap_or_else(|_| "[]".to_string())
}

fn parse_env_vars(json: &str) -> Vec<EnvVar> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Group SELECT 列序
const GROUP_COLUMNS: &str =
    "id, name, routing_mode, auto_from_platform, created_at, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, model_mappings, sort_order, max_retries, group_key, is_default, env_vars, extra";

fn row_to_group(row: &rusqlite::Row) -> SqlResult<Group> {
    let routing_str: String = row.get(2)?;
    let mappings_str: String = row.get(9)?;
    let env_vars_str: String = row.get(14)?;
    let extra_str: String = row.get::<_, String>(15).unwrap_or_default();
    Ok(Group {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        routing_mode: serde_json::from_str(&routing_str).unwrap(),
        auto_from_platform: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        request_timeout_secs: row.get::<_, i64>(6)? as u64,
        connect_timeout_secs: row.get::<_, i64>(7)? as u64,
        source_protocol: row.get::<_, String>(8)?,
        model_mappings: parse_mappings(&mappings_str),
        deleted_at: 0,
        sort_order: row.get::<_, i64>(10)?,
        max_retries: row.get::<_, i64>(11)? as u32,
        group_key: row.get(12)?,
        is_default: row.get::<_, i64>(13)? != 0,
        env_vars: parse_env_vars(&env_vars_str),
        extra: extra_str,
    })
}

#[track_caller]
pub fn create_group(db: &Db, input: CreateGroup) -> impl std::future::Future<Output = Result<Group, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    let routing_str = serde_json::to_string(&input.routing_mode).unwrap();
    let source_protocol = input.source_protocol.unwrap_or_else(|| "anthropic".to_string());
    let mappings_str = serialize_mappings(&input.model_mappings);
    let env_vars_str = serialize_env_vars(&input.env_vars);
    // group_key：用户提供则用，否则自动生成 gk_<32hex>（创建后锁定不可改）。
    let group_key = input
        .group_key
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| format!("gk_{}", uuid::Uuid::new_v4().simple()));

    let id = db

        .call_traced(None, __db_caller, {
            let name = input.name.clone();
            let group_key = group_key.clone();
            let auto_from_platform = input.auto_from_platform.clone();
            let request_timeout_secs = input.request_timeout_secs as i64;
            let connect_timeout_secs = input.connect_timeout_secs as i64;
            let source_protocol = source_protocol.clone();
            let max_retries = input.max_retries as i64;
            move |conn| {
                conn.execute(
                    "INSERT INTO \"group\" (name, group_key, routing_mode, auto_from_platform, created_at, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, model_mappings, max_retries, env_vars) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![name, group_key, routing_str, auto_from_platform, ts, ts, request_timeout_secs, connect_timeout_secs, source_protocol, mappings_str, max_retries, env_vars_str],
                )?;
                Ok(conn.last_insert_rowid() as u64)
            }
        })
        .await
        .map_err(|e| format!("create group: {e}"))?;
    db.invalidate_groups_cache();

    Ok(Group {
        id,
        name: input.name,
        group_key,
        routing_mode: input.routing_mode,
        auto_from_platform: input.auto_from_platform,
        created_at: ts,
        updated_at: ts,
        request_timeout_secs: input.request_timeout_secs,
        connect_timeout_secs: input.connect_timeout_secs,
        source_protocol,
        model_mappings: input.model_mappings,
        deleted_at: 0,
        sort_order: 0,
        max_retries: input.max_retries,
        is_default: false,
        env_vars: input.env_vars,
        extra: String::new(),
    })
    }
}

/// 批量更新 group 的 sort_order：接收有序 id 列表，按序赋值 1, 2, 3, …
#[track_caller]
pub fn reorder_groups<'a>(db: &'a Db, ordered_ids: &'a [u64]) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ordered_ids = ordered_ids.to_vec();
    db
        .call_traced(None, __db_caller, move |conn| {
            for (i, &id) in ordered_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE \"group\" SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                    params![(i + 1) as i64, now(), id as i64],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("reorder group: {e}"))?;
    db.invalidate_groups_cache();
    Ok(())
    }
}

/// 批量更新 platform 的 sort_order
#[track_caller]
pub fn reorder_platforms<'a>(db: &'a Db, ordered_ids: &'a [u64]) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ordered_ids = ordered_ids.to_vec();
    db
        .call_traced(None, __db_caller, move |conn| {
            for (i, &id) in ordered_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE platform SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                    params![(i + 1) as i64, now(), id as i64],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("reorder platform: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

/// 批量更新某分组内平台的 priority（拖拽排序）。ordered_platform_ids 按序赋 1,2,3…
#[track_caller]
pub fn reorder_group_platforms<'a>(
    db: &'a Db,
    group_id: u64,
    ordered_platform_ids: &'a [u64],
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let group_id = group_id as i64;
    let ordered = ordered_platform_ids.to_vec();
    let ts = now();
    db
        .call_traced(None, __db_caller, move |conn| {
            for (i, &pid) in ordered.iter().enumerate() {
                conn.execute(
                    "UPDATE group_platform SET priority = ?1, updated_at = ?2 \
                     WHERE group_id = ?3 AND platform_id = ?4 AND deleted_at = 0",
                    params![(i + 1) as i64, ts, group_id, pid as i64],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("reorder group platforms: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

/// 设置某 group×platform 的 level_priority（per-group 平台优先级）。
/// 入参 clamp 到 [1,10]；仅更新存在的关联行（不存在静默 no-op）。
#[track_caller]
pub fn set_group_platform_level_priority(
    db: &Db,
    group_id: u64,
    platform_id: u64,
    level_priority: i32,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let lp = crate::gateway::models::clamp_level_priority(level_priority);
    let gid = group_id as i64;
    let pid = platform_id as i64;
    let ts = now();
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE group_platform SET level_priority = ?1, updated_at = ?2 \
                 WHERE group_id = ?3 AND platform_id = ?4 AND deleted_at = 0",
                params![lp, ts, gid, pid],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("set group platform level_priority: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

/// 跨分组移动平台：从 from 组移除、加入 to 组（priority = to 组现有最大 + 1）。
#[track_caller]
pub fn move_group_platform(
    db: &Db,
    platform_id: u64,
    from_group_id: u64,
    to_group_id: u64,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let pid = platform_id as i64;
    let from = from_group_id as i64;
    let to = to_group_id as i64;
    let ts = now();
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2 AND deleted_at = 0",
                params![from, pid],
            )?;
            // 物理清除目标组内该平台的所有历史行(含软删残留),避免 UNIQUE(group_id,platform_id) 冲突
            // 场景: 平台曾加入该组又移除(软删行 deleted_at≠0 残留), 重新加入时 INSERT 撞 UNIQUE
            conn.execute(
                "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2",
                params![to, pid],
            )?;
            let max_pri: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(priority), 0) FROM group_platform \
                     WHERE group_id = ?1 AND deleted_at = 0",
                    params![to],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            conn.execute(
                "INSERT INTO group_platform (group_id, platform_id, priority, weight, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, 1, ?4, ?4)",
                params![to, pid, max_pri + 1, ts],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("move group platform: {e}"))?;
    db.invalidate_group_details_cache();
    Ok(())
    }
}

#[track_caller]
pub fn list_groups(db: &Db) -> impl std::future::Future<Output = Result<Vec<Group>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    if let Ok(g) = db.1.groups.read() {
        if let Some(cached) = g.as_ref() {
            return Ok(cached.clone());
        }
    }
    let groups = db
        
        .call_read_traced(None, __db_caller, |conn| {
            let mut stmt = conn.prepare(&format!("SELECT {GROUP_COLUMNS} FROM \"group\" WHERE deleted_at = 0 ORDER BY sort_order, created_at"))?;
            let rows = stmt.query_map([], row_to_group)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Ok(mut g) = db.1.groups.write() {
        *g = Some(groups.clone());
    }
    Ok(groups)
    }
}

#[track_caller]
pub fn get_group(db: &Db, id: u64) -> impl std::future::Future<Output = Result<Option<Group>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(&format!("SELECT {GROUP_COLUMNS} FROM \"group\" WHERE id = ?1 AND deleted_at = 0"))?;
            Ok(stmt.query_row(params![id as i64], row_to_group).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[track_caller]
pub fn update_group(db: &Db, input: UpdateGroup) -> impl std::future::Future<Output = Result<Group, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let existing = get_group(db, input.id).await?.ok_or("group not found")?;

    let updated = Group {
        name: input.name.unwrap_or(existing.name),
        routing_mode: input.routing_mode.unwrap_or(existing.routing_mode),
        request_timeout_secs: if input.request_timeout_secs > 0 { input.request_timeout_secs } else { existing.request_timeout_secs },
        connect_timeout_secs: if input.connect_timeout_secs > 0 { input.connect_timeout_secs } else { existing.connect_timeout_secs },
        source_protocol: input.source_protocol.unwrap_or(existing.source_protocol),
        max_retries: input.max_retries.unwrap_or(existing.max_retries),
        model_mappings: input.model_mappings,
        env_vars: input.env_vars,
        updated_at: now(),
        ..existing
    };

    let routing_str = serde_json::to_string(&updated.routing_mode).unwrap();
    let mappings_str = serialize_mappings(&updated.model_mappings);
    let env_vars_str = serialize_env_vars(&updated.env_vars);
    db
        .call_traced(None, __db_caller, {
            let name = updated.name.clone();
            let updated_at = updated.updated_at;
            let request_timeout_secs = updated.request_timeout_secs as i64;
            let connect_timeout_secs = updated.connect_timeout_secs as i64;
            let source_protocol = updated.source_protocol.clone();
            let max_retries = updated.max_retries as i64;
            let id = updated.id as i64;
            move |conn| {
                conn.execute(
                    "UPDATE \"group\" SET name=?1, routing_mode=?2, updated_at=?3, request_timeout_secs=?4, connect_timeout_secs=?5, source_protocol=?6, model_mappings=?7, max_retries=?8, env_vars=?9 WHERE id=?10",
                    params![name, routing_str, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, mappings_str, max_retries, env_vars_str, id],
                )?;
                Ok(())
            }
        })
        .await
        .map_err(|e| format!("update group: {e}"))?;
    db.invalidate_groups_cache();

    Ok(updated)
    }
}

/// 设置默认分组（单选）。目标 id 置 is_default=1，其余全部清零。
/// 一条 UPDATE 同时清零全部 + 置目标；updated_at 仅刷新被切换的行（保持排序稳定）。
/// 清除默认（target_id 为 None）时把所有 is_default 置 0。
#[track_caller]
pub fn set_default_group(db: &Db, target_id: Option<u64>) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let ts = now();
    db
        .call_traced(None, __db_caller, move |conn| {
            match target_id {
                Some(id) => {
                    conn.execute(
                        "UPDATE \"group\" \
                         SET is_default = CASE WHEN id = ?1 THEN 1 ELSE 0 END, \
                             updated_at = CASE WHEN id = ?1 OR is_default = 1 THEN ?2 ELSE updated_at END \
                         WHERE deleted_at = 0",
                        params![id as i64, ts],
                    )?;
                }
                None => {
                    conn.execute(
                        "UPDATE \"group\" SET is_default = 0, updated_at = ?1 WHERE is_default = 1 AND deleted_at = 0",
                        params![ts],
                    )?;
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("set default group: {e}"))?;
    db.invalidate_groups_cache();
    Ok(())
    }
}

pub async fn delete_group(db: &Db, id: u64) -> Result<(), String> {
    // 检查是否为自动分组
    let group = get_group(db, id).await?.ok_or("group not found")?;
    if !group.auto_from_platform.is_empty() {
        // auto 分组：仅当关联平台已空（源平台已删的孤儿分组）时允许手动删除
        let plats = get_group_platforms(db, id).await?;
        if !plats.is_empty() {
            return Err("auto-created group with linked platforms cannot be deleted manually".to_string());
        }
    }
    force_delete_group(db, id).await
}

