//! `tauri_command!` 的 HTTP（axum）形态支撑（票 07）。
//!
//! 语义对齐 `invoke<T>(name, args)`：请求 JSON body 就是 `args` 对象，命令返回值序列化成
//! 响应 JSON body。三条对齐点：
//!
//! 1. **参数名**：Tauri v2 默认把 Rust 的 snake_case 参数名转成 lowerCamelCase 去 JS 侧 args
//!    里取（`tauri-macros` 的 `ArgumentCase::Camel`，用 heck 的 `to_lower_camel_case`），前端
//!    也确实是这么发的（如 `group_platform_move` 传 `{ platformId, fromGroupId, toGroupId }`）。
//!    这里同样先按 camelCase 取，取不到再按原 snake_case 取（后者是给 Rust 侧调用方的方便，
//!    Tauri 不认，但多认一种键不会改变前端行为）。
//! 2. **缺参**：键不存在 = `null`，交给 serde 决定——`Option<T>` 得 `None`，非 Option 报错。
//!    与 Tauri 的 `CommandArg::from_command` 行为一致。
//! 3. **错误**：Tauri 的 `Err(e)` 是 reject，reject 值是 `e` 的 JSON。这里落成非 2xx 响应，
//!    body 就是 `e` 的 JSON（`Result<_, String>` 即一个 JSON 字符串）。参数解析失败 400，
//!    命令自身失败 500。
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// 宏生成的 axum handler 的返回类型：Ok = 200 + 返回值 JSON，Err = 状态码 + 错误 JSON。
pub type HttpCommandResponse = Result<Json<Value>, CommandError>;

/// 命令的失败响应（= `invoke` 的 reject）。
#[derive(Debug)]
pub struct CommandError {
    pub status: StatusCode,
    /// reject 值的 JSON 形态（`Result<_, String>` 命令就是一个 JSON 字符串）。
    pub body: Value,
}

impl IntoResponse for CommandError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

/// snake_case → lowerCamelCase。命令参数名全是纯 snake_case（无数字、无前导下划线，
/// 实测 77 个参数名全部满足），故按 `_` 切段 + 首字母大写即等价于 heck 的实现。
fn lower_camel_case(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut upper_next = false;
    for ch in snake.chars() {
        if ch == '_' {
            upper_next = !out.is_empty();
            continue;
        }
        if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// 从 args 对象里取一个命令参数。
pub fn extract_arg<T: DeserializeOwned>(body: &Value, name: &str) -> Result<T, CommandError> {
    let camel = lower_camel_case(name);
    let raw = body
        .get(camel.as_str())
        .or_else(|| body.get(name))
        .cloned()
        .unwrap_or(Value::Null);
    serde_json::from_value(raw).map_err(|e| CommandError {
        status: StatusCode::BAD_REQUEST,
        body: Value::String(format!("invalid args `{name}`: {e}")),
    })
}

/// 返回值直出（命令返回类型不是 `Result`）。
pub fn ok_json<T: Serialize>(value: T) -> HttpCommandResponse {
    serde_json::to_value(value)
        .map(Json)
        .map_err(|e| CommandError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: Value::String(format!("failed to serialize command result: {e}")),
        })
}

/// `Result<_, String>` 命令：Err 直接当错误消息。
pub fn string_err<T: Serialize>(result: Result<T, String>) -> HttpCommandResponse {
    match result {
        Ok(v) => ok_json(v),
        Err(e) => Err(CommandError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: Value::String(e),
        }),
    }
}

/// `Result<_, E>` 命令（E 为结构化错误，如 `ProxyStartError`）：Err 序列化成 JSON。
pub fn typed_err<T: Serialize, E: Serialize>(result: Result<T, E>) -> HttpCommandResponse {
    match result {
        Ok(v) => ok_json(v),
        Err(e) => Err(CommandError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: serde_json::to_value(e)
                .unwrap_or_else(|err| Value::String(format!("failed to serialize error: {err}"))),
        }),
    }
}

#[cfg(test)]
#[path = "test_http_command.rs"]
mod test_http_command;
