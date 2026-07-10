use aidog_core::shared::*;
use aidog_core::gateway::{self, db::{self, Db}};
#[allow(unused_imports)]
use gateway::models::*;
#[allow(unused_imports)]
use tauri::State;
#[allow(unused_imports)]
use serde_json::Value;

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub fn check_uv() -> Result<bool, String> {
    tracing::debug!(command = "check_uv", "command invoked");
    Ok(detect_uv())
}

/// 持久化用户的脚本执行器选择（"uv" | "python3"），供后续脚本生成读取，避免每次询问。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn set_script_executor(executor: String, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "set_script_executor", executor = %executor, "command invoked");
    // 经 ScriptInvoker 规范化（"uv" → uv，其余 → python3），保证存库值与解析一致。
    let normalized = gateway::scripts::ScriptInvoker::from_setting(Some(&executor)).as_setting();
    db::set_setting(&db, SetSettingInput {
        scope: "app".to_string(),
        key: "script_executor".to_string(),
        value: serde_json::Value::String(normalized.to_string()),
    }).await
}

/// 自动安装 uv（用户在 modal 选择「是」后调用）。
///
/// 走官方安装脚本 `curl -LsSf https://astral.sh/uv/install.sh | sh`（Unix）。
/// 成功后持久化选择为 "uv"。Windows 暂不支持自动安装（返回错误，由前端引导手动）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn install_uv(db: State<'_, Db>) -> Result<bool, String> {
    tracing::debug!(command = "install_uv", "command invoked");
    if detect_uv() {
        // 已安装 → 直接记录选择。
        db::set_setting(&db, SetSettingInput {
            scope: "app".to_string(),
            key: "script_executor".to_string(),
            value: serde_json::Value::String("uv".to_string()),
        }).await?;
        return Ok(true);
    }

    #[cfg(unix)]
    {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg("curl -LsSf https://astral.sh/uv/install.sh | sh")
            .output()
            .map_err(|e| { tracing::error!(command = "install_uv", error = %e, "spawn uv installer failed"); format!("spawn uv installer: {e}") })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(command = "install_uv", stderr = %stderr, "uv install script failed");
            return Err(format!("uv install failed: {}", stderr.trim()));
        }
        // 官方脚本装到 ~/.local/bin（或 ~/.cargo/bin）；detect_uv 依赖 PATH，可能本进程
        // PATH 未含安装目录 → 这里以「脚本退出成功」为准记录选择，运行时 hook 由用户 shell PATH 解析 uv。
        db::set_setting(&db, SetSettingInput {
            scope: "app".to_string(),
            key: "script_executor".to_string(),
            value: serde_json::Value::String("uv".to_string()),
        }).await?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        let _ = &db;
        Err("auto-install uv is only supported on Unix; please install uv manually".to_string())
    }
}

#[cfg(test)]
#[path = "test_script_executor.rs"]
mod test_script_executor;
