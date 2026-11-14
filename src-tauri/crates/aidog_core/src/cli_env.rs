//! CLI 工具环境检测：Claude Code / Codex CLI 版本 / 安装 / 升级 / 冲突诊断。
//!
//! 抄 `install_uv` 后端 spawn 模式（不动 capability / tauri-plugin-shell 配置）。
//! 本模块各真 spawn 入口（`probe_version`/`which_first`/`which_all`/`cli_install`/
//! `cli_upgrade`）自调 `gateway::skills::runtime_path()` 拿登录 shell 合并 PATH（幂等缓存，
//! 仅首次真跑），逐个 `Command` 上 `.env("PATH", p)` 显式注入（不改进程全局 env），
//! 子进程即可命中用户 brew/nvm/native installer 装的 CLI。
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

/// 受支持的 CLI 客户端。
pub const TOOLS: &[&str] = &["claude", "codex", "pi"];

/// npm registry 包名映射（tool -> npm package）。
const NPM_PACKAGES: &[(&str, &str)] = &[
    ("claude", "@anthropic-ai/claude-code"),
    ("codex", "@openai/codex"),
    ("pi", "@earendil-works/pi-coding-agent"),
];

/// 更新检测缓存（1h TTL）。
static UPDATE_CACHE_INIT: std::sync::OnceLock<
    tokio::sync::RwLock<std::collections::HashMap<String, (std::time::Instant, String)>>,
> = std::sync::OnceLock::new();

/// 缓存 TTL：1 小时。
const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

/// 获取更新缓存（延迟初始化）。
fn update_cache()
-> &'static tokio::sync::RwLock<std::collections::HashMap<String, (std::time::Instant, String)>> {
    UPDATE_CACHE_INIT.get_or_init(|| tokio::sync::RwLock::new(std::collections::HashMap::new()))
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
    use std::os::windows::process::CommandExt;
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
    if let Some(p) = crate::gateway::skills::runtime_path() {
        cmd.env("PATH", p);
    }
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
    if let Some(p) = crate::gateway::skills::runtime_path() {
        cmd.env("PATH", p);
    }
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
    if let Some(p) = crate::gateway::skills::runtime_path() {
        cmd.env("PATH", p);
    }
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

crate::tauri_command! {
pub fn cli_check_versions() -> Vec<CliToolStatus> {
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
}

crate::tauri_command! {
pub async fn cli_install(tool: String) -> Result<(), String> {
    tracing::debug!(command = "cli_install", tool = %tool, "command invoked");
    if !TOOLS.contains(&tool.as_str()) {
        return Err(format!("unsupported tool: {tool}"));
    }
    let runtime_path = crate::gateway::skills::runtime_path();
    match tool.as_str() {
        "claude" => {
            #[cfg(unix)]
            {
                // POSIX 优先 native installer（claude.ai/install.sh），失败回退 npm。
                let script = "tmp=$(mktemp) && curl -fsSL https://claude.ai/install.sh -o $tmp && bash $tmp; status=$?; rm -f $tmp; exit $status";
                let mut cmd = TokioCommand::new("bash");
                cmd.arg("-c").arg(script);
                if let Some(p) = runtime_path {
                    cmd.env("PATH", p);
                }
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
            if let Some(p) = runtime_path {
                cmd.env("PATH", p);
            }
            no_window_tokio(&mut cmd).await;
            run_and_check_async(cmd, "claude install (npm)").await
        }
        "codex" => {
            let mut cmd = TokioCommand::new("npm");
            cmd.args(["i", "-g", "@openai/codex@latest"]);
            if let Some(p) = runtime_path {
                cmd.env("PATH", p);
            }
            no_window_tokio(&mut cmd).await;
            run_and_check_async(cmd, "codex install (npm)").await
        }
        "pi" => {
            // 官方安装命令带 `--ignore-scripts`（quickstart.md：pi 普通 npm 安装不需要
            // 依赖生命周期脚本）。不加会在部分环境触发无谓的构建步骤。
            let mut cmd = TokioCommand::new("npm");
            cmd.args([
                "i",
                "-g",
                "--ignore-scripts",
                "@earendil-works/pi-coding-agent@latest",
            ]);
            if let Some(p) = runtime_path {
                cmd.env("PATH", p);
            }
            no_window_tokio(&mut cmd).await;
            run_and_check_async(cmd, "pi install (npm)").await
        }
        _ => Err(format!("unsupported tool: {tool}")),
    }
}
}

crate::tauri_command! {
pub async fn cli_upgrade(tool: String) -> Result<(), String> {
    tracing::debug!(command = "cli_upgrade", tool = %tool, "command invoked");
    if !TOOLS.contains(&tool.as_str()) {
        return Err(format!("unsupported tool: {tool}"));
    }
    let runtime_path = crate::gateway::skills::runtime_path();
    match tool.as_str() {
        "claude" => {
            // 优先 `claude update`（native installer 自带子命令）。
            let mut cmd = TokioCommand::new("claude");
            cmd.arg("update");
            if let Some(p) = runtime_path {
                cmd.env("PATH", p);
            }
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
            if let Some(p) = runtime_path {
                cmd.env("PATH", p);
            }
            no_window_tokio(&mut cmd).await;
            run_and_check_async(cmd, "claude upgrade (npm)").await
        }
        "codex" => {
            // POSIX 先试 `codex update`；失败 / Windows 走 uninstall + install 自愈。
            #[cfg(unix)]
            {
                let mut cmd = TokioCommand::new("codex");
                cmd.arg("update");
                if let Some(p) = runtime_path {
                    cmd.env("PATH", p);
                }
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
            if let Some(p) = runtime_path {
                u.env("PATH", p);
            }
            no_window_tokio(&mut u).await;
            let _ = u.output().await;
            let mut i = TokioCommand::new("npm");
            i.args(["i", "-g", "@openai/codex@latest"]);
            if let Some(p) = runtime_path {
                i.env("PATH", p);
            }
            no_window_tokio(&mut i).await;
            run_and_check_async(i, "codex upgrade (npm reinstall)").await
        }
        "pi" => {
            // `pi update --self` 是官方自升级子命令（packages.md:35），且从不弹项目信任提示
            // （usage.md:164）。失败回退 npm 重装。
            let mut cmd = TokioCommand::new("pi");
            cmd.args(["update", "--self"]);
            if let Some(p) = runtime_path {
                cmd.env("PATH", p);
            }
            no_window_tokio(&mut cmd).await;
            if let Ok(o) = cmd.output().await {
                if o.status.success() {
                    return Ok(());
                }
                let err = String::from_utf8_lossy(&o.stderr);
                tracing::warn!(error = %err, "pi update --self failed, falling back to npm");
            }
            let mut cmd = TokioCommand::new("npm");
            cmd.args([
                "i",
                "-g",
                "--ignore-scripts",
                "@earendil-works/pi-coding-agent@latest",
            ]);
            if let Some(p) = runtime_path {
                cmd.env("PATH", p);
            }
            no_window_tokio(&mut cmd).await;
            run_and_check_async(cmd, "pi upgrade (npm)").await
        }
        _ => Err(format!("unsupported tool: {tool}")),
    }
}
}

crate::tauri_command! {
pub async fn cli_diagnose_conflicts() -> Vec<CliConflict> {
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
    let client = if let Some(db) = aidog_ctx::try_db() {
        let db = std::sync::Arc::new(db.clone());
        crate::gateway::http_client::build_http_client_system(&db, 8, 0).await
    } else {
        // 无 ctx（内核启动早期 / 测试）= 有意直连：版本检测非代理出站关键面，DB 未就绪时
        // 只影响「最新版本」提示，不静默走错代理（与 search/notification 工厂的 Err 语义不同）。
        reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .ok()?
    };
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::warn!(package = pkg, status = %resp.status(), "npm registry request failed");
        return None;
    }
    let body = resp.text().await.ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    json.get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// semver 比对：local < latest 返回 true（有更新）。
fn has_update_available(local: &str, latest: &str) -> Option<bool> {
    let v_local = semver::Version::parse(local).ok()?;
    let v_latest = semver::Version::parse(latest).ok()?;
    Some(v_local < v_latest)
}

crate::tauri_command! {
/// 检查 CLI 工具是否有更新可用（npm registry + semver 比对）。
/// 返回值含 latest_version / has_update 字段（检测失败时为 None）。
pub async fn cli_check_updates() -> Vec<CliToolStatus> {
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
                let has_update_val = version
                    .as_ref()
                    .and_then(|v| has_update_available(v, &cached_latest));
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
                        let has_update_val = version
                            .as_ref()
                            .and_then(|v| has_update_available(v, &latest));
                        (Some(latest), has_update_val)
                    }
                    None => {
                        tracing::warn!(
                            tool = tool,
                            "failed to fetch latest version from npm registry"
                        );
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
}

// ─── 票 07：按安装来源的版本更新（数据驱动映射表）──────────

/// 更新命令模板表（数据驱动）：source → argv 模板。
/// 占位符：`{pkg}` = NPM_PACKAGES 包名、`{cask}` = HOMEBREW_CASKS 名、`{extra}` = 工具专属额外参数展开。
/// 未知 source 不在表内 = Manual（不猜）；native / pip / scoop / system 亦不在表内（提示手动）。
const UPDATE_TEMPLATES: &[(&str, &[&str])] = &[
    ("npm-global", &["npm", "install", "-g", "{extra}", "{pkg}@latest"]),
    ("nvm", &["npm", "install", "-g", "{extra}", "{pkg}@latest"]),
    ("fnm", &["npm", "install", "-g", "{extra}", "{pkg}@latest"]),
    ("volta", &["npm", "install", "-g", "{extra}", "{pkg}@latest"]),
    ("mise", &["npm", "install", "-g", "{extra}", "{pkg}@latest"]),
    ("bun", &["npm", "install", "-g", "{extra}", "{pkg}@latest"]),
    ("pnpm", &["npm", "install", "-g", "{extra}", "{pkg}@latest"]),
    ("homebrew", &["brew", "upgrade", "--cask", "{cask}"]),
];

/// homebrew cask 名（formulae.brew.sh 2026-11-08 核实：claude-code / codex 有 cask，pi 无 → Manual）。
const HOMEBREW_CASKS: &[(&str, &str)] = &[("claude", "claude-code"), ("codex", "codex")];

/// npm 更新额外参数（pi 的 postinstall 需跳过，对齐 cli_upgrade 兜底路径）。
const NPM_UPDATE_EXTRA: &[(&str, &[&str])] = &[("pi", &["--ignore-scripts"])];

/// 单次更新命令超时：包管理器下载可能较慢。
const UPDATE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// 更新动作：Cmd = 固定形式 argv（模板占位符已替换，不拼用户输入）；Manual = 提示手动。
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateAction {
    Cmd(Vec<String>),
    Manual(&'static str),
}

/// (tool, source) → 更新动作。数据驱动查表；未知 source / 无包名 / 无 cask → Manual。
fn update_action_for(tool: &str, source: &str) -> UpdateAction {
    let Some((_, tpl)) = UPDATE_TEMPLATES.iter().find(|(s, _)| *s == source) else {
        return UpdateAction::Manual("manual: unsupported install source");
    };
    let mut argv: Vec<String> = Vec::new();
    for part in tpl.iter() {
        let s = *part;
        if s.contains("{pkg}") {
            let Some((_, pkg)) = NPM_PACKAGES.iter().find(|&&(t, _)| t == tool) else {
                return UpdateAction::Manual("manual: no npm package for tool");
            };
            argv.push(s.replace("{pkg}", pkg));
            continue;
        }
        if s.contains("{cask}") {
            let Some((_, cask)) = HOMEBREW_CASKS.iter().find(|&&(t, _)| t == tool) else {
                return UpdateAction::Manual("manual: no homebrew cask for tool");
            };
            argv.push(s.replace("{cask}", cask));
            continue;
        }
        if s == "{extra}" {
            if let Some((_, extra)) = NPM_UPDATE_EXTRA.iter().find(|&&(t, _)| t == tool) {
                argv.extend(extra.iter().map(|s| s.to_string()));
            }
            continue;
        }
        argv.push(s.to_string());
    }
    UpdateAction::Cmd(argv)
}

/// 对指定安装路径重跑 `--version`（更新后回显 / before-after 比对用）。
fn version_at(path: &str) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.arg("--version");
    no_window(&mut cmd);
    match cmd.output() {
        Ok(o) if o.status.success() => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            extract_version(&text)
        }
        _ => None,
    }
}

/// 带超时跑一次外部命令，合并 stdout/stderr 返回；非 0 退出 = Err（含退出码与输出）。
/// `path_env` 显式注入 PATH（None = 不注入，测试直接给绝对路径）。
async fn run_command_with_timeout(
    argv: &[String],
    timeout: std::time::Duration,
    path_env: Option<&str>,
) -> Result<String, String> {
    let Some(prog) = argv.first() else {
        return Err("empty argv".into());
    };
    let mut cmd = TokioCommand::new(prog);
    cmd.args(&argv[1..]);
    if let Some(p) = path_env {
        cmd.env("PATH", p);
    }
    no_window_tokio(&mut cmd).await;
    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| format!("timeout after {}s", timeout.as_secs()))?
        .map_err(|e| format!("spawn failed: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".into());
        let tail: String = text.trim().chars().rev().take(400).collect::<String>().chars().rev().collect();
        return Err(format!("exit {code}: {tail}"));
    }
    Ok(text)
}

#[cfg(test)]
mod test_cli_env {
    use super::*;

    #[test]
    fn extract_version_plain() {
        assert_eq!(extract_version("2.1.156"), Some("2.1.156".to_string()));
    }

    #[test]
    fn extract_version_with_label() {
        assert_eq!(
            extract_version("claude 2.1.156 (build abc)"),
            Some("2.1.156".to_string())
        );
    }

    #[test]
    fn extract_version_prerelease() {
        assert_eq!(
            extract_version("2.1.156-beta.1"),
            Some("2.1.156-beta.1".to_string())
        );
    }

    #[test]
    fn extract_version_codex_timestamp_patch() {
        // codex 时间戳式 patch（参考 research cli-install-tech.md）
        assert_eq!(
            extract_version("0.1.2505172116"),
            Some("0.1.2505172116".to_string())
        );
    }

    #[test]
    fn extract_version_missing_dots_not_matched() {
        // 只有一个 dot 不算版本号
        assert_eq!(extract_version("v1.2"), None);
    }

    #[test]
    fn extract_version_none() {
        assert_eq!(extract_version("no version here"), None);
    }

    #[test]
    fn infer_source_nvm() {
        assert_eq!(
            infer_source("/home/u/.nvm/versions/node/v20/bin/claude"),
            "nvm"
        );
    }

    #[test]
    fn infer_source_homebrew_cellar() {
        // Homebrew Cellar 真身须先于通用规则命中
        assert_eq!(
            infer_source("/opt/homebrew/Cellar/claude/1.0/bin/claude"),
            "homebrew"
        );
    }

    #[test]
    fn infer_source_homebrew_bin() {
        assert_eq!(infer_source("/opt/homebrew/bin/claude"), "homebrew");
    }

    #[test]
    fn infer_source_volta() {
        assert_eq!(infer_source("/Users/u/.volta/bin/claude"), "volta");
    }

    #[test]
    fn infer_source_native_installer() {
        assert_eq!(
            infer_source("/Users/u/.local/share/claude/versions/2.1.156/claude"),
            "native"
        );
    }

    #[test]
    fn infer_source_system_default() {
        assert_eq!(infer_source("/usr/local/bin/claude"), "system");
    }

    #[test]
    fn infer_source_windows_backslash() {
        assert_eq!(infer_source(r"C:\Users\u\scoop\shims\claude.exe"), "scoop");
    }

    #[test]
    fn is_conflicting_single_install() {
        let installs = vec![CliInstallation {
            path: "/a".into(),
            version: Some("1.0.0".into()),
            runnable: true,
            source: "system".into(),
            is_path_default: true,
        }];
        assert!(!is_conflicting(&installs));
    }

    #[test]
    fn is_conflicting_two_same_version_both_runnable_no_conflict() {
        // 同版本装两份且都能跑不算冲突（不打扰用户）
        let installs = vec![
            CliInstallation {
                path: "/a".into(),
                version: Some("1.0.0".into()),
                runnable: true,
                source: "system".into(),
                is_path_default: true,
            },
            CliInstallation {
                path: "/b".into(),
                version: Some("1.0.0".into()),
                runnable: true,
                source: "nvm".into(),
                is_path_default: false,
            },
        ];
        assert!(!is_conflicting(&installs));
    }

    #[test]
    fn is_conflicting_version_divergence() {
        let installs = vec![
            CliInstallation {
                path: "/a".into(),
                version: Some("1.0.0".into()),
                runnable: true,
                source: "system".into(),
                is_path_default: true,
            },
            CliInstallation {
                path: "/b".into(),
                version: Some("2.0.0".into()),
                runnable: true,
                source: "nvm".into(),
                is_path_default: false,
            },
        ];
        assert!(is_conflicting(&installs));
    }

    #[test]
    fn is_conflicting_runnable_mixed() {
        let installs = vec![
            CliInstallation {
                path: "/a".into(),
                version: Some("1.0.0".into()),
                runnable: true,
                source: "system".into(),
                is_path_default: true,
            },
            CliInstallation {
                path: "/b".into(),
                version: None,
                runnable: false,
                source: "nvm".into(),
                is_path_default: false,
            },
        ];
        assert!(is_conflicting(&installs));
    }

    #[test]
    fn supported_tools_are_claude_codex_pi() {
        assert_eq!(TOOLS, &["claude", "codex", "pi"]);
    }

    #[test]
    fn every_tool_has_an_npm_package() {
        // 更新检测走 NPM_PACKAGES 查 registry：漏一条则该工具静默永远「无更新」。
        for &tool in TOOLS {
            assert!(
                NPM_PACKAGES.iter().any(|(t, _)| *t == tool),
                "tool {tool} missing from NPM_PACKAGES"
            );
        }
    }
}

/// 票 07：单 agent 更新状态（版本对照 + 可否自动更新）。
#[derive(serde::Serialize, Clone)]
pub struct AgentUpdateStatus {
    pub tool: String,
    pub installed: bool,
    /// PATH 默认命中安装的当前版本。
    pub version: Option<String>,
    /// 包管理器可升级版本（npm registry latest；检测失败/离线 = None，UI 标 unknown）。
    pub latest_version: Option<String>,
    /// Some(true)=有更新 / Some(false)=最新 / None=unknown。
    pub has_update: Option<bool>,
    /// PATH 默认命中安装的来源（infer_source 词表）。
    pub source: Option<String>,
    /// 映射表支持自动更新。
    pub updatable: bool,
    /// 不支持自动更新时的提示（UI 按 key 本地化）。
    pub manual_hint: Option<String>,
}

/// 票 07：单 agent 更新执行结果（前后版本回显）。
#[derive(serde::Serialize, Clone)]
pub struct AgentUpdateResult {
    pub tool: String,
    pub source: String,
    pub before_version: Option<String>,
    pub after_version: Option<String>,
    pub success: bool,
    /// 更新命令输出尾段（截 400 字符）。
    pub message: String,
}

/// PATH 默认命中安装的 (source, 映射表可否自动更新, 手动提示)。
fn default_installation_info(tool: &str) -> (Option<String>, bool, Option<String>) {
    let installs = enumerate_installations(tool);
    let Some(inst) = installs.iter().find(|i| i.is_path_default).or_else(|| installs.first()) else {
        return (None, false, None);
    };
    let action = update_action_for(tool, &inst.source);
    match action {
        UpdateAction::Cmd(_) => (Some(inst.source.clone()), true, None),
        UpdateAction::Manual(reason) => (Some(inst.source.clone()), false, Some(reason.to_string())),
    }
}

crate::tauri_command! {
/// 票 07：各 agent 当前版本 vs 包管理器可升级版本。latest 查不到标 unknown（None）。
pub async fn agent_update_check() -> Vec<AgentUpdateStatus> {
    tracing::debug!(command = "agent_update_check", "command invoked");
    let statuses = cli_check_updates().await;
    statuses
        .into_iter()
        .map(|s| {
            let (source, updatable, manual_hint) = default_installation_info(&s.name);
            AgentUpdateStatus {
                tool: s.name,
                installed: s.installed,
                version: s.version,
                latest_version: s.latest_version,
                has_update: s.has_update,
                source,
                updatable,
                manual_hint: manual_hint.filter(|_| !updatable),
            }
        })
        .collect()
}
}

crate::tauri_command! {
/// 票 07：按安装来源执行单个 agent 更新（UI 确认后的高危动作；不做批量全更）。
/// 取 PATH 默认安装 → 映射表查命令 → 带超时执行 → 重跑版本检测回显前后版本。
pub async fn agent_update_apply(tool: String) -> Result<AgentUpdateResult, String> {
    tracing::debug!(command = "agent_update_apply", tool = %tool, "command invoked");
    if !TOOLS.contains(&tool.as_str()) {
        return Err(format!("unsupported tool: {tool}"));
    }
    let installs = enumerate_installations(&tool);
    let Some(inst) = installs.iter().find(|i| i.is_path_default).or_else(|| installs.first()).cloned() else {
        return Err(format!("{tool} not installed"));
    };
    let before = inst.version.clone();
    let argv = match update_action_for(&tool, &inst.source) {
        UpdateAction::Cmd(argv) => argv,
        UpdateAction::Manual(reason) => return Err(reason.to_string()),
    };
    let output = run_update_command(&argv).await?;
    let after = version_at(&inst.path);
    let tail: String = output.trim().chars().rev().take(400).collect::<String>().chars().rev().collect();
    Ok(AgentUpdateResult {
        tool,
        source: inst.source,
        before_version: before,
        after_version: after,
        success: true,
        message: tail,
    })
}
}

/// run_command_with_timeout 的公开包装：注入登录 shell 合并 PATH（幂等缓存）。
async fn run_update_command(argv: &[String]) -> Result<String, String> {
    run_command_with_timeout(argv, UPDATE_TIMEOUT, crate::gateway::skills::runtime_path()).await
}

#[cfg(test)]
mod test_cli_update {
    use super::*;

    #[test]
    fn update_action_mapping_per_source() {
        // npm 系来源 → Cmd：npm install -g <pkg>@latest
        for src in ["npm-global", "nvm", "fnm", "volta", "mise", "bun", "pnpm"] {
            for (tool, pkg) in NPM_PACKAGES {
                match update_action_for(tool, src) {
                    UpdateAction::Cmd(argv) => {
                        assert_eq!(&argv[..3], ["npm", "install", "-g"], "source {src} tool {tool}");
                        assert!(
                            argv.iter().any(|a| a == &format!("{pkg}@latest")),
                            "source {src} tool {tool}: {argv:?}"
                        );
                    }
                    other => panic!("source {src} tool {tool} 应为 Cmd，实际 {other:?}"),
                }
            }
        }
        // pi 的 npm 更新带 --ignore-scripts（postinstall 跳过）
        match update_action_for("pi", "npm-global") {
            UpdateAction::Cmd(argv) => assert!(argv.contains(&"--ignore-scripts".to_string()), "{argv:?}"),
            other => panic!("pi npm-global 应为 Cmd，实际 {other:?}"),
        }
        // homebrew：cask 表内 → Cmd；表外（pi）→ Manual
        match update_action_for("claude", "homebrew") {
            UpdateAction::Cmd(argv) => {
                assert_eq!(&argv[..4], ["brew", "upgrade", "--cask", "claude-code"], "{argv:?}")
            }
            other => panic!("claude homebrew 应为 Cmd，实际 {other:?}"),
        }
        match update_action_for("codex", "homebrew") {
            UpdateAction::Cmd(argv) => assert_eq!(&argv[..4], ["brew", "upgrade", "--cask", "codex"], "{argv:?}"),
            other => panic!("codex homebrew 应为 Cmd，实际 {other:?}"),
        }
        assert!(matches!(update_action_for("pi", "homebrew"), UpdateAction::Manual(_)));
        // 不支持自动更新的来源 → Manual（native / pip / scoop / system / 未知，不猜）
        for src in ["native", "pip", "scoop", "system", "totally-unknown"] {
            assert!(
                matches!(update_action_for("claude", src), UpdateAction::Manual(_)),
                "source {src} 应为 Manual"
            );
        }
    }

    /// 临时目录假可执行文件：脚本三件套 + 假版本二进制。收尾自清理。
    #[cfg(unix)]
    fn fake_bin_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("aidog-cli-update-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, body: &str| {
            let p = dir.join(name);
            std::fs::write(&p, body).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        write(
            "ok.sh",
            "#!/bin/sh\necho 'updated to 2.0.0'\necho 'warning noise' >&2\nexit 0\n",
        );
        write("fail.sh", "#!/bin/sh\necho 'boom' >&2\nexit 3\n");
        write("slow.sh", "#!/bin/sh\nsleep 5\n");
        write("fake-tool.sh", "#!/bin/sh\necho 'fake-tool 1.2.3'\n");
        dir
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_success_captures_output() {
        let dir = fake_bin_dir("ok");
        let argv = vec![dir.join("ok.sh").to_string_lossy().into_owned()];
        let out = run_command_with_timeout(&argv, std::time::Duration::from_secs(10), None)
            .await
            .expect("success path");
        assert!(out.contains("updated to 2.0.0"), "stdout: {out}");
        assert!(out.contains("warning noise"), "stderr 也应捕获: {out}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_failure_reports_exit_and_stderr() {
        let dir = fake_bin_dir("fail");
        let argv = vec![dir.join("fail.sh").to_string_lossy().into_owned()];
        let err = run_command_with_timeout(&argv, std::time::Duration::from_secs(10), None)
            .await
            .expect_err("failure path");
        assert!(err.contains("exit 3"), "{err}");
        assert!(err.contains("boom"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_timeout_errors() {
        let dir = fake_bin_dir("slow");
        let argv = vec![dir.join("slow.sh").to_string_lossy().into_owned()];
        let err = run_command_with_timeout(&argv, std::time::Duration::from_millis(300), None)
            .await
            .expect_err("timeout path");
        assert!(err.contains("timeout"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn version_at_fake_binary_before_and_after() {
        let dir = fake_bin_dir("ver");
        let bin = dir.join("fake-tool.sh").to_string_lossy().into_owned();
        assert_eq!(version_at(&bin), Some("1.2.3".to_string()));
        assert_eq!(version_at(&dir.join("missing.bin").to_string_lossy()), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
