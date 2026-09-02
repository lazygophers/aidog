//! defaults.json / client-types.json 读取命令。
//!
//! 真值源 = `defaults/registry/`；`platform_preset` 表是它同步后的落地，DB 有数据即以 DB 为准，
//! 从未同步过才回落编译期内置那份（`aidog_db::registry`）。`~/.aidog/platform-presets.json`
//! 本地文件覆盖链已彻底移除，禁改回。`client_types_const.rs` 仍是代码内常量。

const CLIENT_TYPES_BUNDLED: &str = crate::gateway::client_types_const::BUNDLED;

crate::tauri_command! {
    pub async fn get_defaults_json() -> Result<String, String> {
    let db = aidog_ctx::db();
        aidog_db::presets_doc_json(&db).await
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
        if let Some(path) = crate::gateway::logo_sync::logo_cached_path(&dir, &protocol) {
            return Ok(path.to_string_lossy().into_owned());
        }
        Ok(String::new())
    }
}

crate::tauri_command! {
    /// 触发单 protocol 后台 logo 同步（前端懒加载 miss 时调）。非阻塞 spawn，立即返。
    pub async fn sync_protocol_logo(protocol: String) -> Result<(), String> {
        tracing::debug!(command = "sync_protocol_logo", protocol = %protocol, "command invoked");
        let db = std::sync::Arc::new(aidog_ctx::db().clone());
        let dir = crate::shared::aidog_data_dir()?;
        tauri::async_runtime::spawn(async move {
            crate::gateway::logo_sync::sync_one_logo(db, dir, protocol).await;
        });
        Ok(())
    }
}
