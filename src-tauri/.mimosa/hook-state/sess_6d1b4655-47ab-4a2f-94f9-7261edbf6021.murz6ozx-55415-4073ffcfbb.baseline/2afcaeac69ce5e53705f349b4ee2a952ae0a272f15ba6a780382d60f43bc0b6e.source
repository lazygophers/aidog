#![cfg(test)]
use super::*;
use aidog_db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 转发的 db:: / gateway:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn settings_inbox_clear() {
    let db = test_db().await;

    let s = aidog_db::get_notification_settings(&db).await;
    aidog_db::set_setting(
        &db,
        SetSettingInput {
            scope: "notification".to_string(),
            key: "settings".to_string(),
            value: serde_json::to_value(&s).unwrap(),
        },
    )
    .await
    .unwrap();

    assert!(
        aidog_db::list_notifications(&db, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        aidog_db::list_notifications(&db, 100)
            .await
            .unwrap()
            .is_empty()
    );
    aidog_db::clear_notifications(&db).await.unwrap();
}
