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
        // popover 失焦即隐 (v1 handler)。tao macOS windowDidResignKey → Rust 此回调
        // (同步派发, 先于 webview IPC); 旧实现走 popover.tsx onFocusChanged (依赖 webview
        // 就绪 + JS→Rust IPC), 实测 macOS 偶发不触发.
        // 此 handler 仅覆盖「点主窗口」场景 (主窗接 key 触发 popover resignKey);
        // 其余 3 失活场景 (点桌面 / silent_launch 主窗 hide 后点别处 / 点 Dock 菜单栏空白)
        // 由 app_setup.rs 的 NSWindow.setHidesOnDeactivate:YES 覆盖 (app 失活即隐藏).
        // 窗口复用：hide 而非 destroy，保留 webview + NSWindow 指针，下次 show 秒显。
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(false) = event
                && window.label() == "popover" {
                    let _ = window.hide();
                }
        })
        .setup(|app| crate::app_setup::setup(app))
        .invoke_handler(tauri::generate_handler![
            // Platform
            aidog_core::platform_cmd::platform::platform_create,
            aidog_core::platform_cmd::platform::platform_list,
            aidog_core::platform_cmd::platform::platform_get,
            aidog_core::platform_cmd::platform::platform_share_export,
            aidog_core::platform_cmd::platform::platform_share_parse,
            aidog_core::platform_cmd::platform::platform_update,
            aidog_core::platform_cmd::platform::platform_delete,
            aidog_core::platform_cmd::platform::platform_purge_disabled,
            aidog_core::platform_cmd::platform::platform_purge_disabled_preview,
            aidog_core::platform_cmd::platform::platform_ensure_auto_group,
            aidog_core::platform_cmd::platform::platform_set_tray,
            aidog_core::platform_cmd::model_fetch::platform_fetch_models,
            // Batch Operations
            aidog_core::platform_cmd::batch::batch_delete_platforms,
            aidog_core::platform_cmd::batch::batch_override_models,
            aidog_core::platform_cmd::batch::batch_set_status,
            aidog_core::platform_cmd::batch::batch_move_group,
            // Tray Config
            aidog_core::platform_cmd::platform::tray_config_get,
            aidog_core::platform_cmd::platform::tray_config_set,
            aidog_core::platform_cmd::platform::tray_today_stats,
            aidog_core::platform_cmd::stats::stats_query_batch,
            aidog_core::popover::popover_data,
            aidog_core::popover::popover_config_get,
            aidog_core::popover::popover_config_set,
            aidog_core::popover::popover_platform_today,
            // Group
            aidog_core::platform_cmd::group::group_create,
            aidog_core::platform_cmd::group::group_list,
            aidog_core::platform_cmd::group::group_get,
            aidog_core::platform_cmd::group::group_update,
            aidog_core::platform_cmd::group::group_delete,
            aidog_core::platform_cmd::group::group_set_default,
            // GroupPlatform
            aidog_core::platform_cmd::group::group_set_platforms,
            aidog_core::platform_cmd::group::group_get_platforms,
            // Aggregate
            aidog_core::platform_cmd::group::group_detail,
            aidog_core::platform_cmd::group::group_detail_list,
            aidog_core::platform_cmd::group::group_detail_list_paged,
            aidog_core::platform_cmd::group::group_reorder,
            aidog_core::platform_cmd::group::group_platform_reorder,
            aidog_core::platform_cmd::group::group_platform_set_level_priority,
            aidog_core::platform_cmd::group::group_platform_move,
            // UI 态持久化（_ui_* extra 键）
            aidog_core::platform_cmd::ui_extra::set_ui_extra,
            // Proxy
            aidog_core::proxy_cmd::proxy::proxy_start,
            aidog_core::proxy_cmd::proxy::proxy_stop,
            aidog_core::proxy_cmd::proxy::proxy_status,
            aidog_core::proxy_cmd::proxy::proxy_get_settings,
            aidog_core::proxy_cmd::proxy::proxy_set_autostart,
            aidog_core::proxy_cmd::proxy::proxy_set_bind_lan,
            aidog_core::proxy_cmd::proxy::app_set_autolaunch,
            aidog_core::proxy_cmd::proxy::app_get_autolaunch,
            aidog_core::proxy_cmd::proxy::app_set_silent_launch,
            // Proxy Client Settings
            aidog_core::proxy_cmd::proxy::proxy_client_get_settings,
            aidog_core::proxy_cmd::proxy::proxy_client_set_settings,
            // Config Export
            aidog_core::sync_settings::export_claude_config,
            aidog_core::sync_settings::sync_group_settings,
            aidog_core::sync_settings::get_managed_paths,
            // Proxy Logs
            aidog_core::proxy_cmd::proxy_log::proxy_log_list,
            aidog_core::proxy_cmd::proxy_log::proxy_log_list_filtered,
            aidog_core::proxy_cmd::proxy_log::proxy_log_count_filtered,
            aidog_core::proxy_cmd::proxy_log::proxy_log_distinct_models,
            aidog_core::proxy_cmd::proxy_log::proxy_log_get,
            aidog_core::proxy_cmd::proxy_log::proxy_log_clear,
            aidog_core::proxy_cmd::proxy_log::proxy_log_count,
            aidog_core::proxy_cmd::proxy_log::proxy_log_settings_get,
            aidog_core::proxy_cmd::proxy_log::proxy_log_settings_set,
            aidog_core::proxy_cmd::proxy_log::proxy_log_cleanup_expired,
            // Request Logs (cli-proxy test/quota page)
            aidog_core::proxy_cmd::proxy_log::request_log_list,
            // Stats aggregation settings + rebuild
            aidog_core::platform_cmd::stats::stats_settings_get,
            aidog_core::platform_cmd::stats::stats_settings_set,
            aidog_core::platform_cmd::stats::stats_rebuild_from_logs,
            // DB Maintenance (Tier 1: VACUUM reclaim)
            aidog_core::system_cmd::backup::db_compact,
            // Proxy Timeout
            aidog_core::proxy_cmd::proxy_timeout::proxy_timeout_get,
            aidog_core::proxy_cmd::proxy_timeout::proxy_timeout_set,
            // Middleware Rule Engine (C1)
            aidog_core::proxy_cmd::middleware::middleware_list_rules,
            aidog_core::proxy_cmd::middleware::middleware_create_rule,
            aidog_core::proxy_cmd::middleware::middleware_update_rule,
            aidog_core::proxy_cmd::middleware::middleware_delete_rule,
            aidog_core::proxy_cmd::middleware::middleware_settings_get,
            aidog_core::proxy_cmd::middleware::middleware_settings_set,
            aidog_core::proxy_cmd::middleware::middleware_import_default_rules,
            aidog_core::system_cmd::scheduling::scheduling_settings_get,
            aidog_core::system_cmd::scheduling::scheduling_settings_set,
            // Notification (N1)
            aidog_core::system_cmd::notification::notification_settings_get,
            aidog_core::system_cmd::notification::notification_settings_set,
            aidog_core::system_cmd::notification::notification_inbox_list,
            aidog_core::system_cmd::notification::notification_clear,
            aidog_core::system_cmd::notification::notification_test,
            aidog_core::system_cmd::notification::notification_test_tts,
            aidog_core::system_cmd::notification::notification_test_popup,
            aidog_core::system_cmd::notification::notification_test_beep,
            // Notification Hook Integration (N2)
            aidog_core::hooks::inject_hooks,
            aidog_core::hooks::remove_hooks,
            aidog_core::hooks::get_default_hooks_enabled,
            aidog_core::hooks::set_default_hooks_enabled,
            aidog_core::hooks::build_notify_hooks_fragment,
            // 脚本执行器（uv / python3）
            aidog_core::ai_tools_cmd::script_executor::check_uv,
            aidog_core::ai_tools_cmd::script_executor::install_uv,
            aidog_core::ai_tools_cmd::script_executor::set_script_executor,
            // Skills 管理
            aidog_core::ai_tools_cmd::skills::skills_check_env,
            aidog_core::ai_tools_cmd::skills::skills_browse_catalog,
            aidog_core::ai_tools_cmd::skills::skills_search,
            aidog_core::ai_tools_cmd::skills::skills_list_installed,
            aidog_core::ai_tools_cmd::skills::skills_list_refresh,
            aidog_core::ai_tools_cmd::skills::skills_enable,
            aidog_core::ai_tools_cmd::skills::skills_install,
            aidog_core::ai_tools_cmd::skills::skill_detail,
            aidog_core::ai_tools_cmd::skills::skill_read_file,
            aidog_core::ai_tools_cmd::skills::skills_disable,
            aidog_core::ai_tools_cmd::skills::skills_update,
            aidog_core::ai_tools_cmd::skills::skills_uninstall_all,
            aidog_core::ai_tools_cmd::skills::skills_uninstall,
            aidog_core::ai_tools_cmd::skills::skills_align_agents,
            aidog_core::ai_tools_cmd::skills::skills_enable_all,
            // MCP 管理
            aidog_core::ai_tools_cmd::mcp::mcp_list,
            aidog_core::ai_tools_cmd::mcp::mcp_scan,
            aidog_core::ai_tools_cmd::mcp::mcp_import,
            aidog_core::ai_tools_cmd::mcp::mcp_import_json,
            aidog_core::ai_tools_cmd::mcp::mcp_set_agent,
            aidog_core::ai_tools_cmd::mcp::mcp_delete,
            aidog_core::ai_tools_cmd::mcp::mcp_update,
            aidog_core::ai_tools_cmd::mcp::mcp_add,
            aidog_core::ai_tools_cmd::mcp::mcp_resync,
            aidog_core::ai_tools_cmd::mcp::mcp_share_export,
            // 导入导出子系统
            aidog_core::system_cmd::backup::export_to_file,
            aidog_core::system_cmd::backup::export_preview,
            aidog_core::system_cmd::backup::backup_settings_get,
            aidog_core::system_cmd::backup::backup_settings_set,
            aidog_core::system_cmd::backup::backup_run_now,
            aidog_core::system_cmd::backup::import_read_file,
            aidog_core::system_cmd::backup::import_apply,
            aidog_core::system_cmd::backup::ccswitch_detect,
            aidog_core::system_cmd::backup::ccswitch_read,
            aidog_core::system_cmd::backup::ccswitch_import,
            aidog_core::system_cmd::backup::sub2api_parse,
            aidog_core::system_cmd::backup::sub2api_read_file,
            aidog_core::system_cmd::backup::sub2api_import,
            // App Logging
            aidog_core::system_cmd::app_log::app_log_settings_get,
            aidog_core::system_cmd::app_log::app_log_settings_set,
            // Auto-update toggle (gates startup daily check; manual button unaffected)
            aidog_core::system_cmd::auto_update::get_auto_update_enabled,
            aidog_core::system_cmd::auto_update::set_auto_update_enabled,
            // CC / Codex integration toggles
            aidog_core::ai_tools_cmd::coding_tools::coding_tools_settings_get,
            aidog_core::ai_tools_cmd::coding_tools::coding_tools_settings_set,
            // Settings
            aidog_core::system_cmd::fs_autocomplete::fs_autocomplete,
            aidog_core::settings::settings_get,
            aidog_core::settings::settings_set,
            aidog_core::settings::settings_delete,
            aidog_core::settings::settings_list,
            aidog_core::settings::generate_statusline_script,
            aidog_core::settings::read_claude_code_settings,
            // Codex Config
            aidog_core::gateway::codex::codex_config_read,
            aidog_core::gateway::codex::codex_config_write,
            aidog_core::gateway::codex::codex_config_path,
            // Statistics
            aidog_core::platform_cmd::stats::stats_query,
            aidog_core::ai_tools_cmd::model_test::model_test,
            // Platform Usage
            aidog_core::proxy_cmd::proxy_log::platform_usage_stats,
            aidog_core::proxy_cmd::proxy_log::group_usage_stats,
            aidog_core::proxy_cmd::proxy_log::all_group_usage_stats,
            aidog_core::proxy_cmd::proxy_log::all_platform_usage_stats,
            aidog_core::proxy_cmd::proxy_log::get_last_test_result,
            // Platform Quota
            aidog_core::platform_cmd::quota::platform_query_quota,
            aidog_core::platform_cmd::quota::platform_query_quota_newapi,
            aidog_core::platform_cmd::quota::platform_query_quota_devin,
            aidog_core::platform_cmd::platform::platform_reorder,
            // CLI Proxy（cpa-standalone-module s3）：provider CRUD + test + platform + import
            aidog_core::cli_proxy_cmd::provider::cli_proxy_list,
            aidog_core::cli_proxy_cmd::provider::cli_proxy_get,
            aidog_core::cli_proxy_cmd::provider::cli_proxy_create,
            aidog_core::cli_proxy_cmd::provider::cli_proxy_update,
            aidog_core::cli_proxy_cmd::provider::cli_proxy_delete,
            aidog_core::cli_proxy_cmd::test_cmd::cli_proxy_test,
            aidog_core::cli_proxy_cmd::platform::create_cli_proxy_platform,
            aidog_core::cli_proxy_cmd::import::cli_proxy_import,
            // CLI Proxy batch ops（cli-proxy-batch-delete s1）
            aidog_core::cli_proxy_cmd::batch::batch_delete_cli_proxy_providers,
            aidog_core::cli_proxy_cmd::batch::batch_override_cli_proxy_models,
            aidog_core::cli_proxy_cmd::batch::batch_set_cli_proxy_quota,
            // Model Prices
            aidog_core::platform_cmd::price::model_price_list,
            aidog_core::platform_cmd::price::model_price_count,
            aidog_core::platform_cmd::price::model_price_search,
            aidog_core::platform_cmd::price::model_price_list_filtered,
            aidog_core::platform_cmd::price::model_price_count_filtered,
            aidog_core::platform_cmd::price::model_price_resolve,
            aidog_core::platform_cmd::price::model_price_sync,
            aidog_core::platform_cmd::price::price_sync_settings_get,
            aidog_core::platform_cmd::price::price_sync_settings_set,
            // About
            aidog_core::system_cmd::about::about_info,
            // CLI 工具环境（Claude Code / Codex 版本 / 安装 / 升级 / 冲突诊断）
            aidog_core::cli_env::cli_check_versions,
            aidog_core::cli_env::cli_install,
            aidog_core::cli_env::cli_upgrade,
            aidog_core::cli_env::cli_diagnose_conflicts,
            aidog_core::cli_env::cli_check_updates,
            // Platform defaults JSON
            aidog_core::defaults::get_defaults_json,
            aidog_core::defaults::sync_defaults_json,
            aidog_core::defaults::get_protocol_logo_path,
            aidog_core::defaults::sync_protocol_logo,
            // Client types JSON (13 client_type entries, sync 链同 defaults_sync)
            aidog_core::defaults::get_client_types_json,
            aidog_core::defaults::sync_client_types_json,
            // MITM (P3 ST7) — 白名单配置 + CA 安装状态/引导
            aidog_core::proxy_cmd::mitm::mitm_status,
            aidog_core::proxy_cmd::mitm::mitm_enable,
            aidog_core::proxy_cmd::mitm::mitm_disable,
            aidog_core::proxy_cmd::mitm::mitm_install_ca_prepare,
            aidog_core::proxy_cmd::mitm::mitm_uninstall_ca_prepare,
            aidog_core::proxy_cmd::mitm::mitm_set_ca_installed,
            aidog_core::proxy_cmd::mitm::mitm_classify_trust_error,
            aidog_core::proxy_cmd::mitm::mitm_whitelist_add,
            aidog_core::proxy_cmd::mitm::mitm_whitelist_remove,
            aidog_core::proxy_cmd::mitm::mitm_whitelist_toggle,
            aidog_core::proxy_cmd::mitm::mitm_whitelist_import_defaults,
            aidog_core::proxy_cmd::mitm::mitm_whitelist_clear,
            aidog_core::proxy_cmd::mitm::mitm_whitelist_test_url,
            // C8 mitm seam 收敛：手动清空 pinning_suspect 集合（TTL 内强制重试 MITM）
            aidog_core::proxy_cmd::mitm::mitm_reset_suspects,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
