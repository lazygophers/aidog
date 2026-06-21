use crate::gateway::{self, db::{self, Db}};
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


use gateway::quota::PlatformQuota;

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn platform_query_quota(
    base_url: String, api_key: String,
    platform_id: Option<u64>, db: State<'_, Db>,
) -> Result<PlatformQuota, String> {
    tracing::debug!(command = "platform_query_quota", platform_id = ?platform_id, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let q = gateway::quota::query_quota(Some(&Arc::new(db.inner().clone())), &base_url, &api_key, platform_id.unwrap_or(0) as i64).await;
    tracing::info!(platform_id = ?platform_id, success = q.success, tiers = ?q.coding_plan.as_ref().map(|c| c.tiers.len()), "quota query result");
    if q.success {
        persist_quota_to_db(&db, platform_id, &q).await;
    }
    Ok(q)
}

/// New API 专用余额查询（两步：先查 token quota 类型，再按需查用户余额）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn platform_query_quota_newapi(
    base_url: String, api_key: String, extra: String,
    platform_id: Option<u64>, db: State<'_, Db>,
) -> Result<PlatformQuota, String> {
    tracing::debug!(command = "platform_query_quota_newapi", platform_id = ?platform_id, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let q = gateway::quota::query_quota_newapi(Some(&Arc::new(db.inner().clone())), &base_url, &api_key, &extra, platform_id.unwrap_or(0) as i64).await;
    tracing::info!(command = "platform_query_quota_newapi", platform_id = ?platform_id, success = q.success, "quota query result");
    if q.success {
        persist_quota_to_db(&db, platform_id, &q).await;
    }
    Ok(q)
}

/// 将 quota 真查结果写回 platform 表，并作为一次「校准」严格对齐 est = 真实。
/// 走 estimate::calibrate_from_quota：est_coding_plan 写入正确的 EstCodingPlan 形态
/// （est_utilization=真实 util、util_at_last_real=真实、tokens_since_real=0、拟合 coef），
/// 并重置 last_real_query_at + estimate_count。
/// 这修复了旧路径直写 raw CodingPlanInfo JSON（字段 utilization≠est_utilization）→ tray est 显 0/偏差大的根因，
/// 同时保证「真查发生时 est 立即对齐真实」。
pub(crate) async fn persist_quota_to_db(db: &Db, platform_id: Option<u64>, q: &PlatformQuota) {
    let Some(pid) = platform_id else { return };
    let is_coding_plan = q.coding_plan.is_some();
    gateway::estimate::calibrate_from_quota(db, pid, q, is_coding_plan).await;
}

/// 冷启动 est 初始化：对 tray 中启用、且从未真查过（last_real_query_at==0）的平台，
/// 后台触发一次真查并校准对齐 est=真实。避免冷启动 tray 显 0/旧偏差大。
/// 不阻塞：每平台 spawn 独立 async（锁外 await 真查，calibrate_from_quota 短持锁写）。
/// 真查完成后发 tray-refresh，让主线程刷新托盘显示。
pub(crate) async fn cold_start_init_tray_estimates(app: &tauri::AppHandle) {
    let Some(db_state) = app.try_state::<Db>() else { return };
    let Ok(Some(config)) = db::get_tray_config(&db_state).await else { return };
    // 收集 tray 启用、platform 类型、且 last_real_query_at==0 的平台
    let mut targets: Vec<gateway::models::Platform> = Vec::new();
    for item in config.items.iter().filter(|i| i.enabled && i.item_type == "platform") {
        let Some(pid) = item.platform_id else { continue };
        if let Ok(Some(p)) = db::get_platform(&db_state, pid).await {
            if p.last_real_query_at == 0 {
                targets.push(p);
            }
        }
    }
    for p in targets {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let Some(db) = handle.try_state::<Db>() else { return };
            let db_arc = Arc::new(db.inner().clone());
            let is_newapi = matches!(p.platform_type, gateway::models::Protocol::NewApi);
            // 锁外 async 真查
            let q = if is_newapi {
                gateway::quota::query_quota_newapi(Some(&db_arc), &p.base_url, &p.api_key, &p.extra, p.id as i64).await
            } else {
                gateway::quota::query_quota(Some(&db_arc), &p.base_url, &p.api_key, p.id as i64).await
            };
            if !q.success {
                return; // 失败保留，下次再试（不重置 last_real_query_at）
            }
            let is_coding_plan = q.coding_plan.is_some();
            gateway::estimate::calibrate_from_quota(&db, p.id, &q, is_coding_plan).await;
            use tauri::Emitter;
            let _ = handle.emit("tray-refresh", ());
        });
    }
}
