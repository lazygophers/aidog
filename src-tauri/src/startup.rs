//! App entry：tauri Builder + invoke_handler 注册（命令实现见 commands/ 子模块）。
#[allow(unused_imports)]
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
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // aidog:// deep link 协议：scheme 注册（macOS bundle / Win registry / Linux .desktop）
        // + URL 唤起回调。setup 阶段经 DeepLinkExt 挂 on_open_url + 冷启动 get_current 补发。
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| crate::app_setup::setup(app))
        .invoke_handler(tauri::generate_handler![
            // Platform
            crate::commands::platform::platform_create,
            crate::commands::platform::platform_list,
            crate::commands::platform::platform_get,
            crate::commands::platform::platform_share_export,
            crate::commands::platform::platform_share_parse,
            crate::commands::platform::platform_update,
            crate::commands::platform::platform_delete,
            crate::commands::platform::platform_purge_disabled,
            crate::commands::platform::platform_ensure_auto_group,
            crate::commands::platform::platform_set_tray,
            crate::commands::model_fetch::platform_fetch_models,
            // Tray Config
            crate::commands::platform::tray_config_get,
            crate::commands::platform::tray_config_set,
            crate::commands::platform::tray_today_stats,
            crate::commands::stats::stats_query_batch,
            crate::commands::popover::popover_data,
            crate::commands::popover::popover_config_get,
            crate::commands::popover::popover_config_set,
            crate::commands::popover::popover_platform_today,
            // Group
            crate::commands::group::group_create,
            crate::commands::group::group_list,
            crate::commands::group::group_get,
            crate::commands::group::group_update,
            crate::commands::group::group_delete,
            crate::commands::group::group_set_default,
            // GroupPlatform
            crate::commands::group::group_set_platforms,
            crate::commands::group::group_get_platforms,
            // Aggregate
            crate::commands::group::group_detail,
            crate::commands::group::group_detail_list,
            crate::commands::group::group_detail_list_paged,
            crate::commands::group::group_reorder,
            crate::commands::group::group_platform_reorder,
            crate::commands::group::group_platform_set_level_priority,
            crate::commands::group::group_platform_move,
            // Proxy
            crate::commands::proxy::proxy_start,
            crate::commands::proxy::proxy_stop,
            crate::commands::proxy::proxy_status,
            crate::commands::proxy::proxy_get_settings,
            crate::commands::proxy::proxy_set_autostart,
            crate::commands::proxy::proxy_set_bind_lan,
            crate::commands::proxy::app_set_autolaunch,
            crate::commands::proxy::app_get_autolaunch,
            crate::commands::proxy::app_set_silent_launch,
            // Proxy Client Settings
            crate::commands::proxy::proxy_client_get_settings,
            crate::commands::proxy::proxy_client_set_settings,
            // Config Export
            crate::commands::sync_settings::export_claude_config,
            crate::commands::sync_settings::sync_group_settings,
            crate::commands::sync_settings::get_managed_paths,
            // Proxy Logs
            crate::commands::proxy_log::proxy_log_list,
            crate::commands::proxy_log::proxy_log_list_filtered,
            crate::commands::proxy_log::proxy_log_count_filtered,
            crate::commands::proxy_log::proxy_log_get,
            crate::commands::proxy_log::proxy_log_clear,
            crate::commands::proxy_log::proxy_log_count,
            crate::commands::proxy_log::proxy_log_settings_get,
            crate::commands::proxy_log::proxy_log_settings_set,
            // Stats aggregation settings + rebuild
            crate::commands::stats::stats_settings_get,
            crate::commands::stats::stats_settings_set,
            crate::commands::stats::stats_rebuild_from_logs,
            // DB Maintenance (Tier 1: VACUUM reclaim)
            crate::commands::backup::db_compact,
            // Proxy Timeout
            crate::commands::proxy_timeout::proxy_timeout_get,
            crate::commands::proxy_timeout::proxy_timeout_set,
            // Middleware Rule Engine (C1)
            crate::commands::middleware::middleware_list_rules,
            crate::commands::middleware::middleware_create_rule,
            crate::commands::middleware::middleware_update_rule,
            crate::commands::middleware::middleware_delete_rule,
            crate::commands::middleware::middleware_settings_get,
            crate::commands::middleware::middleware_settings_set,
            crate::commands::scheduling::scheduling_settings_get,
            crate::commands::scheduling::scheduling_settings_set,
            // Notification (N1)
            crate::commands::notification::notification_settings_get,
            crate::commands::notification::notification_settings_set,
            crate::commands::notification::notification_inbox_list,
            crate::commands::notification::notification_clear,
            crate::commands::notification::notification_test,
            crate::commands::notification::notification_test_tts,
            crate::commands::notification::notification_test_popup,
            crate::commands::notification::notification_test_beep,
            // Notification Hook Integration (N2)
            crate::commands::hooks::inject_hooks,
            crate::commands::hooks::remove_hooks,
            crate::commands::hooks::get_default_hooks_enabled,
            crate::commands::hooks::set_default_hooks_enabled,
            crate::commands::hooks::build_notify_hooks_fragment,
            // 脚本执行器（uv / python3）
            crate::commands::script_executor::check_uv,
            crate::commands::script_executor::install_uv,
            crate::commands::script_executor::set_script_executor,
            // Skills 管理
            crate::commands::skills::skills_check_env,
            crate::commands::skills::skills_browse_catalog,
            crate::commands::skills::skills_search,
            crate::commands::skills::skills_list_installed,
            crate::commands::skills::skills_list_refresh,
            crate::commands::skills::skills_enable,
            crate::commands::skills::skills_install,
            crate::commands::skills::skill_detail,
            crate::commands::skills::skill_read_file,
            crate::commands::skills::skills_disable,
            crate::commands::skills::skills_update,
            crate::commands::skills::skills_uninstall_all,
            crate::commands::skills::skills_uninstall,
            crate::commands::skills::skills_align_agents,
            crate::commands::skills::skills_enable_all,
            // MCP 管理
            crate::commands::mcp::mcp_list,
            crate::commands::mcp::mcp_scan,
            crate::commands::mcp::mcp_import,
            crate::commands::mcp::mcp_import_json,
            crate::commands::mcp::mcp_set_agent,
            crate::commands::mcp::mcp_delete,
            crate::commands::mcp::mcp_update,
            crate::commands::mcp::mcp_add,
            crate::commands::mcp::mcp_resync,
            // 导入导出子系统
            crate::commands::backup::export_to_file,
            crate::commands::backup::export_preview,
            crate::commands::backup::backup_settings_get,
            crate::commands::backup::backup_settings_set,
            crate::commands::backup::backup_run_now,
            crate::commands::backup::import_read_file,
            crate::commands::backup::import_apply,
            crate::commands::backup::ccswitch_detect,
            crate::commands::backup::ccswitch_read,
            crate::commands::backup::ccswitch_import,
            crate::commands::backup::sub2api_parse,
            crate::commands::backup::sub2api_read_file,
            crate::commands::backup::sub2api_import,
            // App Logging
            crate::commands::app_log::app_log_settings_get,
            crate::commands::app_log::app_log_settings_set,
            // CC / Codex integration toggles
            crate::commands::coding_tools::coding_tools_settings_get,
            crate::commands::coding_tools::coding_tools_settings_set,
            // Settings
            crate::commands::fs_autocomplete::fs_autocomplete,
            crate::commands::settings::settings_get,
            crate::commands::settings::settings_set,
            crate::commands::settings::settings_delete,
            crate::commands::settings::settings_list,
            crate::commands::settings::generate_statusline_script,
            crate::commands::settings::read_claude_code_settings,
            // Codex Config
            crate::gateway::codex::codex_config_read,
            crate::gateway::codex::codex_config_write,
            crate::gateway::codex::codex_config_path,
            // Statistics
            crate::commands::stats::stats_query,
            crate::commands::model_test::model_test,
            // Platform Usage
            crate::commands::proxy_log::platform_usage_stats,
            crate::commands::proxy_log::group_usage_stats,
            crate::commands::proxy_log::all_group_usage_stats,
            crate::commands::proxy_log::all_platform_usage_stats,
            crate::commands::proxy_log::get_last_test_result,
            // Platform Quota
            crate::commands::quota::platform_query_quota,
            crate::commands::quota::platform_query_quota_newapi,
            crate::commands::platform::platform_reorder,
            // Model Prices
            crate::commands::price::model_price_list,
            crate::commands::price::model_price_count,
            crate::commands::price::model_price_search,
crate::commands::price::model_price_list_filtered,
crate::commands::price::model_price_count_filtered,
            crate::commands::price::model_price_resolve,
            crate::commands::price::model_price_sync,
            crate::commands::price::price_sync_settings_get,
            crate::commands::price::price_sync_settings_set,
            // About
            crate::commands::about::about_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
