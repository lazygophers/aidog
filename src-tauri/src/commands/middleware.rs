use crate::gateway::{self, db::Db};
#[allow(unused_imports)]
use crate::logging;
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


use gateway::middleware::MiddlewareEngine;
use gateway::models::{
    CreateMiddlewareRule, MiddlewareRule, MiddlewareSettings, UpdateMiddlewareRule,
};

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn middleware_list_rules(db: State<'_, Db>) -> Result<Vec<MiddlewareRule>, String> {
    tracing::debug!(command = "middleware_list_rules", "command invoked");
    gateway::db::list_middleware_rules(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn middleware_create_rule(
    input: CreateMiddlewareRule,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<MiddlewareRule, String> {
    tracing::debug!(command = "middleware_create_rule", "command invoked");
    let rule = gateway::db::create_middleware_rule(&db, input).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_create_rule", error = %e, "engine reload failed");
    }
    Ok(rule)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn middleware_update_rule(
    input: UpdateMiddlewareRule,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<MiddlewareRule, String> {
    tracing::debug!(command = "middleware_update_rule", "command invoked");
    let rule = gateway::db::update_middleware_rule(&db, input).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_update_rule", error = %e, "engine reload failed");
    }
    Ok(rule)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn middleware_delete_rule(
    id: i64,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<(), String> {
    tracing::debug!(command = "middleware_delete_rule", id, "command invoked");
    gateway::db::delete_middleware_rule(&db, id).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_delete_rule", error = %e, "engine reload failed");
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn middleware_settings_get(db: State<'_, Db>) -> Result<MiddlewareSettings, String> {
    tracing::debug!(command = "middleware_settings_get", "command invoked");
    Ok(gateway::db::get_setting(&db, "middleware", "settings").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn middleware_settings_set(
    db: State<'_, Db>,
    settings: MiddlewareSettings,
) -> Result<(), String> {
    tracing::debug!(command = "middleware_settings_set", "command invoked");
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "middleware".to_string(),
        key: "settings".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize middleware settings: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "middleware_settings_set", error = %e, "persist middleware settings failed"); e })
}

#[cfg(test)]
#[path = "test_middleware.rs"]
mod test_middleware;
