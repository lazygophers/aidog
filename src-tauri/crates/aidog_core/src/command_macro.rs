//! `#[tauri::command]` 手写 tracing 样板收敛点（C3 c3-commands 第 1 批引入）。
//!
//! 迁移前现状：194 个 command 里仅 49 个手写了 `tracing::error!` 覆盖失败分支，其余
//! 逐个手写 `#[tracing::instrument]` + `tracing::debug!("command invoked")` 也参差不齐。
//! [`tauri_command!`] 把这套样板收成一个宏：调用方只写业务签名 + 函数体（含内部 `?`），
//! 宏自动补 `#[tauri::command]` + instrument + entry debug 日志 + **Err 分支自动
//! `tracing::error!`**（默认覆盖，不再靠逐个手写去补齐剩余 145 个）。
//!
//! 2026-09-02（票 05 prefactor）：分支扩到 4 条，覆盖 async/sync × `Result<_, String>`/其他
//! 返回类型，业务代码里**零处手写 `#[tauri::command]`**——全部 206 个命令从这一个宏出来，
//! 后续给命令加 HTTP 形态只需改这里一处。分支顺序不可换：`$ret:ty` 兜底分支会把
//! `Result<_, String>` 一起吃掉，必须排在带 String 的分支之后。
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

    // 同步命令 + `Result<_, String>`：与上一分支等价，只是没有 async。
    // 体用 IIFE 包住，才能在返回前截获 Err 分支补日志（同步没有 `async move` 块可用）。
    (
        $(#[$meta:meta])*
        pub fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> Result<$ret:ty, String> $body:block
    ) => {
        $(#[$meta])*
        #[tauri::command]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        #[allow(clippy::redundant_closure_call)]
        pub fn $name($($arg: $ty),*) -> Result<$ret, String> {
            tracing::debug!(command = stringify!($name), "command invoked");
            let __result: Result<$ret, String> = (move || $body)();
            if let Err(ref __e) = __result {
                tracing::error!(command = stringify!($name), error = %__e, "command failed");
            }
            __result
        }
    };

    // 兜底分支：返回类型不是 `Result<_, String>`（结构化错误如 `Result<_, ProxyStartError>`，
    // 或干脆不是 Result 如 `Vec<CliToolStatus>`）。只补 `#[tauri::command]` + instrument +
    // 入口日志，**不做 Err 自动日志**（错误类型未必实现 Display，且这些命令原本就没有）。
    // 必须排在带 String 的分支之后：`$ret:ty` 会把 `Result<_, String>` 一起吃掉。
    (
        $(#[$meta:meta])*
        pub async fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> $ret:ty $body:block
    ) => {
        $(#[$meta])*
        #[tauri::command]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        pub async fn $name($($arg: $ty),*) -> $ret {
            tracing::debug!(command = stringify!($name), "command invoked");
            $body
        }
    };

    (
        $(#[$meta:meta])*
        pub fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> $ret:ty $body:block
    ) => {
        $(#[$meta])*
        #[tauri::command]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        pub fn $name($($arg: $ty),*) -> $ret {
            tracing::debug!(command = stringify!($name), "command invoked");
            $body
        }
    };
}
