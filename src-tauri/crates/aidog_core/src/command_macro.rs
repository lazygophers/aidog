//! `#[tauri::command]` 手写 tracing 样板收敛点（C3 c3-commands 第 1 批引入）。
//!
//! 迁移前现状：194 个 command 里仅 49 个手写了 `tracing::error!` 覆盖失败分支，其余
//! 逐个手写 `#[tracing::instrument]` + `tracing::debug!("command invoked")` 也参差不齐。
//! [`tauri_command!`] 把这套样板收成一个宏：调用方只写业务签名 + 函数体（含内部 `?`），
//! 宏自动补 `#[tauri::command]` + instrument + entry debug 日志 + **Err 分支自动
//! `tracing::error!`**（默认覆盖，不再靠逐个手写去补齐剩余 145 个）。
//!
//! 2026-09-02（票 05 prefactor）：业务代码里**零处手写 `#[tauri::command]`**——全部 207 个
//! 命令从这一个宏出来，后续给命令加 HTTP 形态只需改这里一处。
//!
//! 2026-09-03（票 07）：**双展开**。每条命令按 feature 生成两种形态，调用方写法不变：
//!
//! - `desktop`（默认开）：函数上挂 `#[tauri::command]`，供 `generate_handler!` 注册，
//!   桌面壳行为一字不变；
//! - `http`（默认开）：额外生成 `pub mod <命令名> { pub async fn http(...) }`——一个 axum
//!   handler，JSON body 反序列化成参数、返回值序列化成 JSON，供票 08 挂 `/rpc/<命令名>`。
//!   语义对齐细节见 [`crate::http_command`]。模块名与函数名同名不冲突（模块在类型命名空间，
//!   函数在值命名空间），`#[tauri::command]` 自身只额外生成 `macro_rules!`，不占模块名。
//!
//! **分支顺序不可换**（6 条，async/sync × `Result<_, String>` / `Result<_, E>` / 非 Result）：
//! `$ret:ty` 兜底分支会把 `Result<_, _>` 一起吃掉，`Result<$ok:ty, $err:ty>` 又会把
//! `Result<_, String>` 一起吃掉，所以必须按「String → 泛型 Err → 兜底」排列。
#[macro_export]
macro_rules! tauri_command {
    (
        $(#[$meta:meta])*
        pub async fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> Result<$ret:ty, String> $body:block
    ) => {
        $(#[$meta])*
        #[cfg_attr(feature = "desktop", tauri::command)]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        pub async fn $name($($arg: $ty),*) -> Result<$ret, String> {
            tracing::debug!(command = stringify!($name), "command invoked");
            let __result: Result<$ret, String> = (async move $body).await;
            if let Err(ref __e) = __result {
                tracing::error!(command = stringify!($name), error = %__e, "command failed");
            }
            __result
        }

        $crate::__aidog_http_command!(async string_err, [$(#[$meta])*] $name ($($arg : $ty),*));
    };

    // 同步命令 + `Result<_, String>`：与上一分支等价，只是没有 async。
    // 体用 IIFE 包住，才能在返回前截获 Err 分支补日志（同步没有 `async move` 块可用）。
    (
        $(#[$meta:meta])*
        pub fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> Result<$ret:ty, String> $body:block
    ) => {
        $(#[$meta])*
        #[cfg_attr(feature = "desktop", tauri::command)]
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

        $crate::__aidog_http_command!(sync string_err, [$(#[$meta])*] $name ($($arg : $ty),*));
    };

    // 结构化错误（如 `Result<_, ProxyStartError>`）：**不做 Err 自动日志**（错误类型未必
    // 实现 Display，且这些命令原本就没有）。HTTP 侧把 Err 序列化成 JSON 当 reject 值，
    // 与 Tauri 的 reject 语义一致。必须排在带 String 的分支之后。
    (
        $(#[$meta:meta])*
        pub async fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> Result<$ok:ty, $err:ty> $body:block
    ) => {
        $(#[$meta])*
        #[cfg_attr(feature = "desktop", tauri::command)]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        pub async fn $name($($arg: $ty),*) -> Result<$ok, $err> {
            tracing::debug!(command = stringify!($name), "command invoked");
            $body
        }

        $crate::__aidog_http_command!(async typed_err, [$(#[$meta])*] $name ($($arg : $ty),*));
    };

    (
        $(#[$meta:meta])*
        pub fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> Result<$ok:ty, $err:ty> $body:block
    ) => {
        $(#[$meta])*
        #[cfg_attr(feature = "desktop", tauri::command)]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        pub fn $name($($arg: $ty),*) -> Result<$ok, $err> {
            tracing::debug!(command = stringify!($name), "command invoked");
            $body
        }

        $crate::__aidog_http_command!(sync typed_err, [$(#[$meta])*] $name ($($arg : $ty),*));
    };

    // 兜底分支：返回类型不是 Result（如 `Vec<CliToolStatus>`）。HTTP 侧直接序列化返回值。
    // 必须排在所有 Result 分支之后：`$ret:ty` 会把 Result 一起吃掉。
    (
        $(#[$meta:meta])*
        pub async fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> $ret:ty $body:block
    ) => {
        $(#[$meta])*
        #[cfg_attr(feature = "desktop", tauri::command)]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        pub async fn $name($($arg: $ty),*) -> $ret {
            tracing::debug!(command = stringify!($name), "command invoked");
            $body
        }

        $crate::__aidog_http_command!(async ok_json, [$(#[$meta])*] $name ($($arg : $ty),*));
    };

    (
        $(#[$meta:meta])*
        pub fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> $ret:ty $body:block
    ) => {
        $(#[$meta])*
        #[cfg_attr(feature = "desktop", tauri::command)]
        #[tracing::instrument(skip_all, fields(trace_id = %$crate::logging::new_trace_id()))]
        pub fn $name($($arg: $ty),*) -> $ret {
            tracing::debug!(command = stringify!($name), "command invoked");
            $body
        }

        $crate::__aidog_http_command!(sync ok_json, [$(#[$meta])*] $name ($($arg : $ty),*));
    };
}

/// [`tauri_command!`] 的 HTTP 形态展开（内部用，不要直接调）。
///
/// `$conv` 是 [`crate::http_command`] 里的归一函数（`string_err` / `typed_err` / `ok_json`），
/// 由外层分支按返回类型选定；`async` / `sync` 决定要不要 `.await`。
/// 模块体里 `use super::*` 是为了让参数类型 `$ty`（写在命令所在模块里的裸路径）能解析。
#[macro_export]
#[doc(hidden)]
macro_rules! __aidog_http_command {
    (async $conv:ident, [$(#[$meta:meta])*] $name:ident ($($arg:ident : $ty:ty),*)) => {
        #[cfg(feature = "http")]
        $(#[$meta])*
        #[doc(hidden)]
        pub mod $name {
            #[allow(unused_imports)]
            use super::*;

            /// axum handler：body 即 `invoke` 的 args 对象，响应体即命令返回值的 JSON。
            pub async fn http(
                $crate::__axum::extract::Json(__body): $crate::__axum::extract::Json<$crate::__serde_json::Value>,
            ) -> $crate::http_command::HttpCommandResponse {
                $(
                    let $arg: $ty = $crate::http_command::extract_arg(&__body, stringify!($arg))?;
                )*
                $crate::http_command::$conv(super::$name($($arg),*).await)
            }
        }
    };

    (sync $conv:ident, [$(#[$meta:meta])*] $name:ident ($($arg:ident : $ty:ty),*)) => {
        #[cfg(feature = "http")]
        $(#[$meta])*
        #[doc(hidden)]
        pub mod $name {
            #[allow(unused_imports)]
            use super::*;

            /// axum handler：body 即 `invoke` 的 args 对象，响应体即命令返回值的 JSON。
            pub async fn http(
                $crate::__axum::extract::Json(__body): $crate::__axum::extract::Json<$crate::__serde_json::Value>,
            ) -> $crate::http_command::HttpCommandResponse {
                $(
                    let $arg: $ty = $crate::http_command::extract_arg(&__body, stringify!($arg))?;
                )*
                $crate::http_command::$conv(super::$name($($arg),*))
            }
        }
    };
}
