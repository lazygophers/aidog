//! defaults.json 读取：app data (`~/.aidog/defaults.json`) 优先 → 回退编译内 bundled。
//!
//! 同 settings.json：用 `include_str!` 把 `src-tauri/defaults/defaults.json` 编入二进制，
//! 不走 Tauri resources（项目现行约定）。
use crate::shared::aidog_data_dir;

const BUNDLED: &str = include_str!("../../defaults/defaults.json");

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn get_defaults_json() -> Result<String, String> {
    tracing::debug!(command = "get_defaults_json", "command invoked");
    // app data 优先（运行时同步链写入）
    if let Ok(dir) = aidog_data_dir() {
        let path = dir.join("defaults.json");
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) if content.trim().is_empty() => {
                    tracing::warn!(path = %path.display(), "defaults.json empty, fallback to bundled");
                }
                Ok(content) => {
                    // 校验可解析，损坏回退 bundled（避免前端拿到坏 JSON 全平台默认值失效）
                    match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(_) => {
                            tracing::debug!(source = "app_data", "defaults.json served from app data");
                            return Ok(content);
                        }
                        Err(e) => tracing::warn!(error = %e, path = %path.display(), "defaults.json parse failed, fallback to bundled"),
                    }
                }
                Err(e) => tracing::warn!(error = %e, path = %path.display(), "read defaults.json failed, fallback to bundled"),
            }
        }
    }
    tracing::debug!(source = "bundled", "defaults.json served from bundled");
    Ok(BUNDLED.to_string())
}
