//! platform-presets.json 读取：app data (`~/.aidog/platform-presets.json`) 优先 → 回退编译内 bundled。
//!
//! 同 settings.json：用 `include_str!` 把 `src-tauri/defaults/platform-presets.json` 编入二进制，
//! 不走 Tauri resources（项目现行约定）。
use aidog_core::shared::aidog_data_dir;

const BUNDLED: &str = include_str!("../../defaults/platform-presets.json");

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
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
                        Ok(_) => {
                            tracing::debug!(source = "app_data", "platform-presets.json served from app data");
                            return Ok(content);
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

/// platform-presets.json 同步（jsDelivr 主 + raw fallback）。无视节流——前端手动按钮专用。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn sync_defaults_json() -> Result<aidog_core::gateway::defaults_sync::DefaultsSyncResult, String> {
    tracing::debug!(command = "sync_defaults_json", "command invoked");
    Ok(aidog_core::gateway::defaults_sync::sync_defaults_json().await)
}

/// 返回 protocol logo 缓存文件路径（前端 `convertFileSrc` 用）。文件不存在/无缓存目录返空串。
#[tauri::command]
pub fn get_protocol_logo_path(protocol: String) -> Result<String, String> {
    let dir = aidog_data_dir()?;
    let path = aidog_core::gateway::logo_sync::logo_cache_path(&dir, &protocol);
    if path.exists() {
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > 0 {
                return Ok(path.to_string_lossy().into_owned());
            }
        }
    }
    Ok(String::new())
}

/// 触发单 protocol 后台 logo 同步（前端懒加载 miss 时调）。非阻塞 spawn，立即返。
#[tauri::command]
pub async fn sync_protocol_logo(
    app: tauri::AppHandle,
    protocol: String,
) -> Result<(), String> {
    use tauri::Manager;
    let db = app.try_state::<aidog_core::gateway::db::Db>()
        .map(|s| std::sync::Arc::new(s.inner().clone()))
        .ok_or("db not initialized")?;
    let dir = aidog_data_dir()?;
    tauri::async_runtime::spawn(async move {
        aidog_core::gateway::logo_sync::sync_one_logo(db, dir, protocol).await;
    });
    Ok(())
}
