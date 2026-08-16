use crate::gateway;
use aidog_db::Db;
use gateway::models::*;
use tauri::State;
use std::sync::Arc;


use aidog_middleware::MiddlewareEngine;
use gateway::models::{
    CreateMiddlewareRule, MiddlewareRule, MiddlewareSettings, UpdateMiddlewareRule,
};

// 复用 mitm 模块的 ImportDefaultsResult（{imported, skipped} 计数，serde camelCase → 前端对象契约）。
use super::mitm::ImportDefaultsResult;

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

crate::tauri_command! {
/// 一键导入默认（内置）中间件规则。
///
/// 用户删除内置规则后无法恢复（migration 20260727-07（原 015）seed 仅首启跑一次）。本命令复用
/// [`aidog_db::seed_builtin_middleware_rules_counted`] 幂等逻辑：按 (name, is_builtin=1)
/// 判定，已存在跳过（不重新启用用户禁用的内置规则），缺失则补入。
///
/// 返 [`ImportDefaultsResult`] `{ imported, skipped }`：前端 toast 反馈计数。
/// 写库后 reload 引擎缓存（与 create/update/delete 同模式）。
pub async fn middleware_import_default_rules(
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<ImportDefaultsResult, String> {
    let res = db
        .write_conn()
        .call(|conn| {
            let (imported, skipped) =
                aidog_db::seed_builtin_middleware_rules_counted(conn)?;
            Ok(ImportDefaultsResult {
                imported: imported as usize,
                skipped: skipped as usize,
            })
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_import_default_rules", error = %e, "engine reload failed");
    }
    Ok(res)
}
}

#[cfg(test)]
#[path = "test_middleware.rs"]
mod test_middleware;
