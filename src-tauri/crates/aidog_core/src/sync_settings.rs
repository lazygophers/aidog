use crate::gateway;
use crate::hooks::{enabled_hook_events, generate_hook_scripts};
use crate::shared::*;
use aidog_db::Db;

crate::tauri_command! {
pub fn export_claude_config(port: u16) -> Result<String, String> {
    tracing::debug!(command = "export_claude_config", port, "command invoked");
    let base_url = format!("http://localhost:{}/claude/v1/messages", port);
    let config_path = dirs::home_dir()
        .ok_or("cannot resolve home directory")?
        .join(".claude.json");

    // 读取已有配置
    let mut config: serde_json::Value = if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).map_err(|e| format!("read config: {e}"))?;
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

    let content =
        serde_json::to_string_pretty(&config).map_err(|e| format!("serialize config: {e}"))?;
    std::fs::write(&config_path, content).map_err(|e| {
        tracing::error!(command = "export_claude_config", error = %e, "write .claude.json failed");
        format!("write config: {e}")
    })?;

    Ok(config_path.to_string_lossy().to_string())
}
}

/// Helper: attempt sync, log errors but don't propagate
pub async fn try_sync_settings(db: &Db) {
    if let Ok(settings) = load_proxy_settings(db).await
        && let Err(e) = do_sync_group_settings(db, settings.port).await
    {
        tracing::warn!(port = settings.port, error = %e, "sync group settings failed");
    }
}

/// DB 存储托管叶子快照的 scope/key。复用 KV `setting` 表，不加新表/列。
/// value = JSON 字符串数组（dot-path 叶子集），前端 invoke `get_managed_paths` 读。
pub const MANAGED_SCOPE: &str = "claude_default_group";
pub const MANAGED_KEY: &str = "managed_paths";

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

/// 默认分组：把默认组 config **全量覆盖**写入 `~/.claude/settings.json`（CC 全局）。
///
/// 设了默认分组 = aidog 接管该文件：写入内容就是默认组 config，用户手写但 aidog 侧
/// 没有的键（permissions / model / 自装 plugins / mcpServers …）一并消失。旧行为是
/// deep merge 保留用户字段，语义上「谁是真值源」始终模糊，现改为单一真值源。
/// 不备份 —— 内容可随时从 aidog 设置页重新生成。
///
/// 只在内容真的变化时落盘（`old == new` 早退），所以「设置变更 / 默认分组变更」才会
/// 触发实际写入，重复同步不动文件。
///
/// 托管快照存 aidog 内部 DB `setting` 表（scope=`claude_default_group`/key=`managed_paths`），
/// = 本次同步后**整个 settings.json** 的全部叶子 dot-path。全量覆盖下这就是 config 自身的
/// 叶子集。前端「从 Claude Code 导入」的字段级 diff 排除该集合 → 仅显示同步**之后**用户在
/// CC 侧新增/改动的字段。`collect_leaf_paths` 跳过 `_aidog_` 前缀。
///
/// 旧版曾往 settings.json 写 `_aidog_managed` key（现已迁 DB）；全量覆盖后旧值自然消失，
/// 无需再显式 remove。
///
/// CC 原生支持 settings.json 的 env 字段 → 用户直接 `claude` 不带任何参数/env 即走该组。
pub async fn write_default_claude_settings(
    db: &Db,
    config: &serde_json::Value,
) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let claude_dir = home.join(".claude");
    std::fs::create_dir_all(&claude_dir).map_err(|e| format!("create ~/.claude dir: {e}"))?;
    let settings_path = claude_dir.join("settings.json");

    // 全量覆盖：写入内容 = 默认组 config，不读旧文件内容参与合并。
    let base = config.clone();

    // 托管集：对写入内容取叶子 dot-path（跳过内部 marker）。即「上次同步时 settings.json
    // 全部叶子」的快照，导入 diff 只显示此快照之后的新增/变化。顺序稳定（递归 + serde_json
    // Map 保插入序），便于幂等 diff。
    let mut managed: Vec<String> = Vec::new();
    collect_leaf_paths(&base, "", &mut managed);

    // 写托管快照入 DB（KV 复用，单一事实源；前端 invoke 读）。
    aidog_db::set_setting(
        db,
        gateway::models::SetSettingInput {
            scope: MANAGED_SCOPE.to_string(),
            key: MANAGED_KEY.to_string(),
            value: serde_json::Value::Array(
                managed.into_iter().map(serde_json::Value::String).collect(),
            ),
        },
    )
    .await?;

    let new_content = serde_json::to_string_pretty(&base)
        .map_err(|e| format!("serialize ~/.claude/settings.json: {e}"))?;
    // 内容未变则不落盘：同步在启动/改分组/改设置等多处触发，只有真变化才写文件。
    let old_content = std::fs::read_to_string(&settings_path).unwrap_or_default();
    if old_content == new_content {
        return Ok(());
    }

    std::fs::write(&settings_path, &new_content)
        .map_err(|e| format!("write ~/.claude/settings.json: {e}"))?;
    tracing::info!(path = %settings_path.display(), "default group: overwrote ~/.claude/settings.json");
    Ok(())
}

/// JSON deep merge：overlay 叠加到 base（in-place）。
/// - overlay 非 object → 直接覆盖 base（*base = overlay.clone()）
/// - overlay 为 object → 逐键递归合并；base 非 object 时先升级为空 object
/// - overlay 中显式 null → 删 base 同键（等同 strip）
pub fn merge_json(base: &mut serde_json::Value, overlay: &serde_json::Value) {
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

/// 一个分组在 pi `/model` 里能选到的模型候选：分组模型映射的对外模型名 ∪ 各关联平台
/// 的有效模型，按出现顺序去重。pi 的自定义 provider 必须自带 models 才能选模型，
/// 全空会生成一个选不了模型的废 provider —— 因此回落到 `/models` 那份静态默认清单
/// （同一份真值源，不另起第二份）。
fn pi_model_candidates(
    mappings: &[aidog_db::models::ModelMapping],
    platform_models: &[aidog_db::models::PlatformModels],
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = mappings
        .iter()
        .map(|m| m.source_model.trim().to_string())
        .chain(platform_models.iter().flat_map(|m| m.all_values()))
        .filter(|m| !m.is_empty())
        .filter(|m| seen.insert(m.clone()))
        .collect();

    if out.is_empty() {
        out = gateway::proxy::STATIC_MODEL_IDS
            .iter()
            .map(|m| m.to_string())
            .collect();
    }
    out
}

/// 出站代理 URL：取 claude config 的 `env.HTTPS_PROXY`，退 `env.HTTP_PROXY`。
/// 这两个键就是「Coding 设置」代理卡片写入的位置，pi 有原生 `httpProxy` 设置项承接。
fn proxy_url_from_config(config: &serde_json::Value) -> Option<String> {
    let env = config.get("env")?;
    ["HTTPS_PROXY", "HTTP_PROXY"]
        .into_iter()
        .find_map(|k| env.get(k).and_then(|v| v.as_str()))
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// 为所有分组生成 settings.{group_key}.json 配置文件到 ~/.aidog/ 目录
/// 核心逻辑：可被多个触发点调用
pub async fn do_sync_group_settings(db: &Db, port: u16) -> Result<Vec<String>, String> {
    let groups = aidog_db::list_groups(db).await?;

    let aidog_dir = dirs::home_dir()
        .ok_or("cannot resolve home directory")?
        .join(".aidog");

    // Ensure ~/.aidog/ exists
    std::fs::create_dir_all(&aidog_dir).map_err(|e| format!("create .aidog dir: {e}"))?;

    // Load base claude code config from app settings (scope=global, key=claude_code)
    // Fallback to compiled-in defaults when DB has no config
    let base_config: serde_json::Value = aidog_db::get_setting(db, "global", "claude_code")
        .await
        .ok()
        .flatten()
        .filter(|v| v.is_object() && v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(|| {
            serde_json::from_str(include_str!("../../../defaults/settings.json"))
                .unwrap_or(serde_json::Value::Object(Default::default()))
        });

    // Collect current group names for cleanup
    let group_keys: std::collections::HashSet<String> =
        groups.iter().map(|g| g.group_key.clone()).collect();

    // 默认通知 hook 物化（镜像 statusLine）：marker `_aidog_hooks.enabled` 为 true 时，
    // 为每个分组 config 注入 hooks.Stop/Notification（strip marker 之前），并对 Codex
    // 全局 config.toml 一次性注入/移除 notify。脚本只生成一次（循环外）。
    let hooks_enabled = aidog_hooks::hooks_marker_enabled(&base_config);
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
                // 注入用户自定义 env_vars（group 维度）。aidog 强写的 proxy 路由字段
                // ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN 禁止覆盖 —— 同名 key 丢弃 + warn。
                for ev in &group.env_vars {
                    let key = ev.key.trim();
                    if key.is_empty() {
                        continue;
                    }
                    if key == "ANTHROPIC_BASE_URL" || key == "ANTHROPIC_AUTH_TOKEN" {
                        tracing::warn!(
                            group = %group_key, env_key = %key,
                            "user env_var skipped: aidog-managed routing field, cannot override"
                        );
                        continue;
                    }
                    env_map.insert(key.to_string(), serde_json::Value::String(ev.value.clone()));
                }
            }
        }

        // 默认通知 hook 物化：marker 开启时为本组 config 注入 CC hooks（strip marker 之前）。
        // N2：遍历 inject_events（enabled 事件）注入，每个指向通用脚本 command。
        if let Some(scripts) = &hook_scripts {
            aidog_hooks::inject_claude_code_hooks(&mut config, scripts, &inject_events);
        }

        // Strip internal aidog UI state — not real Claude Code fields.
        if let Some(obj) = config.as_object_mut() {
            obj.remove("_aidog_statusline");
            obj.remove("_aidog_subagent_statusline");
            obj.remove(aidog_hooks::MARKER_HOOKS);
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
            if let Err(e) = write_default_claude_settings(db, &config).await {
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
                    aidog_hooks::inject_codex_notify(&mut config, &scripts.complete);
                }
                _ => {
                    aidog_hooks::remove_codex_notify(&mut config);
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
            if let Some(group_key) = name
                .strip_prefix("settings.")
                .and_then(|s| s.strip_suffix(".json"))
                && !group_keys.contains(group_key)
                && let Err(e) = std::fs::remove_file(entry.path())
            {
                tracing::debug!(group = %group_key, error = %e, "remove stale settings file failed");
            }
        }
    }

    // Cleanup: remove Codex profile files for deleted groups（用户级 config.toml 不动）。
    if let Err(e) = gateway::codex::cleanup_group_profiles(&group_keys) {
        tracing::warn!(error = %e, "codex profile cleanup failed");
    }

    // pi：所有分组共用同一份 `~/.pi/agent/models.json`（pi 只认单一全局文件），因此
    // 必须在循环外一次性写入全部分组 —— 删除的分组由 aidog- 前缀清扫顺带消失，
    // 不需要单独一趟 cleanup。pi 未装也不应阻塞，失败仅记录。
    // 模型候选要平台维度，故取 details（`groups` 只有分组本体）。
    let pi_groups: Vec<gateway::pi::PiGroup> = aidog_db::list_group_details(db)
        .await?
        .iter()
        .map(|d| gateway::pi::PiGroup {
            group_key: d.group.group_key.clone(),
            models: pi_model_candidates(
                &d.group.model_mappings,
                &d.platforms
                    .iter()
                    .map(|gp| gp.platform.models.clone())
                    .collect::<Vec<_>>(),
            ),
            api: gateway::pi::parse_group_api(&d.group.extra),
        })
        .collect();
    let pi_settings = gateway::pi::PiSettings {
        default_group: groups
            .iter()
            .find(|g| g.is_default)
            .map(|g| g.group_key.clone()),
        http_proxy: proxy_url_from_config(&base_config),
    };
    match gateway::pi::sync_groups(&pi_groups, port, &pi_settings) {
        Ok(paths) => written.extend(paths),
        Err(e) => tracing::warn!(error = %e, "pi config sync failed"),
    }

    Ok(written)
}

crate::tauri_command! {
/// Tauri command — manual sync from UI
pub async fn sync_group_settings() -> Result<Vec<String>, String> {
    let db = aidog_ctx::db();
    let proxy_settings = load_proxy_settings(&db).await?;
    do_sync_group_settings(&db, proxy_settings.port).await
        .map_err(|e| { tracing::error!(command = "sync_group_settings", error = %e, "sync group settings failed"); e })
}
}

crate::tauri_command! {
/// 读默认分组托管叶子 dot-path 快照（DB `setting` 表 scope=`claude_default_group`/
/// key=`managed_paths`）。前端「从 Claude Code 导入」字段级 diff 据此排除托管字段，
/// 只列用户新增/改动。空/缺省 → 空数组（diff 降级为不排除，零回归）。
pub async fn get_managed_paths() -> Result<Vec<String>, String> {
    let db = aidog_ctx::db();
    let v = aidog_db::get_setting(&db, MANAGED_SCOPE, MANAGED_KEY).await?;
    Ok(v.and_then(|val| {
        val.as_array().map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
    })
    .unwrap_or_default())
}
}

#[cfg(test)]
#[path = "test_sync_settings.rs"]
mod test_sync_settings;
