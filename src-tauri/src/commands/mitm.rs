//! P3 MITM 前端配置命令（ST7）。
//!
//! 桥接前端 UI ↔ ST1/ST2 基础设施（ca.rs / whitelist.rs）。
//! - 状态查询 / 启用 / CA 安装准备 / 白名单增删改
//!
//! 设计约束：
//! - 禁改 mitm Rust 模块公共签名（ST3 在跑）→ 白名单写 SQL（INSERT/DELETE/UPDATE enabled）
//!   本 subtask 内联在 command 层，复用 ST2 已建的 mitm_whitelist 表 + list_whitelist 读 API。
//! - CA 装/卸信任库走 tauri-plugin-shell execute（capability mitm-ca.json 限定的命名命令）。
//!   本命令只把 cert_pem 落到 `~/.aidog/mitm-ca.pem` + 返命令 spec（capability name + args），
//!   前端用 `@tauri-apps/plugin-shell` `Command.create(name, args).execute()` 触发 sudo 弹窗（D8）。
//!   执行结果（exit code）由前端回传 `mitm_set_ca_installed(bool)` 落账。

use crate::gateway::{
    self,
    db::Db,
    mitm::{
        ca::{
            classify_trust_error, ensure_root_ca, load_root_ca, set_ca_installed, set_enabled,
            sync_ca_installed_from_system, trust_ca_command, untrust_ca_command,
        },
        whitelist::{list_whitelist, WhitelistEntry},
    },
};
use crate::shared::aidog_data_dir;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

/// CA PEM 在数据目录的文件名（capability mitm-ca.json validator 正则要求 `^.+\.pem$`）。
const CA_PEM_FILENAME: &str = "mitm-ca.pem";

/// 前端展示用的白名单行（DTO：whitelist.rs 的 WhitelistEntry 未派生 Serialize，
/// 此处镜像字段做序列化层，避改 ST2 公共签名）。
#[derive(Debug, Clone, Serialize)]
pub struct WhitelistEntryDto {
    pub host_pattern: String,
    pub rule_type: String,
    pub enabled: bool,
    pub source: String,
}

impl From<WhitelistEntry> for WhitelistEntryDto {
    fn from(e: WhitelistEntry) -> Self {
        Self {
            host_pattern: e.host_pattern,
            rule_type: e.rule_type,
            enabled: e.enabled,
            source: e.source,
        }
    }
}

/// 前端展示用的 MITM 综合状态。
#[derive(Debug, Clone, Serialize)]
pub struct MitmStatus {
    pub enabled: bool,
    pub ca_present: bool,
    pub ca_installed: bool,
    pub ca_fingerprint: String,
    pub whitelist: Vec<WhitelistEntryDto>,
}

/// CA 安装命令 spec（前端 `Command.create(name, args).execute()`）。
#[derive(Debug, Clone, Serialize)]
pub struct CaCommandSpec {
    /// capability `mitm-ca.json` 里的命名命令 key（按 OS 选 macos-trust-ca / windows-trust-ca / linux-shell-ca）。
    pub name: String,
    /// 命令参数（已含 ca_pem_path + 提权 wrapper）。
    pub args: Vec<String>,
    /// 落盘后的 CA PEM 绝对路径（前端展示用）。
    pub ca_pem_path: String,
    /// 兜底手动装展示的真实 sudo 终端命令（提权失败时前端弹窗给用户复制执行）。
    pub manual_display: String,
}

/// CA 卸载命令 spec（ST9 实装命令 reverse；当前仅 fingerprint，返 spec 供前端 ST9 接入）。
#[derive(Debug, Clone, Serialize)]
pub struct CaUninstallSpec {
    pub name: String,
    pub args: Vec<String>,
    /// 兜底手动卸展示的真实 sudo 终端命令。
    pub manual_display: String,
}

// ─── 状态查询 ───────────────────────────────────────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_status(db: State<'_, Db>) -> Result<MitmStatus, String> {
    tracing::debug!(command = "mitm_status", "command invoked");
    let ca = load_root_ca(&db).await?;
    let whitelist = list_whitelist(&db).await?.into_iter().map(Into::into).collect();
    // 修问题 2：ca_present 时调 sync_ca_installed_from_system 双向校验 keychain 实状
    // （手动装/卸后 DB 静态字段不更新），返实状供页面显示。ca_present=false 仍 false。
    let ca_installed = match &ca {
        Some(c) => sync_ca_installed_from_system(&db, c).await,
        None => false,
    };
    Ok(MitmStatus {
        enabled: ca.as_ref().map(|c| c.enabled).unwrap_or(false),
        ca_present: ca.is_some(),
        ca_installed,
        ca_fingerprint: ca.as_ref().map(|c| c.fingerprint.clone()).unwrap_or_default(),
        whitelist,
    })
}

// ─── 启用 / 禁用 ─────────────────────────────────────────────

/// 启用 MITM（D7：首次启用时 ensure_root_ca 生成假 CA）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_enable(db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "mitm_enable", "command invoked");
    // ensure 先建 CA（若 DB 无），再设 enabled=true。两步都需成功。
    let _ca = ensure_root_ca(&db).await?;
    set_enabled(&db, true).await?;
    Ok(())
}

/// 禁用 MITM（CA 保留，仅置 enabled=false；后续 ST9 提供「移除 CA + 卸信任库」清理）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_disable(db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "mitm_disable", "command invoked");
    set_enabled(&db, false).await?;
    Ok(())
}

// ─── CA 安装 ─────────────────────────────────────────────────

/// 准备装信任库：写 CA PEM 到数据目录 + 返命名命令 spec。
///
/// 前端拿 spec 调 `Command.create(spec.name, spec.args).execute()`：
///   - exit code 0 → 调 `mitm_set_ca_installed(true)`
///   - 非 0 / reject → 调 `mitm_set_ca_installed(false)` + UI 弹窗给 spec + ca_pem_path 引导手动装（D8 兜底）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_install_ca_prepare(db: State<'_, Db>) -> Result<CaCommandSpec, String> {
    tracing::debug!(command = "mitm_install_ca_prepare", "command invoked");
    let ca = ensure_root_ca(&db).await?;
    let dir = aidog_data_dir()?;
    let ca_pem_path = dir.join(CA_PEM_FILENAME);
    std::fs::write(&ca_pem_path, &ca.cert_pem)
        .map_err(|e| format!("write ca.pem: {e}"))?;
    // capability 按 OS 限定 3 个命名命令；args 必须匹配 validator 正则（pem 路径 / hex）。
    let (name, args, manual_display) = trust_command_spec(&ca_pem_path);
    Ok(CaCommandSpec {
        name,
        args,
        ca_pem_path: ca_pem_path.to_string_lossy().into_owned(),
        manual_display,
    })
}

/// 准备卸载信任库（ST9 实装 reverse 命令；当前提供 spec 供 UI 展示）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_uninstall_ca_prepare(db: State<'_, Db>) -> Result<CaUninstallSpec, String> {
    tracing::debug!(command = "mitm_uninstall_ca_prepare", "command invoked");
    let ca = load_root_ca(&db)
        .await?
        .ok_or_else(|| "CA not generated".to_string())?;
    // untrust_ca_command 内部从 cert_pem 现算 SHA-1 thumbprint（macOS -Z / Windows -delstore Root）。
    // ponytail: 不读 ST1 存的 SHA-256 fingerprint（capability validator 拒冒号 + OS 语义要 SHA-1）。
    let (name, args, manual_display) = untrust_command_spec(&ca.cert_pem);
    Ok(CaUninstallSpec { name, args, manual_display })
}

/// 前端 shell execute 完成后回写 CA 安装状态（成功 true / 失败 false）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_set_ca_installed(db: State<'_, Db>, installed: bool) -> Result<(), String> {
    tracing::debug!(command = "mitm_set_ca_installed", installed, "command invoked");
    set_ca_installed(&db, installed).await
}

/// 分类 CA 安装失败原因（阶段 B：后端化真源，消除前后端双源分类逻辑）。
///
/// 前端 `Command.create(name, args).execute()` exit 非 0 后，调本命令把 (name, code, stderr)
/// 送后端分类为 `TrustErrorKind`（Cancel/AuthFail/NoAgent/CmdFail），前端按 kind 选 t() 文案。
///
/// `code: Option<i32>` 显式建模 Tauri shell plugin `out.code` 可能 null/undefined
/// （reject / signal kill 路径 code 可能为 null）。
///
/// snake_case args：前端 invoke 传 `{ name, code, stderr }`（serde 自动转 snake_case）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_classify_trust_error(
    name: String,
    code: Option<i32>,
    stderr: String,
) -> Result<String, String> {
    tracing::debug!(command = "mitm_classify_trust_error", %name, code, "command invoked");
    let kind = classify_trust_error(&name, code, &stderr);
    Ok(kind.as_str().to_string())
}

// ─── 白名单增删改（内联 SQL，避改 whitelist.rs 公共签名）──────────

#[derive(Debug, Clone, Deserialize)]
pub struct WhitelistAddInput {
    pub host_pattern: String,
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_whitelist_add(
    input: WhitelistAddInput,
    db: State<'_, Db>,
) -> Result<(), String> {
    tracing::debug!(command = "mitm_whitelist_add", pattern = %input.host_pattern, "command invoked");
    let pattern = input.host_pattern.trim().to_lowercase();
    if pattern.is_empty() {
        return Err("host_pattern is empty".to_string());
    }
    let now = gateway::db::now();
    db.0
        .call(move |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO mitm_whitelist (host_pattern, rule_type, enabled, source, created_at) \
                 VALUES (?1, 'suffix', 1, 'user', ?2)",
                params![pattern, now],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_whitelist_remove(host_pattern: String, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "mitm_whitelist_remove", pattern = %host_pattern, "command invoked");
    let pattern = host_pattern.trim().to_lowercase();
    db.0
        .call(move |conn| {
            conn.execute(
                "DELETE FROM mitm_whitelist WHERE host_pattern = ?1",
                params![pattern],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn mitm_whitelist_toggle(
    host_pattern: String,
    enabled: bool,
    db: State<'_, Db>,
) -> Result<(), String> {
    tracing::debug!(command = "mitm_whitelist_toggle", pattern = %host_pattern, enabled, "command invoked");
    let pattern = host_pattern.trim().to_lowercase();
    db.0
        .call(move |conn| {
            conn.execute(
                "UPDATE mitm_whitelist SET enabled = ?1 WHERE host_pattern = ?2",
                params![enabled as i64, pattern],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
}

// ─── OS 命名命令 spec（capability mitm-ca.json 的 name key）──────────
//
// capability 的 name 是按 OS 配的（macos-trust-ca / windows-trust-ca / linux-shell-ca），
// 与 ca.rs::trust_ca_command 返的 (program, args) 中 program 不同（program 是路径 /usr/bin/security，
// name 是 capability 的命名 key）。这里把 name 与 args 一起返，前端直接喂 Command.create(name, args)。
// Linux trust/untrust 共用 `linux-shell-ca`（/bin/sh -c），capability validator union regex 锁两形式。
//
// ponytail: name 表硬编码 3 OS 分支，与 mitm-ca.json 同步源。capability 改 name 必须同步改这里；
// 若加 CI 校验可用，但 ST7 阶段仅 3 条命令，YAGNI。

fn trust_command_spec(ca_pem_path: &std::path::Path) -> (String, Vec<String>, String) {
    // 复用 ca.rs 的 args 构造（含提权 wrapper），仅 name 替换为 capability key；manual_display 同源。
    let (_program, args, manual_display) = trust_ca_command(&ca_pem_path.to_string_lossy());
    let name = current_os_trust_command_name();
    (name, args, manual_display)
}

fn untrust_command_spec(cert_pem: &str) -> (String, Vec<String>, String) {
    let (_program, args, manual_display) = untrust_ca_command(cert_pem);
    let name = current_os_untrust_command_name();
    (name, args, manual_display)
}

fn current_os_trust_command_name() -> String {
    #[cfg(target_os = "macos")]
    { "macos-trust-ca".to_string() }
    #[cfg(target_os = "windows")]
    { "windows-trust-ca".to_string() }
    #[cfg(all(unix, not(target_os = "macos")))]
    // Linux trust 走 /bin/sh -c "cp <pem> <dest> && update-ca-certificates"（ca.rs::trust_ca_command）。
    // capability `linux-shell-ca` 用 union regex 同时锁 trust 与 untrust 两种 -c 串。
    { "linux-shell-ca".to_string() }
}

fn current_os_untrust_command_name() -> String {
    #[cfg(target_os = "macos")]
    { "macos-remove-ca".to_string() }
    #[cfg(target_os = "windows")]
    { "windows-remove-ca".to_string() }
    #[cfg(all(unix, not(target_os = "macos")))]
    // Linux untrust 走 /bin/sh -c "rm -f <dest> && update-ca-certificates --fresh"（ca.rs::untrust_ca_command）。
    // 与 trust 共用 `linux-shell-ca` 命名命令（capability validator union regex 锁两形式）。
    { "linux-shell-ca".to_string() }
}
