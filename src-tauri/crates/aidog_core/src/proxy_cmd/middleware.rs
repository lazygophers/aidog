use crate::gateway;
use aidog_db::Db;
use gateway::models::*;
use tauri::State;
use std::sync::Arc;


use aidog_middleware::MiddlewareEngine;
use gateway::models::{
    CreateMiddlewareRule, MiddlewareRule, MiddlewareSettings, UpdateMiddlewareRule,
};

crate::tauri_command! {
pub async fn middleware_list_rules(db: State<'_, Db>) -> Result<Vec<MiddlewareRule>, String> {
    aidog_db::list_middleware_rules(&db).await
}
}

crate::tauri_command! {
pub async fn middleware_create_rule(
    input: CreateMiddlewareRule,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<MiddlewareRule, String> {
    let rule = aidog_db::create_middleware_rule(&db, input).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_create_rule", error = %e, "engine reload failed");
    }
    Ok(rule)
}
}

crate::tauri_command! {
pub async fn middleware_update_rule(
    input: UpdateMiddlewareRule,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<MiddlewareRule, String> {
    let rule = aidog_db::update_middleware_rule(&db, input).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_update_rule", error = %e, "engine reload failed");
    }
    Ok(rule)
}
}

crate::tauri_command! {
pub async fn middleware_delete_rule(
    id: i64,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<(), String> {
    tracing::debug!(command = "middleware_delete_rule", id, "command invoked");
    aidog_db::delete_middleware_rule(&db, id).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_delete_rule", error = %e, "engine reload failed");
    }
    Ok(())
}
}

crate::tauri_command! {
pub async fn middleware_settings_get(db: State<'_, Db>) -> Result<MiddlewareSettings, String> {
    Ok(aidog_db::get_setting(&db, "middleware", "settings").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}
}

crate::tauri_command! {
pub async fn middleware_settings_set(
    db: State<'_, Db>,
    settings: MiddlewareSettings,
) -> Result<(), String> {
    aidog_db::set_setting(&db, SetSettingInput {
        scope: "middleware".to_string(),
        key: "settings".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize middleware settings: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "middleware_settings_set", error = %e, "persist middleware settings failed"); e })
}
}

#[cfg(test)]
#[path = "test_middleware.rs"]
mod test_middleware;
