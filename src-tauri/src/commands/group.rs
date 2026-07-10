use aidog_core::shared::*;
use aidog_core::sync_settings::{try_sync_settings, do_sync_group_settings};
use aidog_core::gateway::{self, db::{self, Db}};
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


#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_create(input: CreateGroup, db: State<'_, Db>, app: tauri::AppHandle) -> Result<Group, String> {
    tracing::debug!(command = "group_create", name = %input.name, "command invoked");
    // group_key 校验：用户提供时只允许 [A-Za-z0-9_-] 且非空；None 则 db.rs 自动生成。
    if let Some(gk) = &input.group_key {
        let gk = gk.trim();
        if gk.is_empty() || !gk.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err("group_key 只允许英文、数字、下划线、中划线，且不能为空".into());
        }
    }
    // name 保持原样支持任意 Unicode（含中文），group_key 由 db.rs 自动生成或用户提供
    let result = db::create_group(&db, input).await
        .map_err(|e| { tracing::error!(command = "group_create", error = %e, "create group failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(result)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_list(db: State<'_, Db>) -> Result<Vec<Group>, String> {
    tracing::debug!(command = "group_list", "command invoked");
    db::list_groups(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_get(id: u64, db: State<'_, Db>) -> Result<Option<Group>, String> {
    tracing::debug!(command = "group_get", id, "command invoked");
    db::get_group(&db, id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_update(input: UpdateGroup, db: State<'_, Db>, app: tauri::AppHandle) -> Result<Group, String> {
    tracing::debug!(command = "group_update", id = input.id, "command invoked");
    // name 保持原样支持任意 Unicode（含中文），不转换
    let result = db::update_group(&db, input).await
        .map_err(|e| { tracing::error!(command = "group_update", error = %e, "update group failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(result)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_delete(id: u64, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "group_delete", id, "command invoked");
    db::delete_group(&db, id).await
        .map_err(|e| { tracing::error!(command = "group_delete", id, error = %e, "delete group failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

// ─── GroupPlatform Commands ────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_set_platforms(input: SetGroupPlatforms, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "group_set_platforms", group_id = input.group_id, count = input.platforms.len(), "command invoked");
    db::set_group_platforms(&db, input.group_id, &input.platforms).await
        .map_err(|e| { tracing::error!(command = "group_set_platforms", group_id = input.group_id, error = %e, "set_group_platforms failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_get_platforms(
    group_id: u64,
    db: State<'_, Db>,
) -> Result<Vec<GroupPlatformDetail>, String> {
    tracing::debug!(command = "group_get_platforms", group_id, "command invoked");
    db::get_group_platforms(&db, group_id).await
}

// ─── Aggregate ─────────────────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_detail(id: u64, db: State<'_, Db>) -> Result<Option<GroupDetail>, String> {
    tracing::debug!(command = "group_detail", id, "command invoked");
    db::get_group_detail(&db, id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_detail_list(db: State<'_, Db>) -> Result<Vec<GroupDetail>, String> {
    tracing::debug!(command = "group_detail_list", "command invoked");
    db::list_group_details(&db).await
}

/// 分页取分组详情（前端触底加载）。offset/limit 为页窗（camelCase invoke）；
/// 越界返回空 Vec，前端据此停止加载。后端无 JOIN（单表 group_platform + 内存补 platform）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_detail_list_paged(offset: u64, limit: u64, db: State<'_, Db>) -> Result<Vec<GroupDetail>, String> {
    tracing::debug!(command = "group_detail_list_paged", offset, limit, "command invoked");
    db::list_group_details_paged(&db, offset, limit).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_reorder(ordered_ids: Vec<u64>, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "group_reorder", count = ordered_ids.len(), "command invoked");
    db::reorder_groups(&db, &ordered_ids).await
        .map_err(|e| { tracing::error!(command = "group_reorder", error = %e, "reorder groups failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_platform_reorder(
    group_id: u64,
    ordered_ids: Vec<u64>,
    db: State<'_, Db>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::debug!(command = "group_platform_reorder", group_id, count = ordered_ids.len(), "command invoked");
    db::reorder_group_platforms(&db, group_id, &ordered_ids).await
        .map_err(|e| { tracing::error!(command = "group_platform_reorder", error = %e, "reorder group platforms failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_platform_set_level_priority(
    group_id: u64,
    platform_id: u64,
    level_priority: i32,
    db: State<'_, Db>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::debug!(command = "group_platform_set_level_priority", group_id, platform_id, level_priority, "command invoked");
    db::set_group_platform_level_priority(&db, group_id, platform_id, level_priority).await
        .map_err(|e| { tracing::error!(command = "group_platform_set_level_priority", error = %e, "set level_priority failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_platform_move(
    platform_id: u64,
    from_group_id: u64,
    to_group_id: u64,
    db: State<'_, Db>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::debug!(command = "group_platform_move", platform_id, from_group_id, to_group_id, "command invoked");
    db::move_group_platform(&db, platform_id, from_group_id, to_group_id).await
        .map_err(|e| { tracing::error!(command = "group_platform_move", error = %e, "move group platform failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_set_default(
    id: Option<u64>,
    app: tauri::AppHandle,
    db: State<'_, Db>,
) -> Result<(), String> {
    tracing::debug!(command = "group_set_default", id, "command invoked");
    db::set_default_group(&db, id).await
        .map_err(|e| { tracing::error!(command = "group_set_default", id, error = %e, "set default group failed"); e })?;
    let port = load_proxy_settings(&app).await?.port;
    do_sync_group_settings(&db, port).await
        .map(|_| ())
        .map_err(|e| { tracing::error!(command = "group_set_default", error = %e, "sync after set default failed"); e })
}

#[cfg(test)]
#[path = "test_group.rs"]
mod test_group;
