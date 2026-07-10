use aidog_core::shared::*;
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
use tauri::Emitter;


pub(crate) async fn create_auto_group_for(db: &Db, platform: &Platform, level_priority: Option<i32>) -> Result<(), String> {
    let group_key = slugify(&format!("{}-auto", platform.name));
    let group = db::create_group(db, CreateGroup {
        name: group_key.clone(),
        group_key: Some(group_key),
        routing_mode: RoutingMode::HealthAware,
        auto_from_platform: platform.id.to_string(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
        source_protocol: None,
        max_retries: 10,
        model_mappings: Vec::new(),
        env_vars: Vec::new(),
    }).await?;
    db::set_group_platforms(db, group.id, &[GroupPlatformInput {
        platform_id: platform.id,
        priority: Some(0),
        weight: Some(1),
        level_priority,
    }]).await?;
    tracing::info!(platform_id = platform.id, group_id = group.id, "created auto group for platform");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_create(input: CreatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    tracing::debug!(command = "platform_create", name = %input.name, "command invoked");
    // 分组选项先捕获（input 随即 move 进 create_platform）。
    let auto_group = input.auto_group.unwrap_or(true);
    let join_group_ids = input.join_group_ids.clone().unwrap_or_default();
    let default_level_priority = input.default_level_priority;
    let platform = db::create_platform(&db, input).await
        .map_err(|e| { tracing::error!(command = "platform_create", error = %e, "create platform failed"); e })?;

    // ① 创建默认分组（用户勾选；默认勾 = 旧行为）。
    if auto_group {
        if let Err(e) = create_auto_group_for(&db, &platform, default_level_priority).await {
            tracing::error!(command = "platform_create", platform_id = platform.id, error = %e, "auto-create group failed");
            return Err(e);
        }
    }

    // ② 加入用户指定的已有分组（plain membership；sync 跳过 auto 组，对新平台即纯追加）。
    if !join_group_ids.is_empty() {
        if let Err(e) = db::sync_platform_manual_groups(&db, platform.id, &join_group_ids).await {
            tracing::warn!(command = "platform_create", platform_id = platform.id, error = %e, "join groups failed");
        }
    }

    Ok(platform)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_list(db: State<'_, Db>) -> Result<Vec<Platform>, String> {
    tracing::debug!(command = "platform_list", "command invoked");
    let mut platforms = db::list_platforms(&db).await?;
    // 列表页余额按使用速率配色：per-platform 动态窗口日速率 → days_remaining → balance_level。
    // 阈值走 usage_color::balance_level（唯一源，不漂移）；无用量数据 → neutral（前端退中性）。
    for p in platforms.iter_mut() {
        // 余额 = max(est_balance_remaining, manual "total" 预算剩余)，与 group-info 一致。
        let manual_total_remaining: f64 = p
            .manual_budgets
            .iter()
            .filter(|b| b.enabled && b.kind == "total")
            .map(gateway::manual_budget::remaining)
            .sum();
        let balance = p.est_balance_remaining.max(manual_total_remaining);
        let days_remaining = match db::get_platform_hourly_rate(&db, p.id).await {
            Ok(Some(rate)) if rate > 0.0 && balance > 0.0 => Some((balance / rate) / 24.0),
            _ => None,
        };
        p.balance_level = gateway::usage_color::balance_level(days_remaining).as_str().to_string();
    }
    Ok(platforms)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_get(id: u64, db: State<'_, Db>) -> Result<Option<Platform>, String> {
    tracing::debug!(command = "platform_get", id, "command invoked");
    db::get_platform(&db, id).await
}

/// 单平台可分享配置：剥离 DB 内部 / 运行时字段（id / status / 统计 / 时间戳等），
/// 仅保留可重新导入的配置字段（含明文 api_key）。
/// 顶层 `aidog_platform_share: 1` 作为格式标识，接收端据此校验是否为合法分享串。
/// 含明文 api_key —— 平台分享本质 = 把可用配置给可信对象，由用户显式主动触发。
/// 前端按所选格式（YAML / JSON / Base64）序列化此结构化对象。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SharePlatform {
    /// 格式标识：恒为 1。接收端校验此字段存在（>0）才视为合法分享串。
    pub aidog_platform_share: u32,
    pub name: String,
    pub platform_type: Protocol,
    pub base_url: String,
    pub api_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub extra: String,
    #[serde(default, skip_serializing_if = "PlatformModels::is_empty")]
    pub models: PlatformModels,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_models: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<PlatformEndpoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manual_budgets: Vec<ManualBudget>,
}

/// 导出单平台的可分享数据对象（结构化对象）。
/// 后端只返回干净的数据对象，格式转换（YAML / JSON / Base64）由前端负责，
/// 避免后端把 YAML 引入序列化主路径。本地操作，不落 proxy_log。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_share_export(platform_id: u64, db: State<'_, Db>) -> Result<SharePlatform, String> {
    tracing::debug!(command = "platform_share_export", platform_id, "command invoked");
    let p = match db::get_platform(&db, platform_id).await? {
        Some(p) => p,
        None => return Err(format!("platform {platform_id} not found")),
    };
    Ok(SharePlatform {
        aidog_platform_share: 1,
        name: p.name,
        platform_type: p.platform_type,
        base_url: p.base_url,
        api_key: p.api_key,
        extra: p.extra,
        models: p.models,
        available_models: p.available_models,
        endpoints: p.endpoints,
        manual_budgets: p.manual_budgets,
    })
}

/// 解析分享串（serde_yml 是 YAML 超集，可同时解析 YAML / JSON）。
/// 校验顶层 `aidog_platform_share` 标识存在（>0）；不含则返错，
/// 接收端据此 fallback 到原杂乱文本解析（无回归）。本地操作，不落 proxy_log。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_share_parse(text: String) -> Result<SharePlatform, String> {
    tracing::debug!(command = "platform_share_parse", "command invoked");
    let parsed: SharePlatform = serde_yml::from_str(&text)
        .map_err(|e| format!("not a valid aidog platform share: {e}"))?;
    if parsed.aidog_platform_share == 0 {
        return Err("missing aidog_platform_share marker".to_string());
    }
    Ok(parsed)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_update(input: UpdatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    tracing::debug!(command = "platform_update", id = input.id, "command invoked");
    // 分组选项先捕获（input 随即 move 进 update_platform）。
    let join_group_ids = input.join_group_ids.clone();
    let platform = db::update_platform(&db, input).await
        .map_err(|e| { tracing::error!(command = "platform_update", error = %e, "update platform failed"); e })?;

    // 自动建默认分组是「创建时一次性判断」（见 platform_create），编辑平台不再触发建组/拆组对账。

    // join_group_ids：全量同步手动组成员关系（auto 组不动；None=不改）。
    if let Some(ids) = join_group_ids {
        if let Err(e) = db::sync_platform_manual_groups(&db, platform.id, &ids).await {
            tracing::warn!(command = "platform_update", platform_id = platform.id, error = %e, "sync manual groups failed");
        }
    }

    Ok(platform)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_delete(id: u64, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "platform_delete", id, "command invoked");
    db::delete_platform(&db, id).await
        .map_err(|e| { tracing::error!(command = "platform_delete", id, error = %e, "delete platform failed"); e })
}

/// 一键清理失效（auto_disabled）平台。
/// - `group_id = null`：全局，删全库 auto_disabled 平台。
/// - `group_id = <gid>`：分组级，独占本分组的永久删除，共享（属多分组）的仅从本分组移除关联。
/// 返回 { deletedIds, unassignedIds }。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_purge_disabled(
    group_id: Option<u64>,
    db: State<'_, Db>,
) -> Result<db::PurgeResult, String> {
    tracing::debug!(command = "platform_purge_disabled", ?group_id, "command invoked");
    db::purge_auto_disabled_platforms(&db, group_id).await.map_err(|e| {
        tracing::error!(command = "platform_purge_disabled", ?group_id, error = %e, "purge disabled platforms failed");
        e
    })
}

/// 为平台补建默认 auto 分组（已存在则跳过）。供批量导入（cc-switch / .aidogx）回挂复用：
/// 这些路径直接 INSERT 平台行、不走 platform_create 的建组副作用，故需显式补建。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_ensure_auto_group(id: u64, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "platform_ensure_auto_group", id, "command invoked");
    let platform = match db::get_platform(&db, id).await? {
        Some(p) => p,
        None => return Err(format!("platform {id} not found")),
    };
    // 已有关联 auto 分组 → 跳过（幂等）。
    let groups = db::list_groups(&db).await.unwrap_or_default();
    let platform_id_str = platform.id.to_string();
    if groups.iter().any(|g| g.auto_from_platform == platform_id_str) {
        return Ok(());
    }
    create_auto_group_for(&db, &platform, None).await
}

/// 设置 / 清除托盘展示平台（互斥单平台）。
/// enabled=true → 设 platform_id 为唯一展示平台（tray_display: "balance"|"coding"）；
/// enabled=false → 清空所有。改后刷新托盘。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_set_tray(
    platform_id: u64,
    tray_display: String,
    enabled: bool,
    db: State<'_, Db>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::debug!(command = "platform_set_tray", platform_id, tray_display = %tray_display, enabled, "command invoked");
    if enabled {
        db::set_tray_platform(&db, platform_id, &tray_display).await
            .map_err(|e| { tracing::error!(command = "platform_set_tray", platform_id, error = %e, "set_tray_platform failed"); e })?;
    } else {
        db::clear_tray(&db).await
            .map_err(|e| { tracing::error!(command = "platform_set_tray", error = %e, "clear_tray failed"); e })?;
    }
    // C8 cmd-tray：tray.rs 迁 commands_tray 后，跨 crate 边禁直调 refresh_tray_menu。
    // 改 emit "tray-refresh" event，复用 app_setup.rs 现有 listener（C4 cmd-proxy 模式）。
    let _ = app.emit("tray-refresh", ());
    Ok(())
}

/// 读取托盘配置。无配置时（首次/升级）从旧 show_in_tray 平台迁移生成默认。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn tray_config_get(db: State<'_, Db>) -> Result<TrayConfig, String> {
    tracing::debug!(command = "tray_config_get", "command invoked");
    Ok(db::get_tray_config(&db).await?.unwrap_or_default())
}

/// 保存托盘配置并刷新托盘渲染。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn tray_config_set(
    config: TrayConfig,
    db: State<'_, Db>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::debug!(command = "tray_config_set", "command invoked");
    db::set_tray_config(&db, &config).await
        .map_err(|e| { tracing::error!(command = "tray_config_set", error = %e, "set_tray_config failed"); e })?;
    // C8 cmd-tray：tray.rs 迁 commands_tray 后，跨 crate 边禁直调 refresh_tray_menu。
    // 改 emit "tray-refresh" event，复用 app_setup.rs 现有 listener（C4 cmd-proxy 模式）。
    let _ = app.emit("tray-refresh", ());
    Ok(())
}

/// 获取今日统计摘要（供前端预览使用）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn tray_today_stats(db: State<'_, Db>) -> Result<db::TodayStats, String> {
    tracing::debug!(command = "tray_today_stats", "command invoked");
    db::today_stats(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_reorder(ordered_ids: Vec<u64>, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "platform_reorder", count = ordered_ids.len(), "command invoked");
    db::reorder_platforms(&db, &ordered_ids).await
        .map_err(|e| { tracing::error!(command = "platform_reorder", error = %e, "reorder platforms failed"); e })
}

#[cfg(test)]
#[path = "test_platform.rs"]
mod test_platform;
