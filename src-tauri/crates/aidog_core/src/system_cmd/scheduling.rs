use crate::gateway;
use gateway::models::*;

// ─── Scheduling & Breaker Settings ─────────────────────────

crate::tauri_command! {
    pub async fn scheduling_settings_get() -> Result<SchedulingBreakerSettings, String> {
    let db = aidog_ctx::db();
        Ok(aidog_db::get_scheduling_settings(&db).await)
    }
}

crate::tauri_command! {
    pub async fn scheduling_settings_set(
        settings: SchedulingBreakerSettings) -> Result<(), String> {
    let db = aidog_ctx::db();
        aidog_db::set_setting(&db, SetSettingInput {
            scope: "scheduling".to_string(),
            key: "settings".to_string(),
            value: serde_json::to_value(&settings).map_err(|e| format!("serialize scheduling settings: {e}"))?,
        }).await
            .map_err(|e| { tracing::error!(command = "scheduling_settings_set", error = %e, "persist scheduling settings failed"); e })
    }
}

#[cfg(test)]
#[path = "test_scheduling.rs"]
mod test_scheduling;
