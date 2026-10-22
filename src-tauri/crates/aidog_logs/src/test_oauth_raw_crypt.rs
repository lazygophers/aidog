//! db-obs-encryption 票 01：OAuth 观测表 raw 列写入加密验收——经写入层落库后直读主库
//! aidog.db 断言 `enc1:` 密文形态 + `crypt::decrypt_field` 还原明文（raw 列零读侧消费者，
//! 读回一致性走本测试，不为读侧新造 API）。全走 three_file_db（文件路径构造，错路由
//! 当场 no such table）+ ensure_test_master_key，不触真 keychain。
use super::*;
use aidog_db::test_support::{ensure_test_master_key, three_file_db};

const RAW: &str = r#"{"email":"u@example.com","account_uuid":"acct-42"}"#;

/// 经同一 Db 句柄读主库（db-sqlcipher-full 后 aidog.db 整库加密，裸 Connection::open
/// 直接 NotADatabase；走 Db 的读池连接自动带 key）。
async fn raw_col(db: &aidog_db::Db, sql: &str) -> String {
    let sql = sql.to_string();
    db.call_read_traced(None, std::panic::Location::caller(), move |conn| {
        Ok(conn.query_row(&sql, [], |r| r.get::<_, String>(0))?)
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn oauth_raw_stored_encrypted_and_roundtrips() {
    ensure_test_master_key();
    let (db, _dir) = three_file_db().await;
    insert_oauth_usage_sample(&db, "cc", 11.0, 22.0, RAW)
        .await
        .unwrap();
    upsert_cc_oauth_profile(&db, "cc", "default_claude_max_20x", RAW)
        .await
        .unwrap();

    let sample_raw = raw_col(
        &db,
        "SELECT raw FROM oauth_usage_sample WHERE group_name = 'cc' ORDER BY sampled_at DESC LIMIT 1",
    )
    .await;
    let profile_raw = raw_col(
        &db,
        "SELECT raw FROM cc_oauth_profile WHERE group_name = 'cc'",
    )
    .await;
    for (name, raw) in [("sample", &sample_raw), ("profile", &profile_raw)] {
        assert!(raw.starts_with(aidog_db::crypt::ENC_PREFIX), "{name} 列值须密文形态: {raw}");
        assert!(!raw.contains("u@example.com"), "{name} 列值不得含明文 PII");
    }
    assert_eq!(aidog_db::crypt::decrypt_field(&sample_raw).unwrap(), RAW);
    assert_eq!(aidog_db::crypt::decrypt_field(&profile_raw).unwrap(), RAW);
}

#[tokio::test]
async fn empty_raw_stays_empty_and_upsert_rewrites() {
    ensure_test_master_key();
    let (db, _dir) = three_file_db().await;
    // 空串保留空（encrypt_field 内置短路，无密可加）
    insert_oauth_usage_sample(&db, "cc", 1.0, 2.0, "")
        .await
        .unwrap();
    assert_eq!(
        raw_col(&db, "SELECT raw FROM oauth_usage_sample LIMIT 1").await,
        ""
    );

    // latest-wins upsert：新明文覆盖旧密文后仍是单行密文
    upsert_cc_oauth_profile(&db, "cc", "t1", RAW).await.unwrap();
    upsert_cc_oauth_profile(&db, "cc", "t2", RAW).await.unwrap();
    let tier_raw = raw_col(
        &db,
        "SELECT tier || '|' || raw FROM cc_oauth_profile WHERE group_name = 'cc'",
    )
    .await;
    let Some(cipher) = tier_raw.strip_prefix("t2|") else {
        panic!("upsert 后须 latest-wins 单行 t2: {tier_raw}");
    };
    assert!(cipher.starts_with(aidog_db::crypt::ENC_PREFIX), "覆盖后须仍密文: {cipher}");
}
