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

    let existing_group_keys: std::collections::BTreeSet<String> =
        crate::gateway::db::list_groups(db)
            .await?
            .into_iter()
            .map(|g| g.group_key)
            .collect();
    for g in &payload.group {
        // group_key 作冲突键（fallback name 兼容老导出）；name 仅作显示。
        let gkey = g
            .get("group_key")
            .and_then(|v| v.as_str())
            .or_else(|| g.get("name").and_then(|v| v.as_str()));
        let gname = g
            .get("name")
            .and_then(|v| v.as_str())
            .or(gkey)
            .unwrap_or("");
        if let Some(k) = gkey {
            if existing_group_keys.contains(k) {
                out.push(ConflictItem {
                    scope: super::SCOPE_GROUP.to_string(),
                    key: k.to_string(),
                    existing_summary: format!("已存在同密钥分组「{gname}」"),
                    incoming_summary: format!("导入将覆盖分组「{gname}」配置"),
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
        // group_key 作唯一标识（fallback name 兼容老导出文件）；name 作显示名（冲突可重命名）。
        let group_key = g
            .get("group_key")
            .and_then(|v| v.as_str())
            .unwrap_or(&name)
            .to_string();
        let key = group_key.clone();
        let decision = dec
            .get(&(super::SCOPE_GROUP.to_string(), key.clone()))
            .copied();
        let (effective_name, skip) = resolve_name(&name, decision);
        if skip {
            bump(&mut report.skipped, super::SCOPE_GROUP);
            continue;
        }
        if let Err(e) = upsert_group_row(db, &group_key, &effective_name, g).await {
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

// ── db 行级 upsert（按 group_key 查重；name 作显示名可重命名；group_key 锁定不改） ──

async fn upsert_group_row(
    db: &Db,
    group_key: &str,
    effective_name: &str,
    row: &serde_json::Value,
) -> Result<(), String> {
    let row = row.clone();
    let group_key = group_key.to_string();
    let effective_name = effective_name.to_string();
    db.0
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

fn update_group_cols(
    tx: &rusqlite::Transaction,
    id: i64,
    row: &serde_json::Value,
    effective: &str,
) -> rusqlite::Result<()> {
    let now = now_ts();
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
        "UPDATE \"group\" SET name = ?1, routing_mode = ?2, auto_from_platform = ?3, sort_order = ?4, updated_at = ?5 WHERE id = ?6",
        rusqlite::params![effective, routing_mode, auto_from_platform, sort_order, now, id],
    )?;
    Ok(())
}

async fn upsert_platform_row(
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
    // breaker 阈值现存于 extra.breaker。新格式导出已含 extra.breaker；旧格式（breaker 在顶层）
    // 双兜底：若顶层有非 0 breaker 列且 extra 内尚无 breaker，则合并进 extra（无损迁入）。
    let extra = effective_extra_with_breaker(row);
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
            json_str(row, "platform_type"),
            json_str(row, "base_url"),
            json_str(row, "api_key"),
            extra,
            json_str(row, "models"),
            json_str(row, "available_models"),
            json_str(row, "endpoints"),
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
fn effective_extra_with_breaker(row: &serde_json::Value) -> String {
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

async fn relink_group_platform(db: &Db, group_key: &str, platform_name: &str) -> Result<(), String> {
    let g = group_key.to_string();
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
    db.0
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
    db.0
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
                         VALUES (?1, ?2, 'round_robin', '', 0, ?3, ?3)",
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
    ///
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

    /// 旧格式导入（breaker 在顶层）→ 无损迁入 extra.breaker。
    #[test]
    fn legacy_top_level_breaker_migrates_into_extra() {
        let mut row = platform_payload("Old", "https://a.example.com");
        row["breaker_failure_threshold"] = serde_json::json!(6);
        row["breaker_open_secs"] = serde_json::json!(180);
        row["breaker_half_open_max"] = serde_json::json!(3);
        let extra = effective_extra_with_breaker(&row);
        let b = crate::gateway::models::parse_breaker(&extra);
        assert_eq!((b.failure_threshold, b.open_secs, b.half_open_max), (6, 180, 3));
    }

    /// 直插一个 platform（绕过 apply 事务），返回 rowid。
    async fn insert_test_platform(db: &Db, name: &str) -> i64 {
        let name = name.to_string();
        db.0
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO platform (name, created_at, updated_at) VALUES (?1, 0, 0)",
                    rusqlite::params![name],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await
            .unwrap()
    }

    async fn group_id_by_name(db: &Db, name: &str) -> Option<i64> {
        let name = name.to_string();
        db.0
            .call(move |conn| {
                Ok(conn
                    .query_row(
                        "SELECT id FROM \"group\" WHERE name = ?1 AND deleted_at = 0",
                        [&name],
                        |r| r.get::<_, i64>(0),
                    )
                    .ok())
            })
            .await
            .unwrap()
    }

    async fn link_count(db: &Db, gid: i64) -> i64 {
        db.0
            .call(move |conn| {
                Ok(conn
                    .query_row(
                        "SELECT COUNT(*) FROM group_platform WHERE group_id = ?1 AND deleted_at = 0",
                        [gid],
                        |r| r.get::<_, i64>(0),
                    )
                    .unwrap_or(0))
            })
            .await
            .unwrap()
    }

    /// 组不存在 → 按 name 建组（生成 group_key）+ 关联本次新建 platform。
    #[tokio::test]
    async fn ensure_group_creates_when_absent() {
        let db = test_db().await;
        // 预置一个旧平台（before 快照含它，不应被关联）。
        insert_test_platform(&db, "old").await;
        let before = snapshot_platform_ids(&db).await.unwrap();
        // 本次"导入"新建两个平台。
        insert_test_platform(&db, "new1").await;
        insert_test_platform(&db, "new2").await;

        ensure_group_and_attach(&db, "sub2api", &before).await.unwrap();

        let gid = group_id_by_name(&db, "sub2api").await.expect("group created");
        // 校验 group_key 生成。
        let gkey: String = db
            .0
            .call(move |conn| {
                Ok(conn
                    .query_row("SELECT group_key FROM \"group\" WHERE id = ?1", [gid], |r| {
                        r.get::<_, String>(0)
                    })
                    .unwrap())
            })
            .await
            .unwrap();
        assert!(gkey.starts_with("gk_"), "group_key 应生成 gk_ 前缀");
        // 仅关联本次新建的 2 个平台（old 不在内）。
        assert_eq!(link_count(&db, gid).await, 2);
    }

    /// 同名组已存在 → 不重复建组，仅 attach（ON CONFLICT 幂等）。
    #[tokio::test]
    async fn ensure_group_idempotent() {
        let db = test_db().await;
        let before1 = snapshot_platform_ids(&db).await.unwrap();
        insert_test_platform(&db, "p1").await;
        ensure_group_and_attach(&db, "sub2api", &before1).await.unwrap();
        let gid = group_id_by_name(&db, "sub2api").await.unwrap();
        assert_eq!(link_count(&db, gid).await, 1);

        // 第二次导入：组已存在 → 复用同 id，不重复建组。
        let before2 = snapshot_platform_ids(&db).await.unwrap();
        insert_test_platform(&db, "p2").await;
        ensure_group_and_attach(&db, "sub2api", &before2).await.unwrap();
        let gid2 = group_id_by_name(&db, "sub2api").await.unwrap();
        assert_eq!(gid, gid2, "同名组不应重复创建");
        // 组数确认只有一个。
        let group_count: i64 = db
            .0
            .call(|conn| {
                Ok(conn
                    .query_row(
                        "SELECT COUNT(*) FROM \"group\" WHERE name = 'sub2api' AND deleted_at = 0",
                        [],
                        |r| r.get::<_, i64>(0),
                    )
                    .unwrap())
            })
            .await
            .unwrap();
        assert_eq!(group_count, 1);
        // 第二次关联追加 p2 → 共 2 个关联。
        assert_eq!(link_count(&db, gid).await, 2);
    }

    /// auto_group=false 等价于不调 ensure → 不建组（行为契约：import 跳过 ensure）。
    #[tokio::test]
    async fn no_ensure_means_no_group() {
        let db = test_db().await;
        insert_test_platform(&db, "p").await;
        // 不调用 ensure_group_and_attach（模拟 auto_group=false）。
        assert!(group_id_by_name(&db, "sub2api").await.is_none());
    }

    /// 新格式导入（breaker 已在 extra）→ 原样保留，不被顶层 0 覆盖。
    #[test]
    fn new_format_extra_breaker_preserved() {
        let mut row = platform_payload("New", "https://a.example.com");
        row["extra"] = serde_json::json!(crate::gateway::models::merge_breaker_into_extra(
            "{}",
            &crate::gateway::models::PlatformBreaker { failure_threshold: 9, open_secs: 30, half_open_max: 1 },
        ));
        // 顶层全 0（新导出不再含顶层 breaker）。
        let extra = effective_extra_with_breaker(&row);
        let b = crate::gateway::models::parse_breaker(&extra);
        assert_eq!((b.failure_threshold, b.open_secs, b.half_open_max), (9, 30, 1));
    }
}
