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


use tauri::menu::{MenuBuilder, MenuItemBuilder};
// TrayColumn / TrayLayout / TRAY_FONT_SIZE / TrayMenuBuild 数据类型下沉 aidog_core（C2），
// 本文件保留 build_tray_menu / tray_layout / tray_separator 等 UI 构造函数 + 实现 TrayMenuBuild
// trait 供 aidog_core::refresh_tray_menu 注入调用（防 core→commands 反向依赖循环）。
pub(crate) use aidog_core::tray_render::{TrayColumn, TrayLayout};
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
pub(crate) use aidog_core::tray_render::TRAY_FONT_SIZE;
use aidog_core::tray_render::TrayMenuBuild;
use std::future::Future;
use std::pin::Pin;

/// TrayMenuBuild 的 commands::tray 实现：把 core 的 refresh_tray_menu 调用桥接到本文件的
/// build_tray_menu / tray_layout / tray_separator。
pub(crate) struct TrayMenuBuildImpl;

impl TrayMenuBuild for TrayMenuBuildImpl {
    fn build_menu<'a>(
        &'a self,
        app: &'a tauri::AppHandle,
    ) -> Pin<Box<dyn Future<Output = Result<tauri::menu::Menu<tauri::Wry>, String>> + Send + 'a>> {
        Box::pin(build_tray_menu(app))
    }
    fn layout<'a>(
        &'a self,
        app: &'a tauri::AppHandle,
    ) -> Pin<Box<dyn Future<Output = TrayLayout> + Send + 'a>> {
        Box::pin(tray_layout(app))
    }
    fn separator<'a>(
        &'a self,
        app: &'a tauri::AppHandle,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>> {
        Box::pin(tray_separator(app))
    }
}

/// 计算单个 platform item 的（名, 值）二元组。
/// display="coding" 或平台具 coding plan → 值=`{%}%`（剩余百分比）；否则 值=`{balance:.2}`。
pub(crate) fn platform_item_parts(platform: &Platform, display: &str) -> (String, String) {
    let name = platform.name.clone();
    let plan = gateway::estimate::EstCodingPlan::from_json(&platform.est_coding_plan);
    let first_tier = plan.tiers.first();
    let is_coding = display == "coding" || first_tier.is_some();
    let value = if is_coding {
        let util = first_tier.map(|t| t.est_utilization).unwrap_or(0.0);
        format!("{:.0}%", (100.0 - util).max(0.0))
    } else {
        format!("${}", trim_trailing_zeros(&format!("{:.2}", platform.est_balance_remaining)))
    };
    (name, value)
}

/// 从托盘配置生成有序渲染布局（已按 order 排序、跳过 disabled、跳过取数失败项）。
/// separator items 不生成列，而是作为相邻数据列之间的间隙。
/// gaps[i] = columns[i] 与 columns[i+1] 之间的间隙；None = 默认空白。
pub(crate) async fn tray_layout(app: &tauri::AppHandle) -> TrayLayout {
    let empty = TrayLayout { columns: Vec::new(), gaps: Vec::new() };
    let Some(db) = app.try_state::<Db>() else { return empty; };
    let Ok(Some(config)) = db::get_tray_config(&db).await else { return empty; };
    let mut items: Vec<&TrayItem> = config.items.iter().filter(|i| i.enabled).collect();
    items.sort_by_key(|i| i.order);

    let mut columns: Vec<TrayColumn> = Vec::new();
    let mut gaps: Vec<Option<String>> = Vec::new();
    let mut pending_sep: Option<String> = None;

    for item in items {
        if item.item_type == "separator" {
            pending_sep = Some(if item.display.is_empty() { "·".to_string() } else { item.display.clone() });
            continue;
        }

        // Non-separator item → compute column data
        if !columns.is_empty() {
            gaps.push(pending_sep.take());
        }

        let two_line = item.line_mode == "two";
        let (name, value) = match item.item_type.as_str() {
            "platform" => {
                let Some(pid) = item.platform_id else { continue };
                let Ok(Some(platform)) = db::get_platform(&db, pid).await else { continue };
                platform_item_parts(&platform, &item.display)
            }
            "today_usage" => {
                let stats = db::today_stats(&db).await.unwrap_or(db::TodayStats {
                    tokens: 0, input_tokens: 0, output_tokens: 0, cache_tokens: 0,
                    cache_rate: 0.0, cost: 0.0, total_requests: 0,
                });
                let metric = item.metric.as_deref().unwrap_or("tokens");
                let (label, val) = match metric {
                    "cache_rate" => ("Cache".to_string(), format!("{:.0}%", stats.cache_rate)),
                    "cost" => {
                        let d = item.decimals.unwrap_or(5) as usize;
                        ("花费".to_string(), format!("${}", trim_trailing_zeros(&format!("{:.d$}", stats.cost, d = d))))
                    }
                    "requests" => ("请求".to_string(), format!("{}", stats.total_requests)),
                    _ => ("今日".to_string(), format!("{} tok", stats.tokens)),
                };
                (label, val)
            }
            _ => continue,
        };
        if name.is_empty() && value.is_empty() {
            continue;
        }
        // 自定义 label 优先
        let name = item.label.clone().unwrap_or(name);
        columns.push(TrayColumn {
            name, value,
            color: item.color.clone(),
            font_size: item.font_size,
            two_line,
            align: item.align.clone(),
            align_row2: item.align_row2.clone(),
        });
    }

    TrayLayout { columns, gaps }
}

/// 托盘配置的分隔符（多 item 横排间隔）。
pub(crate) async fn tray_separator(app: &tauri::AppHandle) -> String {
    if let Some(db) = app.try_state::<Db>() {
        if let Ok(Some(config)) = db::get_tray_config(&db).await {
            return config.separator;
        }
    }
    default_separator_str()
}

pub(crate) fn default_separator_str() -> String { "  ".to_string() }

/// 菜单内 quota 项的纯文字概要（无颜色/字号，separator 拼接；每列横排 "名 值"）。
pub(crate) async fn tray_quota_text(app: &tauri::AppHandle) -> Option<String> {
    let layout = tray_layout(app).await;
    if layout.columns.is_empty() {
        return None;
    }
    let default_sep = tray_separator(app).await;
    let mut texts: Vec<String> = Vec::new();
    for (i, col) in layout.columns.iter().enumerate() {
        if i > 0 {
            let gap = layout.gaps.get(i - 1).and_then(|g| g.clone()).unwrap_or_else(|| " ".to_string());
            texts.push(gap);
        }
        texts.push(format!("{} {}", col.name, col.value));
    }
    Some(texts.join(&default_sep))
}

pub(crate) async fn build_tray_menu(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    let running = {
        let handle = app.state::<ProxyHandle>();
        let h = handle.0.lock().map_err(|e| e.to_string())?;
        h.is_some()
    };

    let settings = load_proxy_settings(app).await?;
    let status_text = if running {
        format!("● Proxy Running :{}", settings.port)
    } else {
        "○ Proxy Stopped".to_string()
    };

    let toggle_id = if running { "proxy_stop" } else { "proxy_start" };
    let toggle_text = if running { "Stop Proxy" } else { "Start Proxy" };

    let mut builder = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("status", status_text).enabled(false).build(app).map_err(|e| e.to_string())?);

    // tray quota 详情项（选定平台余额 / coding%）
    if let Some(quota_text) = tray_quota_text(app).await {
        builder = builder
            .item(&MenuItemBuilder::with_id("tray_quota", quota_text).enabled(false).build(app).map_err(|e| e.to_string())?);
    }

    let menu = builder
        .separator()
        .item(&MenuItemBuilder::with_id(toggle_id, toggle_text).build(app).map_err(|e| e.to_string())?)
        .separator()
        .item(&MenuItemBuilder::with_id("show", "Show Window").build(app).map_err(|e| e.to_string())?)
        .item(&MenuItemBuilder::with_id("quit", "Quit").build(app).map_err(|e| e.to_string())?)
        .build().map_err(|e| e.to_string())?;

    Ok(menu)
}

/// 去除浮点数格式化尾部多余的零：10.10 → "10.1", 0.00 → "0", 965.80 → "965.8"
pub(crate) fn trim_trailing_zeros(s: &str) -> String {
    if let Some(_pos) = s.find('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
    } else {
        s.to_string()
    }
}
