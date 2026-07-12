use aidog_core::gateway::{self, db::Db};
use gateway::models::*;
use tauri::State;


use gateway::models::ProxyTimeoutSettings;

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_timeout_get(db: State<'_, Db>) -> Result<ProxyTimeoutSettings, String> {
    tracing::debug!(command = "proxy_timeout_get", "command invoked");
    Ok(gateway::db::get_setting(&db, "proxy", "timeout").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_timeout_set(db: State<'_, Db>, settings: ProxyTimeoutSettings) -> Result<(), String> {
    tracing::debug!(command = "proxy_timeout_set", "command invoked");
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "proxy".to_string(),
        key: "timeout".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_timeout_set", error = %e, "persist timeout settings failed"); e })
}

#[cfg(test)]
#[path = "test_proxy_timeout.rs"]
mod test_proxy_timeout;
