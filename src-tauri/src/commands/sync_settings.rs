use crate::shared::*;
use crate::commands::hooks::{generate_hook_scripts, enabled_hook_events};
use crate::gateway::{self, db::Db};
#[allow(unused_imports)]
use crate::logging;
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


#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub fn export_claude_config(port: u16, _app: tauri::AppHandle) -> Result<String, String> {
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
pub(crate) async fn try_sync_settings(app: &tauri::AppHandle, db: &Db) {
    if let Ok(settings) = load_proxy_settings(app).await {
        if let Err(e) = do_sync_group_settings(db, settings.port).await {
            tracing::warn!(port = settings.port, error = %e, "sync group settings failed");
        }
    }
}

/// aidog 托管字段 marker 键：写入 `~/.claude/settings.json`，值为 dot-path 字符串数组，
/// 记录 aidog 默认分组**实际注入/托管**的字段路径（叶子级）。前端「从 Claude Code 导入」
/// 的字段级 diff 读此 marker 精确排除托管路径，只列用户自加（非托管）的差异。
/// `_aidog_` 前缀 → 前端 `buildImportDiffTree` 顶层自动跳过；CC 忽略未知 key。
pub const MARKER_MANAGED: &str = "_aidog_managed";

/// 递归收集 JSON object 的叶子 dot-path（如 `env.ANTHROPIC_BASE_URL`、`enabledPlugins.x@y`）。
/// - object → 递归每个键，拼 `prefix.key`
/// - 非 object（标量/数组/null）→ 当前 prefix 即为一个叶子 path
/// - 跳过 `_aidog_` 前缀键（内部 marker，非真实托管字段）
///
/// 用于写入侧把默认组实际写入的字段路径记入托管集（单一事实源）。
fn collect_leaf_paths(value: &serde_json::Value, prefix: &str, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if k.starts_with("_aidog_") {
                    continue;
                }
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                collect_leaf_paths(v, &path, out);
            }
        }
        _ => {
            if !prefix.is_empty() {
                out.push(prefix.to_string());
            }
        }
    }
}

/// 默认分组：把默认组 config deep merge 写入 `~/.claude/settings.json`（CC 全局）。
///
/// deep merge 规则：aidog 管理字段（env.ANTHROPIC_BASE_URL/AUTH_TOKEN、statusLine、
/// hooks 等）覆盖同键；用户手写的其它字段（permissions / model 等）保留。
/// 嵌套 object 递归合并；非 object（标量/数组）直接覆盖。
///
/// 托管 marker：aidog 注入字段的叶子 dot-path（含 base_config 全字段 + env 注入 + hooks +
/// enabledPlugins/mcpServers 中 aidog 自身写入的条目）写入 `_aidog_managed`。用户事后用命令
/// 自加的条目（不在本次注入集）不进托管集 → 导入 diff 仍能列出。
///
/// CC 原生支持 settings.json 的 env 字段 → 用户直接 `claude` 不带任何参数/env 即走该组。
pub(crate) fn write_default_claude_settings(config: &serde_json::Value) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir)
        .map_err(|e| format!("create ~/.claude dir: {e}"))?;
    let settings_path = claude_dir.join("settings.json");

    // 读现有（不存在→空对象）
    let existing = std::fs::read_to_string(&settings_path).ok();
    let mut base: serde_json::Value = match existing.as_deref() {
        Some(s) if !s.trim().is_empty() => serde_json::from_str(s)
            .map_err(|e| format!("parse existing ~/.claude/settings.json: {e}"))?,
        _ => serde_json::Value::Object(serde_json::Map::new()),
    };
    if !base.is_object() {
        base = serde_json::Value::Object(serde_json::Map::new());
    }

    // 托管集：aidog 本次注入的叶子 dot-path（基于 config，跳过内部 marker）。
    // 顺序稳定（递归 + serde_json Map 保插入序），便于幂等 diff。
    let mut managed: Vec<String> = Vec::new();
    collect_leaf_paths(config, "", &mut managed);

    // deep merge：config 叠加到 base（不覆盖用户自加的 enabledPlugins/mcpServers 条目）
    merge_json(&mut base, config);

    // 写入/更新托管 marker（替换旧值，反映本次实际注入集）。
    if let Some(obj) = base.as_object_mut() {
        obj.insert(
            MARKER_MANAGED.to_string(),
            serde_json::Value::Array(
                managed
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    let new_content = serde_json::to_string_pretty(&base)
        .map_err(|e| format!("serialize merged ~/.claude/settings.json: {e}"))?;
    let old_content = existing.unwrap_or_default();
    if old_content == new_content {
        return Ok(());
    }

    std::fs::write(&settings_path, &new_content)
        .map_err(|e| format!("write ~/.claude/settings.json: {e}"))?;
    tracing::info!(path = %settings_path.display(), "default group: merged ~/.claude/settings.json");
    Ok(())
}

/// JSON deep merge：overlay 叠加到 base（in-place）。
/// - overlay 非 object → 直接覆盖 base（*base = overlay.clone()）
/// - overlay 为 object → 逐键递归合并；base 非 object 时先升级为空 object
/// - overlay 中显式 null → 删 base 同键（等同 strip）
pub(crate) fn merge_json(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match overlay {
        serde_json::Value::Object(over_map) => {
            if !base.is_object() {
                *base = serde_json::Value::Object(serde_json::Map::new());
            }
            let base_map = base.as_object_mut().expect("ensured object");
            for (k, v) in over_map {
                if v.is_null() {
                    base_map.remove(k);
                    continue;
                }
                match base_map.get_mut(k) {
                    Some(existing) => merge_json(existing, v),
                    None => {
                        base_map.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        _ => {
            *base = overlay.clone();
        }
    }
}

/// 为所有分组生成 settings.{group_key}.json 配置文件到 ~/.aidog/ 目录
/// 核心逻辑：可被多个触发点调用
pub(crate) async fn do_sync_group_settings(db: &Db, port: u16) -> Result<Vec<String>, String> {
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
            serde_json::from_str(include_str!("../../defaults/settings.json"))
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

    // 默认分组捕获：循环内为默认组算出的 config（已 strip 内部 marker），循环结束后
    // merge 写入 ~/.claude/settings.json 全局。None = 无默认组（循环后跳过全局写入）。
    let mut default_claude_config: Option<serde_json::Value> = None;

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

        // 捕获默认组 config（已 strip 内部 marker），循环后 merge 写 ~/.claude/settings.json。
        if group.is_default {
            default_claude_config = Some(config.clone());
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

    // 默认分组全局 merge：把默认组 config deep merge 写入 ~/.claude/settings.json
    // （用户全局，CC 原生支持 settings.json 的 env 字段 → 完整免参数免 env）。
    // 同时 merge 写入 ~/.codex/config.toml（注入 aidog profile，codex env_key=AIDOG_KEY
    // 固有限制需用户 export AIDOG_KEY=<group_key>，UI 提示说明）。
    // 无默认组 → 不主动清除已写入的全局文件（避免误删用户手写配置）；仅 Codex 全局
    // remove 仅在明确取消默认（group_set_default(None) 路径）触发，本同步路径不主动清。
    match default_claude_config {
        Some(config) => {
            if let Err(e) = write_default_claude_settings(&config) {
                tracing::warn!(error = %e, "default group: merge ~/.claude/settings.json failed");
            }
            if let Err(e) = gateway::codex::write_default_profile_to_config(port) {
                tracing::warn!(error = %e, "default group: merge ~/.codex/config.toml failed");
            }
        }
        None => {
            // 无默认组：移除 aidog 之前注入的全局默认 profile（若曾注入过）。
            // 仅删 aidog 标识，用户其它字段保留。
            tracing::debug!("no default group, removing aidog global default profile");
            if let Err(e) = gateway::codex::remove_default_profile_from_config() {
                tracing::warn!(error = %e, "no default group: remove codex default profile failed");
            }
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
pub async fn sync_group_settings(app: tauri::AppHandle, db: State<'_, Db>) -> Result<Vec<String>, String> {
    tracing::debug!(command = "sync_group_settings", "command invoked");
    let proxy_settings = load_proxy_settings(&app).await?;
    do_sync_group_settings(&db, proxy_settings.port).await
        .map_err(|e| { tracing::error!(command = "sync_group_settings", error = %e, "sync group settings failed"); e })
}

#[cfg(test)]
#[path = "test_sync_settings.rs"]
mod test_sync_settings;
