//! `#[tauri::command]` 手写 tracing 样板收敛点（C3 c3-commands 第 1 批引入）。
//!
//! 迁移前现状：194 个 command 里仅 49 个手写了 `tracing::error!` 覆盖失败分支，其余
//! 逐个手写 `#[tracing::instrument]` + `tracing::debug!("command invoked")` 也参差不齐。
//! [`tauri_command!`] 把这套样板收成一个宏：调用方只写业务签名 + 函数体（含内部 `?`），
//! 宏自动补 `#[tauri::command]` + instrument + entry debug 日志 + **Err 分支自动
//! `tracing::error!`**（默认覆盖，不再靠逐个手写去补齐剩余 145 个）。
#[macro_export]
macro_rules! tauri_command {
    (
        $(#[$meta:meta])*
        pub async fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> Result<$ret:ty, String> $body:block
    ) => {
        $(#[$meta])*
        #[tauri::command]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        pub async fn $name($($arg: $ty),*) -> Result<$ret, String> {
            tracing::debug!(command = stringify!($name), "command invoked");
            let __result: Result<$ret, String> = (async move $body).await;
            if let Err(ref __e) = __result {
                tracing::error!(command = stringify!($name), error = %__e, "command failed");
            }
            __result
        }
    };
}
