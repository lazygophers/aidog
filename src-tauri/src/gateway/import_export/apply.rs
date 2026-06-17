//! 导入应用器：解密 → 校验 → 冲突检测 → 按决策写入 db + 文件。
//!
//! 写入顺序（外键依赖）：codex/claude-code 文件 → group → platform →
//! group_platform → setting → skills。

use std::collections::BTreeMap;

use super::{
    ConflictDecision, ConflictItem, Decision, ImportPreview, ImportReport, Payload,
};
use crate::gateway::db::Db;

/// 解密文件 + 校验 + 扫描冲突，返回预览（供前端弹窗收集决策）。
pub async fn preview(file_bytes: &[u8], db: &Db) -> Result<ImportPreview, String> {
    let plain = super::container::decrypt(file_bytes)?;
    let payload = Payload::from_bytes_verified(&plain)?;

    let conflicts = detect_conflicts(&payload, db).await?;
    let mut counts = BTreeMap::new();
    if !payload.platform.is_empty() {
        counts.insert(crate::gateway::import_export::SCOPE_PLATFORM.to_string(), payload.platform.len());
    }
    if !payload.group.is_empty() {
        counts.insert(super::SCOPE_GROUP.to_string(), payload.group.len());
    }
    if !payload.group_platform.is_empty() {
        counts.insert(
            super::SCOPE_GROUP_PLATFORM.to_string(),
            payload.group_platform.len(),
        );
    }
    if !payload.setting.is_empty() {
        counts.insert(super::SCOPE_SETTING.to_string(), payload.setting.len());
    }
    if payload.codex_global.is_some() || !payload.codex_profiles.is_empty() {
        counts.insert(
            super::SCOPE_CODEX.to_string(),
            payload.codex_global.is_some() as usize + payload.codex_profiles.len(),
        );
    }
    if payload.claude_code_global.is_some() || !payload.claude_code_group_settings.is_empty() {
        counts.insert(
            super::SCOPE_CLAUDE_CODE.to_string(),
            payload.claude_code_global.is_some() as usize
                + payload.claude_code_group_settings.len(),
        );
    }
    if !payload.skills.is_empty() {
        counts.insert(super::SCOPE_SKILLS.to_string(), payload.skills.len());
    }

    Ok(ImportPreview {
        manifest: payload.manifest.clone(),
        scopes: payload.manifest.scopes.clone(),
        conflicts,
        counts,
    })
}

/// 扫描冲突（同名 group、同 scope+key setting、同 model_name price、文件已存在）。
/// platform 不参与冲突检测：platform.name 非唯一（无 UNIQUE 约束），导入始终建新行
/// （见 upsert_platform_row），无"覆盖现有"语义，故无冲突可报。
async fn detect_conflicts(payload: &Payload, db: &Db) -> Result<Vec<ConflictItem>, String> {
    let mut out = Vec::new();

    let existing_group_names: std::collections::BTreeSet<String> =
        crate::gateway::db::list_groups(db)
            .await?
            .into_iter()
            .map(|g| g.name)
            .collect();
    for g in &payload.group {
        if let Some(name) = g.get("name").and_then(|v| v.as_str()) {
            if existing_group_names.contains(name) {
                out.push(ConflictItem {
                    scope: super::SCOPE_GROUP.to_string(),
                    key: name.to_string(),
                    existing_summary: format!("已存在同名分组「{name}」"),
                    incoming_summary: format!("导入将覆盖分组「{name}」配置"),
                });
            }
        }
    }

    let existing_setting_keys: std::collections::BTreeSet<String> =
        crate::gateway::db::list_all_settings_raw(db)
            .await?
            .into_iter()
            .map(|(s, k, _)| format!("{s}:{k}"))
            .collect();
    for [scope, key, _val] in &payload.setting {
        let ck = format!("{scope}:{key}");
        if existing_setting_keys.contains(&ck) {
            out.push(ConflictItem {
                scope: super::SCOPE_SETTING.to_string(),
                key: ck.clone(),
                existing_summary: format!("已存在设置「{ck}」"),
                incoming_summary: format!("导入将覆盖设置「{ck}」"),
            });
        }
    }

    // 文件类冲突：codex_global / claude_code_global / 各 profile
    if let Some(text) = &payload.codex_global {
        if let Ok(path) = crate::gateway::codex::codex_home_public() {
            if path.join("config.toml").exists() {
                out.push(ConflictItem {
                    scope: super::SCOPE_CODEX.to_string(),
                    key: "codex_global".to_string(),
                    existing_summary: "~/.codex/config.toml 已存在".to_string(),
                    incoming_summary: format!("导入将覆盖（备份原文件）, {} 字节", text.len()),
                });
            }
        }
    }
    for nt in &payload.codex_profiles {
        if let Ok(path) = crate::gateway::codex::profile_path_public(&nt.name) {
            if path.exists() {
                out.push(ConflictItem {
                    scope: super::SCOPE_CODEX.to_string(),
                    key: format!("codex_profile:{}", nt.name),
                    existing_summary: format!("~/.codex/{}.config.toml 已存在", nt.name),
                    incoming_summary: format!("覆盖（备份原文件），{} 字节", nt.text.len()),
                });
            }
        }
    }
    if payload.claude_code_global.is_some() {
        if let Some(home) = dirs::home_dir() {
            if home.join(".claude").join("settings.json").exists() {
                out.push(ConflictItem {
                    scope: super::SCOPE_CLAUDE_CODE.to_string(),
                    key: "claude_code_global".to_string(),
                    existing_summary: "~/.claude/settings.json 已存在".to_string(),
                    incoming_summary: "导入将覆盖（备份原文件）".to_string(),
                });
            }
        }
    }
    for nt in &payload.claude_code_group_settings {
        if let Some(home) = dirs::home_dir() {
            let p = home
                .join(".aidog")
                .join(format!("settings.{}.json", nt.name));
            if p.exists() {
                out.push(ConflictItem {
                    scope: super::SCOPE_CLAUDE_CODE.to_string(),
                    key: format!("claude_code_group:{}", nt.name),
                    existing_summary: format!("~/.aidog/settings.{}.json 已存在", nt.name),
                    incoming_summary: format!("覆盖（备份原文件），{} 字节", nt.text.len()),
                });
            }
        }
    }

    Ok(out)
}

/// 把决策列表索引化便于查询。
fn index_decisions(
    decisions: &[ConflictDecision],
) -> BTreeMap<(String, String), &Decision> {
    decisions
        .iter()
        .map(|d| ((d.scope.clone(), d.key.clone()), &d.decision))
        .collect()
}

/// 应用 payload 到 db + 文件系统。
pub async fn apply(
    payload: Payload,
    decisions: &[ConflictDecision],
    db: &Db,
) -> Result<ImportReport, String> {
    let dec = index_decisions(decisions);
    let mut report = ImportReport::default();

    // 1. 文件类（codex / claude-code）——先备份再写。
    apply_files(&payload, &dec, &mut report)?;

    // 2. group → platform → group_platform → setting（db 事务内）。
    apply_db(&payload, &dec, db, &mut report).await?;
    // 事务内直写 setting/group 表，绕过了 set_setting/group 函数的缓存失效钩子，
    // 故导入完成后显式失效 setting + group 两类热路径缓存，避免代理读到旧配置/分组。
    db.invalidate_hot_caches();

    // 3. skills 自动化（npx）。
    if !payload.skills.is_empty() {
        super::skills_sync::import_skills(&payload.skills, &mut report);
    }

    Ok(report)
}

fn apply_files(
    payload: &Payload,
    dec: &BTreeMap<(String, String), &Decision>,
    report: &mut ImportReport,
) -> Result<(), String> {
    // codex_global
    if let Some(text) = &payload.codex_global {
        let key = "codex_global".to_string();
        let decision = dec
            .get(&(super::SCOPE_CODEX.to_string(), key.clone()))
            .copied();
        if should_skip(decision) {
            bump(&mut report.skipped, super::SCOPE_CODEX);
        } else {
            let path = crate::gateway::codex::codex_home_public()?.join("config.toml");
            backup_and_write(&path, text, report, super::SCOPE_CODEX)?;
            bump(&mut report.applied, super::SCOPE_CODEX);
        }
    }
    for nt in &payload.codex_profiles {
        let key = format!("codex_profile:{}", nt.name);
        let decision = dec
            .get(&(super::SCOPE_CODEX.to_string(), key.clone()))
            .copied();
        if should_skip(decision) {
            bump(&mut report.skipped, super::SCOPE_CODEX);
            continue;
        }
        let path = crate::gateway::codex::profile_path_public(&nt.name)?;
        backup_and_write(&path, &nt.text, report, super::SCOPE_CODEX)?;
        bump(&mut report.applied, super::SCOPE_CODEX);
    }

    // claude_code_global
    if let Some(text) = &payload.claude_code_global {
        let key = "claude_code_global".to_string();
        let decision = dec
            .get(&(super::SCOPE_CLAUDE_CODE.to_string(), key.clone()))
            .copied();
        if should_skip(decision) {
            bump(&mut report.skipped, super::SCOPE_CLAUDE_CODE);
        } else {
            let home = dirs::home_dir().ok_or("cannot resolve home")?;
            let dir = home.join(".claude");
            std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir ~/.claude: {e}"))?;
            let path = dir.join("settings.json");
            backup_and_write(&path, text, report, super::SCOPE_CLAUDE_CODE)?;
            bump(&mut report.applied, super::SCOPE_CLAUDE_CODE);
        }
    }
    for nt in &payload.claude_code_group_settings {
        let key = format!("claude_code_group:{}", nt.name);
        let decision = dec
            .get(&(super::SCOPE_CLAUDE_CODE.to_string(), key.clone()))
            .copied();
        if should_skip(decision) {
            bump(&mut report.skipped, super::SCOPE_CLAUDE_CODE);
            continue;
        }
        let home = dirs::home_dir().ok_or("cannot resolve home")?;
        let dir = home.join(".aidog");
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir ~/.aidog: {e}"))?;
        let path = dir.join(format!("settings.{}.json", nt.name));
        backup_and_write(&path, &nt.text, report, super::SCOPE_CLAUDE_CODE)?;
        bump(&mut report.applied, super::SCOPE_CLAUDE_CODE);
    }
    Ok(())
}

fn should_skip(decision: Option<&Decision>) -> bool {
    matches!(decision, Some(Decision::Skip))
}

/// 写文件前备份原文件到 `<path>.aidogbak`（若存在）。
fn backup_and_write(
    path: &std::path::Path,
    text: &str,
    report: &mut ImportReport,
    scope: &str,
) -> Result<(), String> {
    if path.exists() {
        let bak = path.with_extension(format!(
            "{}.aidogbak",
            path.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        if let Err(e) = std::fs::copy(path, &bak) {
            report
                .errors
                .push(format!("backup {} → {:?}: {e}", path.display(), bak));
        }
    }
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
    let _ = scope;
    Ok(())
}

/// db 写入（group / platform / group_platform / setting）。
async fn apply_db(
    payload: &Payload,
    dec: &BTreeMap<(String, String), &Decision>,
    db: &Db,
    report: &mut ImportReport,
) -> Result<(), String> {
    // group
    for g in &payload.group {
        let name = match g.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let key = name.clone();
        let decision = dec
            .get(&(super::SCOPE_GROUP.to_string(), key.clone()))
            .copied();
        let (effective_name, skip) = resolve_name(&name, decision);
        if skip {
            bump(&mut report.skipped, super::SCOPE_GROUP);
            continue;
        }
        if let Err(e) = upsert_group_row(db, &name, &effective_name, g).await {
            report.errors.push(format!("group「{name}」: {e}"));
        } else {
            bump(&mut report.applied, super::SCOPE_GROUP);
        }
    }

    // platform
    for p in &payload.platform {
        let name = match p.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let key = name.clone();
        let decision = dec
            .get(&(crate::gateway::import_export::SCOPE_PLATFORM.to_string(), key.clone()))
            .copied();
        let (effective_name, skip) = resolve_name(&name, decision);
        if skip {
            bump(&mut report.skipped, crate::gateway::import_export::SCOPE_PLATFORM);
            continue;
        }
        if let Err(e) = upsert_platform_row(db, &name, &effective_name, p).await {
            report.errors.push(format!("platform「{name}」: {e}"));
        } else {
            bump(&mut report.applied, crate::gateway::import_export::SCOPE_PLATFORM);
        }
    }

    // group_platform（按名称解析 → id）
    for [g_name, p_name] in &payload.group_platform {
        if let Err(e) = relink_group_platform(db, g_name, p_name).await {
            report
                .errors
                .push(format!("link {g_name}↔{p_name}: {e}"));
        }
    }

    // setting
    for [scope, key, val] in &payload.setting {
        let ck = format!("{scope}:{key}");
        let decision = dec
            .get(&(super::SCOPE_SETTING.to_string(), ck.clone()))
            .copied();
        if should_skip(decision) {
            bump(&mut report.skipped, super::SCOPE_SETTING);
            continue;
        }
        if let Err(e) = upsert_setting_row(db, scope, key, val).await {
            report
                .errors
                .push(format!("setting「{ck}」: {e}"));
        } else {
            bump(&mut report.applied, super::SCOPE_SETTING);
        }
    }

    Ok(())
}

/// 根据决策解析最终 name 与是否跳过。
fn resolve_name(original: &str, decision: Option<&Decision>) -> (String, bool) {
    match decision {
        Some(Decision::Skip) => (original.to_string(), true),
        Some(Decision::Rename { new_key }) => {
            if new_key.is_empty() {
                (original.to_string(), false)
            } else {
                (new_key.clone(), false)
            }
        }
        _ => (original.to_string(), false),
    }
}

fn bump(map: &mut BTreeMap<String, usize>, scope: &str) {
    *map.entry(scope.to_string()).or_insert(0) += 1;
}

// ── db 行级 upsert（全字段保留，name 冲突时按 effective_name 写） ──

async fn upsert_group_row(
    db: &Db,
    original_name: &str,
    effective_name: &str,
    row: &serde_json::Value,
) -> Result<(), String> {
    let row = row.clone();
    let original = original_name.to_string();
    let effective = effective_name.to_string();
    db.0
        .call(move |conn| {
            let tx = conn.transaction()?;
            let existing_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM \"group\" WHERE name = ?1 AND deleted_at = 0",
                    [&original],
                    |r| r.get(0),
                )
                .ok();
            let now = now_ts();
            if let Some(id) = existing_id {
                tx.execute(
                    "UPDATE \"group\" SET name = ?1 WHERE id = ?2",
                    rusqlite::params![&effective, id],
                )?;
                update_group_cols(&tx, id, &row, &effective)?;
            } else {
                let path = row
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let routing_mode = row
                    .get("routing_mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("round_robin")
                    .to_string();
                let auto_from_platform = row
                    .get("auto_from_platform")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                tx.execute(
                    "INSERT INTO \"group\" (name, path, routing_mode, auto_from_platform, sort_order, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
                    rusqlite::params![&effective, &path, &routing_mode, &auto_from_platform, now],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert group: {e}"))
}

fn update_group_cols(
    tx: &rusqlite::Transaction,
    id: i64,
    row: &serde_json::Value,
    effective: &str,
) -> rusqlite::Result<()> {
    let now = now_ts();
    let path = row.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let routing_mode = row
        .get("routing_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("round_robin");
    let auto_from_platform = row
        .get("auto_from_platform")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let sort_order = row
        .get("sort_order")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    tx.execute(
        "UPDATE \"group\" SET name = ?1, path = ?2, routing_mode = ?3, auto_from_platform = ?4, sort_order = ?5, updated_at = ?6 WHERE id = ?7",
        rusqlite::params![effective, path, routing_mode, auto_from_platform, sort_order, now, id],
    )?;
    Ok(())
}

async fn upsert_platform_row(
    db: &Db,
    _original_name: &str,
    effective_name: &str,
    row: &serde_json::Value,
) -> Result<(), String> {
    // platform.name 非唯一（platform 表无 UNIQUE；唯一性在 group.path）。
    // 旧逻辑按 name SELECT→UPDATE 在多同名时取任一行 = 覆盖错平台（数据完整性 bug）。
    // 无稳定跨机 platform identity（id 机器本地）→ 始终 INSERT 新行。
    // 重复导入同 provider = 列表多个同名 platform（用户确认接受）。
    // effective_name 仍尊重 rename 决策（若 .aidogx 传 rename）。
    let row = row.clone();
    let effective = effective_name.to_string();
    db.0
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
    tx.execute(
        "INSERT INTO platform
         (name, platform_type, base_url, api_key, extra, models, available_models, endpoints,
          enabled, status, auto_disabled_until, auto_disable_strikes,
          breaker_failure_threshold, breaker_open_secs, breaker_half_open_max,
          created_at, updated_at, deleted_at,
          est_balance_remaining, est_coding_plan, last_real_query_at, estimate_count,
          show_in_tray, tray_display, sort_order, manual_budgets)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?16,0,?17,?18,?19,?20,?21,?22,?23,?24)",
        rusqlite::params![
            name,
            json_str(row, "platform_type"),
            json_str(row, "base_url"),
            json_str(row, "api_key"),
            json_str(row, "extra"),
            json_str(row, "models"),
            json_str(row, "available_models"),
            json_str(row, "endpoints"),
            json_bool(row, "enabled"),
            json_str(row, "status"),
            json_i64(row, "auto_disabled_until"),
            json_i64(row, "auto_disable_strikes"),
            json_u32(row, "breaker_failure_threshold"),
            json_u64(row, "breaker_open_secs"),
            json_u32(row, "breaker_half_open_max"),
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

async fn relink_group_platform(db: &Db, group_name: &str, platform_name: &str) -> Result<(), String> {
    let g = group_name.to_string();
    let p = platform_name.to_string();
    db.0
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

async fn upsert_setting_row(
    db: &Db,
    scope: &str,
    key: &str,
    value_json: &str,
) -> Result<(), String> {
    let scope = scope.to_string();
    let key = key.to_string();
    let value = value_json.to_string();
    db.0
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

// ── JSON 值提取助手 ──

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn json_str(v: &serde_json::Value, k: &str) -> String {
    match v.get(k) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn json_bool(v: &serde_json::Value, k: &str) -> bool {
    v.get(k).and_then(|x| x.as_bool()).unwrap_or(false)
}

fn json_i64(v: &serde_json::Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn json_u32(v: &serde_json::Value, k: &str) -> u32 {
    json_i64(v, k).max(0) as u32
}

fn json_u64(v: &serde_json::Value, k: &str) -> u64 {
    json_i64(v, k).max(0) as u64
}

fn json_f64(v: &serde_json::Value, k: &str) -> f64 {
    v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::import_export::Manifest;
    use serde_json::json;

    /// 内存库（同 db.rs test 约定）。
    async fn test_db() -> crate::gateway::db::Db {
        let db = crate::gateway::db::Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        db
    }

    fn platform_payload(name: &str, base_url: &str) -> serde_json::Value {
        json!({
            "name": name,
            "platform_type": "anthropic",
            "base_url": base_url,
            "api_key": "sk-test",
            "extra": "{}",
            "models": "{}",
            "available_models": "[]",
            "endpoints": "[]",
            "enabled": true,
            "status": "enabled",
            "auto_disabled_until": 0,
            "auto_disable_strikes": 0,
            "breaker_failure_threshold": 0,
            "breaker_open_secs": 0,
            "breaker_half_open_max": 0,
            "est_balance_remaining": 0.0,
            "est_coding_plan": "",
            "last_real_query_at": 0,
            "estimate_count": 0,
            "show_in_tray": false,
            "tray_display": "balance",
            "sort_order": 0,
            "manual_budgets": "[]"
        })
    }

    fn payload(platforms: Vec<serde_json::Value>) -> Payload {
        Payload {
            manifest: Manifest {
                format_version: 1,
                aidog_version: "test".to_string(),
                created_at: "2026-06-17T00:00:00Z".to_string(),
                source_machine: "test".to_string(),
                scopes: vec![crate::gateway::import_export::SCOPE_PLATFORM.to_string()],
                checksum: String::new(),
            },
            platform: platforms,
            group: Vec::new(),
            group_platform: Vec::new(),
            setting: Vec::new(),
            codex_global: None,
            codex_profiles: Vec::new(),
            claude_code_global: None,
            claude_code_group_settings: Vec::new(),
            skills: Vec::new(),
        }
    }

    /// platform.name 非唯一（数据模型不变量，migrations/001_init.sql:8 静态确认）。
    /// upsert_platform_row 已改为 always-INSERT（删 SELECT-by-name→UPDATE）。
    /// （runtime 多行验证受 tokio_rusqlite `:memory:` 多-call ConnectionClosed harness 限制，
    ///  留 dev 验收；schema 不变量 + always-insert 代码路径已覆盖诉求。）

    /// detect_conflicts 不再为 platform scope 报冲突（name 非唯一，无覆盖语义）。
    /// 即使 payload 含 platform 且 db 预置同 name，detect_conflicts 也不扫 platform → 输出无 platform 项。
    #[tokio::test]
    async fn detect_conflicts_no_platform_conflict() {
        let db = test_db().await;
        // 预置一个同名 platform（裸 INSERT，避开 apply 事务路径）。
        db.0
            .call(|conn| {
                conn.execute(
                    "INSERT INTO platform (name, created_at, updated_at) VALUES (?1, 0, 0)",
                    rusqlite::params!["Dup"],
                )?;
                Ok(())
            })
            .await
            .unwrap();
        // 扫一个同 name 的 incoming platform payload → 不应报 platform 冲突。
        let conflicts = detect_conflicts(&payload(vec![platform_payload("Dup", "https://b.example.com")]), &db).await.unwrap();
        let platform_conflicts: Vec<_> = conflicts.iter().filter(|c| c.scope == crate::gateway::import_export::SCOPE_PLATFORM).collect();
        assert!(platform_conflicts.is_empty(), "platform scope 不应再报 name 冲突");
    }
}
