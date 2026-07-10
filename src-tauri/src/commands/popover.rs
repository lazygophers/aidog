use aidog_core::shared::*;
use crate::commands::tray::tray_layout;
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


#[derive(serde::Serialize)]
pub(crate) struct PopoverEntry {
    name: String,
    value: String,
    color: TrayColor,
}

/// Popover 弹窗全部数据：配置（驱动渲染）+ 所含 item type 对应数据。
///
/// 内容完全由 `config.items` 的 order + visible 驱动；后端按所含 type 一次性返回所有可能用到的数据
/// （tray 列 / 今日统计 / 各平台当日 / 代理状态），前端按配置顺序裁剪展示。
#[derive(serde::Serialize)]
pub(crate) struct PopoverData {
    /// 配置本身（前端据此排序 + 显隐渲染）。
    config: gateway::models::PopoverConfig,
    /// 平台余额 / coding 列（来自 tray 配置，对应 item type "platform_balance"）。
    entries: Vec<PopoverEntry>,
    /// 今日全局统计（金额 / 缓存率 / token / 请求数）。
    today_stats: db::TodayStats,
    /// 各平台当日使用（只含已用），对应 item type "platform_today"。
    platform_today: Vec<db::TodayPlatformStat>,
    proxy_running: bool,
    proxy_port: u16,
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn popover_data(db: State<'_, Db>, app: tauri::AppHandle) -> Result<PopoverData, String> {
    tracing::debug!(command = "popover_data", "command invoked");
    let config = db::get_popover_config(&db).await?;
    let layout = tray_layout(&app).await;
    let entries: Vec<PopoverEntry> = layout.columns.into_iter().map(|c| PopoverEntry {
        name: c.name,
        value: c.value,
        color: c.color,
    }).collect();
    let today_stats = db::today_stats(&db).await?;
    let platform_today = db::today_platform_stats(&db).await?;
    let proxy_running = {
        let handle = app.try_state::<ProxyHandle>();
        handle.map(|h| h.0.lock().map(|g| g.is_some()).unwrap_or(false)).unwrap_or(false)
    };
    let settings = load_proxy_settings(&app).await.unwrap_or(ProxySettings {
        port: 9876, autostart: false, silent_launch: false, bind_lan: true,
    });
    Ok(PopoverData {
        config,
        entries,
        today_stats,
        platform_today,
        proxy_running,
        proxy_port: settings.port,
    })
}

/// 读取 PopoverConfig（无配置 → 默认配置）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn popover_config_get(db: State<'_, Db>) -> Result<gateway::models::PopoverConfig, String> {
    tracing::debug!(command = "popover_config_get", "command invoked");
    db::get_popover_config(&db).await
}

/// 保存 PopoverConfig。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn popover_config_set(
    config: gateway::models::PopoverConfig,
    db: State<'_, Db>,
) -> Result<(), String> {
    tracing::debug!(command = "popover_config_set", "command invoked");
    db::set_popover_config(&db, &config).await
        .map_err(|e| { tracing::error!(command = "popover_config_set", error = %e, "set_popover_config failed"); e })
}

/// 各平台当日使用（供设置页预览）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn popover_platform_today(db: State<'_, Db>) -> Result<Vec<db::TodayPlatformStat>, String> {
    tracing::debug!(command = "popover_platform_today", "command invoked");
    db::today_platform_stats(&db).await
}

#[cfg(test)]
#[path = "test_popover.rs"]
mod test_popover;
