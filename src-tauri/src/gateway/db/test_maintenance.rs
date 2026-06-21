#![cfg(test)]
use super::*;
use super::test_support::*;

    /// migrate_auto_vacuum 幂等：内存库 Db::new 建表前已设 auto_vacuum=INCREMENTAL(2)，
    /// 故迁移探测命中 `current == 2` 分支 → 置标记 + 返回 false（无 VACUUM 必要）。
    /// 第二次跑因标记为 true 直接跳过。验证标记持久 + 探测后跳过两条幂等路径。
    #[tokio::test]
    async fn migrate_auto_vacuum_is_idempotent() {
        let db = test_db().await;
        // 迁移前：标记未置
        let flag_before = get_setting(&db, "db", "compact_migrated_v1").await.unwrap();
        assert!(flag_before.is_none(), "flag should be absent before migration");

        // 第一次迁移：auto_vacuum 已是 INCREMENTAL（新库建表前设过）→ 置标记 + 返回 false
        let migrated = migrate_auto_vacuum(&db).await.expect("first migration");
        assert!(!migrated, "memory db already INCREMENTAL, no VACUUM needed");

        // 标记已置
        let flag = get_setting(&db, "db", "compact_migrated_v1").await.unwrap();
        assert_eq!(flag, Some(serde_json::Value::Bool(true)));

        // 第二次迁移：标记 true → 直接跳过，不探测不 VACUUM
        let migrated2 = migrate_auto_vacuum(&db).await.expect("second migration");
        assert!(!migrated2, "second call should skip (already marked)");

        // auto_vacuum 保持 INCREMENTAL
        let av: i64 = db
            
            .call_traced(None, std::panic::Location::caller(), |c| Ok(c.query_row("PRAGMA auto_vacuum", [], |r| r.get(0))?))
            .await
            .unwrap();
        assert_eq!(av, 2, "auto_vacuum should be INCREMENTAL");
    }



    /// compact_database 全量 VACUUM 返回 before/after 字节，after <= before（压缩单调非增）。
    #[tokio::test]
    async fn compact_database_returns_sizes() {
        let db = test_db().await;
        // 插入若干行 + 删除一部分，制造 free pages
        for i in 0..50 {
            insert_proxy_log_at(&db, chrono::Utc::now().timestamp_millis() + i).await;
        }
        db
            .call_traced(None, std::panic::Location::caller(), |conn| {
                conn.execute("DELETE FROM proxy_log WHERE id LIKE 'test-%'", [])?;
                Ok(())
            })
            .await
            .unwrap();
        let result = compact_database(&db).await.expect("compact");
        assert!(result.before_bytes > 0, "before_bytes should be positive");
        assert!(result.after_bytes <= result.before_bytes, "VACUUM should not grow db");
    }
