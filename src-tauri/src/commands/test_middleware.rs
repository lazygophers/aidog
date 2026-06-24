#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db_and_engine;
use tauri::Manager;

fn create_payload(name: &str) -> CreateMiddlewareRule {
    serde_json::from_value(serde_json::json!({
        "name": name,
        "rule_type": "sensitive_word",
        "pattern": "x",
        "config": "{}"
    }))
    .expect("deserialize CreateMiddlewareRule")
}

#[tokio::test]
async fn rules_crud_and_settings() {
    let app = mock_app_with_db_and_engine().await;
    let db = app.state::<Db>();
    let engine = app.state::<Arc<MiddlewareEngine>>();

    let base = middleware_list_rules(db.clone()).await.unwrap().len();

    let rule = middleware_create_rule(create_payload("r1"), db.clone(), engine.clone()).await.unwrap();
    assert_eq!(middleware_list_rules(db.clone()).await.unwrap().len(), base + 1);

    let upd: UpdateMiddlewareRule = serde_json::from_value(serde_json::json!({
        "id": rule.id,
        "name": "r1-renamed",
        "rule_type": "sensitive_word"
    }))
    .unwrap();
    middleware_update_rule(upd, db.clone(), engine.clone()).await.unwrap();

    middleware_delete_rule(rule.id, db.clone(), engine.clone()).await.unwrap();
    assert_eq!(middleware_list_rules(db.clone()).await.unwrap().len(), base);

    // settings roundtrip
    let s = middleware_settings_get(db.clone()).await.unwrap();
    middleware_settings_set(db.clone(), s).await.unwrap();
}
