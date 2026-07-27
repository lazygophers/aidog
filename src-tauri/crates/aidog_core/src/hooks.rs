use crate::shared::*;
use crate::sync_settings::do_sync_group_settings;
use crate::gateway::{self, db::Db};
use gateway::models::*;
use tauri::State;


pub fn generate_hook_scripts(
    invoker: gateway::scripts::ScriptInvoker,
) -> Result<gateway::hooks::ScriptPaths, String> {
    let scripts_dir = aidog_scripts_dir()?;
    let chmod755 = |path: &std::path::Path, filename: &str| -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)
                .map_err(|e| format!("stat hook script {filename}: {e}"))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms)
                .map_err(|e| format!("chmod hook script {filename}: {e}"))?;
        }
        #[cfg(not(unix))]
        let _ = (path, filename);
        Ok(())
    };
    let write_type_script = |filename: &str, legacy: &str, notif_type: &str| -> Result<String, String> {
        let path = scripts_dir.join(filename);
        let content = gateway::hooks::build_hook_script(notif_type);
        std::fs::write(&path, &content).map_err(|e| format!("write hook script {filename}: {e}"))?;
        chmod755(&path, filename)?;
        // 迁移清理：删除 ~/.aidog/ 根下旧版 bash 脚本（避免残留）。
        cleanup_legacy_root_script(legacy);
        Ok(invoker.command_for(&path.to_string_lossy()))
    };
    // 通用事件脚本（N2）：读 stdin hook_event_name，无内插 type。
    let event_path = scripts_dir.join(gateway::hooks::SCRIPT_EVENT_NOTIFY);
    std::fs::write(&event_path, gateway::hooks::build_event_notify_script())
        .map_err(|e| format!("write event notify script: {e}"))?;
    chmod755(&event_path, gateway::hooks::SCRIPT_EVENT_NOTIFY)?;
    let event_notify = invoker.command_for(&event_path.to_string_lossy());

    // waiting 脚本已并入通用事件脚本（N2），不再生成；仅清理历史 ~/.aidog/*.sh 残留。
    cleanup_legacy_root_script(gateway::hooks::LEGACY_SCRIPT_WAITING);

    Ok(gateway::hooks::ScriptPaths {
        complete: write_type_script(
            gateway::hooks::SCRIPT_COMPLETE,
            gateway::hooks::LEGACY_SCRIPT_COMPLETE,
            "task_complete",
        )?,
        event_notify,
    })
}

/// 从 NotificationSettings 解析 enabled 的 CC hook 事件名列表（用于注入遍历）。
/// per_event 为空（旧配置/未配）时回退默认精选 ON 集，保证总开关开时有事件可注入。
pub async fn enabled_hook_events(db: &Db) -> Vec<String> {
    let settings = gateway::db::get_notification_settings(db).await;
    if settings.per_event.is_empty() {
        return gateway::models::DEFAULT_ON_EVENTS.iter().map(|s| s.to_string()).collect();
    }
    settings
        .per_event
        .iter()
        .filter(|(_, es)| es.enabled)
        .map(|(name, _)| name.clone())
        .collect()
}

/// 把内置默认模板物化进 NotificationSettings.per_type[task_complete/waiting_input]（仅在缺失/空时填）。
/// 用户已自定义模板则不覆盖。
pub async fn seed_default_templates(db: &Db) -> Result<(), String> {
    use gateway::models::{NotifType, TypeSetting};
    let mut settings = gateway::db::get_notification_settings(db).await;
    let mut changed = false;
    for t in [NotifType::TaskComplete, NotifType::WaitingInput] {
        let key = t.as_str().to_string();
        let entry = settings.per_type.entry(key).or_insert_with(TypeSetting::default);
        if entry.template.trim().is_empty() {
            entry.template = t.default_template().to_string();
            changed = true;
        }
    }
    if changed {
        gateway::db::set_setting(db, SetSettingInput {
            scope: "notification".to_string(),
            key: "settings".to_string(),
            value: serde_json::to_value(&settings).map_err(|e| format!("serialize notification settings: {e}"))?,
        }).await?;
    }
    Ok(())
}

/// 读取「默认为所有分组注入通知 hook」总开关状态（基线 `claude_code._aidog_hooks.enabled`）。
/// 无基线配置时回退编译内默认（defaults/settings.json 默认开）。
/// `&Db` 版本：供 command 包装层转发，也供测试直调（不经 tauri::State，见 test_hooks 模块注释）。
async fn default_hooks_enabled_from_db(db: &Db) -> bool {
    let config = gateway::db::get_setting(db, "global", "claude_code").await
        .ok().flatten()
        .filter(|v| v.is_object() && v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(|| serde_json::from_str(include_str!("../../../defaults/settings.json"))
            .unwrap_or(serde_json::Value::Object(Default::default())));
    gateway::hooks::hooks_marker_enabled(&config)
}

/// 构造通知 hook 片段供前端 Hooks 编辑器并入草稿（只读式）。
/// - 确保 notify 脚本已落盘 `~/.aidog/scripts/`（`generate_hook_scripts`）。
/// - 在空对象上走 `inject_claude_code_hooks`，取出其 `hooks` 子对象
///   （`{Stop:[...], Notification:[...]}`）返回。
///
/// **不写 DB、不 sync**：物化由用户正常保存触发既有链路。
/// `&Db` 版本：供 command 包装层转发，也供测试直调。
async fn build_notify_hooks_fragment_from_db(db: &Db) -> Result<serde_json::Value, String> {
    let invoker = resolve_script_invoker(db).await;
    let scripts = generate_hook_scripts(invoker)?;
    let events = enabled_hook_events(db).await;
    let mut config = serde_json::json!({});
    gateway::hooks::inject_claude_code_hooks(&mut config, &scripts, &events);
    Ok(config
        .get("hooks")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(Default::default())))
}

crate::tauri_command! {
    /// 一键注入通知 hook。
    /// - `client="claude_code"`：把 hooks.Stop/Notification 注入基线 `claude_code` 配置，
    ///   re-sync 物化到所有 `settings.{group}.json`（与 statusLine 同机制）。
    /// - `client="codex"`：把 `notify=[<complete 脚本>]` 注入 `~/.codex/config.toml`。
    /// 同时物化内置默认模板。`group` 入参用于 API 对称（Claude Code hooks 走基线对全分组生效）。
    pub async fn inject_hooks(
        app: tauri::AppHandle,
        db: State<'_, Db>,
        group: String,
        client: String,
    ) -> Result<(), String> {
        tracing::debug!(command = "inject_hooks", group = %group, client = %client, "command invoked");
        let hook_client = gateway::hooks::HookClient::from_str(&client)?;
        let invoker = resolve_script_invoker(&db).await;
        let scripts = generate_hook_scripts(invoker)?;
        seed_default_templates(&db).await?;

        match hook_client {
            gateway::hooks::HookClient::ClaudeCode => {
                // 读基线 claude_code 配置（无则用编译内默认）注入 hooks，回写 + re-sync。
                let mut config = gateway::db::get_setting(&db, "global", "claude_code").await
                    .ok().flatten()
                    .filter(|v| v.is_object())
                    .unwrap_or_else(|| serde_json::from_str(include_str!("../../../defaults/settings.json"))
                        .unwrap_or(serde_json::Value::Object(Default::default())));
                let events = enabled_hook_events(&db).await;
                gateway::hooks::inject_claude_code_hooks(&mut config, &scripts, &events);
                gateway::db::set_setting(&db, SetSettingInput {
                    scope: "global".to_string(),
                    key: "claude_code".to_string(),
                    value: config,
                }).await?;
                let port = load_proxy_settings(&app).await?.port;
                do_sync_group_settings(&db, port).await?;
            }
            gateway::hooks::HookClient::Codex => {
                let mut config = gateway::codex::codex_config_read()?;
                gateway::hooks::inject_codex_notify(&mut config, &scripts.complete);
                gateway::codex::codex_config_write(config)?;
            }
        }
        Ok(())
    }
}

crate::tauri_command! {
    /// 一键移除通知 hook（strip）。client 同 inject_hooks。
    pub async fn remove_hooks(
        app: tauri::AppHandle,
        db: State<'_, Db>,
        group: String,
        client: String,
    ) -> Result<(), String> {
        tracing::debug!(command = "remove_hooks", group = %group, client = %client, "command invoked");
        let hook_client = gateway::hooks::HookClient::from_str(&client)?;
        match hook_client {
            gateway::hooks::HookClient::ClaudeCode => {
                let Some(mut config) = gateway::db::get_setting(&db, "global", "claude_code").await
                    .ok().flatten().filter(|v| v.is_object()) else {
                    // 无基线配置 → 无 aidog hook 可清，re-sync 即可（settings 文件 strip 已生效）。
                    let port = load_proxy_settings(&app).await?.port;
                    return do_sync_group_settings(&db, port).await.map(|_| ());
                };
                gateway::hooks::remove_claude_code_hooks(&mut config);
                gateway::db::set_setting(&db, SetSettingInput {
                    scope: "global".to_string(),
                    key: "claude_code".to_string(),
                    value: config,
                }).await?;
                let port = load_proxy_settings(&app).await?.port;
                do_sync_group_settings(&db, port).await?;
            }
            gateway::hooks::HookClient::Codex => {
                let mut config = gateway::codex::codex_config_read()?;
                gateway::hooks::remove_codex_notify(&mut config);
                gateway::codex::codex_config_write(config)?;
            }
        }
        Ok(())
    }
}

crate::tauri_command! {
    /// 读取「默认为所有分组注入通知 hook」总开关状态（基线 `claude_code._aidog_hooks.enabled`）。
    pub async fn get_default_hooks_enabled(db: State<'_, Db>) -> Result<bool, String> {
        tracing::debug!(command = "get_default_hooks_enabled", "command invoked");
        Ok(default_hooks_enabled_from_db(&db).await)
    }
}

crate::tauri_command! {
    /// 设置「默认为所有分组注入通知 hook」总开关：写基线 `claude_code._aidog_hooks.enabled`，
    /// re-sync 物化（开=全分组 CC hooks + Codex notify；关=全移除）。
    pub async fn set_default_hooks_enabled(
        app: tauri::AppHandle,
        db: State<'_, Db>,
        enabled: bool,
    ) -> Result<(), String> {
        tracing::debug!(command = "set_default_hooks_enabled", enabled, "command invoked");
        // 读基线 claude_code 配置（无则用编译内默认），设置 marker，回写。
        let mut config = gateway::db::get_setting(&db, "global", "claude_code").await
            .ok().flatten()
            .filter(|v| v.is_object() && v.as_object().is_some_and(|o| !o.is_empty()))
            .unwrap_or_else(|| serde_json::from_str(include_str!("../../../defaults/settings.json"))
                .unwrap_or(serde_json::Value::Object(Default::default())));
        if let Some(obj) = config.as_object_mut() {
            obj.insert(
                gateway::hooks::MARKER_HOOKS.to_string(),
                serde_json::json!({ "enabled": enabled }),
            );
        }
        gateway::db::set_setting(&db, SetSettingInput {
            scope: "global".to_string(),
            key: "claude_code".to_string(),
            value: config,
        }).await?;
        // 开启时确保默认模板已物化（与 inject_hooks 行为一致）。
        if enabled {
            seed_default_templates(&db).await?;
        }
        let port = load_proxy_settings(&app).await?.port;
        do_sync_group_settings(&db, port).await?;
        Ok(())
    }
}

crate::tauri_command! {
    /// 构造通知 hook 片段供前端 Hooks 编辑器并入草稿（只读式），薄转发 `&Db` 版本。
    pub async fn build_notify_hooks_fragment(db: State<'_, Db>) -> Result<serde_json::Value, String> {
        tracing::debug!(command = "build_notify_hooks_fragment", "command invoked");
        build_notify_hooks_fragment_from_db(&db).await
    }
}

#[cfg(test)]
mod test_hooks {
    use super::*;
    use crate::gateway::db::test_support::{test_db, sample_group, HomeGuard};

    /// aidog_core 不能 dev-dep aidog_test_util（循环依赖），故不经 tauri::State/AppHandle
    /// 走 command 包装层，直测 &Db 版本函数（command 本身只是薄转发）。

    #[tokio::test]
    async fn enabled_events_default_when_per_event_empty() {
        let db = test_db().await;
        let events = enabled_hook_events(&db).await;
        assert!(!events.is_empty());
        assert_eq!(
            events.len(),
            gateway::models::DEFAULT_ON_EVENTS.len()
        );
    }

    #[tokio::test]
    async fn seed_templates_fills_then_idempotent() {
        let db = test_db().await;
        seed_default_templates(&db).await.unwrap();
        let settings = gateway::db::get_notification_settings(&db).await;
        let complete = settings.per_type.get(NotifType::TaskComplete.as_str()).unwrap();
        assert!(!complete.template.trim().is_empty());
        let first_template = complete.template.clone();
        // 幂等：再跑一次不覆盖已有模板
        seed_default_templates(&db).await.unwrap();
        let settings2 = gateway::db::get_notification_settings(&db).await;
        assert_eq!(
            settings2.per_type.get(NotifType::TaskComplete.as_str()).unwrap().template,
            first_template
        );
    }

    #[tokio::test]
    async fn generate_scripts_writes_files() {
        let _guard = HomeGuard::new();
        let invoker = gateway::scripts::ScriptInvoker::Python3;
        let scripts = generate_hook_scripts(invoker).unwrap();
        assert!(!scripts.complete.is_empty());
        assert!(!scripts.event_notify.is_empty());
    }

    #[tokio::test]
    async fn build_fragment_returns_hooks_object() {
        let _guard = HomeGuard::new();
        let db = test_db().await;
        let fragment = build_notify_hooks_fragment_from_db(&db).await.unwrap();
        assert!(fragment.is_object());
    }

    #[tokio::test]
    async fn claude_code_inject_remove_and_sync() {
        let _guard = HomeGuard::new();
        let db = test_db().await;
        gateway::db::create_group(&db, sample_group("default", vec![])).await.unwrap();

        // inject（&Db 版本等价逻辑，绕开 tauri::State/AppHandle 依赖的 port 解析）
        let mut config: serde_json::Value = serde_json::from_str(include_str!("../../../defaults/settings.json")).unwrap();
        let events = enabled_hook_events(&db).await;
        let invoker = gateway::scripts::ScriptInvoker::Python3;
        let scripts = generate_hook_scripts(invoker).unwrap();
        gateway::hooks::inject_claude_code_hooks(&mut config, &scripts, &events);
        gateway::db::set_setting(&db, SetSettingInput {
            scope: "global".to_string(),
            key: "claude_code".to_string(),
            value: config.clone(),
        }).await.unwrap();
        do_sync_group_settings(&db, 0).await.unwrap();
        assert!(config.get("hooks").is_some());

        // remove
        let mut config2 = gateway::db::get_setting(&db, "global", "claude_code").await.unwrap().unwrap();
        gateway::hooks::remove_claude_code_hooks(&mut config2);
        gateway::db::set_setting(&db, SetSettingInput {
            scope: "global".to_string(),
            key: "claude_code".to_string(),
            value: config2.clone(),
        }).await.unwrap();
        assert!(config2.get("hooks").and_then(|h| h.get("Stop")).is_none());
    }

    #[tokio::test]
    async fn default_hooks_enabled_get_set() {
        let _guard = HomeGuard::new();
        let db = test_db().await;
        // 默认值（defaults/settings.json 决定）
        let default_val = default_hooks_enabled_from_db(&db).await;

        // 写 marker 翻转，验证读回一致
        let mut config: serde_json::Value = serde_json::from_str(include_str!("../../../defaults/settings.json")).unwrap();
        if let Some(obj) = config.as_object_mut() {
            obj.insert(
                gateway::hooks::MARKER_HOOKS.to_string(),
                serde_json::json!({ "enabled": !default_val }),
            );
        }
        gateway::db::set_setting(&db, SetSettingInput {
            scope: "global".to_string(),
            key: "claude_code".to_string(),
            value: config,
        }).await.unwrap();
        let flipped = default_hooks_enabled_from_db(&db).await;
        assert_eq!(flipped, !default_val);
    }

    #[tokio::test]
    async fn codex_notify_inject_remove() {
        let mut config = serde_json::json!({});
        gateway::hooks::inject_codex_notify(&mut config, "/path/to/aidog-notify-complete.py");
        assert!(config.get("notify").is_some());
        gateway::hooks::remove_codex_notify(&mut config);
        assert!(config.get("notify").is_none());
    }

    #[tokio::test]
    async fn unknown_client_errors() {
        let result = gateway::hooks::HookClient::from_str("unknown");
        assert!(result.is_err());
    }
}
