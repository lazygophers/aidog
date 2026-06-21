//! 导入冲突扫描：同名 group、同 scope+key setting、文件已存在。

use super::super::{ConflictItem, Payload};
use crate::gateway::db::Db;

/// 扫描冲突（同名 group、同 scope+key setting、同 model_name price、文件已存在）。
/// platform 不参与冲突检测：platform.name 非唯一（无 UNIQUE 约束），导入始终建新行
/// （见 upsert_platform_row），无"覆盖现有"语义，故无冲突可报。
pub(super) async fn detect_conflicts(payload: &Payload, db: &Db) -> Result<Vec<ConflictItem>, String> {
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
                    scope: super::super::SCOPE_GROUP.to_string(),
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
                scope: super::super::SCOPE_SETTING.to_string(),
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
                    scope: super::super::SCOPE_CODEX.to_string(),
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
                    scope: super::super::SCOPE_CODEX.to_string(),
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
                    scope: super::super::SCOPE_CLAUDE_CODE.to_string(),
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
                    scope: super::super::SCOPE_CLAUDE_CODE.to_string(),
                    key: format!("claude_code_group:{}", nt.name),
                    existing_summary: format!("~/.aidog/settings.{}.json 已存在", nt.name),
                    incoming_summary: format!("覆盖（备份原文件），{} 字节", nt.text.len()),
                });
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
#[path = "test_conflicts.rs"]
mod test_conflicts;
