use aidog_core::gateway::{self, db::{self, Db}};
use gateway::models::*;
use tauri::State;


// ─── AI 编程工具联动开关 ──────────────────────────
//
// 两开关存 DB（scope="global", key="coding_tools_settings"），变化时按 diff 触发
// 写外部文件（仅当值真变才写），失败记 warn 不中断其它字段。
// - apply_to_claude_plugin → ~/.claude/config.json 的 primaryApiKey="any"
// - skip_claude_onboarding → ~/.claude.json 的 hasCompletedOnboarding=true

// 默认值 = 两开关 OFF（UI 显示关）。功能与开关解耦：
// 启动初始化（ensure_default_coding_tools_settings）在 DB 无记录时写外部文件让功能开箱生效，
// 但不落 DB 记录、`coding_tools_settings_get` 返 false —— 开关代表用户显式控制，
// 未操作 = 关（显示关），但功能在用。用户 toggle 后才有 DB 记录，ensure 尊重不再默认写。
const CODING_TOOLS_DEFAULT_APPLY: bool = false;
const CODING_TOOLS_DEFAULT_SKIP: bool = false;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct CodingToolsSettings {
    #[serde(default = "coding_tools_default_apply")]
    apply_to_claude_plugin: bool,
    #[serde(default = "coding_tools_default_skip")]
    skip_claude_onboarding: bool,
}

pub(crate) fn coding_tools_default_apply() -> bool { CODING_TOOLS_DEFAULT_APPLY }
pub(crate) fn coding_tools_default_skip() -> bool { CODING_TOOLS_DEFAULT_SKIP }

impl Default for CodingToolsSettings {
    fn default() -> Self {
        Self {
            apply_to_claude_plugin: CODING_TOOLS_DEFAULT_APPLY,
            skip_claude_onboarding: CODING_TOOLS_DEFAULT_SKIP,
        }
    }
}

pub(crate) async fn load_coding_tools_settings(db: &Db) -> CodingToolsSettings {
    match db::get_setting(db, "global", "coding_tools_settings").await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => CodingToolsSettings::default(),
    }
}

/// 启动初始化：DB **无** coding_tools_settings 记录时（用户未操作过两开关）
/// 写外部文件（~/.claude/config.json primaryApiKey + ~/.claude.json hasCompletedOnboarding）
/// 让功能开箱生效，但**不落 DB 记录**，`coding_tools_settings_get` 仍返 false（开关显示关）。
///
/// 语义：功能（文件写入）与开关（用户显式控制）解耦。
/// - 全新库启动 → 文件写入 + DB 无记录 + UI 关。
/// - 用户 toggle 后才有 DB 记录，下次启动 ensure 看到记录，完全尊重不再默认写。
///
/// 幂等：claude_integration 内部对已存在字段做 diff 跳过（重复写无副作用）。
/// 失败仅 warn 不中断启动。
pub async fn ensure_default_coding_tools_settings(db: &Db) -> Result<(), String> {
    if db::get_setting(db, "global", "coding_tools_settings").await.ok().flatten().is_some() {
        // 用户已 toggle 过两开关，完全尊重 DB 值，不强制默认写。
        return Ok(());
    }

    // 无记录（用户未操作）→ 写两外部文件让功能开箱生效。
    // 独立 try，失败 warn 不中断另一个；不落 DB 记录。
    match gateway::claude_integration::write_plugin_primary_key() {
        Ok(_changed) => tracing::info!("ensure_default_coding_tools_settings: wrote ~/.claude/config.json primaryApiKey"),
        Err(e) => tracing::warn!(
            error = %e,
            "ensure_default_coding_tools_settings: write ~/.claude/config.json failed"
        ),
    }
    match gateway::claude_integration::set_has_completed_onboarding() {
        Ok(_changed) => tracing::info!("ensure_default_coding_tools_settings: wrote ~/.claude.json hasCompletedOnboarding"),
        Err(e) => tracing::warn!(
            error = %e,
            "ensure_default_coding_tools_settings: write ~/.claude.json failed"
        ),
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn coding_tools_settings_get(db: State<'_, Db>) -> Result<CodingToolsSettings, String> {
    tracing::debug!(command = "coding_tools_settings_get", "command invoked");
    let current = load_coding_tools_settings(&db).await;
    Ok(current)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn coding_tools_settings_set(
    apply_to_claude_plugin: Option<bool>,
    skip_claude_onboarding: Option<bool>,
    db: State<'_, Db>,
) -> Result<CodingToolsSettings, String> {
    tracing::debug!(command = "coding_tools_settings_set", "command invoked");
    let mut current = load_coding_tools_settings(&db).await;

    // 按字段 diff 触发副作用写文件；写失败立即返 Err（前端回滚 + 显示真因），
    // 不再静默 warn+返原值（旧实现致前端乐观翻转后被 setSettings(原值) 回滚 = 「开关点击无反应」）。
    if let Some(v) = apply_to_claude_plugin {
        if v != current.apply_to_claude_plugin {
            let res = if v {
                gateway::claude_integration::write_plugin_primary_key()
            } else {
                gateway::claude_integration::clear_plugin_primary_key()
            };
            match res {
                Ok(_changed) => current.apply_to_claude_plugin = v,
                Err(e) => {
                    tracing::warn!(
                        command = "coding_tools_settings_set",
                        field = "apply_to_claude_plugin",
                        error = %e,
                        "write ~/.claude/config.json failed; field not persisted"
                    );
                    return Err(format!("write ~/.claude/config.json: {e}"));
                }
            }
        }
    }

    if let Some(v) = skip_claude_onboarding {
        if v != current.skip_claude_onboarding {
            let res = if v {
                gateway::claude_integration::set_has_completed_onboarding()
            } else {
                gateway::claude_integration::clear_has_completed_onboarding()
            };
            match res {
                Ok(_changed) => current.skip_claude_onboarding = v,
                Err(e) => {
                    tracing::warn!(
                        command = "coding_tools_settings_set",
                        field = "skip_claude_onboarding",
                        error = %e,
                        "write ~/.claude.json failed; field not persisted"
                    );
                    return Err(format!("write ~/.claude.json: {e}"));
                }
            }
        }
    }

    // 写回 DB。
    let value = serde_json::to_value(&current).map_err(|e| e.to_string())?;
    db::set_setting(&db, SetSettingInput {
        scope: "global".to_string(),
        key: "coding_tools_settings".to_string(),
        value,
    }).await?;
    Ok(current)
}

#[cfg(test)]
#[path = "test_coding_tools.rs"]
mod test_coding_tools;
