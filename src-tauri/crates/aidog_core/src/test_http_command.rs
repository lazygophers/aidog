//! 票 07：HTTP 展开与 Tauri 展开的等价性。
//!
//! **等价的判据**：Tauri 侧 `invoke` 拿到的 JSON = 命令返回值经 serde 序列化的结果
//! （`#[tauri::command]` 除了包一层 IPC 分发，不改返回值），reject 值 = `Err` 经 serde
//! 序列化的结果。所以「HTTP 展开与 Tauri 展开等价」可判定为：
//! `<cmd>::http(Json(args)).await` == `serde_json::to_value(<cmd>(args).await)`，
//! 且 `Err` 落成非 2xx + 同样的 JSON body。下面对**真实命令**（覆盖三种返回形态里的两种）
//! 与**本文件内用同一个宏定义的命令**（补齐结构化错误 / 非 Result / 多词参数）各测一遍。
use super::*;
use axum::Json;
use serde_json::json;

// ── 宏的 6 条分支各来一个样本（真实命令覆盖不到的形态在这里补齐）─────────────

#[derive(serde::Serialize, PartialEq, Debug)]
pub struct T07Echo {
    host_pattern: String,
    retry_count: Option<u32>,
}

#[derive(serde::Serialize, Debug)]
pub struct T07Err {
    kind: String,
    code: i32,
}

crate::tauri_command! {
    /// 多词参数：验证 camelCase（Tauri/前端形态）与 snake_case 两种键都取得到。
    pub async fn t07_echo(host_pattern: String, retry_count: Option<u32>) -> Result<T07Echo, String> {
        Ok(T07Echo { host_pattern, retry_count })
    }
}

crate::tauri_command! {
    /// 结构化错误分支（同 `proxy_start` 的 `Result<_, ProxyStartError>`）。
    pub async fn t07_typed(fail: bool) -> Result<i32, T07Err> {
        if fail {
            Err(T07Err { kind: "boom".to_string(), code: 7 })
        } else {
            Ok(7)
        }
    }
}

crate::tauri_command! {
    /// 非 Result 分支（同 `cli_check_versions` 的 `Vec<CliToolStatus>`）。
    pub fn t07_plain(seed: i64) -> Vec<i64> {
        vec![seed, seed + 1]
    }
}

// ── 参数名映射 ───────────────────────────────────────────────────

#[test]
fn lower_camel_case_matches_tauri_convention() {
    assert_eq!(lower_camel_case("url"), "url");
    assert_eq!(lower_camel_case("host_pattern"), "hostPattern");
    assert_eq!(lower_camel_case("from_group_id"), "fromGroupId");
    assert_eq!(
        lower_camel_case("apply_to_claude_plugin"),
        "applyToClaudePlugin"
    );
}

#[tokio::test]
async fn args_accept_camel_case_and_snake_case() {
    let expected =
        serde_json::to_value(t07_echo("a.com".to_string(), Some(3)).await.unwrap()).unwrap();

    let Json(camel) = t07_echo::http(Json(json!({ "hostPattern": "a.com", "retryCount": 3 })))
        .await
        .unwrap();
    let Json(snake) = t07_echo::http(Json(json!({ "host_pattern": "a.com", "retry_count": 3 })))
        .await
        .unwrap();

    assert_eq!(camel, expected);
    assert_eq!(snake, expected);
}

#[tokio::test]
async fn missing_optional_arg_becomes_none() {
    let Json(got) = t07_echo::http(Json(json!({ "hostPattern": "a.com" })))
        .await
        .unwrap();
    assert_eq!(
        got,
        serde_json::to_value(t07_echo("a.com".to_string(), None).await.unwrap()).unwrap()
    );
}

#[tokio::test]
async fn missing_required_arg_is_400() {
    let err = t07_echo::http(Json(json!({}))).await.err().unwrap();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(err.body.as_str().unwrap().contains("host_pattern"));
}

#[tokio::test]
async fn wrong_arg_type_is_400() {
    let err = t07_echo::http(Json(json!({ "hostPattern": 12 })))
        .await
        .err()
        .unwrap();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

// ── 返回值三形态 ─────────────────────────────────────────────────

#[tokio::test]
async fn typed_error_arm_matches_direct_call() {
    let Json(ok) = t07_typed::http(Json(json!({ "fail": false })))
        .await
        .unwrap();
    assert_eq!(
        ok,
        serde_json::to_value(t07_typed(false).await.unwrap()).unwrap()
    );

    let err = t07_typed::http(Json(json!({ "fail": true })))
        .await
        .err()
        .unwrap();
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        err.body,
        serde_json::to_value(t07_typed(true).await.unwrap_err()).unwrap()
    );
}

#[tokio::test]
async fn plain_return_arm_matches_direct_call() {
    let Json(got) = t07_plain::http(Json(json!({ "seed": 41 }))).await.unwrap();
    assert_eq!(got, serde_json::to_value(t07_plain(41)).unwrap());
    assert_eq!(got, json!([41, 42]));
}

// ── 真实命令抽样（无需 AppCtx / DB 的三个）───────────────────────────

#[tokio::test]
async fn about_info_http_matches_direct_call() {
    let Json(got) = crate::system_cmd::about::about_info::http(Json(json!({})))
        .await
        .unwrap();
    assert_eq!(
        got,
        serde_json::to_value(crate::system_cmd::about::about_info()).unwrap()
    );
    assert!(got.get("app_version").is_some());
}

#[tokio::test]
async fn mitm_classify_trust_error_http_matches_direct_call() {
    use crate::proxy_cmd::mitm::mitm_classify_trust_error;

    let args = json!({ "name": "security", "stderr": "user canceled" });
    let Json(got) = mitm_classify_trust_error::http(Json(args)).await.unwrap();
    assert_eq!(
        got,
        serde_json::to_value(
            mitm_classify_trust_error("security".to_string(), None, "user canceled".to_string())
                .await
                .unwrap()
        )
        .unwrap()
    );
}

#[tokio::test]
async fn platform_share_parse_http_matches_direct_call() {
    use crate::platform_cmd::platform::platform_share_parse;

    // 失败路径：reject 值（错误字符串）原样进 body，状态码 500。
    let err = platform_share_parse::http(Json(json!({ "text": "not yaml: [" })))
        .await
        .err()
        .unwrap();
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        err.body,
        json!(
            platform_share_parse("not yaml: [".to_string())
                .await
                .unwrap_err()
        )
    );
}
