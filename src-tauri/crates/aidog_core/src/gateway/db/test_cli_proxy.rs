#![cfg(test)]
use super::test_support::test_db;
use super::*;
use crate::gateway::models::CreateCliProxyProvider;

fn input(name: &str) -> CreateCliProxyProvider {
    CreateCliProxyProvider {
        name: name.to_string(),
        wire_protocol: "anthropic".into(),
        base_url: "https://api.x.com/v1".into(),
        api_key: "sk-test".into(),
        models: vec!["claude-sonnet-4".into()],
        extra: String::new(),
        status: "active".into(),
        group_id: None,
    }
}

#[tokio::test]
async fn cli_proxy_provider_crud_roundtrip() {
    let db = test_db().await;

    // empty
    assert!(list_cli_proxy_providers(&db).await.unwrap().is_empty());
    assert!(get_cli_proxy_provider(&db, 1).await.unwrap().is_none());

    // create
    let created = create_cli_proxy_provider(&db, input("p1")).await.unwrap();
    assert_eq!(created.name, "p1");
    assert_eq!(created.models, vec!["claude-sonnet-4".to_string()]);
    assert_eq!(created.status, "active");
    assert!(created.group_id.is_none());
    assert!(created.id > 0);
    let id = created.id;

    // get
    let got = get_cli_proxy_provider(&db, id).await.unwrap().unwrap();
    assert_eq!(got.id, id);
    assert_eq!(got.wire_protocol, "anthropic");
    assert_eq!(got.base_url, "https://api.x.com/v1");

    // list
    create_cli_proxy_provider(&db, input("p2")).await.unwrap();
    let all = list_cli_proxy_providers(&db).await.unwrap();
    assert_eq!(all.len(), 2);

    // update（全量覆写）
    let mut upd = input("p1-renamed");
    upd.wire_protocol = "openai".into();
    upd.models = vec!["gpt-4".into(), "gpt-5".into()];
    upd.group_id = Some(42);
    upd.status = "disabled".into();
    upd.extra = "{\"k\":\"v\"}".into();
    let updated = update_cli_proxy_provider(&db, id, upd).await.unwrap().unwrap();
    assert_eq!(updated.name, "p1-renamed");
    assert_eq!(updated.wire_protocol, "openai");
    assert_eq!(updated.models.len(), 2);
    assert_eq!(updated.group_id, Some(42));
    assert_eq!(updated.status, "disabled");
    assert_eq!(updated.extra, "{\"k\":\"v\"}");

    // update 不存在 → None
    assert!(update_cli_proxy_provider(&db, 9999, input("x")).await.unwrap().is_none());

    // delete
    assert!(delete_cli_proxy_provider(&db, id).await.unwrap());
    assert!(get_cli_proxy_provider(&db, id).await.unwrap().is_none());
    assert!(!delete_cli_proxy_provider(&db, id).await.unwrap()); // 再删返 false
    assert_eq!(list_cli_proxy_providers(&db).await.unwrap().len(), 1);
}

/// 验 migration 幂等：init_tables 重复跑不报错（CREATE TABLE IF NOT EXISTS）。
#[tokio::test]
async fn migration_045_idempotent() {
    let db = test_db().await;
    // 再跑一次 init —— 内部走完整 migration 流（含 045），不报错
    db.init_tables().await.expect("re-init must be idempotent");
    // 表仍可用
    create_cli_proxy_provider(&db, input("after-reinit"))
        .await
        .expect("table still usable after re-init");
}
