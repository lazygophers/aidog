mod gateway;
mod logging;

use gateway::db::{self, Db};
use gateway::models::*;
use tauri::State;
use serde_json::Value;
use std::sync::Arc;

// ─── Helpers ───────────────────────────────────────────────

/// Convert any string to a slug: lowercase, alphanumeric + hyphens only.
/// Chinese/special chars are transliterated or stripped.
fn slugify(input: &str) -> String {
    input
        .to_lowercase()
        .replace(" ", "-")
        .replace("（", "-")
        .replace("）", "")
        .replace("(", "-")
        .replace(")", "")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == ' ' {
                '-'
            } else {
                // Strip non-ASCII non-alphanumeric (Chinese chars etc.)
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        // Collapse multiple hyphens
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Validate group name is a valid slug (lowercase alphanumeric + hyphen)
fn validate_group_key(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("group name cannot be empty".to_string());
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(format!(
            "group name '{}' must contain only lowercase letters, digits, and hyphens",
            name
        ));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("group name cannot start or end with hyphen".to_string());
    }
    Ok(())
}

/// 为所有平台确保存在关联的自动分组（一个平台一个，相互独立）
async fn ensure_platform_groups(db: &Db) {
    let platforms = match db::list_platforms(db).await {
        Ok(p) => p,
        Err(e) => { tracing::error!("ensure_platform_groups: list_platforms failed: {e}"); return; }
    };
    // 一次性取出已有分组的 auto_from_platform 集合，避免循环内重复全表查询（N+1）
    let mut existing_auto: std::collections::HashSet<String> = db::list_groups(db).await
        .unwrap_or_default()
        .into_iter()
        .map(|g| g.auto_from_platform)
        .collect();
    for platform in &platforms {
        // 用户显式选「不创建分组」的平台 → 永久跳过（auto_group 持久化标记）。
        if !platform.auto_group {
            continue;
        }
        // 检查是否已存在关联此平台的分组
        let platform_id_str = platform.id.to_string();
        if existing_auto.contains(&platform_id_str) {
            continue;
        }
        // 自动创建分组（路由纯按 apikey=group_key，不再生成 path）
        let group_key = slugify(&format!("{}-auto", platform.name));
        let group = match db::create_group(db, CreateGroup {
            name: group_key.clone(),
            group_key: Some(group_key.clone()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: platform_id_str.clone(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: 2,
            model_mappings: Vec::new(),
        }).await {
            Ok(g) => g,
            Err(e) => { tracing::error!("ensure_platform_groups: create_group failed for {}: {e}", platform.name); continue; }
        };
        existing_auto.insert(platform_id_str);
        // 将平台关联到自动分组
        if let Err(e) = db::set_group_platforms(db, group.id, &[GroupPlatformInput {
            platform_id: platform.id,
            priority: Some(0),
            weight: Some(1),
        }]).await {
            tracing::error!("ensure_platform_groups: set_group_platforms failed for {}: {e}", platform.name);
        }
        tracing::info!("ensure_platform_groups: created group '{}' for platform '{}'", group_key, platform.name);
    }
}

// ─── About / Version Info ──────────────────────────────────

/// 关于页版本信息（字段 snake_case，前端 AboutInfo 对齐）。
#[derive(serde::Serialize)]
struct AboutInfo {
    app_version: String,
    tauri_version: String,
    os: String,
    arch: String,
    family: String,
    profile: String,
    /// build.rs 注入的 git 短 commit（无 git 时 "unknown"）
    git_commit: String,
    /// build.rs 注入的构建时间（epoch 秒字符串，前端格式化）
    build_time: String,
}

#[tauri::command]
fn about_info() -> AboutInfo {
    AboutInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: std::env::consts::FAMILY.to_string(),
        profile: if cfg!(debug_assertions) { "debug" } else { "release" }.to_string(),
        git_commit: env!("AIDOG_GIT_COMMIT").to_string(),
        build_time: env!("AIDOG_BUILD_TIME").to_string(),
    }
}

// ─── Platform Commands ─────────────────────────────────────

/// 为平台创建默认 auto 分组并关联（name `{slug}-auto`，
/// Failover / max_retries 2）。供 platform_create（勾选默认分组）与
/// platform_update（补建缺失的 auto 分组）复用，避免两处重复构造。
async fn create_auto_group_for(db: &Db, platform: &Platform) -> Result<(), String> {
    let group_key = slugify(&format!("{}-auto", platform.name));
    let group = db::create_group(db, CreateGroup {
        name: group_key.clone(),
        group_key: Some(group_key),
        routing_mode: RoutingMode::Failover,
        auto_from_platform: platform.id.to_string(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
        source_protocol: None,
        max_retries: 2,
        model_mappings: Vec::new(),
    }).await?;
    db::set_group_platforms(db, group.id, &[GroupPlatformInput {
        platform_id: platform.id,
        priority: Some(0),
        weight: Some(1),
    }]).await?;
    tracing::info!(platform_id = platform.id, group_id = group.id, "created auto group for platform");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_create(input: CreatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    tracing::debug!(command = "platform_create", name = %input.name, "command invoked");
    // 分组选项先捕获（input 随即 move 进 create_platform）。
    let auto_group = input.auto_group.unwrap_or(true);
    let join_group_ids = input.join_group_ids.clone().unwrap_or_default();
    let platform = db::create_platform(&db, input).await
        .map_err(|e| { tracing::error!(command = "platform_create", error = %e, "create platform failed"); e })?;

    // ① 创建默认分组（用户勾选；默认勾 = 旧行为）。
    if auto_group {
        if let Err(e) = create_auto_group_for(&db, &platform).await {
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
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_list(db: State<'_, Db>) -> Result<Vec<Platform>, String> {
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
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_get(id: u64, db: State<'_, Db>) -> Result<Option<Platform>, String> {
    tracing::debug!(command = "platform_get", id, "command invoked");
    db::get_platform(&db, id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_update(input: UpdatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    tracing::debug!(command = "platform_update", id = input.id, "command invoked");
    // 分组选项先捕获（input 随即 move 进 update_platform）。
    let join_group_ids = input.join_group_ids.clone();
    let platform = db::update_platform(&db, input).await
        .map_err(|e| { tracing::error!(command = "platform_update", error = %e, "update platform failed"); e })?;

    // auto 分组对账：desired = platform.auto_group（update_platform 已合并 input.auto_group）。
    // desired && 无 auto 组 → 补建；!desired && 有 auto 组 → force_delete（auto 组只含本平台）。
    let groups = db::list_groups(&db).await.unwrap_or_default();
    let platform_id_str = platform.id.to_string();
    let existing_auto = groups.iter().find(|g| g.auto_from_platform == platform_id_str);
    if platform.auto_group && existing_auto.is_none() {
        if let Err(e) = create_auto_group_for(&db, &platform).await {
            tracing::warn!(command = "platform_update", platform_id = platform.id, error = %e, "auto-create group failed");
        }
    } else if !platform.auto_group {
        if let Some(g) = existing_auto {
            if let Err(e) = db::force_delete_group(&db, g.id).await {
                tracing::warn!(command = "platform_update", platform_id = platform.id, group_id = g.id, error = %e, "force delete auto group failed");
            }
        }
    }

    // join_group_ids：全量同步手动组成员关系（auto 组不动；None=不改）。
    if let Some(ids) = join_group_ids {
        if let Err(e) = db::sync_platform_manual_groups(&db, platform.id, &ids).await {
            tracing::warn!(command = "platform_update", platform_id = platform.id, error = %e, "sync manual groups failed");
        }
    }

    Ok(platform)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_delete(id: u64, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "platform_delete", id, "command invoked");
    db::delete_platform(&db, id).await
        .map_err(|e| { tracing::error!(command = "platform_delete", id, error = %e, "delete platform failed"); e })
}

/// 设置 / 清除托盘展示平台（互斥单平台）。
/// enabled=true → 设 platform_id 为唯一展示平台（tray_display: "balance"|"coding"）；
/// enabled=false → 清空所有。改后刷新托盘。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_set_tray(
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
    refresh_tray_menu(&app).await?;
    Ok(())
}

/// 读取托盘配置。无配置时（首次/升级）从旧 show_in_tray 平台迁移生成默认。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn tray_config_get(db: State<'_, Db>) -> Result<TrayConfig, String> {
    tracing::debug!(command = "tray_config_get", "command invoked");
    Ok(db::get_tray_config(&db).await?.unwrap_or_default())
}

/// 保存托盘配置并刷新托盘渲染。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn tray_config_set(
    config: TrayConfig,
    db: State<'_, Db>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tracing::debug!(command = "tray_config_set", "command invoked");
    db::set_tray_config(&db, &config).await
        .map_err(|e| { tracing::error!(command = "tray_config_set", error = %e, "set_tray_config failed"); e })?;
    refresh_tray_menu(&app).await?;
    Ok(())
}

/// 获取今日统计摘要（供前端预览使用）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn tray_today_stats(db: State<'_, Db>) -> Result<db::TodayStats, String> {
    tracing::debug!(command = "tray_today_stats", "command invoked");
    db::today_stats(&db).await
}

// ─── Popover ────────────────────────────────────────────────

/// Popover 弹窗单条数据（tray 列 → 序列化给前端）。
#[derive(serde::Serialize)]
struct PopoverEntry {
    name: String,
    value: String,
    color: TrayColor,
}

/// Popover 弹窗全部数据：配置（驱动渲染）+ 所含 item type 对应数据。
///
/// 内容完全由 `config.items` 的 order + visible 驱动；后端按所含 type 一次性返回所有可能用到的数据
/// （tray 列 / 今日统计 / 各平台当日 / 代理状态），前端按配置顺序裁剪展示。
#[derive(serde::Serialize)]
struct PopoverData {
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
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn popover_data(db: State<'_, Db>, app: tauri::AppHandle) -> Result<PopoverData, String> {
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
        port: 9876, autostart: false, silent_launch: false,
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
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn popover_config_get(db: State<'_, Db>) -> Result<gateway::models::PopoverConfig, String> {
    tracing::debug!(command = "popover_config_get", "command invoked");
    db::get_popover_config(&db).await
}

/// 保存 PopoverConfig。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn popover_config_set(
    config: gateway::models::PopoverConfig,
    db: State<'_, Db>,
) -> Result<(), String> {
    tracing::debug!(command = "popover_config_set", "command invoked");
    db::set_popover_config(&db, &config).await
        .map_err(|e| { tracing::error!(command = "popover_config_set", error = %e, "set_popover_config failed"); e })
}

/// 各平台当日使用（供设置页预览）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn popover_platform_today(db: State<'_, Db>) -> Result<Vec<db::TodayPlatformStat>, String> {
    tracing::debug!(command = "popover_platform_today", "command invoked");
    db::today_platform_stats(&db).await
}

// ─── Group Commands ────────────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_create(mut input: CreateGroup, db: State<'_, Db>, app: tauri::AppHandle) -> Result<Group, String> {
    tracing::debug!(command = "group_create", name = %input.name, "command invoked");
    // Auto-slugify and validate group name
    input.name = slugify(&input.name);
    validate_group_key(&input.name)
        .map_err(|e| { tracing::warn!(command = "group_create", error = %e, "invalid group name"); e })?;
    let result = db::create_group(&db, input).await
        .map_err(|e| { tracing::error!(command = "group_create", error = %e, "create group failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(result)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_list(db: State<'_, Db>) -> Result<Vec<Group>, String> {
    tracing::debug!(command = "group_list", "command invoked");
    db::list_groups(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_get(id: u64, db: State<'_, Db>) -> Result<Option<Group>, String> {
    tracing::debug!(command = "group_get", id, "command invoked");
    db::get_group(&db, id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_update(mut input: UpdateGroup, db: State<'_, Db>, app: tauri::AppHandle) -> Result<Group, String> {
    tracing::debug!(command = "group_update", id = input.id, "command invoked");
    // Auto-slugify and validate if name is being updated
    if let Some(ref name) = input.name {
        let slug = slugify(name);
        validate_group_key(&slug)
            .map_err(|e| { tracing::warn!(command = "group_update", error = %e, "invalid group name"); e })?;
        input.name = Some(slug);
    }
    let result = db::update_group(&db, input).await
        .map_err(|e| { tracing::error!(command = "group_update", error = %e, "update group failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(result)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_delete(id: u64, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "group_delete", id, "command invoked");
    db::delete_group(&db, id).await
        .map_err(|e| { tracing::error!(command = "group_delete", id, error = %e, "delete group failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

// ─── GroupPlatform Commands ────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_set_platforms(input: SetGroupPlatforms, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "group_set_platforms", group_id = input.group_id, count = input.platforms.len(), "command invoked");
    db::set_group_platforms(&db, input.group_id, &input.platforms).await
        .map_err(|e| { tracing::error!(command = "group_set_platforms", group_id = input.group_id, error = %e, "set_group_platforms failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_get_platforms(
    group_id: u64,
    db: State<'_, Db>,
) -> Result<Vec<GroupPlatformDetail>, String> {
    tracing::debug!(command = "group_get_platforms", group_id, "command invoked");
    db::get_group_platforms(&db, group_id).await
}

// ─── Aggregate ─────────────────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_detail(id: u64, db: State<'_, Db>) -> Result<Option<GroupDetail>, String> {
    tracing::debug!(command = "group_detail", id, "command invoked");
    db::get_group_detail(&db, id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_detail_list(db: State<'_, Db>) -> Result<Vec<GroupDetail>, String> {
    tracing::debug!(command = "group_detail_list", "command invoked");
    db::list_group_details(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_reorder(ordered_ids: Vec<u64>, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "group_reorder", count = ordered_ids.len(), "command invoked");
    db::reorder_groups(&db, &ordered_ids).await
        .map_err(|e| { tracing::error!(command = "group_reorder", error = %e, "reorder groups failed"); e })?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

// ─── Proxy Commands ────────────────────────────────────────

use std::sync::Mutex as StdMutex;
use tokio::task::JoinHandle;

/// 代理服务器状态
struct ProxyHandle(StdMutex<Option<JoinHandle<()>>>);

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_start(
    port: u16,
    app: tauri::AppHandle,
) -> Result<String, String> {
    tracing::debug!(command = "proxy_start", port, "command invoked");
    // 检查是否已运行
    let handle = app.state::<ProxyHandle>();
    {
        let h = handle.0.lock().map_err(|e| e.to_string())?;
        if h.is_some() {
            tracing::warn!(command = "proxy_start", "proxy already running");
            return Err("proxy already running".to_string());
        }
    }

    // 获取 DB 的路径并克隆一份连接
    let db_path = aidog_data_dir()?.join("aidog.db");
    let proxy_db = Db::new(db_path.to_str().unwrap_or("")).await
        .map_err(|e| { tracing::error!(command = "proxy_start", error = %e, "open proxy db failed"); e })?;
    let proxy_db = std::sync::Arc::new(proxy_db);

    // 复用 setup 阶段 app.manage 的同一 MiddlewareEngine 单例（CRUD reload 与代理消费同源）。
    let middleware = app.state::<Arc<MiddlewareEngine>>().inner().clone();
    let (proxy_handle, actual_port) = gateway::proxy::start_proxy(proxy_db, port, Some(app.clone()), middleware).await
        .map_err(|e| { tracing::error!(command = "proxy_start", port, error = %e, "start_proxy failed"); e })?;

    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        *h = Some(proxy_handle);
    }

    // 保存实际使用的端口到设置
    let saved = load_proxy_settings(&app).await.unwrap_or(ProxySettings { port: 9876, autostart: true, silent_launch: false });
    save_proxy_settings(&app, actual_port, true, saved.silent_launch).await?;

    // 同步所有分组的 settings 文件（端口可能变了）
    if let Some(db) = app.try_state::<Db>() {
        if let Err(e) = do_sync_group_settings(&db, actual_port).await {
            tracing::warn!(command = "proxy_start", port = actual_port, error = %e, "sync group settings after start failed");
        }
    }

    // 更新托盘菜单
    refresh_tray_menu(&app).await?;

    let msg = if actual_port != port {
        format!("proxy started on port {} ({} was occupied)", actual_port, port)
    } else {
        format!("proxy started on port {}", actual_port)
    };
    tracing::info!(command = "proxy_start", port = actual_port, "proxy started");
    Ok(msg)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_stop(app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "proxy_stop", "command invoked");
    let handle = app.state::<ProxyHandle>();
    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        if let Some(jh) = h.take() {
            jh.abort();
        }
    }

    // 更新设置
    if let Ok(settings) = load_proxy_settings(&app).await {
        save_proxy_settings(&app, settings.port, false, settings.silent_launch).await
            .map_err(|e| { tracing::error!(command = "proxy_stop", error = %e, "persist proxy settings failed"); e })?;
    }

    refresh_tray_menu(&app).await?;
    tracing::info!(command = "proxy_stop", "proxy stopped");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
fn proxy_status(app: tauri::AppHandle) -> Result<bool, String> {
    tracing::debug!(command = "proxy_status", "command invoked");
    let handle = app.state::<ProxyHandle>();
    let h = handle.0.lock().map_err(|e| e.to_string())?;
    Ok(h.is_some())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_get_settings(app: tauri::AppHandle) -> Result<ProxySettings, String> {
    tracing::debug!(command = "proxy_get_settings", "command invoked");
    load_proxy_settings(&app).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "proxy_set_autostart", enabled, "command invoked");
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, enabled, current.silent_launch).await
        .map_err(|e| { tracing::error!(command = "proxy_set_autostart", error = %e, "persist proxy settings failed"); e })?;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
fn app_set_autolaunch(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "app_set_autolaunch", enabled, "command invoked");
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| { tracing::error!(command = "app_set_autolaunch", error = %e, "enable autolaunch failed"); format!("enable autolaunch: {e}") })?;
    } else {
        manager.disable().map_err(|e| { tracing::error!(command = "app_set_autolaunch", error = %e, "disable autolaunch failed"); format!("disable autolaunch: {e}") })?;
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
fn app_get_autolaunch(app: tauri::AppHandle) -> Result<bool, String> {
    tracing::debug!(command = "app_get_autolaunch", "command invoked");
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    manager.is_enabled().map_err(|e| { tracing::warn!(command = "app_get_autolaunch", error = %e, "get autolaunch failed"); format!("get autolaunch: {e}") })
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn app_set_silent_launch(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "app_set_silent_launch", enabled, "command invoked");
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, current.autostart, enabled).await
        .map_err(|e| { tracing::error!(command = "app_set_silent_launch", error = %e, "persist proxy settings failed"); e })?;
    Ok(())
}

// ─── Proxy Client Settings (upstream HTTP proxy) ─────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_client_get_settings(app: tauri::AppHandle) -> Result<gateway::models::ProxyClientSettings, String> {
    tracing::debug!(command = "proxy_client_get_settings", "command invoked");
    let db = app.try_state::<Db>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| { tracing::error!(command = "proxy_client_get_settings", "db not initialized"); "db not initialized".to_string() })?;
    let settings = gateway::http_client::load_proxy_client_settings(&Arc::new(db)).await;
    Ok(settings)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_client_set_settings(app: tauri::AppHandle, settings: gateway::models::ProxyClientSettings) -> Result<(), String> {
    tracing::debug!(command = "proxy_client_set_settings", "command invoked");
    let db = app.try_state::<Db>()
        .map(|s| s.inner())
        .ok_or_else(|| { tracing::error!(command = "proxy_client_set_settings", "db not initialized"); "db not initialized".to_string() })?;
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize proxy client settings: {e}"))?;
    db::set_setting(db, gateway::models::SetSettingInput {
        scope: "proxy".to_string(),
        key: "proxy_client".to_string(),
        value,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_client_set_settings", error = %e, "persist proxy client settings failed"); e })
}

// ─── Platform Model Fetch ──────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_fetch_models(
    protocol: Protocol,
    base_url: String,
    api_key: String,
    db: State<'_, Db>,
) -> Result<Vec<String>, String> {
    tracing::debug!(command = "platform_fetch_models", protocol = ?protocol, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let db_arc = Arc::new(db.inner().clone());
    let client = gateway::http_client::build_http_client_system(&db_arc, 30, 10).await;

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = gateway::db::now();
    let target_protocol = format!("{:?}", protocol).to_lowercase();

    // fetch-models 日志构造器（复用 model_test 标记模式：source_protocol 约定串 + platform_id=0）
    let make_log = |upstream_status: i32, user_status: i32, body: &str, log_url: &str| -> gateway::models::ProxyLog {
        gateway::models::ProxyLog {
            id: request_id.clone(),
            group_key: "[fetch-models]".into(),
            model: String::new(),
            actual_model: String::new(),
            source_protocol: "fetch-models".into(),
            target_protocol: target_protocol.clone(),
            platform_id: 0,
            request_headers: r#"{"source":"fetch-models"}"#.into(),
            request_body: String::new(),
            upstream_request_headers: String::new(),
            upstream_request_body: String::new(),
            response_body: body.into(),
            request_url: "/fetch-models".into(),
            upstream_request_url: log_url.to_string(),
            upstream_response_headers: String::new(),
            upstream_status_code: upstream_status,
            user_response_headers: r#"{"content-type":"application/json"}"#.to_string(),
            user_response_body: body.into(),
            status_code: user_status,
            duration_ms: start.elapsed().as_millis() as i32,
            input_tokens: 0,
            output_tokens: 0,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: false,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at,
            updated_at: created_at,
            deleted_at: 0,
        }
    };

    // Mock / Claude Code 透传平台无真实上游模型列表，不拉取模型
    if matches!(protocol, Protocol::Mock | Protocol::ClaudeCode) {
        return Ok(Vec::new());
    }

    // URL + 鉴权与 proxy.rs models 端点 relay 单一事实源（build_models_url / apply_models_auth）。
    let url = gateway::proxy::build_models_url(&protocol, &base_url);
    let rb = gateway::proxy::apply_models_auth(client.get(&url), &protocol, &api_key);
    tracing::info!(method = "GET", url = %url, "fetch models request");
    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("fetch models request failed: {e}");
            if let Err(le) = db::upsert_proxy_log(&db, make_log(0, 502, &format!("upstream error: {e}"), &url)).await {
                tracing::warn!(command = "platform_fetch_models", error = %le, "persist fetch-models log failed");
            }
            return Err(format!("fetch models: {e}"));
        }
    };
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
    tracing::info!(url = %url, %status, "fetch models response status");
    tracing::debug!(url = %url, body = %body, "fetch models response body");
    // 记录 fetch-models 请求到 proxy_log（成功响应，保留原文便于排查）
    let upstream_status = status.as_u16() as i32;
    if let Err(le) = db::upsert_proxy_log(&db, make_log(upstream_status, upstream_status, &body, &url)).await {
        tracing::warn!(command = "platform_fetch_models", error = %le, "persist fetch-models log failed");
    }
    let resp: Value = serde_json::from_str::<Value>(&body)
        .map_err(|e| {
            tracing::error!("parse response failed: {e}, body={}", &body[..body.len().min(500)]);
            format!("parse response: {e}")
        })?;

    // 解析 {"data": [{"id": "..."}, ...]} 格式
    let models = resp
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            let mut ids: Vec<String> = arr
                .iter()
                .filter_map(|item| item.get("id").and_then(|v| v.as_str()).map(String::from))
                .collect();
            ids.sort();
            ids
        })
        .unwrap_or_default();

    Ok(models)
}

// ─── Statistics ─────────────────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn stats_query(
    db: State<'_, Db>,
    query: StatsQuery,
) -> Result<StatsResult, String> {
    tracing::debug!(command = "stats_query", "command invoked");
    db::query_stats(&db, &query).await
}

// ─── Model Testing ─────────────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn model_test(
    db: State<'_, Db>,
    req: ModelTestRequest,
) -> Result<ModelTestResult, String> {
    tracing::debug!(command = "model_test", platform_id = req.platform_id, "command invoked");
    let platform = db::get_platform(&db, req.platform_id).await?
        .ok_or_else(|| { tracing::warn!(command = "model_test", platform_id = req.platform_id, "platform not found"); "platform not found".to_string() })?;

    let model = req.model.clone().or(platform.models.default.clone())
        .ok_or_else(|| { tracing::warn!(command = "model_test", platform_id = req.platform_id, "no model specified and no default model configured"); "no model specified and no default model configured".to_string() })?;

    let prompt = req.prompt.clone().unwrap_or_else(|| {
        let idx = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize % TEST_PROMPTS.len();
        TEST_PROMPTS[idx].to_string()
    });

    let chat_req = gateway::adapter::ChatRequest {
        model: model.clone(),
        messages: vec![gateway::adapter::Message {
            role: gateway::adapter::Role::User,
            content: gateway::adapter::MessageContent::Text(prompt.clone()),
        }],
        system: None,
        max_tokens: Some(req.max_tokens.unwrap_or(64)),
        temperature: Some(0.0),
        top_p: None,
        stream: Some(false),
        tools: None,
        tool_choice: None,
        extra: None,
    };

    // 优先使用 endpoint 匹配（同 proxy 逻辑），回退到平台主配置
    // model-test 优先选 coding_plan endpoint（测试 coding 端点更有意义），否则取第一个
    let (target_protocol, target_base_url, client_type, coding_plan) = if !platform.endpoints.is_empty() {
        let ep = platform.endpoints.iter().find(|ep| ep.coding_plan)
            .unwrap_or(&platform.endpoints[0]);
        (ep.protocol.clone(), ep.base_url.clone(), ep.client_type.clone(), ep.coding_plan)
    } else {
        (platform.platform_type.clone(), platform.base_url.clone(), ClientType::default(), false)
    };

    let (mut req_body, mut api_path) = gateway::adapter::convert_request(&chat_req, &target_protocol, &platform.platform_type);
    // coding plan 注入（与 proxy.rs 对齐）
    if coding_plan {
        gateway::proxy::inject_coding_plan_fields(&mut req_body, &target_protocol);
        gateway::proxy::override_coding_plan_path(&mut api_path, &target_protocol);
    }
    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);

    // ── 使用与 proxy 相同的客户端 header 模拟逻辑 ──
    // model_test 无入站请求头（平台测试），传空 HeaderMap —— 仅 apply 模拟头，无透传。
    let upstream_headers = gateway::proxy::build_upstream_headers(&client_type, &target_protocol, &platform.api_key, &axum::http::HeaderMap::new());

    let db_arc = Arc::new(db.inner().clone());
    let client = gateway::http_client::build_http_client(
        &db_arc, 30, 10, Some(&platform.extra), None,
    ).await;

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = gateway::db::now();

    let req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(req_body_str.clone());
    let req_builder = gateway::proxy::apply_client_headers(req_builder, &client_type, &target_protocol, &platform.api_key);

    // ── 辅助: 构造测试日志 ──
    let make_log = |body_override: &str, upstream_status: i32, user_status: i32,
                     upstream_resp_headers: &str, user_resp_body: &str,
                     in_tok: i32, out_tok: i32| -> gateway::models::ProxyLog {
        gateway::models::ProxyLog {
            id: request_id.clone(),
            group_key: "[test]".into(),
            model: model.clone(),
            actual_model: model.clone(),
            source_protocol: "test".into(),
            target_protocol: format!("{:?}", target_protocol).to_lowercase(),
            platform_id: platform.id,
            request_headers: r#"{"source":"model-test"}"#.into(),
            request_body: serde_json::to_string(&serde_json::json!({"messages":[{"role":"user","content":prompt}]})).unwrap_or_default(),
            upstream_request_headers: serde_json::Value::Object(
                upstream_headers.iter().map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone()))).collect()
            ).to_string(),
            upstream_request_body: req_body_str.clone(),
            response_body: body_override.into(),
            request_url: format!("/model-test/{}", platform.id),
            upstream_request_url: url.clone(),
            upstream_response_headers: upstream_resp_headers.into(),
            upstream_status_code: upstream_status,
            user_response_headers: r#"{"content-type":"application/json"}"#.to_string(),
            user_response_body: user_resp_body.into(),
            status_code: user_status,
            duration_ms: start.elapsed().as_millis() as i32,
            input_tokens: in_tok,
            output_tokens: out_tok,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: false,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at,
            updated_at: created_at,
            deleted_at: 0,
        }
    };

    // ── Mock 平台：本地生成响应（不发真实 HTTP），与 proxy handle_mock 对齐。
    //   model_test 入站协议固定 "test"；mock build_response 对未知协议走默认 Anthropic 格式。
    //   response_preview 直接取 cfg.response_text（mock 配置的响应文本），无需解析响应体。
    if matches!(target_protocol, Protocol::Mock) {
        let cfg = gateway::adapter::mock::resolve_mock_config(&platform.extra, &chat_req, &req_body);
        if cfg.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(cfg.delay_ms)).await;
        }
        let source_proto_str = "test";
        let (success, status_code, resp_body, err_msg, in_tok, out_tok, preview): (bool, u16, String, String, i32, i32, String) = match cfg.error_mode.as_str() {
            "http_error" => {
                let body = gateway::adapter::mock::build_error_body(source_proto_str, cfg.status_code, "mock http_error");
                let body_str = serde_json::to_string(&body).unwrap_or_default();
                (false, cfg.status_code, body_str, format!("mock http_error (status {})", cfg.status_code), 0, 0, String::new())
            }
            "rate_limit_429" => {
                let body = gateway::adapter::mock::build_error_body(source_proto_str, 429, "mock rate limit");
                let body_str = serde_json::to_string(&body).unwrap_or_default();
                (false, 429, body_str, "mock rate_limit_429".to_string(), 0, 0, String::new())
            }
            "timeout" => {
                // model_test 不真 hang（proxy 里 sleep 600s 是为让客户端超时）；直接返回 504。
                let body = gateway::adapter::mock::build_error_body(source_proto_str, 504, "mock timeout");
                let body_str = serde_json::to_string(&body).unwrap_or_default();
                (false, 504, body_str, "mock timeout".to_string(), 0, 0, String::new())
            }
            _ => {
                let body = gateway::adapter::mock::build_response(&cfg, source_proto_str, &model);
                let body_str = serde_json::to_string(&body).unwrap_or_default();
                (true, 200, body_str, String::new(), cfg.input_tokens, cfg.output_tokens, cfg.response_text.clone())
            }
        };
        let duration_ms = start.elapsed().as_millis() as i32;
        let log_entry = make_log(&resp_body, status_code as i32, status_code as i32, r#"{"content-type":"application/json"}"#, &resp_body, in_tok, out_tok);
        if let Err(le) = db::upsert_proxy_log(&db, log_entry).await {
            tracing::warn!(command = "model_test", platform_id = platform.id, error = %le, "persist mock test log failed");
        }
        tracing::info!(command = "model_test", platform_id = platform.id, mock = true, success, status = status_code, "model test mock response");
        return Ok(ModelTestResult {
            success,
            model: model.clone(),
            prompt_preview: truncate_str(&prompt, 100),
            response_preview: preview,
            duration_ms,
            input_tokens: in_tok,
            output_tokens: out_tok,
            error: err_msg,
        });
    }

    tracing::info!(method = "POST", url = %url, "model test request");
    tracing::debug!(method = "POST", url = %url, body = %req_body_str, "model test request body");
    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let result = ModelTestResult {
                success: false,
                model: model.clone(),
                prompt_preview: truncate_str(&prompt, 100),
                response_preview: String::new(),
                duration_ms: start.elapsed().as_millis() as i32,
                input_tokens: 0,
                output_tokens: 0,
                error: format!("request failed: {e}"),
            };
            tracing::warn!(command = "model_test", platform_id = platform.id, error = %e, "model test request failed");
            if let Err(le) = db::upsert_proxy_log(&db, make_log(
                &format!("upstream error: {e}"), 0, 502, "", &format!("upstream error: {e}"), 0, 0,
            )).await {
                tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
            }
            return Ok(result);
        }
    };

    let duration_ms = start.elapsed().as_millis() as i32;
    let upstream_status_code = resp.status().as_u16() as i32;
    let status = resp.status();

    // 捕获上游响应头
    let upstream_resp_headers = {
        let mut h = serde_json::Map::new();
        for (k, v) in resp.headers() {
            if let Ok(s) = v.to_str() {
                h.insert(k.to_string(), serde_json::Value::String(s.to_string()));
            }
        }
        serde_json::Value::Object(h).to_string()
    };

    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        let result = ModelTestResult {
            success: false,
            model: model.clone(),
            prompt_preview: truncate_str(&prompt, 100),
            response_preview: truncate_str(&body, 200),
            duration_ms,
            input_tokens: 0,
            output_tokens: 0,
            error: format!("HTTP {}", status),
        };
        tracing::warn!(command = "model_test", platform_id = platform.id, %status, "model test non-success upstream status");
        if let Err(le) = db::upsert_proxy_log(&db, make_log(
            &body, upstream_status_code, upstream_status_code,
            &upstream_resp_headers, &body, 0, 0,
        )).await {
            tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
        }
        return Ok(result);
    }

    let resp_json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let response_text = extract_response_text(&resp_json, &target_protocol);
    let (in_tok, out_tok) = extract_test_usage(&resp_json, &target_protocol);

    let result = ModelTestResult {
        success: true,
        model: model.clone(),
        prompt_preview: truncate_str(&prompt, 100),
        response_preview: truncate_str(&response_text, 300),
        duration_ms,
        input_tokens: in_tok,
        output_tokens: out_tok,
        error: String::new(),
    };

    if let Err(le) = db::upsert_proxy_log(&db, make_log(
        &body, upstream_status_code, 200,
        &upstream_resp_headers, &body, in_tok, out_tok,
    )).await {
        tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
    }

    Ok(result)
}

#[allow(dead_code)]
fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}\u{2026}", &s[..max]) }
}

#[allow(dead_code)]
fn extract_response_text(v: &Value, protocol: &Protocol) -> String {
    match protocol {
        Protocol::Anthropic => {
            v.get("content").and_then(|c| c.get(0)).and_then(|b| b.get("text"))
                .and_then(|t| t.as_str()).unwrap_or("").to_string()
        }
        _ => {
            v.get("choices").and_then(|c| c.get(0))
                .and_then(|c| c.get("message")).and_then(|m| m.get("content"))
                .and_then(|t| t.as_str()).unwrap_or("").to_string()
        }
    }
}

#[allow(dead_code)]
fn extract_test_usage(v: &Value, protocol: &Protocol) -> (i32, i32) {
    let usage = v.get("usage");
    match protocol {
        Protocol::Anthropic => {
            let in_tok = usage.and_then(|u| u.get("input_tokens")).and_then(|t| t.as_i64()).unwrap_or(0) as i32;
            let out_tok = usage.and_then(|u| u.get("output_tokens")).and_then(|t| t.as_i64()).unwrap_or(0) as i32;
            (in_tok, out_tok)
        }
        _ => {
            let in_tok = usage.and_then(|u| u.get("prompt_tokens")).and_then(|t| t.as_i64()).unwrap_or(0) as i32;
            let out_tok = usage.and_then(|u| u.get("completion_tokens")).and_then(|t| t.as_i64()).unwrap_or(0) as i32;
            (in_tok, out_tok)
        }
    }
}

// ─── Claude Code Config Export ─────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
fn export_claude_config(port: u16, _app: tauri::AppHandle) -> Result<String, String> {
    tracing::debug!(command = "export_claude_config", port, "command invoked");
    let base_url = format!("http://localhost:{}/claude/v1/messages", port);
    let config_path = dirs::home_dir()
        .ok_or("cannot resolve home directory")?
        .join(".claude.json");

    // 读取已有配置
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("read config: {e}"))?;
        serde_json::from_str(&content).unwrap_or(serde_json::Value::Object(Default::default()))
    } else {
        serde_json::Value::Object(Default::default())
    };

    // 设置 ANTHROPIC_BASE_URL
    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            "ANTHROPIC_BASE_URL".to_string(),
            serde_json::Value::String(base_url.clone()),
        );
    }

    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("serialize config: {e}"))?;
    std::fs::write(&config_path, content)
        .map_err(|e| { tracing::error!(command = "export_claude_config", error = %e, "write .claude.json failed"); format!("write config: {e}") })?;

    Ok(config_path.to_string_lossy().to_string())
}

/// Helper: attempt sync, log errors but don't propagate
async fn try_sync_settings(app: &tauri::AppHandle, db: &Db) {
    if let Ok(settings) = load_proxy_settings(app).await {
        if let Err(e) = do_sync_group_settings(db, settings.port).await {
            tracing::warn!(port = settings.port, error = %e, "sync group settings failed");
        }
    }
}

/// 为所有分组生成 settings.{group_key}.json 配置文件到 ~/.aidog/ 目录
/// 核心逻辑：可被多个触发点调用
async fn do_sync_group_settings(db: &Db, port: u16) -> Result<Vec<String>, String> {
    let groups = gateway::db::list_groups(db).await?;

    let aidog_dir = dirs::home_dir()
        .ok_or("cannot resolve home directory")?
        .join(".aidog");

    // Ensure ~/.aidog/ exists
    std::fs::create_dir_all(&aidog_dir)
        .map_err(|e| format!("create .aidog dir: {e}"))?;

    // Load base claude code config from app settings (scope=global, key=claude_code)
    // Fallback to compiled-in defaults when DB has no config
    let base_config: serde_json::Value = gateway::db::get_setting(db, "global", "claude_code").await
        .ok()
        .flatten()
        .filter(|v| v.is_object() && v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(|| {
            serde_json::from_str(include_str!("../defaults/settings.json"))
                .unwrap_or(serde_json::Value::Object(Default::default()))
        });

    // Collect current group names for cleanup
    let group_keys: std::collections::HashSet<String> = groups.iter().map(|g| g.group_key.clone()).collect();

    // 默认通知 hook 物化（镜像 statusLine）：marker `_aidog_hooks.enabled` 为 true 时，
    // 为每个分组 config 注入 hooks.Stop/Notification（strip marker 之前），并对 Codex
    // 全局 config.toml 一次性注入/移除 notify。脚本只生成一次（循环外）。
    let hooks_enabled = gateway::hooks::hooks_marker_enabled(&base_config);
    let hook_scripts = if hooks_enabled {
        let invoker = resolve_script_invoker(db).await;
        match generate_hook_scripts(invoker) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!(error = %e, "generate hook scripts for default inject failed");
                None
            }
        }
    } else {
        None
    };
    // N2：注入哪些 CC 事件（settings.per_event 中 enabled，回退默认精选集）。每组一致，循环外算一次。
    let inject_events = if hooks_enabled {
        enabled_hook_events(db).await
    } else {
        Vec::new()
    };

    let mut written = Vec::new();

    for group in &groups {
        let group_key = &group.group_key;

        let mut config = base_config.clone();

        // Set proxy routing fields inside env
        if let Some(obj) = config.as_object_mut() {
            if !obj.contains_key("env") {
                obj.insert("env".into(), serde_json::Value::Object(Default::default()));
            }
            if let Some(env_map) = obj.get_mut("env").and_then(|v| v.as_object_mut()) {
                env_map.insert(
                    "ANTHROPIC_BASE_URL".to_string(),
                    serde_json::Value::String(format!("http://127.0.0.1:{}/proxy", port)),
                );
                env_map.insert(
                    "ANTHROPIC_AUTH_TOKEN".to_string(),
                    serde_json::Value::String(group_key.clone()),
                );
            }
        }

        // 默认通知 hook 物化：marker 开启时为本组 config 注入 CC hooks（strip marker 之前）。
        // N2：遍历 inject_events（enabled 事件）注入，每个指向通用脚本 command。
        if let Some(scripts) = &hook_scripts {
            gateway::hooks::inject_claude_code_hooks(&mut config, scripts, &inject_events);
        }

        // Strip internal aidog UI state — not real Claude Code fields.
        if let Some(obj) = config.as_object_mut() {
            obj.remove("_aidog_statusline");
            obj.remove("_aidog_subagent_statusline");
            obj.remove(gateway::hooks::MARKER_HOOKS);
        }

        let file_path = aidog_dir.join(format!("settings.{}.json", group_key));
        let content = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("serialize config for {}: {e}", group_key))?;

        // Diff check: only write when content differs
        let existing = std::fs::read_to_string(&file_path).unwrap_or_default();
        if existing != content {
            std::fs::write(&file_path, &content)
                .map_err(|e| format!("write config for {}: {e}", group_key))?;
            written.push(file_path.to_string_lossy().to_string());
        }

        // Codex profile：为该分组生成 `$CODEX_HOME/<group>.config.toml`
        //（profile 文件 = 用户级层，可含 model_providers）。与 Claude Code
        // json 生成并行，互不影响。失败仅记录、不中断（Codex 未装也不应阻塞）。
        match gateway::codex::write_group_profile(group_key, port) {
            Ok(Some(p)) => written.push(p),
            Ok(None) => {}
            Err(e) => tracing::warn!(group = %group_key, error = %e, "codex profile sync failed"),
        }
    }

    // Codex notify（全局 config.toml，非 per-group）：marker 开启时一次性注入指向
    // complete 脚本的 notify；关闭时移除 aidog notify。Codex 未装/读写失败仅记录、不中断。
    match gateway::codex::codex_config_read() {
        Ok(mut config) => {
            match (&hook_scripts, hooks_enabled) {
                (Some(scripts), true) => {
                    gateway::hooks::inject_codex_notify(&mut config, &scripts.complete);
                }
                _ => {
                    gateway::hooks::remove_codex_notify(&mut config);
                }
            }
            if let Err(e) = gateway::codex::codex_config_write(config) {
                tracing::warn!(error = %e, "codex notify sync write failed");
            }
        }
        Err(e) => tracing::warn!(error = %e, "codex notify sync read failed"),
    }

    // Cleanup: remove settings files for deleted groups
    if let Ok(entries) = std::fs::read_dir(&aidog_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(group_key) = name.strip_prefix("settings.").and_then(|s| s.strip_suffix(".json")) {
                if !group_keys.contains(group_key) {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        tracing::debug!(group = %group_key, error = %e, "remove stale settings file failed");
                    }
                }
            }
        }
    }

    // Cleanup: remove Codex profile files for deleted groups（用户级 config.toml 不动）。
    if let Err(e) = gateway::codex::cleanup_group_profiles(&group_keys) {
        tracing::warn!(error = %e, "codex profile cleanup failed");
    }

    Ok(written)
}

/// Tauri command — manual sync from UI
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn sync_group_settings(app: tauri::AppHandle, db: State<'_, Db>) -> Result<Vec<String>, String> {
    tracing::debug!(command = "sync_group_settings", "command invoked");
    let proxy_settings = load_proxy_settings(&app).await?;
    do_sync_group_settings(&db, proxy_settings.port).await
        .map_err(|e| { tracing::error!(command = "sync_group_settings", error = %e, "sync group settings failed"); e })
}

// ─── Proxy Log Commands ────────────────────────────────────

use gateway::models::{ProxyLog, ProxyLogSummary, ProxyLogSettings, ProxyLogFilter};

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_log_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<ProxyLogSummary>, String> {
    tracing::debug!(command = "proxy_log_list", limit, offset, "command invoked");
    gateway::db::list_proxy_logs(&db, limit, offset).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_log_list_filtered(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<ProxyLogSummary>, String> {
    tracing::debug!(command = "proxy_log_list_filtered", limit, offset, "command invoked");
    gateway::db::filtered_list_proxy_logs(&db, &filter, limit, offset).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_log_count_filtered(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
) -> Result<u32, String> {
    tracing::debug!(command = "proxy_log_count_filtered", "command invoked");
    gateway::db::filtered_count_proxy_logs(&db, &filter).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_log_get(id: String, db: State<'_, Db>) -> Result<Option<ProxyLog>, String> {
    tracing::debug!(command = "proxy_log_get", id = %id, "command invoked");
    gateway::db::get_proxy_log(&db, &id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_log_clear(db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "proxy_log_clear", "command invoked");
    gateway::db::clear_proxy_logs(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_log_count(db: State<'_, Db>) -> Result<u32, String> {
    tracing::debug!(command = "proxy_log_count", "command invoked");
    gateway::db::count_proxy_logs(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_usage_stats(platform_id: u64, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    tracing::debug!(command = "platform_usage_stats", platform_id, "command invoked");
    gateway::db::get_platform_usage_stats(&db, platform_id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn group_usage_stats(group_key: String, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    tracing::debug!(command = "group_usage_stats", group_key = %group_key, "command invoked");
    gateway::db::get_group_usage_stats(&db, &group_key).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn all_group_usage_stats(db: State<'_, Db>) -> Result<std::collections::HashMap<String, gateway::models::PlatformUsageStats>, String> {
    tracing::debug!(command = "all_group_usage_stats", "command invoked");
    gateway::db::get_all_group_usage_stats(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_log_settings_get(db: State<'_, Db>) -> Result<ProxyLogSettings, String> {
    tracing::debug!(command = "proxy_log_settings_get", "command invoked");
    let val = gateway::db::get_setting(&db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(val)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_log_settings_set(db: State<'_, Db>, settings: ProxyLogSettings) -> Result<(), String> {
    tracing::debug!(command = "proxy_log_settings_set", "command invoked");
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize log settings: {e}"))?;
    gateway::db::set_setting(&db, gateway::models::SetSettingInput {
        scope: "proxy".into(),
        key: "logging".into(),
        value,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_log_settings_set", error = %e, "persist log settings failed"); e })?;
    // Run field-level cleanup for user/upstream request data
    if let Err(e) = gateway::db::cleanup_user_request_fields(&db, settings.user_request_retention_days).await {
        tracing::warn!(command = "proxy_log_settings_set", error = %e, "cleanup user_request fields failed");
    }
    if let Err(e) = gateway::db::cleanup_upstream_request_fields(&db, settings.upstream_request_retention_days).await {
        tracing::warn!(command = "proxy_log_settings_set", error = %e, "cleanup upstream_request fields failed");
    }
    // Delete entire log rows older than overall retention
    if settings.retention_days > 0 {
        if let Err(e) = gateway::db::cleanup_proxy_logs(&db, settings.retention_days).await {
            tracing::warn!(command = "proxy_log_settings_set", error = %e, "cleanup proxy_logs failed");
        }
    }
    Ok(())
}

// ─── Proxy Timeout Settings ─────────────────────────────────

use gateway::models::ProxyTimeoutSettings;

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_timeout_get(db: State<'_, Db>) -> Result<ProxyTimeoutSettings, String> {
    tracing::debug!(command = "proxy_timeout_get", "command invoked");
    Ok(gateway::db::get_setting(&db, "proxy", "timeout").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn proxy_timeout_set(db: State<'_, Db>, settings: ProxyTimeoutSettings) -> Result<(), String> {
    tracing::debug!(command = "proxy_timeout_set", "command invoked");
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "proxy".to_string(),
        key: "timeout".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_timeout_set", error = %e, "persist timeout settings failed"); e })
}

// ─── Middleware Rule Engine (C1) ───────────────────────────

use gateway::middleware::MiddlewareEngine;
use gateway::models::{
    CreateMiddlewareRule, MiddlewareRule, MiddlewareSettings, UpdateMiddlewareRule,
};

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn middleware_list_rules(db: State<'_, Db>) -> Result<Vec<MiddlewareRule>, String> {
    tracing::debug!(command = "middleware_list_rules", "command invoked");
    gateway::db::list_middleware_rules(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn middleware_create_rule(
    input: CreateMiddlewareRule,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<MiddlewareRule, String> {
    tracing::debug!(command = "middleware_create_rule", "command invoked");
    let rule = gateway::db::create_middleware_rule(&db, input).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_create_rule", error = %e, "engine reload failed");
    }
    Ok(rule)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn middleware_update_rule(
    input: UpdateMiddlewareRule,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<MiddlewareRule, String> {
    tracing::debug!(command = "middleware_update_rule", "command invoked");
    let rule = gateway::db::update_middleware_rule(&db, input).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_update_rule", error = %e, "engine reload failed");
    }
    Ok(rule)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn middleware_delete_rule(
    id: i64,
    db: State<'_, Db>,
    engine: State<'_, Arc<MiddlewareEngine>>,
) -> Result<(), String> {
    tracing::debug!(command = "middleware_delete_rule", id, "command invoked");
    gateway::db::delete_middleware_rule(&db, id).await?;
    if let Err(e) = engine.reload(&db).await {
        tracing::warn!(command = "middleware_delete_rule", error = %e, "engine reload failed");
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn middleware_settings_get(db: State<'_, Db>) -> Result<MiddlewareSettings, String> {
    tracing::debug!(command = "middleware_settings_get", "command invoked");
    Ok(gateway::db::get_setting(&db, "middleware", "settings").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn middleware_settings_set(
    db: State<'_, Db>,
    settings: MiddlewareSettings,
) -> Result<(), String> {
    tracing::debug!(command = "middleware_settings_set", "command invoked");
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "middleware".to_string(),
        key: "settings".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize middleware settings: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "middleware_settings_set", error = %e, "persist middleware settings failed"); e })
}

// ─── Scheduling & Breaker Settings ─────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn scheduling_settings_get(db: State<'_, Db>) -> Result<SchedulingBreakerSettings, String> {
    tracing::debug!(command = "scheduling_settings_get", "command invoked");
    Ok(gateway::db::get_scheduling_settings(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn scheduling_settings_set(
    db: State<'_, Db>,
    settings: SchedulingBreakerSettings,
) -> Result<(), String> {
    tracing::debug!(command = "scheduling_settings_set", "command invoked");
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "scheduling".to_string(),
        key: "settings".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize scheduling settings: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "scheduling_settings_set", error = %e, "persist scheduling settings failed"); e })
}

// ─── Notification（N1 — 系统通知模块）─────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn notification_settings_get(db: State<'_, Db>) -> Result<NotificationSettings, String> {
    tracing::debug!(command = "notification_settings_get", "command invoked");
    Ok(gateway::db::get_notification_settings(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn notification_settings_set(
    db: State<'_, Db>,
    settings: NotificationSettings,
) -> Result<(), String> {
    tracing::debug!(command = "notification_settings_set", "command invoked");
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "notification".to_string(),
        key: "settings".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize notification settings: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "notification_settings_set", error = %e, "persist notification settings failed"); e })
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn notification_inbox_list(db: State<'_, Db>, limit: Option<i64>) -> Result<Vec<Notification>, String> {
    tracing::debug!(command = "notification_inbox_list", "command invoked");
    gateway::db::list_notifications(&db, limit.unwrap_or(100)).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn notification_clear(db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "notification_clear", "command invoked");
    gateway::db::clear_notifications(&db).await
}

// ─── Skills 管理 ───────────────────────────────────────────

use gateway::skills::{
    CachedSkills, CatalogEntry, SkillAgent, SkillScope, SkillsEnv, SkillsOpResult,
};

/// 探测 npx / node 环境（写操作前置）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_check_env() -> Result<SkillsEnv, String> {
    tracing::debug!(command = "skills_check_env", "command invoked");
    Ok(gateway::skills::check_env())
}

/// 读取上游代理设置并构造 npx/npm 用代理 URL（enabled → Some，否则 None）。
/// 所有 skills npx / catalog 抓取命令复用此值注入代理，使 skill 下载/查询尊重上游代理。
async fn skills_proxy_url(db: &State<'_, Db>) -> Option<String> {
    let db_arc = Arc::new(db.inner().clone());
    let settings = gateway::http_client::load_proxy_client_settings(&db_arc).await;
    gateway::skills::proxy_env_url(&settings)
}

/// 浏览 catalog（HTTP 抓 skills.sh，回退 npx find）。尊重上游代理。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_browse_catalog(db: State<'_, Db>) -> Result<Vec<CatalogEntry>, String> {
    tracing::debug!(command = "skills_browse_catalog", "command invoked");
    let proxy = skills_proxy_url(&db).await;
    Ok(gateway::skills::browse_catalog(proxy.as_deref()).await)
}

/// 搜索 catalog。尊重上游代理。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_search(db: State<'_, Db>, keyword: String) -> Result<Vec<CatalogEntry>, String> {
    tracing::debug!(command = "skills_search", keyword = %keyword, "command invoked");
    let proxy = skills_proxy_url(&db).await;
    Ok(gateway::skills::search(&keyword, proxy.as_deref()).await)
}

/// 列指定 scope 下已装 skills —— **立即返回缓存**（内存→磁盘，命中即 0 子进程）。
/// 冷启动（无缓存）返回空 + `stale=true`，前端据此显加载态并触发 `skills_list_refresh`。
/// SWR 的 "stale" 半：不跑 npx，开页瞬间渲染。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_list_installed(scope: SkillScope) -> Result<CachedSkills, String> {
    tracing::debug!(command = "skills_list_installed", "command invoked");
    Ok(gateway::skills::list_cached(&scope))
}

/// 强制跑 `npx skills list --json`、更新内存+磁盘缓存、返回 fresh（`stale=false`）。尊重上游代理。
/// SWR 的 "revalidate" 半：前端后台调用，完成后更新列表。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_list_refresh(db: State<'_, Db>, scope: SkillScope) -> Result<CachedSkills, String> {
    tracing::debug!(command = "skills_list_refresh", "command invoked");
    let proxy = skills_proxy_url(&db).await;
    Ok(gateway::skills::list_refresh(&scope, proxy.as_deref()))
}

/// 为某 agent 启用 skill（shell out `npx skills add <path> -a <slug> [-g] -y`）。
/// `path` = skill 本地安装路径（前端传 `SkillInfo.installed_path`），不依赖锁文件 source。
/// 启用可能触发 skill 下载 → 尊重上游代理。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_enable(
    db: State<'_, Db>,
    name: String,
    path: String,
    agent: SkillAgent,
    scope: SkillScope,
) -> Result<SkillsOpResult, String> {
    tracing::debug!(command = "skills_enable", name = %name, "command invoked");
    let proxy = skills_proxy_url(&db).await;
    let res = gateway::skills::enable(&name, &path, agent, &scope, proxy.as_deref());
    if res.success {
        gateway::skills::invalidate(&scope);
    }
    Ok(res)
}

/// 从 catalog 安装 skill 到多个 agent（shell out `npx skills add <id> -a <slug> -y`）。
/// `id` = `owner/repo@skill`（CatalogEntry.id）。尊重上游代理。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_install(
    db: State<'_, Db>,
    id: String,
    agents: Vec<SkillAgent>,
    scope: SkillScope,
) -> Result<SkillsOpResult, String> {
    tracing::debug!(command = "skills_install", id = %id, agents = ?agents, "command invoked");
    let proxy = skills_proxy_url(&db).await;
    let res = gateway::skills::install(&id, &agents, &scope, proxy.as_deref());
    if res.success {
        gateway::skills::invalidate(&scope);
    }
    Ok(res)
}

/// 列已装 skill 目录文件树（详情视图浏览，只读）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skill_detail(installed_path: String) -> Result<gateway::skills::SkillDetail, String> {
    tracing::debug!(command = "skill_detail", path = %installed_path, "command invoked");
    gateway::skills::detail(&installed_path)
}

/// 读 skill 内单文件（只读浏览）。带路径遍历防护 + 二进制检测 + 大小上限。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skill_read_file(
    installed_path: String,
    rel: String,
) -> Result<gateway::skills::SkillFileContent, String> {
    tracing::debug!(command = "skill_read_file", path = %installed_path, rel = %rel, "command invoked");
    gateway::skills::read_file(&installed_path, &rel)
}

/// 为某 agent 关闭 skill（shell out `npx skills remove -s -a -y`）。尊重上游代理。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_disable(
    db: State<'_, Db>,
    name: String,
    agent: SkillAgent,
    scope: SkillScope,
) -> Result<SkillsOpResult, String> {
    tracing::debug!(command = "skills_disable", name = %name, "command invoked");
    let proxy = skills_proxy_url(&db).await;
    let res = gateway::skills::disable(&name, agent, &scope, proxy.as_deref());
    if res.success {
        gateway::skills::invalidate(&scope);
    }
    Ok(res)
}

/// 组级 agent 批量：对某 source 组（group_source=None = 「其他」组）内所有 skill
/// 统一启用/禁用某 agent。内部已 invalidate。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_set_group_agent(
    db: State<'_, Db>,
    group_source: Option<String>,
    agent: SkillAgent,
    enable: bool,
    scope: SkillScope,
) -> Result<SkillsOpResult, String> {
    tracing::debug!(
        command = "skills_set_group_agent",
        group_source = ?group_source,
        agent = ?agent,
        enable,
        "command invoked"
    );
    let proxy = skills_proxy_url(&db).await;
    Ok(gateway::skills::set_group_agent(
        group_source.as_deref(),
        agent,
        enable,
        &scope,
        proxy.as_deref(),
    ))
}

/// 更新已装 skills（shell out `npx skills update`）。尊重上游代理（拉取更新）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_update(db: State<'_, Db>, scope: SkillScope) -> Result<SkillsOpResult, String> {
    tracing::debug!(command = "skills_update", "command invoked");
    let proxy = skills_proxy_url(&db).await;
    let res = gateway::skills::update(&scope, proxy.as_deref());
    if res.success {
        gateway::skills::invalidate(&scope);
    }
    Ok(res)
}

/// 一键卸载当前 scope 下所有平台所有 skills（破坏性，前端二次确认）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_uninstall_all(db: State<'_, Db>, scope: SkillScope) -> Result<SkillsOpResult, String> {
    tracing::debug!(command = "skills_uninstall_all", "command invoked");
    let proxy = skills_proxy_url(&db).await;
    let res = gateway::skills::uninstall_all(&scope, proxy.as_deref());
    if res.success {
        gateway::skills::invalidate(&scope);
    }
    Ok(res)
}

/// 卸载单一 skill（破坏性，前端二次确认）：删规范存储 + 所有 agent 启用配置。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_uninstall(
    db: State<'_, Db>,
    name: String,
    scope: SkillScope,
) -> Result<SkillsOpResult, String> {
    tracing::debug!(command = "skills_uninstall", "command invoked");
    let proxy = skills_proxy_url(&db).await;
    let result = gateway::skills::uninstall(&name, &scope, proxy.as_deref());
    tracing::debug!(
        command = "skills_uninstall",
        name = %name,
        scope = ?scope,
        success = result.success,
        stdout = %result.stdout.trim(),
        stderr = %result.stderr.trim(),
        "npx remove result",
    );
    if result.success {
        gateway::skills::invalidate(&scope);
    }
    Ok(result)
}

/// 组级卸载：卸载某 source 分组（groupSource=null = 「其他」组）内所有 skill。
/// 破坏性，前端二次确认。内部已 invalidate。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_uninstall_group(
    db: State<'_, Db>,
    group_source: Option<String>,
    scope: SkillScope,
) -> Result<SkillsOpResult, String> {
    tracing::debug!(
        command = "skills_uninstall_group",
        group_source = ?group_source,
        "command invoked"
    );
    let proxy = skills_proxy_url(&db).await;
    Ok(gateway::skills::uninstall_group(
        group_source.as_deref(),
        &scope,
        proxy.as_deref(),
    ))
}

/// 对齐两 agent 的 skills 启用配置（使 `to` 与 `from` 完全一致）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_align_agents(
    db: State<'_, Db>,
    from: SkillAgent,
    to: SkillAgent,
    scope: SkillScope,
) -> Result<SkillsOpResult, String> {
    tracing::debug!(command = "skills_align_agents", "command invoked");
    let proxy = skills_proxy_url(&db).await;
    let res = gateway::skills::align_agents(from, to, &scope, proxy.as_deref());
    if res.success {
        gateway::skills::invalidate(&scope);
    }
    Ok(res)
}

/// 为某 agent 启用当前 scope 全部已装 skills（只增不减）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn skills_enable_all(
    db: State<'_, Db>,
    agent: SkillAgent,
    scope: SkillScope,
) -> Result<SkillsOpResult, String> {
    tracing::debug!(command = "skills_enable_all", "command invoked");
    let proxy = skills_proxy_url(&db).await;
    let res = gateway::skills::enable_all(agent, &scope, proxy.as_deref());
    if res.success {
        gateway::skills::invalidate(&scope);
    }
    Ok(res)
}

// ─── MCP 管理 ─────────────────────────────────────────────

/// 列出 DB 中所有 MCP server（env/headers 已脱敏）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn mcp_list(db: State<'_, Db>) -> Result<Vec<gateway::mcp::McpServerInfo>, String> {
    tracing::debug!(command = "mcp_list", "command invoked");
    let rows = gateway::db::list_mcp_servers(&db).await?;
    Ok(rows.into_iter().map(gateway::mcp::McpServerInfo::from).collect())
}

/// 扫描 Claude Code + Codex 配置的所有 MCP，去重合并（env/headers 已脱敏）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn mcp_scan(db: State<'_, Db>) -> Result<Vec<gateway::mcp::McpScanItem>, String> {
    tracing::debug!(command = "mcp_scan", "command invoked");
    gateway::mcp::scan_all(&db).await
}

/// 批量导入 MCP（从 agent 配置取原值入 DB，enabled = source agent）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn mcp_import(
    db: State<'_, Db>,
    items: Vec<gateway::mcp::McpImportPayload>,
) -> Result<gateway::mcp::ImportReport, String> {
    tracing::debug!(command = "mcp_import", count = items.len(), "command invoked");
    gateway::mcp::import_items(&db, items).await
}

/// per-agent 启用/禁用：改 DB enabled_agents + 同步写/删 agent 配置。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn mcp_set_agent(
    db: State<'_, Db>,
    name: String,
    agent: String,
    enabled: bool,
) -> Result<(), String> {
    tracing::debug!(command = "mcp_set_agent", name = %name, agent = %agent, enabled, "command invoked");
    let agent = gateway::mcp::McpAgent::from_slug(&agent)
        .ok_or_else(|| format!("unknown agent slug: {agent}"))?;
    gateway::mcp::set_agent_enabled(&db, &name, agent, enabled).await
}

/// 删除 MCP：DB + 所有 enabled agent 配置（破坏性，前端二次确认）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn mcp_delete(db: State<'_, Db>, name: String) -> Result<(), String> {
    tracing::debug!(command = "mcp_delete", name = %name, "command invoked");
    gateway::mcp::delete_server(&db, &name).await
}

/// 手动添加 MCP：校验 name 唯一 → 入库（enabled 空，不写 agent 配置）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn mcp_add(
    db: State<'_, Db>,
    payload: gateway::mcp::McpUpdatePayload,
) -> Result<gateway::mcp::McpServerInfo, String> {
    tracing::debug!(command = "mcp_add", name = %payload.name, "command invoked");
    gateway::mcp::add_server(&db, payload).await
}

/// 编辑 MCP：全字段更新（含改名/transport 切换）+ 同步 enabled agent 配置。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn mcp_update(
    db: State<'_, Db>,
    old_name: String,
    payload: gateway::mcp::McpUpdatePayload,
) -> Result<gateway::mcp::McpServerInfo, String> {
    tracing::debug!(command = "mcp_update", old = %old_name, "command invoked");
    gateway::mcp::update_server(&db, &old_name, payload).await
}

/// 重新同步全部：从 DB 全量重写所有 enabled agent 的 MCP 配置文件，
/// 修复外部污染（如 env:null 致 Claude Code 跳过 server）。返回重写条数。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn mcp_resync(db: State<'_, Db>) -> Result<usize, String> {
    tracing::debug!(command = "mcp_resync", "command invoked");
    gateway::mcp::resync_all(&db).await
}

// ─── 导入导出子系统 ───────────────────────────────────────

/// 导出：收集各 scope 数据 → 加密 → 写入用户选择路径。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn export_to_file(
    db: State<'_, Db>,
    scopes: Vec<String>,
    path: String,
) -> Result<(), String> {
    tracing::debug!(command = "export_to_file", scopes = ?scopes, path = %path, "command invoked");
    let mut payload = gateway::import_export::collect::collect(&db, &scopes).await?;
    let bytes = payload.serialize_with_checksum()?;
    let encrypted = gateway::import_export::encrypt(&bytes)?;
    std::fs::write(&path, &encrypted).map_err(|e| format!("write export file: {e}"))?;
    Ok(())
}

/// 读取定时备份设置 (缺省/解析失败 → 默认)。
#[tauri::command]
async fn backup_settings_get(db: State<'_, Db>) -> Result<gateway::backup::BackupSettings, String> {
    tracing::debug!(command = "backup_settings_get", "command invoked");
    Ok(gateway::backup::BackupSettings::load(&db).await.sanitized())
}

/// 写入定时备份设置 (前端勾选/改间隔/改保留天数)。
#[tauri::command]
async fn backup_settings_set(
    db: State<'_, Db>,
    mut settings: gateway::backup::BackupSettings,
) -> Result<gateway::backup::BackupSettings, String> {
    tracing::debug!(command = "backup_settings_set", "command invoked");
    // 走过此命令 (UI 保存入口) = 用户手动确认, 强制标记为当前版本;
    // 前端不传 defaults_version → serde default=0, 这里覆写后即便 enabled=false 也永久尊重。
    settings.defaults_version = gateway::backup::CURRENT_DEFAULTS_VERSION;
    let sanitized = settings.sanitized();
    sanitized.save(&db).await?;
    Ok(sanitized)
}

/// 立即触发一次备份 (忽略 throttle; 失败返回 error, 前端 toast)。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn backup_run_now(db: State<'_, Db>) -> Result<gateway::backup::BackupResult, String> {
    tracing::debug!(command = "backup_run_now", "command invoked");
    let ts = chrono::Utc::now().timestamp_millis();
    match gateway::backup::run_backup(&db).await {
        Ok(path) => Ok(gateway::backup::BackupResult {
            ok: true,
            path: Some(path.to_string_lossy().to_string()),
            error: None,
            timestamp: ts,
        }),
        Err(e) => Ok(gateway::backup::BackupResult {
            ok: false,
            path: None,
            error: Some(e),
            timestamp: ts,
        }),
    }
}

/// 导入预览：读文件 → 解密 → 校验 → 扫描冲突，返回前端弹窗所需信息。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn import_read_file(
    db: State<'_, Db>,
    path: String,
) -> Result<gateway::import_export::ImportPreview, String> {
    tracing::debug!(command = "import_read_file", path = %path, "command invoked");
    let bytes = std::fs::read(&path).map_err(|e| format!("read import file: {e}"))?;
    gateway::import_export::apply::preview(&bytes, &db).await
}

/// 导入应用：按用户决策写入 db + 文件 + skills。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn import_apply(
    db: State<'_, Db>,
    path: String,
    decisions: Vec<gateway::import_export::ConflictDecision>,
) -> Result<gateway::import_export::ImportReport, String> {
    tracing::debug!(command = "import_apply", path = %path, decisions = decisions.len(), "command invoked");
    let bytes = std::fs::read(&path).map_err(|e| format!("read import file: {e}"))?;
    let plain = gateway::import_export::decrypt(&bytes)?;
    let payload = gateway::import_export::Payload::from_bytes_verified(&plain)?;
    gateway::import_export::apply::apply(payload, &decisions, &db).await
}

/// cc-switch 导入：探测本地 cc-switch 配置（SQLite / 旧 JSON）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn ccswitch_detect(
    override_path: Option<String>,
) -> Result<gateway::import_export::CcswitchDetection, String> {
    tracing::debug!(command = "ccswitch_detect", "command invoked");
    gateway::import_export::ccswitch::detect(override_path).await
}

/// cc-switch 导入：读取 providers（仅 claude + codex），返回原始 DTO。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn ccswitch_read(
    db: State<'_, Db>,
    path: Option<String>,
) -> Result<gateway::import_export::CcswitchReadResult, String> {
    tracing::debug!(command = "ccswitch_read", "command invoked");
    gateway::import_export::ccswitch::read(&db, path).await
}

/// cc-switch 导入：接收前端转换好的 Platform JSON + 决策，走 apply::apply 写入。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn ccswitch_import(
    db: State<'_, Db>,
    platform_payload: Vec<serde_json::Value>,
    decisions: Vec<gateway::import_export::ConflictDecision>,
) -> Result<gateway::import_export::ImportReport, String> {
    tracing::debug!(
        command = "ccswitch_import",
        payload_count = platform_payload.len(),
        decisions = decisions.len(),
        "command invoked"
    );
    gateway::import_export::ccswitch::import(platform_payload, &decisions, &db).await
}

/// 测试通知：直接走分发逻辑（前端设置页"测试"按钮），不经 /api/notify 端点。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn notification_test(
    db: State<'_, Db>,
    app: tauri::AppHandle,
    notif_type: String,
    content: Option<String>,
) -> Result<gateway::notification::DispatchResult, String> {
    tracing::debug!(command = "notification_test", notif_type = %notif_type, "command invoked");
    let mut vars = std::collections::HashMap::new();
    vars.insert("project".to_string(), "aidog".to_string());
    vars.insert("status".to_string(), "test".to_string());
    vars.insert("time".to_string(), chrono::Local::now().format("%H:%M:%S").to_string());
    vars.insert("session".to_string(), "test-session".to_string());
    vars.insert("group".to_string(), "test".to_string());
    let db_arc = std::sync::Arc::new(db.inner().clone());
    Ok(gateway::notification::dispatch(&db_arc, Some(&app), None, &notif_type, content.as_deref(), &vars).await)
}

/// 仅测 TTS 通道（绕过 dispatch，按当前 settings.tts_backend 播报 text）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn notification_test_tts(
    db: State<'_, Db>,
    app: tauri::AppHandle,
    text: String,
) -> Result<(), String> {
    tracing::debug!(command = "notification_test_tts", "command invoked");
    let db_arc = std::sync::Arc::new(db.inner().clone());
    let settings = gateway::db::get_notification_settings(&db_arc).await;
    gateway::notification::speak(Some(&app), settings.tts_backend, &text);
    Ok(())
}

/// 仅测系统弹窗通道（绕过 dispatch，直接调 tauri-plugin-notification）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn notification_test_popup(
    app: tauri::AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    tracing::debug!(command = "notification_test_popup", "command invoked");
    gateway::notification::show_popup(&app, &title, &body);
    Ok(())
}

/// 仅测系统提示音通道（跨平台 spawn system beep）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn notification_test_beep() -> Result<(), String> {
    tracing::debug!(command = "notification_test_beep", "command invoked");
    gateway::notification::play_beep();
    Ok(())
}

// ─── Platform Quota (Balance & Coding Plan) ────────────────

use gateway::quota::PlatformQuota;

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_query_quota(
    base_url: String, api_key: String,
    platform_id: Option<u64>, db: State<'_, Db>,
) -> Result<PlatformQuota, String> {
    tracing::debug!(command = "platform_query_quota", platform_id = ?platform_id, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let q = gateway::quota::query_quota(Some(&Arc::new(db.inner().clone())), &base_url, &api_key).await;
    tracing::info!(platform_id = ?platform_id, success = q.success, tiers = ?q.coding_plan.as_ref().map(|c| c.tiers.len()), "quota query result");
    if q.success {
        persist_quota_to_db(&db, platform_id, &q).await;
    }
    Ok(q)
}

/// New API 专用余额查询（两步：先查 token quota 类型，再按需查用户余额）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_query_quota_newapi(
    base_url: String, api_key: String, extra: String,
    platform_id: Option<u64>, db: State<'_, Db>,
) -> Result<PlatformQuota, String> {
    tracing::debug!(command = "platform_query_quota_newapi", platform_id = ?platform_id, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let q = gateway::quota::query_quota_newapi(Some(&Arc::new(db.inner().clone())), &base_url, &api_key, &extra).await;
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
async fn persist_quota_to_db(db: &Db, platform_id: Option<u64>, q: &PlatformQuota) {
    let Some(pid) = platform_id else { return };
    let is_coding_plan = q.coding_plan.is_some();
    gateway::estimate::calibrate_from_quota(db, pid, q, is_coding_plan).await;
}

/// 冷启动 est 初始化：对 tray 中启用、且从未真查过（last_real_query_at==0）的平台，
/// 后台触发一次真查并校准对齐 est=真实。避免冷启动 tray 显 0/旧偏差大。
/// 不阻塞：每平台 spawn 独立 async（锁外 await 真查，calibrate_from_quota 短持锁写）。
/// 真查完成后发 tray-refresh，让主线程刷新托盘显示。
async fn cold_start_init_tray_estimates(app: &tauri::AppHandle) {
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
                gateway::quota::query_quota_newapi(Some(&db_arc), &p.base_url, &p.api_key, &p.extra).await
            } else {
                gateway::quota::query_quota(Some(&db_arc), &p.base_url, &p.api_key).await
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

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn platform_reorder(ordered_ids: Vec<u64>, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "platform_reorder", count = ordered_ids.len(), "command invoked");
    db::reorder_platforms(&db, &ordered_ids).await
        .map_err(|e| { tracing::error!(command = "platform_reorder", error = %e, "reorder platforms failed"); e })
}

// ─── Path Autocomplete ─────────────────────────────────────

use serde::Serialize;

#[derive(Serialize)]
struct PathEntry {
    name: String,
    full_path: String,
    is_dir: bool,
    /// Unix timestamp (seconds)
    modified: i64,
}

/// Expand `~` to home directory and resolve path
fn expand_path(input: &str) -> std::path::PathBuf {
    if input.starts_with("~/") || input == "~" {
        if let Some(home) = dirs::home_dir() {
            if input == "~" {
                return home;
            }
            return home.join(&input[2..]);
        }
    }
    std::path::PathBuf::from(input)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
fn fs_autocomplete(input: String) -> Result<Vec<PathEntry>, String> {
    tracing::debug!(command = "fs_autocomplete", "command invoked");
    let path = expand_path(input.trim());

    // Determine parent dir and prefix filter
    let (parent, prefix) = if input.ends_with('/') || input == "~" || input.ends_with('~') {
        (path.clone(), "".to_string())
    } else {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
        });
        (parent, file_name)
    };

    if !parent.exists() || !parent.is_dir() {
        return Ok(vec![]);
    }

    let entries: Vec<PathEntry> = std::fs::read_dir(&parent)
        .map_err(|e| { tracing::warn!(command = "fs_autocomplete", error = %e, "read_dir failed"); e.to_string() })?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Filter by prefix
            if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                return None;
            }

            let metadata = entry.metadata().ok()?;
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let full_path = entry.path().to_string_lossy().to_string();

            Some(PathEntry {
                name,
                full_path,
                is_dir: metadata.is_dir(),
                modified,
            })
        })
        .collect();

    // Sort: directories first, then by modification time descending
    let mut sorted = entries;
    sorted.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.modified.cmp(&a.modified),
        }
    });

    // Limit results
    sorted.truncate(20);

    Ok(sorted)
}

// ─── Settings Commands ─────────────────────────────────────

use gateway::models::SetSettingInput;

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn settings_get(
    scope: String,
    key: String,
    db: State<'_, Db>,
) -> Result<Option<serde_json::Value>, String> {
    tracing::debug!(command = "settings_get", scope = %scope, key = %key, "command invoked");
    db::get_setting(&db, &scope, &key).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn settings_set(input: SetSettingInput, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "settings_set", scope = %input.scope, key = %input.key, "command invoked");
    db::set_setting(&db, input).await
        .map_err(|e| { tracing::error!(command = "settings_set", error = %e, "persist setting failed"); e })?;
    // Auto-sync group settings files when claude code config changes
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn settings_delete(scope: String, key: String, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "settings_delete", scope = %scope, key = %key, "command invoked");
    db::delete_setting(&db, &scope, &key).await
        .map_err(|e| { tracing::error!(command = "settings_delete", error = %e, "delete setting failed"); e })
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn settings_list(scope: String, db: State<'_, Db>) -> Result<Vec<String>, String> {
    tracing::debug!(command = "settings_list", scope = %scope, "command invoked");
    db::list_setting_keys(&db, &scope).await
}

// ─── StatusLine Script Generation ──────────────────────────

/// Generate statusline script file in ~/.aidog/scripts/ and return the **command
/// string** to invoke it (`uv run --script <path>` or `python3 <path>`).
/// `script_type`: "statusline" | "subagent"
///
/// statusline 脚本现为 Python（PEP723，stdlib only；`content` 由前端 statusline-gen
/// 拼成，内嵌渲染引擎，输出与旧 bash 逐字节一致——见 scripts/statusline-golden 回归）。
/// 写 `aidog-statusline.py` / `aidog-subagent-statusline.py`，复用 ScriptInvoker
/// 决定 command 串（与通知 hook 同机制）。迁移清理旧版 `.sh`（scripts/ 下 + ~/.aidog/ 根）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn generate_statusline_script(
    script_type: String,
    content: String,
    db: State<'_, Db>,
) -> Result<String, String> {
    tracing::debug!(command = "generate_statusline_script", script_type = %script_type, "command invoked");
    let scripts_dir = aidog_scripts_dir()?;
    let (filename, legacy_sh) = if script_type == "subagent" {
        ("aidog-subagent-statusline.py", "aidog-subagent-statusline.sh")
    } else {
        ("aidog-statusline.py", "aidog-statusline.sh")
    };
    // 迁移清理：删除旧版 bash 脚本（~/.aidog/ 根 + scripts/ 下）。
    cleanup_legacy_root_script(filename);
    cleanup_legacy_root_script(legacy_sh);
    cleanup_legacy_scripts_dir_file(&scripts_dir, legacy_sh);
    let path = scripts_dir.join(filename);
    std::fs::write(&path, &content).map_err(|e| { tracing::error!(command = "generate_statusline_script", error = %e, "write script failed"); format!("write script: {e}") })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).map_err(|e| { tracing::error!(command = "generate_statusline_script", error = %e, "stat script failed"); format!("stat script: {e}") })?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).map_err(|e| { tracing::error!(command = "generate_statusline_script", error = %e, "chmod script failed"); format!("chmod script: {e}") })?;
    }
    let invoker = resolve_script_invoker(&db).await;
    Ok(invoker.command_for(&path.to_string_lossy()))
}

// ─── Notification Hook Integration (N2) ────────────────────

/// 生成 Python hook 脚本到 ~/.aidog/scripts/，chmod 755，清理旧版 ~/.aidog/*.sh，返回各自的
/// **command 串**（`uv run --script <path>` 或 `python3 <path>`，由 `invoker` 决定）。
/// - `complete`（task_complete）：**Codex notify 仍用**（agent-turn-complete 语义）。
/// - `waiting`（waiting_input）：保留兼容。
/// - `event_notify`（通用 aidog-notify.py）：N2 hook 事件通知，所有 CC 事件共用，读 stdin
///   取 hook_event_name + 事件字段 POST `{event, vars}`（不传 type，后端按 per_event 解析）。
fn generate_hook_scripts(
    invoker: gateway::scripts::ScriptInvoker,
) -> Result<gateway::hooks::ScriptPaths, String> {
    let scripts_dir = aidog_scripts_dir()?;
    let chmod755 = |path: &std::path::Path, filename: &str| -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)
                .map_err(|e| format!("stat hook script {filename}: {e}"))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms)
                .map_err(|e| format!("chmod hook script {filename}: {e}"))?;
        }
        #[cfg(not(unix))]
        let _ = (path, filename);
        Ok(())
    };
    let write_type_script = |filename: &str, legacy: &str, notif_type: &str| -> Result<String, String> {
        let path = scripts_dir.join(filename);
        let content = gateway::hooks::build_hook_script(notif_type);
        std::fs::write(&path, &content).map_err(|e| format!("write hook script {filename}: {e}"))?;
        chmod755(&path, filename)?;
        // 迁移清理：删除 ~/.aidog/ 根下旧版 bash 脚本（避免残留）。
        cleanup_legacy_root_script(legacy);
        Ok(invoker.command_for(&path.to_string_lossy()))
    };
    // 通用事件脚本（N2）：读 stdin hook_event_name，无内插 type。
    let event_path = scripts_dir.join(gateway::hooks::SCRIPT_EVENT_NOTIFY);
    std::fs::write(&event_path, gateway::hooks::build_event_notify_script())
        .map_err(|e| format!("write event notify script: {e}"))?;
    chmod755(&event_path, gateway::hooks::SCRIPT_EVENT_NOTIFY)?;
    let event_notify = invoker.command_for(&event_path.to_string_lossy());

    // waiting 脚本已并入通用事件脚本（N2），不再生成；仅清理历史 ~/.aidog/*.sh 残留。
    cleanup_legacy_root_script(gateway::hooks::LEGACY_SCRIPT_WAITING);

    Ok(gateway::hooks::ScriptPaths {
        complete: write_type_script(
            gateway::hooks::SCRIPT_COMPLETE,
            gateway::hooks::LEGACY_SCRIPT_COMPLETE,
            "task_complete",
        )?,
        event_notify,
    })
}

/// 从 NotificationSettings 解析 enabled 的 CC hook 事件名列表（用于注入遍历）。
/// per_event 为空（旧配置/未配）时回退默认精选 ON 集，保证总开关开时有事件可注入。
async fn enabled_hook_events(db: &Db) -> Vec<String> {
    let settings = gateway::db::get_notification_settings(db).await;
    if settings.per_event.is_empty() {
        return gateway::models::DEFAULT_ON_EVENTS.iter().map(|s| s.to_string()).collect();
    }
    settings
        .per_event
        .iter()
        .filter(|(_, es)| es.enabled)
        .map(|(name, _)| name.clone())
        .collect()
}

/// 把内置默认模板物化进 NotificationSettings.per_type[task_complete/waiting_input]（仅在缺失/空时填）。
/// 用户已自定义模板则不覆盖。
async fn seed_default_templates(db: &Db) -> Result<(), String> {
    use gateway::models::{NotifType, TypeSetting};
    let mut settings = gateway::db::get_notification_settings(db).await;
    let mut changed = false;
    for t in [NotifType::TaskComplete, NotifType::WaitingInput] {
        let key = t.as_str().to_string();
        let entry = settings.per_type.entry(key).or_insert_with(TypeSetting::default);
        if entry.template.trim().is_empty() {
            entry.template = t.default_template().to_string();
            changed = true;
        }
    }
    if changed {
        gateway::db::set_setting(db, SetSettingInput {
            scope: "notification".to_string(),
            key: "settings".to_string(),
            value: serde_json::to_value(&settings).map_err(|e| format!("serialize notification settings: {e}"))?,
        }).await?;
    }
    Ok(())
}

/// 一键注入通知 hook。
/// - `client="claude_code"`：把 hooks.Stop/Notification 注入基线 `claude_code` 配置，
///   re-sync 物化到所有 `settings.{group}.json`（与 statusLine 同机制）。
/// - `client="codex"`：把 `notify=[<complete 脚本>]` 注入 `~/.codex/config.toml`。
/// 同时物化内置默认模板。`group` 入参用于 API 对称（Claude Code hooks 走基线对全分组生效）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn inject_hooks(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    group: String,
    client: String,
) -> Result<(), String> {
    tracing::debug!(command = "inject_hooks", group = %group, client = %client, "command invoked");
    let hook_client = gateway::hooks::HookClient::from_str(&client)?;
    let invoker = resolve_script_invoker(&db).await;
    let scripts = generate_hook_scripts(invoker)?;
    seed_default_templates(&db).await?;

    match hook_client {
        gateway::hooks::HookClient::ClaudeCode => {
            // 读基线 claude_code 配置（无则用编译内默认）注入 hooks，回写 + re-sync。
            let mut config = gateway::db::get_setting(&db, "global", "claude_code").await
                .ok().flatten()
                .filter(|v| v.is_object())
                .unwrap_or_else(|| serde_json::from_str(include_str!("../defaults/settings.json"))
                    .unwrap_or(serde_json::Value::Object(Default::default())));
            let events = enabled_hook_events(&db).await;
            gateway::hooks::inject_claude_code_hooks(&mut config, &scripts, &events);
            gateway::db::set_setting(&db, SetSettingInput {
                scope: "global".to_string(),
                key: "claude_code".to_string(),
                value: config,
            }).await?;
            let port = load_proxy_settings(&app).await?.port;
            do_sync_group_settings(&db, port).await
                .map_err(|e| { tracing::error!(command = "inject_hooks", error = %e, "re-sync after inject failed"); e })?;
        }
        gateway::hooks::HookClient::Codex => {
            let mut config = gateway::codex::codex_config_read()?;
            gateway::hooks::inject_codex_notify(&mut config, &scripts.complete);
            gateway::codex::codex_config_write(config)?;
        }
    }
    Ok(())
}

/// 一键移除通知 hook（strip）。client 同 inject_hooks。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn remove_hooks(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    group: String,
    client: String,
) -> Result<(), String> {
    tracing::debug!(command = "remove_hooks", group = %group, client = %client, "command invoked");
    let hook_client = gateway::hooks::HookClient::from_str(&client)?;
    match hook_client {
        gateway::hooks::HookClient::ClaudeCode => {
            let Some(mut config) = gateway::db::get_setting(&db, "global", "claude_code").await
                .ok().flatten().filter(|v| v.is_object()) else {
                // 无基线配置 → 无 aidog hook 可清，re-sync 即可（settings 文件 strip 已生效）。
                let port = load_proxy_settings(&app).await?.port;
                return do_sync_group_settings(&db, port).await.map(|_| ());
            };
            gateway::hooks::remove_claude_code_hooks(&mut config);
            gateway::db::set_setting(&db, SetSettingInput {
                scope: "global".to_string(),
                key: "claude_code".to_string(),
                value: config,
            }).await?;
            let port = load_proxy_settings(&app).await?.port;
            do_sync_group_settings(&db, port).await
                .map_err(|e| { tracing::error!(command = "remove_hooks", error = %e, "re-sync after remove failed"); e })?;
        }
        gateway::hooks::HookClient::Codex => {
            let mut config = gateway::codex::codex_config_read()?;
            gateway::hooks::remove_codex_notify(&mut config);
            gateway::codex::codex_config_write(config)?;
        }
    }
    Ok(())
}

/// 读取「默认为所有分组注入通知 hook」总开关状态（基线 `claude_code._aidog_hooks.enabled`）。
/// 无基线配置时回退编译内默认（defaults/settings.json 默认开）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn get_default_hooks_enabled(db: State<'_, Db>) -> Result<bool, String> {
    tracing::debug!(command = "get_default_hooks_enabled", "command invoked");
    let config = gateway::db::get_setting(&db, "global", "claude_code").await
        .ok().flatten()
        .filter(|v| v.is_object() && v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(|| serde_json::from_str(include_str!("../defaults/settings.json"))
            .unwrap_or(serde_json::Value::Object(Default::default())));
    Ok(gateway::hooks::hooks_marker_enabled(&config))
}

/// 设置「默认为所有分组注入通知 hook」总开关：写基线 `claude_code._aidog_hooks.enabled`，
/// re-sync 物化（开=全分组 CC hooks + Codex notify；关=全移除）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn set_default_hooks_enabled(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    enabled: bool,
) -> Result<(), String> {
    tracing::debug!(command = "set_default_hooks_enabled", enabled, "command invoked");
    // 读基线 claude_code 配置（无则用编译内默认），设置 marker，回写。
    let mut config = gateway::db::get_setting(&db, "global", "claude_code").await
        .ok().flatten()
        .filter(|v| v.is_object() && v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(|| serde_json::from_str(include_str!("../defaults/settings.json"))
            .unwrap_or(serde_json::Value::Object(Default::default())));
    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            gateway::hooks::MARKER_HOOKS.to_string(),
            serde_json::json!({ "enabled": enabled }),
        );
    }
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "global".to_string(),
        key: "claude_code".to_string(),
        value: config,
    }).await?;
    // 开启时确保默认模板已物化（与 inject_hooks 行为一致）。
    if enabled {
        seed_default_templates(&db).await?;
    }
    let port = load_proxy_settings(&app).await?.port;
    do_sync_group_settings(&db, port).await
        .map_err(|e| { tracing::error!(command = "set_default_hooks_enabled", error = %e, "re-sync after set default hooks failed"); e })?;
    Ok(())
}

/// 构造通知 hook 片段供前端 Hooks 编辑器并入草稿（只读式）。
/// - 确保 notify 脚本已落盘 `~/.aidog/scripts/`（`generate_hook_scripts`）。
/// - 在空对象上走 `inject_claude_code_hooks`，取出其 `hooks` 子对象
///   （`{Stop:[...], Notification:[...]}`）返回。
/// **不写 DB、不 sync**：物化由用户正常保存触发既有链路。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn build_notify_hooks_fragment(db: State<'_, Db>) -> Result<serde_json::Value, String> {
    tracing::debug!(command = "build_notify_hooks_fragment", "command invoked");
    let invoker = resolve_script_invoker(&db).await;
    let scripts = generate_hook_scripts(invoker)?;
    let events = enabled_hook_events(&db).await;
    let mut config = serde_json::json!({});
    gateway::hooks::inject_claude_code_hooks(&mut config, &scripts, &events);
    Ok(config
        .get("hooks")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(Default::default())))
}

// ─── Settings Persistence ──────────────────────────────────

/// 统一数据目录：~/.aidog/
fn aidog_data_dir() -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let dir = home.join(".aidog");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create ~/.aidog: {e}"))?;
    Ok(dir)
}

/// 生成脚本目录：~/.aidog/scripts/（hook / statusline 脚本统一存放，不再 ~/.aidog/ 根）。
fn aidog_scripts_dir() -> Result<std::path::PathBuf, String> {
    let dir = aidog_data_dir()?.join("scripts");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create ~/.aidog/scripts: {e}"))?;
    Ok(dir)
}

/// 删除 ~/.aidog/ 根下遗留的旧脚本文件（迁移到 scripts/ 后清理，避免残留 stale 路径）。
/// best-effort：删除失败仅记录，不阻断。
fn cleanup_legacy_root_script(filename: &str) {
    if let Ok(root) = aidog_data_dir() {
        let legacy = root.join(filename);
        if legacy.exists() {
            if let Err(e) = std::fs::remove_file(&legacy) {
                tracing::warn!(file = %filename, error = %e, "cleanup legacy ~/.aidog script failed");
            }
        }
    }
}

/// 删除 ~/.aidog/scripts/ 下遗留的旧脚本文件（statusline 由 .sh 迁 .py，清理同目录旧 .sh）。
/// best-effort：删除失败仅记录，不阻断。
fn cleanup_legacy_scripts_dir_file(scripts_dir: &std::path::Path, filename: &str) {
    let legacy = scripts_dir.join(filename);
    if legacy.exists() {
        if let Err(e) = std::fs::remove_file(&legacy) {
            tracing::warn!(file = %filename, error = %e, "cleanup legacy scripts/ .sh failed");
        }
    }
}

// ─── uv / python3 执行器探测与安装（R2/R4） ────────────────

/// 探测 uv 是否可用（`uv --version` 成功退出）。无副作用，仅读环境。
fn detect_uv() -> bool {
    std::process::Command::new("uv")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 解析当前应使用的脚本执行器。
///
/// 优先用户持久化选择（`app/script_executor` = "uv" | "python3"）；未持久化时按 live
/// 探测（uv 可用 → uv，否则 python3）。生成脚本 command 串时调用，保证 hook / statusline /
/// codex 一致。
async fn resolve_script_invoker(db: &Db) -> gateway::scripts::ScriptInvoker {
    use gateway::scripts::ScriptInvoker;
    if let Ok(Some(v)) = db::get_setting(db, "app", "script_executor").await {
        if let Some(s) = v.as_str() {
            return ScriptInvoker::from_setting(Some(s));
        }
    }
    ScriptInvoker::from_uv_available(detect_uv())
}

/// 检测 uv 可用性（前端 uv 询问 modal 用）。返回 `true` 表示 uv 已安装。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
fn check_uv() -> Result<bool, String> {
    tracing::debug!(command = "check_uv", "command invoked");
    Ok(detect_uv())
}

/// 持久化用户的脚本执行器选择（"uv" | "python3"），供后续脚本生成读取，避免每次询问。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn set_script_executor(executor: String, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "set_script_executor", executor = %executor, "command invoked");
    // 经 ScriptInvoker 规范化（"uv" → uv，其余 → python3），保证存库值与解析一致。
    let normalized = gateway::scripts::ScriptInvoker::from_setting(Some(&executor)).as_setting();
    db::set_setting(&db, SetSettingInput {
        scope: "app".to_string(),
        key: "script_executor".to_string(),
        value: serde_json::Value::String(normalized.to_string()),
    }).await
}

/// 自动安装 uv（用户在 modal 选择「是」后调用）。
///
/// 走官方安装脚本 `curl -LsSf https://astral.sh/uv/install.sh | sh`（Unix）。
/// 成功后持久化选择为 "uv"。Windows 暂不支持自动安装（返回错误，由前端引导手动）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn install_uv(db: State<'_, Db>) -> Result<bool, String> {
    tracing::debug!(command = "install_uv", "command invoked");
    if detect_uv() {
        // 已安装 → 直接记录选择。
        db::set_setting(&db, SetSettingInput {
            scope: "app".to_string(),
            key: "script_executor".to_string(),
            value: serde_json::Value::String("uv".to_string()),
        }).await?;
        return Ok(true);
    }

    #[cfg(unix)]
    {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg("curl -LsSf https://astral.sh/uv/install.sh | sh")
            .output()
            .map_err(|e| { tracing::error!(command = "install_uv", error = %e, "spawn uv installer failed"); format!("spawn uv installer: {e}") })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(command = "install_uv", stderr = %stderr, "uv install script failed");
            return Err(format!("uv install failed: {}", stderr.trim()));
        }
        // 官方脚本装到 ~/.local/bin（或 ~/.cargo/bin）；detect_uv 依赖 PATH，可能本进程
        // PATH 未含安装目录 → 这里以「脚本退出成功」为准记录选择，运行时 hook 由用户 shell PATH 解析 uv。
        db::set_setting(&db, SetSettingInput {
            scope: "app".to_string(),
            key: "script_executor".to_string(),
            value: serde_json::Value::String("uv".to_string()),
        }).await?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        let _ = &db;
        Err("auto-install uv is only supported on Unix; please install uv manually".to_string())
    }
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
fn read_claude_code_settings() -> Result<serde_json::Value, String> {
    tracing::debug!(command = "read_claude_code_settings", "command invoked");
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        tracing::warn!(command = "read_claude_code_settings", "~/.claude/settings.json not found");
        return Err("~/.claude/settings.json not found".into());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| { tracing::warn!(command = "read_claude_code_settings", error = %e, "read settings failed"); format!("read settings: {e}") })?;
    serde_json::from_str(&content).map_err(|e| { tracing::warn!(command = "read_claude_code_settings", error = %e, "parse settings failed"); format!("parse settings: {e}") })
}

/// Load app log settings from DB (must be called after init_tables)
async fn load_app_log_settings_from_db(db: &Db) -> logging::AppLogSettings {
    db::get_setting(db, "app", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Load app log settings from file (pre-DB, uses defaults + env)
fn load_app_log_settings() -> logging::AppLogSettings {
    // Try loading from a simple JSON file before DB is available
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".aidog")
        .join("log_settings.json");
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(s) = serde_json::from_str(&data) {
                return s;
            }
        }
    }
    logging::AppLogSettings::default()
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn app_log_settings_get(db: State<'_, Db>) -> Result<logging::AppLogSettings, String> {
    tracing::debug!(command = "app_log_settings_get", "command invoked");
    Ok(load_app_log_settings_from_db(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn app_log_settings_set(settings: logging::AppLogSettings, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "app_log_settings_set", "command invoked");
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    db::set_setting(&db, SetSettingInput { scope: "app".to_string(), key: "logging".to_string(), value }).await
        .map_err(|e| { tracing::error!(command = "app_log_settings_set", error = %e, "persist log settings failed"); e })?;
    // Also persist to file so it's available before DB init on next startup
    if let Some(dir) = dirs::home_dir() {
        let path = dir.join(".aidog").join("log_settings.json");
        if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&settings).unwrap_or_default()) {
            tracing::warn!(command = "app_log_settings_set", error = %e, "write log_settings.json failed");
        }
    }
    Ok(())
}

// ─── Model Price Commands ──────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn model_price_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_list", limit, offset, "command invoked");
    gateway::db::list_model_prices(&db, limit, offset).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn model_price_count(db: State<'_, Db>) -> Result<u32, String> {
    tracing::debug!(command = "model_price_count", "command invoked");
    gateway::db::count_model_prices(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn model_price_search(db: State<'_, Db>, query: String, limit: u32) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_search", query = %query, limit, "command invoked");
    gateway::db::search_model_prices(&db, &query, limit).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn model_price_list_filtered(
    db: State<'_, Db>,
    query: Option<String>,
    source: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_list_filtered", limit, offset, "command invoked");
    gateway::db::filtered_list_model_prices(&db, query.as_deref(), source.as_deref(), limit, offset).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn model_price_count_filtered(
    db: State<'_, Db>,
    query: Option<String>,
    source: Option<String>,
) -> Result<u32, String> {
    tracing::debug!(command = "model_price_count_filtered", "command invoked");
    gateway::db::filtered_count_model_prices(&db, query.as_deref(), source.as_deref()).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn model_price_resolve(
    db: State<'_, Db>,
    model_name: String,
    platform_type: String,
    input_tokens: Option<i64>,
) -> Result<gateway::models::ResolvedPrice, String> {
    let input_tokens = input_tokens.unwrap_or(0);
    tracing::debug!(command = "model_price_resolve", model_name = %model_name, platform_type = %platform_type, input_tokens, "command invoked");
    let settings = gateway::price_sync::get_sync_settings(&db).await;
    gateway::db::resolve_price(&db, &model_name, &platform_type, settings.fallback_input_price, settings.fallback_output_price, input_tokens).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn model_price_sync(db: State<'_, Db>) -> Result<gateway::models::PriceSyncResult, String> {
    tracing::debug!(command = "model_price_sync", "command invoked");
    gateway::price_sync::sync_github_prices(&db).await
        .map_err(|e| { tracing::error!(command = "model_price_sync", error = %e, "model price sync failed"); e })
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn price_sync_settings_get(db: State<'_, Db>) -> Result<gateway::models::PriceSyncSettings, String> {
    tracing::debug!(command = "price_sync_settings_get", "command invoked");
    Ok(gateway::price_sync::get_sync_settings(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
async fn price_sync_settings_set(db: State<'_, Db>, settings: gateway::models::PriceSyncSettings) -> Result<(), String> {
    tracing::debug!(command = "price_sync_settings_set", "command invoked");
    gateway::price_sync::save_sync_settings(&db, &settings).await;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ProxySettings {
    port: u16,
    autostart: bool,
    #[serde(default)]
    silent_launch: bool,
}

/// 从 DB 读取 proxy settings；首次运行时自动迁移 proxy_settings.json 文件
async fn load_proxy_settings(app: &tauri::AppHandle) -> Result<ProxySettings, String> {
    let db = app.try_state::<Db>()
        .map(|s| s.inner())
        .ok_or("db not initialized")?;

    // 从 DB 读取
    if let Some(val) = db::get_setting(db, "proxy", "settings").await? {
        let s: ProxySettings = serde_json::from_value(val)
            .map_err(|e| format!("parse proxy settings: {e}"))?;
        return Ok(s);
    }

    // DB 无记录：尝试从旧文件迁移
    let file_path = aidog_data_dir()?.join("proxy_settings.json");
    if file_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            if let Ok(s) = serde_json::from_str::<ProxySettings>(&content) {
                // 迁移到 DB
                if let Err(e) = save_proxy_settings_to_db(db, &s).await {
                    tracing::warn!(error = %e, "migrate proxy_settings.json to db failed");
                }
                // 删除旧文件
                if let Err(e) = std::fs::remove_file(&file_path) {
                    tracing::debug!(error = %e, "remove migrated proxy_settings.json failed");
                }
                return Ok(s);
            }
        }
    }

    // 默认值
    Ok(ProxySettings { port: 9876, autostart: true, silent_launch: false })
}

async fn save_proxy_settings_to_db(db: &Db, settings: &ProxySettings) -> Result<(), String> {
    let value = serde_json::to_value(settings)
        .map_err(|e| format!("serialize proxy settings: {e}"))?;
    db::set_setting(db, gateway::models::SetSettingInput {
        scope: "proxy".to_string(),
        key: "settings".to_string(),
        value,
    }).await
}

async fn save_proxy_settings(
    app: &tauri::AppHandle,
    port: u16,
    autostart: bool,
    silent_launch: bool,
) -> Result<(), String> {
    let db = app.try_state::<Db>()
        .map(|s| s.inner())
        .ok_or("db not initialized")?;
    let settings = ProxySettings { port, autostart, silent_launch };
    save_proxy_settings_to_db(db, &settings).await
}

// ─── Tray ──────────────────────────────────────────────────

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;

/// 托盘渲染（多 item）。从 settings tray config 读取 enabled items（按 order），
/// 每项独立颜色（三态）/ 字号 / line_mode，作为「一列」参与列对齐。
/// 多平台两行模式（iStat Menus 式）：第一行各列标签横排、第二行各列值横排，列上下对齐。
/// 列对齐用 NSTextTab（NSParagraphStyle tabStops），每列一个 tab stop（按列宽估位置）。
/// 全部 single 且无 two_line 列 → 退单行横排（separator 拼接）。
/// 纯文字无 emoji。GUI 实际渲染（暗亮模式对比度 / 列对齐 / 垂直居中）留用户验证。
///
/// 托盘单列：name（标签）+ value（值）+ 颜色（三态）+ 字号 + two_line（该列是否两行展示）。
/// - two_line=true：第一行该列显 name，第二行该列显 value。
/// - two_line=false（single）：第一行该列显 "name value"，第二行该列留空（tab 占位）。
#[derive(Debug, Clone)]
struct TrayColumn {
    name: String,
    value: String,
    color: TrayColor,
    // 以下 4 字段为 macOS 富文本渲染（set_tray_attributed_title）专属参数；
    // 非 macOS 走 fallback 纯文本路径不读取，故平台条件 allow(dead_code)。
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    font_size: f64,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    two_line: bool,
    /// "left" | "center" | "right"
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    align: String,
    /// 两行模式第二行对齐，None = 跟随 align
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    align_row2: Option<String>,
}

/// 托盘渲染布局：columns（数据列）+ gaps（列间间隙）。
/// gaps[i] = columns[i] 与 columns[i+1] 之间的间隙；None = 默认 2px 空白。
struct TrayLayout {
    columns: Vec<TrayColumn>,
    /// 长度 = columns.len() - 1（若 columns.len() ≥ 2）。
    /// None = 默认空白间隙；Some(text) = 自定义分隔符文本。
    gaps: Vec<Option<String>>,
}

/// 计算单个 platform item 的（名, 值）二元组。
/// display="coding" 或平台具 coding plan → 值=`{%}%`（剩余百分比）；否则 值=`{balance:.2}`。
fn platform_item_parts(platform: &Platform, display: &str) -> (String, String) {
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
async fn tray_layout(app: &tauri::AppHandle) -> TrayLayout {
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
                    tokens: 0, cache_rate: 0.0, cost: 0.0, total_requests: 0,
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
async fn tray_separator(app: &tauri::AppHandle) -> String {
    if let Some(db) = app.try_state::<Db>() {
        if let Ok(Some(config)) = db::get_tray_config(&db).await {
            return config.separator;
        }
    }
    default_separator_str()
}

fn default_separator_str() -> String { "  ".to_string() }

/// 菜单内 quota 项的纯文字概要（无颜色/字号，separator 拼接；每列横排 "名 值"）。
async fn tray_quota_text(app: &tauri::AppHandle) -> Option<String> {
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

async fn build_tray_menu(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
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

/// macOS 菜单栏 tray 文字字号（pt）。默认 set_title 用系统字号（偏大），
/// 这里用 NSStatusItem button 的 attributedTitle 设小号 NSFont（参考菜单栏紧凑文字，约 9pt）。
/// 两行（\n）由 NSFont 行高决定，配合居中段落样式保持紧凑。
#[cfg(target_os = "macos")]
const TRAY_FONT_SIZE: f64 = 9.0;

/// 去除浮点数格式化尾部多余的零：10.10 → "10.1", 0.00 → "0", 965.80 → "965.8"
fn trim_trailing_zeros(s: &str) -> String {
    if let Some(_pos) = s.find('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
    } else {
        s.to_string()
    }
}

/// 将 TrayColor（三态）解析为 NSColor：
/// - follow → labelColor（系统自适应明暗）
/// - preset red/green/orange → systemRed/Green/Orange（自适应明暗）
/// - custom → 解析 hex（#RRGGBB / RRGGBB），sRGB 构造；解析失败回退 labelColor
///   注意：custom 固定色在某些菜单栏主题下可读性差（前端已警告）。
#[cfg(target_os = "macos")]
fn resolve_tray_color(color: &TrayColor) -> objc2::rc::Retained<objc2_app_kit::NSColor> {
    use objc2_app_kit::NSColor;
    match color.mode.as_str() {
        "preset" => match color.value.as_str() {
            "red" => NSColor::systemRedColor(),
            "green" => NSColor::systemGreenColor(),
            "orange" => NSColor::systemOrangeColor(),
            _ => NSColor::labelColor(),
        },
        "custom" => {
            let hex = color.value.trim().trim_start_matches('#');
            if hex.len() == 6 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                ) {
                    return NSColor::colorWithSRGBRed_green_blue_alpha(
                        r as f64 / 255.0,
                        g as f64 / 255.0,
                        b as f64 / 255.0,
                        1.0,
                    );
                }
            }
            NSColor::labelColor()
        }
        // "follow" 及未知 → labelColor
        _ => NSColor::labelColor(),
    }
}

/// 估算列宽（pt）：以最长一行字符数 × 估字宽 + padding。
/// menuBarFont 近似等宽（CJK 全角约 1 字宽 = fontSize，ASCII 半角约 fontSize*0.6）。
/// 精确测量文本渲染宽度：用 AppKit sizeWithAttributes 返回实际像素宽。
/// 需要 MainThread（AppKit 要求），调用方已在主线程闭包内。
#[cfg(target_os = "macos")]
fn measure_text_width(text: &str, font_size: f64) -> f64 {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSFont, NSFontAttributeName, NSStringDrawing};
    use objc2_foundation::{NSDictionary, NSString};

    let ns_text = NSString::from_str(text);
    let font = NSFont::boldSystemFontOfSize(font_size);
    let font_key: &NSString = unsafe { NSFontAttributeName };
    let font_obj: &AnyObject = (*font).as_ref();
    let attrs: Retained<NSDictionary<NSString, AnyObject>> =
        NSDictionary::from_slices(&[font_key], &[font_obj]);
    // SAFETY: attrs 类型正确（NSFontAttributeName → NSFont）。
    unsafe { ns_text.sizeWithAttributes(Some(&attrs)) }.width
}

/// macOS：用 attributedTitle 给 tray button 设多列小字（每列独立颜色/字号）。
/// Tauri/tray-icon 的 set_title 走 button.setTitle(NSString) 无字号/颜色控制，故直连 NSStatusItem button。
/// 通过 tauri TrayIcon::with_inner_tray_icon 拿 tray_icon::TrayIcon，再 ns_status_item() 取底层 NSStatusItem。
/// 闭包在主线程执行（with_inner_tray_icon 保证），满足 AppKit 主线程约束。
///
/// 布局（iStat Menus 式）：
/// - 有任一 two_line 列 → **两行多列模式**：
///   - 第一行各列：two_line→name；single→"name value"
///   - 第二行各列：two_line→value；single→""（占位，tab 推进保持列对齐）
///   - 列间 `\t`，行间一个 `\n`；NSParagraphStyle.tabStops 每列一个 NSTextTab(left, 累加列宽)
///   - per-column 着色/字号：逐 cell 用 make_part 构造带 attributes 的子串 append，
///     tab/换行字符用 follow 颜色（无 range:setAttributes，规避 utf16 偏移坑）。
/// - 无 two_line 列 → **单行模式**：沿用 separator 横排拼接（无回归）。
///   整串套用同一 NSParagraphStyle（tabStops + 固定行高居中）+ baselineOffset 垂直居中。
#[cfg(target_os = "macos")]
fn set_tray_attributed_title(
    tray: &tauri::tray::TrayIcon,
    columns: Vec<TrayColumn>,
    gaps: Vec<Option<String>>,
    _separator: String,
) -> Result<(), String> {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSParagraphStyleAttributeName};
    use objc2_app_kit::{NSMutableParagraphStyle, NSTextAlignment, NSTextTab, NSTextTabType};
    use objc2_app_kit::NSBaselineOffsetAttributeName;
    use objc2_foundation::{NSArray, NSAttributedString, NSMutableAttributedString, NSDictionary, NSNumber, NSString};
    use objc2::AnyThread;

    tray.with_inner_tray_icon(move |inner| -> Result<(), String> {
        // SAFETY: with_inner_tray_icon 在主线程执行闭包，AppKit 调用满足主线程要求。
        let status_item = inner
            .ns_status_item()
            .ok_or_else(|| "ns_status_item unavailable".to_string())?;
        // MainThreadMarker：闭包已在主线程，断言获取。
        let mtm = objc2_foundation::MainThreadMarker::new()
            .ok_or_else(|| "not on main thread".to_string())?;
        let button = status_item
            .button(mtm)
            .ok_or_else(|| "status item has no button".to_string())?;

        let two_line_mode = columns.iter().any(|c| c.two_line);

        // 段落样式：两行模式压缩行高（min==max）让两行紧凑；单行模式不压缩，字号更大。
        // 两行：9pt × 2 行 ≈ 20pt，贴近菜单栏 ~22pt 高度。
        // 单行：13pt × 1 行，充分利用菜单栏垂直空间。
        let para = NSMutableParagraphStyle::new();
        // 两行模式用左对齐（tabStops 控制列位置）；单行模式居中。
        para.setAlignment(if two_line_mode {
            NSTextAlignment::Left
        } else {
            NSTextAlignment::Center
        });
        let line_h = if two_line_mode {
            TRAY_FONT_SIZE + 5.0 // 两行模式，行间距 10px
        } else {
            0.0 // 单行不压缩行高，使用系统默认
        };
        if two_line_mode {
            para.setMinimumLineHeight(line_h);
            para.setMaximumLineHeight(line_h);
            para.setLineSpacing(0.0);
        }

        // 两行模式：两行共用同一个段落样式（para），均使用 LeftTabStopType。
        // 列宽 = max(第一行该列文字, 第二行该列文字) 实测宽 + padding；位置累加（loc = 各列右边界）。
        // 对齐：通过在文本前填充空格实现右/居中对齐（精确测量 + 空格宽度推算）。
        // 两行都用 left tab @列右边界 → 同一列两行起始位置相同 → 列边界对齐。
        let mut col_widths: Vec<f64> = Vec::new();
        if two_line_mode {
            const COL_PADDING: f64 = 5.0; // 列间最小间距 5px
            let mut left_tabs: Vec<Retained<NSTextTab>> = Vec::new();
            let mut loc: f64 = 0.0;
            for col in columns.iter() {
                let line1 = if col.two_line {
                    col.name.clone()
                } else {
                    format!("{} {}", col.name, col.value)
                };
                let line2 = if col.two_line { col.value.clone() } else { String::new() };
                let w1 = measure_text_width(&line1, TRAY_FONT_SIZE);
                let w2 = measure_text_width(&line2, TRAY_FONT_SIZE + 3.0);
                let col_w = w1.max(w2) + COL_PADDING;
                col_widths.push(col_w);
                loc += col_w;
                left_tabs.push(NSTextTab::initWithType_location(
                    NSTextTab::alloc(),
                    NSTextTabType::LeftTabStopType,
                    loc,
                ));
            }
            let left_array: Retained<NSArray<NSTextTab>> = NSArray::from_retained_slice(&left_tabs);
            para.setTabStops(Some(&left_array));
        }

        // 根据对齐设置在文本前填充空格：right → 左侧填充至列宽；center → 两侧填充。
        let align_text = |text: &str, col_w: f64, font_size: f64, align: &str| -> String {
            if align == "left" || text.is_empty() {
                return text.to_string();
            }
            let text_w = measure_text_width(text, font_size);
            let space_w = measure_text_width(" ", font_size);
            if space_w <= 0.0 { return text.to_string(); }
            let extra = (col_w - text_w).max(0.0);
            let n_spaces = (extra / space_w).round() as usize;
            match align {
                "right" => format!("{}{}", " ".repeat(n_spaces), text),
                "center" => {
                    let half = n_spaces / 2;
                    format!("{}{}{}", " ".repeat(half), text, " ".repeat(n_spaces - half))
                }
                _ => text.to_string(),
            }
        };

        // baselineOffset：两行模式需要负偏移下推居中；单行模式无需偏移。
        let baseline_offset = NSNumber::new_f64(if two_line_mode { -7.0 } else { -5.0 });

        // 单行模式：每列字号覆盖为更大值（只有一行，充分利用菜单栏高度）。
        let single_line_font_size: f64 = 13.0;

        use objc2::runtime::AnyObject;
        let para_key: &NSString = unsafe { NSParagraphStyleAttributeName };
        let baseline_key: &NSString = unsafe { NSBaselineOffsetAttributeName };
        let font_key: &NSString = unsafe { NSFontAttributeName };
        let color_key: &NSString = unsafe { NSForegroundColorAttributeName };

        // 构造单段 attributed string（文字 + 字号 + 颜色 + 指定段落/baseline）。
        // 两行模式：标签行和值行共用 `para`（LeftTabStopType），列边界自然对齐。
        let make_part = |text: &str, font_size: f64, color: &TrayColor, para_style: &NSMutableParagraphStyle| -> Retained<NSAttributedString> {
            let ns_text = NSString::from_str(text);
            let font: Retained<NSFont> = NSFont::boldSystemFontOfSize(font_size);
            let ns_color = resolve_tray_color(color);

            let keys: [&NSString; 4] = [font_key, color_key, para_key, baseline_key];
            let font_obj: &AnyObject = (*font).as_ref();
            let color_obj: &AnyObject = (*ns_color).as_ref();
            let para_obj: &AnyObject = para_style.as_ref();
            let baseline_obj: &AnyObject = (*baseline_offset).as_ref();
            let objects: [&AnyObject; 4] = [font_obj, color_obj, para_obj, baseline_obj];
            let attrs: Retained<NSDictionary<NSString, objc2::runtime::AnyObject>> =
                NSDictionary::from_slices(&keys, &objects);
            // SAFETY: attrs 键为 NSAttributedStringKey(NSString)、值为合法 AppKit 对象，类型正确。
            unsafe {
                NSAttributedString::initWithString_attributes(
                    NSAttributedString::alloc(),
                    &ns_text,
                    Some(&attrs),
                )
            }
        };

        let follow_color = TrayColor::default(); // mode=follow（tab/换行/separator 用）
        let result = NSMutableAttributedString::new();

        if two_line_mode {
            let _default_gap = " ".to_string();
            // 第一行（标签行）：各列首段，列间 \t + gap 文字。整行用 `para`（left tab）。
            for (idx, col) in columns.iter().enumerate() {
                if idx > 0 {
                    result.appendAttributedString(&make_part("\t", TRAY_FONT_SIZE, &follow_color, &para));
                    let gap_text = gaps.get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_default();
                    if !gap_text.is_empty() {
                        result.appendAttributedString(&make_part(&gap_text, TRAY_FONT_SIZE, &follow_color, &para));
                    }
                }
                let line1 = if col.two_line {
                    col.name.clone()
                } else {
                    format!("{} {}", col.name, col.value)
                };
                let col_w = col_widths.get(idx).copied().unwrap_or(0.0);
                let aligned = align_text(&line1, col_w, TRAY_FONT_SIZE, &col.align);
                result.appendAttributedString(&make_part(&aligned, TRAY_FONT_SIZE, &col.color, &para));
            }
            // 行间换行
            let nl_font = columns.first().map(|c| c.font_size).unwrap_or(TRAY_FONT_SIZE);
            result.appendAttributedString(&make_part("\n", nl_font, &follow_color, &para));
            // 第二行（值行）：与标签行相同结构，对齐取 align_row2（fallback align）。字体比标签行大1pt。
            for (idx, col) in columns.iter().enumerate() {
                let row2_font = TRAY_FONT_SIZE + 3.0;
                if idx > 0 {
                    result.appendAttributedString(&make_part("\t", row2_font, &follow_color, &para));
                    let gap_text = gaps.get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_default();
                    if !gap_text.is_empty() {
                        result.appendAttributedString(&make_part(&gap_text, row2_font, &follow_color, &para));
                    }
                }
                let line2 = if col.two_line { col.value.clone() } else { String::new() };
                if !line2.is_empty() {
                    let row2_align = col.align_row2.as_deref().unwrap_or(&col.align);
                    let col_w = col_widths.get(idx).copied().unwrap_or(0.0);
                    let aligned = align_text(&line2, col_w, row2_font, row2_align);
                    result.appendAttributedString(&make_part(&aligned, row2_font, &col.color, &para));
                }
            }
        } else {
            // 单行模式：每列 "名 值"，列间用 gap 拼接。字号加大（只有一行，充分利用菜单栏高度）。
            let default_gap = " ".to_string();
            let join_font = single_line_font_size;
            for (idx, col) in columns.iter().enumerate() {
                if idx > 0 {
                    let gap_text = gaps.get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_else(|| default_gap.clone());
                    result.appendAttributedString(&make_part(&gap_text, join_font, &follow_color, &para));
                }
                let text = format!("{} {}", col.name, col.value);
                result.appendAttributedString(&make_part(&text, single_line_font_size, &col.color, &para));
            }
        }

        button.setAttributedTitle(&result);
        Ok(())
    })
    .map_err(|e| e.to_string())?
}

async fn refresh_tray_menu(app: &tauri::AppHandle) -> Result<(), String> {
    let tray = app.tray_by_id("main").ok_or("tray not found")?;
    let menu = build_tray_menu(app).await?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    // macOS 菜单栏：有 quota 值时隐藏 logo + 两行小字 title；无值时恢复 logo + 清 title。
    // 非 macOS 平台仅 menu item 降级（不调 set_title / set_icon）。
    #[cfg(target_os = "macos")]
    {
        let layout = tray_layout(app).await;
        if layout.columns.is_empty() {
            tray.set_icon(app.default_window_icon().cloned())
                .map_err(|e| e.to_string())?;
            tray.set_title(None::<&str>).map_err(|e| e.to_string())?;
        } else {
            let separator = tray_separator(app).await;
            tray.set_icon(None).map_err(|e| e.to_string())?;
            // 兜底文字：各列 "名 值"，间隙用 separator
            let fallback_text = layout.columns
                .iter()
                .map(|c| format!("{} {}", c.name, c.value))
                .collect::<Vec<_>>()
                .join(separator.as_str());
            tray.set_title(Some(&fallback_text)).map_err(|e| e.to_string())?;
            if let Err(e) = set_tray_attributed_title(&tray, layout.columns, layout.gaps, separator) {
                tracing::warn!("tray attributed title failed, fallback to default font: {e}");
            }
        }
    }
    Ok(())
}

// ─── App Entry ─────────────────────────────────────────────

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // 初始化日志（尽早，在 DB 之前）
            let data_dir = aidog_data_dir().expect("failed to resolve data dir");
            let log_settings = load_app_log_settings();
            logging::init_logging(&data_dir, &log_settings);
            logging::cleanup_old_logs(&data_dir, log_settings.retention_hours);

            // 在 data dir 创建 SQLite
            let db_path = data_dir.join("aidog.db");
            let db = tauri::async_runtime::block_on(async {
                let db = Db::new(db_path.to_str().unwrap()).await.expect("failed to open database");
                db.init_tables().await.expect("failed to init tables");
                // 为所有平台确保存在关联分组（一个平台一个）
                ensure_platform_groups(&db).await;
                db
            });
            app.manage(db);

            // 启动时同步所有 settings 文件（检查不一致并更新）
            {
                let handle = app.handle();
                let db_state = app.state::<Db>();
                tauri::async_runtime::block_on(try_sync_settings(handle, &db_state));
            }

            // 中间件规则引擎单例（C1）：启动时从 DB 加载规则建缓存；CRUD command 写后 reload。
            {
                let engine = Arc::new(MiddlewareEngine::new());
                let db_state = app.state::<Db>();
                if let Err(e) = tauri::async_runtime::block_on(engine.reload(&db_state)) {
                    tracing::warn!(error = %e, "middleware engine initial load failed");
                }
                app.manage(engine);
            }

            app.manage(ProxyHandle(StdMutex::new(None)));

            // 定时备份调度器 (spawn_scheduler 内部 spawn 常驻 loop, 启动首次检查补「关机错过」)。
            gateway::backup::spawn_scheduler(app.handle().clone());

            // 通知授权（①）：启动时请求一次系统通知权限。
            // desktop 上 tauri-plugin-notification 为 no-op 返回 Granted（无害）；
            // mobile 会真实弹原生授权框。失败仅 warn，不 panic、不阻塞启动。
            {
                use tauri_plugin_notification::NotificationExt;
                match app.notification().request_permission() {
                    Ok(state) => tracing::info!("notify: request_permission state={:?}", state),
                    Err(e) => tracing::warn!(error = %e, "notify: request_permission failed"),
                }
            }

            // 系统托盘
            let menu = tauri::async_runtime::block_on(build_tray_menu(app.handle()))?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&menu)
                .tooltip("AiDog — AI API Gateway")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::{MouseButton, MouseButtonState};
                    if let tauri::tray::TrayIconEvent::Click { button, button_state, rect, .. } = event {
                        // 只响应 Down，忽略 Up（否则 Down 创建 → Up 立刻销毁）
                        if button != MouseButton::Left || button_state != MouseButtonState::Down { return; }
                        let app = tray.app_handle().clone();
                        tracing::info!(button = ?button, "tray click → toggle popover");
                        // 切换：已打开则关闭
                        if let Some(w) = app.get_webview_window("popover") {
                            let _ = w.destroy();
                            return;
                        }
                        // 定位：居中于 tray 图标正下方
                        // rect 坐标为 Physical 像素，position() 接受 Logical 坐标，需除以 scale factor
                        let scale = app.get_webview_window("main")
                            .and_then(|w| w.scale_factor().ok())
                            .unwrap_or(2.0);
                        let (rx, ry) = match rect.position {
                            tauri::Position::Physical(p) => (p.x as f64 / scale, p.y as f64 / scale),
                            tauri::Position::Logical(p) => (p.x, p.y),
                        };
                        let (rw, rh) = match rect.size {
                            tauri::Size::Physical(s) => (s.width as f64 / scale, s.height as f64 / scale),
                            tauri::Size::Logical(s) => (s.width, s.height),
                        };
                        let pw = 300.0;
                        let ph = 420.0;
                        let x = rx + rw / 2.0 - pw / 2.0;
                        let y = ry + rh;
                        tracing::info!(x, y, pw, ph, scale, "popover position");
                        if let Err(e) = tauri::webview::WebviewWindowBuilder::new(
                            &app, "popover",
                            tauri::WebviewUrl::App("popover.html".into()),
                        )
                        .inner_size(pw, ph)
                        .position(x, y)
                        .decorations(false)
                        .transparent(true)
                        .always_on_top(true)
                        .skip_taskbar(true)
                        .focused(true)
                        .build()
                        {
                            tracing::error!(error = %e, "create popover failed");
                        } else {
                            tracing::info!("popover window created successfully");
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "proxy_start" => {
                        let settings = tauri::async_runtime::block_on(load_proxy_settings(app)).unwrap_or(ProxySettings {
                            port: 9876,
                            autostart: true,
                            silent_launch: false,
                        });
                        let port = settings.port;
                        tauri::async_runtime::block_on(async {
                            if let Err(e) = proxy_start(port, app.clone()).await {
                                tracing::error!(port, error = %e, "tray: proxy start failed");
                            }
                        });
                    }
                    "proxy_stop" => {
                        tauri::async_runtime::block_on(async {
                            if let Err(e) = proxy_stop(app.clone()).await {
                                tracing::error!(error = %e, "tray: proxy stop failed");
                            }
                        });
                    }
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app).map_err(|e| e.to_string())?;

            // 监听后台预估发出的 tray-refresh 事件，在主线程刷新托盘（避免后台线程直接操作 tray）
            {
                use tauri::Listener;
                let handle = app.handle().clone();
                app.listen("tray-refresh", move |_| {
                    let _ = tauri::async_runtime::block_on(refresh_tray_menu(&handle));
                });
            }

            // 自动启动代理
            let settings = tauri::async_runtime::block_on(load_proxy_settings(app.handle()))?;
            if settings.autostart {
                let port = settings.port;
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = proxy_start(port, handle).await {
                        tracing::error!(port, error = %e, "autostart: proxy start failed");
                    }
                });
            }

            // 冷启动 est 初始化：tray 平台从未真查（last_real_query_at==0）→ 后台真查对齐 est=真实。
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    cold_start_init_tray_estimates(&handle).await;
                });
            }

            // 静默启动：隐藏主窗口，仅托盘运行
            if settings.silent_launch {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Platform
            platform_create,
            platform_list,
            platform_get,
            platform_update,
            platform_delete,
            platform_set_tray,
            platform_fetch_models,
            // Tray Config
            tray_config_get,
            tray_config_set,
            tray_today_stats,
            popover_data,
            popover_config_get,
            popover_config_set,
            popover_platform_today,
            // Group
            group_create,
            group_list,
            group_get,
            group_update,
            group_delete,
            // GroupPlatform
            group_set_platforms,
            group_get_platforms,
            // Aggregate
            group_detail,
            group_detail_list,
            group_reorder,
            // Proxy
            proxy_start,
            proxy_stop,
            proxy_status,
            proxy_get_settings,
            proxy_set_autostart,
            app_set_autolaunch,
            app_get_autolaunch,
            app_set_silent_launch,
            // Proxy Client Settings
            proxy_client_get_settings,
            proxy_client_set_settings,
            // Config Export
            export_claude_config,
            sync_group_settings,
            // Proxy Logs
            proxy_log_list,
            proxy_log_list_filtered,
            proxy_log_count_filtered,
            proxy_log_get,
            proxy_log_clear,
            proxy_log_count,
            proxy_log_settings_get,
            proxy_log_settings_set,
            // Proxy Timeout
            proxy_timeout_get,
            proxy_timeout_set,
            // Middleware Rule Engine (C1)
            middleware_list_rules,
            middleware_create_rule,
            middleware_update_rule,
            middleware_delete_rule,
            middleware_settings_get,
            middleware_settings_set,
            scheduling_settings_get,
            scheduling_settings_set,
            // Notification (N1)
            notification_settings_get,
            notification_settings_set,
            notification_inbox_list,
            notification_clear,
            notification_test,
            notification_test_tts,
            notification_test_popup,
            notification_test_beep,
            // Notification Hook Integration (N2)
            inject_hooks,
            remove_hooks,
            get_default_hooks_enabled,
            set_default_hooks_enabled,
            build_notify_hooks_fragment,
            // 脚本执行器（uv / python3）
            check_uv,
            install_uv,
            set_script_executor,
            // Skills 管理
            skills_check_env,
            skills_browse_catalog,
            skills_search,
            skills_list_installed,
            skills_list_refresh,
            skills_enable,
            skills_install,
            skill_detail,
            skill_read_file,
            skills_disable,
            skills_update,
            skills_uninstall_all,
            skills_uninstall,
            skills_uninstall_group,
            skills_align_agents,
            skills_enable_all,
            skills_set_group_agent,
            // MCP 管理
            mcp_list,
            mcp_scan,
            mcp_import,
            mcp_set_agent,
            mcp_delete,
            mcp_update,
            mcp_add,
            mcp_resync,
            // 导入导出子系统
            export_to_file,
            backup_settings_get,
            backup_settings_set,
            backup_run_now,
            import_read_file,
            import_apply,
            ccswitch_detect,
            ccswitch_read,
            ccswitch_import,
            // App Logging
            app_log_settings_get,
            app_log_settings_set,
            // Settings
            fs_autocomplete,
            settings_get,
            settings_set,
            settings_delete,
            settings_list,
            generate_statusline_script,
            read_claude_code_settings,
            // Codex Config
            gateway::codex::codex_config_read,
            gateway::codex::codex_config_write,
            gateway::codex::codex_config_path,
            // Statistics
            stats_query,
            model_test,
            // Platform Usage
            platform_usage_stats,
            group_usage_stats,
            all_group_usage_stats,
            // Platform Quota
            platform_query_quota,
            platform_query_quota_newapi,
            platform_reorder,
            // Model Prices
            model_price_list,
            model_price_count,
            model_price_search,
model_price_list_filtered,
model_price_count_filtered,
            model_price_resolve,
            model_price_sync,
            price_sync_settings_get,
            price_sync_settings_set,
            // About
            about_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
