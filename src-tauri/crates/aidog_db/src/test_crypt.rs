//! db-field-encryption 票 01 测试：加密原语 / 主密钥解析 / platform.api_key 端到端 / 存量迁移。
//! 全部走临时 key 文件（three_file_db 注入 AIDOG_DB_MASTER_KEY_FILE），不触真 keychain。
#![cfg(test)]
use crate::crypt;
use crate::test_support::*;
use crate::*;
use rusqlite::params;

const PLAIN: &str = "sk-ant-test-1234567890";

fn input(name: &str, api_key: &str) -> CreatePlatform {
    CreatePlatform {
        name: name.into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://api.example.com".into(),
        api_key: api_key.into(),
        extra: String::new(),
        models: None,
        available_models: None,
        endpoints: None,
        manual_budgets: None,
        auto_group: None,
        join_group_ids: None,
        expires_at: None,
        quota_source: None,
    }
}

/// 直接 sqlite 打开 platform.db 读 api_key 原始列值（不经行编解码层）。
fn raw_api_key(dir: &std::path::Path, id: i64) -> String {
    let conn = rusqlite::Connection::open(dir.join("platform.db")).unwrap();
    conn.query_row(
        "SELECT api_key FROM platform WHERE id = ?1",
        params![id],
        |r| r.get::<_, String>(0),
    )
    .unwrap()
}

// ── 加密原语 ─────────────────────────────────────────────────────────────

#[test]
fn encrypt_roundtrip_nonce_unique_prefix() {
    ensure_test_master_key();
    let a = crypt::encrypt_field(PLAIN).unwrap();
    let b = crypt::encrypt_field(PLAIN).unwrap();
    assert!(a.starts_with(crypt::ENC_PREFIX), "密文须带前缀标记: {a}");
    assert_ne!(a, b, "nonce 须随机：同明文两次密文不同");
    assert!(!a.contains(PLAIN), "密文不得含明文");
    assert_eq!(crypt::decrypt_field(&a).unwrap(), PLAIN);
}

#[test]
fn decrypt_plaintext_passthrough_and_empty() {
    ensure_test_master_key();
    // 存量未迁移行兼容：非前缀形态按明文原样返回
    assert_eq!(crypt::decrypt_field("sk-legacy").unwrap(), "sk-legacy");
    assert_eq!(crypt::decrypt_field("").unwrap(), "");
    // 空 key 不产生密文（保留 TRIM(api_key)='' 的「从未配置」语义）
    assert_eq!(crypt::encrypt_field("").unwrap(), "");
}

#[test]
fn decrypt_tampered_ciphertext_errors_not_panics() {
    ensure_test_master_key();
    let c = crypt::encrypt_field(PLAIN).unwrap();
    let mut bytes = c.into_bytes();
    let last = bytes.len() - 3;
    bytes[last] = b'X';
    let tampered = String::from_utf8(bytes).unwrap();
    assert!(crypt::decrypt_field(&tampered).is_err(), "篡改密文须报错");
}

#[test]
fn wrong_key_fails_to_decrypt() {
    ensure_test_master_key();
    let good = [1u8; 32];
    let bad = [2u8; 32];
    let c = crypt::encrypt_with(&good, PLAIN).unwrap();
    assert_eq!(crypt::decrypt_with(&good, &c).unwrap(), PLAIN);
    assert!(crypt::decrypt_with(&bad, &c).is_err(), "异主密钥须解密失败");
}

// ── 主密钥解析 ───────────────────────────────────────────────────────────

#[test]
fn unobtainable_key_source_clear_error() {
    // 显式 env 指向不可创建路径：清晰错误、不 panic（不触 keychain——env 优先于 keychain）
    let err = crypt::resolve_master_key_from(Some(std::path::Path::new(
        "/nonexistent-aidog-dir/master.key",
    )))
    .unwrap_err();
    assert!(err.contains("master key"), "错误信息须可读: {err}");
}

#[test]
fn key_file_created_0600_and_reloadable() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("master.key");
    let k1 = crypt::load_or_create_key_file(&p).unwrap();
    let k2 = crypt::load_or_create_key_file(&p).unwrap();
    assert_eq!(k1.as_slice(), k2.as_slice());
    assert_eq!(std::fs::metadata(&p).unwrap().len(), 32);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&p).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "降级 key 文件须 0600");
    }
}

// ── platform.api_key 端到端（三独立文件路径构造 Db，错路由当场 no such table）──

#[tokio::test]
async fn create_stores_ciphertext_reads_back_plaintext() {
    let (db, dir) = three_file_db().await;
    let p = create_platform(&db, input("crypt-e2e", PLAIN)).await.unwrap();
    let raw = raw_api_key(dir.path(), p.id as i64);
    assert!(raw.starts_with(crypt::ENC_PREFIX), "列值须密文形态: {raw}");
    assert!(!raw.contains(PLAIN), "列值不得含明文 key");
    assert_eq!(
        get_platform(&db, p.id).await.unwrap().unwrap().api_key,
        PLAIN,
        "读回须透明解密"
    );
}

#[tokio::test]
async fn update_reencrypts_rotated_key() {
    let (db, dir) = three_file_db().await;
    let p = create_platform(&db, input("crypt-rot", PLAIN)).await.unwrap();
    let rotated = "sk-rotated-9999";
    let updated = update_platform(
        &db,
        UpdatePlatform {
            id: p.id,
            name: None,
            platform_type: None,
            base_url: None,
            api_key: Some(rotated.into()),
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.api_key, rotated, "返回值仍是明文");
    let raw = raw_api_key(dir.path(), p.id as i64);
    assert!(raw.starts_with(crypt::ENC_PREFIX));
    assert!(!raw.contains(rotated));
    assert_eq!(
        get_platform(&db, p.id).await.unwrap().unwrap().api_key,
        rotated
    );
}

// ── 存量迁移 ─────────────────────────────────────────────────────────────

/// 绕过收口直接塞明文行（模拟存量 DB）。
async fn insert_legacy_row(db: &Db, name: &str, api_key_sql: &str) -> i64 {
    let (name, api_key_sql) = (name.to_string(), api_key_sql.to_string());
    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            &format!(
                "INSERT INTO platform (name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at, manual_budgets, expires_at, quota_script, quota_source) \
                 VALUES ('{name}', '\"anthropic\"', 'https://x.example.com', {api_key_sql}, '{{}}', '{{}}', '[]', '[]', 1, 0, 0, '[]', 0, '', 'auto')"
            ),
            [],
        )
        ?;
        Ok(conn.last_insert_rowid())
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn legacy_plaintext_row_migrated_idempotently() {
    let (db, dir) = three_file_db().await;
    let id = insert_legacy_row(&db, "legacy-plain", "'sk-legacy-plain-42'").await;

    // 迁移前：明文兼容读取
    assert_eq!(
        get_platform(&db, id as u64).await.unwrap().unwrap().api_key,
        "sk-legacy-plain-42"
    );

    // 启动迁移入口重放（init_tables_raw 幂等）
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();

    let migrated = raw_api_key(dir.path(), id);
    assert!(migrated.starts_with(crypt::ENC_PREFIX), "迁移后须密文: {migrated}");
    assert!(!migrated.contains("sk-legacy-plain-42"));
    // 读回仍透明
    assert_eq!(
        get_platform(&db, id as u64).await.unwrap().unwrap().api_key,
        "sk-legacy-plain-42"
    );

    // 幂等：再跑一次密文值不变
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    assert_eq!(raw_api_key(dir.path(), id), migrated, "二次迁移不得改写密文");
}

#[tokio::test]
async fn migration_failure_keeps_bad_row_and_continues() {
    let (db, dir) = three_file_db().await;
    // 一行正常明文 + 一行非法 UTF-8（读侧失败注入：该行迁移必失败）
    let good = insert_legacy_row(&db, "legacy-good", "'sk-good-77'").await;
    let bad = insert_legacy_row(&db, "legacy-bad", "CAST(x'80FF4142' AS TEXT)").await;

    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();

    // 好行照常迁移
    let good_raw = raw_api_key(dir.path(), good);
    assert!(good_raw.starts_with(crypt::ENC_PREFIX), "好行须已加密: {good_raw}");
    // 坏行原样保留（字节级），下次启动重试
    let conn = rusqlite::Connection::open(dir.path().join("platform.db")).unwrap();
    let bad_bytes: Vec<u8> = conn
        .query_row(
            "SELECT CAST(api_key AS BLOB) FROM platform WHERE id = ?1",
            params![bad],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .unwrap();
    assert_eq!(bad_bytes, vec![0x80, 0xFF, 0x41, 0x42], "失败行明文字节须保留");
}
