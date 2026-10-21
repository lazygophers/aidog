//! registry 灌种覆盖：首种全量、幂等空转、prune 上一版残留行、种后计费走镜像表。
use super::*;
use aidog_db::test_support::test_db;

#[tokio::test]
async fn seed_fresh_db_fills_all_bundled_rows() {
    let db = test_db().await;
    let r = seed_registry_from_bundled(&db).await.unwrap();
    let want_platforms = aidog_db::registry::bundled_platform_files().len() as u32;
    let want_models = aidog_db::registry::bundled_model_files().len() as u32;
    assert_eq!(r.added, want_platforms + want_models, "首种全为 added");
    assert_eq!(r.failed, 0, "内置文件必须全部解析成功: {:?}", r.failures);
    let entries = aidog_db::select_model_entries(&db, None).await.unwrap();
    assert_eq!(entries.len() as u32, want_models);
}

#[tokio::test]
async fn seed_is_idempotent_second_run_no_writes() {
    let db = test_db().await;
    seed_registry_from_bundled(&db).await.unwrap();
    let r2 = seed_registry_from_bundled(&db).await.unwrap();
    assert_eq!(r2.added, 0, "二次种不得新增");
    assert_eq!(r2.updated, 0, "二次种不得改写");
    assert_eq!(r2.unchanged, r2.total, "全部 unchanged");
}

#[tokio::test]
async fn seed_prunes_stale_rows_from_older_binary() {
    let db = test_db().await;
    seed_registry_from_bundled(&db).await.unwrap();
    // 模拟上一版二进制残留：塞一行不在内置清单的镜像行
    let stale = ModelEntry {
        platform_code: "stale_platform".into(),
        model_id: "stale-model".into(),
        display_name: "stale-model".into(),
        canonical_model: "stale-model".into(),
        family: "stale".into(),
        version: String::new(),
        predecessor: String::new(),
        capabilities: vec!["text".into()],
        builtin_tools_excluded: vec![],
        max_input_tokens: None,
        max_output_tokens: None,
        context_window: None,
        official: false,
        price_data: r#"{"model_id":"stale-model"}"#.into(),
        updated_at: 0,
    };
    let write_failures = aidog_db::upsert_model_entries_best_effort(&db, vec![stale]).await.unwrap().1;
    assert!(write_failures.is_empty(), "写入残留行失败: {write_failures:?}");

    let r = seed_registry_from_bundled(&db).await.unwrap();
    assert_eq!(r.added, 0);
    assert_eq!(r.updated, 0);
    let entries = aidog_db::select_model_entries(&db, None).await.unwrap();
    assert!(
        entries.iter().all(|e| e.platform_code != "stale_platform"),
        "残留行必须被 prune"
    );
}

#[tokio::test]
async fn seeded_row_feeds_resolve_price() {
    let db = test_db().await;
    seed_registry_from_bundled(&db).await.unwrap();
    let settings = get_sync_settings(&db).await;
    let r = aidog_db::resolve_price(
        &db,
        "anthropic",
        "claude-opus-4-6",
        settings.fallback_input_price,
        settings.fallback_output_price,
        0,
        0,
        false,
    )
    .await
    .unwrap();
    assert_eq!(r.price.source, "model_entry");
    assert!(r.price.input_cost_per_token > 0.0);
}
