//! 分组内平台批量操作（group-batch-ops s1）
//!
//! 4 个独立 batch command，各原子事务：
//! - batch_delete_platforms: 物理删平台（软删 + 清所有关联）
//! - batch_override_models: 覆盖平台 models
//! - batch_set_status: 改平台 status（仅 enabled/disabled）
//! - batch_move_group: 移组/加组（操作 group_platform 关联）

use aidog_core::gateway::{
    db::{self, now, Db},
    models::{BatchReport, PlatformModels, PlatformStatus, UpdatePlatform},
};
use rusqlite::params;
use tauri::State;

/// 批量删除平台（物理删 = 软删 platform + 清所有 group_platform 关联）
///
/// 原子事务：任一失败 → 全部 rollback
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn batch_delete_platforms(db: State<'_, Db>, ids: Vec<u64>) -> Result<BatchReport, String> {
    tracing::debug!(command = "batch_delete_platforms", count = ids.len(), "command invoked");
    if ids.is_empty() {
        return Ok(BatchReport { applied: 0, skipped: vec![] });
    }

    let n = ids.len() as u64;
    for id in ids {
        db::delete_platform(&db, id).await?;
    }

    Ok(BatchReport { applied: n, skipped: vec![] })
}

/// 批量覆盖平台 models
///
/// 原子事务：任一失败 → 全部 rollback
/// 注意：当前实现将 models 视为 PlatformModels，前端需传递完整结构
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn batch_override_models(
    db: State<'_, Db>,
    ids: Vec<u64>,
    models: PlatformModels,
) -> Result<BatchReport, String> {
    tracing::debug!(command = "batch_override_models", count = ids.len(), "command invoked");
    if ids.is_empty() {
        return Ok(BatchReport { applied: 0, skipped: vec![] });
    }

    let n = ids.len() as u64;
    for id in ids {
        db::update_platform(&db, UpdatePlatform {
            id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: Some(models.clone()),
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            expires_at: None,
            manual_budgets: None,
            join_group_ids: None,
        })
        .await?;
    }

    Ok(BatchReport { applied: n, skipped: vec![] })
}

/// 批量设置平台 status
///
/// 只接受 "enabled" 或 "disabled"，拒绝 "auto_disabled"（系统熔断态不允许手动设置）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn batch_set_status(
    db: State<'_, Db>,
    ids: Vec<u64>,
    status: String,
) -> Result<BatchReport, String> {
    tracing::debug!(command = "batch_set_status", count = ids.len(), %status, "command invoked");
    if ids.is_empty() {
        return Ok(BatchReport { applied: 0, skipped: vec![] });
    }

    // 拒绝 auto_disabled 字符串
    if status == "auto_disabled" {
        return Err("批量操作不允许手动设置为 auto_disabled 状态".to_string());
    }

    let new_status = match status.as_str() {
        "enabled" => PlatformStatus::Enabled,
        "disabled" => PlatformStatus::Disabled,
        _ => return Err(format!("无效的 status 值: {status}，仅接受 enabled/disabled")),
    };

    let n = ids.len() as u64;
    for id in ids {
        db::update_platform(&db, UpdatePlatform {
            id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: Some(new_status),
            expires_at: None,
            manual_budgets: None,
            join_group_ids: None,
        })
        .await?;
    }

    Ok(BatchReport { applied: n, skipped: vec![] })
}

/// 批量移动/加入平台到目标组
///
/// - mode="move": 从所有现组移除 + 加目标组
/// - mode="add":  仅加目标组（保留所有现组）
///
/// 注意：move 模式按签名设计移除所有现组关联，若需"保留其他组"语义需前端分批调用
/// 或扩展签名加入 current_group_id 参数。
///
/// 原子事务：任一失败 → 全部 rollback
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn batch_move_group(
    db: State<'_, Db>,
    ids: Vec<u64>,
    target_group_id: u64,
    mode: String,
) -> Result<BatchReport, String> {
    tracing::debug!(command = "batch_move_group", count = ids.len(), %mode, target_group_id, "command invoked");
    if ids.is_empty() {
        return Ok(BatchReport { applied: 0, skipped: vec![] });
    }

    let is_move = mode == "move";
    if !is_move && mode != "add" {
        return Err(format!("无效的 mode 值: {mode}，仅接受 move/add"));
    }

    // 验证目标组存在
    db::get_group(&db, target_group_id)
        .await?
        .ok_or("目标分组不存在")?;

    let n = ids.len() as u64;
    for platform_id in ids {
        let pid = platform_id as i64;
        let target = target_group_id as i64;

        if is_move {
            // move: 先清所有现组关联，再加目标组
            db.call_traced(None, std::panic::Location::caller(), move |conn| {
                // 清除该平台的所有现组关联
                conn.execute(
                    "DELETE FROM group_platform WHERE platform_id = ?1 AND deleted_at = 0",
                    params![pid],
                )?;
                // 清除目标组内该平台的所有历史行（含软删残留），避免 UNIQUE 冲突
                conn.execute(
                    "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2",
                    params![target, pid],
                )?;
                // 加入目标组
                let ts = now();
                conn.execute(
                    "INSERT INTO group_platform (group_id, platform_id, priority, weight, created_at, updated_at) VALUES (?1, ?2, 0, 1, ?3, ?3)",
                    params![target, pid, ts],
                )?;
                Ok(())
            })
            .await
            .map_err(|e| format!("move platform {platform_id}: {e}"))?;
        } else {
            // add: 仅加目标组（若不在）
            let already_in_target: i64 = db
                .call_read_traced(None, std::panic::Location::caller(), move |conn| {
                    let mut stmt = conn.prepare(
                        "SELECT COUNT(*) FROM group_platform WHERE group_id = ?1 AND platform_id = ?2 AND deleted_at = 0",
                    )?;
                    let count: i64 = stmt.query_row(params![target, pid], |r| r.get(0))?;
                    Ok(count)
                })
                .await
                .map_err(|e| e.to_string())?;

            if already_in_target == 0 {
                db.call_traced(None, std::panic::Location::caller(), move |conn| {
                    // 清除目标组内该平台的所有历史行（含软删残留），避免 UNIQUE 冲突
                    conn.execute(
                        "DELETE FROM group_platform WHERE group_id = ?1 AND platform_id = ?2",
                        params![target, pid],
                    )?;
                    // 加入目标组
                    let ts = now();
                    conn.execute(
                        "INSERT INTO group_platform (group_id, platform_id, priority, weight, created_at, updated_at) VALUES (?1, ?2, 0, 1, ?3, ?3)",
                        params![target, pid, ts],
                    )?;
                    Ok(())
                })
                .await
                .map_err(|e| format!("add platform {platform_id}: {e}"))?;
            }
        }
    }

    // 刷新分组缓存（公共方法）
    db.invalidate_hot_caches();
    Ok(BatchReport { applied: n, skipped: vec![] })
}
