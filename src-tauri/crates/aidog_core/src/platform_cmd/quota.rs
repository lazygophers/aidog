use crate::gateway;
use aidog_db::{self as db, Db};
use std::sync::Arc;

use gateway::quota::PlatformQuota;

crate::tauri_command! {
pub async fn platform_query_quota(
    base_url: String, api_key: String,
    platform_id: Option<u64>) -> Result<PlatformQuota, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "platform_query_quota", platform_id = ?platform_id, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let q = gateway::quota::query_quota(Some(&Arc::new(db.clone())), &base_url, &api_key, platform_id.unwrap_or(0) as i64).await;
    tracing::info!(platform_id = ?platform_id, success = q.success, tiers = ?q.coding_plan.as_ref().map(|c| c.tiers.len()), "quota query result");
    if q.success {
        persist_quota_to_db(db, platform_id, &q).await;
    }
    Ok(q)
}
}

crate::tauri_command! {
/// New API 专用余额查询（两步：先查 token quota 类型，再按需查用户余额）
pub async fn platform_query_quota_newapi(
    base_url: String, api_key: String, extra: String,
    platform_id: Option<u64>) -> Result<PlatformQuota, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "platform_query_quota_newapi", platform_id = ?platform_id, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let q = gateway::quota::query_quota_newapi(Some(&Arc::new(db.clone())), &base_url, &api_key, &extra, platform_id.unwrap_or(0) as i64).await;
    tracing::info!(command = "platform_query_quota_newapi", platform_id = ?platform_id, success = q.success, "quota query result");
    if q.success {
        persist_quota_to_db(db, platform_id, &q).await;
    }
    Ok(q)
}
}

crate::tauri_command! {
/// Devin 用量查询（ACU 数，est_cost 记 ACU 不折 $）。
/// extra 需含 `{"devin":{"org_id":"<id>"}}`，缺 org_id → 失败 PlatformQuota。
pub async fn platform_query_quota_devin(
    base_url: String, api_key: String, extra: String,
    platform_id: Option<u64>) -> Result<PlatformQuota, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "platform_query_quota_devin", platform_id = ?platform_id, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let q = gateway::quota::query_quota_devin(Some(&Arc::new(db.clone())), &base_url, &api_key, &extra, platform_id.unwrap_or(0) as i64).await;
    tracing::info!(command = "platform_query_quota_devin", platform_id = ?platform_id, success = q.success, "quota query result");
    if q.success {
        persist_quota_to_db(db, platform_id, &q).await;
    }
    Ok(q)
}
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

/// 冷启动同时真查的平台数上限（出站并发，避免启动瞬间 N 个 HTTP）。
const COLD_START_CONCURRENCY: usize = 4;

/// 冷启动 est 初始化：对**所有启用、带 key、且有 quota 查询脚本**的平台后台真查一次并校准。
/// 两件事一起解决：
/// ① est 冷启动即有值——不再只覆盖 tray 平台，Groups 卡片 / Home 汇总（只读 est，自身不发
///    真查）打开就有余额，不必手点刷新按钮；
/// ② 重排 coding plan 的窗口重置定时——`spawn_refresh_at` 是内存态，进程重启即丢，
///    这里真查一次由 `calibrate_from_quota` 顺带排回下一个 `resets_at`。
/// 不阻塞：分批 spawn（每批 COLD_START_CONCURRENCY 个并发），每平台失败即跳过。
/// 真查完成后发 tray-refresh，让主线程刷新托盘显示。
pub async fn cold_start_init_estimates() {
    let db_state = aidog_ctx::db();
    let Ok(list) = db::list_platforms(db_state).await else {
        return;
    };
    // 10 分钟内刚真查过的跳过：连续重启不重复轰上游（est 仍新鲜，定时由下次校准补排）。
    let fresh_before = db::now() - 10 * 60_000;
    let targets: Vec<gateway::models::Platform> = list
        .into_iter()
        .filter(|p| {
            p.enabled
                && !p.api_key.trim().is_empty()
                && p.last_real_query_at < fresh_before
                && has_quota_script(p)
        })
        .collect();
    for batch in targets.chunks(COLD_START_CONCURRENCY) {
        let tasks: Vec<_> = batch
            .iter()
            .cloned()
            .map(|p| {
                tokio::spawn(async move {
                    let db = aidog_ctx::db();
                    let db_arc = Arc::new(db.clone());
                    // 统一脚本路径（quota-scripts T4）：query_quota 按平台行协议路由（物化列 →
                    // registry 变体），newapi 两步查询 / devin ACU / 11 平台族全覆盖，
                    // 不再 newapi/devin 三分支特判。
                    let q = gateway::quota::query_quota(
                        Some(&db_arc),
                        &p.base_url,
                        &p.api_key,
                        p.id as i64,
                    )
                    .await;
                    if !q.success {
                        return; // 失败保留，下次再试（不重置 last_real_query_at）
                    }
                    let is_coding_plan = q.coding_plan.is_some();
                    gateway::estimate::calibrate_from_quota(db, p.id, &q, is_coding_plan).await;
                    aidog_ctx::emit_unit("tray-refresh");
                })
            })
            .collect();
        for t in tasks {
            let _ = t.await;
        }
    }
}

/// 该平台能否查 quota：与前端 `platformHasQuotaScript` 同口径——物化列 / 自定义脚本 /
/// 行协议的 registry 变体任一命中即可查。无脚本的平台不发无谓出站（也不落错误日志）。
fn has_quota_script(p: &gateway::models::Platform) -> bool {
    db::registry::resolve_quota_script(&p.platform_type.wire_str(), &p.extra, &p.quota_script)
        .is_some()
}
