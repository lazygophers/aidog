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
    let conn = crate::sqlcipher::open_keyed_conn(dir.join("platform.db")).unwrap();
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
    // 原字符恰为 'X' 时旧写法等于没篡改（~1/64 偶发红）；换成必不同的字符
    bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
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
    let conn = crate::sqlcipher::open_keyed_conn(dir.path().join("platform.db")).unwrap();
    let bad_bytes: Vec<u8> = conn
        .query_row(
            "SELECT CAST(api_key AS BLOB) FROM platform WHERE id = ?1",
            params![bad],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .unwrap();
    assert_eq!(bad_bytes, vec![0x80, 0xFF, 0x41, 0x42], "失败行明文字节须保留");
}

// ── setting 表 cc_proxy_auth 值加密（票 03）──────────────────────────────

const CC_PW: &str = "pw-Aa11Bb22Cc33Dd44";

/// 直接 sqlite 打开主库 aidog.db 读 cc_proxy_auth 原始列值（不经收口）。
fn raw_cc_value(dir: &std::path::Path, key: &str) -> String {
    let conn = crate::sqlcipher::open_keyed_conn(dir.join("aidog.db")).unwrap();
    conn.query_row(
        "SELECT value FROM setting WHERE scope = 'cc_proxy_auth' AND key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .unwrap()
}

async fn set_cc(db: &Db, key: &str, v: &str) {
    set_setting(
        db,
        crate::models::SetSettingInput {
            scope: CC_PROXY_AUTH_SCOPE.to_string(),
            key: key.to_string(),
            value: serde_json::Value::String(v.to_string()),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn cc_proxy_auth_stores_ciphertext_reads_plaintext() {
    let (db, dir) = three_file_db().await;
    set_cc(&db, "gk1", CC_PW).await;
    let raw = raw_cc_value(dir.path(), "gk1");
    assert!(
        raw.starts_with(&format!("\"{}", crypt::ENC_PREFIX)),
        "列值须 JSON 串包裹的密文形态: {raw}"
    );
    assert!(!raw.contains(CC_PW), "列值不得含明文密码");
    // 读回透明（首次 DB 读 + 二次缓存命中两条路径都是明文）
    for _ in 0..2 {
        assert_eq!(
            get_setting(&db, CC_PROXY_AUTH_SCOPE, "gk1").await.unwrap().unwrap(),
            serde_json::Value::String(CC_PW.to_string())
        );
    }
}

#[tokio::test]
async fn other_scope_value_stays_plaintext() {
    let (db, dir) = three_file_db().await;
    set_setting(
        &db,
        crate::models::SetSettingInput {
            scope: "lang".to_string(),
            key: "k".to_string(),
            value: serde_json::Value::String("zh-Hans".to_string()),
        },
    )
    .await
    .unwrap();
    let conn = crate::sqlcipher::open_keyed_conn(dir.path().join("aidog.db")).unwrap();
    let raw: String = conn
        .query_row("SELECT value FROM setting WHERE scope = 'lang'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw, "\"zh-Hans\"", "非 cc_proxy_auth scope 不得加密");
}

/// 绕过收口直接塞明文 JSON 行（模拟存量 DB）。
async fn insert_legacy_cc_row(db: &Db, key: &str, value_sql: &str) {
    let (key, value_sql) = (key.to_string(), value_sql.to_string());
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            &format!(
                "INSERT INTO setting (scope, key, value, created_at, updated_at) \
                 VALUES ('cc_proxy_auth', '{key}', {value_sql}, 0, 0)"
            ),
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cc_proxy_auth_legacy_row_migrated_idempotently() {
    let (db, dir) = three_file_db().await;
    insert_legacy_cc_row(&db, "lg1", "'\"pw-legacy-7777\"'").await;

    // 迁移前：明文兼容读取
    assert_eq!(
        get_setting(&db, CC_PROXY_AUTH_SCOPE, "lg1").await.unwrap().unwrap(),
        serde_json::Value::String("pw-legacy-7777".to_string())
    );

    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    let migrated = raw_cc_value(dir.path(), "lg1");
    assert!(migrated.starts_with(&format!("\"{}", crypt::ENC_PREFIX)), "迁移后须密文: {migrated}");
    assert!(!migrated.contains("pw-legacy-7777"));
    assert_eq!(
        get_setting(&db, CC_PROXY_AUTH_SCOPE, "lg1").await.unwrap().unwrap(),
        serde_json::Value::String("pw-legacy-7777".to_string()),
        "读回仍透明"
    );

    // 幂等：再跑一次密文值不变
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    assert_eq!(raw_cc_value(dir.path(), "lg1"), migrated, "二次迁移不得改写密文");
}

#[tokio::test]
async fn cc_proxy_auth_migration_failure_keeps_row_and_continues() {
    let (db, dir) = three_file_db().await;
    // 一行正常明文 + 一行非法 UTF-8（读侧 JSON 解析必失败，失败注入）
    insert_legacy_cc_row(&db, "good", "'\"pw-good-11\"'").await;
    insert_legacy_cc_row(&db, "bad", "CAST(x'80FF4142' AS TEXT)").await;

    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();

    let good_raw = raw_cc_value(dir.path(), "good");
    assert!(good_raw.starts_with(&format!("\"{}", crypt::ENC_PREFIX)), "好行须已加密: {good_raw}");
    // 坏行原样保留（字节级），下次启动重试
    let conn = crate::sqlcipher::open_keyed_conn(dir.path().join("aidog.db")).unwrap();
    let bad_bytes: Vec<u8> = conn
        .query_row(
            "SELECT CAST(value AS BLOB) FROM setting WHERE scope = 'cc_proxy_auth' AND key = 'bad'",
            [],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .unwrap();
    assert_eq!(bad_bytes, vec![0x80, 0xFF, 0x41, 0x42], "失败行明文字节须保留");
}


// ── mcp_server.env_json/headers_json 存量迁移（票 02）──────────────────

/// 绕过收口直接塞 mcp_server 行（env/headers 用 SQL 字面量，模拟存量明文 DB）。
async fn insert_legacy_mcp_row(db: &Db, name: &str, env_sql: &str, headers_sql: &str) -> i64 {
    let sql = format!(
        "INSERT INTO mcp_server (name, transport, command, args_json, env_json, url, headers_json, enabled_agents, created_at, updated_at) \
         VALUES ('{name}', 'stdio', 'npx', '[]', {env_sql}, '', {headers_sql}, '', 0, 0)"
    );
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(&sql, [])?;
        Ok(conn.last_insert_rowid())
    })
    .await
    .unwrap()
}

fn raw_mcp_json(dir: &std::path::Path, id: i64) -> (String, String) {
    let conn = crate::sqlcipher::open_keyed_conn(dir.join("aidog.db")).unwrap();
    conn.query_row(
        "SELECT env_json, headers_json FROM mcp_server WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

#[tokio::test]
async fn mcp_legacy_plaintext_rows_migrated_idempotently() {
    let (db, dir) = three_file_db().await;
    let id = insert_legacy_mcp_row(
        &db,
        "mcp-legacy",
        r#"'{"API_TOKEN":"mcp-secret-42"}'"#,
        r#"'{"Authorization":"Bearer mcp-bk-42"}'"#,
    )
    .await;

    // 启动迁移入口重放（init_tables_raw 幂等）
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();

    let (env, headers) = raw_mcp_json(dir.path(), id);
    assert!(env.starts_with(crypt::ENC_PREFIX), "env_json 迁移后须密文: {env}");
    assert!(headers.starts_with(crypt::ENC_PREFIX), "headers_json 迁移后须密文: {headers}");
    assert!(!env.contains("mcp-secret-42"));
    assert!(!headers.contains("mcp-bk-42"));

    // 幂等：再跑一次密文值不变
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    assert_eq!(raw_mcp_json(dir.path(), id), (env.clone(), headers.clone()), "二次迁移不得改写密文");
}

#[tokio::test]
async fn mcp_migration_failure_keeps_bad_row_and_continues() {
    let (db, dir) = three_file_db().await;
    // 一行正常明文 + 一行非法 UTF-8（读侧失败注入：该行迁移必失败）
    let good = insert_legacy_mcp_row(&db, "mcp-good", r#"'{"A":"b"}'"#, "'{}'").await;
    let bad = insert_legacy_mcp_row(&db, "mcp-bad", "CAST(x'80FF' AS TEXT)", "'{}'").await;

    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();

    // 好行照常迁移
    let (env, _) = raw_mcp_json(dir.path(), good);
    assert!(env.starts_with(crypt::ENC_PREFIX), "好行须已加密: {env}");
    // 坏行原样保留（字节级），下次启动重试
    let conn = crate::sqlcipher::open_keyed_conn(dir.path().join("aidog.db")).unwrap();
    let bad_bytes: Vec<u8> = conn
        .query_row(
            "SELECT CAST(env_json AS BLOB) FROM mcp_server WHERE id = ?1",
            params![bad],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .unwrap();
    assert_eq!(bad_bytes, vec![0x80, 0xFF], "失败行明文字节须保留");
}

// ── oauth 观测表 raw 列存量迁移（db-obs-encryption 票 01）───────────────

const OAUTH_RAW: &str = r#"{"email":"u@example.com","account_uuid":"acct-42"}"#;

/// 绕过收口直接塞 oauth_usage_sample 明文行（模拟存量 DB）。
async fn insert_legacy_sample(db: &Db, group: &str, raw_sql: &str) -> i64 {
    let (group, raw_sql) = (group.to_string(), raw_sql.to_string());
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            &format!(
                "INSERT INTO oauth_usage_sample (group_name, five_hour_pct, seven_day_pct, raw, sampled_at) \
                 VALUES ('{group}', 0, 0, {raw_sql}, 0)"
            ),
            [],
        )?;
        Ok(conn.last_insert_rowid())
    })
    .await
    .unwrap()
}

/// 绕过收口直接塞 cc_oauth_profile 明文行（模拟存量 DB）。
async fn insert_legacy_profile(db: &Db, group: &str, raw_sql: &str) {
    let (group, raw_sql) = (group.to_string(), raw_sql.to_string());
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            &format!(
                "INSERT INTO cc_oauth_profile (group_name, tier, raw, updated_at) \
                 VALUES ('{group}', 't', {raw_sql}, 0)"
            ),
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap()
}

fn raw_oauth_col(dir: &std::path::Path, sql: &str) -> String {
    let conn = crate::sqlcipher::open_keyed_conn(dir.join("aidog.db")).unwrap();
    conn.query_row(sql, [], |r| r.get::<_, String>(0)).unwrap()
}

#[tokio::test]
async fn oauth_raw_legacy_rows_migrated_idempotently() {
    let (db, dir) = three_file_db().await;
    let sid = insert_legacy_sample(&db, "cc", &format!("'{OAUTH_RAW}'")).await;
    insert_legacy_profile(&db, "cc", &format!("'{OAUTH_RAW}'")).await;

    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();

    let sample = raw_oauth_col(
        dir.path(),
        &format!("SELECT raw FROM oauth_usage_sample WHERE id = {sid}"),
    );
    let profile = raw_oauth_col(
        dir.path(),
        "SELECT raw FROM cc_oauth_profile WHERE group_name = 'cc'",
    );
    for (name, raw) in [("sample", &sample), ("profile", &profile)] {
        assert!(raw.starts_with(crypt::ENC_PREFIX), "{name} 迁移后须密文: {raw}");
        assert!(!raw.contains("u@example.com"));
        assert_eq!(crypt::decrypt_field(raw).unwrap(), OAUTH_RAW, "{name} 读回须还原明文");
    }

    // 幂等：再跑一次密文值不变
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    assert_eq!(
        raw_oauth_col(dir.path(), &format!("SELECT raw FROM oauth_usage_sample WHERE id = {sid}")),
        sample,
        "二次迁移不得改写密文"
    );
    assert_eq!(
        raw_oauth_col(dir.path(), "SELECT raw FROM cc_oauth_profile WHERE group_name = 'cc'"),
        profile,
        "二次迁移不得改写密文"
    );
}

#[tokio::test]
async fn oauth_raw_migration_failure_keeps_row_and_continues() {
    let (db, dir) = three_file_db().await;
    // 两表各一行正常明文 + sample 表一行非法 UTF-8（读侧失败注入：该行迁移必失败）
    let good = insert_legacy_sample(&db, "good", &format!("'{OAUTH_RAW}'")).await;
    insert_legacy_sample(&db, "bad", "CAST(x'80FF4142' AS TEXT)").await;
    insert_legacy_profile(&db, "good", &format!("'{OAUTH_RAW}'")).await;

    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();

    let sample = raw_oauth_col(dir.path(), &format!("SELECT raw FROM oauth_usage_sample WHERE id = {good}"));
    assert!(sample.starts_with(crypt::ENC_PREFIX), "好行须已加密: {sample}");
    let profile = raw_oauth_col(dir.path(), "SELECT raw FROM cc_oauth_profile WHERE group_name = 'good'");
    assert!(profile.starts_with(crypt::ENC_PREFIX), "另一表好行须已加密: {profile}");
    // 坏行原样保留（字节级），下次启动重试
    let conn = crate::sqlcipher::open_keyed_conn(dir.path().join("aidog.db")).unwrap();
    let bad_bytes: Vec<u8> = conn
        .query_row(
            "SELECT CAST(raw AS BLOB) FROM oauth_usage_sample WHERE group_name = 'bad'",
            [],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .unwrap();
    assert_eq!(bad_bytes, vec![0x80, 0xFF, 0x41, 0x42], "失败行明文字节须保留");
}
