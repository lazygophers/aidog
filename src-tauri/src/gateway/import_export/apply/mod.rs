//! 导入应用器：解密 → 校验 → 冲突检测 → 按决策写入 db + 文件。
//!
//! 写入顺序（外键依赖）：codex/claude-code 文件 → group → platform →
//! group_platform → setting → skills。
//!
//! 子模块划分：
//! - [`conflicts`]：冲突扫描（group/setting/文件）。
//! - [`files`]：文件类（codex/claude-code）备份后写。
//! - [`db_rows`]：db 行级 upsert + auto-group。
//! - [`json_helpers`]：JSON 值提取助手。

use std::collections::BTreeMap;

use super::{ConflictDecision, Decision, ImportPreview, ImportReport, Payload};
use crate::gateway::db::Db;

mod conflicts;
mod db_rows;
mod files;
mod json_helpers;

// 对外 API 路径保持 `import_export::apply::X` 不变。
pub use db_rows::{ensure_group_and_attach, snapshot_platform_ids};

/// 解密文件 + 校验 + 扫描冲突，返回预览（供前端弹窗收集决策）。
pub async fn preview(file_bytes: &[u8], db: &Db) -> Result<ImportPreview, String> {
    let plain = super::container::decrypt(file_bytes)?;
    let payload = Payload::from_bytes_verified(&plain)?;

    let conflicts = conflicts::detect_conflicts(&payload, db).await?;
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
    files::apply_files(&payload, &dec, &mut report)?;

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

pub(super) fn should_skip(decision: Option<&Decision>) -> bool {
    matches!(decision, Some(Decision::Skip))
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
        if let Err(e) = db_rows::upsert_group_row(db, &group_key, &effective_name, g).await {
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
        if let Err(e) = db_rows::upsert_platform_row(db, &name, &effective_name, p).await {
            report.errors.push(format!("platform「{name}」: {e}"));
        } else {
            bump(&mut report.applied, crate::gateway::import_export::SCOPE_PLATFORM);
        }
    }

    // group_platform（按名称解析 → id）
    for [g_name, p_name] in &payload.group_platform {
        if let Err(e) = db_rows::relink_group_platform(db, g_name, p_name).await {
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
        if let Err(e) = db_rows::upsert_setting_row(db, scope, key, val).await {
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

pub(super) fn bump(map: &mut BTreeMap<String, usize>, scope: &str) {
    *map.entry(scope.to_string()).or_insert(0) += 1;
}
