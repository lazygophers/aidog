use crate::gateway;
use aidog_db::{self as db, Db};
use std::sync::Arc;

use gateway::quota::PlatformQuota;

crate::tauri_command! {
pub async fn platform_query_quota(
    base_url: String, api_key: String,
    platform_id: Option<u64>) -> Result<PlatformQuota, String> {
    quota_command_adapter(
        aidog_ctx::db(),
        "platform_query_quota",
        &base_url,
        &api_key,
        QuotaQueryKind::Auto,
        platform_id,
        true,
    )
    .await
}
}

crate::tauri_command! {
/// New API 专用余额查询（两步：先查 token quota 类型，再按需查用户余额）
pub async fn platform_query_quota_newapi(
    base_url: String, api_key: String, extra: String,
    platform_id: Option<u64>) -> Result<PlatformQuota, String> {
    quota_command_adapter(
        aidog_ctx::db(),
        "platform_query_quota_newapi",
        &base_url,
        &api_key,
        QuotaQueryKind::NewApi(&extra),
        platform_id,
        false,
    )
    .await
}
}

crate::tauri_command! {
/// Devin 用量查询（ACU 数，est_cost 记 ACU 不折 $）。
/// extra 需含 `{"devin":{"org_id":"<id>"}}`，缺 org_id → 失败 PlatformQuota。
pub async fn platform_query_quota_devin(
    base_url: String, api_key: String, extra: String,
    platform_id: Option<u64>) -> Result<PlatformQuota, String> {
    quota_command_adapter(
        aidog_ctx::db(),
        "platform_query_quota_devin",
        &base_url,
        &api_key,
        QuotaQueryKind::Devin(&extra),
        platform_id,
        false,
    )
    .await
}
}

/// 查询分派：三个命令各调不同 gateway 入口。不用闭包注入——`FnOnce(&Arc<Db>) -> Fut`
/// 返回的 Future 借用闭包入参，生命周期过不了编译；枚举静态分派无此问题。
enum QuotaQueryKind<'a> {
    /// base_url 启发式 + 行协议脚本路由（`query_quota`）
    Auto,
    /// New API 两步查询，携带 extra
    NewApi(&'a str),
    /// Devin ACU 查询，携带 extra（需含 org_id）
    Devin(&'a str),
}

/// 三个 quota 查询命令共用的薄 adapter（票 architecture-deepening #10）：
/// 取 db → debug 日志（api_key 恒 `[REDACTED]`）→ 分派查询 → info 结果日志 →
/// 成功才 persist → 原样返回 `Ok(q)`。日志/persist/返回路径只此一份。
///
/// `tiers_in_result_log`：`platform_query_quota` 的 info 日志带 `tiers` 不带 `command`，
/// newapi / devin 带 `command` 不带 `tiers`——收敛前即如此，原样保留不归一。
async fn quota_command_adapter(
    db: &Db,
    command: &'static str,
    base_url: &str,
    api_key: &str,
    kind: QuotaQueryKind<'_>,
    platform_id: Option<u64>,
    tiers_in_result_log: bool,
) -> Result<PlatformQuota, String> {
    tracing::debug!(command, platform_id = ?platform_id, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let db_arc = Arc::new(db.clone());
    let pid = platform_id.unwrap_or(0) as i64;
    let q = match kind {
        QuotaQueryKind::Auto => {
            gateway::quota::query_quota(Some(&db_arc), base_url, api_key, pid).await
        }
        QuotaQueryKind::NewApi(extra) => {
            gateway::quota::query_quota_newapi(Some(&db_arc), base_url, api_key, extra, pid).await
        }
        QuotaQueryKind::Devin(extra) => {
            gateway::quota::query_quota_devin(Some(&db_arc), base_url, api_key, extra, pid).await
        }
    };
    if tiers_in_result_log {
        tracing::info!(platform_id = ?platform_id, success = q.success, tiers = ?q.coding_plan.as_ref().map(|c| c.tiers.len()), "quota query result");
    } else {
        tracing::info!(command, platform_id = ?platform_id, success = q.success, "quota query result");
    }
    if q.success {
        persist_quota_to_db(db, platform_id, &q).await;
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
    // 无查询脚本但 registry 带 plan_quotas 的平台（spec B2，bailian_coding）：
    // 播种静态锚点（幂等，仅 est 为空时写），纯本地 per-request 预估，无需真查。
    for p in list.iter().filter(|p| p.enabled && !has_quota_script(p)) {
        let _ = gateway::estimate::seed_plan_anchor_if_empty(
            db_state,
            p.id,
            &p.platform_type.wire_str(),
        )
        .await;
    }
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
    // manual 配额方式（quota-ia spec §2）：脚本侧整体不生效，视同无脚本——
    // 刷新循环跳过、种子锚点照播（plan_quotas 与 source 无关）。
    !db::is_manual_quota_source(&p.quota_source)
        && db::registry::resolve_quota_script(
            &p.platform_type.wire_str(),
            &p.extra,
            &p.quota_script,
        )
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn adapter_preserves_query_failures_for_all_three_commands() {
        let db = Db::new(":memory:").await.unwrap();
        for (command, kind, tiers) in [
            ("platform_query_quota", QuotaQueryKind::Auto, true),
            (
                "platform_query_quota_newapi",
                QuotaQueryKind::NewApi("{}"),
                false,
            ),
            (
                "platform_query_quota_devin",
                QuotaQueryKind::Devin("{}"),
                false,
            ),
        ] {
            // Invalid URL prevents network IO even if a script tries to send a request.
            let q = quota_command_adapter(&db, command, "://invalid", "", kind, None, tiers)
                .await
                .expect("query failures remain Ok(PlatformQuota)");
            assert!(!q.success, "{command}");
            assert!(q.error.is_some(), "{command}");
            assert!(q.balance.is_none(), "{command}");
            assert!(q.coding_plan.is_none(), "{command}");
        }
    }
}
