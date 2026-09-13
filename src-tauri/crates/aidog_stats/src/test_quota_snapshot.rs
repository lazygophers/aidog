#![cfg(test)]
use crate::*;
use aidog_db::Db;
use aidog_db::models::*;
use aidog_db::test_support::test_db;

fn q(start: Option<i64>, end: Option<i64>, platform_id: Option<u64>) -> QuotaSnapshotsQuery {
    QuotaSnapshotsQuery { start, end, platform_id }
}

#[tokio::test]
async fn insert_query_roundtrip() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    // P1 两条（不同时刻）+ P2 一条 + P1 一条窗外。
    insert_quota_snapshot(&db, 1, 10.5).await.unwrap();
    insert_quota_snapshot(&db, 2, 7.25).await.unwrap();
    insert_quota_snapshot(&db, 1, 9.0).await.unwrap();
    insert_old_snapshot(&db, 1, 88.0, now - 40 * 86_400_000).await;

    // 全部平台（30d 缺省窗盖住 now 附近三条，40d 前那条在窗外）。
    let all = quota_snapshots(&db, &q(None, None, None)).await.unwrap();
    assert_eq!(all.len(), 3, "窗外行不应返回");
    // 升序 + 逐行值往返。
    assert!(all.windows(2).all(|w| w[0].created_at <= w[1].created_at));
    let balances: Vec<(i64, f64)> = all
        .iter()
        .map(|s| (s.platform_id, s.est_balance_remaining))
        .collect();
    assert!(balances.contains(&(1, 10.5)));
    assert!(balances.contains(&(1, 9.0)));
    assert!(balances.contains(&(2, 7.25)));

    // 平台过滤：只剩 P2。
    let p2 = quota_snapshots(&db, &q(None, None, Some(2))).await.unwrap();
    assert_eq!(p2.len(), 1);
    assert_eq!(p2[0].platform_id, 2);
    assert!((p2[0].est_balance_remaining - 7.25).abs() < 1e-9);

    // 显式宽窗：40d 前那条也回来（4 条）。
    let wide = quota_snapshots(&db, &q(Some(now - 41 * 86_400_000), Some(now + 1000), None))
        .await
        .unwrap();
    assert_eq!(wide.len(), 4);
}

#[tokio::test]
async fn retention_cleanup_deletes_expired_only() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    insert_old_snapshot(&db, 1, 5.0, now - 91 * 86_400_000).await; // 过期
    insert_old_snapshot(&db, 1, 4.0, now - 89 * 86_400_000).await; // 保留
    insert_quota_snapshot(&db, 1, 3.0).await.unwrap(); // 刚写入，保留

    cleanup_quota_snapshots(&db, 90, RetentionUnit::Day).await.unwrap();
    let left = quota_snapshots(&db, &q(Some(0), Some(now + 1000), None))
        .await
        .unwrap();
    assert_eq!(left.len(), 2, "只有 91d 前那条应被清");
    assert!(left.iter().all(|s| s.est_balance_remaining != 5.0));

    // value=0（永久保留）→ 跳过清理，不报错。
    cleanup_quota_snapshots(&db, 0, RetentionUnit::Day).await.unwrap();
    let still = quota_snapshots(&db, &q(Some(0), Some(now + 1000), None))
        .await
        .unwrap();
    assert_eq!(still.len(), 2);
}

#[tokio::test]
async fn empty_platform_and_empty_window_return_empty() {
    let db = test_db().await;
    insert_quota_snapshot(&db, 1, 1.0).await.unwrap();

    // 未注册平台 id。
    let none = quota_snapshots(&db, &q(None, None, Some(999))).await.unwrap();
    assert!(none.is_empty());

    // 空窗（远古区间）。
    let empty = quota_snapshots(&db, &q(Some(1000), Some(2000), None)).await.unwrap();
    assert!(empty.is_empty());

    // 空库默认窗（另一 test_db 实例）也不报错。
    let db2 = test_db().await;
    let fresh = quota_snapshots(&db2, &q(None, None, None)).await.unwrap();
    assert!(fresh.is_empty());
}

/// 直插指定 created_at 的行（绕过 insert_quota_snapshot 的 now() 盖戳，供窗口/过期测试）。
async fn insert_old_snapshot(db: &Db, platform_id: u64, balance: f64, created_at: i64) {
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT INTO quota_snapshot (platform_id, est_balance_remaining, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![platform_id as i64, balance, created_at],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}
