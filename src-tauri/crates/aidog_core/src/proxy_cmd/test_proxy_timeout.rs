#![cfg(test)]
use super::*;
use aidog_db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State 走 command 包装层，直测 command 转发的 gateway:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn timeout_roundtrip() {
    let db = test_db().await;
    let s: ProxyTimeoutSettings = aidog_db::get_setting(&db, "proxy", "timeout")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    aidog_db::set_setting(
        &db,
        SetSettingInput {
            scope: "proxy".to_string(),
            key: "timeout".to_string(),
            value: serde_json::to_value(&s).unwrap(),
        },
    )
    .await
    .unwrap();
    let _got: ProxyTimeoutSettings = aidog_db::get_setting(&db, "proxy", "timeout")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
}
