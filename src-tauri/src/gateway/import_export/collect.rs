//! 导出收集器：从 db + 文件系统读取各 scope 数据，组装 [`Payload`]。

use std::collections::BTreeSet;

use super::{Manifest, NamedText, Payload};
use crate::gateway::{codex, db::Db};

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
            .map(serde_json::to_value)
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

fn read_text_optional(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn hostname_or_unknown() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
