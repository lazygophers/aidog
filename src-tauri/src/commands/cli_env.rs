//! CLI 工具环境检测：Claude Code / Codex CLI 版本 / 安装 / 升级 / 冲突诊断。
//!
//! 抄 `install_uv` 后端 spawn 模式（不动 capability / tauri-plugin-shell 配置）。
//! 复用启动期 `gateway::skills::ensure_runtime_path` 已并入登录 shell PATH 的成果，
//! 子进程直接 spawn 即可命中用户 brew/nvm/native installer 装的 CLI。
//!
//! 三态检测（参考 cc-switch `ShellProbe`）：
//! - `installed=true, broken=false`：spawn exit 0 + 解析出版本号
//! - `installed=true, broken=true`：spawn exit ≠ 0 但 binary 存在（平台二进制损坏等）
//! - `installed=false`：spawn 失败 / command not found
//!
//! 冲突判定（参考 cc-switch `is_conflicting` 严阈值）：
//! 多处安装 + (版本分歧 | 运行态混合) 才标红；同版本同能跑两份不打扰。

use std::process::Command;
use tokio::process::Command as TokioCommand;

/// MVP 工具范围：仅 claude + codex（research 建议降维护成本）。
pub const TOOLS: &[&str] = &["claude", "codex"];

/// npm registry 包名映射（tool -> npm package）。
const NPM_PACKAGES: &[(&str, &str)] = &[
    ("claude", "@anthropic-ai/claude-code"),
    ("codex", "@openai/codex"),
];

/// 更新检测缓存（1h TTL）。
static UPDATE_CACHE_INIT: std::sync::OnceLock<
    tokio::sync::RwLock<std::collections::HashMap<String, (std::time::Instant, String)>>,
> = std::sync::OnceLock::new();

/// 缓存 TTL：1 小时。
const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

/// 获取更新缓存（延迟初始化）。
fn update_cache() -> &'static tokio::sync::RwLock<
    std::collections::HashMap<String, (std::time::Instant, String)>
> {
    UPDATE_CACHE_INIT.get_or_init(|| {
        tokio::sync::RwLock::new(std::collections::HashMap::new())
    })
}

#[derive(serde::Serialize, Clone)]
pub struct CliInstallation {
    /// 候选入口路径（未 canonicalize）。
    pub path: String,
    /// `--version` 成功时的版本号。
    pub version: Option<String>,
    /// `--version` 是否 exit 0。
    pub runnable: bool,
    /// 路径前缀推断的安装来源（nvm/homebrew/volta/fnm/mise/native/npm-global/system）。
    pub source: String,
    /// 是否为 PATH 默认命中的那处（`which` / `where` 第一行）。
    pub is_path_default: bool,
}

#[derive(serde::Serialize, Clone)]
pub struct CliToolStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    /// 装了但 `--version` 跑不起来（平台二进制损坏等）。
    pub broken: bool,
    /// 多处安装且版本分歧或运行态混合（严阈值）。
    pub conflict: bool,
    /// npm registry 最新版本（检测失败/离线时为 None）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    /// 是否有更新可用（None=检测失败/离线，Some(true)=有更新，Some(false)=已是最新）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_update: Option<bool>,
}

#[derive(serde::Serialize, Clone)]
pub struct CliConflict {
    pub tool: String,
    pub installations: Vec<CliInstallation>,
    pub is_conflicting: bool,
    /// 仅报告 + 建议，不自动卸载（破坏性操作禁主动执行）。
    pub suggestion: String,
}

/// Windows CREATE_NO_WINDOW flag（`0x08000000`）：抑制子进程闪黑窗。
/// POSIX 平台 no-op。
#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

/// tokio::process::Command 版本的 no_window。
#[cfg(windows)]
async fn no_window_tokio(cmd: &mut TokioCommand) {
    use tokio::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
async fn no_window_tokio(_cmd: &mut TokioCommand) {}

/// 从 `--version` 输出提取版本号（正则 `\d+\.\d+\.\d+(-[\w.]+)?` 等价实现）。
/// 兼容 codex 时间戳式 patch（如 `0.1.2505172116`）。
fn extract_version(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            while i < bytes.len() {
                let c = bytes[i];
                if c.is_ascii_digit() || c == b'.' {
                    if c == b'.' {
                        dots += 1;
                    }
                    i += 1;
                } else if c == b'-' || c == b'+' {
                    // prerelease / build metadata：吃后续 alnum / . / - / +
                    i += 1;
                    while i < bytes.len() {
                        let cc = bytes[i];
                        if cc.is_ascii_alphanumeric() || cc == b'.' || cc == b'-' || cc == b'+' {
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    break;
                } else {
                    break;
                }
            }
            if dots >= 2 {
                let s = String::from_utf8_lossy(&bytes[start..i]).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// spawn `tool --version`，返回 `(installed, version, path)`。
fn probe_version(tool: &str) -> (bool, Option<String>, Option<String>) {
    let mut cmd = Command::new(tool);
    cmd.arg("--version");
    no_window(&mut cmd);
    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return (false, None, None),
    };
    let path = which_first(tool);
    if !output.status.success() {
        // 装了但 `--version` 跑不起来（exit ≠ 0）：标 broken。
        return (true, None, path);
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let version = extract_version(&combined);
    (true, version, path)
}

/// `which tool` / `where tool` 拿 PATH 默认命中的第一处。
fn which_first(tool: &str) -> Option<String> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("where");
        c.arg(tool);
        c
    } else {
        let mut c = Command::new("which");
        c.arg(tool);
        c
    };
    no_window(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `which -a tool` / `where tool` 枚举所有 PATH 命中的二进制（不去重）。
fn which_all(tool: &str) -> Vec<String> {
    let mut cmd = if cfg!(windows) {
        // Windows `where tool` 默认返回所有匹配
        let mut c = Command::new("where");
        c.arg(tool);
        c
    } else {
        let mut c = Command::new("which");
        c.arg("-a").arg(tool);
        c
    };
    no_window(&mut cmd);
    let Some(out) = cmd.output().ok() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// canonicalize 路径（解 symlink）去重用。
fn canonicalize_path(p: &str) -> String {
    std::fs::canonicalize(p)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| p.to_string())
}

/// 由路径前缀推断 source（参考 cc-switch `infer_install_source`，顺序敏感）。
/// Homebrew Cellar 真身须先于通用规则命中。
fn infer_source(path: &str) -> String {
    let lower = path.to_lowercase().replace('\\', "/");
    if lower.contains("/.nvm/") {
        "nvm".into()
    } else if lower.contains("/homebrew/") || lower.contains("/cellar/") {
        "homebrew".into()
    } else if lower.contains("/.volta/") || lower.contains("/volta/") {
        "volta".into()
    } else if lower.contains("fnm_multishells") {
        "fnm".into()
    } else if lower.contains("/mise/") {
        "mise".into()
    } else if lower.contains("/.bun/") {
        "bun".into()
    } else if lower.contains("/pnpm/") {
        "pnpm".into()
    } else if lower.contains("/scoop/") {
        "scoop".into()
    } else if lower.contains("/library/python")
        || lower.contains("/scripts/")
        || lower.contains("/site-packages/")
    {
        "pip".into()
    } else if lower.contains("/.local/share/claude/") {
        "native".into()
    } else if lower.contains("/.local/bin/")
        || lower.contains("/.npm-global/bin/")
        || lower.contains("/n/bin/")
    {
        "npm-global".into()
    } else {
        "system".into()
    }
}

/// 枚举某工具所有安装：`which -a` + canonicalize 去重 + source 推断 + 逐个 `--version`。
/// `is_path_default=true` 排最前（UI 一眼看到命令行默认用的是哪处）。
fn enumerate_installations(tool: &str) -> Vec<CliInstallation> {
    let raw_paths = which_all(tool);
    let default_path = which_first(tool);
    let default_real = default_path.as_ref().map(|d| canonicalize_path(d));
    let mut seen = std::collections::HashSet::new();
    let mut installs = Vec::new();
    for p in raw_paths {
        let real = canonicalize_path(&p);
        if !seen.insert(real.clone()) {
            continue;
        }
        let is_default = default_real.as_ref().map(|d| d == &real).unwrap_or(false);
        let mut cmd = Command::new(&p);
        cmd.arg("--version");
        no_window(&mut cmd);
        let (version, runnable) = match cmd.output() {
            Ok(o) if o.status.success() => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                (extract_version(&text), true)
            }
            Ok(_) => (None, false),
            Err(_) => (None, false),
        };
        installs.push(CliInstallation {
            path: p.clone(),
            version,
            runnable,
            source: infer_source(&p),
            is_path_default: is_default,
        });
    }
    installs.sort_by_key(|i| !i.is_path_default);
    installs
}

/// 严阈值冲突判定：多处安装 + (版本分歧 | 运行态混合)。
/// 同版本装两份且都能跑**不算冲突**（不打扰用户）。
fn is_conflicting(installs: &[CliInstallation]) -> bool {
    if installs.len() < 2 {
        return false;
    }
    let distinct_versions: std::collections::HashSet<&Option<String>> =
        installs.iter().map(|i| &i.version).collect();
    let runnable_mixed =
        installs.iter().any(|i| i.runnable) && installs.iter().any(|i| !i.runnable);
    distinct_versions.len() > 1 || runnable_mixed
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub fn cli_check_versions() -> Vec<CliToolStatus> {
    tracing::debug!(command = "cli_check_versions", "command invoked");
    TOOLS
        .iter()
        .map(|&tool| {
            let (installed, version, path) = probe_version(tool);
            let broken = installed && version.is_none();
            let conflict = if which_all(tool).len() >= 2 {
                let installs = enumerate_installations(tool);
                is_conflicting(&installs)
            } else {
                false
            };
            CliToolStatus {
                name: tool.to_string(),
                installed,
                version,
                path,
                broken,
                conflict,
                latest_version: None,
                has_update: None,
            }
        })
        .collect()
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_install(tool: String) -> Result<(), String> {
    tracing::debug!(command = "cli_install", tool = %tool, "command invoked");
    if !TOOLS.contains(&tool.as_str()) {
        return Err(format!("unsupported tool: {tool}"));
    }
    match tool.as_str() {
        "claude" => {
            #[cfg(unix)]
            {
                // POSIX 优先 native installer（claude.ai/install.sh），失败回退 npm。
                let script = "tmp=$(mktemp) && curl -fsSL https://claude.ai/install.sh -o $tmp && bash $tmp; status=$?; rm -f $tmp; exit $status";
                let mut cmd = TokioCommand::new("bash");
                cmd.arg("-c").arg(script);
                no_window_tokio(&mut cmd).await;
                match run_and_check_async(cmd, "claude install (native)").await {
                    Ok(()) => return Ok(()),
                    Err(native_err) => {
                        tracing::warn!(error = %native_err, "claude native install failed, falling back to npm");
                    }
                }
            }
            // Windows / native 失败回退：npm 全局装
            let mut cmd = TokioCommand::new("npm");
            cmd.args(["i", "-g", "@anthropic-ai/claude-code@latest"]);
            no_window_tokio(&mut cmd).await;
            run_and_check_async(cmd, "claude install (npm)").await
        }
        "codex" => {
            let mut cmd = TokioCommand::new("npm");
            cmd.args(["i", "-g", "@openai/codex@latest"]);
            no_window_tokio(&mut cmd).await;
            run_and_check_async(cmd, "codex install (npm)").await
        }
        _ => Err(format!("unsupported tool: {tool}")),
    }
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_upgrade(tool: String) -> Result<(), String> {
    tracing::debug!(command = "cli_upgrade", tool = %tool, "command invoked");
    if !TOOLS.contains(&tool.as_str()) {
        return Err(format!("unsupported tool: {tool}"));
    }
    match tool.as_str() {
        "claude" => {
            // 优先 `claude update`（native installer 自带子命令）。
            let mut cmd = TokioCommand::new("claude");
            cmd.arg("update");
            no_window_tokio(&mut cmd).await;
            if let Ok(o) = cmd.output().await {
                if o.status.success() {
                    return Ok(());
                }
                let err = String::from_utf8_lossy(&o.stderr);
                tracing::warn!(error = %err, "claude update failed, falling back to npm");
            }
            // npm 兜底
            let mut cmd = TokioCommand::new("npm");
            cmd.args(["i", "-g", "@anthropic-ai/claude-code@latest"]);
            no_window_tokio(&mut cmd).await;
            run_and_check_async(cmd, "claude upgrade (npm)").await
        }
        "codex" => {
            // POSIX 先试 `codex update`；失败 / Windows 走 uninstall + install 自愈。
            #[cfg(unix)]
            {
                let mut cmd = TokioCommand::new("codex");
                cmd.arg("update");
                no_window_tokio(&mut cmd).await;
                if let Ok(o) = cmd.output().await {
                    if o.status.success() {
                        return Ok(());
                    }
                    let err = String::from_utf8_lossy(&o.stderr);
                    tracing::warn!(error = %err, "codex update failed, falling back to npm reinstall");
                }
            }
            // 自愈：uninstall（容忍失败）+ install
            let mut u = TokioCommand::new("npm");
            u.args(["uninstall", "-g", "@openai/codex"]);
            no_window_tokio(&mut u).await;
            let _ = u.output().await;
            let mut i = TokioCommand::new("npm");
            i.args(["i", "-g", "@openai/codex@latest"]);
            no_window_tokio(&mut i).await;
            run_and_check_async(i, "codex upgrade (npm reinstall)").await
        }
        _ => Err(format!("unsupported tool: {tool}")),
    }
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_diagnose_conflicts() -> Vec<CliConflict> {
    tracing::debug!(command = "cli_diagnose_conflicts", "command invoked");
    TOOLS
        .iter()
        .filter_map(|&tool| {
            let installs = enumerate_installations(tool);
            if installs.is_empty() {
                return None;
            }
            let is_conf = is_conflicting(&installs);
            let suggestion = if is_conf {
                if let Some(d) = installs.iter().find(|i| i.is_path_default) {
                    format!(
                        "建议保留 PATH 默认 ({}) 并卸载其他安装，避免版本遮蔽；可用 `npm uninstall -g` / `brew uninstall` 等对应 source 命令清理。",
                        d.path
                    )
                } else {
                    "多处安装版本分歧或运行态混合，建议卸载旧版本以避免遮蔽。".to_string()
                }
            } else if installs.len() >= 2 {
                "多处安装但版本一致且均可运行，无需处理。".to_string()
            } else {
                String::new()
            };
            Some(CliConflict {
                tool: tool.to_string(),
                installations: installs,
                is_conflicting: is_conf,
                suggestion,
            })
        })
        .collect()
}

/// 跑命令 + 非 0 退出码转 Err（含 stderr）。async 版本用于 install/upgrade。
async fn run_and_check_async(mut cmd: TokioCommand, label: &str) -> Result<(), String> {
    let output = cmd.output().await.map_err(|e| {
        tracing::error!(command = %label, error = %e, "spawn failed");
        format!("spawn {label}: {e}")
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = if stderr.trim().is_empty() {
            stdout.to_string()
        } else {
            stderr.to_string()
        };
        tracing::error!(command = %label, stderr = %combined, "command failed");
        return Err(format!("{label} failed: {}", combined.trim()));
    }
    Ok(())
}

/// 从 npm registry 获取指定包的 latest 版本。
async fn fetch_npm_latest(pkg: &str) -> Option<String> {
    let url = format!("https://registry.npmjs.org/{}/latest", pkg);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok()?;
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::warn!(package = pkg, status = %resp.status(), "npm registry request failed");
        return None;
    }
    let body = resp.text().await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    json.get("version").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// semver 比对：local < latest 返回 true（有更新）。
fn has_update_available(local: &str, latest: &str) -> Option<bool> {
    let v_local = semver::Version::parse(local).ok()?;
    let v_latest = semver::Version::parse(latest).ok()?;
    Some(v_local < v_latest)
}

/// 检查 CLI 工具是否有更新可用（npm registry + semver 比对）。
/// 返回值含 latest_version / has_update 字段（检测失败时为 None）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_check_updates() -> Vec<CliToolStatus> {
    tracing::debug!(command = "cli_check_updates", "command invoked");

    let mut results = Vec::new();

    for &tool in TOOLS {
        // 先获取本地版本
        let (installed, version, path) = probe_version(tool);
        let broken = installed && version.is_none();
        let conflict = if which_all(tool).len() >= 2 {
            let installs = enumerate_installations(tool);
            is_conflicting(&installs)
        } else {
            false
        };

        // 查找对应的 npm 包名
        let npm_pkg = NPM_PACKAGES
            .iter()
            .find(|&&(t, _)| t == tool)
            .map(|(_, pkg)| *pkg);

        let (latest_version, has_update) = if let Some(pkg) = npm_pkg {
            // 检查缓存
            let now = std::time::Instant::now();
            let cache = update_cache().read().await;
            let cached = cache.get(tool).and_then(|(ts, v)| {
                if now.duration_since(*ts) < CACHE_TTL {
                    Some(v.clone())
                } else {
                    None
                }
            });

            if let Some(cached_latest) = cached {
                drop(cache);
                tracing::debug!(tool = tool, latest_version = %cached_latest, "using cached version");
                let has_update_val = version.as_ref().and_then(|v| has_update_available(v, &cached_latest));
                (Some(cached_latest), has_update_val)
            } else {
                drop(cache);
                // 缓存未命中或过期，请求 npm registry
                match fetch_npm_latest(pkg).await {
                    Some(latest) => {
                        tracing::debug!(tool = tool, latest_version = %latest, "fetched from npm registry");
                        // 更新缓存
                        let mut cache = update_cache().write().await;
                        cache.insert(tool.to_string(), (now, latest.clone()));
                        let has_update_val = version.as_ref().and_then(|v| has_update_available(v, &latest));
                        (Some(latest), has_update_val)
                    }
                    None => {
                        tracing::warn!(tool = tool, "failed to fetch latest version from npm registry");
                        (None, None)
                    }
                }
            }
        } else {
            (None, None)
        };

        results.push(CliToolStatus {
            name: tool.to_string(),
            installed,
            version,
            path,
            broken,
            conflict,
            latest_version,
            has_update,
        });
    }

    results
}

#[cfg(test)]
#[path = "test_cli_env.rs"]
mod test_cli_env;
