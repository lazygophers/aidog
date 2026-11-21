//! 设置子页「客户端会话读取」的读写往返：保留天数 + 允许读取对话原文。
use super::*;
use aidog_db::test_support::three_file_db;

#[tokio::test]
async fn transcript_consent_defaults_false_and_roundtrips() {
    let (db, _dir) = three_file_db().await;
    assert!(!load_transcript_consent(&db).await, "缺省 false");
    save_transcript_consent(&db, true).await.unwrap();
    assert!(load_transcript_consent(&db).await, "写 true 读回 true");
    let raw = aidog_db::get_setting(&db, "client_usage", "transcript_consent").await.unwrap();
    assert_eq!(raw, Some(serde_json::json!(true)), "存 client_usage/transcript_consent");
    save_transcript_consent(&db, false).await.unwrap();
    assert!(!load_transcript_consent(&db).await);
}

#[tokio::test]
async fn transcript_consent_does_not_touch_read_gate() {
    let (db, _dir) = three_file_db().await;
    save_transcript_consent(&db, true).await.unwrap();
    let c = load_consent(&db).await;
    assert!(SUPPORTED_CLIENTS.iter().all(|code| !consent_allows(&c, code)), "原文开关不改读取闸门");
}

#[tokio::test]
async fn retention_defaults_to_90_and_roundtrips() {
    let (db, _dir) = three_file_db().await;
    assert_eq!(aidog_db::client_usage::load_retention_days(&db).await, 90);
    aidog_db::client_usage::save_retention_days(&db, 0).await.unwrap();
    assert_eq!(aidog_db::client_usage::load_retention_days(&db).await, 0, "0 = 永久");
}
