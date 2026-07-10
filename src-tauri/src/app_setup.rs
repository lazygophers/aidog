//! App setup（启动初始化逻辑）下沉自 lib.rs 的 run() setup 闭包，零行为变更。
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tauri::Manager;
use aidog_core::gateway::{self, db::Db};
use aidog_core::logging;
use aidog_core::shared::{aidog_data_dir, load_proxy_settings, ProxyHandle, ProxySettings};
use aidog_core::gateway::middleware::MiddlewareEngine;
use crate::commands::app_log::{load_app_log_settings_from_db, migrate_log_settings_file_to_db};
use aidog_core::sync_settings::try_sync_settings;
use crate::commands::coding_tools::ensure_default_coding_tools_settings;
use commands_proxy::proxy::{proxy_start, proxy_stop};
use commands_platform::tray::build_tray_menu;
use aidog_core::tray_render::refresh_tray_menu;
use commands_platform::quota::cold_start_init_tray_estimates;
use tauri::tray::TrayIconBuilder;

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
            // 最先修运行时 PATH：GUI(launchd/Finder) env 极简，brew/nvm/pyenv 装的
            // node/npx/python/uv 不在 PATH → skills 检测/安装/导入「环境缺失」。并入登录
            // shell PATH（幂等、静默、失败不阻断），覆盖后续全部子进程。须在任何子进程 spawn 前。
            gateway::skills::ensure_runtime_path();

            let data_dir = aidog_data_dir().expect("failed to resolve data dir");

            // 先开 DB 再初始化日志：app log 设置单一事实源 = DB settings 表（禁独立文件）。
            // 历史 ~/.aidog/log_settings.json 在此一次性迁移进 DB 后删除。
            let db_path = data_dir.join("aidog.db");
            let db = tauri::async_runtime::block_on(async {
                use tracing::Instrument;
                // 启动期 init：包进带真实唯一链路 id 的 span，init_tables 的建表 / 迁移 SQL
                // 经 call_traced 环境捕获带上该 id（非固定常量）。
                let init_span = tracing::info_span!("db_init", trace_id = %logging::new_trace_id());
                async {
                    let db = Db::new(db_path.to_str().unwrap()).await.expect("failed to open database");
                    db.init_tables().await.expect("failed to init tables");
                    // 自动建默认分组改为「创建平台时一次性判断」（见 platform_create），
                    // 不再在启动时为所有平台兜底建组（避免覆盖用户「不分组」选择）。
                    db
                }
                .instrument(init_span)
                .await
            });
            // 后台 auto_vacuum 迁移：老库（auto_vacuum=NONE）需 VACUUM 重建切到 INCREMENTAL
            // 才能回收 free pages。非阻塞——spawn 独立 task，失败仅 warn 不置标记，下次启动重试。
            // VACUUM 锁库期间代理写请求排队（busy_timeout=5000 兜底）。
            // Db::clone 廉价（仅 channel sender 共享同一后台线程连接），manage 前即可 spawn。
            {
                let db_clone = db.clone();
                tauri::async_runtime::spawn(async move {
                    use tracing::Instrument;
                    let span = tracing::info_span!("db_migrate_auto_vacuum", trace_id = %logging::new_trace_id());
                    async {
                        match gateway::db::migrate_auto_vacuum(&db_clone).await {
                            Ok(true) => tracing::info!("db auto_vacuum migration completed on startup"),
                            Ok(false) => tracing::debug!("db auto_vacuum migration skipped (already migrated or INCREMENTAL)"),
                            Err(e) => tracing::warn!(error = %e, "db auto_vacuum migration failed on startup, will retry next launch"),
                        }
                    }
                    .instrument(span)
                    .await
                });
            }
            // 一次性纠正聚合表虚高（agg 重复计数 bug，版本门控只跑一次）。非阻塞 spawn。
            {
                let db_clone = db.clone();
                tauri::async_runtime::spawn(async move {
                    use tracing::Instrument;
                    let span = tracing::info_span!("db_rebuild_stats_agg", trace_id = %logging::new_trace_id());
                    async {
                        match gateway::db::rebuild_stats_agg_once_if_needed(&db_clone).await {
                            Ok(true) => tracing::info!("stats_agg rebuilt from proxy_log (one-time dedup correction)"),
                            Ok(false) => tracing::debug!("stats_agg rebuild skipped (already corrected)"),
                            Err(e) => tracing::warn!(error = %e, "stats_agg one-time rebuild failed, will retry next launch"),
                        }
                    }
                    .instrument(span)
                    .await
                });
            }
            // 一次性纠正历史 count_tokens 计费污染（count_tokens 行曾计入 stats_agg，占全库 cost 17.6%）。
            // 排除 count_tokens 后覆盖写 + 删孤儿桶，版本门控只跑一次。非阻塞 spawn。
            {
                let db_clone = db.clone();
                tauri::async_runtime::spawn(async move {
                    use tracing::Instrument;
                    let span = tracing::info_span!("db_correct_count_tokens_agg", trace_id = %logging::new_trace_id());
                    async {
                        match gateway::db::correct_count_tokens_agg_once_if_needed(&db_clone).await {
                            Ok(true) => tracing::info!("stats_agg corrected: count_tokens contributions removed (one-time)"),
                            Ok(false) => tracing::debug!("stats_agg count_tokens correction skipped (already done)"),
                            Err(e) => tracing::warn!(error = %e, "stats_agg count_tokens correction failed, will retry next launch"),
                        }
                    }
                    .instrument(span)
                    .await
                });
            }
            app.manage(db);

            // 初始化日志（DB 已开，读 DB 设置；迁移遗留文件）
            let (log_settings, ) = tauri::async_runtime::block_on(async {
                let db_state = app.state::<Db>();
                migrate_log_settings_file_to_db(&db_state).await;
                (load_app_log_settings_from_db(&db_state).await,)
            });
            logging::init_logging(&data_dir, &log_settings);
            logging::cleanup_old_logs(&data_dir, log_settings.retention_hours);

            // 启动时同步所有 settings 文件（检查不一致并更新）
            {
                let handle = app.handle();
                let db_state = app.state::<Db>();
                tauri::async_runtime::block_on(try_sync_settings(handle, &db_state));
            }

            // 启动初始化 CC/Codex 联动开关：DB 无记录时视为默认开（写 ~/.claude/config.json
            // 与 ~/.claude.json），并落 DB true。开箱即生效，无需进设置页。
            // 失败仅 warn 不阻塞启动。
            {
                let db_state = app.state::<Db>();
                if let Err(e) = tauri::async_runtime::block_on(ensure_default_coding_tools_settings(&db_state)) {
                    tracing::warn!(error = %e, "ensure_default_coding_tools_settings failed");
                }
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

            // platform-presets.json 同步调度器（同 backup/scheduler.rs 模式）：单 spawn，启动首跑补
            // 「关机错过」+ 24h 循环。非阻塞 spawn，失败仅 warn；不破坏现有功能（child 1 reader
            // 端自动回退 bundled）。maybe_sync_on_startup 内部判 24h 节流，重复触发安全。
            tauri::async_runtime::spawn(async move {
                use tracing::Instrument;
                let startup_span =
                    tracing::info_span!("defaults_sync_startup", trace_id = %logging::new_trace_id());
                gateway::defaults_sync::maybe_sync_on_startup()
                    .instrument(startup_span)
                    .await;
                let interval = std::time::Duration::from_secs(24 * 3600);
                loop {
                    tokio::time::sleep(interval).await;
                    let cycle_span = tracing::info_span!(
                        "defaults_sync_daily",
                        trace_id = %logging::new_trace_id()
                    );
                    gateway::defaults_sync::maybe_sync_on_startup()
                        .instrument(cycle_span)
                        .await;
                }
            });

            // client-types.json 同步调度器（同 defaults_sync 模式：启动 hook + 24h 循环）。
            // 失败仅 warn；不破坏现有功能（reader 端自动回退 bundled）。
            tauri::async_runtime::spawn(async move {
                use tracing::Instrument;
                let startup_span =
                    tracing::info_span!("client_types_sync_startup", trace_id = %logging::new_trace_id());
                gateway::client_types_sync::maybe_sync_on_startup()
                    .instrument(startup_span)
                    .await;
                let interval = std::time::Duration::from_secs(24 * 3600);
                loop {
                    tokio::time::sleep(interval).await;
                    let cycle_span = tracing::info_span!(
                        "client_types_sync_daily",
                        trace_id = %logging::new_trace_id()
                    );
                    gateway::client_types_sync::maybe_sync_on_startup()
                        .instrument(cycle_span)
                        .await;
                }
            });

            // Protocol logo 后台批量同步：启动时预热 `~/.aidog/logos/<protocol>.png`，
            // 三路 fallback（simpleicons → favicon → clearbit），缓存命中跳过，不阻塞启动。
            // 非 DB 依赖预热场景：clone 现有 Db handle + app_data_dir 后 spawn，失败仅 debug log。
            {
                let db_state = app.state::<Db>();
                let db = std::sync::Arc::new(db_state.inner().clone());
                let dir = data_dir.clone();
                tauri::async_runtime::spawn(async move {
                    use tracing::Instrument;
                    let span = tracing::info_span!(
                        "logo_sync_startup",
                        trace_id = %logging::new_trace_id()
                    );
                    gateway::logo_sync::sync_all_logos(db, dir).instrument(span).await;
                });
            }

            // 内置每日定时清理：永久删除软删超过 3 天的平台行（deleted_at>0 且 < now-3d）。
            // 启动首跑补「关机错过」，之后每 24h 一轮；非关键路径，失败仅 warn。
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use tracing::Instrument;
                    let interval = std::time::Duration::from_secs(24 * 3600);
                    let older_than_secs: i64 = 3 * 24 * 3600;
                    loop {
                        // 每个清理周期一个真实唯一链路 id：本周期内所有 SQL 共享该 id（SQL 日志
                        // req= 经 call_traced 的环境捕获自动带上），不同周期 id 不同。
                        let cycle_span = tracing::info_span!(
                            "scheduled_cleanup",
                            trace_id = %logging::new_trace_id()
                        );
                        async {
                            if let Some(db) = handle.try_state::<Db>() {
                                match gateway::db::purge_all_soft_deleted(&db, older_than_secs).await {
                                    Ok(map) if !map.is_empty() && map.values().any(|&n| n > 0) => {
                                        tracing::info!(
                                            purged = ?map,
                                            "scheduled: purged old soft-deleted rows across all tables"
                                        );
                                    }
                                    Ok(_) => tracing::debug!(
                                        "scheduled: purge_all_soft_deleted ran, nothing to delete"
                                    ),
                                    Err(e) => tracing::warn!(
                                        error = %e,
                                        "scheduled: purge_all_soft_deleted failed (all tables errored)"
                                    ),
                                }
                                // 通知收件箱 retention 硬删（默认 7 天；inbox_retention_days=0 → 永不清理）。
                                let retention_days = gateway::db::get_notification_settings(&db).await.inbox_retention_days;
                                if let Err(e) = gateway::db::cleanup_notifications(&db, retention_days).await {
                                    tracing::warn!(error = %e, "scheduled: cleanup notifications failed");
                                }
                                // 聚合统计表 retention 硬删（默认 365 天；stats retention_days=0 → 永不清理）。
                                let stats_settings: gateway::models::StatsSettings = gateway::db::get_setting(&db, "stats", "settings").await
                                    .ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                                if let Err(e) = gateway::db::cleanup_stats_agg(&db, stats_settings.retention_days).await {
                                    tracing::warn!(error = %e, "scheduled: cleanup stats_agg failed");
                                }
                            }
                        }
                        .instrument(cycle_span)
                        .await;
                        tokio::time::sleep(interval).await;
                    }
                });
            }

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
                        match tauri::webview::WebviewWindowBuilder::new(
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
                        .build() {
                            Ok(w) => {
                                #[cfg(target_os = "macos")]
                                {
                                    // C1: NSWindow.hidesOnDeactivate —— app 失活自动隐藏 popover,
                                    // 覆盖 3 失活场景 (点桌面 / silent_launch 主窗 hide 后点别处 / 点
                                    // Dock 菜单栏空白). tao Focused(false) 只在 windowDidResignKey
                                    // 触发, floating popover 在 inactive app 不 resignKey → v1 handler
                                    // 单独不够. Apple docs:
                                    // https://developer.apple.com/documentation/appkit/nswindow/hidesondeactivate
                                    use objc2_app_kit::NSWindow;
                                    use objc2::rc::Retained;
                                    match w.ns_window() {
                                        Ok(ptr) => {
                                            // SAFETY: ns_window() 返回指向主线程上当前 autoreleased
                                            // NSWindow 的指针；retain_autoreleased 在进行类型转换前获
                                            // 得所有权。NSWindow 类通过 objc2-app-kit NSWindow feature
                                            // 绑定（继承自 NSResponder）暴露了 setHidesOnDeactivate。
                                            let ns_window = unsafe {
                                                Retained::<NSWindow>::retain_autoreleased(ptr.cast())
                                            };
                                            if let Some(ns_window) = ns_window {
                                                ns_window.setHidesOnDeactivate(true);
                                                tracing::info!("popover setHidesOnDeactivate:YES applied");
                                            } else {
                                                tracing::warn!("popover ns_window pointer was nil");
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!(error = %e, "popover ns_window() unavailable");
                                        }
                                    }
                                }
                                tracing::info!("popover window created successfully");
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "create popover failed");
                            }
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "proxy_start" => {
                        let settings = tauri::async_runtime::block_on(load_proxy_settings(app)).unwrap_or(ProxySettings {
                            port: 9876,
                            autostart: true,
                            silent_launch: false,
                            bind_lan: true,
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
                    let _ = tauri::async_runtime::block_on(refresh_tray_menu(&handle, &commands_platform::tray::TrayMenuBuildImpl));
                });
            }

            // 定时托盘刷新 + 跨日重算：托盘标题（含「今日花费/Token/请求」today_usage）此前
            // 完全由事件驱动（每请求 / quota 真查 / 配置变更 emit "tray-refresh"）。应用跨本地
            // 00:00 仍空闲（无新请求）时无任何事件触发 refresh_tray_menu，today_stats 的 SQL 窗口
            // 已滚到新一天，但标题仍冻结在昨日累计值 → 与手动打开 popover（实时查 today_stats）不一致。
            // 这里补一个常驻定时器：粗粒度 5 分钟兜底刷新 + 精确对齐下一次本地 00:00，保证跨日后
            // 标题立即重算。非热路径（≤ 每 5 分钟一次 today_stats 查询 + set_title），不引入高频轮询。
            #[cfg(target_os = "macos")]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use chrono::{Local, TimeZone};
                    use tracing::Instrument;
                    let coarse = std::time::Duration::from_secs(300);
                    loop {
                        // 距下一次本地 00:00 的秒数（含 1s 余量越过边界），与粗粒度间隔取小者。
                        let now = Local::now();
                        let secs_to_midnight = (now + chrono::Duration::days(1))
                            .date_naive()
                            .and_hms_opt(0, 0, 0)
                            .and_then(|m| Local.from_local_datetime(&m).single())
                            .map(|m| (m - now).num_seconds().max(0) as u64 + 1)
                            .unwrap_or(coarse.as_secs());
                        let sleep = coarse.min(std::time::Duration::from_secs(secs_to_midnight));
                        tokio::time::sleep(sleep).await;
                        // 每次托盘刷新一个真实唯一链路 id：本次 today_stats 等 SQL 共享该 id。
                        let cycle_span = tracing::info_span!(
                            "tray_refresh_tick",
                            trace_id = %logging::new_trace_id()
                        );
                        let _ = refresh_tray_menu(&handle, &commands_platform::tray::TrayMenuBuildImpl).instrument(cycle_span).await;
                    }
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
                    use tracing::Instrument;
                    let span = tracing::info_span!("cold_start_init_tray", trace_id = %logging::new_trace_id());
                    cold_start_init_tray_estimates(&handle).instrument(span).await;
                });
            }

            // 静默启动：隐藏主窗口，仅托盘运行
            if settings.silent_launch {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            // aidog:// deep link 协议层：挂 on_open_url + 冷启动 get_current 补发 +
            // Win/Linux register_all。失败仅 warn 不阻塞启动（非关键路径）。
            // macOS scheme 注册在 bundle 期（Info.plist CFBundleURLTypes）完成，dev 模式
            // 需手动 LSRegisterURL（见 README / task journal）。
            crate::deep_link::setup(app.handle());

            Ok(())
}
