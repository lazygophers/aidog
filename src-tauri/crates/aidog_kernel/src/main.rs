//! aidog 无界面内核（票 08）。
//!
//! ```text
//! aidog-kernel          纯内核：代理转发 + 定时任务，不开任何 HTTP 管理面
//! aidog-kernel --ui     额外挂 /rpc/<命令>（211 个）、SSE /events、静态前端资源
//! ```
//!
//! 两种形态跑的是**同一套**代理转发、协议转换、路由、计费、统计、MCP、skills、hooks 与
//! 定时任务；`--ui` 只是多开一个管理面监听。日志同时进标准输出与 `~/.aidog/logs/`。
//!
//! 与桌面壳（root package `aidog`）的关系：命令体、代理、调度全部共用同一批 crate，
//! 差别只有外壳 —— 桌面壳装 `TauriCtx`，这里装 [`ctx::HeadlessCtx`]。本 crate 不链 tauri。

mod ctx;
mod rpc;
mod server;

use std::path::PathBuf;
use std::sync::Arc;

use aidog_core::kernel_settings::load_kernel_settings;
use aidog_core::shared::{aidog_data_dir, load_proxy_settings};
use aidog_db::Db;
use aidog_middleware::MiddlewareEngine;
use aidog_stats::DbInitTables;

const HELP: &str = "\
aidog-kernel — aidog 的无界面内核

用法:
  aidog-kernel              纯内核（代理转发 + 定时任务，无 HTTP 管理面）
  aidog-kernel --ui         额外提供 HTTP 管理接口 /rpc/*、事件流 /events 与 Web 界面

选项:
  --ui              开启管理面（**只监听 127.0.0.1**；端口见「设置 → 内核管理面」，默认 9891。
                    要从别的设备访问，请自行架反向代理回连本机）
  --ui-dir <PATH>   Web 界面静态资源目录（默认取环境变量 AIDOG_UI_DIR，再回落
                    <可执行文件目录>/ui，最后 ./dist）
  -h, --help        打印本帮助
  -V, --version     打印版本
";

/// 命令行选项。刻意不引 clap：两个开关而已，多一个依赖不划算。
#[derive(Debug, Default, PartialEq)]
struct Options {
    ui: bool,
    ui_dir: Option<PathBuf>,
}

enum ParseOutcome {
    Run(Options),
    /// 打印一段文字后正常退出（--help / --version）。
    Print(String),
    Error(String),
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> ParseOutcome {
    let mut opts = Options::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--ui" => opts.ui = true,
            "--ui-dir" => match it.next() {
                Some(v) => opts.ui_dir = Some(PathBuf::from(v)),
                None => return ParseOutcome::Error("--ui-dir needs a path".into()),
            },
            "-h" | "--help" => return ParseOutcome::Print(HELP.to_string()),
            "-V" | "--version" => {
                return ParseOutcome::Print(format!("aidog-kernel {}", env!("CARGO_PKG_VERSION")));
            }
            other => return ParseOutcome::Error(format!("unknown argument `{other}`")),
        }
    }
    ParseOutcome::Run(opts)
}

/// 静态资源目录：`--ui-dir` → `AIDOG_UI_DIR` → `<可执行文件目录>/ui` → `./dist`。
fn resolve_ui_dir(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(d) = explicit {
        return Some(d);
    }
    if let Ok(d) = std::env::var("AIDOG_UI_DIR") {
        return Some(PathBuf::from(d));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join("ui");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    Some(PathBuf::from("dist"))
}

fn main() {
    let outcome = parse_args(std::env::args().skip(1));
    let opts = match outcome {
        ParseOutcome::Run(o) => o,
        ParseOutcome::Print(s) => {
            println!("{s}");
            return;
        }
        ParseOutcome::Error(e) => {
            eprintln!("aidog-kernel: {e}\n\n{HELP}");
            std::process::exit(2);
        }
    };

    // rustls 0.23 需显式装 process-level CryptoProvider（ring），否则首次 TLS builder() panic。
    // 与桌面壳 startup.rs 同一行，理由相同。
    let _ = rustls::crypto::ring::default_provider().install_default();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    rt.block_on(async move {
        if let Err(e) = run(opts).await {
            // 日志可能尚未初始化，两条路都写一份，保证「错误可从标准输出读到」。
            eprintln!("aidog-kernel: fatal: {e}");
            tracing::error!(error = %e, "kernel: fatal");
            std::process::exit(1);
        }
    });
}

async fn run(opts: Options) -> Result<(), String> {
    let data_dir = aidog_data_dir()?;

    // adapter 出站 client 构建器注入（与桌面壳 app_setup 同一句，未注入时 adapter 回落直连）。
    aidog_adapter::quota::http::set_client_builder(Arc::new(|db| {
        let db = db.clone();
        Box::pin(async move { aidog_core::gateway::http_client::build_http_client_system(&db, 10, 5).await })
    }));

    // 先开 DB 再初始化日志：app log 设置的单一事实源是 DB settings 表。
    let db_path = data_dir.join("aidog.db");
    let db = Db::new(db_path.to_str().ok_or("db path is not valid utf-8")?)
        .await
        .map_err(|e| format!("open database: {e}"))?;
    db.init_tables()
        .await
        .map_err(|e| format!("init tables: {e}"))?;

    // 日志：控制台（标准输出）+ 文件，与桌面壳同一份 init_logging，同一份 DB 设置。
    // guard 必须活到进程结束，否则非阻塞 writer 的缓冲不落盘。
    aidog_core::system_cmd::app_log::migrate_log_settings_file_to_db(&db).await;
    let log_settings = aidog_core::system_cmd::app_log::load_app_log_settings_from_db(&db).await;
    let _log_guard = aidog_db::logging::init_logging(&data_dir, &log_settings);
    aidog_db::logging::cleanup_old_logs(&data_dir, log_settings.retention_hours);

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        ui = opts.ui,
        data_dir = %data_dir.display(),
        "aidog kernel starting"
    );

    // 中间件规则引擎：必须在代理开始接请求之前同步装好（空桶 fail-open 会静默绕过规则，
    // 理由见桌面壳 app_setup 同段注释）。
    let middleware = Arc::new(MiddlewareEngine::new());
    if let Err(e) = middleware.reload(&db).await {
        tracing::warn!(error = %e, "middleware engine initial load failed");
    }

    let ctx = Arc::new(ctx::HeadlessCtx::new(db.clone(), middleware));
    aidog_ctx::install(ctx.clone());

    startup_tasks(&db).await;
    spawn_scheduled_jobs();

    // 管理面：只有 --ui 才构造。纯内核形态下这段整个不执行 —— 没有任何 HTTP 管理面在听。
    let kernel_settings = load_kernel_settings(&db).await;
    match management_bind_addr(&opts, &kernel_settings) {
        Some(addr) => {
            start_management(
                ctx.clone(),
                addr,
                kernel_settings.auth_token.clone(),
                resolve_ui_dir(opts.ui_dir),
            )
            .await?
        }
        None => tracing::info!("kernel: pure mode, no HTTP management surface is listening"),
    }

    // 代理：按设置自启（与桌面壳同一份 proxy_start）。
    let proxy_settings = load_proxy_settings(&db).await?;
    if proxy_settings.autostart {
        if let Err(e) = aidog_core::proxy_cmd::proxy::proxy_start(proxy_settings.port).await {
            tracing::error!(port = proxy_settings.port, error = %e, "kernel: proxy autostart failed");
        }
    } else {
        tracing::info!("kernel: proxy autostart disabled in settings, not starting");
    }

    wait_for_shutdown().await;
    tracing::info!("kernel: shutdown signal received, exiting");
    Ok(())
}

/// 管理面该绑在哪 —— **唯一**决定「有没有 HTTP 管理面在听」的地方。
///
/// - 没带 `--ui` → `None`：一个 socket 都不开（纯内核形态的定义）。
/// - 带了 `--ui` → **永远 `127.0.0.1`**，只有端口可配。没有任何开关能把它换成 `0.0.0.0`。
///
/// 要从别的设备访问界面，请自行架反向代理（nginx / caddy）回连本机，由它负责 TLS 与鉴权
/// （2026-09-03 审查裁决，理由见 `aidog_core::kernel_settings` 模块文档）。代理端口自己的
/// `ProxySettings::bind_lan` 是另一个维度，这里**不读**。
fn management_bind_addr(
    opts: &Options,
    settings: &aidog_core::kernel_settings::KernelSettings,
) -> Option<std::net::SocketAddr> {
    if !opts.ui {
        return None;
    }
    Some(std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        settings.port,
    ))
}

/// 起管理面。
async fn start_management(
    ctx: Arc<ctx::HeadlessCtx>,
    addr: std::net::SocketAddr,
    auth_token: String,
    ui_dir: Option<PathBuf>,
) -> Result<(), String> {
    let has_auth = !auth_token.trim().is_empty();
    let state = server::ManagementState::new(ctx, auth_token);
    let (local, _handle) =
        server::serve_management(server::management_router(state, ui_dir), addr).await?;
    tracing::info!(
        addr = %local,
        auth = has_auth,
        commands = rpc::RPC_COMMAND_NAMES.len(),
        "kernel: management surface listening (/rpc/*, /events, web UI)"
    );
    Ok(())
}

/// 启动期一次性维护任务。与桌面壳 `app_setup` 同一批、同样 fire-and-forget。
async fn startup_tasks(db: &Db) {
    // preset 缓存预热：不预热则路由/计价热路径一直读二进制里的旧值，与 DB 分裂。
    {
        let db = db.clone();
        tokio::spawn(async move {
            if let Err(e) = aidog_db::refresh_presets_cache(&db).await {
                tracing::warn!(error = %e, "preset cache warmup failed, falling back to bundled registry");
            }
        });
    }
    // 老库 auto_vacuum 迁移（失败下次启动重试）。
    {
        let db = db.clone();
        tokio::spawn(async move {
            match aidog_db::migrate_auto_vacuum(&db).await {
                Ok(true) => tracing::info!("db auto_vacuum migration completed on startup"),
                Ok(false) => tracing::debug!("db auto_vacuum migration skipped"),
                Err(e) => tracing::warn!(error = %e, "db auto_vacuum migration failed, will retry next launch"),
            }
        });
    }
    // 上次进程被杀留下的 status_code=0 在途行补写 499。
    {
        let db = db.clone();
        tokio::spawn(async move {
            match aidog_logs::sweep_incomplete_proxy_logs(&db).await {
                Ok(0) => tracing::debug!("no incomplete proxy_log rows to sweep"),
                Ok(n) => tracing::info!(rows = n, "swept incomplete proxy_log rows to 499"),
                Err(e) => tracing::warn!(error = %e, "sweep incomplete proxy logs failed"),
            }
        });
    }
    // 聚合表两次一次性纠正（各自版本门控，只跑一次）。
    {
        let db = db.clone();
        tokio::spawn(async move {
            if let Err(e) = aidog_stats::rebuild_stats_agg_once_if_needed(&db).await {
                tracing::warn!(error = %e, "stats_agg one-time rebuild failed");
            }
            if let Err(e) = aidog_stats::correct_count_tokens_agg_once_if_needed(&db).await {
                tracing::warn!(error = %e, "stats_agg count_tokens correction failed");
            }
        });
    }
    // 外部工具配置联动 + settings 文件同步（与桌面壳同为非关键路径）。
    {
        let db = db.clone();
        tokio::spawn(async move {
            aidog_core::sync_settings::try_sync_settings(&db).await;
            if let Err(e) =
                aidog_core::ai_tools_cmd::coding_tools::ensure_default_coding_tools_settings(&db)
                    .await
            {
                tracing::warn!(error = %e, "ensure_default_coding_tools_settings failed");
            }
        });
    }
}

/// 常驻定时任务。**纯内核形态一样跑**——后台维护不依赖界面进程。
///
/// 四项与桌面壳一一对应：保留期清理链（含通知 / 统计 retention 与阈值 VACUUM）、
/// 价格（registry）同步、定时备份、通知（由清理链内的 `cleanup_notifications` 与
/// 代理热路径的 `dispatch` 覆盖，无独立 loop）。
fn spawn_scheduled_jobs() {
    // 保留期清理链。周期由 `ProxyLogSettings::cleanup_interval_secs` 派生（票 01 特意把
    // 派生逻辑放在 aidog_db 上，就是为了内核这份能原样复用同一份规则）。
    tokio::spawn(async move {
        const OLDER_THAN_SECS: i64 = 3 * 24 * 3600;
        const VACUUM_THRESHOLD_BYTES: i64 = 100 * 1024 * 1024;
        loop {
            let db = aidog_ctx::db();
            let interval = aidog_core::proxy_cmd::proxy_log::retention_cleanup_interval(db).await;
            // 启动不立即跑；等待期间保留期被改会提前唤醒，本轮只重算周期不清理。
            if aidog_core::proxy_cmd::proxy_log::wait_next_retention_cycle(interval).await {
                continue;
            }
            match aidog_db::purge_all_soft_deleted(db, OLDER_THAN_SECS).await {
                Ok(map) if map.values().any(|&n| n > 0) => {
                    tracing::info!(purged = ?map, "scheduled: purged old soft-deleted rows")
                }
                Ok(_) => tracing::debug!("scheduled: purge_all_soft_deleted ran, nothing to delete"),
                Err(e) => tracing::warn!(error = %e, "scheduled: purge_all_soft_deleted failed"),
            }
            let retention_days = aidog_db::get_notification_settings(db)
                .await
                .inbox_retention_days;
            if let Err(e) = aidog_db::cleanup_notifications(db, retention_days).await {
                tracing::warn!(error = %e, "scheduled: cleanup notifications failed");
            }
            let stats_settings: aidog_core::gateway::models::StatsSettings =
                aidog_db::get_setting(db, "stats", "settings")
                    .await
                    .ok()
                    .flatten()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
            if let Err(e) = aidog_stats::cleanup_stats_agg(db, stats_settings.retention_days).await {
                tracing::warn!(error = %e, "scheduled: cleanup stats_agg failed");
            }
            let log_settings: aidog_core::gateway::models::ProxyLogSettings =
                aidog_db::get_setting(db, "proxy", "logging")
                    .await
                    .ok()
                    .flatten()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
            aidog_core::proxy_cmd::proxy_log::run_retention_cleanup(db, &log_settings).await;
            match aidog_db::db_file_size(db).await {
                Ok(size) if size > VACUUM_THRESHOLD_BYTES => {
                    match aidog_db::compact_database(db).await {
                        Ok(r) => tracing::info!(
                            before = r.before_bytes,
                            after = r.after_bytes,
                            "scheduled: full VACUUM completed"
                        ),
                        Err(e) => {
                            tracing::warn!(error = %e, "scheduled: full VACUUM failed, retry next cycle")
                        }
                    }
                }
                Ok(_) => tracing::debug!("scheduled: db size below VACUUM threshold, skip"),
                Err(e) => tracing::warn!(error = %e, "scheduled: db_file_size probe failed"),
            }
        }
    });

    // registry / 价格同步：启动尝试一次，之后交给 maybe_auto_sync 自己的间隔判定，
    // 每小时敲一次门（开关与间隔全在 maybe_auto_sync 内部，外层不加逻辑）。
    tokio::spawn(async move {
        loop {
            if let Err(e) = aidog_core::gateway::price_sync::maybe_auto_sync(aidog_ctx::db()).await {
                tracing::warn!(error = %e, "scheduled: price auto-sync failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });

    // 定时备份（loop 本体在 aidog_backup，与桌面壳同一份）。
    tokio::spawn(aidog_backup::scheduler_loop());
}

/// 等 Ctrl-C / SIGTERM（systemd `stop` 发的就是 SIGTERM）。
async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "kernel: cannot listen SIGTERM, falling back to Ctrl-C only");
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
#[path = "test_kernel.rs"]
mod test_kernel;
