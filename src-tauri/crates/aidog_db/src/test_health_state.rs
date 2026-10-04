#![cfg(test)]
use super::health_state::*;
use super::test_support::*;

// ── R4（routing-health-optim）：platform_health_state 持久化往返 ──

#[tokio::test]
async fn health_state_upsert_load_roundtrip() {
    let db = test_db().await;
    let row = PlatformHealthStateRow {
        platform_id: 42,
        breaker_state: "open".to_string(),
        breaker_until_ms: 1_800_000,
        quota_cooldown_until_ms: 1_700_000,
        auth_cooldown_until_ms: 0,
        balance_cooldown_until_ms: 1_650_000,
        last_connect_fail_ms: 1_000_000,
        updated_at: 900_000,
    };
    upsert_platform_health_state(&db, row.clone())
        .await
        .unwrap();
    // 二次 upsert（清除语义：整行覆盖）
    let cleared = PlatformHealthStateRow {
        platform_id: 42,
        breaker_state: "closed".to_string(),
        ..Default::default()
    };
    upsert_platform_health_state(&db, cleared).await.unwrap();
    let rows = load_platform_health_states(&db).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].platform_id, 42);
    assert_eq!(rows[0].breaker_state, "closed");
    assert_eq!(rows[0].breaker_until_ms, 0);
}

#[tokio::test]
async fn health_state_load_empty_table() {
    let db = test_db().await;
    assert!(load_platform_health_states(&db).await.unwrap().is_empty());
}
