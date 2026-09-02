use crate::gateway;
use aidog_db::{self as db, Db};

pub fn slugify(input: &str) -> String {
    input
        .to_lowercase()
        .replace(" ", "-")
        .replace("（", "-")
        .replace("）", "")
        .replace("(", "-")
        .replace(")", "")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == ' ' {
                '-'
            } else {
                // Strip non-ASCII non-alphanumeric (Chinese chars etc.)
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        // Collapse multiple hyphens
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// 代理服务器状态。票 06 下沉到 `aidog_ctx`（与 Tauri 无关，改由 `AppCtx` 持有），
/// 此处保留 re-export：`use crate::shared::*` 的调用点不必改 import。
pub use aidog_ctx::ProxyHandle;

fn default_bind_lan() -> bool {
    false
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ProxySettings {
    pub port: u16,
    pub autostart: bool,
    #[serde(default)]
    pub silent_launch: bool,
    /// 代理绑定地址：true=0.0.0.0(局域网可访问) / false=127.0.0.1(仅本机)。
    /// 默认**关**（安全缺省：不在用户不知情时把代理开放给局域网）。缺省只影响新装与
    /// 字段缺失的旧记录；已存库里的显式 `true` 照常反序列化为 true，老用户行为不变。
    #[serde(default = "default_bind_lan")]
    pub bind_lan: bool,
}

/// 从 DB 读取 proxy settings；首次运行时自动迁移 proxy_settings.json 文件。
///
/// 票 06：入参由 `&AppHandle` 改为 `&Db` —— 本函数从来只用 AppHandle 取 Db，
/// 直接收 Db 后它与外壳无关，无界面内核可原样复用。
pub async fn load_proxy_settings(db: &Db) -> Result<ProxySettings, String> {
    // 从 DB 读取
    if let Some(val) = db::get_setting(db, "proxy", "settings").await? {
        let s: ProxySettings =
            serde_json::from_value(val).map_err(|e| format!("parse proxy settings: {e}"))?;
        return Ok(s);
    }

    // DB 无记录：尝试从旧文件迁移
    let file_path = aidog_data_dir()?.join("proxy_settings.json");
    if file_path.exists()
        && let Ok(content) = std::fs::read_to_string(&file_path)
        && let Ok(s) = serde_json::from_str::<ProxySettings>(&content)
    {
        // 迁移到 DB
        if let Err(e) = save_proxy_settings_to_db(db, &s).await {
            tracing::warn!(error = %e, "migrate proxy_settings.json to db failed");
        }
        // 删除旧文件
        if let Err(e) = std::fs::remove_file(&file_path) {
            tracing::debug!(error = %e, "remove migrated proxy_settings.json failed");
        }
        return Ok(s);
    }

    // 默认值
    Ok(ProxySettings {
        port: 9890,
        autostart: true,
        silent_launch: false,
        bind_lan: false,
    })
}

pub async fn save_proxy_settings_to_db(db: &Db, settings: &ProxySettings) -> Result<(), String> {
    let value =
        serde_json::to_value(settings).map_err(|e| format!("serialize proxy settings: {e}"))?;
    db::set_setting(
        db,
        gateway::models::SetSettingInput {
            scope: "proxy".to_string(),
            key: "settings".to_string(),
            value,
        },
    )
    .await
}

pub async fn save_proxy_settings(
    db: &Db,
    port: u16,
    autostart: bool,
    silent_launch: bool,
    bind_lan: bool,
) -> Result<(), String> {
    let settings = ProxySettings {
        port,
        autostart,
        silent_launch,
        bind_lan,
    };
    save_proxy_settings_to_db(db, &settings).await
}

pub fn aidog_data_dir() -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let dir = home.join(".aidog");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create ~/.aidog: {e}"))?;
    Ok(dir)
}

/// 生成脚本目录：~/.aidog/scripts/（hook / statusline 脚本统一存放，不再 ~/.aidog/ 根）。
pub fn aidog_scripts_dir() -> Result<std::path::PathBuf, String> {
    let dir = aidog_data_dir()?.join("scripts");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create ~/.aidog/scripts: {e}"))?;
    Ok(dir)
}

/// 删除 ~/.aidog/ 根下遗留的旧脚本文件（迁移到 scripts/ 后清理，避免残留 stale 路径）。
/// best-effort：删除失败仅记录，不阻断。
pub fn cleanup_legacy_root_script(filename: &str) {
    if let Ok(root) = aidog_data_dir() {
        let legacy = root.join(filename);
        if legacy.exists()
            && let Err(e) = std::fs::remove_file(&legacy)
        {
            tracing::warn!(file = %filename, error = %e, "cleanup legacy ~/.aidog script failed");
        }
    }
}

/// 删除 ~/.aidog/scripts/ 下遗留的旧脚本文件（statusline 由 .sh 迁 .py，清理同目录旧 .sh）。
/// best-effort：删除失败仅记录，不阻断。
pub fn cleanup_legacy_scripts_dir_file(scripts_dir: &std::path::Path, filename: &str) {
    let legacy = scripts_dir.join(filename);
    if legacy.exists()
        && let Err(e) = std::fs::remove_file(&legacy)
    {
        tracing::warn!(file = %filename, error = %e, "cleanup legacy scripts/ .sh failed");
    }
}

pub fn detect_uv() -> bool {
    let mut cmd = std::process::Command::new("uv");
    cmd.arg("--version");
    if let Some(p) = gateway::skills::runtime_path() {
        cmd.env("PATH", p);
    }
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// 解析当前应使用的脚本执行器。
///
/// 优先用户持久化选择（`app/script_executor` = "uv" | "python3"）；未持久化时按 live
/// 探测（uv 可用 → uv，否则 python3）。生成脚本 command 串时调用，保证 hook / statusline /
/// codex 一致。
pub async fn resolve_script_invoker(db: &Db) -> gateway::scripts::ScriptInvoker {
    use gateway::scripts::ScriptInvoker;
    if let Ok(Some(v)) = db::get_setting(db, "app", "script_executor").await
        && let Some(s) = v.as_str()
    {
        return ScriptInvoker::from_setting(Some(s));
    }
    ScriptInvoker::from_uv_available(detect_uv())
}
