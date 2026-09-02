use crate::gateway;
use gateway::models::*;

use gateway::models::ProxyTimeoutSettings;

crate::tauri_command! {
pub async fn proxy_timeout_get() -> Result<ProxyTimeoutSettings, String> {
    let db = aidog_ctx::db();
    Ok(aidog_db::get_setting(db, "proxy", "timeout").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}
}

crate::tauri_command! {
pub async fn proxy_timeout_set( settings: ProxyTimeoutSettings) -> Result<(), String> {
    let db = aidog_ctx::db();
    aidog_db::set_setting(db, SetSettingInput {
        scope: "proxy".to_string(),
        key: "timeout".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_timeout_set", error = %e, "persist timeout settings failed"); e })
}
}

#[cfg(test)]
#[path = "test_proxy_timeout.rs"]
mod test_proxy_timeout;
