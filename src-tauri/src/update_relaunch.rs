//! 桌面更新收尾命令（票 04）：确认重启后接入内核排空闸门。
//! 排空状态机本体在 `aidog_core::gateway::proxy::drain`（内核 agent 维护）；
//! 这两个命令只是桌面壳侧的入口：begin 置排空态拒新请求，
//! wait+restart 在在途清零后由 Tauri 重启进已安装的新版本。
//! 带 AppHandle，不能走 `tauri_command!`（宏会展开 http handler），用原生 `#[tauri::command]`。

/// 置排空态：此后代理新请求一律 503 updating。首次返回 true，已在排空中返回 false。
#[tauri::command]
pub async fn update_begin_drain() -> Result<bool, String> {
    tracing::debug!(command = "update_begin_drain", "command invoked");
    Ok(aidog_core::gateway::proxy::begin_drain())
}

/// 解除排空态：安装失败或用户取消时放弃本次更新，恢复代理正常服务（新请求不再 503）。
/// 幂等：不在排空中返回 false。
#[tauri::command]
pub async fn update_abort_drain() -> Result<bool, String> {
    tracing::debug!(command = "update_abort_drain", "command invoked");
    Ok(aidog_core::gateway::proxy::abort_drain())
}

/// 桌面聊天请求可能持续较久；用户拍板最多等 120 秒，到点仍重启进新版。
const DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// 等待在途清零，或最多 120 秒后重启进新版。
/// 调用前应已 `update_begin_drain`；重启是进程级动作，无返回路径。
#[tauri::command]
pub async fn update_relaunch_when_drained(app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "update_relaunch_when_drained", "command invoked");
    tauri::async_runtime::spawn(async move {
        let _ = wait_drained_with_timeout(aidog_core::gateway::proxy::wait_empty()).await;
        app.restart();
    });
    Ok(())
}

/// true = 已排空；false = 超时，调用方两种情况都继续重启。
async fn wait_drained_with_timeout(wait: impl std::future::Future<Output = ()>) -> bool {
    tokio::time::timeout(DRAIN_TIMEOUT, wait).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 在途永不归零时本测试必须红转绿——超时后必须返回去执行重启。
    #[tokio::test(start_paused = true)]
    async fn timeout_restarts_even_when_inflight_never_drains() {
        let never = std::future::pending::<()>();
        assert!(
            !wait_drained_with_timeout(never).await,
            "排空超时后必须返回（仍执行重启），不得永久挂起"
        );
    }

    /// 在途清零立即返回，不等满 120 秒。
    #[tokio::test(start_paused = true)]
    async fn drained_before_timeout_returns_immediately() {
        assert!(wait_drained_with_timeout(std::future::ready(())).await);
    }
}
