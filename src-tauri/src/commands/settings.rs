use crate::shared::*;
use crate::commands::sync_settings::try_sync_settings;
use crate::gateway::{self, db::{self, Db}};
#[allow(unused_imports)]
use crate::logging;
#[allow(unused_imports)]
use gateway::models::*;
#[allow(unused_imports)]
use tauri::State;
#[allow(unused_imports)]
use serde_json::Value;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use tauri::Manager;


use gateway::models::SetSettingInput;

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn settings_get(
    scope: String,
    key: String,
    db: State<'_, Db>,
) -> Result<Option<serde_json::Value>, String> {
    tracing::debug!(command = "settings_get", scope = %scope, key = %key, "command invoked");
    db::get_setting(&db, &scope, &key).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn settings_set(input: SetSettingInput, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "settings_set", scope = %input.scope, key = %input.key, "command invoked");
    db::set_setting(&db, input).await
        .map_err(|e| { tracing::error!(command = "settings_set", error = %e, "persist setting failed"); e })?;
    // Auto-sync group settings files when claude code config changes
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn settings_delete(scope: String, key: String, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "settings_delete", scope = %scope, key = %key, "command invoked");
    db::delete_setting(&db, &scope, &key).await
        .map_err(|e| { tracing::error!(command = "settings_delete", error = %e, "delete setting failed"); e })
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn settings_list(scope: String, db: State<'_, Db>) -> Result<Vec<String>, String> {
    tracing::debug!(command = "settings_list", scope = %scope, "command invoked");
    db::list_setting_keys(&db, &scope).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn generate_statusline_script(
    script_type: String,
    content: String,
    db: State<'_, Db>,
) -> Result<String, String> {
    tracing::debug!(command = "generate_statusline_script", script_type = %script_type, "command invoked");
    let scripts_dir = aidog_scripts_dir()?;
    let (filename, legacy_sh) = if script_type == "subagent" {
        ("aidog-subagent-statusline.py", "aidog-subagent-statusline.sh")
    } else {
        ("aidog-statusline.py", "aidog-statusline.sh")
    };
    // 迁移清理：删除旧版 bash 脚本（~/.aidog/ 根 + scripts/ 下）。
    cleanup_legacy_root_script(filename);
    cleanup_legacy_root_script(legacy_sh);
    cleanup_legacy_scripts_dir_file(&scripts_dir, legacy_sh);
    let path = scripts_dir.join(filename);
    std::fs::write(&path, &content).map_err(|e| { tracing::error!(command = "generate_statusline_script", error = %e, "write script failed"); format!("write script: {e}") })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).map_err(|e| { tracing::error!(command = "generate_statusline_script", error = %e, "stat script failed"); format!("stat script: {e}") })?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).map_err(|e| { tracing::error!(command = "generate_statusline_script", error = %e, "chmod script failed"); format!("chmod script: {e}") })?;
    }
    let invoker = resolve_script_invoker(&db).await;
    Ok(invoker.command_for(&path.to_string_lossy()))
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub fn read_claude_code_settings() -> Result<serde_json::Value, String> {
    tracing::debug!(command = "read_claude_code_settings", "command invoked");
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        tracing::warn!(command = "read_claude_code_settings", "~/.claude/settings.json not found");
        return Err("~/.claude/settings.json not found".into());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| { tracing::warn!(command = "read_claude_code_settings", error = %e, "read settings failed"); format!("read settings: {e}") })?;
    serde_json::from_str(&content).map_err(|e| { tracing::warn!(command = "read_claude_code_settings", error = %e, "parse settings failed"); format!("parse settings: {e}") })
}
