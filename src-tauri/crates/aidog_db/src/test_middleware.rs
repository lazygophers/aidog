#![cfg(test)]
use super::*;
use super::test_support::*;
use rusqlite::params;

    // ── Notification 收件箱 CRUD（N1）──
    #[tokio::test]
    async fn notification_inbox_crud() {
        let db = test_db().await;
        // 空库
        assert!(list_notifications(&db, 50).await.unwrap().is_empty());

        let id1 = insert_notification(&db, "task_complete", "任务完成", "项目 X 完成").await.unwrap();
        let id2 = insert_notification(&db, "error", "出错", "构建失败").await.unwrap();
        assert!(id2 > id1);

        let list = list_notifications(&db, 50).await.unwrap();
        assert_eq!(list.len(), 2);
        // 倒序：最新在前
        assert_eq!(list[0].id, id2);
        assert_eq!(list[0].notif_type, "error");
        assert_eq!(list[1].title, "任务完成");

        // limit 生效
        for i in 0..5 {
            insert_notification(&db, "task_complete", &format!("t{i}"), "b").await.unwrap();
        }
        assert_eq!(list_notifications(&db, 3).await.unwrap().len(), 3);

        // 清空
        clear_notifications(&db).await.unwrap();
        assert!(list_notifications(&db, 50).await.unwrap().is_empty());
    }



    // ── Notification retention 硬删（默认 7 天 + 不清理）──
    #[tokio::test]
    async fn cleanup_notifications_hard_deletes_old_rows() {
        let db = test_db().await;
        let now = now();
        let old = now - 100 * 24 * 3600 * 1000; // 100 天前
        let recent = now - 24 * 3600 * 1000; // 1 天前
        for (ts, title) in [(old, "old"), (recent, "recent")] {
            db
                .call_traced(None, std::panic::Location::caller(), move |conn| {
                    conn.execute(
                        "INSERT INTO notification (notif_type, title, body, created_at) VALUES ('error', ?1, '', ?2)",
                        params![title, ts],
                    )?;
                    Ok(())
                })
                .await
                .unwrap();
        }
        assert_eq!(list_notifications(&db, 50).await.unwrap().len(), 2);

        // retention=7 → 删 100 天前，留 1 天前
        cleanup_notifications(&db, 7).await.unwrap();
        let list = list_notifications(&db, 50).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "recent");

        // retention=0 → 跳过清理（永不清理）
        cleanup_notifications(&db, 0).await.unwrap();
        assert_eq!(list_notifications(&db, 50).await.unwrap().len(), 1);
    }



    #[tokio::test]
    async fn notification_settings_default_when_absent() {
        let db = test_db().await;
        let s = get_notification_settings(&db).await;
        assert!(s.enabled && s.tts_enabled);
        // 写入后读回
        set_setting(&db, SetSettingInput {
            scope: "notification".into(),
            key: "settings".into(),
            value: serde_json::json!({"enabled": false, "tts_enabled": false}),
        }).await.unwrap();
        let s2 = get_notification_settings(&db).await;
        assert!(!s2.enabled && !s2.tts_enabled);
    }
