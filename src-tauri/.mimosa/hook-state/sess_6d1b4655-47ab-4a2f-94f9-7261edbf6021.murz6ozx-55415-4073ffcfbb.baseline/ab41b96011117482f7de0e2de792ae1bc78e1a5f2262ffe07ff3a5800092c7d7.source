//! MITM 统计读侧命令（cc-sub-mitm 票 12）：趋势采样 / OAuth refresh 失败计数 /
//! 旁路视图 / 盲转计数 / pure cc 组 export 文案。export 非纯只读：首次调用会生成并
//! 持久化 KV 密码（scope `cc_proxy_auth`，与票 11 sync 写 settings.json 的同一份同 key
//! ——UI export 行与 settings 密码同源）；其余命令只读。UI 打开页面 / 切 tab 时拉一次，
//! 不做后台轮询。

use crate::shared::load_proxy_settings;

/// OAuth token refresh 观测统计（票 10 `oauth_token_refresh_failures` 的 command 壳）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MitmRefreshStats {
    pub attempts: i64,
    pub failures: i64,
}

crate::tauri_command! {
/// 某 group 的 5h/7d 窗口利用率采样点（蹭 `/api/oauth/usage` 自然流量入库，票 10），
/// 按 sampled_at 升序返回（趋势图直接喂 LineChart）。
pub async fn mitm_usage_trend(
    group_name: String,
    limit: Option<u32>,
) -> Result<Vec<aidog_logs::OauthUsageSampleDto>, String> {
    let db = aidog_ctx::db();
    aidog_logs::list_oauth_usage_samples(db, &group_name, limit.unwrap_or(200)).await
}
}

crate::tauri_command! {
/// platform.claude.com/v1/oauth/token 刷新观测统计（open bug #91703：refresh 过代理
/// 可能挂，UI 警示消费）。since_ms 缺省 0 = 全量。
pub async fn mitm_oauth_refresh_stats(
    since_ms: Option<i64>,
) -> Result<MitmRefreshStats, String> {
    let db = aidog_ctx::db();
    let (attempts, failures) =
        aidog_logs::oauth_token_refresh_failures(db, since_ms.unwrap_or(0)).await?;
    Ok(MitmRefreshStats { attempts, failures })
}
}

crate::tauri_command! {
/// 最近 limit 条 MITM 旁路观测行（统计页折叠入口；body 不返回）。
pub async fn mitm_bypass_list(
    limit: Option<u32>,
) -> Result<Vec<aidog_logs::MitmBypassRow>, String> {
    let db = aidog_ctx::db();
    aidog_logs::list_mitm_bypass_rows(db, limit.unwrap_or(50)).await
}
}

crate::tauri_command! {
/// 某 group 的订阅套餐档位（蹭 GET /api/oauth/profile 自然流量 upsert，balance-full；
/// 5h/7d 剩余额度走 `mitm_usage_trend` 最新采样点，此命令只回 tier）。无行 = null。
pub async fn cc_plan_info(
    group_name: String,
) -> Result<Option<aidog_logs::CcProfileDto>, String> {
    let db = aidog_ctx::db();
    aidog_logs::get_cc_oauth_profile(db, &group_name).await
}
}

crate::tauri_command! {
/// 单行 mitm_log 的 body 两列（独立观测页详情）。开关关 / token 路径时列天然为空串。
pub async fn mitm_bypass_detail(
    id: i64,
) -> Result<Option<aidog_logs::MitmBypassDetail>, String> {
    let db = aidog_ctx::db();
    aidog_logs::get_mitm_bypass_detail(db, id).await
}
}

crate::tauri_command! {
/// `blocked_reason='mitm_opaque'` 行数（统计页「未计入成本」提示）。since_ms 缺省 0 = 全量。
pub async fn mitm_opaque_count(since_ms: Option<i64>) -> Result<i64, String> {
    let db = aidog_ctx::db();
    aidog_logs::count_mitm_opaque(db, since_ms.unwrap_or(0)).await
}
}

crate::tauri_command! {
/// pure cc 组的 HTTPS_PROXY export 语句（票 11 `cc_proxy_export_line` 的 command 壳；
/// 密码与 sync 写 settings.json 的同一份 KV，端口语义同 sync）。Desktop / 多 shell
/// 场景（#96258：CC Desktop 忽略 settings env 块）用户复制到终端用。
pub async fn cc_proxy_export(group_name: String) -> Result<String, String> {
    let db = aidog_ctx::db();
    // KV key 是 group_key（自动组 gk_<hex>）而非 group name——必须先按名反查 Group，
    // 否则查出的是与 sync 注入（sync_settings.rs::cc_proxy_password）不同的两个密码。
    // CONNECT 不校验密码所以现状没炸，但 UI export 行与 settings.json 必须同源。
    let group_key = aidog_db::list_groups(db)
        .await?
        .into_iter()
        .find(|g| g.name == group_name)
        .map(|g| g.group_key)
        .ok_or_else(|| format!("group not found: {group_name}"))?;
    let port = load_proxy_settings(db).await?.port;
    let password = crate::sync_settings::cc_proxy_password(db, &group_key).await?;
    Ok(crate::sync_settings::cc_proxy_export_line(&group_name, &password, port))
}
}
