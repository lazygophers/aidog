//! agent 配置 Profile（方案）快照：保存 / 一键切回（magpie 票 06）。
//!
//! 快照 = 05 盘点的全部 agent 配置文件面（claude / codex / pi / aidog 自有）
//! 及分组→agent 映射所需的 DB 数据引用（group 行 + group_platform 对，
//! 复用 import_export 的 upsert / relink 逻辑，不整库导出）。
//!
//! 存储：`~/.aidog/profiles/<name>.json`（aidog 自有数据目录，文件名即方案名，
//! 带 schema_version）。
//!
//! apply 原子写回（临时文件 + rename，禁半写）；快照里不存在的文件 →
//! apply 删除（还原「未装」状态）；同名 save 需显式 overwrite。
//!
//! 高危面（用户家目录文件）与 `agent_config_uninstall` 同级授权语义：
//! 前端先 `dry_run=true` 弹差异清单，确认后才 `dry_run=false` 执行。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use aidog_db::Db;
use serde::{Deserialize, Serialize};

use super::agent_config::{codex_home_dir, is_aidog_base_url, is_aidog_codex_profile_toml, pi_agent_dir};

#[cfg(test)]
#[path = "test_profile.rs"]
mod test_profile;

/// 快照格式版本（破坏性形状变更时递增；apply 只认认识到的版本）。
pub(crate) const SCHEMA_VERSION: u32 = 1;

/// 快照里的单个文件条目。`content = None` 表示快照时该文件不存在。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileFile {
    /// 稳定标识：静态面（`claude_dot_json` 等）或 `kind:文件名`
    /// （`codex_profile:` / `aidog_settings:` / `aidog_script:`）。
    pub key: String,
    /// 文件全文（文本）；None = 快照时不存在。
    pub content: Option<String>,
    /// unix 权限位（含可执行位；非 unix / 取不到 = None）。apply 后回设。
    #[serde(default)]
    pub mode: Option<u32>,
}

/// 完整快照 payload（`~/.aidog/profiles/<name>.json` 的 JSON 形状）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilePayload {
    pub schema_version: u32,
    pub name: String,
    /// Unix 秒。
    pub created_at: u64,
    pub aidog_version: String,
    pub files: Vec<ProfileFile>,
    /// `group` 表行（serde_json::Value，复用 import_export 的行级 upsert）。
    pub groups: Vec<serde_json::Value>,
    /// (group_key, platform_name)。
    pub group_platform: Vec<[String; 2]>,
}

/// 列表项元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub name: String,
    pub created_at: u64,
}

/// 单条差异 / 应用结果（dry_run 清单与执行报告共用形状）。
#[derive(Debug, Clone, Serialize)]
pub struct ProfileDiffItem {
    /// 文件 key 或 `db:groups`。
    pub target: String,
    /// create / update / delete / skip。
    pub op: &'static str,
    /// 人读说明。
    pub detail: String,
    /// 执行阶段单面失败原因（best-effort：单面失败不中断其余面）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ─── 名称与路径 ────────────────────────────────────────────────

/// 方案名只做文件名成分：非空、≤64 字符、非 `.`/`..`、无路径分隔符与控制字符。
fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("profile name must not be empty".into());
    }
    if name == "." || name == ".." {
        return Err(format!("invalid profile name: {name:?}"));
    }
    if name.chars().count() > 64 {
        return Err("profile name too long (max 64 chars)".into());
    }
    if name
        .chars()
        .any(|c| matches!(c, '/' | '\\') || c.is_control())
    {
        return Err(format!("invalid profile name: {name:?}"));
    }
    Ok(())
}

fn profiles_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| h.join(".aidog").join("profiles"))
        .ok_or_else(|| "cannot resolve home directory".to_string())
}

fn profile_path(name: &str) -> Result<PathBuf, String> {
    validate_name(name)?;
    Ok(profiles_dir()?.join(format!("{name}.json")))
}

/// 动态 key 的文件名成分安全校验（同 [`validate_name`] 口径，防路径穿越）。
fn valid_filename(fname: &str) -> bool {
    !fname.is_empty()
        && fname != "."
        && fname != ".."
        && !fname.chars().any(|c| matches!(c, '/' | '\\') || c.is_control())
}

/// 快照 key → 本机绝对路径。动态前缀按 05 卸载盘点同一指纹口径收窄
/// （codex profile 限 `*.config.toml`、aidog settings 限 `settings.*.json`、
/// 脚本限 aidog_hooks 已知文件名），key 来自磁盘上的快照文件，不可信。
fn path_for_key(key: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let mapped = match key {
        "claude_dot_json" => Some(home.join(".claude.json")),
        "claude_settings_json" => Some(home.join(".claude").join("settings.json")),
        "claude_config_json" => Some(home.join(".claude").join("config.json")),
        "codex_config_toml" => codex_home_dir().map(|d| d.join("config.toml")),
        "pi_models_json" => pi_agent_dir().map(|d| d.join("models.json")),
        "pi_settings_json" => pi_agent_dir().map(|d| d.join("settings.json")),
        _ => None,
    };
    if let Some(p) = mapped {
        return Ok(p);
    }
    let (prefix, fname) = key
        .split_once(':')
        .ok_or_else(|| format!("invalid profile file key: {key:?}"))?;
    if !valid_filename(fname) {
        return Err(format!("invalid profile file key: {key:?}"));
    }
    match prefix {
        "codex_profile" => {
            if fname == "config.toml" || !fname.ends_with(".config.toml") {
                return Err(format!("invalid codex profile key: {key:?}"));
            }
            codex_home_dir()
                .map(|d| d.join(fname))
                .ok_or_else(|| "cannot resolve codex home".to_string())
        }
        "aidog_settings" => {
            if !fname.starts_with("settings.") || !fname.ends_with(".json") {
                return Err(format!("invalid aidog settings key: {key:?}"));
            }
            Ok(home.join(".aidog").join(fname))
        }
        "aidog_script" => {
            if !script_names().iter().any(|s| s == fname) {
                return Err(format!("unknown aidog script key: {key:?}"));
            }
            Ok(home.join(".aidog").join("scripts").join(fname))
        }
        _ => Err(format!("unknown profile file key: {key:?}")),
    }
}

/// hook 通知脚本名（与卸载盘点同一清单，aidog_hooks 常量）。
fn script_names() -> Vec<String> {
    vec![
        aidog_hooks::SCRIPT_COMPLETE.to_string(),
        aidog_hooks::SCRIPT_EVENT_NOTIFY.to_string(),
        aidog_hooks::LEGACY_SCRIPT_COMPLETE.to_string(),
        aidog_hooks::LEGACY_SCRIPT_WAITING.to_string(),
    ]
}

#[cfg(unix)]
fn file_mode(p: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).ok().map(|m| m.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn file_mode(_p: &Path) -> Option<u32> {
    None
}

fn file_entry(key: &str, path: &Path) -> ProfileFile {
    ProfileFile {
        key: key.to_string(),
        content: std::fs::read_to_string(path).ok(),
        mode: file_mode(path),
    }
}

/// 扫描磁盘上当前的动态面文件（指纹过滤同 05 卸载盘点）。
fn scan_dynamic_files() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return out,
    };
    // codex 分组 profile（TOML 指纹）
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
                out.push((format!("codex_profile:{name}"), e.path()));
            }
        }
    }
    // aidog 分组 settings 文件
    let data_dir = home.join(".aidog");
    if let Ok(entries) = std::fs::read_dir(&data_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("settings.") && name.ends_with(".json") {
                out.push((format!("aidog_settings:{name}"), e.path()));
            }
        }
    }
    // hook 通知脚本
    let scripts_dir = data_dir.join("scripts");
    for fname in script_names() {
        let p = scripts_dir.join(&fname);
        if p.exists() {
            out.push((format!("aidog_script:{fname}"), p));
        }
    }
    out
}

/// 采集当前全部配置面（静态 6 面 + 磁盘动态面）。
fn collect_files() -> Vec<ProfileFile> {
    let mut files = Vec::new();
    for key in [
        "claude_dot_json",
        "claude_settings_json",
        "claude_config_json",
        "codex_config_toml",
        "pi_models_json",
        "pi_settings_json",
    ] {
        match path_for_key(key) {
            Ok(p) => files.push(file_entry(key, &p)),
            Err(_) => files.push(ProfileFile {
                key: key.to_string(),
                content: None,
                mode: None,
            }),
        }
    }
    for (key, path) in scan_dynamic_files() {
        files.push(file_entry(&key, &path));
    }
    files
}

/// 当前 DB 的 group 行 + group_platform 对（复用导出收集器的形状）。
async fn collect_groups(db: &Db) -> Result<(Vec<serde_json::Value>, Vec<[String; 2]>), String> {
    let groups = aidog_db::list_groups(db).await?;
    let rows = groups
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("serialize group: {e}"))?;
    let pairs = aidog_db::list_all_group_platform_pairs(db)
        .await?
        .into_iter()
        .map(|(g, p)| [g, p])
        .collect();
    Ok((rows, pairs))
}

// ─── 原子写 ────────────────────────────────────────────────────

/// 写 `<file>.aidogtmp` → 回设权限位 → rename（同目录原子替换，禁半写）；
/// 任何一步失败都清掉临时文件。
pub(crate) fn atomic_write(path: &Path, content: &str, mode: Option<u32>) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("no parent dir for {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("create dir {}: {e}", parent.display()))?;
    let fname = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| format!("no file name for {}", path.display()))?;
    let tmp = parent.join(format!("{fname}.aidogtmp"));
    let write = || -> Result<(), String> {
        std::fs::write(&tmp, content).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(m & 0o777))
                .map_err(|e| format!("chmod {}: {e}", tmp.display()))?;
        }
        std::fs::rename(&tmp, path)
            .map_err(|e| format!("rename to {}: {e}", path.display()))
    };
    match write() {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

// ─── save / load ───────────────────────────────────────────────

pub(crate) async fn save_profile(
    db: &Db,
    name: &str,
    overwrite: bool,
) -> Result<ProfileMeta, String> {
    let path = profile_path(name)?;
    if path.exists() && !overwrite {
        return Err(format!(
            "profile already exists: {name} (pass overwrite=true to replace)"
        ));
    }
    let files = collect_files();
    let (groups, group_platform) = collect_groups(db).await?;
    let payload = ProfilePayload {
        schema_version: SCHEMA_VERSION,
        name: name.to_string(),
        created_at: chrono::Utc::now().timestamp().max(0) as u64,
        aidog_version: env!("CARGO_PKG_VERSION").to_string(),
        files,
        groups,
        group_platform,
    };
    let json = serde_json::to_string_pretty(&payload).map_err(|e| format!("serialize: {e}"))?;
    atomic_write(&path, &json, None)?;
    Ok(ProfileMeta {
        name: name.to_string(),
        created_at: payload.created_at,
    })
}

fn load_profile(name: &str) -> Result<ProfilePayload, String> {
    let path = profile_path(name)?;
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("read profile {name}: {e}"))?;
    let payload: ProfilePayload =
        serde_json::from_str(&raw).map_err(|e| format!("parse profile {name}: {e}"))?;
    if payload.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported profile schema_version {} (expected {SCHEMA_VERSION}): {name}",
            payload.schema_version
        ));
    }
    Ok(payload)
}

// ─── diff / apply ──────────────────────────────────────────────

/// review #3：静态面「快照时不存在、现在存在」的文件删前指纹门——无 aidog
/// 指纹 → op=skip 保留用户文件，误删用户现文件的路径关闭。动态面
/// （codex_profile:/aidog_settings:/aidog_script:）采集时就按指纹 / 自有目录
/// 收窄，恒视为 aidog 产物。
fn has_aidog_fingerprint(key: &str, content: &str) -> bool {
    match key {
        "codex_config_toml" => toml::from_str::<toml::Value>(content)
            .map(|t| is_aidog_codex_profile_toml(&t))
            .unwrap_or(false),
        "pi_models_json" | "pi_settings_json" => {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(content) else {
                return false;
            };
            let prefix = crate::gateway::pi::PROVIDER_PREFIX;
            v.get("providers")
                .and_then(|p| p.as_object())
                .is_some_and(|m| m.keys().any(|k| k.starts_with(prefix)))
                || v.get("defaultProvider")
                    .and_then(|x| x.as_str())
                    .is_some_and(|x| x.starts_with(prefix))
        }
        "claude_dot_json" | "claude_settings_json" | "claude_config_json" => {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(content) else {
                return false;
            };
            is_aidog_base_url(
                v.get("ANTHROPIC_BASE_URL").unwrap_or(&serde_json::Value::Null),
            ) || v.get("mcpServers")
                .and_then(|m| m.get("aidog"))
                .is_some()
                || v.get("primaryApiKey").and_then(|x| x.as_str()) == Some("any")
                || content.contains(".aidog")
        }
        // 动态面 key（带 `:` 前缀）：指纹已验
        _ => true,
    }
}

enum FileOp {
    Write {
        path: PathBuf,
        content: String,
        mode: Option<u32>,
    },
    Delete {
        path: PathBuf,
    },
    /// review #3：无 aidog 指纹的静态面现存文件，跳过删除（保留用户文件）。
    Skip,
    /// db:groups 面占位（执行时调 [`restore_groups`]）。
    DbGroups,
}

fn group_key_of(row: &serde_json::Value) -> Option<&str> {
    row.get("group_key")
        .and_then(|v| v.as_str())
        .or_else(|| row.get("name").and_then(|v| v.as_str()))
}

/// 快照 ↔ 现状的文件级差异（含磁盘多余动态文件），返回执行序列 + 清单项。
fn file_ops_and_diff(payload: &ProfilePayload) -> Result<Vec<(FileOp, ProfileDiffItem)>, String> {
    let mut out = Vec::new();
    let snapshot_keys: BTreeSet<&str> = payload.files.iter().map(|f| f.key.as_str()).collect();
    for f in &payload.files {
        let path = path_for_key(&f.key)?;
        let current = std::fs::read_to_string(&path).ok();
        match (&f.content, current) {
            (Some(snap), cur) if cur.as_deref() != Some(snap.as_str()) => {
                let op = if cur.is_some() { "update" } else { "create" };
                out.push((
                    FileOp::Write {
                        path: path.clone(),
                        content: snap.clone(),
                        mode: f.mode,
                    },
                    ProfileDiffItem {
                        target: f.key.clone(),
                        op,
                        detail: format!("还原 {op}：{}", path.display()),
                        error: None,
                    },
                ));
            }
            (None, Some(cur)) => {
                if has_aidog_fingerprint(&f.key, &cur) {
                    out.push((
                        FileOp::Delete { path },
                        ProfileDiffItem {
                            target: f.key.clone(),
                            op: "delete",
                            detail: "快照时不存在，删除现文件".to_string(),
                            error: None,
                        },
                    ));
                } else {
                    out.push((
                        FileOp::Skip,
                        ProfileDiffItem {
                            target: f.key.clone(),
                            op: "skip",
                            detail: "快照时不存在且无 aidog 指纹，保留用户文件".to_string(),
                            error: None,
                        },
                    ));
                }
            }
            _ => {}
        }
    }
    // 磁盘上有、快照没有的动态面文件 → 删除（还原「未装」状态）
    for (key, path) in scan_dynamic_files() {
        if snapshot_keys.contains(key.as_str()) {
            continue;
        }
        out.push((
            FileOp::Delete { path },
            ProfileDiffItem {
                target: key,
                op: "delete",
                detail: "快照外的多余文件，删除".to_string(),
                error: None,
            },
        ));
    }
    Ok(out)
}

/// 分组面差异：快照外的现存组会删除、快照组及其映射会对齐。
async fn groups_diff(db: &Db, payload: &ProfilePayload) -> Result<Option<ProfileDiffItem>, String> {
    let snapshot_keys: BTreeSet<&str> = payload.groups.iter().filter_map(group_key_of).collect();
    let current = aidog_db::list_groups(db).await?;
    let current_keys: BTreeSet<&str> = current.iter().map(|g| g.group_key.as_str()).collect();
    let current_pairs = aidog_db::list_all_group_platform_pairs(db).await?;
    let snapshot_pairs: BTreeSet<[String; 2]> = payload.group_platform.iter().cloned().collect();
    let current_pair_set: BTreeSet<[String; 2]> =
        current_pairs.iter().map(|(g, p)| [g.clone(), p.clone()]).collect();
    let extra_groups: Vec<&str> = current_keys
        .difference(&snapshot_keys)
        .copied()
        .collect();
    let pairs_differ = current_pair_set != snapshot_pairs;
    if extra_groups.is_empty() && !pairs_differ {
        return Ok(None);
    }
    let mut detail = String::from("分组/映射将与快照对齐");
    if !extra_groups.is_empty() {
        detail.push_str(&format!("；将删除快照外分组 {} 个", extra_groups.len()));
    }
    if pairs_differ {
        detail.push_str("；映射关系有差异");
    }
    Ok(Some(ProfileDiffItem {
        target: "db:groups".to_string(),
        op: "update",
        detail,
        error: None,
    }))
}

/// 恢复分组面：删快照外组 → 清快照组现有关联 → upsert 行 → 重连映射。
async fn restore_groups(db: &Db, payload: &ProfilePayload) -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();
    let snapshot_keys: BTreeSet<&str> = payload.groups.iter().filter_map(group_key_of).collect();
    let current = aidog_db::list_groups(db).await?;

    for g in &current {
        if !snapshot_keys.contains(g.group_key.as_str()) {
            // 先物理清映射行再软删组：delete_group 保留关联行，
            // list_all_group_platform_pairs 不按 deleted_at 过滤组，残留会照常解析出名字。
            let r = async {
                aidog_db::set_group_platforms(db, g.id, &[]).await?;
                aidog_db::delete_group(db, g.id).await
            }
            .await;
            if let Err(e) = r {
                errors.push(format!("delete group {}: {e}", g.group_key));
            }
        }
    }
    // current 里仍有效的快照组（上面只删了快照外的）
    for row in &payload.groups {
        let Some(name) = row.get("name").and_then(|v| v.as_str()) else {
            errors.push("snapshot group row missing name".to_string());
            continue;
        };
        let Some(group_key) = group_key_of(row) else {
            errors.push("snapshot group row missing group_key".to_string());
            continue;
        };
        let r = async {
            if let Some(g) = current
                .iter()
                .find(|g| snapshot_keys.contains(g.group_key.as_str()) && g.group_key == group_key)
            {
                aidog_db::set_group_platforms(db, g.id, &[]).await?;
            }
            crate::gateway::import_export::apply::upsert_group_row(db, group_key, name, row)
                .await
        }
        .await;
        if let Err(e) = r {
            errors.push(format!("restore group {group_key}: {e}"));
        }
    }
    for [gk, pname] in &payload.group_platform {
        // relink 按 group.name 匹配；pair 里存的是 group_key，先解析回 name
        let gname = payload
            .groups
            .iter()
            .find(|r| group_key_of(r) == Some(gk.as_str()))
            .and_then(|r| r.get("name").and_then(|v| v.as_str()))
            .unwrap_or(gk);
        if let Err(e) =
            crate::gateway::import_export::apply::relink_group_platform(db, gname, pname)
                .await
        {
            errors.push(format!("link {gk}↔{pname}: {e}"));
        }
    }
    db.invalidate_hot_caches();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub(crate) async fn apply_profile(
    db: &Db,
    name: &str,
    dry_run: bool,
) -> Result<Vec<ProfileDiffItem>, String> {
    let payload = load_profile(name)?;
    let mut ops = file_ops_and_diff(&payload)?;
    if let Some(gitem) = groups_diff(db, &payload).await? {
        ops.push((FileOp::DbGroups, gitem));
    }
    if dry_run {
        return Ok(ops.into_iter().map(|(_, item)| item).collect());
    }

    let mut report = Vec::with_capacity(ops.len());
    for (op, mut item) in ops {
        match op {
            FileOp::Write { path, content, mode } => {
                if let Err(e) = atomic_write(&path, &content, mode) {
                    item.error = Some(e);
                }
            }
            FileOp::Delete { path } => {
                if let Err(e) = std::fs::remove_file(&path)
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    item.error = Some(format!("remove {}: {e}", path.display()));
                }
            }
            FileOp::Skip => {}
            FileOp::DbGroups => {
                if let Err(e) = restore_groups(db, &payload).await {
                    item.error = Some(e);
                }
            }
        }
        report.push(item);
    }
    Ok(report)
}

// ─── list / delete ─────────────────────────────────────────────

pub(crate) fn list_profiles() -> Result<Vec<ProfileMeta>, String> {
    let dir = profiles_dir()?;
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(&p) else {
                continue;
            };
            if let Ok(meta) = serde_json::from_str::<ProfileMeta>(&raw) {
                out.push(meta);
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.name.cmp(&b.name)));
    Ok(out)
}

pub(crate) fn delete_profile(name: &str) -> Result<(), String> {
    let path = profile_path(name)?;
    if !path.exists() {
        return Err(format!("profile not found: {name}"));
    }
    std::fs::remove_file(&path).map_err(|e| format!("delete profile {name}: {e}"))
}

// ─── RPC / Tauri 命令 ──────────────────────────────────────────

crate::tauri_command! {
    /// 保存当前 agent 配置 + 分组映射为快照方案（magpie 票 06）。
    /// 同名方案已存在时须 `overwrite=true` 显式覆盖。
    pub async fn profile_save(name: String, overwrite: bool) -> Result<ProfileMeta, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "profile_save", %name, overwrite, "command invoked");
        save_profile(db, &name, overwrite).await
    }
}

crate::tauri_command! {
    /// 一键切回快照方案（magpie 票 06）。`dry_run=true` 只返回差异清单不落盘；
    /// `false` 执行（best-effort，单面失败记 error 继续）。高危面（用户家目录
    /// 文件）：前端执行前须先 dry_run 弹差异并二次确认。
    pub async fn profile_apply(name: String, dry_run: bool) -> Result<Vec<ProfileDiffItem>, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "profile_apply", %name, dry_run, "command invoked");
        apply_profile(db, &name, dry_run).await
    }
}

crate::tauri_command! {
    /// 列出已保存的配置方案（magpie 票 06）。
    pub async fn profile_list() -> Result<Vec<ProfileMeta>, String> {
        tracing::debug!(command = "profile_list", "command invoked");
        list_profiles()
    }
}

crate::tauri_command! {
    /// 删除配置方案（magpie 票 06）。只删快照文件，不碰当前配置。
    pub async fn profile_delete(name: String) -> Result<(), String> {
        tracing::debug!(command = "profile_delete", %name, "command invoked");
        delete_profile(&name)
    }
}
