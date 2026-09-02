//! App setup（启动初始化逻辑）下沉自 lib.rs 的 run() setup 闭包，零行为变更。
use aidog_core::ai_tools_cmd::coding_tools::ensure_default_coding_tools_settings;
use aidog_core::gateway;
use aidog_core::logging;
use aidog_core::platform_cmd::quota::cold_start_init_tray_estimates;
use aidog_core::proxy_cmd::proxy::{proxy_start, proxy_stop};
use aidog_core::shared::{ProxySettings, aidog_data_dir, load_proxy_settings};
use aidog_core::sync_settings::try_sync_settings;
use aidog_core::system_cmd::app_log::{
    load_app_log_settings_from_db, migrate_log_settings_file_to_db,
};
use aidog_core::tray_render::{TrayMenuBuildImpl, build_tray_menu, refresh_tray_menu};
use aidog_db::Db;
use aidog_middleware::MiddlewareEngine;
use aidog_stats::DbInitTables;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tauri::Manager;
use tauri::tray::TrayIconBuilder;

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 运行时 PATH 修复（GUI launchd/Finder env 极简，brew/nvm/pyenv 装的
    // node/npx/python/uv 不在 PATH）已下沉到各真正 spawn 子进程的入口自调
    // `gateway::skills::runtime_path()` 拿合并 PATH 后 per-Command `.env("PATH", p)`
    // 注入（skills 检测/安装、cli_env、script_executor、skills_sync；不改进程全局
    // env，OnceLock 幂等缓存首次探测结果，避免与其他线程 `getenv` 数据竞争，也不再在
    // 冷启动关键路径同步跑一次登录 shell）。

    let data_dir = aidog_data_dir().expect("failed to resolve data dir");

    // aidog_adapter::quota 出站 client 构建器注入（读 DB 代理设置 + 全局缓存；
    // 未注入时 adapter 侧回落直连）。adapter 不依赖 core，经回调单向提供。
    aidog_adapter::quota::http::set_client_builder(std::sync::Arc::new(|db| {
        let db = db.clone();
        Box::pin(async move {
            aidog_core::gateway::http_client::build_http_client_system(&db, 10, 5).await
        })
    }));

    // 先开 DB 再初始化日志：app log 设置单一事实源 = DB settings 表（禁独立文件）。
    // 历史 ~/.aidog/log_settings.json 在此一次性迁移进 DB 后删除。
    let db_path = data_dir.join("aidog.db");
    let db = tauri::async_runtime::block_on(async {
        use tracing::Instrument;
        // 启动期 init：包进带真实唯一链路 id 的 span，init_tables 的建表 / 迁移 SQL
        // 经 call_traced 环境捕获带上该 id（非固定常量）。
        let init_span = tracing::info_span!("db_init", trace_id = %logging::new_trace_id());
        async {
            let db = Db::new(db_path.to_str().unwrap())
                .await
                .expect("failed to open database");
            db.init_tables().await.expect("failed to init tables");
            // 自动建默认分组改为「创建平台时一次性判断」（见 platform_create），
            // 不再在启动时为所有平台兜底建组（避免覆盖用户「不分组」选择）。
            db
        }
        .instrument(init_span)
        .await
    });
    // preset 缓存预热：`platform_preset` 表（registry 同步落地）与编译期内置那份的并集
    // 装进进程内缓存，供路由/计费热路径的 `peak` / `models.peak` / 端点锁死
    // 同步读取。不预热则这些读取会一直用二进制里的旧值，与前端（读 DB）判定分裂。
    {
        let db_clone = db.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = aidog_db::refresh_presets_cache(&db_clone).await {
                tracing::warn!(error = %e, "preset cache warmup failed, falling back to bundled registry");
            }
        });
    }
    // 后台 auto_vacuum 迁移：老库（auto_vacuum=NONE）需 VACUUM 重建切到 INCREMENTAL
    // 才能回收 free pages。非阻塞——spawn 独立 task，失败仅 warn 不置标记，下次启动重试。
    // VACUUM 锁库期间代理写请求排队（busy_timeout=5000 兜底）。
    // Db::clone 廉价（仅 channel sender 共享同一后台线程连接），manage 前即可 spawn。
    {
        let db_clone = db.clone();
        tauri::async_runtime::spawn(async move {
            use tracing::Instrument;
            let span =
                tracing::info_span!("db_migrate_auto_vacuum", trace_id = %logging::new_trace_id());
            async {
                        match aidog_db::migrate_auto_vacuum(&db_clone).await {
                            Ok(true) => tracing::info!("db auto_vacuum migration completed on startup"),
                            Ok(false) => tracing::debug!("db auto_vacuum migration skipped (already migrated or INCREMENTAL)"),
                            Err(e) => tracing::warn!(error = %e, "db auto_vacuum migration failed on startup, will retry next launch"),
                        }
                    }
                    .instrument(span)
                    .await
        });
    }
    // 启动兜底：把上次进程被杀（升级重启 / dev 热重载 / crash）时留下的 status_code=0
    // 在途行补写为 499。请求级 Drop guard 只覆盖进程活着的场景，进程被杀时来不及跑，
    // 那些行会永久停在 0（Logs 页显示空白条目）。启动时无请求在途，翻全部 0 行安全。
    {
        let db_clone = db.clone();
        tauri::async_runtime::spawn(async move {
            use tracing::Instrument;
            let span = tracing::info_span!("db_sweep_incomplete_logs", trace_id = %logging::new_trace_id());
            async {
                match aidog_logs::sweep_incomplete_proxy_logs(&db_clone).await {
                    Ok(0) => tracing::debug!("no incomplete proxy_log rows to sweep"),
                    Ok(n) => tracing::info!(
                        rows = n,
                        "swept incomplete proxy_log rows to 499 (interrupted by process exit)"
                    ),
                    Err(e) => tracing::warn!(error = %e, "sweep incomplete proxy logs failed"),
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
            let span =
                tracing::info_span!("db_rebuild_stats_agg", trace_id = %logging::new_trace_id());
            async {
                        match aidog_stats::rebuild_stats_agg_once_if_needed(&db_clone).await {
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
                        match aidog_stats::correct_count_tokens_agg_once_if_needed(&db_clone).await {
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

    // 初始化日志（DB 已开，读 DB 设置；迁移遗留文件）+ 清理过期日志文件：
    // 挪到窗口显示之后台 spawn —— 纯观测性副作用（写日志文件/清旧日志），
    // 无其它启动逻辑读 log_settings 或依赖 logging 提前就绪；未初始化前 tracing
    // 宏落空 subscriber（no-op），不 panic、不丢功能，只是这段窗口内的早期日志
    // 不落盘。WorkerGuard 经 handle.manage 存状态表，AppHandle 与 App 同生共死，
    // 时序上仍覆盖到进程退出前最后一刻（同原 app.manage(guard) 契约）。
    {
        let handle = app.handle().clone();
        let dir = data_dir.clone();
        tauri::async_runtime::spawn(async move {
            use tracing::Instrument;
            let span = tracing::info_span!("log_init_startup", trace_id = %logging::new_trace_id());
            async {
                let Some(db_state) = handle.try_state::<Db>() else {
                    tracing::warn!("log_init_startup: Db state missing, skip");
                    return;
                };
                migrate_log_settings_file_to_db(&db_state).await;
                let log_settings = load_app_log_settings_from_db(&db_state).await;
                if let Some(guard) = logging::init_logging(&dir, &log_settings) {
                    handle.manage(guard);
                }
                logging::cleanup_old_logs(&dir, log_settings.retention_hours);
            }
            .instrument(span)
            .await
        });
    }

    // 启动时同步所有 settings 文件（检查不一致并更新）：挪到窗口显示之后台 spawn。
    // 无其它启动逻辑读取其结果（fire-and-forget，失败仅 warn），与 s2 同类
    // 「advisory 文件同步」非关键路径。
    {
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            use tracing::Instrument;
            let span =
                tracing::info_span!("sync_settings_startup", trace_id = %logging::new_trace_id());
            async {
                let Some(db_state) = handle.try_state::<Db>() else {
                    tracing::warn!("sync_settings_startup: Db state missing, skip");
                    return;
                };
                try_sync_settings(&handle, &db_state).await;
            }
            .instrument(span)
            .await
        });
    }

    // 启动初始化 CC/Codex 联动开关：DB 无记录时视为默认开（写 ~/.claude/config.json
    // 与 ~/.claude.json），并落 DB true。开箱即生效，无需进设置页。
    // 失败仅 warn 不阻塞启动。挪到窗口显示之后台 spawn：写的是外部工具配置文件，
    // 无 setup() 内其它逻辑依赖其完成，用户手动打开对应工具前该窗口早已跑完。
    {
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            use tracing::Instrument;
            let span = tracing::info_span!("coding_tools_defaults_startup", trace_id = %logging::new_trace_id());
            async {
                let Some(db_state) = handle.try_state::<Db>() else {
                    tracing::warn!("coding_tools_defaults_startup: Db state missing, skip");
                    return;
                };
                if let Err(e) = ensure_default_coding_tools_settings(&db_state).await {
                    tracing::warn!(error = %e, "ensure_default_coding_tools_settings failed");
                }
            }
            .instrument(span)
            .await
        });
    }

    // 中间件规则引擎单例（C1）：启动时从 DB 加载规则建缓存；CRUD command 写后 reload。
    //
    // 保留同步 block_on（不挪后台 spawn）：`MiddlewareEngine::new()` 起手是空桶，
    // `resolve_rules` 对空桶 fail-open——即命中不到任何规则时视同「无规则」放行，
    // 不是拒绝。若把 reload 挪到 app.manage(engine) 之后台异步执行，会有一段窗口
    // engine 已挂进状态表但桶是空的；此时若代理已在接受请求（autostart 的
    // proxy_start 就在本函数下方 spawn），该窗口内的请求会**静默绕过**中间件规则
    // （屏蔽/改写/限流等业务规则失效，而非报错），这是行为变更，违反本任务
    // 「只动时序不改业务逻辑」边界，且量级达秒级（DB 查询+重建桶），不是可忽略的
    // 理论竞态。经排查 try_sync_settings/ensure_default_coding_tools_settings/
    // log 初始化三处均无此类「初始为空即改变业务语义」的风险（详见上方各自注释），
    // 唯独 engine reload 属于「必须启动期同步完成」，故维持 block_on。
    {
        let engine = Arc::new(MiddlewareEngine::new());
        let db_state = app.state::<Db>();
        if let Err(e) = tauri::async_runtime::block_on(engine.reload(&db_state)) {
            tracing::warn!(error = %e, "middleware engine initial load failed");
        }
        app.manage(engine.clone());

        // 票 06：装进程级 AppCtx。必须在 Db + MiddlewareEngine 就绪之后、任何命令可能被
        // 调用之前（命令走 invoke，webview 起来才可能触发，此处已足够早）。
        // ProxyHandle 由 TauriCtx 独占持有，不再 app.manage —— 两份句柄会让
        // 「代理是否在跑」在托盘 / popover / proxy_status 之间自相矛盾。
        aidog_core::tauri_ctx::install(
            app.handle().clone(),
            app.state::<Db>().inner().clone(),
            engine,
        );
    }

    // 定时备份调度器 (spawn_scheduler 内部 spawn 常驻 loop, 启动首次检查补「关机错过」)。
    aidog_backup::spawn_scheduler(app.handle().clone());

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
            gateway::logo_sync::sync_all_logos(db, dir)
                .instrument(span)
                .await;
        });
    }

    // registry 自动同步：启动时一次性尝试（开关/间隔判定全在 maybe_auto_sync 内部，
    // 外层不加逻辑）。全新安装未点过「立即同步」按钮时 model_entry / platform_preset 表为空，
    // 此调用是唯一接回生产的入口。失败仅 warn，不阻塞启动。
    {
        let db = app.state::<Db>().inner().clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = gateway::price_sync::maybe_auto_sync(&db).await {
                tracing::warn!(error = %e, "price auto-sync failed at startup");
            }
        });
    }

    // 内置每日定时清理：永久删除软删超过 3 天的平台行（deleted_at>0 且 < now-3d）
    // + proxy_log 三级 retention 清理链（user/upstream fields + retention_days + tombstone）
    // + 阈值触发全量 VACUUM（db>100MB；retention 后大块 free pages 回收）。
    // 启动不立即跑（用户要求「启动不做定时操作」）；周期不再写死 24h，而是每轮从当前
    // ProxyLogSettings 派生（三档保留期最短非零值 ÷4，夹在 [1min, 24h]，见
    // ProxyLogSettings::cleanup_interval_secs）。非关键路径，失败仅 warn。
    //
    // VACUUM 经 db.call_traced 跑在 DB 专属后台线程（共享唯一连接），不阻塞 async
    // runtime；锁库期间代理写请求排队（busy_timeout=5000 兜底）。compact_database 内部
    // 已 wal_checkpoint(TRUNCATE)+ANALYZE，无需额外善后。
    {
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            use tracing::Instrument;
            let older_than_secs: i64 = 3 * 24 * 3600;
            // 全量 VACUUM 触发阈值：100MB。低于此 incremental_vacuum 已够；高于此
            // compact_database 整库重建激进回收（response_body 累积型胀库主因）。
            const VACUUM_THRESHOLD_BYTES: i64 = 100 * 1024 * 1024;
            loop {
                // 周期跟随保留期：每轮重读设置重算（Db 尚未就绪时退化为 24h 再试）。
                let interval = match handle.try_state::<Db>() {
                    Some(db) => {
                        aidog_core::proxy_cmd::proxy_log::retention_cleanup_interval(&db).await
                    }
                    None => std::time::Duration::from_secs(24 * 3600),
                };
                // 启动不立即跑：先等一个周期再执行清理。等待期间保留期被改 → 提前唤醒，
                // 本轮只重算周期不清理（proxy_log_settings_set 已同步跑过清理链）。
                if aidog_core::proxy_cmd::proxy_log::wait_next_retention_cycle(interval).await {
                    continue;
                }
                // 每个清理周期一个真实唯一链路 id：本周期内所有 SQL 共享该 id（SQL 日志
                // req= 经 call_traced 的环境捕获自动带上），不同周期 id 不同。
                let cycle_span = tracing::info_span!(
                    "scheduled_cleanup",
                    trace_id = %logging::new_trace_id()
                );
                async {
                            if let Some(db) = handle.try_state::<Db>() {
                                match aidog_db::purge_all_soft_deleted(&db, older_than_secs).await {
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
                                let retention_days = aidog_db::get_notification_settings(&db).await.inbox_retention_days;
                                if let Err(e) = aidog_db::cleanup_notifications(&db, retention_days).await {
                                    tracing::warn!(error = %e, "scheduled: cleanup notifications failed");
                                }
                                // 聚合统计表 retention 硬删（默认 365 天；stats retention_days=0 → 永不清理）。
                                let stats_settings: gateway::models::StatsSettings = aidog_db::get_setting(&db, "stats", "settings").await
                                    .ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                                if let Err(e) = aidog_stats::cleanup_stats_agg(&db, stats_settings.retention_days).await {
                                    tracing::warn!(error = %e, "scheduled: cleanup stats_agg failed");
                                }
                                // proxy_log 三级 retention（默认 7d/7d/90d 保留不动）：复用
                                // settings_set/cleanup_expired 同一清理链，单步失败 warn 容错。
                                // purge_deleted_proxy_logs 内部已调 incremental_vacuum(100) 回收
                                // 小块 free pages；大块回收由阈值 VACUUM 兜底。
                                let log_settings: gateway::models::ProxyLogSettings = aidog_db::get_setting(&db, "proxy", "logging").await
                                    .ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                                aidog_core::proxy_cmd::proxy_log::run_retention_cleanup(&db, &log_settings).await;
                                // 阈值触发全量 VACUUM：胀库（>100MB）时整库重建回收大块 free pages。
                                // 失败仅 warn（锁冲突 / 磁盘满等），不阻塞后续周期。
                                match aidog_db::db_file_size(&db).await {
                                    Ok(size) if size > VACUUM_THRESHOLD_BYTES => {
                                        tracing::info!(
                                            size_bytes = size,
                                            threshold = VACUUM_THRESHOLD_BYTES,
                                            "scheduled: db size exceeds threshold, running full VACUUM"
                                        );
                                        match aidog_db::compact_database(&db).await {
                                            Ok(r) => tracing::info!(
                                                before = r.before_bytes,
                                                after = r.after_bytes,
                                                "scheduled: full VACUUM completed"
                                            ),
                                            Err(e) => tracing::warn!(
                                                error = %e,
                                                "scheduled: full VACUUM failed (locked or disk full), will retry next cycle"
                                            ),
                                        }
                                    }
                                    Ok(size) => tracing::debug!(
                                        size_bytes = size,
                                        threshold = VACUUM_THRESHOLD_BYTES,
                                        "scheduled: db size below VACUUM threshold, skip"
                                    ),
                                    Err(e) => tracing::warn!(
                                        error = %e,
                                        "scheduled: db_file_size probe failed, skip VACUUM this cycle"
                                    ),
                                }
                            }
                        }
                        .instrument(cycle_span)
                        .await;
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
            if let tauri::tray::TrayIconEvent::Click {
                button,
                button_state,
                rect,
                ..
            } = event
            {
                // 只响应 Down，忽略 Up（否则 Down 创建 → Up 立刻销毁）
                if button != MouseButton::Left || button_state != MouseButtonState::Down {
                    return;
                }
                let app = tray.app_handle().clone();
                tracing::info!(button = ?button, "tray click → toggle popover");
                // 按需创建：开着就销毁，关着就现建（启动期不再预建，省 58 MB 常驻）。
                if crate::popover_window::is_open() {
                    crate::popover_window::close(&app);
                    return;
                }
                // 定位：居中于 tray 图标正下方
                // rect 坐标为 Physical 像素，position() 接受 Logical 坐标，需除以 scale factor
                let scale = app
                    .get_webview_window("main")
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
                let x = rx + rw / 2.0 - pw / 2.0;
                let y = ry + rh;
                tracing::info!(x, y, scale, "popover show position");
                crate::popover_window::open(&app, Some((x, y)));
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "proxy_start" => {
                let settings = tauri::async_runtime::block_on(load_proxy_settings(app)).unwrap_or(
                    ProxySettings {
                        port: 9890,
                        autostart: true,
                        silent_launch: false,
                        bind_lan: false,
                    },
                );
                let port = settings.port;
                let app_handle = app.clone();
                tauri::async_runtime::block_on(async move {
                    if let Err(e) = proxy_start(port, app_handle.clone()).await {
                        tracing::error!(port, error = %e, "tray: proxy start failed");
                        // 无前端窗口路径（托盘点启动同自启动，proxy-port-no-drift s3）：
                        // emit 结构化错误供 App.tsx 监听转系统通知（i18n 在前端做，
                        // Rust 侧不硬编码文案）+ 复用既有 tray-refresh 事件确认未启动态。
                        use tauri::Emitter;
                        let _ = app_handle.emit("proxy-start-failed", &e);
                        let _ = app_handle.emit("tray-refresh", ());
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
        .build(app)
        .map_err(|e| e.to_string())?;

    // 监听后台预估发出的 tray-refresh 事件，在主线程刷新托盘（避免后台线程直接操作 tray）
    // trailing 防抖：单请求生命周期内多次 emit（4-6 次）合并成一次菜单重建，避免 UI 卡顿
    {
        use tauri::Listener;
        let handle = app.handle().clone();
        let pending_task: Arc<StdMutex<Option<tauri::async_runtime::JoinHandle<()>>>> =
            Arc::new(StdMutex::new(None));
        app.listen("tray-refresh", move |_| {
            let handle = handle.clone();
            let pending = pending_task.clone();
            tauri::async_runtime::spawn(async move {
                // 取消上一次 pending 的刷新任务
                if let Some(old_task) = pending.lock().unwrap().take() {
                    old_task.abort();
                }
                // 启动新的延迟刷新任务（200ms 后执行）
                // ponytail: 50ms → 200ms，配合 upsert_log 终态 emit 节流进一步降低重建频率。
                // 单请求生命周期内终态 emit 通常 1-2 次，200ms trailing 合并多请求 burst。
                let new_task = tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    let _ = refresh_tray_menu(&handle, &TrayMenuBuildImpl).await;
                });
                *pending.lock().unwrap() = Some(new_task);
            });
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
                let _ = refresh_tray_menu(&handle, &TrayMenuBuildImpl)
                    .instrument(cycle_span)
                    .await;
            }
        });
    }

    // 自动启动代理
    let settings = tauri::async_runtime::block_on(load_proxy_settings(app.handle()))?;
    if settings.autostart {
        let port = settings.port;
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = proxy_start(port, handle.clone()).await {
                tracing::error!(port, error = %e, "autostart: proxy start failed");
                // 无前端窗口路径：emit 结构化错误供 App.tsx 监听转系统通知
                // （proxy-port-no-drift s3，与托盘点启动分支同处理，见上）。
                use tauri::Emitter;
                let _ = handle.emit("proxy-start-failed", &e);
                let _ = handle.emit("tray-refresh", ());
            }
        });
    }

    // 冷启动 est 初始化：tray 平台从未真查（last_real_query_at==0）→ 后台真查对齐 est=真实。
    {
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            use tracing::Instrument;
            let span =
                tracing::info_span!("cold_start_init_tray", trace_id = %logging::new_trace_id());
            cold_start_init_tray_estimates(&handle)
                .instrument(span)
                .await;
        });
    }

    // 静默启动：隐藏主窗口，仅托盘运行
    if settings.silent_launch
        && let Some(w) = app.get_webview_window("main")
    {
        let _ = w.hide();
    }

    // aidog:// deep link 协议层：挂 on_open_url + 冷启动 get_current 补发 +
    // Win/Linux register_all。失败仅 warn 不阻塞启动（非关键路径）。
    // macOS scheme 注册在 bundle 期（Info.plist CFBundleURLTypes）完成，dev 模式
    // 需手动 LSRegisterURL（见 README / task journal）。
    crate::deep_link::setup(app.handle());

    Ok(())
}
