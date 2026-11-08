//! agent 配置一键卸载（magpie 票 05）。
//!
//! 与 [`crate::ai_tools_cmd::mcp::self_install`] / claude / codex / pi 配置生成配对：
//! 把 aidog 写进本机 agent 配置的内容一键移除并恢复原值。三个原则：
//! - **merge-by-name 类**（MCP）：按 aidog 命名键删自己的条目，不碰用户其余内容。
//! - **文件生成类**：删除前按标记 / 指纹确认由 aidog 生成（aidog 自有目录 `~/.aidog`、
//!   DB 托管快照、TOML 结构指纹），是 → 删；否 → 移除 aidog 注入段、保留其余。
//! - **覆盖用户已有键**：install 侧写前备份原值（DB scope=[BACKUP_SCOPE]），
//!   uninstall 恢复原值；无备份（键原本不存在）→ 删除 aidog 形状键。
//!
//! 高危面（用户家目录文件）与 `mcp_self_install` 同级授权语义：执行前靠 UI 确认，
//! command 本身只做已确认动作（`dry_run=true` 仅返回清单不落盘）。

use std::path::{Path, PathBuf};

use aidog_db::models::SetSettingInput;
use aidog_db::Db;
use serde::Serialize;

use crate::ai_tools_cmd::mcp::SELF_INSTALL_NAME;

/// DB `setting` 表 scope：install 覆盖用户已有键前的原值备份。
/// key 形如 `claude_json.ANTHROPIC_BASE_URL`；uninstall 恢复后删除该行。
pub(crate) const BACKUP_SCOPE: &str = "agent_config_backup";

pub(crate) const KEY_CLAUDE_BASE_URL: &str = "claude_json.ANTHROPIC_BASE_URL";
pub(crate) const KEY_PRIMARY_API_KEY: &str = "claude_config.primaryApiKey";
pub(crate) const KEY_ONBOARDING: &str = "claude_json.hasCompletedOnboarding";

/// uninstall 单个动作（dry_run 清单与执行报告共用形状）。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct UninstallAction {
    /// 写入面：mcp / claude_env / claude_tools / claude_settings /
    /// group_settings / codex_profile / codex_notify / pi / hooks。
    pub surface: &'static str,
    /// 目标（文件路径 / 键名 / db:<key>）。
    pub target: String,
    /// 操作：remove / restore / restore_or_remove / delete_file。
    pub op: &'static str,
    /// 人读说明。
    pub detail: String,
    /// 执行阶段单面失败的原因（best-effort：单面失败不中断其余面）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 卸载步骤（内部；对应 [`UninstallAction`]，apply 时消费）。
#[derive(Debug)]
enum Step {
    /// 从 `~/.claude.json` / `~/.codex/config.toml` 移除自装 MCP 条目（按名）。
    McpSelfRemove,
    /// 删 DB `mcp_server` 行（aidog 自装记录）。
    McpDbRowRemove,
    /// 覆盖型键：有备份恢复原值（并删备份行），无备份删 aidog 形状键。
    RestoreOrRemoveKey {
        path: PathBuf,
        key: &'static str,
        backup_key: &'static str,
    },
    /// 从 `~/.claude/settings.json` 移除托管叶子（按 DB 快照 dot-path），清快照。
    RemoveManagedLeaves,
    /// 仅清 DB 托管快照（settings.json 已不存在时）。
    ClearManagedSnapshot,
    /// 删单个文件（分组 settings / codex profile / hook 脚本；指纹已验）。
    DeleteFile(PathBuf),
    /// 从 `~/.codex/config.toml` 移除 aidog 默认 profile（复用既有守卫）。
    CodexDefaultProfileRemove,
    /// 移除 codex `notify`（remove_codex_notify 仅删指向 aidog 脚本的值）。
    CodexNotifyRemove,
    /// pi：空组 sync —— 删全部 `aidog-` 前缀 provider + 我们的 defaultProvider。
    PiRemove,
}

// ─── install 侧备份 helper ─────────────────────────────────────

/// install 覆盖用户已有键前备份原值（magpie 票 05）。已有备份（更早的用户原值）
/// 不覆盖 —— 重复 install 不丢最早的备份。
/// `old` 为 None → 备份 null 哨兵（键原本不存在，uninstall 删键）。
/// review #5：`old == new`（写同值）也备份——备份语义是「卸载后回到用户安装前
/// 状态」；不备份则 uninstall 无备份分支会删掉用户自己的键。
pub(crate) async fn backup_before_overwrite(
    db: &Db,
    backup_key: &str,
    old: Option<serde_json::Value>,
) {
    let existing = aidog_db::get_setting(db, BACKUP_SCOPE, backup_key)
        .await
        .ok()
        .flatten();
    if existing.is_some() {
        return;
    }
    let value = old.unwrap_or(serde_json::Value::Null);
    let _ = aidog_db::set_setting(
        db,
        SetSettingInput {
            scope: BACKUP_SCOPE.to_string(),
            key: backup_key.to_string(),
            value,
        },
    )
    .await;
}

// ─── 收集（dry_run 清单 = 纯读）与执行 ─────────────────────────

/// 收集将执行的卸载动作。纯读：不写文件、不改 DB。
async fn collect_actions(db: &Db) -> Result<Vec<(Step, UninstallAction)>, String> {
    let mut steps: Vec<(Step, UninstallAction)> = Vec::new();
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;

    // MCP 自装条目（~/.claude.json / ~/.codex/config.toml，按名；merge-by-name 只删本条目）
    let agents = crate::gateway::mcp::agents_with_entry(SELF_INSTALL_NAME);
    if !agents.is_empty() {
        steps.push((
            Step::McpSelfRemove,
            act(
                "mcp",
                SELF_INSTALL_NAME,
                "remove",
                format!("从 agent 配置移除自装 MCP 条目（{}）", agents.join("、")),
            ),
        ));
    }
    // DB 自装行
    if crate::gateway::mcp::store::get_mcp_server(db, SELF_INSTALL_NAME)
        .await?
        .is_some()
    {
        steps.push((
            Step::McpDbRowRemove,
            act("mcp", format!("db:{SELF_INSTALL_NAME}"), "remove", "删除 DB mcp_server 自装记录"),
        ));
    }

    // 覆盖型键：install 写前有备份 → 恢复原值；无备份（键原本不存在）→ 删 aidog 形状键
    let dot_json = home.join(".claude.json");
    let cj = read_json(&dot_json)?;
    if is_aidog_base_url(cj.get("ANTHROPIC_BASE_URL").unwrap_or(&serde_json::Value::Null)) {
        steps.push((
            Step::RestoreOrRemoveKey {
                path: dot_json.clone(),
                key: "ANTHROPIC_BASE_URL",
                backup_key: KEY_CLAUDE_BASE_URL,
            },
            act("claude_env", "ANTHROPIC_BASE_URL", "restore_or_remove", "恢复/移除代理 base_url 注入"),
        ));
    }
    if cj.get("hasCompletedOnboarding").and_then(|v| v.as_bool()) == Some(true) {
        steps.push((
            Step::RestoreOrRemoveKey {
                path: dot_json,
                key: "hasCompletedOnboarding",
                backup_key: KEY_ONBOARDING,
            },
            act("claude_env", "hasCompletedOnboarding", "restore_or_remove", "恢复/移除跳过 onboarding 注入"),
        ));
    }
    let config_json = home.join(".claude").join("config.json");
    let cc = read_json(&config_json)?;
    if cc.get("primaryApiKey").and_then(|v| v.as_str()) == Some("any") {
        steps.push((
            Step::RestoreOrRemoveKey {
                path: config_json,
                key: "primaryApiKey",
                backup_key: KEY_PRIMARY_API_KEY,
            },
            act("claude_tools", "primaryApiKey", "restore_or_remove", "恢复/移除插件 primaryApiKey=any"),
        ));
    }

    // 托管叶子：按 DB 快照（sync_settings MANAGED_SCOPE/MANAGED_KEY）删
    // ~/.claude/settings.json 的叶子 + 清快照；文件不在只清快照
    let managed = aidog_db::get_setting(
        db,
        crate::sync_settings::MANAGED_SCOPE,
        crate::sync_settings::MANAGED_KEY,
    )
    .await?;
    let leaves: Vec<String> = managed
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    if !leaves.is_empty() {
        let settings_json = home.join(".claude").join("settings.json");
        if settings_json.exists() {
            steps.push((
                Step::RemoveManagedLeaves,
                act("claude_settings", leaves.join(", "), "remove", "按 DB 快照移除默认组托管叶子"),
            ));
        }
        steps.push((
            Step::ClearManagedSnapshot,
            act("claude_settings", "db:managed_paths", "remove", "清除 DB 托管叶子快照"),
        ));
    }

    // 分组 settings 文件（~/.aidog 自有目录，文件名 settings.*.json 即指纹）
    let data_dir = home.join(".aidog");
    if let Ok(entries) = std::fs::read_dir(&data_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("settings.") && name.ends_with(".json") {
                steps.push((
                    Step::DeleteFile(e.path()),
                    act("group_settings", name, "delete_file", "删除 aidog 分组 settings 文件"),
                ));
            }
        }
    }

    // codex 分组 profile：TOML 指纹确认 aidog 生成才删（用户自己的 *.config.toml 保留）
    if let Some(dir) = codex_home_dir()
        && let Ok(entries) = std::fs::read_dir(&dir)
    {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name == "config.toml" || !name.ends_with(".config.toml") {
                continue;
            }
            let parsed = std::fs::read_to_string(e.path())
                .ok()
                .and_then(|c| toml::from_str::<toml::Value>(&c).ok());
            if parsed.as_ref().is_some_and(is_aidog_codex_profile_toml) {
                steps.push((
                    Step::DeleteFile(e.path()),
                    act("codex_profile", name, "delete_file", "删除 aidog 生成的 codex 分组 profile"),
                ));
            }
        }
    }

    // codex 默认 profile + notify（config.toml；remove_* 自带 aidog 守卫）
    let codex_cfg = crate::gateway::codex::codex_config_read()?;
    if is_aidog_codex_profile(&codex_cfg) {
        steps.push((
            Step::CodexDefaultProfileRemove,
            act(
                "codex_profile",
                "config.toml: model_provider / model_providers.aidog",
                "remove",
                "移除 codex 默认 profile 的 aidog 注入",
            ),
        ));
    }
    if is_aidog_notify(codex_cfg.get("notify").unwrap_or(&serde_json::Value::Null)) {
        steps.push((
            Step::CodexNotifyRemove,
            act("codex_notify", "config.toml: notify", "remove", "移除指向 aidog 通知脚本的 notify"),
        ));
    }

    // pi：aidog- 前缀 provider 或我们写的 defaultProvider（空组 sync 只删自己的）
    if let Some(pi_dir) = pi_agent_dir() {
        let pm = read_json(&pi_dir.join("models.json"))?;
        let ps = read_json(&pi_dir.join("settings.json"))?;
        let prefix = crate::gateway::pi::PROVIDER_PREFIX;
        let has_aidog_provider = pm
            .get("providers")
            .and_then(|p| p.as_object())
            .is_some_and(|m| m.keys().any(|k| k.starts_with(prefix)));
        let has_aidog_default = ps
            .get("defaultProvider")
            .and_then(|v| v.as_str())
            .is_some_and(|v| v.starts_with(prefix));
        if has_aidog_provider || has_aidog_default {
            steps.push((
                Step::PiRemove,
                act(
                    "pi",
                    "models.json / settings.json",
                    "remove",
                    "移除 aidog- 前缀 provider 与我们写的 defaultProvider",
                ),
            ));
        }
    }

    // hook 通知脚本（~/.aidog/scripts 自有目录，按 aidog_hooks 脚本名）
    let scripts_dir = home.join(".aidog").join("scripts");
    for filename in [
        aidog_hooks::SCRIPT_COMPLETE,
        aidog_hooks::SCRIPT_EVENT_NOTIFY,
        aidog_hooks::LEGACY_SCRIPT_COMPLETE,
        aidog_hooks::LEGACY_SCRIPT_WAITING,
    ] {
        let p = scripts_dir.join(filename);
        if p.exists() {
            steps.push((
                Step::DeleteFile(p),
                act("hooks", filename, "delete_file", "删除 aidog hook 通知脚本"),
            ));
        }
    }

    Ok(steps)
}

/// 执行全部收集到的步骤。best-effort：单步失败记 [`UninstallAction::error`] 继续。
async fn apply(db: &Db, step: Step) -> Result<(), String> {
    match step {
        Step::McpSelfRemove => {
            crate::gateway::mcp::remove_named_from_agents(SELF_INSTALL_NAME);
            Ok(())
        }
        Step::McpDbRowRemove => {
            crate::gateway::mcp::store::delete_mcp_server(db, SELF_INSTALL_NAME).await
        }
        Step::RestoreOrRemoveKey {
            path,
            key,
            backup_key,
        } => {
            let backup = aidog_db::get_setting(db, BACKUP_SCOPE, backup_key).await?;
            let mut root = read_json(&path)?;
            let mut keep_backup_row = false;
            match backup {
                // null 哨兵 / 无备份：键原本不存在，删 aidog 形状键
                Some(serde_json::Value::Null) | None => {
                    if let Some(obj) = root.as_object_mut() {
                        obj.remove(key);
                    }
                }
                // 有备份：恢复用户原值（install 覆盖前的值）。
                // 恢复值 == 当前值（写同值场景，review #5）→ 保留备份行：
                // 二次卸载不再走删键分支，用户原值永不被删。
                Some(v) => {
                    let cur = root.get(key).cloned();
                    let obj = root
                        .as_object_mut()
                        .ok_or_else(|| format!("{} root is not an object", path.display()))?;
                    obj.insert(key.to_string(), v);
                    keep_backup_row = cur.as_ref() == obj.get(key);
                }
            }
            write_json(&path, &root)?;
            if keep_backup_row {
                Ok(())
            } else {
                aidog_db::delete_setting(db, BACKUP_SCOPE, backup_key).await
            }
        }
        Step::RemoveManagedLeaves => {
            let managed = aidog_db::get_setting(
                db,
                crate::sync_settings::MANAGED_SCOPE,
                crate::sync_settings::MANAGED_KEY,
            )
            .await?;
            let leaves = managed
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
            let path = home.join(".claude").join("settings.json");
            let mut root = read_json(&path)?;
            for leaf in leaves.iter().filter_map(|v| v.as_str()) {
                remove_leaf(&mut root, leaf);
            }
            if root.as_object().is_some_and(|o| o.is_empty()) {
                std::fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))?;
            } else {
                write_json(&path, &root)?;
            }
            Ok(())
        }
        Step::ClearManagedSnapshot => {
            aidog_db::delete_setting(
                db,
                crate::sync_settings::MANAGED_SCOPE,
                crate::sync_settings::MANAGED_KEY,
            )
            .await
        }
        Step::DeleteFile(path) => {
            if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|e| format!("remove {}: {e}", path.display()))?;
            }
            Ok(())
        }
        Step::CodexDefaultProfileRemove => {
            crate::gateway::codex::remove_default_profile_from_config()?;
            Ok(())
        }
        Step::CodexNotifyRemove => {
            let mut cfg = crate::gateway::codex::codex_config_read()?;
            aidog_hooks::remove_codex_notify(&mut cfg);
            crate::gateway::codex::codex_config_write(cfg)
        }
        Step::PiRemove => {
            crate::gateway::pi::sync_groups(&[], 0, &crate::gateway::pi::PiSettings::default())?;
            Ok(())
        }
    }
}

/// dry_run 清单：只收集不执行。
pub(crate) async fn plan_actions(db: &Db) -> Result<Vec<UninstallAction>, String> {
    Ok(collect_actions(db)
        .await?
        .into_iter()
        .map(|(_, a)| a)
        .collect())
}

/// 执行卸载，返回逐面结果（含单面失败原因）。
pub(crate) async fn run_uninstall(db: &Db) -> Result<Vec<UninstallAction>, String> {
    let steps = collect_actions(db).await?;
    let mut out = Vec::with_capacity(steps.len());
    for (step, mut action) in steps {
        if let Err(e) = apply(db, step).await {
            action.error = Some(e);
        }
        out.push(action);
    }
    Ok(out)
}

crate::tauri_command! {
    /// 一键卸载 aidog 写进本机 agent 配置的全部内容（magpie 票 05）。
    /// `dry_run=true` 只返回将执行的动作清单不落盘；`false` 执行（best-effort，
    /// 单面失败记 error 继续）。高危面（用户家目录文件）：前端执行前须二次确认，
    /// 与 `mcp_self_install` 同级授权语义。
    pub async fn agent_config_uninstall(
        dry_run: bool) -> Result<Vec<UninstallAction>, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "agent_config_uninstall", dry_run, "command invoked");
        if dry_run {
            plan_actions(db).await
        } else {
            run_uninstall(db).await
        }
    }
}

// ─── 指纹判定与文件小工具 ──────────────────────────────────────

/// 构造 [`UninstallAction`]（error 恒 None，只在 apply 阶段填）。
fn act(
    surface: &'static str,
    target: impl Into<String>,
    op: &'static str,
    detail: impl Into<String>,
) -> UninstallAction {
    UninstallAction {
        surface,
        target: target.into(),
        op,
        detail: detail.into(),
        error: None,
    }
}

/// `$CODEX_HOME`（缺省 `~/.codex`），与 `gateway::codex` 同口径；不创建目录。
pub(crate) fn codex_home_dir() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("CODEX_HOME")
        && !custom.trim().is_empty()
    {
        return Some(PathBuf::from(custom));
    }
    dirs::home_dir().map(|h| h.join(".codex"))
}

/// pi agent 目录（`PI_CODING_AGENT_DIR`，缺省 `~/.pi/agent`），与 `gateway::pi`
/// 同口径；不创建目录。
pub(crate) fn pi_agent_dir() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("PI_CODING_AGENT_DIR")
        && !custom.trim().is_empty()
    {
        return Some(PathBuf::from(custom));
    }
    dirs::home_dir().map(|h| h.join(".pi").join("agent"))
}

/// codex 分组 profile TOML 的 aidog 指纹（[`toml::Value`] 形态，语义同
/// [`is_aidog_codex_profile`]）。
pub(crate) fn is_aidog_codex_profile_toml(t: &toml::Value) -> bool {
    t.get("model_provider").and_then(|v| v.as_str()) == Some("aidog")
        || t.get("model_providers").and_then(|p| p.get("aidog")).is_some()
}

/// `ANTHROPIC_BASE_URL` 的 aidog 指纹：值含 aidog 代理路径段。
pub(crate) fn is_aidog_base_url(v: &serde_json::Value) -> bool {
    v.as_str()
        .map(|s| s.contains("/claude/v1/messages"))
        .unwrap_or(false)
}

/// codex profile（config.toml / `<group>.config.toml`）的 aidog 指纹：
/// 顶层 `model_provider == "aidog"` 或存在 `[model_providers.aidog]`。
fn is_aidog_codex_profile(v: &serde_json::Value) -> bool {
    v.get("model_provider").and_then(|x| x.as_str()) == Some("aidog")
        || v.get("model_providers").and_then(|p| p.get("aidog")).is_some()
}

/// codex `notify` 的 aidog 指纹：数组里有元素指向 aidog 脚本
/// （与 aidog_hooks `references_aidog_script` 同口径：脚本名 aidog-notify*）。
fn is_aidog_notify(v: &serde_json::Value) -> bool {
    v.as_array().map(|a| {
        a.iter()
            .any(|x| x.as_str().map(|s| s.contains("aidog-notify")).unwrap_or(false))
    })
    .unwrap_or(false)
}

/// 读 JSON 文件；缺失 / 空返回空对象，损坏报错。
fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::Value::Object(Default::default()));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(serde_json::Value::Object(Default::default()));
    }
    serde_json::from_str(&content).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// pretty 写回 JSON；父目录不存在自动创建（review #4：走同目录临时文件 +
/// rename 的 [`super::profile::atomic_write`]，禁非原子半写）。
fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|e| format!("serialize {}: {e}", path.display()))?;
    super::profile::atomic_write(path, &content, None)
}

/// 按 dot-path 删除叶子键；删空的对象父级逐级上剪（根不动，由调用方决定删文件）。
fn remove_leaf(root: &mut serde_json::Value, leaf: &str) {
    let parts: Vec<&str> = leaf.split('.').collect();
    fn rec(v: &mut serde_json::Value, parts: &[&str]) -> bool {
        if let Some(o) = v.as_object_mut() {
            if parts.len() == 1 {
                return o.remove(parts[0]).is_some();
            }
            let removed = o.get_mut(parts[0]).map(|child| rec(child, &parts[1..]));
            if let Some(true) = removed
                && o.get(parts[0])
                    .and_then(|c| c.as_object())
                    .map(|c| c.is_empty())
                    .unwrap_or(false)
            {
                o.remove(parts[0]);
            }
            return removed.unwrap_or(false);
        }
        false
    }
    rec(root, &parts);
}

#[cfg(test)]
#[path = "test_agent_config.rs"]
mod test_agent_config;
