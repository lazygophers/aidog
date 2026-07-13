//! 导出收集器：从 db + 文件系统读取各 scope 数据，组装 [`Payload`]。

use std::collections::BTreeSet;

use super::{Manifest, NamedText, Payload};
use crate::gateway::{
    codex,
    db::Db,
    models::{Platform, PlatformModels, PlatformEndpoint, Protocol},
};
use serde::Serialize;

/// 收集错误（非致命项收集为 payload 缺省值，仅致命错误返回 Err）。
pub async fn collect(db: &Db, scopes: &[String]) -> Result<Payload, String> {
    let scope_set: BTreeSet<&str> = scopes.iter().map(|s| s.as_str()).collect();

    let mut payload = Payload {
        manifest: Manifest {
            format_version: 1,
            aidog_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            source_machine: hostname_or_unknown(),
            scopes: scopes.to_vec(),
            checksum: String::new(),
        },
        platform: Vec::new(),
        group: Vec::new(),
        group_platform: Vec::new(),
        setting: Vec::new(),
        codex_global: None,
        codex_profiles: Vec::new(),
        claude_code_global: None,
        claude_code_group_settings: Vec::new(),
        skills: Vec::new(),
        mcp: Vec::new(),
        middleware: Vec::new(),
        model_price: Vec::new(),
    };

    if scope_set.contains(super::SCOPE_PLATFORM) {
        let platforms = crate::gateway::db::list_platforms(db).await?;
        payload.platform = platforms
            .into_iter()
            .map(|p| serde_json::to_value(to_export(p)))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("serialize platform: {e}"))?;
    }

    if scope_set.contains(super::SCOPE_GROUP) {
        let groups = crate::gateway::db::list_groups(db).await?;
        payload.group = groups
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()
            .map_err(|e| format!("serialize group: {e}"))?;
    }

    if scope_set.contains(super::SCOPE_GROUP_PLATFORM) {
        let pairs = crate::gateway::db::list_all_group_platform_pairs(db).await?;
        payload.group_platform = pairs.into_iter().map(|(g, p)| [g, p]).collect();
    }

    if scope_set.contains(super::SCOPE_SETTING) {
        let rows = crate::gateway::db::list_all_settings_raw(db).await?;
        payload.setting = rows.into_iter().map(|(s, k, v)| [s, k, v]).collect();
    }

    if scope_set.contains(super::SCOPE_CODEX) {
        collect_codex(&mut payload)?;
    }

    if scope_set.contains(super::SCOPE_CLAUDE_CODE) {
        collect_claude_code(&mut payload)?;
    }

    if scope_set.contains(super::SCOPE_SKILLS) {
        payload.skills = super::skills_sync::export_skills();
    }

    if scope_set.contains(super::SCOPE_MCP) {
        let rows = crate::gateway::db::list_mcp_servers(db).await?;
        payload.mcp = rows
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()
            .map_err(|e| format!("serialize mcp: {e}"))?;
    }

    if scope_set.contains(super::SCOPE_MIDDLEWARE) {
        let rows = crate::gateway::db::list_middleware_rules(db).await?;
        payload.middleware = rows
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()
            .map_err(|e| format!("serialize middleware: {e}"))?;
    }

    if scope_set.contains(super::SCOPE_MODEL_PRICE) {
        let rows = crate::gateway::db::list_all_model_prices(db).await?;
        payload.model_price = rows
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()
            .map_err(|e| format!("serialize model_price: {e}"))?;
    }

    Ok(payload)
}

fn collect_codex(payload: &mut Payload) -> Result<(), String> {
    let home = codex::codex_home_public()?;
    let global = home.join("config.toml");
    payload.codex_global = read_text_optional(&global);

    // 各 group profile = `<group>.config.toml`。从已收集的 group 名遍历。
    let group_keys: Vec<String> = payload
        .group
        .iter()
        .filter_map(|g| g.get("group_key").and_then(|v| v.as_str()).or_else(|| g.get("name").and_then(|v| v.as_str())).map(String::from))
        .collect();
    for name in &group_keys {
        let path = codex::profile_path_public(name)?;
        if let Some(text) = read_text_optional(&path) {
            payload.codex_profiles.push(NamedText {
                name: name.clone(),
                text,
            });
        }
    }
    Ok(())
}

fn collect_claude_code(payload: &mut Payload) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    // 全局 ~/.claude/settings.json
    let global = home.join(".claude").join("settings.json");
    payload.claude_code_global = read_text_optional(&global);

    // 各 group ~/.aidog/settings.{group}.json
    let aidog_dir = home.join(".aidog");
    let group_keys: Vec<String> = payload
        .group
        .iter()
        .filter_map(|g| g.get("group_key").and_then(|v| v.as_str()).or_else(|| g.get("name").and_then(|v| v.as_str())).map(String::from))
        .collect();
    for name in &group_keys {
        let path = aidog_dir.join(format!("settings.{name}.json"));
        if let Some(text) = read_text_optional(&path) {
            payload
                .claude_code_group_settings
                .push(NamedText { name: name.clone(), text });
        }
    }
    Ok(())
}

// ── platform 导出清洗（分享场景） ─────────────────────────────
//
// 三层清洗（PRD 07-01-export-extra-cleanup）：
// 1. extra：空 (`{}`/`""`) → 省略；非空 string → parse 为 JSON object 序列化（非裸 string）
// 2. 配置空值省略：models / available_models / endpoints 空时不进 payload
// 3. 运行时 + 状态不导出：auto_disabled_until / auto_disable_strikes / expires_at / deleted_at /
//    est_balance_remaining / est_coding_plan / last_real_query_at / estimate_count / status / enabled /
//    last_error / last_error_at / balance_level / show_in_tray / tray_display / sort_order / manual_budgets
//    分享 = 给别人，不带原用户使用数据 / 启用意图。导入缺失字段走 `#[serde(default)]` / json_* 回默认。
//
// 用 collect 阶段 transform（不改 Platform Rust 字段类型，不改 DB schema）。
/// 导出中间结构：白名单字段 + 清洗规则。仅 `to_export` 构造，`collect` 序列化。
#[derive(Serialize)]
struct ExportPlatform {
    id: u64,
    name: String,
    platform_type: Protocol,
    base_url: String,
    api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "PlatformModels::is_empty")]
    models: PlatformModels,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    available_models: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    endpoints: Vec<PlatformEndpoint>,
    created_at: i64,
    updated_at: i64,
}

/// `Platform` → `ExportPlatform`：extra 空 → None；非空 → parse 为 obj（parse 失败兜底 None）。
fn to_export(p: Platform) -> ExportPlatform {
    let extra = parse_export_extra(&p.extra);
    ExportPlatform {
        id: p.id,
        name: p.name,
        platform_type: p.platform_type,
        base_url: p.base_url,
        api_key: p.api_key,
        extra,
        models: p.models,
        available_models: p.available_models,
        endpoints: p.endpoints,
        created_at: p.created_at,
        updated_at: p.updated_at,
    }
}

/// 导出 extra 清洗：空 / `{}` / 非法 JSON → None（省略）；合法 obj / 其它 JSON → Some(Value)。
/// 注：分享场景下 extra 习惯放 JSON object（breaker / mock / newapi）；非法 string 兜底省略（design 决策）。
/// UI 态 `_ui_*` 前缀键在导出前剥离（仿 `_aidog_statusline` strip 模式，sync_settings.rs），
/// 防分享/快照携带本机 UI 状态；strip 后空 obj 一并省略（与「空 extra 省略」对称）。
fn parse_export_extra(extra: &str) -> Option<serde_json::Value> {
    let trimmed = extra.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return None;
    }
    let mut value = serde_json::from_str::<serde_json::Value>(trimmed).ok()?;
    strip_ui_keys(&mut value);
    if value.as_object().is_some_and(|o| o.is_empty()) {
        return None;
    }
    Some(value)
}

/// 原地删除 JSON object 中所有 `_ui_*` 前缀键（前端 UI 态：_ui_collapsed / _ui_expand_plat / _ui_expand_grp）。
/// 非 object 值不动（extra 历史上可能塞裸 string，design 兜底不动）。
fn strip_ui_keys(value: &mut serde_json::Value) {
    if let Some(obj) = value.as_object_mut() {
        obj.retain(|k, _| !k.starts_with("_ui_"));
    }
}

fn read_text_optional(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn hostname_or_unknown() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
