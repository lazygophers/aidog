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

    /// One-time fix: normalize all group names to slug format
    pub fn fix_group_names(&self) {
        let conn = match self.0.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let _ = conn.execute_batch(
            "UPDATE groups SET name = REPLACE(REPLACE(REPLACE(REPLACE(LOWER(name),' ','-'),'（','-'),'(','-'),'）','-');
             UPDATE groups SET name = REPLACE(name, ')', '-');
             UPDATE groups SET name = REPLACE(name, '--', '-');
             UPDATE groups SET name = REPLACE(name, '--', '-');
             UPDATE groups SET name = LTRIM(name, '-');
             UPDATE groups SET name = RTRIM(name, '-');"
        );
    }
}

fn now() -> String {
    chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ─── Platform CRUD ─────────────────────────────────────────

/// SELECT 列序
const PLATFORM_COLUMNS: &str =
    "id, name, protocol, base_url, api_key, extra, models, available_models, enabled, created_at, updated_at";

/// 从查询行构造 Platform
fn row_to_platform(row: &rusqlite::Row) -> SqlResult<Platform> {
    let protocol_str: String = row.get(2)?;
    let models_str: String = row.get(6)?;
    let available_str: String = row.get(7)?;
    Ok(Platform {
        id: row.get(0)?,
        name: row.get(1)?,
        protocol: serde_json::from_str(&protocol_str).unwrap(),
        base_url: row.get(3)?,
        api_key: row.get(4)?,
        extra: row.get(5)?,
        models: parse_models(&models_str),
        available_models: parse_available_models(&available_str),
        enabled: row.get::<_, i64>(8)? == 1,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub fn create_platform(db: &Db, mut input: CreatePlatform) -> Result<Platform, String> {
    let id = new_id();
    let ts = now();
    let protocol_str = serde_json::to_string(&input.protocol).unwrap();
    // If name is empty, auto-generate: {protocol}-{random8}
    if input.name.trim().is_empty() {
        let proto_label = format!("{:?}", input.protocol).to_lowercase();
        let rand_suffix = &uuid::Uuid::new_v4().to_string()[..8];
        input.name = format!("{}-{}", proto_label, rand_suffix);
    }
    let models = input.models.unwrap_or_default();
    let models_str = serialize_models(&models);
    let available_models = input.available_models.unwrap_or_default();
    let available_str = serialize_available_models(&available_models);
    let platform = Platform {
        id: id.clone(),
        name: input.name,
        protocol: input.protocol,
        base_url: input.base_url,
        api_key: input.api_key,
        extra: input.extra,
        models,
        available_models,
        enabled: true,
        created_at: ts.clone(),
        updated_at: ts,
    };

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!("INSERT INTO platforms ({PLATFORM_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"),
        params![id, platform.name, protocol_str, platform.base_url, platform.api_key, platform.extra, models_str, available_str, platform.enabled as i64, platform.created_at, platform.updated_at],
    )
    .map_err(|e| format!("create platform: {e}"))?;

    Ok(platform)
}

pub fn list_platforms(db: &Db) -> Result<Vec<Platform>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platforms ORDER BY created_at");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_platform)
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn get_platform(db: &Db, id: &str) -> Result<Option<Platform>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let sql = format!("SELECT {PLATFORM_COLUMNS} FROM platforms WHERE id = ?1");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(params![id], row_to_platform)
        .optional()
        .map_err(|e| e.to_string())?;

    Ok(result)
}

pub fn update_platform(db: &Db, input: UpdatePlatform) -> Result<Platform, String> {
    let existing = get_platform(db, &input.id)?.ok_or("platform not found")?;

    let updated = Platform {
        name: input.name.unwrap_or(existing.name),
        protocol: input.protocol.unwrap_or(existing.protocol),
        base_url: input.base_url.unwrap_or(existing.base_url),
        api_key: input.api_key.unwrap_or(existing.api_key),
        extra: input.extra.or(existing.extra),
        models: input.models.unwrap_or(existing.models),
        available_models: input.available_models.unwrap_or(existing.available_models),
        enabled: input.enabled.unwrap_or(existing.enabled),
        updated_at: now(),
        ..existing
    };

    let protocol_str = serde_json::to_string(&updated.protocol).unwrap();
    let models_str = serialize_models(&updated.models);
    let available_str = serialize_available_models(&updated.available_models);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE platforms SET name=?1, protocol=?2, base_url=?3, api_key=?4, extra=?5, models=?6, available_models=?7, enabled=?8, updated_at=?9 WHERE id=?10",
        params![
            updated.name,
            protocol_str,
            updated.base_url,
            updated.api_key,
            updated.extra,
            models_str,
            available_str,
            updated.enabled as i64,
            updated.updated_at,
            updated.id,
        ],
    )
    .map_err(|e| format!("update platform: {e}"))?;

    Ok(updated)
}

pub fn delete_platform(db: &Db, id: &str) -> Result<(), String> {
    // 删除关联的自动分组
    let conn_inner = db.0.lock().map_err(|e| e.to_string())?;
    let auto_group_ids: Vec<String> = conn_inner
        .prepare("SELECT id FROM groups WHERE auto_from_platform = ?1")
        .map_err(|e| e.to_string())?
        .query_map(params![id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    drop(conn_inner);

    for gid in &auto_group_ids {
        force_delete_group(db, gid)?;
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM platforms WHERE id = ?1", params![id])
        .map_err(|e| format!("delete platform: {e}"))?;
    Ok(())
}

// ─── Group CRUD ────────────────────────────────────────────

pub fn create_group(db: &Db, input: CreateGroup) -> Result<Group, String> {
    let id = new_id();
    let ts = now();
    let routing_str = serde_json::to_string(&input.routing_mode).unwrap();
    let group = Group {
        id: id.clone(),
        name: input.name,
        path: input.path,
        routing_mode: input.routing_mode,
        auto_from_platform: input.auto_from_platform.clone(),
        created_at: ts.clone(),
        updated_at: ts,
    };

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO groups (id, name, path, routing_mode, auto_from_platform, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, group.name, group.path, routing_str, group.auto_from_platform, group.created_at, group.updated_at],
    )
    .map_err(|e| format!("create group: {e}"))?;

    Ok(group)
}

pub fn list_groups(db: &Db) -> Result<Vec<Group>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, path, routing_mode, auto_from_platform, created_at, updated_at FROM groups ORDER BY created_at")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let routing_str: String = row.get(3)?;
            Ok(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                routing_mode: serde_json::from_str(&routing_str).unwrap(),
                auto_from_platform: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn get_group(db: &Db, id: &str) -> Result<Option<Group>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, path, routing_mode, auto_from_platform, created_at, updated_at FROM groups WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(params![id], |row| {
            let routing_str: String = row.get(3)?;
            Ok(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                routing_mode: serde_json::from_str(&routing_str).unwrap(),
                auto_from_platform: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .optional()
        .map_err(|e| e.to_string())?;

    Ok(result)
}

pub fn update_group(db: &Db, input: UpdateGroup) -> Result<Group, String> {
    let existing = get_group(db, &input.id)?.ok_or("group not found")?;

    let updated = Group {
        name: input.name.unwrap_or(existing.name),
        path: input.path.unwrap_or(existing.path),
        routing_mode: input.routing_mode.unwrap_or(existing.routing_mode),
        updated_at: now(),
        ..existing
    };

    let routing_str = serde_json::to_string(&updated.routing_mode).unwrap();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE groups SET name=?1, path=?2, routing_mode=?3, updated_at=?4 WHERE id=?5",
        params![updated.name, updated.path, routing_str, updated.updated_at, updated.id],
    )
    .map_err(|e| format!("update group: {e}"))?;

    Ok(updated)
}

pub fn delete_group(db: &Db, id: &str) -> Result<(), String> {
    // 检查是否为自动分组
    let group = get_group(db, id)?.ok_or("group not found")?;
    if group.auto_from_platform.is_some() {
        return Err("auto-created group cannot be deleted manually".to_string());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM groups WHERE id = ?1", params![id])
        .map_err(|e| format!("delete group: {e}"))?;
    Ok(())
}

/// 强制删除分组（含自动分组），仅供平台删除时内部调用
pub fn force_delete_group(db: &Db, id: &str) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM groups WHERE id = ?1", params![id])
        .map_err(|e| format!("delete group: {e}"))?;
    Ok(())
}

// ─── GroupPlatform 关联 ────────────────────────────────────

pub fn set_group_platforms(
    db: &Db,
    group_id: &str,
    platforms: &[GroupPlatformInput],
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM group_platforms WHERE group_id = ?1",
        params![group_id],
    )
    .map_err(|e| e.to_string())?;

    for p in platforms {
        conn.execute(
            "INSERT INTO group_platforms (group_id, platform_id, priority, weight) VALUES (?1, ?2, ?3, ?4)",
            params![group_id, p.platform_id, p.priority.unwrap_or(0), p.weight.unwrap_or(1)],
        )
        .map_err(|e| format!("insert group platform: {e}"))?;
    }

    Ok(())
}

pub fn get_group_platforms(db: &Db, group_id: &str) -> Result<Vec<GroupPlatformDetail>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            &format!(
                "SELECT gp.priority, gp.weight, p.{PLATFORM_COLUMNS} \
                 FROM group_platforms gp JOIN platforms p ON gp.platform_id = p.id \
                 WHERE gp.group_id = ?1 ORDER BY gp.priority"
            ),
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![group_id], |row| {
            // row layout: priority(0), weight(1), then platform columns starting at 2
            let protocol_str: String = row.get(4)?;
            let models_str: String = row.get(8)?;
            let available_str: String = row.get(9)?;
            Ok(GroupPlatformDetail {
                platform: Platform {
                    id: row.get(2)?,
                    name: row.get(3)?,
                    protocol: serde_json::from_str(&protocol_str).unwrap(),
                    base_url: row.get(5)?,
                    api_key: row.get(6)?,
                    extra: row.get(7)?,
                    models: parse_models(&models_str),
                    available_models: parse_available_models(&available_str),
                    enabled: row.get::<_, i64>(10)? == 1,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                },
                priority: row.get(0)?,
                weight: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

// ─── ModelMapping CRUD ─────────────────────────────────────

pub fn create_model_mapping(db: &Db, input: CreateModelMapping) -> Result<ModelMapping, String> {
    let id = new_id();
    let mapping = ModelMapping {
        id: id.clone(),
        group_id: input.group_id,
        source_model: input.source_model,
        target_platform_id: input.target_platform_id,
        target_model: input.target_model,
    };

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO model_mappings (id, group_id, source_model, target_platform_id, target_model) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, mapping.group_id, mapping.source_model, mapping.target_platform_id, mapping.target_model],
    )
    .map_err(|e| format!("create model mapping: {e}"))?;

    Ok(mapping)
}

pub fn list_model_mappings(db: &Db, group_id: &str) -> Result<Vec<ModelMapping>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, group_id, source_model, target_platform_id, target_model FROM model_mappings WHERE group_id = ?1 ORDER BY source_model")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![group_id], |row| {
            Ok(ModelMapping {
                id: row.get(0)?,
                group_id: row.get(1)?,
                source_model: row.get(2)?,
                target_platform_id: row.get(3)?,
                target_model: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn update_model_mapping(db: &Db, input: UpdateModelMapping) -> Result<ModelMapping, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, group_id, source_model, target_platform_id, target_model FROM model_mappings WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    let existing = stmt
        .query_row(params![input.id], |row| {
            Ok(ModelMapping {
                id: row.get(0)?,
                group_id: row.get(1)?,
                source_model: row.get(2)?,
                target_platform_id: row.get(3)?,
                target_model: row.get(4)?,
            })
        })
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or("model mapping not found")?;

    let updated = ModelMapping {
        source_model: input.source_model.unwrap_or(existing.source_model),
        target_platform_id: input.target_platform_id.unwrap_or(existing.target_platform_id),
        target_model: input.target_model.unwrap_or(existing.target_model),
        ..existing
    };

    conn.execute(
        "UPDATE model_mappings SET source_model=?1, target_platform_id=?2, target_model=?3 WHERE id=?4",
        params![updated.source_model, updated.target_platform_id, updated.target_model, updated.id],
    )
    .map_err(|e| format!("update model mapping: {e}"))?;

    Ok(updated)
}

pub fn delete_model_mapping(db: &Db, id: &str) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM model_mappings WHERE id = ?1", params![id])
        .map_err(|e| format!("delete model mapping: {e}"))?;
    Ok(())
}

// ─── 聚合查询 ──────────────────────────────────────────────

pub fn get_group_detail(db: &Db, id: &str) -> Result<Option<GroupDetail>, String> {
    let group = match get_group(db, id)? {
        Some(g) => g,
        None => return Ok(None),
    };
    let platforms = get_group_platforms(db, id)?;
    let model_mappings = list_model_mappings(db, id)?;

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
        let platforms = get_group_platforms(db, &g.id)?;
        let model_mappings = list_model_mappings(db, &g.id)?;
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
        .prepare("SELECT value FROM settings WHERE scope = ?1 AND key = ?2")
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
        "INSERT INTO settings (scope, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(scope, key) DO UPDATE SET value = ?3, updated_at = ?4",
        params![input.scope, input.key, value_str, ts],
    )
    .map_err(|e| format!("upsert setting: {e}"))?;
    Ok(())
}

pub fn delete_setting(db: &Db, scope: &str, key: &str) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM settings WHERE scope = ?1 AND key = ?2",
        params![scope, key],
    )
    .map_err(|e| format!("delete setting: {e}"))?;
    Ok(())
}

pub fn list_setting_keys(db: &Db, scope: &str) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT key FROM settings WHERE scope = ?1 ORDER BY key")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![scope], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

// ─── ProxyLog CRUD ─────────────────────────────────────────

pub fn insert_proxy_log(db: &Db, log: &super::models::ProxyLog) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO proxy_logs (id, group_name, model, request_headers, request_body, response_body, status_code, duration_ms, input_tokens, output_tokens, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![log.id, log.group_name, log.model, log.request_headers, log.request_body, log.response_body, log.status_code, log.duration_ms, log.input_tokens, log.output_tokens, log.created_at],
    ).map_err(|e| format!("insert proxy log: {e}"))?;
    Ok(())
}

pub fn list_proxy_logs(db: &Db, limit: u32, offset: u32) -> Result<Vec<super::models::ProxyLogSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, group_name, model, status_code, duration_ms, input_tokens, output_tokens, created_at
             FROM proxy_logs ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit, offset], |row| {
            Ok(super::models::ProxyLogSummary {
                id: row.get(0)?,
                group_name: row.get(1)?,
                model: row.get(2)?,
                status_code: row.get(3)?,
                duration_ms: row.get(4)?,
                input_tokens: row.get(5)?,
                output_tokens: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn get_proxy_log(db: &Db, id: &str) -> Result<Option<super::models::ProxyLog>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, group_name, model, request_headers, request_body, response_body, status_code, duration_ms, input_tokens, output_tokens, created_at
             FROM proxy_logs WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;
    stmt.query_row(params![id], |row| {
        Ok(super::models::ProxyLog {
            id: row.get(0)?,
            group_name: row.get(1)?,
            model: row.get(2)?,
            request_headers: row.get(3)?,
            request_body: row.get(4)?,
            response_body: row.get(5)?,
            status_code: row.get(6)?,
            duration_ms: row.get(7)?,
            input_tokens: row.get(8)?,
            output_tokens: row.get(9)?,
            created_at: row.get(10)?,
        })
    })
    .optional()
    .map_err(|e| e.to_string())
}

pub fn clear_proxy_logs(db: &Db) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM proxy_logs", [])
        .map_err(|e| format!("clear proxy logs: {e}"))?;
    Ok(())
}

/// Delete logs older than N days. Pass 0 to skip.
pub fn cleanup_proxy_logs(db: &Db, retention_days: u32) -> Result<(), String> {
    if retention_days == 0 {
        return Ok(());
    }
    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
    let cutoff_str = cutoff.to_rfc3339();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM proxy_logs WHERE created_at < ?1", params![cutoff_str])
        .map_err(|e| format!("cleanup proxy logs: {e}"))?;
    Ok(())
}

pub fn count_proxy_logs(db: &Db) -> Result<u32, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT COUNT(*) FROM proxy_logs", [], |row| row.get(0))
        .map_err(|e| e.to_string())
}
