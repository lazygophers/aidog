use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use std::sync::Mutex;

use super::models::*;

pub struct Db(pub Mutex<Connection>);

impl Db {
    pub fn new(path: &str) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn init_tables(&self) -> Result<(), String> {
        let sql = include_str!("../../migrations/001_init.sql");
        // 只执行 CREATE TABLE 语句（跳过 PRAGMA，因为已在 new 中设置）
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(sql).map_err(|e| e.to_string())?;
        Ok(())
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

pub fn create_platform(db: &Db, input: CreatePlatform) -> Result<Platform, String> {
    let id = new_id();
    let now = now();
    let protocol_str = serde_json::to_string(&input.protocol).unwrap();
    let platform = Platform {
        id: id.clone(),
        name: input.name,
        protocol: input.protocol,
        base_url: input.base_url,
        api_key: input.api_key,
        extra: input.extra,
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
    };

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO platforms (id, name, protocol, base_url, api_key, extra, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, platform.name, protocol_str, platform.base_url, platform.api_key, platform.extra, platform.enabled as i64, platform.created_at, platform.updated_at],
    )
    .map_err(|e| format!("create platform: {e}"))?;

    Ok(platform)
}

pub fn list_platforms(db: &Db) -> Result<Vec<Platform>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, protocol, base_url, api_key, extra, enabled, created_at, updated_at FROM platforms ORDER BY created_at")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let protocol_str: String = row.get(2)?;
            Ok(Platform {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: serde_json::from_str(&protocol_str).unwrap(),
                base_url: row.get(3)?,
                api_key: row.get(4)?,
                extra: row.get(5)?,
                enabled: row.get::<_, i64>(6)? == 1,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn get_platform(db: &Db, id: &str) -> Result<Option<Platform>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, protocol, base_url, api_key, extra, enabled, created_at, updated_at FROM platforms WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(params![id], |row| {
            let protocol_str: String = row.get(2)?;
            Ok(Platform {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: serde_json::from_str(&protocol_str).unwrap(),
                base_url: row.get(3)?,
                api_key: row.get(4)?,
                extra: row.get(5)?,
                enabled: row.get::<_, i64>(6)? == 1,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
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
        enabled: input.enabled.unwrap_or(existing.enabled),
        updated_at: now(),
        ..existing
    };

    let protocol_str = serde_json::to_string(&updated.protocol).unwrap();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE platforms SET name=?1, protocol=?2, base_url=?3, api_key=?4, extra=?5, enabled=?6, updated_at=?7 WHERE id=?8",
        params![
            updated.name,
            protocol_str,
            updated.base_url,
            updated.api_key,
            updated.extra,
            updated.enabled as i64,
            updated.updated_at,
            updated.id,
        ],
    )
    .map_err(|e| format!("update platform: {e}"))?;

    Ok(updated)
}

pub fn delete_platform(db: &Db, id: &str) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM platforms WHERE id = ?1", params![id])
        .map_err(|e| format!("delete platform: {e}"))?;
    Ok(())
}

// ─── Group CRUD ────────────────────────────────────────────

pub fn create_group(db: &Db, input: CreateGroup) -> Result<Group, String> {
    let id = new_id();
    let now = now();
    let routing_str = serde_json::to_string(&input.routing_mode).unwrap();
    let group = Group {
        id: id.clone(),
        name: input.name,
        path: input.path,
        routing_mode: input.routing_mode,
        created_at: now.clone(),
        updated_at: now,
    };

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO groups (id, name, path, routing_mode, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, group.name, group.path, routing_str, group.created_at, group.updated_at],
    )
    .map_err(|e| format!("create group: {e}"))?;

    Ok(group)
}

pub fn list_groups(db: &Db) -> Result<Vec<Group>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, path, routing_mode, created_at, updated_at FROM groups ORDER BY created_at")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let routing_str: String = row.get(3)?;
            Ok(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                routing_mode: serde_json::from_str(&routing_str).unwrap(),
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<SqlResult<Vec<_>>>().map_err(|e| e.to_string())
}

pub fn get_group(db: &Db, id: &str) -> Result<Option<Group>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, path, routing_mode, created_at, updated_at FROM groups WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(params![id], |row| {
            let routing_str: String = row.get(3)?;
            Ok(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                routing_mode: serde_json::from_str(&routing_str).unwrap(),
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
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
            "SELECT gp.priority, gp.weight, p.id, p.name, p.protocol, p.base_url, p.api_key, p.extra, p.enabled, p.created_at, p.updated_at \
             FROM group_platforms gp JOIN platforms p ON gp.platform_id = p.id \
             WHERE gp.group_id = ?1 ORDER BY gp.priority",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![group_id], |row| {
            let protocol_str: String = row.get(4)?;
            Ok(GroupPlatformDetail {
                platform: Platform {
                    id: row.get(2)?,
                    name: row.get(3)?,
                    protocol: serde_json::from_str(&protocol_str).unwrap(),
                    base_url: row.get(5)?,
                    api_key: row.get(6)?,
                    extra: row.get(7)?,
                    enabled: row.get::<_, i64>(8)? == 1,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
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
