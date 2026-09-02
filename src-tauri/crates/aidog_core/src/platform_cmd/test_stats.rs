#![cfg(test)]
use super::*;
use aidog_db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 转发的 db:: / gateway:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn query_and_batch() {
    let db = test_db().await;
    let q = StatsQuery {
        start: None,
        end: None,
        granularity: Some("day".into()),
        group_by: None,
        filter_group: None,
        filter_model: None,
        filter_platform: None,
    };
    let _ = aidog_stats::query_stats(&db, &q).await.unwrap();
    let batch = aidog_stats::query_stats_batch(&db, vec![q.clone(), q])
        .await
        .unwrap();
    assert_eq!(batch.len(), 2);
}

#[tokio::test]
async fn settings_roundtrip_and_rebuild() {
    let db = test_db().await;
    let s: StatsSettings = aidog_db::get_setting(&db, "stats", "settings")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    aidog_db::set_setting(
        &db,
        gateway::models::SetSettingInput {
            scope: "stats".into(),
            key: "settings".into(),
            value: serde_json::to_value(&s).unwrap(),
        },
    )
    .await
    .unwrap();
    aidog_stats::rebuild_stats_agg_from_logs(&db).await.unwrap();
}
