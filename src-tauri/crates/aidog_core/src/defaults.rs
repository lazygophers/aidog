//! defaults.json / client-types.json 读取命令。
//!
//! 真值源 = `defaults/registry/`（编译期 include，见 `aidog_db::registry`）与代码内
//! `client_types_const.rs`；app data 覆盖与远端同步链整体移除——内置即唯一，禁改。

const CLIENT_TYPES_BUNDLED: &str = crate::gateway::client_types_const::BUNDLED;

crate::tauri_command! {
    pub async fn get_defaults_json() -> Result<String, String> {
        Ok(crate::gateway::registry::presets_json().to_string())
    }
}

crate::tauri_command! {
    pub async fn get_client_types_json() -> Result<String, String> {
        Ok(CLIENT_TYPES_BUNDLED.to_string())
    }
}

crate::tauri_command! {
    // 原 `pub fn`（同步 I/O）；转 async 以适配 tauri_command! 宏。
    /// 返回 protocol logo 缓存文件路径（前端 `convertFileSrc` 用）。文件不存在/无缓存目录返空串。
    pub async fn get_protocol_logo_path(protocol: String) -> Result<String, String> {
        tracing::debug!(command = "get_protocol_logo_path", protocol = %protocol, "command invoked");
        let dir = crate::shared::aidog_data_dir()?;
        let path = crate::gateway::logo_sync::logo_cache_path(&dir, &protocol);
        if path.exists()
            && let Ok(meta) = std::fs::metadata(&path)
                && meta.len() > 0 {
                    return Ok(path.to_string_lossy().into_owned());
                }
        Ok(String::new())
    }
}

crate::tauri_command! {
    /// 触发单 protocol 后台 logo 同步（前端懒加载 miss 时调）。非阻塞 spawn，立即返。
    pub async fn sync_protocol_logo(
        app: tauri::AppHandle,
        protocol: String,
    ) -> Result<(), String> {
        tracing::debug!(command = "sync_protocol_logo", protocol = %protocol, "command invoked");
        use tauri::Manager;
        let db = app.try_state::<aidog_db::Db>()
            .map(|s| std::sync::Arc::new(s.inner().clone()))
            .ok_or("db not initialized")?;
        let dir = crate::shared::aidog_data_dir()?;
        tauri::async_runtime::spawn(async move {
            crate::gateway::logo_sync::sync_one_logo(db, dir, protocol).await;
        });
        Ok(())
    }
}
