//! settings 域 command（C3 c3-commands 第 2 批：commands_config → aidog_core 下沉）。
//!
//! 薄壳：转 `gateway::db` 的 `*_setting` 函数 + statusline 脚本生成 + claude code 配置读取。

use crate::gateway;
use crate::sync_settings::try_sync_settings;
use aidog_db::{self as db};

use gateway::models::SetSettingInput;

crate::tauri_command! {
    pub async fn settings_get(scope: String, key: String) -> Result<Option<serde_json::Value>, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "settings_get", scope = %scope, key = %key, "command invoked");
        db::get_setting(db, &scope, &key).await
    }
}

crate::tauri_command! {
    pub async fn settings_set(input: SetSettingInput) -> Result<(), String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "settings_set", scope = %input.scope, key = %input.key, "command invoked");
        db::set_setting(db, input).await?;
        // Auto-sync group settings files when claude code config changes
        try_sync_settings(db).await;
        // P2 #4: 同步刷新 ProxyState 设置缓存，禁陈旧（请求路径直接读缓存）。
        // proxy 未启动 → no-op（refresh 内部判 weak stale）。
        gateway::proxy::refresh_proxy_settings_cache(db).await;
        Ok(())
    }
}

crate::tauri_command! {
    pub async fn settings_delete(scope: String, key: String) -> Result<(), String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "settings_delete", scope = %scope, key = %key, "command invoked");
        db::delete_setting(db, &scope, &key).await
    }
}

crate::tauri_command! {
    pub async fn settings_list(scope: String) -> Result<Vec<String>, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "settings_list", scope = %scope, "command invoked");
        db::list_setting_keys(db, &scope).await
    }
}

crate::tauri_command! {
    /// 只读预览：按 UI 当前的 statusline 配置渲染脚本正文，不落盘。
    /// 真正的脚本物化在 `do_sync_group_settings`（每次启动 / 每次改设置无条件重写）。
    pub async fn preview_statusline_script(
        script_type: String,
        stored: serde_json::Value) -> Result<String, String> {
        tracing::debug!(command = "preview_statusline_script", script_type = %script_type, "command invoked");
        Ok(crate::statusline::preview(&stored, script_type != "subagent"))
    }
}

crate::tauri_command! {
    // 原 `pub fn`（同步 I/O）；转 async 以适配 tauri_command! 宏（invoke 端行为不变，
    // Tauri command 无论同步/异步前端均走 Promise）。
    pub async fn read_claude_code_settings() -> Result<serde_json::Value, String> {
        let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
        let path = home.join(".claude").join("settings.json");
        if !path.exists() {
            return Err("~/.claude/settings.json not found".into());
        }
        let content = std::fs::read_to_string(&path).map_err(|e| format!("read settings: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("parse settings: {e}"))
    }
}

#[cfg(test)]
mod test_settings {
    use super::*;
    use aidog_db::test_support::test_db;

    /// aidog_core 不能 dev-dep aidog_test_util（循环依赖），故不经 tauri::State/AppHandle
    /// 走 command 包装层，直测 command 转发的 db:: 函数（command 本身只是薄转发 + tracing）。
    #[tokio::test]
    async fn get_delete_list_settings() {
        let db = test_db().await;

        assert!(
            db::get_setting(&db, "scope1", "k1")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db::list_setting_keys(&db, "scope1")
                .await
                .unwrap()
                .is_empty()
        );

        db::set_setting(
            &db,
            SetSettingInput {
                scope: "scope1".into(),
                key: "k1".into(),
                value: serde_json::json!({"v": 1}),
            },
        )
        .await
        .unwrap();

        assert!(
            db::get_setting(&db, "scope1", "k1")
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(db::list_setting_keys(&db, "scope1").await.unwrap().len(), 1);

        db::delete_setting(&db, "scope1", "k1").await.unwrap();
        assert!(
            db::get_setting(&db, "scope1", "k1")
                .await
                .unwrap()
                .is_none()
        );
    }
}
