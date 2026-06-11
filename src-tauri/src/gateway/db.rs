use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use std::sync::Mutex;

use super::models::*;

pub struct Db(pub Mutex<Connection>);

/// 从 JSON 字符串反序列化 models
fn parse_models(json: &str) -> PlatformModels {
    serde_json::from_str(json).unwrap_or_default()
}

/// 将 models 序列化为 JSON 字符串
fn serialize_models(models: &PlatformModels) -> String {
    serde_json::to_string(models).unwrap_or_else(|_| "{}".to_string())
}

/// 从 JSON 字符串反序列化可用模型列表
fn parse_available_models(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

/// 将可用模型列表序列化为 JSON 字符串
fn serialize_available_models(models: &[String]) -> String {
    serde_json::to_string(models).unwrap_or_else(|_| "[]".to_string())
}

/// 从 JSON 字符串反序列化协议端点列表
fn parse_endpoints(json: &str) -> Vec<PlatformEndpoint> {
    serde_json::from_str(json).unwrap_or_default()
}

/// 将协议端点列表序列化为 JSON 字符串
fn serialize_endpoints(endpoints: &[PlatformEndpoint]) -> String {
    serde_json::to_string(endpoints).unwrap_or_else(|_| "[]".to_string())
}

impl Db {
    pub fn new(path: &str) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn init_tables(&self) -> Result<(), String> {
        let sql = include_str!("../../migrations/001_init.sql");
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(sql).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// 当前毫秒级 Unix 时间戳
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ─── Platform CRUD ─────────────────────────────────────────

/// SELECT 列序
const PLATFORM_COLUMNS: &str =
    "id, name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at";

/// 从查询行构造 Platform
fn row_to_platform(row: &rusqlite::Row) -> SqlResult<Platform> {
    let platform_type_str: String = row.get(2)?;
    let models_str: String = row.get(6)?;
    let available_str: String = row.get(7)?;
    let endpoints_str: String = row.get(8)?;
    Ok(Platform {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        platform_type: serde_json::from_str(&platform_type_str).unwrap(),
        base_url: row.get(3)?,
        api_key: row.get(4)?,
        extra: row.get(5)?,
        models: parse_models(&models_str),
        available_models: parse_available_models(&available_str),
        endpoints: parse_endpoints(&endpoints_str),
        enabled: row.get::<_, i64>(9)? == 1,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        deleted_at: 0,
    })
}

pub fn create_platform(db: &Db, mut input: CreatePlatform) -> Result<Platform, String> {
    let ts = now();
    let platform_type_str = serde_json::to_string(&input.platform_type).unwrap();
    // If name is empty, auto-generate: {platform_type}-{random8}
    if input.name.trim().is_empty() {
        let proto_label = format!("{:?}", input.platform_type).to_lowercase();
        let rand_suffix = &uuid::Uuid::new_v4().simple().to_string()[..8];
        input.name = format!("{}-{}", proto_label, rand_suffix);
    }
    let models = input.models.unwrap_or_default();
    let models_str = serialize_models(&models);
    let available_models = input.available_models.unwrap_or_default();
    let available_str = serialize_available_models(&available_models);
    let endpoints = input.endpoints.unwrap_or_default();
    let endpoints_str = serialize_endpoints(&endpoints);

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO platform (name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![input.name, platform_type_str, input.base_url, input.api_key, input.extra, models_str, available_str, endpoints_str, true as i64, ts, ts],
    )
    .map_err(|e| format!("create platform: {e}"))?;
    let id = conn.last_insert_rowid() as u64;

    Ok(Platform {
        id,
        name: input.name,
        platform_type: input.platform_type,
        base_url: input.base_url,
        api_key: input.api_key,
        extra: input.extra,
        models,
        available_models,
        endpoints,
        enabled: true,
        created_at: ts,
        updated_at: ts,
        deleted_at: 0,
    })
}

pub fn list_platforms(db: &Db) -> Result<Vec<Platform>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE deleted_at = 0 ORDER BY created_at");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_platform)
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn get_platform(db: &Db, id: u64) -> Result<Option<Platform>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platform WHERE id = ?1 AND deleted_at = 0");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(params![id as i64], row_to_platform)
        .optional()
        .map_err(|e| e.to_string())?;

    Ok(result)
}

pub fn update_platform(db: &Db, input: UpdatePlatform) -> Result<Platform, String> {
    let existing = get_platform(db, input.id)?.ok_or("platform not found")?;

    let updated = Platform {
        name: input.name.unwrap_or(existing.name),
        platform_type: input.platform_type.unwrap_or(existing.platform_type),
        base_url: input.base_url.unwrap_or(existing.base_url),
        api_key: input.api_key.unwrap_or(existing.api_key),
        extra: input.extra.unwrap_or(existing.extra),
        models: input.models.unwrap_or(existing.models),
        available_models: input.available_models.unwrap_or(existing.available_models),
        endpoints: input.endpoints.unwrap_or(existing.endpoints),
        enabled: input.enabled.unwrap_or(existing.enabled),
        updated_at: now(),
        ..existing
    };

    let platform_type_str = serde_json::to_string(&updated.platform_type).unwrap();
    let models_str = serialize_models(&updated.models);
    let available_str = serialize_available_models(&updated.available_models);
    let endpoints_str = serialize_endpoints(&updated.endpoints);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE platform SET name=?1, platform_type=?2, base_url=?3, api_key=?4, extra=?5, models=?6, available_models=?7, endpoints=?8, enabled=?9, updated_at=?10 WHERE id=?11",
        params![
            updated.name,
            platform_type_str,
            updated.base_url,
            updated.api_key,
            updated.extra,
            models_str,
            available_str,
            endpoints_str,
            updated.enabled as i64,
            updated.updated_at,
            updated.id as i64,
        ],
    )
    .map_err(|e| format!("update platform: {e}"))?;

    Ok(updated)
}

pub fn delete_platform(db: &Db, id: u64) -> Result<(), String> {
    // 删除关联的自动分组
    let conn_inner = db.0.lock().map_err(|e| e.to_string())?;
    let auto_group_ids: Vec<i64> = conn_inner
        .prepare("SELECT id FROM \"group\" WHERE auto_from_platform = ?1 AND deleted_at = 0")
        .map_err(|e| e.to_string())?
        .query_map(params![id.to_string()], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    drop(conn_inner);

    for gid in &auto_group_ids {
        force_delete_group(db, *gid as u64)?;
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE platform SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])
        .map_err(|e| format!("delete platform: {e}"))?;
    Ok(())
}

// ─── Group CRUD ────────────────────────────────────────────

/// 序列化 / 反序列化内联 model_mappings
fn serialize_mappings(mappings: &[ModelMapping]) -> String {
    serde_json::to_string(mappings).unwrap_or_else(|_| "[]".to_string())
}

fn parse_mappings(json: &str) -> Vec<ModelMapping> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Group SELECT 列序
const GROUP_COLUMNS: &str =
    "id, name, path, routing_mode, auto_from_platform, created_at, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, model_mappings";

fn row_to_group(row: &rusqlite::Row) -> SqlResult<Group> {
    let routing_str: String = row.get(3)?;
    let mappings_str: String = row.get(10)?;
    Ok(Group {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        path: row.get(2)?,
        routing_mode: serde_json::from_str(&routing_str).unwrap(),
        auto_from_platform: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        request_timeout_secs: row.get::<_, i64>(7)? as u64,
        connect_timeout_secs: row.get::<_, i64>(8)? as u64,
        source_protocol: row.get::<_, String>(9)?,
        model_mappings: parse_mappings(&mappings_str),
        deleted_at: 0,
    })
}

pub fn create_group(db: &Db, input: CreateGroup) -> Result<Group, String> {
    let ts = now();
    let routing_str = serde_json::to_string(&input.routing_mode).unwrap();
    let source_protocol = input.source_protocol.unwrap_or_else(|| "anthropic".to_string());
    let mappings_str = serialize_mappings(&input.model_mappings);

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO \"group\" (name, path, routing_mode, auto_from_platform, created_at, updated_at, request_timeout_secs, connect_timeout_secs, source_protocol, model_mappings) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![input.name, input.path, routing_str, input.auto_from_platform, ts, ts, input.request_timeout_secs as i64, input.connect_timeout_secs as i64, source_protocol, mappings_str],
    )
    .map_err(|e| format!("create group: {e}"))?;
    let id = conn.last_insert_rowid() as u64;

    Ok(Group {
        id,
        name: input.name,
        path: input.path,
        routing_mode: input.routing_mode,
        auto_from_platform: input.auto_from_platform,
        created_at: ts,
        updated_at: ts,
        request_timeout_secs: input.request_timeout_secs,
        connect_timeout_secs: input.connect_timeout_secs,
        source_protocol,
        model_mappings: input.model_mappings,
        deleted_at: 0,
    })
}

pub fn list_groups(db: &Db) -> Result<Vec<Group>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!("SELECT {GROUP_COLUMNS} FROM \"group\" WHERE deleted_at = 0 ORDER BY created_at"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_group)
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn get_group(db: &Db, id: u64) -> Result<Option<Group>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!("SELECT {GROUP_COLUMNS} FROM \"group\" WHERE id = ?1 AND deleted_at = 0"))
        .map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(params![id as i64], row_to_group)
        .optional()
        .map_err(|e| e.to_string())?;

    Ok(result)
}

pub fn update_group(db: &Db, input: UpdateGroup) -> Result<Group, String> {
    let existing = get_group(db, input.id)?.ok_or("group not found")?;

    let updated = Group {
        name: input.name.unwrap_or(existing.name),
        path: input.path.unwrap_or(existing.path),
        routing_mode: input.routing_mode.unwrap_or(existing.routing_mode),
        request_timeout_secs: if input.request_timeout_secs > 0 { input.request_timeout_secs } else { existing.request_timeout_secs },
        connect_timeout_secs: if input.connect_timeout_secs > 0 { input.connect_timeout_secs } else { existing.connect_timeout_secs },
        source_protocol: input.source_protocol.unwrap_or(existing.source_protocol),
        model_mappings: input.model_mappings,
        updated_at: now(),
        ..existing
    };

    let routing_str = serde_json::to_string(&updated.routing_mode).unwrap();
    let mappings_str = serialize_mappings(&updated.model_mappings);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE \"group\" SET name=?1, path=?2, routing_mode=?3, updated_at=?4, request_timeout_secs=?5, connect_timeout_secs=?6, source_protocol=?7, model_mappings=?8 WHERE id=?9",
        params![updated.name, updated.path, routing_str, updated.updated_at, updated.request_timeout_secs as i64, updated.connect_timeout_secs as i64, updated.source_protocol, mappings_str, updated.id as i64],
    )
    .map_err(|e| format!("update group: {e}"))?;

    Ok(updated)
}

pub fn delete_group(db: &Db, id: u64) -> Result<(), String> {
    // 检查是否为自动分组
    let group = get_group(db, id)?.ok_or("group not found")?;
    if !group.auto_from_platform.is_empty() {
        return Err("auto-created group cannot be deleted manually".to_string());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE \"group\" SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])
        .map_err(|e| format!("delete group: {e}"))?;
    Ok(())
}

/// 强制删除分组（含自动分组），仅供平台删除时内部调用
pub fn force_delete_group(db: &Db, id: u64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE \"group\" SET deleted_at = ?1 WHERE id = ?2", params![now(), id as i64])
        .map_err(|e| format!("delete group: {e}"))?;
    Ok(())
}

// ─── GroupPlatform 关联 ────────────────────────────────────

pub fn set_group_platforms(
    db: &Db,
    group_id: u64,
    platforms: &[GroupPlatformInput],
) -> Result<(), String> {
    let ts = now();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    // 物理清除旧关联后重建（关联表无需软删保留）
    conn.execute(
        "DELETE FROM group_platform WHERE group_id = ?1",
        params![group_id as i64],
    )
    .map_err(|e| e.to_string())?;

    for p in platforms {
        conn.execute(
            "INSERT INTO group_platform (group_id, platform_id, priority, weight, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![group_id as i64, p.platform_id as i64, p.priority.unwrap_or(0), p.weight.unwrap_or(1), ts, ts],
        )
        .map_err(|e| format!("insert group platform: {e}"))?;
    }

    Ok(())
}

pub fn get_group_platforms(db: &Db, group_id: u64) -> Result<Vec<GroupPlatformDetail>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            &format!(
                "SELECT gp.priority, gp.weight, p.{PLATFORM_COLUMNS} \
                 FROM group_platform gp JOIN platform p ON gp.platform_id = p.id \
                 WHERE gp.group_id = ?1 AND gp.deleted_at = 0 AND p.deleted_at = 0 ORDER BY gp.priority"
            ),
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![group_id as i64], |row| {
            // row layout: priority(0), weight(1), then platform columns starting at 2
            let platform_type_str: String = row.get(4)?;
            let models_str: String = row.get(8)?;
            let available_str: String = row.get(9)?;
            let endpoints_str: String = row.get(10)?;
            Ok(GroupPlatformDetail {
                platform: Platform {
                    id: row.get::<_, i64>(2)? as u64,
                    name: row.get(3)?,
                    platform_type: serde_json::from_str(&platform_type_str).unwrap(),
                    base_url: row.get(5)?,
                    api_key: row.get(6)?,
                    extra: row.get(7)?,
                    models: parse_models(&models_str),
                    available_models: parse_available_models(&available_str),
                    endpoints: parse_endpoints(&endpoints_str),
                    enabled: row.get::<_, i64>(11)? == 1,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                    deleted_at: 0,
                },
                priority: row.get(0)?,
                weight: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

// ─── 聚合查询 ──────────────────────────────────────────────

pub fn get_group_detail(db: &Db, id: u64) -> Result<Option<GroupDetail>, String> {
    let group = match get_group(db, id)? {
        Some(g) => g,
        None => return Ok(None),
    };
    let platforms = get_group_platforms(db, id)?;
    let model_mappings = group.model_mappings.clone();

    Ok(Some(GroupDetail {
        group,
        platforms,
        model_mappings,
    }))
}

pub fn list_group_details(db: &Db) -> Result<Vec<GroupDetail>, String> {
    let groups = list_groups(db)?;
    let mut details = Vec::with_capacity(groups.len());
    for g in groups {
        let platforms = get_group_platforms(db, g.id)?;
        let model_mappings = g.model_mappings.clone();
        details.push(GroupDetail {
            group: g,
            platforms,
            model_mappings,
        });
    }
    Ok(details)
}

// ─── Settings CRUD ─────────────────────────────────────────

pub fn get_setting(
    db: &Db,
    scope: &str,
    key: &str,
) -> Result<Option<serde_json::Value>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT value FROM setting WHERE scope = ?1 AND key = ?2 AND deleted_at = 0")
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_row(params![scope, key], |row| {
            let v: String = row.get(0)?;
            Ok(serde_json::from_str(&v).unwrap_or(serde_json::Value::Null))
        })
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(result)
}

pub fn set_setting(db: &Db, input: SetSettingInput) -> Result<(), String> {
    let ts = now();
    let value_str =
        serde_json::to_string(&input.value).map_err(|e| format!("serialize setting: {e}"))?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO setting (scope, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(scope, key) DO UPDATE SET value = ?3, updated_at = ?4, deleted_at = 0",
        params![input.scope, input.key, value_str, ts],
    )
    .map_err(|e| format!("upsert setting: {e}"))?;
    Ok(())
}

pub fn delete_setting(db: &Db, scope: &str, key: &str) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE setting SET deleted_at = ?1 WHERE scope = ?2 AND key = ?3",
        params![now(), scope, key],
    )
    .map_err(|e| format!("delete setting: {e}"))?;
    Ok(())
}

pub fn list_setting_keys(db: &Db, scope: &str) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT key FROM setting WHERE scope = ?1 AND deleted_at = 0 ORDER BY key")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![scope], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

// ─── ProxyLog CRUD ─────────────────────────────────────────

/// Upsert (INSERT OR REPLACE) a proxy log entry — used for incremental logging
pub fn upsert_proxy_log(db: &Db, log: &super::models::ProxyLog) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO proxy_log (id, group_name, model, actual_model, source_protocol, target_protocol, platform_id, request_headers, request_body, upstream_request_headers, upstream_request_body, response_body, request_url, upstream_request_url, upstream_response_headers, upstream_status_code, user_response_headers, user_response_body, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, created_at, updated_at, deleted_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)",
        params![log.id, log.group_name, log.model, log.actual_model, log.source_protocol, log.target_protocol, log.platform_id as i64, log.request_headers, log.request_body, log.upstream_request_headers, log.upstream_request_body, log.response_body, log.request_url, log.upstream_request_url, log.upstream_response_headers, log.upstream_status_code, log.user_response_headers, log.user_response_body, log.status_code, log.duration_ms, log.input_tokens, log.output_tokens, log.cache_tokens, log.created_at, log.updated_at, log.deleted_at],
    ).map_err(|e| format!("upsert proxy log: {e}"))?;
    Ok(())
}

pub fn list_proxy_logs(db: &Db, limit: u32, offset: u32) -> Result<Vec<super::models::ProxyLogSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, group_name, model, actual_model, source_protocol, target_protocol, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, created_at
             FROM proxy_log WHERE deleted_at = 0 ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit, offset], |row| {
            Ok(super::models::ProxyLogSummary {
                id: row.get(0)?,
                group_name: row.get(1)?,
                model: row.get(2)?,
                actual_model: row.get(3)?,
                source_protocol: row.get(4)?,
                target_protocol: row.get(5)?,
                status_code: row.get(6)?,
                duration_ms: row.get(7)?,
                input_tokens: row.get(8)?,
                output_tokens: row.get(9)?,
                cache_tokens: row.get(10)?,
                created_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn get_proxy_log(db: &Db, id: &str) -> Result<Option<super::models::ProxyLog>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, group_name, model, actual_model, source_protocol, target_protocol, platform_id, request_headers, request_body, upstream_request_headers, upstream_request_body, response_body, request_url, upstream_request_url, upstream_response_headers, upstream_status_code, user_response_headers, user_response_body, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, created_at, updated_at, deleted_at
             FROM proxy_log WHERE id = ?1 AND deleted_at = 0",
        )
        .map_err(|e| e.to_string())?;
    stmt.query_row(params![id], |row| {
        Ok(super::models::ProxyLog {
            id: row.get(0)?,
            group_name: row.get(1)?,
            model: row.get(2)?,
            actual_model: row.get(3)?,
            source_protocol: row.get(4)?,
            target_protocol: row.get(5)?,
            platform_id: row.get::<_, i64>(6)? as u64,
            request_headers: row.get(7)?,
            request_body: row.get(8)?,
            upstream_request_headers: row.get(9)?,
            upstream_request_body: row.get(10)?,
            response_body: row.get(11)?,
            request_url: row.get(12)?,
            upstream_request_url: row.get(13)?,
            upstream_response_headers: row.get(14)?,
            upstream_status_code: row.get(15)?,
            user_response_headers: row.get(16)?,
            user_response_body: row.get(17)?,
            status_code: row.get(18)?,
            duration_ms: row.get(19)?,
            input_tokens: row.get(20)?,
            output_tokens: row.get(21)?,
            cache_tokens: row.get(22)?,
            created_at: row.get(23)?,
            updated_at: row.get(24)?,
            deleted_at: row.get(25)?,
        })
    })
    .optional()
    .map_err(|e| e.to_string())
}

pub fn clear_proxy_logs(db: &Db) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE proxy_log SET deleted_at = ?1 WHERE deleted_at = 0", params![now()])
        .map_err(|e| format!("clear proxy logs: {e}"))?;
    Ok(())
}

/// Delete logs older than N days. Pass 0 to skip.
pub fn cleanup_proxy_logs(db: &Db, retention_days: u32) -> Result<(), String> {
    if retention_days == 0 {
        return Ok(());
    }
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).timestamp_millis();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE proxy_log SET deleted_at = ?1 WHERE created_at < ?2 AND deleted_at = 0", params![now(), cutoff])
        .map_err(|e| format!("cleanup proxy logs: {e}"))?;
    Ok(())
}

/// Clear user request fields (headers, body, user response) for logs older than retention_days.
/// Does NOT delete the log row — keeps token stats and metadata.
pub fn cleanup_user_request_fields(db: &Db, retention_days: u32) -> Result<(), String> {
    if retention_days == 0 {
        return Ok(());
    }
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).timestamp_millis();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE proxy_log SET request_headers = '', request_body = '', user_response_headers = '', user_response_body = '' WHERE created_at < ?1 AND (request_headers != '' OR request_body != '')",
        params![cutoff],
    ).map_err(|e| format!("cleanup user request fields: {e}"))?;
    Ok(())
}

/// Clear upstream request fields (headers, body, response headers) for logs older than retention_days.
/// Does NOT delete the log row — keeps token stats and metadata.
pub fn cleanup_upstream_request_fields(db: &Db, retention_days: u32) -> Result<(), String> {
    if retention_days == 0 {
        return Ok(());
    }
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).timestamp_millis();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE proxy_log SET upstream_request_headers = '', upstream_request_body = '', upstream_response_headers = '' WHERE created_at < ?1 AND (upstream_request_headers != '' OR upstream_request_body != '')",
        params![cutoff],
    ).map_err(|e| format!("cleanup upstream request fields: {e}"))?;
    Ok(())
}

pub fn count_proxy_logs(db: &Db) -> Result<u32, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0", [], |row| row.get(0))
        .map_err(|e| e.to_string())
}

pub fn get_platform_usage_stats(db: &Db, platform_id: u64) -> Result<super::models::PlatformUsageStats, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    // platform_id 现为整数；自动分组日志可能未带 platform_id（=0），通过 group.auto_from_platform（存十进制字符串）回溯
    let where_clause = "deleted_at = 0 AND (platform_id = ?1 OR (platform_id = 0 AND group_name IN (SELECT name FROM \"group\" WHERE auto_from_platform = ?2 AND deleted_at = 0)))";
    // Overall stats
    let stats: super::models::PlatformUsageStats = conn.query_row(
        &format!("SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens) \
         FROM proxy_log WHERE {where_clause}"),
        params![platform_id as i64, platform_id.to_string()],
        |row| {
            let total: i64 = row.get(0).unwrap_or(0);
            let success: i64 = row.get(1).unwrap_or(0);
            let inp: i64 = row.get(2).unwrap_or(0);
            let out: i64 = row.get(3).unwrap_or(0);
            let cache: i64 = row.get(4).unwrap_or(0);
            Ok(super::models::PlatformUsageStats {
                total_requests: total,
                success_count: success,
                total_input_tokens: inp,
                total_output_tokens: out,
                total_cache_tokens: cache,
                cache_rate: if inp > 0 { cache as f64 / inp as f64 * 100.0 } else { 0.0 },
                recent_failures: 0,
                recent_total: 0,
            })
        }
    ).map_err(|e| format!("platform usage stats: {e}"))?;

    // Recent 5 requests health
    let (recent_failures, recent_total): (i64, i64) = conn.query_row(
        &format!("SELECT COUNT(*), SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END) \
         FROM (SELECT status_code FROM proxy_log WHERE {where_clause} ORDER BY created_at DESC LIMIT 5)"),
        params![platform_id as i64, platform_id.to_string()],
        |row| Ok((row.get(1).unwrap_or(0), row.get(0).unwrap_or(0)))
    ).unwrap_or((0, 0));

    Ok(super::models::PlatformUsageStats {
        recent_failures,
        recent_total,
        ..stats
    })
}

pub fn get_group_usage_stats(db: &Db, group_name: &str) -> Result<super::models::PlatformUsageStats, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let stats: super::models::PlatformUsageStats = conn.query_row(
        "SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens) \
         FROM proxy_log WHERE group_name = ?1 AND deleted_at = 0",
        params![group_name],
        |row| {
            let total: i64 = row.get(0).unwrap_or(0);
            let success: i64 = row.get(1).unwrap_or(0);
            let inp: i64 = row.get(2).unwrap_or(0);
            let out: i64 = row.get(3).unwrap_or(0);
            let cache: i64 = row.get(4).unwrap_or(0);
            Ok(super::models::PlatformUsageStats {
                total_requests: total,
                success_count: success,
                total_input_tokens: inp,
                total_output_tokens: out,
                total_cache_tokens: cache,
                cache_rate: if inp > 0 { cache as f64 / inp as f64 * 100.0 } else { 0.0 },
                recent_failures: 0,
                recent_total: 0,
            })
        }
    ).map_err(|e| format!("group usage stats: {e}"))?;

    let (recent_failures, recent_total): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END) \
         FROM (SELECT status_code FROM proxy_log WHERE group_name = ?1 AND deleted_at = 0 ORDER BY created_at DESC LIMIT 5)",
        params![group_name],
        |row| Ok((row.get(1).unwrap_or(0), row.get(0).unwrap_or(0)))
    ).unwrap_or((0, 0));

    Ok(super::models::PlatformUsageStats {
        recent_failures,
        recent_total,
        ..stats
    })
}

struct QueryParams {
    start: i64,
    end: i64,
    filter_group: Option<String>,
    filter_model: Option<String>,
    filter_protocol: Option<String>,
}

impl QueryParams {
    fn to_sql_params(&self) -> Vec<Box<dyn rusqlite::types::ToSql>> {
        let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(self.start),
            Box::new(self.end),
        ];
        if let Some(ref v) = self.filter_group { p.push(Box::new(v.clone())); }
        if let Some(ref v) = self.filter_model { p.push(Box::new(v.clone())); }
        if let Some(ref v) = self.filter_protocol { p.push(Box::new(v.clone())); }
        p
    }
}

pub fn query_stats(db: &Db, query: &StatsQuery) -> Result<StatsResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let end = query.end.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let start = query.start.unwrap_or_else(|| {
        (chrono::Utc::now() - chrono::Duration::days(7)).timestamp_millis()
    });

    let qp = QueryParams {
        start,
        end,
        filter_group: query.filter_group.clone(),
        filter_model: query.filter_model.clone(),
        filter_protocol: query.filter_protocol.clone(),
    };

    // Build WHERE clause
    let mut where_parts = vec!["created_at >= ?1".to_string(), "created_at <= ?2".to_string()];
    if qp.filter_group.is_some() {
        where_parts.push("group_name = ?3".to_string());
    }
    if qp.filter_model.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize;
        where_parts.push(format!("(model = ?{idx} OR actual_model = ?{idx})"));
    }
    if qp.filter_protocol.is_some() {
        let idx = 3 + qp.filter_group.is_some() as usize + qp.filter_model.is_some() as usize;
        where_parts.push(format!("target_protocol = ?{idx}"));
    }
    let where_sql = where_parts.join(" AND ");

    let time_fmt = match query.granularity.as_deref() {
        Some("hourly") => "%Y-%m-%d %H:00",
        _ => "%Y-%m-%d",
    };

    // ── Overview ──
    let overview_sql = format!(
        "SELECT COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms) \
         FROM proxy_log WHERE deleted_at = 0 AND {where_sql}"
    );
    let p = qp.to_sql_params();
    let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
    let overview = conn.prepare(&overview_sql)
        .map_err(|e| e.to_string())?
        .query_row(refs.as_slice(), |row| {
            let total: i32 = row.get(0).unwrap_or(0);
            let success: i32 = row.get(1).unwrap_or(0);
            Ok(StatsOverview {
                total_requests: total,
                success_rate: if total > 0 { success as f64 / total as f64 * 100.0 } else { 0.0 },
                total_input_tokens: row.get(2).unwrap_or(0),
                total_output_tokens: row.get(3).unwrap_or(0),
                total_cache_tokens: row.get(4).unwrap_or(0),
                cache_rate: {
                    let inp: i64 = row.get(2).unwrap_or(0);
                    if inp > 0 { row.get::<_, i64>(4).unwrap_or(0) as f64 / inp as f64 * 100.0 } else { 0.0 }
                },
                avg_duration_ms: row.get(5).unwrap_or(0.0),
            })
        }).map_err(|e| format!("overview: {e}"))?;

    // ── Time buckets ──
    let bucket_sql = format!(
        "SELECT strftime('{time_fmt}', created_at/1000, 'unixepoch'), COUNT(*), \
         SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
         SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END), \
         SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms) \
         FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 1"
    );
    let p = qp.to_sql_params();
    let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
    let buckets: Vec<StatsBucket> = conn.prepare(&bucket_sql)
        .map_err(|e| e.to_string())?
        .query_map(refs.as_slice(), |row| {
            Ok(StatsBucket {
                time_bucket: row.get(0).unwrap_or_default(),
                total_requests: row.get(1).unwrap_or(0),
                success_count: row.get(2).unwrap_or(0),
                error_count: row.get(3).unwrap_or(0),
                input_tokens: row.get(4).unwrap_or(0),
                output_tokens: row.get(5).unwrap_or(0),
                cache_tokens: row.get(6).unwrap_or(0),
                avg_duration_ms: row.get(7).unwrap_or(0.0),
            })
        }).map_err(|e| format!("buckets: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // ── Dimension breakdown ──
    let dimension_data = if let Some(ref gb) = query.group_by {
        let dim_col = match gb.as_str() {
            "platform" => "target_protocol",
            "model" => "actual_model",
            "group" => "group_name",
            _ => "target_protocol",
        };
        let dim_sql = format!(
            "SELECT {dim_col}, COUNT(*), \
             SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END), \
             SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens), AVG(duration_ms) \
             FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 2 DESC LIMIT 50"
        );
        let p = qp.to_sql_params();
        let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
        conn.prepare(&dim_sql)
            .map_err(|e| e.to_string())?
            .query_map(refs.as_slice(), |row| {
                Ok(DimensionEntry {
                    name: row.get(0).unwrap_or_default(),
                    total_requests: row.get(1).unwrap_or(0),
                    success_count: row.get(2).unwrap_or(0),
                    input_tokens: row.get(3).unwrap_or(0),
                    output_tokens: row.get(4).unwrap_or(0),
                    cache_tokens: row.get(5).unwrap_or(0),
                    avg_duration_ms: row.get(6).unwrap_or(0.0),
                })
            }).map_err(|e| format!("dimension: {e}"))?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        vec![]
    };

    Ok(StatsResult { overview, buckets, dimension_data })
}
