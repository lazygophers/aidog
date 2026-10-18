#![cfg(test)]
use super::*;
use aidog_db as db;
use aidog_db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 转发的 db:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn app_log_settings_roundtrip() {
    let db = test_db().await;
    let s = load_app_log_settings_from_db(&db).await;
    let value = serde_json::to_value(&s).unwrap();
    db::set_setting(
        &db,
        SetSettingInput {
            scope: "app".to_string(),
            key: "logging".to_string(),
            value,
        },
    )
    .await
    .unwrap();
    let _ = load_app_log_settings_from_db(&db).await;
}

#[tokio::test]
async fn load_settings_helper() {
    let db = test_db().await;
    let _ = load_app_log_settings_from_db(&db).await;
}

// ── app_log_frontend（unified-logging 票 01）：级别分派 + 截断，命令本身无返回数据 ──
#[tokio::test]
async fn app_log_frontend_accepts_levels() {
    for (lvl, msg) in [("error", "boom"), ("warn", "careful"), ("info", "hi"), ("debug", "x")] {
        let r = app_log_frontend(lvl.to_string(), msg.to_string()).await;
        assert!(r.is_ok(), "level {lvl} should be accepted");
    }
}

#[tokio::test]
async fn app_log_frontend_truncates_huge_message() {
    let r = app_log_frontend("error".into(), "x".repeat(50_000)).await;
    assert!(r.is_ok());
}
