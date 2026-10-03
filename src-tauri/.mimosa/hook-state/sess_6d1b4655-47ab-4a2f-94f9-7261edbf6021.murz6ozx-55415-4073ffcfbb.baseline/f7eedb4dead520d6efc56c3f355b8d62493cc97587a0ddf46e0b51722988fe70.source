//! defaults.json / client-types.json 读取命令。
//!
//! 真值源 = `defaults/registry/`；`platform_preset` 表是它同步后的落地，DB 有数据即以 DB 为准，
//! 从未同步过才回落编译期内置那份（`aidog_db::registry`）。`~/.aidog/platform-presets.json`
//! 本地文件覆盖链已彻底移除，禁改回。`client_types_const.rs` 仍是代码内常量。

const CLIENT_TYPES_BUNDLED: &str = crate::gateway::client_types_const::BUNDLED;

/// logo 缓存文件扩展名 → `data:` URL 的 MIME 类型。
///
/// 必须覆盖 `logo_sync::LOGO_CACHE_EXTS` 的每一项 —— 漏一个，浏览器形态下那种格式的 logo
/// 就悄悄不显示了。`test_defaults.rs` 里有一条测试盯着这个对应关系。
fn logo_mime_for_ext(ext: Option<&str>) -> Option<&'static str> {
    match ext? {
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        "png" => Some("image/png"),
        "jpg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

crate::tauri_command! {
    pub async fn get_defaults_json() -> Result<String, String> {
    let db = aidog_ctx::db();
        aidog_db::presets_doc_json(db).await
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
    /// 返回 protocol logo 缓存文件的 `data:` URL（**浏览器形态**用）。
    ///
    /// 桌面壳走 `convertFileSrc(path)` 把本地文件映射成 webview 可读的 `asset://` URL；
    /// 浏览器里没有这套协议，本命令直接把缓存文件读成 base64 data URL 回传。
    ///
    /// 只读 `~/.aidog/logos/<protocol>.<ext>`（扩展名由 `logo_cached_path` 在固定白名单里挑），
    /// **不接受任意路径**——管理面上不留通用文件读原语。无缓存 / 读失败返空串。
    pub async fn get_protocol_logo_data_url(protocol: String) -> Result<String, String> {
        tracing::debug!(command = "get_protocol_logo_data_url", protocol = %protocol, "command invoked");
        let dir = crate::shared::aidog_data_dir()?;
        let Some(path) = crate::gateway::logo_sync::logo_cached_path(&dir, &protocol) else {
            return Ok(String::new());
        };
        let Some(mime) = logo_mime_for_ext(path.extension().and_then(|e| e.to_str())) else {
            return Ok(String::new());
        };
        let bytes = std::fs::read(&path).map_err(|e| format!("read logo: {e}"))?;
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(format!("data:{mime};base64,{b64}"))
    }
}

crate::tauri_command! {
    /// 触发单 protocol 后台 logo 同步（前端懒加载 miss 时调）。非阻塞 spawn，立即返。
    pub async fn sync_protocol_logo(protocol: String) -> Result<(), String> {
        tracing::debug!(command = "sync_protocol_logo", protocol = %protocol, "command invoked");
        let db = std::sync::Arc::new(aidog_ctx::db().clone());
        let dir = crate::shared::aidog_data_dir()?;
        tokio::spawn(async move {
            crate::gateway::logo_sync::sync_one_logo(db, dir, protocol).await;
        });
        Ok(())
    }
}

#[cfg(test)]
#[path = "test_defaults.rs"]
mod test_defaults;
