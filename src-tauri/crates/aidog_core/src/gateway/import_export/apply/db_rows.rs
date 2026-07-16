//! db 行级 upsert：group / platform / group_platform / setting + auto-group。

use super::json_helpers::{
    json_bool, json_f64, json_i64, json_raw, json_str, json_u32, json_u64, now_ts,
};
use crate::gateway::db::Db;

// ── db 行级 upsert（按 group_key 查重；name 作显示名可重命名；group_key 锁定不改） ──

pub(super) async fn upsert_group_row(
    db: &Db,
    group_key: &str,
    effective_name: &str,
    row: &serde_json::Value,
) -> Result<(), String> {
    let row = row.clone();
    let group_key = group_key.to_string();
    let effective_name = effective_name.to_string();
    // config-db-split：group 表落 platform.db，走 platform 写连接。
    db.platform_write_conn()
        .call(move |conn| {
            let tx = conn.transaction()?;
            let existing_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM \"group\" WHERE group_key = ?1 AND deleted_at = 0",
                    [&group_key],
                    |r| r.get(0),
                )
                .ok();
            let now = now_ts();
            if let Some(id) = existing_id {
                tx.execute(
                    "UPDATE \"group\" SET name = ?1 WHERE id = ?2",
                    rusqlite::params![&effective_name, id],
                )?;
                update_group_cols(&tx, id, &row, &effective_name)?;
            } else {
                let routing_mode = routing_mode_json(&row);
                let auto_from_platform = row
                    .get("auto_from_platform")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                tx.execute(
                    "INSERT INTO \"group\" (name, group_key, routing_mode, auto_from_platform, sort_order, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
                    rusqlite::params![&effective_name, &group_key, &routing_mode, &auto_from_platform, now],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert group: {e}"))
}

/// routing_mode 在 DB 中以 `serde_json::to_string` 存储、读取时 `from_str` 反序列化。
/// 导出 payload 的 routing_mode 是 JSON 字符串变体（如 `"failover"`），须保留引号框写回，
/// 否则 `from_str("failover")` 报 "expected value"。缺省 fallback 用 JSON 引号形式。
fn routing_mode_json(row: &serde_json::Value) -> String {
    match row.get("routing_mode") {
        Some(serde_json::Value::Null) | None => "\"load_balance\"".to_string(),
        Some(v) => v.to_string(),
    }
}

fn update_group_cols(
    tx: &rusqlite::Transaction,
    id: i64,
    row: &serde_json::Value,
    effective: &str,
) -> rusqlite::Result<()> {
    let now = now_ts();
    let routing_mode = routing_mode_json(row);
    let auto_from_platform = row
        .get("auto_from_platform")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let sort_order = row
        .get("sort_order")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    tx.execute(
        "UPDATE \"group\" SET name = ?1, routing_mode = ?2, auto_from_platform = ?3, sort_order = ?4, updated_at = ?5 WHERE id = ?6",
        rusqlite::params![effective, routing_mode, auto_from_platform, sort_order, now, id],
    )?;
    Ok(())
}

pub(super) async fn upsert_platform_row(
    db: &Db,
    _original_name: &str,
    effective_name: &str,
    row: &serde_json::Value,
) -> Result<(), String> {
    // platform.name 非唯一（platform 表无 UNIQUE；唯一性在 group.name）。
    // 旧逻辑按 name SELECT→UPDATE 在多同名时取任一行 = 覆盖错平台（数据完整性 bug）。
    // 无稳定跨机 platform identity（id 机器本地）→ 始终 INSERT 新行。
    // 重复导入同 provider = 列表多个同名 platform（用户确认接受）。
    // effective_name 仍尊重 rename 决策（若 .aidogx 传 rename）。
    let row = row.clone();
    let effective = effective_name.to_string();
    // config-db-split：platform 表落 platform.db，走 platform 写连接。
    db.platform_write_conn()
        .call(move |conn| {
            let tx = conn.transaction()?;
            let now = now_ts();
            insert_platform_row(&tx, &effective, &row, now)?;
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| format!("insert platform: {e}"))
}

fn insert_platform_row(
    tx: &rusqlite::Transaction,
    name: &str,
    row: &serde_json::Value,
    now: i64,
) -> rusqlite::Result<()> {
    // breaker 阈值现存于 extra.breaker。新格式导出已含 extra.breaker；旧格式（breaker 在顶层）
    // 双兜底：若顶层有非 0 breaker 列且 extra 内尚无 breaker，则合并进 extra（无损迁入）。
    let extra = effective_extra_with_breaker(row);
    // 新格式导出清洗后空配置字段被省略 → json_str 缺失给空串。空串非合法 JSON，
    // read 端 parse_models/parse_available_models/parse_endpoints 会刷 warn 日志（淹没真实问题）。
    // 故缺失/空时写标准空 JSON（与 db.rs create_platform serialize_* 默认值对齐）。
    let models = json_str(row, "models");
    let models = if models.trim().is_empty() { "{}".to_string() } else { models };
    let available_models = json_str(row, "available_models");
    let available_models = if available_models.trim().is_empty() {
        "[]".to_string()
    } else {
        available_models
    };
    let endpoints = json_str(row, "endpoints");
    let endpoints = if endpoints.trim().is_empty() { "[]".to_string() } else { endpoints };
    tx.execute(
        "INSERT INTO platform
         (name, platform_type, base_url, api_key, extra, models, available_models, endpoints,
          enabled, status, auto_disabled_until, auto_disable_strikes,
          created_at, updated_at, deleted_at,
          est_balance_remaining, est_coding_plan, last_real_query_at, estimate_count,
          show_in_tray, tray_display, sort_order, manual_budgets)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13,0,?14,?15,?16,?17,?18,?19,?20,?21)",
        rusqlite::params![
            name,
            json_raw(row, "platform_type"),
            json_str(row, "base_url"),
            json_str(row, "api_key"),
            extra,
            models,
            available_models,
            endpoints,
            json_bool(row, "enabled"),
            json_str(row, "status"),
            json_i64(row, "auto_disabled_until"),
            json_i64(row, "auto_disable_strikes"),
            now,
            json_f64(row, "est_balance_remaining"),
            json_str(row, "est_coding_plan"),
            json_i64(row, "last_real_query_at"),
            json_i64(row, "estimate_count"),
            json_bool(row, "show_in_tray"),
            json_str(row, "tray_display"),
            json_i64(row, "sort_order"),
            json_str(row, "manual_budgets"),
        ],
    )?;
    Ok(())
}

/// 取导入行的 extra，并兼容旧格式：顶层 breaker_* 非 0 且 extra 尚无 breaker → 合并进 extra.breaker。
/// 新格式 extra 已含 breaker → 原样保留（不被顶层覆盖）。
pub(super) fn effective_extra_with_breaker(row: &serde_json::Value) -> String {
    let extra = json_str(row, "extra");
    // extra 内已有 breaker 覆盖 → 直接用。
    let has_extra_breaker = crate::gateway::models::parse_breaker(&extra);
    if has_extra_breaker.failure_threshold != 0
        || has_extra_breaker.open_secs != 0
        || has_extra_breaker.half_open_max != 0
    {
        return extra;
    }
    let ft = json_u32(row, "breaker_failure_threshold");
    let os = json_u64(row, "breaker_open_secs");
    let hom = json_u32(row, "breaker_half_open_max");
    if ft == 0 && os == 0 && hom == 0 {
        return extra;
    }
    crate::gateway::models::merge_breaker_into_extra(
        &extra,
        &crate::gateway::models::PlatformBreaker {
            failure_threshold: ft,
            open_secs: os,
            half_open_max: hom,
        },
    )
}

pub(super) async fn relink_group_platform(db: &Db, group_key: &str, platform_name: &str) -> Result<(), String> {
    let g = group_key.to_string();
    let p = platform_name.to_string();
    // config-db-split：group/platform/group_platform 表落 platform.db，走 platform 写连接。
    db.platform_write_conn()
        .call(move |conn| {
            let tx = conn.transaction()?;
            let gid: Option<i64> = tx
                .query_row(
                    "SELECT id FROM \"group\" WHERE name = ?1 AND deleted_at = 0",
                    [&g],
                    |r| r.get(0),
                )
                .ok();
            let pid: Option<i64> = tx
                .query_row(
                    "SELECT id FROM platform WHERE name = ?1 AND deleted_at = 0",
                    [&p],
                    |r| r.get(0),
                )
                .ok();
            match (gid, pid) {
                (Some(gid), Some(pid)) => {
                    let now = now_ts();
                    tx.execute(
                        "INSERT INTO group_platform (group_id, platform_id, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?3)
                         ON CONFLICT(group_id, platform_id) DO UPDATE SET deleted_at = 0, updated_at = ?3",
                        rusqlite::params![gid, pid, now],
                    )?;
                    tx.commit()?;
                    Ok(())
                }
                _ => Err(tokio_rusqlite::Error::Other(
                    format!("missing group/platform: {g} / {p}").into(),
                )),
            }
        })
        .await
        .map_err(|e| format!("relink: {e}"))
}

// ── 导入 auto-group（sub2api / cc-switch 两路共享） ──────────────
//
// 根因约束：apply 走 insert_platform_row 直接 INSERT，不触发 platform_create
// 命令级 auto-group 副作用（记忆 import-apply-bypasses-platform-create），故
// auto-group 必须显式做。
//
// 去重策略（main 拍板）：
// - group 按 name 查找复用（ensure-by-name，不重复建同名组）；
// - 平台接受重复（platform.name 非 UNIQUE，重复导入重复建平台 = always-INSERT 语义），
//   关联用本次导入新建的 platform_id 集合，不做跨次去重。

/// 快照当前未删除 platform 的 id 集合（apply 前调用，用于回出本次新建行）。
pub async fn snapshot_platform_ids(db: &Db) -> Result<std::collections::BTreeSet<i64>, String> {
    // config-db-split：platform 表落 platform.db，走 platform 写连接。
    db.platform_write_conn()
        .call(|conn| {
            let mut stmt =
                conn.prepare("SELECT id FROM platform WHERE deleted_at = 0")?;
            let ids = stmt
                .query_map([], |r| r.get::<_, i64>(0))?
                .collect::<Result<std::collections::BTreeSet<i64>, _>>()?;
            Ok(ids)
        })
        .await
        .map_err(|e| format!("snapshot platform ids: {e}"))
}

/// ensure group(name) 幂等（同名复用，不存在则 create 生成 gk_<32hex>）+ 关联 platform_ids。
///
/// `before` 为 apply 前的 platform id 快照；本函数内部重新取全量 id，差集 = 本次新建。
/// 关联走 group_platform ON CONFLICT 幂等（apply.rs relink 同语义）。
pub async fn ensure_group_and_attach(
    db: &Db,
    group_name: &str,
    before: &std::collections::BTreeSet<i64>,
) -> Result<(), String> {
    let group_name = group_name.to_string();
    let before = before.clone();
    // config-db-split：group/platform/group_platform 表落 platform.db，走 platform 写连接。
    db.platform_write_conn()
        .call(move |conn| {
            let tx = conn.transaction()?;
            // 1. ensure group by name（命中复用；未命中 create 生成 group_key）。
            let gid: i64 = match tx
                .query_row(
                    "SELECT id FROM \"group\" WHERE name = ?1 AND deleted_at = 0",
                    [&group_name],
                    |r| r.get(0),
                )
                .ok()
            {
                Some(id) => id,
                None => {
                    let now = now_ts();
                    let group_key = format!("gk_{}", uuid::Uuid::new_v4().simple());
                    tx.execute(
                        "INSERT INTO \"group\" (name, group_key, routing_mode, auto_from_platform, sort_order, created_at, updated_at)
                         VALUES (?1, ?2, '\"load_balance\"', '', 0, ?3, ?3)",
                        rusqlite::params![&group_name, &group_key, now],
                    )?;
                    tx.last_insert_rowid()
                }
            };

            // 2. 本次新建的 platform id = 全量 − before 快照。
            let new_ids: Vec<i64> = {
                let mut stmt =
                    tx.prepare("SELECT id FROM platform WHERE deleted_at = 0")?;
                let all = stmt
                    .query_map([], |r| r.get::<_, i64>(0))?
                    .collect::<Result<Vec<i64>, _>>()?;
                all.into_iter().filter(|id| !before.contains(id)).collect()
            };

            // 3. attach（ON CONFLICT 幂等）。
            let now = now_ts();
            for pid in new_ids {
                tx.execute(
                    "INSERT INTO group_platform (group_id, platform_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?3)
                     ON CONFLICT(group_id, platform_id) DO UPDATE SET deleted_at = 0, updated_at = ?3",
                    rusqlite::params![gid, pid, now],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| format!("ensure_group_and_attach: {e}"))?;
    db.invalidate_hot_caches();
    Ok(())
}

pub(super) async fn upsert_setting_row(
    db: &Db,
    scope: &str,
    key: &str,
    value_json: &str,
) -> Result<(), String> {
    let scope = scope.to_string();
    let key = key.to_string();
    let value = value_json.to_string();
    db.write_conn()
        .call(move |conn| {
            let now = now_ts();
            conn.execute(
                "INSERT INTO setting (scope, key, value, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(scope, key) DO UPDATE SET value = ?3, updated_at = ?4, deleted_at = 0",
                rusqlite::params![&scope, &key, &value, now],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert setting: {e}"))
}

/// 按 name 查重写入中间件规则（middleware_rule 表无 name UNIQUE 约束，故手动查重避免重复导入）。
/// 命中同名则 UPDATE（保留原 id/created_at），否则 INSERT。
pub(super) async fn upsert_middleware_rule_by_name(
    db: &Db,
    rule: &crate::gateway::models::MiddlewareRule,
) -> Result<(), String> {
    let r = rule.clone();
    db.write_conn()
        .call(move |conn| {
            let now = now_ts();
            let existing_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM middleware_rule WHERE name = ?1",
                    [&r.name],
                    |row| row.get(0),
                )
                .ok();
            let rule_type = r.rule_type.as_str();
            let scope = r.scope.as_str();
            let match_type = r.match_type.as_str();
            let action = r.action.as_str();
            if let Some(id) = existing_id {
                conn.execute(
                    "UPDATE middleware_rule SET
                       description = ?2, rule_type = ?3, scope = ?4, scope_ref = ?5,
                       match_type = ?6, pattern = ?7, action = ?8, config = ?9, priority = ?10,
                       enabled = ?11, is_builtin = ?12, updated_at = ?13
                     WHERE id = ?1",
                    rusqlite::params![
                        id, r.description, rule_type, scope, r.scope_ref,
                        match_type, r.pattern, action, r.config, r.priority,
                        r.enabled as i64, r.is_builtin as i64, now,
                    ],
                )?;
            } else {
                conn.execute(
                    "INSERT INTO middleware_rule
                       (name, description, rule_type, scope, scope_ref, match_type, pattern, action, config, priority, enabled, is_builtin, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
                    rusqlite::params![
                        r.name, r.description, rule_type, scope, r.scope_ref,
                        match_type, r.pattern, action, r.config, r.priority,
                        r.enabled as i64, r.is_builtin as i64, now,
                    ],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert middleware rule: {e}"))
}

#[cfg(test)]
#[path = "test_db_rows.rs"]
mod test_db_rows;
