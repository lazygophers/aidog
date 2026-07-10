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

use super::{ConflictDecision, Decision, ImportItem, ImportPreview, ImportReport, Payload, Selection};
use crate::gateway::db::Db;

mod conflicts;
mod db_rows;
mod files;
mod json_helpers;

#[cfg(test)]
#[path = "test_apply.rs"]
mod test_apply;

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
    if !payload.mcp.is_empty() {
        counts.insert(super::SCOPE_MCP.to_string(), payload.mcp.len());
    }
    if !payload.middleware.is_empty() {
        counts.insert(super::SCOPE_MIDDLEWARE.to_string(), payload.middleware.len());
    }
    if !payload.model_price.is_empty() {
        counts.insert(super::SCOPE_MODEL_PRICE.to_string(), payload.model_price.len());
    }

    let items = build_items(&payload, &conflicts);

    Ok(ImportPreview {
        manifest: payload.manifest.clone(),
        scopes: payload.manifest.scopes.clone(),
        conflicts,
        counts,
        items,
    })
}

/// 枚举全部可导入条目（前端逐项勾选）。条目 key 与 [`apply`] 迭代时构造的键严格一致，
/// 否则白名单过滤会漏选 / 错选。
fn build_items(
    payload: &Payload,
    conflicts: &[super::ConflictItem],
) -> Vec<ImportItem> {
    use super::*;
    let conflict_set: std::collections::BTreeSet<(&str, &str)> = conflicts
        .iter()
        .map(|c| (c.scope.as_str(), c.key.as_str()))
        .collect();
    let is_conflict = |scope: &str, key: &str| conflict_set.contains(&(scope, key));
    let mut out = Vec::new();

    // platform：name 非唯一 → 用数组下标 idx:N 作稳定 key；label = name。platform 无冲突语义。
    for (i, p) in payload.platform.iter().enumerate() {
        let name = p
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("(unnamed)")
            .to_string();
        let key = format!("idx:{i}");
        out.push(ImportItem {
            scope: SCOPE_PLATFORM.to_string(),
            key,
            label: name,
            conflict: false,
        });
    }

    // group：key = group_key（fallback name）；label = name。
    for g in &payload.group {
        let name = g.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let gkey = g
            .get("group_key")
            .and_then(|v| v.as_str())
            .unwrap_or(name)
            .to_string();
        out.push(ImportItem {
            conflict: is_conflict(SCOPE_GROUP, &gkey),
            scope: SCOPE_GROUP.to_string(),
            key: gkey,
            label: name.to_string(),
        });
    }

    // group_platform：key = `<g>::<p>`。
    for [g, p] in &payload.group_platform {
        out.push(ImportItem {
            scope: SCOPE_GROUP_PLATFORM.to_string(),
            key: format!("{g}::{p}"),
            label: format!("{g} ↔ {p}"),
            conflict: false,
        });
    }

    // setting：key = `<scope>:<key>`。
    for [scope, key, _] in &payload.setting {
        let k = format!("{scope}:{key}");
        out.push(ImportItem {
            conflict: is_conflict(SCOPE_SETTING, &k),
            scope: SCOPE_SETTING.to_string(),
            label: k.clone(),
            key: k,
        });
    }

    // codex 文件。
    if payload.codex_global.is_some() {
        let k = "codex_global".to_string();
        out.push(ImportItem {
            conflict: is_conflict(SCOPE_CODEX, &k),
            scope: SCOPE_CODEX.to_string(),
            label: "~/.codex/config.toml".to_string(),
            key: k,
        });
    }
    for nt in &payload.codex_profiles {
        let k = format!("codex_profile:{}", nt.name);
        out.push(ImportItem {
            conflict: is_conflict(SCOPE_CODEX, &k),
            scope: SCOPE_CODEX.to_string(),
            label: format!("{}.config.toml", nt.name),
            key: k,
        });
    }

    // claude_code 文件。
    if payload.claude_code_global.is_some() {
        let k = "claude_code_global".to_string();
        out.push(ImportItem {
            conflict: is_conflict(SCOPE_CLAUDE_CODE, &k),
            scope: SCOPE_CLAUDE_CODE.to_string(),
            label: "~/.claude/settings.json".to_string(),
            key: k,
        });
    }
    for nt in &payload.claude_code_group_settings {
        let k = format!("claude_code_group:{}", nt.name);
        out.push(ImportItem {
            conflict: is_conflict(SCOPE_CLAUDE_CODE, &k),
            scope: SCOPE_CLAUDE_CODE.to_string(),
            label: format!("settings.{}.json", nt.name),
            key: k,
        });
    }

    // skills：key = name。
    for s in &payload.skills {
        out.push(ImportItem {
            scope: SCOPE_SKILLS.to_string(),
            key: s.name.clone(),
            label: s.name.clone(),
            conflict: false,
        });
    }

    // mcp：name 唯一但用数组下标 idx:N 作稳定 key（与 apply_db 迭代一致）；label = name。
    for (i, m) in payload.mcp.iter().enumerate() {
        let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("(unnamed)");
        out.push(ImportItem {
            scope: SCOPE_MCP.to_string(),
            key: format!("idx:{i}"),
            label: name.to_string(),
            conflict: false,
        });
    }

    // middleware：key = idx:N；label = name。
    for (i, r) in payload.middleware.iter().enumerate() {
        let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("(unnamed)");
        out.push(ImportItem {
            scope: SCOPE_MIDDLEWARE.to_string(),
            key: format!("idx:{i}"),
            label: name.to_string(),
            conflict: false,
        });
    }

    // model_price：key = `model:<model_name>`（model_name 唯一）；label = model_name。
    for mp in &payload.model_price {
        let name = mp.get("model_name").and_then(|v| v.as_str()).unwrap_or("(unnamed)");
        out.push(ImportItem {
            scope: SCOPE_MODEL_PRICE.to_string(),
            key: format!("model:{name}"),
            label: name.to_string(),
            conflict: false,
        });
    }

    out
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

/// 导出逐项过滤：按 (scope, key) 白名单裁剪 payload 各字段。`None` = 全量导出（向后兼容）。
/// key 构造与 [`build_items`] 逐字一致，保证前端勾选项能命中。
pub fn filter_payload(payload: &mut Payload, selection: Option<&Selection>) {
    let sel = match selection {
        None => return,
        Some(s) => s,
    };
    let keep = |scope: &str, key: &str| sel.contains(&(scope.to_string(), key.to_string()));

    let mut platform = Vec::new();
    for (i, p) in std::mem::take(&mut payload.platform).into_iter().enumerate() {
        if keep(super::SCOPE_PLATFORM, &format!("idx:{i}")) {
            platform.push(p);
        }
    }
    payload.platform = platform;

    payload.group.retain(|g| {
        let name = g.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let gkey = g.get("group_key").and_then(|v| v.as_str()).unwrap_or(name);
        keep(super::SCOPE_GROUP, gkey)
    });

    payload
        .group_platform
        .retain(|[g, p]| keep(super::SCOPE_GROUP_PLATFORM, &format!("{g}::{p}")));

    payload
        .setting
        .retain(|[scope, key, _]| keep(super::SCOPE_SETTING, &format!("{scope}:{key}")));

    if payload.codex_global.is_some() && !keep(super::SCOPE_CODEX, "codex_global") {
        payload.codex_global = None;
    }
    payload
        .codex_profiles
        .retain(|nt| keep(super::SCOPE_CODEX, &format!("codex_profile:{}", nt.name)));

    if payload.claude_code_global.is_some()
        && !keep(super::SCOPE_CLAUDE_CODE, "claude_code_global")
    {
        payload.claude_code_global = None;
    }
    payload
        .claude_code_group_settings
        .retain(|nt| keep(super::SCOPE_CLAUDE_CODE, &format!("claude_code_group:{}", nt.name)));

    payload.skills.retain(|s| keep(super::SCOPE_SKILLS, &s.name));

    let mut mcp = Vec::new();
    for (i, m) in std::mem::take(&mut payload.mcp).into_iter().enumerate() {
        if keep(super::SCOPE_MCP, &format!("idx:{i}")) {
            mcp.push(m);
        }
    }
    payload.mcp = mcp;

    let mut middleware = Vec::new();
    for (i, r) in std::mem::take(&mut payload.middleware).into_iter().enumerate() {
        if keep(super::SCOPE_MIDDLEWARE, &format!("idx:{i}")) {
            middleware.push(r);
        }
    }
    payload.middleware = middleware;

    payload.model_price.retain(|mp| {
        let name = mp.get("model_name").and_then(|v| v.as_str()).unwrap_or("");
        keep(super::SCOPE_MODEL_PRICE, &format!("model:{name}"))
    });
}

/// 导出预览：collect 全量 → build_items（无冲突）→ 返回条目供前端逐项勾选。
pub fn export_items(payload: &Payload) -> Vec<ImportItem> {
    build_items(payload, &[])
}

/// 判断条目是否被选中导入。`selection == None` 时一律选中（不过滤，旧行为）。
pub(super) fn is_selected(selection: Option<&Selection>, scope: &str, key: &str) -> bool {
    match selection {
        None => true,
        Some(sel) => sel.contains(&(scope.to_string(), key.to_string())),
    }
}

/// 应用 payload 到 db + 文件系统。
///
/// `selection` = 用户勾选的条目白名单（(scope, key) 集合）；`None` = 导入全部
/// （ccswitch / sub2api 异源路径用，它们自建 payload 不走逐项勾选）。
pub async fn apply(
    payload: Payload,
    decisions: &[ConflictDecision],
    selection: Option<&Selection>,
    db: &Db,
) -> Result<ImportReport, String> {
    let dec = index_decisions(decisions);
    let mut report = ImportReport::default();

    // 1. 文件类（codex / claude-code）——先备份再写。
    files::apply_files(&payload, &dec, selection, &mut report)?;

    // 2. group → platform → group_platform → setting（db 事务内）。
    apply_db(&payload, &dec, selection, db, &mut report).await?;
    // 事务内直写 setting/group 表，绕过了 set_setting/group 函数的缓存失效钩子，
    // 故导入完成后显式失效 setting + group 两类热路径缓存，避免代理读到旧配置/分组。
    db.invalidate_hot_caches();

    // 3. skills 自动化（npx）——仅导入选中的。
    let skills_sel: Vec<_> = payload
        .skills
        .iter()
        .filter(|s| is_selected(selection, super::SCOPE_SKILLS, &s.name))
        .cloned()
        .collect();
    if !skills_sel.is_empty() {
        super::skills_sync::import_skills(&skills_sel, &mut report);
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
    selection: Option<&Selection>,
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
        if !is_selected(selection, super::SCOPE_GROUP, &key) {
            continue;
        }
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

    // platform（白名单按数组下标 idx:N，与 build_items 一致；name 非唯一不可作 key）
    for (i, p) in payload.platform.iter().enumerate() {
        let name = match p.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !is_selected(selection, crate::gateway::import_export::SCOPE_PLATFORM, &format!("idx:{i}")) {
            continue;
        }
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
        if !is_selected(selection, super::SCOPE_GROUP_PLATFORM, &format!("{g_name}::{p_name}")) {
            continue;
        }
        if let Err(e) = db_rows::relink_group_platform(db, g_name, p_name).await {
            report
                .errors
                .push(format!("link {g_name}↔{p_name}: {e}"));
        }
    }

    // setting
    for [scope, key, val] in &payload.setting {
        let ck = format!("{scope}:{key}");
        if !is_selected(selection, super::SCOPE_SETTING, &ck) {
            continue;
        }
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

    // mcp（key = idx:N，与 build_items 一致；upsert by name，UNIQUE 冲突覆盖）
    for (i, m) in payload.mcp.iter().enumerate() {
        if !is_selected(selection, super::SCOPE_MCP, &format!("idx:{i}")) {
            continue;
        }
        match serde_json::from_value::<crate::gateway::mcp::McpServerRow>(m.clone()) {
            Ok(row) => match crate::gateway::db::upsert_mcp_server(db, &row).await {
                Ok(()) => bump(&mut report.applied, super::SCOPE_MCP),
                Err(e) => report.errors.push(format!("mcp「{}」: {e}", row.name)),
            },
            Err(e) => report.errors.push(format!("mcp parse: {e}")),
        }
    }

    // middleware（key = idx:N；按 name upsert，name 非唯一约束 → 手动查重）
    for (i, r) in payload.middleware.iter().enumerate() {
        if !is_selected(selection, super::SCOPE_MIDDLEWARE, &format!("idx:{i}")) {
            continue;
        }
        match serde_json::from_value::<crate::gateway::models::MiddlewareRule>(r.clone()) {
            Ok(rule) => match db_rows::upsert_middleware_rule_by_name(db, &rule).await {
                Ok(()) => bump(&mut report.applied, super::SCOPE_MIDDLEWARE),
                Err(e) => report.errors.push(format!("middleware「{}」: {e}", rule.name)),
            },
            Err(e) => report.errors.push(format!("middleware parse: {e}")),
        }
    }

    // model_price（key = model:<model_name>；upsert by (model_name, source) UNIQUE 覆盖）
    for mp in &payload.model_price {
        let model_name = match mp.get("model_name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !is_selected(selection, super::SCOPE_MODEL_PRICE, &format!("model:{model_name}")) {
            continue;
        }
        match serde_json::from_value::<crate::gateway::models::ModelPrice>(mp.clone()) {
            Ok(p) => {
                let res = crate::gateway::db::upsert_model_price(
                    db,
                    &p.model_name,
                    &p.source,
                    &p.price_data,
                    p.max_input_tokens,
                    p.max_output_tokens,
                    p.context_window,
                )
                .await;
                match res {
                    Ok(()) => bump(&mut report.applied, super::SCOPE_MODEL_PRICE),
                    Err(e) => report.errors.push(format!("model_price「{}」: {e}", p.model_name)),
                }
            }
            Err(e) => report.errors.push(format!("model_price parse: {e}")),
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

#[cfg(test)]
#[path = "test_selection.rs"]
mod test_selection;
