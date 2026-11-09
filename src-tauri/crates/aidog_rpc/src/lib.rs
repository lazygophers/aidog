//! 管理接口命令路由表（票 08 / API-接口模块票 01）。
//!
//! 每一项与 `src-tauri/src/startup.rs` 的 `tauri::generate_handler![...]` 一一对应：
//! 那边出 Tauri IPC 形态，这边出 HTTP 形态，**两张表必须是同一批命令**。桌面壳与无界面
//! 内核都从这里取表（ADR 0010），不再各抄一份。
//!
//! 路由路径里的命令名不手写，由 [`rpc_last_seg`] 从路径末段 `stringify!` 出来 —— 路径写错
//! 编译不过，所以路径对了名字就一定对。漂移护栏（少写/多写一条）：
//! `node scripts/t08-rpc-names.mjs` 抽本文件的命令名集合，与 `scripts/t06-handler-names.mjs`
//! 抽的注册表逐行比对，必须零差集。
//!
//! 本表挂在代理端口的 `/api/` 前缀下，由 `aidog_core::gateway::proxy::management` 负责
//! 本机限制 / 开关 / 凭据，再把去掉 `/api` 的请求交给 [`management_router`]。
use std::convert::Infallible;
use std::time::Duration;

use axum::Router;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use futures::stream::Stream;

pub mod secret_fields;

/// 取路径末段（= 命令名）。macro_rules 没有「取最后一个」，靠对尾部递归实现。
macro_rules! rpc_last_seg {
    ($a:ident) => { stringify!($a) };
    ($a:ident :: $($rest:ident)::+) => { rpc_last_seg!($($rest)::+) };
}

/// 命令的读写性质（声明处标注，缺标注编译不过）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    /// 只读：不改 aidog 的数据与设置。
    Readonly,
    /// 会改数据（含写文件、发外部请求、起停服务）：测试面板发送前要确认。
    Mutating,
}

/// 一条对外命令的声明。
#[derive(Clone, Copy, Debug)]
pub struct CommandMeta {
    pub name: &'static str,
    pub kind: CommandKind,
    /// 专门导出密钥信息的命令：管理接口对外整条禁用（403）。
    pub exports_secret: bool,
}

macro_rules! rpc_kind {
    (readonly) => { CommandKind::Readonly };
    (mutating) => { CommandKind::Mutating };
}

macro_rules! rpc_secret {
    () => { false };
    (exports_secret) => { true };
}

/// `exports_secret` 命令在管理接口上的替身：不执行命令，直接 403。
async fn exports_secret_forbidden() -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    (
        axum::http::StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({
            "error": "exports_secret_forbidden",
            "message": "该命令导出密钥信息，管理接口不对外开放；请在 aidog 桌面界面中操作",
        })),
    )
}

/// 把命令声明列表展开成 `.route("/rpc/<命令名>", post(<路径>::http))`。
///
/// 每行 `readonly|mutating <路径> [exports_secret],`：性质必填（缺了宏不匹配，编译失败），
/// `[exports_secret]` 可选。门禁脚本 `scripts/check-command-annotations.mjs` 另从源文本核对。
macro_rules! rpc_routes {
    ($( $kind:ident $($seg:ident)::+ $([$secret:ident])? ),* $(,)?) => {
        /// 全部命令的 HTTP 路由。未登记的路径由 axum 自然落 404。
        ///
        /// 泛型在状态类型上：命令 handler 自己不吃 axum state（依赖走进程级 `AppCtx`），
        /// 所以本表能直接并进任何带 state 的 Router。
        pub fn rpc_router<S>() -> Router<S>
        where
            S: Clone + Send + Sync + 'static,
        {
            let mut r = Router::new();
            $(
                let path = format!("/rpc/{}", rpc_last_seg!($($seg)::+));
                r = if rpc_secret!($($secret)?) {
                    r.route(&path, post(exports_secret_forbidden))
                } else {
                    r.route(&path, post($($seg)::+ :: http))
                };
            )*
            r
        }

        /// 本表登记的命令名（测试与自检用）。
        pub const RPC_COMMAND_NAMES: &[&str] = &[ $( rpc_last_seg!($($seg)::+) ),* ];

        /// 本表登记的命令声明（名字 + 读写性质 + 是否导出密钥）。
        pub const RPC_COMMANDS: &[CommandMeta] = &[ $(
            CommandMeta {
                name: rpc_last_seg!($($seg)::+),
                kind: rpc_kind!($kind),
                exports_secret: rpc_secret!($($secret)?),
            }
        ),* ];
    };
}

rpc_routes! {
    mutating aidog_core::platform_cmd::platform::platform_create,
    readonly aidog_core::platform_cmd::platform::platform_list,
    readonly aidog_core::platform_cmd::platform::platform_get,
    readonly aidog_core::platform_cmd::platform::platform_share_export [exports_secret],
    readonly aidog_core::platform_cmd::platform::platform_share_parse,
    mutating aidog_core::platform_cmd::platform::platform_update,
    mutating aidog_core::platform_cmd::platform::platform_delete,
    mutating aidog_core::platform_cmd::platform::platform_purge_disabled,
    readonly aidog_core::platform_cmd::platform::platform_purge_disabled_preview,
    mutating aidog_core::platform_cmd::platform::platform_ensure_auto_group,
    mutating aidog_core::platform_cmd::platform::platform_set_tray,
    mutating aidog_core::platform_cmd::model_fetch::platform_fetch_models,
    mutating aidog_core::platform_cmd::batch::batch_delete_platforms,
    mutating aidog_core::platform_cmd::batch::batch_override_models,
    mutating aidog_core::platform_cmd::batch::batch_set_status,
    mutating aidog_core::platform_cmd::batch::batch_move_group,
    readonly aidog_core::platform_cmd::platform::tray_config_get,
    mutating aidog_core::platform_cmd::platform::tray_config_set,
    readonly aidog_core::platform_cmd::platform::tray_today_stats,
    readonly aidog_core::platform_cmd::platform::tray_peak_status,
    readonly aidog_core::platform_cmd::stats::stats_query_batch,
    readonly aidog_core::popover::popover_data,
    readonly aidog_core::popover::popover_config_get,
    mutating aidog_core::popover::popover_config_set,
    readonly aidog_core::popover::popover_platform_today,
    // 票 I20：Flutter 壳用它把菜单栏一帧数据喂给 Runner 里的 NSStatusItem。
    readonly aidog_core::menubar_state::menu_bar_state,
    mutating aidog_core::platform_cmd::group::group_create,
    readonly aidog_core::platform_cmd::group::group_list,
    readonly aidog_core::platform_cmd::group::group_get,
    mutating aidog_core::platform_cmd::group::group_update,
    mutating aidog_core::platform_cmd::group::group_delete,
    mutating aidog_core::platform_cmd::group::group_set_default,
    mutating aidog_core::platform_cmd::group::group_set_platforms,
    readonly aidog_core::platform_cmd::group::group_get_platforms,
    readonly aidog_core::platform_cmd::group::group_detail,
    readonly aidog_core::platform_cmd::group::group_detail_list,
    readonly aidog_core::platform_cmd::group::group_detail_list_paged,
    mutating aidog_core::platform_cmd::group::group_reorder,
    mutating aidog_core::platform_cmd::group::group_platform_reorder,
    mutating aidog_core::platform_cmd::group::group_platform_set_level_priority,
    mutating aidog_core::platform_cmd::group::group_platform_move,
    mutating aidog_core::platform_cmd::ui_extra::set_ui_extra,
    mutating aidog_core::proxy_cmd::proxy::proxy_start,
    mutating aidog_core::proxy_cmd::proxy::proxy_stop,
    readonly aidog_core::proxy_cmd::proxy::proxy_status,
    readonly aidog_core::proxy_cmd::proxy::proxy_get_settings,
    mutating aidog_core::proxy_cmd::proxy::proxy_set_autostart,
    mutating aidog_core::proxy_cmd::proxy::proxy_set_bind_lan,
    readonly aidog_core::kernel_settings::kernel_settings_get,
    mutating aidog_core::kernel_settings::kernel_settings_set,
    readonly aidog_core::kernel_settings::api_call_log_list,
    readonly aidog_core::proxy_cmd::search::search_searx_instances,
    readonly aidog_core::proxy_cmd::tool_log::tool_log_list,
    readonly aidog_core::proxy_cmd::tool_log::tool_log_get,
    readonly aidog_core::proxy_cmd::tool_log::tool_log_retention_get,
    mutating aidog_core::proxy_cmd::tool_log::tool_log_retention_set,
    mutating aidog_core::kernel_settings::api_test_send,
    readonly aidog_core::display_currency::display_currency_get,
    mutating aidog_core::display_currency::display_currency_set,
    readonly aidog_core::display_currency::balance_currency_map,
    mutating aidog_core::proxy_cmd::proxy::app_set_autolaunch,
    readonly aidog_core::proxy_cmd::proxy::app_get_autolaunch,
    mutating aidog_core::proxy_cmd::proxy::app_set_silent_launch,
    readonly aidog_core::proxy_cmd::proxy::proxy_client_get_settings,
    mutating aidog_core::proxy_cmd::proxy::proxy_client_set_settings,
    mutating aidog_core::sync_settings::export_claude_config,
    mutating aidog_core::ai_tools_cmd::agent_config::agent_config_uninstall,
    mutating aidog_core::ai_tools_cmd::profile::profile_save,
    mutating aidog_core::ai_tools_cmd::profile::profile_apply,
    readonly aidog_core::ai_tools_cmd::profile::profile_list,
    mutating aidog_core::ai_tools_cmd::profile::profile_delete,
    mutating aidog_core::sync_settings::sync_group_settings,
    readonly aidog_core::sync_settings::get_managed_paths,
    readonly aidog_core::proxy_cmd::proxy_log::proxy_log_list,
    readonly aidog_core::proxy_cmd::proxy_log::proxy_log_list_filtered,
    readonly aidog_core::proxy_cmd::proxy_log::proxy_log_count_filtered,
    readonly aidog_core::proxy_cmd::proxy_log::proxy_log_distinct_models,
    readonly aidog_core::proxy_cmd::proxy_log::proxy_log_get,
    mutating aidog_core::proxy_cmd::proxy_log::proxy_log_clear,
    readonly aidog_core::proxy_cmd::proxy_log::proxy_log_count,
    readonly aidog_core::proxy_cmd::proxy_log::proxy_log_settings_get,
    mutating aidog_core::proxy_cmd::proxy_log::proxy_log_settings_set,
    mutating aidog_core::proxy_cmd::proxy_log::proxy_log_cleanup_expired,
    readonly aidog_core::proxy_cmd::proxy_log::proxy_log_cleanup_estimate,
    readonly aidog_core::proxy_cmd::proxy_log::session_stats,
    readonly aidog_core::proxy_cmd::proxy_log::request_log_list,
    readonly aidog_core::platform_cmd::stats::stats_settings_get,
    mutating aidog_core::platform_cmd::stats::stats_settings_set,
    mutating aidog_core::platform_cmd::stats::stats_rebuild_from_logs,
    mutating aidog_backup::commands::db_compact,
    readonly aidog_core::proxy_cmd::proxy_timeout::proxy_timeout_get,
    mutating aidog_core::proxy_cmd::proxy_timeout::proxy_timeout_set,
    readonly aidog_core::proxy_cmd::middleware::middleware_list_rules,
    mutating aidog_core::proxy_cmd::middleware::middleware_create_rule,
    mutating aidog_core::proxy_cmd::middleware::middleware_update_rule,
    mutating aidog_core::proxy_cmd::middleware::middleware_delete_rule,
    readonly aidog_core::proxy_cmd::middleware::middleware_budget_status,
    readonly aidog_core::proxy_cmd::middleware::middleware_settings_get,
    readonly aidog_core::proxy_cmd::middleware::middleware_try,
    mutating aidog_core::proxy_cmd::middleware::middleware_settings_set,
    readonly aidog_core::system_cmd::scheduling::scheduling_settings_get,
    mutating aidog_core::system_cmd::scheduling::scheduling_settings_set,
    readonly aidog_core::system_cmd::notification::notification_settings_get,
    mutating aidog_core::system_cmd::notification::notification_settings_set,
    readonly aidog_core::system_cmd::notification::notification_inbox_list,
    mutating aidog_core::system_cmd::notification::notification_clear,
    mutating aidog_core::system_cmd::notification::notification_test,
    mutating aidog_core::system_cmd::notification::notification_test_tts,
    mutating aidog_core::system_cmd::notification::notification_test_popup,
    mutating aidog_core::system_cmd::notification::notification_test_beep,
    mutating aidog_core::hooks::inject_hooks,
    mutating aidog_core::hooks::remove_hooks,
    readonly aidog_core::hooks::get_default_hooks_enabled,
    mutating aidog_core::hooks::set_default_hooks_enabled,
    readonly aidog_core::hooks::build_notify_hooks_fragment,
    readonly aidog_core::ai_tools_cmd::script_executor::check_uv,
    mutating aidog_core::ai_tools_cmd::script_executor::install_uv,
    mutating aidog_core::ai_tools_cmd::script_executor::set_script_executor,
    readonly aidog_core::ai_tools_cmd::skills::skills_check_env,
    readonly aidog_core::ai_tools_cmd::skills::skills_browse_catalog,
    readonly aidog_core::ai_tools_cmd::skills::skills_search,
    readonly aidog_core::ai_tools_cmd::skills::skills_list_installed,
    mutating aidog_core::ai_tools_cmd::skills::skills_list_refresh,
    mutating aidog_core::ai_tools_cmd::skills::skills_enable,
    mutating aidog_core::ai_tools_cmd::skills::skills_install,
    readonly aidog_core::ai_tools_cmd::skills::skill_detail,
    readonly aidog_core::ai_tools_cmd::skills::skill_read_file,
    mutating aidog_core::ai_tools_cmd::skills::skills_disable,
    mutating aidog_core::ai_tools_cmd::skills::skills_update,
    mutating aidog_core::ai_tools_cmd::skills::skills_uninstall_all,
    mutating aidog_core::ai_tools_cmd::skills::skills_uninstall,
    mutating aidog_core::ai_tools_cmd::skills::skills_install_batch,
    mutating aidog_core::ai_tools_cmd::skills::skills_uninstall_batch,
    mutating aidog_core::ai_tools_cmd::skills::skills_align_agents,
    mutating aidog_core::ai_tools_cmd::skills::skills_enable_all,
    readonly aidog_core::ai_tools_cmd::mcp::mcp_list,
    readonly aidog_core::ai_tools_cmd::mcp::mcp_scan,
    mutating aidog_core::ai_tools_cmd::mcp::mcp_import,
    mutating aidog_core::ai_tools_cmd::mcp::mcp_import_json,
    mutating aidog_core::ai_tools_cmd::mcp::mcp_set_agent,
    mutating aidog_core::ai_tools_cmd::mcp::mcp_delete,
    mutating aidog_core::ai_tools_cmd::mcp::mcp_update,
    mutating aidog_core::ai_tools_cmd::mcp::mcp_add,
    mutating aidog_core::ai_tools_cmd::mcp::mcp_resync,
    readonly aidog_core::ai_tools_cmd::mcp::mcp_share_export [exports_secret],
    readonly aidog_core::ai_tools_cmd::mcp::mcp_recommended_list,
    mutating aidog_core::ai_tools_cmd::mcp::mcp_self_install,
    mutating aidog_backup::commands::export_to_file [exports_secret],
    readonly aidog_backup::commands::export_preview,
    readonly aidog_backup::commands::backup_settings_get,
    mutating aidog_backup::commands::backup_settings_set,
    mutating aidog_backup::commands::backup_run_now,
    readonly aidog_backup::commands::import_read_file,
    mutating aidog_backup::commands::import_apply,
    readonly aidog_backup::commands::ccswitch_detect,
    readonly aidog_backup::commands::ccswitch_read,
    mutating aidog_backup::commands::ccswitch_import,
    readonly aidog_backup::commands::sub2api_parse,
    readonly aidog_backup::commands::sub2api_read_file,
    mutating aidog_backup::commands::sub2api_import,
    readonly aidog_core::system_cmd::app_log::app_log_settings_get,
    mutating aidog_core::system_cmd::app_log::app_log_settings_set,
    readonly aidog_core::system_cmd::auto_update::get_auto_update_enabled,
    mutating aidog_core::system_cmd::auto_update::set_auto_update_enabled,
    readonly aidog_core::ai_tools_cmd::coding_tools::coding_tools_settings_get,
    mutating aidog_core::ai_tools_cmd::coding_tools::coding_tools_settings_set,
    readonly aidog_core::system_cmd::fs_autocomplete::fs_autocomplete,
    readonly aidog_core::settings::settings_get,
    mutating aidog_core::settings::settings_set,
    mutating aidog_core::settings::settings_delete,
    readonly aidog_core::settings::custom_slot_usage_count,
    mutating aidog_core::settings::custom_slot_purge,
    readonly aidog_core::settings::settings_list,
    readonly aidog_core::settings::preview_statusline_script,
    readonly aidog_core::settings::read_claude_code_settings,
    readonly aidog_core::settings::claude_output_styles_list,
    readonly aidog_core::presets::settings_preset_list,
    readonly aidog_core::gateway::codex::codex_config_read,
    mutating aidog_core::gateway::codex::codex_config_write,
    readonly aidog_core::gateway::codex::codex_config_path,
    readonly aidog_core::gateway::pi::pi_models_path,
    readonly aidog_core::gateway::pi::pi_settings_read,
    mutating aidog_core::gateway::pi::pi_settings_write,
    readonly aidog_core::platform_cmd::stats::stats_query,
    readonly aidog_core::platform_cmd::stats::scatter_histogram,
    readonly aidog_core::platform_cmd::stats::quota_snapshots,
    mutating aidog_core::ai_tools_cmd::model_test::model_test,
    readonly aidog_core::proxy_cmd::proxy_log::platform_usage_stats,
    readonly aidog_core::proxy_cmd::proxy_log::group_usage_stats,
    readonly aidog_core::proxy_cmd::proxy_log::all_group_usage_stats,
    readonly aidog_core::proxy_cmd::proxy_log::group_last_requests,
    readonly aidog_core::proxy_cmd::proxy_log::all_platform_usage_stats,
    readonly aidog_core::proxy_cmd::proxy_log::get_last_test_result,
    mutating aidog_core::platform_cmd::quota::platform_query_quota,
    mutating aidog_core::platform_cmd::quota::platform_query_quota_newapi,
    mutating aidog_core::platform_cmd::quota::platform_query_quota_devin,
    mutating aidog_core::platform_cmd::platform::platform_reorder,
    readonly aidog_core::platform_cmd::price::price_sync_settings_get,
    mutating aidog_core::platform_cmd::price::price_sync_settings_set,
    readonly aidog_core::platform_cmd::model_entry::model_entry_list,
    readonly aidog_core::platform_cmd::model_entry::model_entry_get,
    readonly aidog_core::platform_cmd::model_entry::model_info_snapshot,
    readonly aidog_core::system_cmd::about::about_info,
    readonly aidog_core::cli_env::cli_check_versions,
    mutating aidog_core::cli_env::cli_install,
    mutating aidog_core::cli_env::cli_upgrade,
    readonly aidog_core::cli_env::cli_diagnose_conflicts,
    readonly aidog_core::cli_env::cli_check_updates,
    readonly aidog_core::cli_env::agent_update_check,
    mutating aidog_core::cli_env::agent_update_apply,
    readonly aidog_core::defaults::get_defaults_json,
    readonly aidog_core::defaults::get_protocol_logo_path,
    readonly aidog_core::defaults::get_protocol_logo_data_url,
    mutating aidog_core::defaults::sync_protocol_logo,
    readonly aidog_core::defaults::get_client_types_json,
    readonly aidog_core::client_vocab::get_client_vocab,
    readonly aidog_core::proxy_cmd::mitm::mitm_status,
    mutating aidog_core::proxy_cmd::mitm::mitm_enable,
    mutating aidog_core::proxy_cmd::mitm::mitm_disable,
    mutating aidog_core::proxy_cmd::mitm::mitm_install_ca_prepare,
    mutating aidog_core::proxy_cmd::mitm::mitm_install_ca,
    mutating aidog_core::proxy_cmd::mitm::mitm_uninstall_ca_prepare,
    mutating aidog_core::proxy_cmd::mitm::mitm_set_ca_installed,
    readonly aidog_core::proxy_cmd::mitm::mitm_classify_trust_error,
    mutating aidog_core::proxy_cmd::mitm::mitm_whitelist_add,
    mutating aidog_core::proxy_cmd::mitm::mitm_whitelist_remove,
    mutating aidog_core::proxy_cmd::mitm::mitm_whitelist_toggle,
    mutating aidog_core::proxy_cmd::mitm::mitm_whitelist_import_defaults,
    mutating aidog_core::proxy_cmd::mitm::mitm_whitelist_clear,
    readonly aidog_core::proxy_cmd::mitm::mitm_whitelist_test_url,
    mutating aidog_core::proxy_cmd::mitm::mitm_reset_suspects,
    // 前端已调、此前漏登记（浏览器形态 404）。db_master_key_reveal 故意不登记：
    // 弹宿主机 osascript 管理员窗并把主密钥经 HTTP 回传，见 scripts/check-frontend-rpc-coverage.mjs 白名单。
    readonly aidog_core::proxy_cmd::proxy_log::group_daily_usage,
    readonly aidog_core::proxy_cmd::proxy_log::stats_flow_breakdown,
    readonly aidog_core::proxy_cmd::proxy_log::group_latest_routing,
    readonly aidog_core::proxy_cmd::mitm_stats::mitm_usage_trend,
    readonly aidog_core::proxy_cmd::mitm_stats::mitm_oauth_refresh_stats,
    readonly aidog_core::proxy_cmd::mitm_stats::mitm_bypass_list,
    readonly aidog_core::proxy_cmd::mitm_stats::mitm_bypass_detail,
    readonly aidog_core::proxy_cmd::mitm_stats::mitm_opaque_count,
    readonly aidog_core::proxy_cmd::mitm_stats::cc_plan_info,
    mutating aidog_core::proxy_cmd::mitm_stats::cc_proxy_export [exports_secret],
    mutating aidog_core::system_cmd::notification::notification_test_channel,
    readonly aidog_core::system_cmd::notification::notification_delivery_list,
    readonly aidog_core::system_cmd::notification::notification_rule_validate,
    readonly aidog_core::platform_cmd::platform::platform_error_status,
    readonly aidog_core::proxy_cmd::cost_simulator::simulator_est_cost,
    mutating aidog_core::system_cmd::app_log::app_log_frontend,
}

/// SSE 心跳间隔：穿过反向代理 / NAT 时防止空闲连接被掐。
const SSE_KEEPALIVE_SECS: u64 = 15;

/// `/events`：每个连接一个 broadcast 订阅。事件名 = `emit` 的第一个参数，data = payload 的
/// JSON 原文，形状与 Tauri 侧一字不差。订阅者跟不上而丢的事件直接跳过（刷新语义幂等）。
async fn events_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    use futures::StreamExt;
    let rx = aidog_ctx::subscribe_events();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|item| async move {
        item.ok()
            .map(|ev| Ok(Event::default().event(ev.name).data(ev.payload.to_string())))
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(SSE_KEEPALIVE_SECS))
            .text("keep-alive"),
    )
}

/// 管理接口的完整路由：`/rpc/<命令>` + `/events`（路径相对 `/api`）。
/// 内核 `--ui` 在此之上再 `nest_service("/ui", ...)`。
pub fn management_router() -> Router {
    rpc_router::<()>()
        .layer(axum::middleware::from_fn(secret_fields::guard))
        .route("/events", get(events_handler))
}
