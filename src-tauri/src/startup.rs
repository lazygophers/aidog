//! App entry：tauri Builder + invoke_handler 注册（命令实现见 commands/ 子模块）。
#[allow(unused_imports)]
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // rustls 0.23 需显式装 process-level CryptoProvider（ring），否则首次 TLS builder() panic。
    // 测试侧各自 install_default，生产侧在此统一装一次（幂等，AlreadyInstalled 返 Err 无害）。
    let _ = rustls::crypto::ring::default_provider().install_default();
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
        // P3 MITM：装假 CA 到系统信任库。shell scope 在 capabilities/mitm-ca.json 限定仅装/卸 CA 命令。
        .plugin(tauri_plugin_shell::init())
        // popover 失焦即关 (v1 handler)。tao macOS windowDidResignKey → Rust 此回调
        // (同步派发, 先于 webview IPC); 旧实现走 popover.tsx onFocusChanged (依赖 webview
        // 就绪 + JS→Rust IPC), 实测 macOS 偶发不触发.
        // 此 handler 仅覆盖「点主窗口」场景 (主窗接 key 触发 popover resignKey);
        // 其余 3 失活场景 (点桌面 / silent_launch 主窗 hide 后点别处 / 点 Dock 菜单栏空白)
        // 由 app_setup.rs 的 NSWindow.setHidesOnDeactivate:YES 覆盖 (app 失活即隐藏).
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(false) = event {
                if window.label() == "popover" {
                    let _ = window.destroy();
                }
            }
        })
        .setup(|app| crate::app_setup::setup(app))
        .invoke_handler(tauri::generate_handler![
            // Platform
            commands_platform::platform::platform_create,
            commands_platform::platform::platform_list,
            commands_platform::platform::platform_get,
            commands_platform::platform::platform_share_export,
            commands_platform::platform::platform_share_parse,
            commands_platform::platform::platform_update,
            commands_platform::platform::platform_delete,
            commands_platform::platform::platform_purge_disabled,
            commands_platform::platform::platform_ensure_auto_group,
            commands_platform::platform::platform_set_tray,
            commands_platform::model_fetch::platform_fetch_models,
            // Tray Config
            commands_platform::platform::tray_config_get,
            commands_platform::platform::tray_config_set,
            commands_platform::platform::tray_today_stats,
            commands_platform::stats::stats_query_batch,
            crate::commands::popover::popover_data,
            crate::commands::popover::popover_config_get,
            crate::commands::popover::popover_config_set,
            crate::commands::popover::popover_platform_today,
            // Group
            commands_platform::group::group_create,
            commands_platform::group::group_list,
            commands_platform::group::group_get,
            commands_platform::group::group_update,
            commands_platform::group::group_delete,
            commands_platform::group::group_set_default,
            // GroupPlatform
            commands_platform::group::group_set_platforms,
            commands_platform::group::group_get_platforms,
            // Aggregate
            commands_platform::group::group_detail,
            commands_platform::group::group_detail_list,
            commands_platform::group::group_detail_list_paged,
            commands_platform::group::group_reorder,
            commands_platform::group::group_platform_reorder,
            commands_platform::group::group_platform_set_level_priority,
            commands_platform::group::group_platform_move,
            // Proxy
            commands_proxy::proxy::proxy_start,
            commands_proxy::proxy::proxy_stop,
            commands_proxy::proxy::proxy_status,
            commands_proxy::proxy::proxy_get_settings,
            commands_proxy::proxy::proxy_set_autostart,
            commands_proxy::proxy::proxy_set_bind_lan,
            commands_proxy::proxy::app_set_autolaunch,
            commands_proxy::proxy::app_get_autolaunch,
            commands_proxy::proxy::app_set_silent_launch,
            // Proxy Client Settings
            commands_proxy::proxy::proxy_client_get_settings,
            commands_proxy::proxy::proxy_client_set_settings,
            // Config Export
            aidog_core::sync_settings::export_claude_config,
            aidog_core::sync_settings::sync_group_settings,
            aidog_core::sync_settings::get_managed_paths,
            // Proxy Logs
            commands_proxy::proxy_log::proxy_log_list,
            commands_proxy::proxy_log::proxy_log_list_filtered,
            commands_proxy::proxy_log::proxy_log_count_filtered,
            commands_proxy::proxy_log::proxy_log_get,
            commands_proxy::proxy_log::proxy_log_clear,
            commands_proxy::proxy_log::proxy_log_count,
            commands_proxy::proxy_log::proxy_log_settings_get,
            commands_proxy::proxy_log::proxy_log_settings_set,
            commands_proxy::proxy_log::proxy_log_cleanup_expired,
            // Stats aggregation settings + rebuild
            commands_platform::stats::stats_settings_get,
            commands_platform::stats::stats_settings_set,
            commands_platform::stats::stats_rebuild_from_logs,
            // DB Maintenance (Tier 1: VACUUM reclaim)
            crate::commands::backup::db_compact,
            // Proxy Timeout
            commands_proxy::proxy_timeout::proxy_timeout_get,
            commands_proxy::proxy_timeout::proxy_timeout_set,
            // Middleware Rule Engine (C1)
            commands_proxy::middleware::middleware_list_rules,
            commands_proxy::middleware::middleware_create_rule,
            commands_proxy::middleware::middleware_update_rule,
            commands_proxy::middleware::middleware_delete_rule,
            commands_proxy::middleware::middleware_settings_get,
            commands_proxy::middleware::middleware_settings_set,
            commands_proxy::middleware::middleware_import_default_rules,
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
            aidog_core::hooks::inject_hooks,
            aidog_core::hooks::remove_hooks,
            aidog_core::hooks::get_default_hooks_enabled,
            aidog_core::hooks::set_default_hooks_enabled,
            aidog_core::hooks::build_notify_hooks_fragment,
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
            crate::commands::mcp::mcp_share_export,
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
            // Auto-update toggle (gates startup daily check; manual button unaffected)
            crate::commands::auto_update::get_auto_update_enabled,
            crate::commands::auto_update::set_auto_update_enabled,
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
            aidog_core::gateway::codex::codex_config_read,
            aidog_core::gateway::codex::codex_config_write,
            aidog_core::gateway::codex::codex_config_path,
            // Statistics
            commands_platform::stats::stats_query,
            crate::commands::model_test::model_test,
            // Platform Usage
            commands_proxy::proxy_log::platform_usage_stats,
            commands_proxy::proxy_log::group_usage_stats,
            commands_proxy::proxy_log::all_group_usage_stats,
            commands_proxy::proxy_log::all_platform_usage_stats,
            commands_proxy::proxy_log::get_last_test_result,
            // Platform Quota
            commands_platform::quota::platform_query_quota,
            commands_platform::quota::platform_query_quota_newapi,
            commands_platform::platform::platform_reorder,
            // Model Prices
            commands_platform::price::model_price_list,
            commands_platform::price::model_price_count,
            commands_platform::price::model_price_search,
            commands_platform::price::model_price_list_filtered,
            commands_platform::price::model_price_count_filtered,
            commands_platform::price::model_price_resolve,
            commands_platform::price::model_price_sync,
            commands_platform::price::price_sync_settings_get,
            commands_platform::price::price_sync_settings_set,
            // About
            crate::commands::about::about_info,
            // CLI 工具环境（Claude Code / Codex 版本 / 安装 / 升级 / 冲突诊断）
            crate::commands::cli_env::cli_check_versions,
            crate::commands::cli_env::cli_install,
            crate::commands::cli_env::cli_upgrade,
            crate::commands::cli_env::cli_diagnose_conflicts,
            crate::commands::cli_env::cli_check_updates,
            // Platform defaults JSON
            crate::commands::defaults::get_defaults_json,
            crate::commands::defaults::sync_defaults_json,
            crate::commands::defaults::get_protocol_logo_path,
            crate::commands::defaults::sync_protocol_logo,
            // Client types JSON (13 client_type entries, sync 链同 defaults_sync)
            crate::commands::defaults::get_client_types_json,
            crate::commands::defaults::sync_client_types_json,
            // MITM (P3 ST7) — 白名单配置 + CA 安装状态/引导
            commands_proxy::mitm::mitm_status,
            commands_proxy::mitm::mitm_enable,
            commands_proxy::mitm::mitm_disable,
            commands_proxy::mitm::mitm_install_ca_prepare,
            commands_proxy::mitm::mitm_uninstall_ca_prepare,
            commands_proxy::mitm::mitm_set_ca_installed,
            commands_proxy::mitm::mitm_classify_trust_error,
            commands_proxy::mitm::mitm_whitelist_add,
            commands_proxy::mitm::mitm_whitelist_remove,
            commands_proxy::mitm::mitm_whitelist_toggle,
            commands_proxy::mitm::mitm_whitelist_import_defaults,
            commands_proxy::mitm::mitm_whitelist_clear,
            commands_proxy::mitm::mitm_whitelist_test_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
