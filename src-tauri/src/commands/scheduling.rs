use aidog_core::gateway::{self, db::Db};
#[allow(unused_imports)]
use aidog_core::logging;
#[allow(unused_imports)]
use gateway::models::*;
#[allow(unused_imports)]
use tauri::State;
#[allow(unused_imports)]
use serde_json::Value;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use tauri::Manager;


// ─── Scheduling & Breaker Settings ─────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn scheduling_settings_get(db: State<'_, Db>) -> Result<SchedulingBreakerSettings, String> {
    tracing::debug!(command = "scheduling_settings_get", "command invoked");
    Ok(gateway::db::get_scheduling_settings(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn scheduling_settings_set(
    db: State<'_, Db>,
    settings: SchedulingBreakerSettings,
) -> Result<(), String> {
    tracing::debug!(command = "scheduling_settings_set", "command invoked");
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "scheduling".to_string(),
        key: "settings".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize scheduling settings: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "scheduling_settings_set", error = %e, "persist scheduling settings failed"); e })
}

#[cfg(test)]
#[path = "test_scheduling.rs"]
mod test_scheduling;
