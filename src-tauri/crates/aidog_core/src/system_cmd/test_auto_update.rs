#![cfg(test)]
use super::*;
use crate::gateway::db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 转发的 db:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn defaults_true_when_unset() {
    let db = test_db().await;
    let v = load_auto_update_enabled(&db).await;
    assert!(v, "auto_update_enabled defaults true when missing");
}

#[tokio::test]
async fn roundtrip_persists() {
    let db = test_db().await;
    db::set_setting(&db, SetSettingInput {
        scope: "app".into(),
        key: "auto_update_enabled".into(),
        value: serde_json::Value::Bool(false),
    }).await.unwrap();
    assert!(!load_auto_update_enabled(&db).await, "false persists");
    db::set_setting(&db, SetSettingInput {
        scope: "app".into(),
        key: "auto_update_enabled".into(),
        value: serde_json::Value::Bool(true),
    }).await.unwrap();
    assert!(load_auto_update_enabled(&db).await, "true persists");
}

#[tokio::test]
async fn corrupt_value_falls_back_to_true() {
    let db = test_db().await;
    // 直接写非 bool JSON，load 路径应兜底 true
    db::set_setting(&db, SetSettingInput {
        scope: "app".into(),
        key: "auto_update_enabled".into(),
        value: serde_json::Value::String("garbage".into()),
    }).await.unwrap();
    assert!(load_auto_update_enabled(&db).await, "non-bool falls back to true");
}
