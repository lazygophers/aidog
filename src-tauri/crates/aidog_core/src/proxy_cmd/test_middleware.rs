#![cfg(test)]
use super::*;
use aidog_db as db;
use aidog_db::test_support::test_db;

// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
// 故不经 tauri::State 走 command 包装层，直测 command 转发的 db:: 函数
// （command 本身只是薄转发 + tracing + engine.reload，逻辑等价）。
// MiddlewareEngine::new() 0 参构造，reload(&db) 不依赖 tauri State。

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
    let db = test_db().await;
    let engine = MiddlewareEngine::new();

    let base = db::list_middleware_rules(&db).await.unwrap().len();

    let rule = db::create_middleware_rule(&db, create_payload("r1")).await.unwrap();
    engine.reload(&db).await.unwrap();
    assert_eq!(db::list_middleware_rules(&db).await.unwrap().len(), base + 1);

    let upd: UpdateMiddlewareRule = serde_json::from_value(serde_json::json!({
        "id": rule.id,
        "name": "r1-renamed",
        "rule_type": "sensitive_word"
    }))
    .unwrap();
    db::update_middleware_rule(&db, upd).await.unwrap();
    engine.reload(&db).await.unwrap();

    db::delete_middleware_rule(&db, rule.id).await.unwrap();
    engine.reload(&db).await.unwrap();
    assert_eq!(db::list_middleware_rules(&db).await.unwrap().len(), base);

    // settings roundtrip
    let s: MiddlewareSettings = aidog_db::get_setting(&db, "middleware", "settings").await
        .ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    aidog_db::set_setting(&db, SetSettingInput {
        scope: "middleware".to_string(), key: "settings".to_string(),
        value: serde_json::to_value(&s).unwrap(),
    }).await.unwrap();
}
