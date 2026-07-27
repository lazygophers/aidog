//! platform-presets.json 读取：app data (`~/.aidog/platform-presets.json`) 优先 → **deep merge**
//! 编译内 bundled（补 app data 缺的 protocol key）→ 缺失/损坏回退 bundled。
//!
//! reader deep merge（2026-07-10）：用户 app data 旧缺 bundled 新增 protocol（如 `glm_coding`）
//! 时，merge 补全让派生层即时拿到全量，不依赖 24h 节流的同步链覆盖。merge 规则见
//! `crate::gateway::defaults_sync::merge_with_bundled`。
//!
//! 同 settings.json：用 `include_str!` 把 `src-tauri/defaults/platform-presets.json` 编入二进制，
//! 不走 Tauri resources（项目现行约定）。
//! C3 c3-commands 第 2 批：commands_config/src/defaults.rs → aidog_core 下沉（原路径
//! `../../../defaults/...` 相对 commands_config/src/，落到 aidog_core/src/ 后同深度，路径不变）。
use crate::shared::aidog_data_dir;

const BUNDLED: &str = include_str!("../../../defaults/platform-presets.json");

const CLIENT_TYPES_BUNDLED: &str = include_str!("../../../defaults/client-types.json");

crate::tauri_command! {
    pub async fn get_defaults_json() -> Result<String, String> {
        tracing::debug!(command = "get_defaults_json", "command invoked");
        // app data 优先（运行时同步链写入）
        if let Ok(dir) = aidog_data_dir() {
            let path = dir.join("platform-presets.json");
            if path.exists() {
                match std::fs::read_to_string(&path) {
                    Ok(content) if content.trim().is_empty() => {
                        tracing::warn!(path = %path.display(), "platform-presets.json empty, fallback to bundled");
                    }
                    Ok(content) => {
                        // 校验可解析，损坏回退 bundled（避免前端拿到坏 JSON 全平台默认值失效）
                        match serde_json::from_str::<serde_json::Value>(&content) {
                            Ok(app_value) => {
                                // deep merge：app data 优先，bundled 补 app data 缺的 protocol key
                                // （用户 app data 旧缺 glm_coding 等 → bundled 补全，派生层不依赖同步）
                                let merged = match serde_json::from_str::<serde_json::Value>(BUNDLED) {
                                    Ok(bundled_value) => crate::gateway::defaults_sync::merge_with_bundled(
                                        &app_value,
                                        &bundled_value,
                                    ),
                                    Err(e) => {
                                        // bundled 解析失败（不可能发生，编译期 JSON 已固定）→ 退 app data 原值
                                        tracing::error!(error = %e, "platform-presets.json bundled parse failed (should never happen), serving app data");
                                        app_value
                                    }
                                };
                                match serde_json::to_string(&merged) {
                                    Ok(s) => {
                                        tracing::debug!(source = "app_data+merged", "platform-presets.json served from app data (deep merged with bundled)");
                                        return Ok(s);
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "platform-presets.json merged serialize failed, fallback to bundled");
                                    }
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, path = %path.display(), "platform-presets.json parse failed, fallback to bundled"),
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, path = %path.display(), "read platform-presets.json failed, fallback to bundled"),
                }
            }
        }
        tracing::debug!(source = "bundled", "platform-presets.json served from bundled");
        Ok(BUNDLED.to_string())
    }
}

crate::tauri_command! {
    /// client-types.json 读取：app data (`~/.aidog/client-types.json`) 优先 → **deep merge**
    /// 编译内 bundled（按 value 去重补 app data 缺的 client_type）→ 缺失/损坏回退 bundled。
    /// 同 get_defaults_json 模式（deep merge + schema gate 失败 / 损坏 → bundled）。
    pub async fn get_client_types_json() -> Result<String, String> {
        tracing::debug!(command = "get_client_types_json", "command invoked");
        if let Ok(dir) = aidog_data_dir() {
            let path = dir.join("client-types.json");
            if path.exists() {
                match std::fs::read_to_string(&path) {
                    Ok(content) if content.trim().is_empty() => {
                        tracing::warn!(path = %path.display(), "client-types.json empty, fallback to bundled");
                    }
                    Ok(content) => {
                        match serde_json::from_str::<serde_json::Value>(&content) {
                            Ok(app_value) => {
                                let merged = match serde_json::from_str::<serde_json::Value>(CLIENT_TYPES_BUNDLED) {
                                    Ok(bundled_value) => crate::gateway::client_types_sync::merge_with_bundled(
                                        &app_value,
                                        &bundled_value,
                                    ),
                                    Err(e) => {
                                        tracing::error!(error = %e, "client-types.json bundled parse failed (should never happen), serving app data");
                                        app_value
                                    }
                                };
                                match serde_json::to_string(&merged) {
                                    Ok(s) => {
                                        tracing::debug!(source = "app_data+merged", "client-types.json served from app data (deep merged with bundled)");
                                        return Ok(s);
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "client-types.json merged serialize failed, fallback to bundled");
                                    }
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, path = %path.display(), "client-types.json parse failed, fallback to bundled"),
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, path = %path.display(), "read client-types.json failed, fallback to bundled"),
                }
            }
        }
        tracing::debug!(source = "bundled", "client-types.json served from bundled");
        Ok(CLIENT_TYPES_BUNDLED.to_string())
    }
}

crate::tauri_command! {
    /// platform-presets.json 同步（jsDelivr 主 + raw fallback）。无视节流——前端手动按钮专用。
    pub async fn sync_defaults_json() -> Result<crate::gateway::defaults_sync::DefaultsSyncResult, String> {
        tracing::debug!(command = "sync_defaults_json", "command invoked");
        Ok(crate::gateway::defaults_sync::sync_defaults_json().await)
    }
}

crate::tauri_command! {
    /// client-types.json 同步（jsDelivr 主 + raw fallback）。无视节流——前端手动按钮专用。
    pub async fn sync_client_types_json() -> Result<crate::gateway::client_types_sync::ClientTypesSyncResult, String> {
        tracing::debug!(command = "sync_client_types_json", "command invoked");
        Ok(crate::gateway::client_types_sync::sync_client_types_json().await)
    }
}

crate::tauri_command! {
    // 原 `pub fn`（同步 I/O）；转 async 以适配 tauri_command! 宏。
    /// 返回 protocol logo 缓存文件路径（前端 `convertFileSrc` 用）。文件不存在/无缓存目录返空串。
    pub async fn get_protocol_logo_path(protocol: String) -> Result<String, String> {
        tracing::debug!(command = "get_protocol_logo_path", protocol = %protocol, "command invoked");
        let dir = aidog_data_dir()?;
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
        let db = app.try_state::<crate::gateway::db::Db>()
            .map(|s| std::sync::Arc::new(s.inner().clone()))
            .ok_or("db not initialized")?;
        let dir = aidog_data_dir()?;
        tauri::async_runtime::spawn(async move {
            crate::gateway::logo_sync::sync_one_logo(db, dir, protocol).await;
        });
        Ok(())
    }
}
