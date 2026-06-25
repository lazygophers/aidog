use super::super::super::db;
use super::super::super::models::*;
use super::super::select_platform;

async fn mk_test_db() -> db::Db {
    let db = db::Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    db
}

async fn mk_platform(db: &db::Db, name: &str, models: Option<PlatformModels>) -> Platform {
    db::create_platform(
        db,
        CreatePlatform {
            name: name.into(),
            platform_type: Protocol::Anthropic,
            base_url: "https://example.invalid".into(),
            api_key: "k".into(),
            extra: String::new(),
            models,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None, default_level_priority: None,
        },
    )
    .await
    .expect("create platform")
}

async fn mk_group(
    db: &db::Db,
    name: &str,
    mode: RoutingMode,
    platform_ids: &[u64],
    mappings: Vec<ModelMapping>,
) -> Group {
    let g = db::create_group(
        db,
        CreateGroup {
            name: name.into(),
            group_key: Some(name.into()),
            routing_mode: mode,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2,
            model_mappings: mappings,
        },
    )
    .await
    .expect("create group");
    let inputs: Vec<GroupPlatformInput> = platform_ids
        .iter()
        .enumerate()
        .map(|(i, &pid)| GroupPlatformInput {
            platform_id: pid,
            priority: Some(i as i32),
            weight: Some(1),
            level_priority: None,
        })
        .collect();
    db::set_group_platforms(db, g.id, &inputs).await.expect("set group platforms");
    // reload group to pick up model_mappings + id
    db::get_group(db, g.id).await.expect("get group").expect("group exists")
}

#[tokio::test]
async fn explicit_mapping_selects_target_platform() {
    let db = mk_test_db().await;
    let p1 = mk_platform(&db, "P1", None).await;
    let p2 = mk_platform(&db, "P2", None).await;
    let mapping = ModelMapping {
        source_model: "claude".into(),
        target_platform_id: p2.id,
        target_model: "glm-4".into(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
    };
    let g = mk_group(&db, "g", RoutingMode::Failover, &[p1.id, p2.id], vec![mapping]).await;
    let r = select_platform(&db, &g, "claude").await.expect("route");
    assert_eq!(r.platform.id, p2.id);
    assert_eq!(r.target_model, "glm-4");
    assert!(r.mapping.is_some());
}

#[tokio::test]
async fn mapping_target_not_in_group_falls_back() {
    let db = mk_test_db().await;
    let p1 = mk_platform(&db, "P1", None).await;
    let mapping = ModelMapping {
        source_model: "claude".into(),
        target_platform_id: 9999, // not in group
        target_model: "x".into(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
    };
    let g = mk_group(&db, "g", RoutingMode::Failover, &[p1.id], vec![mapping]).await;
    let r = select_platform(&db, &g, "claude").await.expect("route");
    assert_eq!(r.platform.id, p1.id);
}

#[tokio::test]
async fn no_mapping_auto_matches_model_via_failover() {
    let db = mk_test_db().await;
    let models = PlatformModels {
        default: Some("glm-4-plus".into()),
        sonnet: None,
        opus: None,
        haiku: None,
        gpt: None,
    };
    let p1 = mk_platform(&db, "P1", Some(models)).await;
    let g = mk_group(&db, "g", RoutingMode::Failover, &[p1.id], vec![]).await;
    let r = select_platform(&db, &g, "claude").await.expect("route");
    assert_eq!(r.platform.id, p1.id);
    assert!(r.mapping.is_none());
}

#[tokio::test]
async fn load_balance_mode_selects_enabled() {
    let db = mk_test_db().await;
    let p1 = mk_platform(&db, "P1", None).await;
    let p2 = mk_platform(&db, "P2", None).await;
    let g = mk_group(&db, "g", RoutingMode::LoadBalance, &[p1.id, p2.id], vec![]).await;
    let r = select_platform(&db, &g, "anything").await.expect("route");
    assert!(r.platform.id == p1.id || r.platform.id == p2.id);
}

#[tokio::test]
async fn empty_group_errs() {
    let db = mk_test_db().await;
    let g = mk_group(&db, "g", RoutingMode::Failover, &[], vec![]).await;
    assert!(select_platform(&db, &g, "x").await.is_err());
}
