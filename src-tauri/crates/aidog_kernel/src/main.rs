//! aidog 无界面内核（票 08）。
//!
//! ```text
//! aidog-kernel          纯内核：代理转发 + 定时任务
//! aidog-kernel --ui     额外在代理端口的 /api/ui/ 提供 Web 界面
//! aidog-kernel mcp      stdio 上的 MCP server（newline-delimited JSON-RPC）
//! ```
//!
//! 管理接口（`POST /api/rpc/<命令>`、`GET /api/events`，ADR 0010）与桌面版一样挂在**代理端口**
//! 上，两种形态都有，由设置里的开关统一启停；`--ui` 只决定是否多提供 `/api/ui/` 网页界面。
//! 日志同时进标准输出与 `~/.aidog/logs/`。
//!
//! 与桌面壳（root package `aidog`）的关系：命令体、代理、调度全部共用同一批 crate，
//! 差别只有外壳 —— 桌面壳装 `TauriCtx`，这里装 [`ctx::HeadlessCtx`]。本 crate 不链 tauri。

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL_ALLOCATOR: MiMalloc = MiMalloc;

mod ctx;
mod mcp_server;
mod search;

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
  aidog-kernel              纯内核（代理转发 + 定时任务）
  aidog-kernel --ui         额外在 http://127.0.0.1:<代理端口>/api/ui/ 提供 Web 界面
  aidog-kernel mcp          stdio 上的 MCP server（newline-delimited JSON-RPC；
                            由 Claude Code / Codex 的 mcpServers 配置拉起，不连库不抢锁；
                            provider 设置经环境变量 AIDOG_MCP_TOOLS_JSON 注入）

管理接口 /api/rpc/<命令> 与 /api/events 挂在代理端口上（只接受本机连接），
由「设置 → API」里的开关统一启停；--ui 要求该开关为开。

选项:
  --ui              提供 Web 界面（/api/ui/）。管理接口开关关着时启动失败
  --ui-dir <PATH>   Web 界面静态资源目录（默认取环境变量 AIDOG_UI_DIR，再回落
                    <可执行文件目录>/ui，最后 ./dist）
  -h, --help        打印本帮助
  -V, --version     打印版本
";

/// 界面地址的机器可读输出前缀。GUI 外壳把内核当子进程拉起时按这个前缀读地址。
const LISTEN_LINE_PREFIX: &str = "AIDOG_KERNEL_LISTEN=";

/// 命令行选项。刻意不引 clap：两个开关而已，多一个依赖不划算。
#[derive(Debug, Default, PartialEq)]
struct Options {
    ui: bool,
    ui_dir: Option<PathBuf>,
}

enum ParseOutcome {
    Run(Options),
    /// 子命令 `mcp`：stdio MCP server（不连库、不抢单实例锁；provider 设置走
    /// 环境变量 `AIDOG_MCP_TOOLS_JSON`，见 mcp_server 模块头）。
    RunMcp,
    /// 打印一段文字后正常退出（--help / --version）。
    Print(String),
    Error(String),
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> ParseOutcome {
    let mut opts = Options::default();
    let mut it = args.into_iter();
    // 子命令分发：`mcp` 是第一个位置参数，不接受额外参数。
    let mut pending = it.next();
    if pending.as_deref() == Some("mcp") {
        return match it.next() {
            Some(extra) => ParseOutcome::Error(format!(
                "unknown argument `{extra}` (subcommand `mcp` takes no options)"
            )),
            None => ParseOutcome::RunMcp,
        };
    }
    while let Some(a) = pending.take().or_else(|| it.next()) {
        match a.as_str() {
            "--ui" => opts.ui = true,
            "--port" => {
                return ParseOutcome::Error(
                    "--port was removed: the management API now shares the proxy port \
                     (set it in the app, 设置 → 代理)"
                        .into(),
                );
            }
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
        ParseOutcome::RunMcp => {
            // MCP stdio 模式：不连库、不抢单实例锁、不装 rustls（骨架无出站 TLS）。
            // stdout 上只许有 MCP 消息（规范 MUST），一切诊断写 stderr。
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime");
            if let Err(e) = rt.block_on(mcp_server::run()) {
                eprintln!("aidog-kernel mcp: fatal: {e}");
                std::process::exit(1);
            }
            return;
        }
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

    // 单实例锁：在开库之前。第二个实例继续跑下去的代价见 `acquire_single_instance_lock` 文档。
    // 日志此刻还没初始化，所以提示走标准输出 —— 调用方（GUI 外壳）读的也是这一路。
    let lock = match acquire_single_instance_lock(&data_dir)? {
        LockOutcome::Acquired(f) => f,
        LockOutcome::AlreadyRunning(existing) => {
            if existing.is_empty() {
                println!("aidog-kernel: another instance is already running (pure mode)");
            } else {
                println!("aidog-kernel: another instance is already running");
                println!("{existing}");
            }
            std::process::exit(0);
        }
    };

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

    // 管理接口（ADR 0010）：挂在代理端口上，开关在设置里。`--ui` 只多提供 /api/ui/，
    // 且要求开关为开 —— 否则会得到一个打不开的界面。
    let kernel_settings = load_kernel_settings(&db).await;
    check_ui_allowed(&opts, &kernel_settings)?;
    let mut mgmt = aidog_rpc::management_router();
    if opts.ui {
        mgmt = with_ui(mgmt, resolve_ui_dir(opts.ui_dir));
    }
    aidog_core::gateway::proxy::install_management_router(mgmt);

    // 代理：按设置自启（与桌面壳同一份 proxy_start）。
    let proxy_settings = load_proxy_settings(&db).await?;
    if proxy_settings.autostart {
        if let Err(e) = aidog_core::proxy_cmd::proxy::proxy_start(proxy_settings.port).await {
            tracing::error!(port = proxy_settings.port, error = %e, "kernel: proxy autostart failed");
        }
    } else {
        tracing::info!("kernel: proxy autostart disabled in settings, not starting");
    }

    // 界面地址同时进标准输出与锁文件：前者给拉起我们的父进程，后者给下一个抢锁失败的实例。
    if opts.ui {
        let line = format!(
            "{LISTEN_LINE_PREFIX}http://127.0.0.1:{}/api/ui/",
            proxy_settings.port
        );
        println!("{line}");
        record_listen_addr(&lock, &line);
    }

    wait_for_shutdown().await;
    tracing::info!("kernel: shutdown signal received, exiting");
    Ok(())
}

/// `--ui` 要求管理接口开关为开：关着时 `/api/ui/` 只会回「未启用」，不如启动就失败并说明原因。
fn check_ui_allowed(
    opts: &Options,
    settings: &aidog_core::kernel_settings::KernelSettings,
) -> Result<(), String> {
    if opts.ui && !settings.api_enabled {
        return Err("--ui needs the management API, which is disabled in settings \
                    (enable it in 设置 → API first)"
            .into());
    }
    Ok(())
}

/// 在管理接口路由上加 `/ui`（= 代理端口的 `/api/ui/`）：静态资源 + SPA 回落，只在该前缀内回落。
/// 目录不存在只 warn：前端资源缺失不该让管理接口一起挂掉。
fn with_ui(router: axum::Router, ui_dir: Option<PathBuf>) -> axum::Router {
    match ui_dir {
        Some(dir) if dir.is_dir() => {
            let serve = tower_http::services::ServeDir::new(&dir)
                .fallback(tower_http::services::ServeFile::new(dir.join("index.html")));
            tracing::info!(dir = %dir.display(), "kernel: serving web UI assets at /api/ui/");
            router.nest_service("/ui", serve)
        }
        other => {
            tracing::warn!(dir = ?other, "kernel: web UI asset directory not found, /api/ui/ unavailable");
            router
        }
    }
}

/// 单实例锁的结果。
enum LockOutcome {
    /// 抢到了。`File` 必须活到进程结束 —— 它一析构锁就释放。
    Acquired(std::fs::File),
    /// 已有实例在跑。带的是它写在锁文件里的那一行（纯内核形态下为空）。
    AlreadyRunning(String),
}

/// 抢单实例锁（`~/.aidog/kernel.lock`）。
///
/// 为什么需要它：两个内核同开一个库，SQLite 是 WAL + busy_timeout，**数据层面是安全的**，
/// 但两个进程各有一份进程内缓存（`refresh_presets_cache`、`MiddlewareEngine`）会各自变陈旧
/// 且互不通知；且两个都会按设置自启代理，第二个抢不到端口只 `tracing::error!` 一行就继续跑，
/// 留下一个半死不活的进程还在跑定时清理。共存期里旧桌面壳与新外壳同装时这一定发生。
///
/// 用 std 的文件锁（rustc 1.89 起稳定），不引依赖。锁是**劝告性**的，但进程退出或被杀时由
/// 内核释放，比「写 pid 文件再判断进程在不在」少一整类陈旧状态。
fn acquire_single_instance_lock(data_dir: &std::path::Path) -> Result<LockOutcome, String> {
    let path = data_dir.join("kernel.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(LockOutcome::Acquired(file)),
        Err(std::fs::TryLockError::WouldBlock) => {
            // 锁是劝告性的，读文件内容不受影响 —— 里面是已有实例 bind 之后写下的监听地址。
            let existing = std::fs::read_to_string(&path).unwrap_or_default();
            Ok(LockOutcome::AlreadyRunning(existing.trim().to_string()))
        }
        Err(e) => Err(format!("lock {}: {e}", path.display())),
    }
}

/// 把实际监听地址写进锁文件，供后来者读。写失败只 warn：拿不到地址顶多让第二个实例少打一行
/// 提示，不该让已经起好的服务退出。
fn record_listen_addr(lock: &std::fs::File, line: &str) {
    use std::io::{Seek, Write};
    let write = || -> std::io::Result<()> {
        lock.set_len(0)?;
        let mut f = lock;
        f.rewind()?;
        f.write_all(line.as_bytes())?;
        f.flush()
    };
    if let Err(e) = write() {
        tracing::warn!(error = %e, "kernel: cannot record listen address into lock file");
    }
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
            if let Err(e) = aidog_db::cleanup_session_name_cache(db).await {
                tracing::warn!(error = %e, "scheduled: cleanup session_name_cache failed");
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

    // registry 灌种：启动一次（编译期内置 → 镜像表，幂等 diff，内容变才写；
    // 远程同步已删 2026-10-21，无周期任务）。
    tokio::spawn(async move {
        if let Err(e) = aidog_core::gateway::price_sync::seed_registry_from_bundled(aidog_ctx::db()).await {
            tracing::warn!(error = %e, "startup: registry seed from bundled failed");
        }
    });

    // MCP 推荐清单：启动后台预取一次（TTL 24h 内 sync 内部自跳过），失败静默
    // （保留缓存/内置，无 UI 提示）。无手动刷新入口（票 04）。
    tokio::spawn(async move {
        if let Err(e) = aidog_core::gateway::mcp_recommend::sync_mcp_recommended(aidog_ctx::db()).await {
            tracing::warn!(error = %e, "mcp recommended prefetch failed");
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
