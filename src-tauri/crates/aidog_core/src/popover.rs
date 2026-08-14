//! Popover 弹窗 command（C3 c3-commands 第 1 批：原 commands_tray::popover 迁入，纯搬运）。

use crate::shared::*;
use crate::tray_render::tray_layout_with_stats;
use crate::gateway::{self, db::{self, Db}};
use gateway::models::*;
use tauri::{State, Manager};

#[derive(serde::Serialize)]
pub struct PopoverEntry {
    name: String,
    value: String,
    color: TrayColor,
}

/// Popover 弹窗全部数据：配置（驱动渲染）+ 所含 item type 对应数据。
///
/// 内容完全由 `config.items` 的 order + visible 驱动；后端按所含 type 一次性返回所有可能用到的数据
/// （tray 列 / 今日统计 / 各平台当日 / 代理状态），前端按配置顺序裁剪展示。
#[derive(serde::Serialize)]
pub struct PopoverData {
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

crate::tauri_command! {
    pub async fn popover_data(db: State<'_, Db>, app: tauri::AppHandle) -> Result<PopoverData, String> {
        // today_stats 先取（tray_layout 若含 today_usage item 复用同一份，消重复聚合），
        // 其余 4 个无依赖 await 并发（config / layout / platform_today / proxy settings）。
        let today_stats = db::today_stats(&db).await?;
        let (config, layout, platform_today, settings) = tokio::join!(
            db::get_popover_config(&db),
            tray_layout_with_stats(&app, Some(&today_stats)),
            db::today_platform_stats(&db),
            load_proxy_settings(&app),
        );
        let config = config?;
        let platform_today = platform_today?;
        let settings = settings.unwrap_or(ProxySettings {
            port: 9890, autostart: false, silent_launch: false, bind_lan: true,
        });
        let entries: Vec<PopoverEntry> = layout.columns.into_iter().map(|c| PopoverEntry {
            name: c.name,
            value: c.value,
            color: c.color,
        }).collect();
        let proxy_running = {
            let handle = app.try_state::<ProxyHandle>();
            handle.map(|h| h.0.lock().map(|g| g.is_some()).unwrap_or(false)).unwrap_or(false)
        };
        Ok(PopoverData {
            config,
            entries,
            today_stats,
            platform_today,
            proxy_running,
            proxy_port: settings.port,
        })
    }
}

crate::tauri_command! {
    /// 读取 PopoverConfig（无配置 → 默认配置）。
    pub async fn popover_config_get(db: State<'_, Db>) -> Result<gateway::models::PopoverConfig, String> {
        db::get_popover_config(&db).await
    }
}

crate::tauri_command! {
    /// 保存 PopoverConfig。
    pub async fn popover_config_set(
        config: gateway::models::PopoverConfig,
        db: State<'_, Db>,
    ) -> Result<(), String> {
        db::set_popover_config(&db, &config).await
    }
}

crate::tauri_command! {
    /// 各平台当日使用（供设置页预览）。
    pub async fn popover_platform_today(db: State<'_, Db>) -> Result<Vec<db::TodayPlatformStat>, String> {
        db::today_platform_stats(&db).await
    }
}

#[cfg(test)]
mod test_popover {
    use super::*;
    use crate::gateway::db::test_support::test_db;

    /// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
    /// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 转发的 db:: 函数
    /// （command 本身只是薄转发 + tracing，逻辑等价）。
    #[tokio::test]
    async fn config_roundtrip_and_today() {
        let db = test_db().await;
        let cfg = db::get_popover_config(&db).await.unwrap();
        db::set_popover_config(&db, &cfg).await.unwrap();
        let _ = db::today_platform_stats(&db).await.unwrap();
    }
}
