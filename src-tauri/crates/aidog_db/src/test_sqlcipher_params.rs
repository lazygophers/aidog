//! 加密参数（页大小 16384 + HMAC 开关）迁移测试：新库直建、存量 4096 迁移、幂等、
//! 失败 .bak 保留、HMAC 默认/关闭、sidecar 跟随。全部临时 key 文件 + tempfile 唯一路径，
//! 不触真 keychain（.claude/rules/db-pool-routing.md「测试不得弹系统 GUI 窗」）。
#![cfg(all(test, unix))]
use crate::sqlcipher;
use crate::test_support::*;
use crate::*;

/// 建 4096 页存量加密库（含一行数据），返回 (dir, 路径)。
fn seed_legacy_4096_db(name: &str) -> (tempfile::TempDir, String) {
    ensure_test_master_key();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(name).to_string_lossy().into_owned();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let key = crate::crypt::master_key().unwrap();
    conn.execute_batch(&format!(
        "PRAGMA key = \"x'{}'\"; PRAGMA cipher_page_size = 4096;
         CREATE TABLE t(v TEXT);
         CREATE TABLE extra(id INTEGER PRIMARY KEY AUTOINCREMENT, payload BLOB, amount REAL);
         INSERT INTO extra(payload, amount) VALUES (x'00ff', 1.25);",
        sqlcipher::hex_key(key)
    ))
    .unwrap();
    conn.execute("INSERT INTO t(v) VALUES ('hello')", [])
        .unwrap();
    drop(conn);
    (dir, path)
}

/// 裸连接（显式给定参数，不触发 ensure/migration）：读迁移前的库状态用。
fn raw_conn(path: &str, page: u32, hmac_off: bool) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(path).unwrap();
    let key = crate::crypt::master_key().unwrap();
    let mut sql = format!(
        "PRAGMA key = \"x'{}'\"; PRAGMA cipher_page_size = {page};",
        sqlcipher::hex_key(key)
    );
    if hmac_off {
        sql.push_str(" PRAGMA cipher_use_hmac = OFF;");
    }
    conn.execute_batch(&sql).unwrap();
    conn
}

fn raw_hmac_enabled(path: &str) -> bool {
    let conn = raw_conn(path, 4096, false);
    let raw: String = conn
        .query_row("PRAGMA cipher_use_hmac", [], |r| r.get(0))
        .unwrap();
    raw == "1"
}

/// 迁移后状态读法：open_keyed_conn（自身会跑 ensure，读到的即生效参数）。
fn page_size_of(path: &str) -> u32 {
    let conn = sqlcipher::open_keyed_conn(path).unwrap();
    let raw: String = conn
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .unwrap();
    raw.parse().unwrap()
}

fn hmac_enabled_of(path: &str) -> bool {
    let conn = sqlcipher::open_keyed_conn(path).unwrap();
    let raw: String = conn
        .query_row("PRAGMA cipher_use_hmac", [], |r| r.get(0))
        .unwrap();
    raw == "1"
}

fn bak_count(path: &str) -> usize {
    let prefix = format!(
        "{}.aidog-cipher-backup-",
        std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
    );
    let dir = std::path::Path::new(path).parent().unwrap();
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .count()
}

fn set_hmac_off() {
    sqlcipher::write_cipher_config(sqlcipher::CipherConfig {
        page_size: sqlcipher::CIPHER_PAGE_SIZE,
        hmac_off: true,
    })
    .unwrap();
}

/// ① 新库直接建为 16384 页，HMAC 默认开启（行为与现状一致）。
#[tokio::test]
async fn new_db_uses_16384_page_size() {
    let _home = HomeGuard::new();
    ensure_test_master_key();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aidog.db").to_string_lossy().into_owned();
    let db = Db::new(&path).await.unwrap();
    drop(db);
    assert_eq!(page_size_of(&path), sqlcipher::CIPHER_PAGE_SIZE);
    assert!(hmac_enabled_of(&path), "出厂默认 HMAC 保持开启");
}

/// ② 4096 存量库启动迁移 → 16384 + 数据完整。
#[tokio::test]
async fn existing_4096_database_migrates() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    // 裸连接读迁移前状态（open_keyed_conn 会自己触发迁移，不能用来读迁移前）。
    {
        let conn = raw_conn(&path, 4096, false);
        let raw: String = conn
            .query_row("PRAGMA page_size", [], |r| r.get(0))
            .unwrap();
        assert_eq!(raw, "4096");
    }
    let db = Db::new(&path).await.unwrap();
    let v: String = db
        .call_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT v FROM t", [], |r| r.get(0))
        })
        .await
        .unwrap();
    assert_eq!(v, "hello");
    drop(db);
    assert_eq!(page_size_of(&path), sqlcipher::CIPHER_PAGE_SIZE);
    assert_eq!(bak_count(&path), 0, "迁移成功后 .bak 应删除");
}

/// 配置损坏时加密库按实测参数打开，不迁移且不覆写配置。
#[tokio::test]
async fn corrupt_config_opens_encrypted_db_without_rewriting_config() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let config = _home.home().join(".aidog/db-cipher.json");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, b"not json").unwrap();
    let db = Db::new(&path).await.unwrap();
    let value: String = db
        .call_read_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT v FROM t", [], |r| r.get(0))
        })
        .await
        .unwrap();
    assert_eq!(value, "hello");
    drop(db);
    let conn = raw_conn(&path, 4096, false);
    let raw: String = conn
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw, "4096", "损坏配置不能触发参数迁移");
    assert_eq!(std::fs::read(config).unwrap(), b"not json");
}

/// 明文库遇损坏配置时拒绝迁移，保留库内容与配置原文。
#[test]
fn corrupt_config_keeps_plaintext_db_unchanged() {
    let _home = HomeGuard::new();
    ensure_test_master_key();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aidog.db").to_string_lossy().into_owned();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t VALUES ('plain');")
        .unwrap();
    drop(conn);
    let config = _home.home().join(".aidog/db-cipher.json");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, b"not json").unwrap();
    let error = sqlcipher::migrate_plaintext_if_needed(&path).unwrap_err();
    assert!(error.contains(&config.display().to_string()));
    let conn = rusqlite::Connection::open(&path).unwrap();
    let value: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(value, "plain");
    assert_eq!(std::fs::read(config).unwrap(), b"not json");
}

/// ③ 迁移幂等：二次启动不再触发（无新 .bak，页大小不变）。
#[tokio::test]
async fn migration_is_idempotent() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let db = Db::new(&path).await.unwrap();
    drop(db);
    assert_eq!(bak_count(&path), 0);
    let db = Db::new(&path).await.unwrap();
    drop(db);
    assert_eq!(page_size_of(&path), sqlcipher::CIPHER_PAGE_SIZE);
    assert_eq!(bak_count(&path), 0, "二次启动不得再迁移");
}

/// 目标文件所在目录支持 fsync，供 rename/删除持久化排序使用。
#[test]
fn database_directory_sync_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db.sqlite").to_string_lossy().into_owned();
    std::fs::write(&path, b"test").unwrap();
    sqlcipher::sync_parent_directory(&path).unwrap();
}

/// 比较隐藏 rowid 及 rowid 别名被列名遮蔽时的可用别名。
#[test]
fn logical_copy_check_compares_implicit_rowids() {
    let dir = tempfile::tempdir().unwrap();
    let left = rusqlite::Connection::open(dir.path().join("left.db")).unwrap();
    let right = rusqlite::Connection::open(dir.path().join("right.db")).unwrap();
    left.execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t(rowid,v) VALUES (7,'same');")
        .unwrap();
    right
        .execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t(rowid,v) VALUES (8,'same');")
        .unwrap();
    assert!(sqlcipher::verify_logical_copy(&left, &right).is_err());

    left.execute_batch("DROP TABLE t; CREATE TABLE t(rowid TEXT, v TEXT); INSERT INTO t(_rowid_,rowid,v) VALUES (11,'same','same');")
        .unwrap();
    right.execute_batch("DROP TABLE t; CREATE TABLE t(rowid TEXT, v TEXT); INSERT INTO t(_rowid_,rowid,v) VALUES (12,'same','same');")
        .unwrap();
    assert!(sqlcipher::verify_logical_copy(&left, &right).is_err());
}

/// WITHOUT ROWID 表按主键排序并比较可见行。
#[test]
fn logical_copy_check_orders_without_rowid_by_primary_key() {
    let dir = tempfile::tempdir().unwrap();
    let left = rusqlite::Connection::open(dir.path().join("left.db")).unwrap();
    let right = rusqlite::Connection::open(dir.path().join("right.db")).unwrap();
    left.execute_batch("CREATE TABLE t(a TEXT, b INTEGER, PRIMARY KEY(a,b)) WITHOUT ROWID; INSERT INTO t VALUES ('b',2),('a',1);")
        .unwrap();
    right.execute_batch("CREATE TABLE t(a TEXT, b INTEGER, PRIMARY KEY(a,b)) WITHOUT ROWID; INSERT INTO t VALUES ('a',1),('b',2);")
        .unwrap();
    sqlcipher::verify_logical_copy(&left, &right).unwrap();
}

/// 逻辑一致性校验拒绝相同 schema 下的行内容差异。
#[test]
fn logical_copy_check_rejects_row_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let left = rusqlite::Connection::open(dir.path().join("left.db")).unwrap();
    let right = rusqlite::Connection::open(dir.path().join("right.db")).unwrap();
    left.execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t VALUES ('source');")
        .unwrap();
    right
        .execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t VALUES ('different');")
        .unwrap();
    assert!(sqlcipher::verify_logical_copy(&left, &right).is_err());
}

/// 重启恢复：替换已完成但进程未清理备份时，验证新库后清理残留备份。
#[test]
fn startup_cleans_backup_after_verified_replacement() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    {
        let conn = sqlcipher::open_keyed_conn(&path).unwrap();
        drop(conn);
    }
    let stale_backup = format!("{path}.aidog-cipher-backup-{}", std::process::id());
    let user_backup = format!("{path}.bak-123456");
    std::fs::copy(&path, &stale_backup).unwrap();
    std::fs::write(&user_backup, b"user backup").unwrap();
    assert_eq!(bak_count(&path), 1);
    sqlcipher::clear_params_cache();
    let conn = sqlcipher::open_keyed_conn(&path).unwrap();
    let value: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(value, "hello");
    assert_eq!(bak_count(&path), 0);
    assert_eq!(std::fs::read(user_backup).unwrap(), b"user backup");
}

/// 替换后校验失败模拟：损坏活动库时只从已验证的本任务备份恢复。
#[test]
fn startup_restores_verified_backup_when_active_database_is_corrupt() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let backup = format!("{path}.aidog-cipher-backup-{}", std::process::id());
    std::fs::copy(&path, &backup).unwrap();
    sqlcipher::write_migration_journal_for_test(&path, &backup, false).unwrap();
    std::fs::write(&path, b"corrupted replacement").unwrap();

    assert!(!sqlcipher::migrate_plaintext_if_needed(&path).unwrap());
    let conn = raw_conn(&path, 4096, false);
    let value: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(value, "hello");
    assert_eq!(bak_count(&path), 0, "验证恢复库后清理迁移备份");
}

/// 无 journal 的孤立旧备份不得覆盖健康活动库中的后续合法写入。
#[test]
fn startup_preserves_orphan_backup_after_later_write() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let backup = format!("{path}.aidog-cipher-backup-{}", std::process::id());
    std::fs::copy(&path, &backup).unwrap();
    {
        let conn = raw_conn(&path, 4096, false);
        conn.execute("UPDATE t SET v = 'later legitimate write'", [])
            .unwrap();
    }

    let error = sqlcipher::migrate_plaintext_if_needed(&path).unwrap_err();
    assert!(error.contains("refusing rollback"));
    let conn = raw_conn(&path, 4096, false);
    let value: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(value, "later legitimate write");
    assert_eq!(bak_count(&path), 1, "活动库与旧备份均保留");
}

/// 持久迁移 journal 表明替换尚未完成时，允许恢复结构完整但内容不同的活动库。
#[test]
fn journaled_migration_restores_verified_backup_on_content_mismatch() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let backup = format!("{path}.aidog-cipher-backup-{}", std::process::id());
    std::fs::copy(&path, &backup).unwrap();
    sqlcipher::write_migration_journal_for_test(&path, &backup, false).unwrap();
    {
        let conn = raw_conn(&path, 4096, false);
        conn.execute("UPDATE t SET v = 'incomplete replacement'", [])
            .unwrap();
    }

    assert!(!sqlcipher::migrate_plaintext_if_needed(&path).unwrap());
    let conn = raw_conn(&path, 4096, false);
    let value: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(value, "hello");
    assert_eq!(bak_count(&path), 0);
}

/// 无 journal 时活动库损坏也不能盲目回滚到可能过期的备份。
#[test]
fn startup_preserves_orphan_backup_when_active_database_is_corrupt() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let backup = format!("{path}.aidog-cipher-backup-{}", std::process::id());
    std::fs::copy(&path, &backup).unwrap();
    std::fs::write(&path, b"corrupted active database").unwrap();

    assert!(sqlcipher::migrate_plaintext_if_needed(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupted active database");
    assert_eq!(bak_count(&path), 1, "无法判定时保留两份");
}

/// 同一路径的两个并发迁移调用串行化，后到者重探测并幂等返回。
#[test]
fn concurrent_parameter_migrations_serialize() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let gate = std::sync::Arc::new(std::sync::Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let path = path.clone();
        let gate = gate.clone();
        workers.push(std::thread::spawn(move || {
            gate.wait();
            sqlcipher::migrate_cipher_params(
                &path,
                &format!("{path}.shared-migration-target"),
                4096,
                false,
                sqlcipher::CipherConfig {
                    page_size: sqlcipher::CIPHER_PAGE_SIZE,
                    hmac_off: false,
                },
            )
        }));
    }
    gate.wait();
    for worker in workers {
        assert_eq!(
            worker.join().unwrap().unwrap(),
            (sqlcipher::CIPHER_PAGE_SIZE, false)
        );
    }
    let conn = sqlcipher::open_keyed_conn(&path).unwrap();
    let value: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(value, "hello");
}

/// ④ 迁移失败路径：export 失败 → Err + .bak 保留 + 原库 4096 可读。
#[test]
fn migration_failure_keeps_backup_and_original() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let bad_dst = "/nonexistent-aidog-perf-cipher/x.db";
    let r = sqlcipher::migrate_cipher_params(
        &path,
        bad_dst,
        4096,
        false,
        sqlcipher::CipherConfig {
            page_size: sqlcipher::CIPHER_PAGE_SIZE,
            hmac_off: false,
        },
    );
    assert!(r.is_err(), "dst 目录不存在必须失败");
    {
        let conn = raw_conn(&path, 4096, false);
        let raw: String = conn
            .query_row("PRAGMA page_size", [], |r| r.get(0))
            .unwrap();
        assert_eq!(raw, "4096", "原库不动");
        let v: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "hello");
    }
    assert_eq!(bak_count(&path), 1, "失败后 .bak 保留");
}

/// 中断恢复：导出阶段失败后旧库与备份保留；重试完成迁移并保留原数据。
#[test]
fn retry_after_interrupted_export_preserves_database() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let dst = format!("{path}.retry-mig");
    std::fs::create_dir(&dst).unwrap();
    assert!(
        sqlcipher::migrate_cipher_params(
            &path,
            &dst,
            4096,
            false,
            sqlcipher::CipherConfig {
                page_size: sqlcipher::CIPHER_PAGE_SIZE,
                hmac_off: false
            },
        )
        .is_err()
    );
    std::fs::remove_dir(&dst).unwrap();
    std::fs::write(&dst, b"interrupted export").unwrap();
    std::fs::write(format!("{dst}-wal"), b"stale wal").unwrap();
    std::fs::write(format!("{dst}-shm"), b"stale shm").unwrap();
    assert_eq!(bak_count(&path), 1);
    {
        let conn = raw_conn(&path, 4096, false);
        let value: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(value, "hello");
    }
    let dst = format!("{path}.retry-mig");
    sqlcipher::migrate_cipher_params(
        &path,
        &dst,
        4096,
        false,
        sqlcipher::CipherConfig {
            page_size: sqlcipher::CIPHER_PAGE_SIZE,
            hmac_off: false,
        },
    )
    .unwrap();
    assert_eq!(bak_count(&path), 0, "经验证的新库迁移成功后清理旧备份");
    let conn = sqlcipher::open_keyed_conn(&path).unwrap();
    let value: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(value, "hello");
    assert_eq!(bak_count(&path), 0, "验证当前库后清除任务备份");
}

/// exclusive checkpoint 持锁期间拒绝其他连接写入，释放迁移连接后允许继续。
#[test]
fn exclusive_lock_blocks_writer_until_connection_drops() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let lock_conn = raw_conn(&path, 4096, false);
    sqlcipher::checkpoint_truncate(&lock_conn).unwrap();
    let writer = raw_conn(&path, 4096, false);
    let err = writer
        .execute("INSERT INTO t(v) VALUES ('blocked')", [])
        .unwrap_err();
    assert!(err.sqlite_error_code().is_some_and(|code| {
        matches!(
            code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
        )
    }));
    drop(lock_conn);
    writer
        .execute("INSERT INTO t(v) VALUES ('after-unlock')", [])
        .unwrap();
    let count: i64 = writer
        .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);
}

/// WAL reader 阻止完整 checkpoint 时，迁移必须中止且保留原库与未检查点提交。
#[test]
fn wal_checkpoint_busy_aborts_without_losing_committed_rows() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let writer = raw_conn(&path, 4096, false);
    writer
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    writer
        .execute("INSERT INTO t(v) VALUES ('wal-committed')", [])
        .unwrap();
    let reader = raw_conn(&path, 4096, false);
    reader.execute_batch("BEGIN; SELECT v FROM t;").unwrap();
    writer
        .execute("INSERT INTO t(v) VALUES ('wal-pending')", [])
        .unwrap();
    assert!(std::path::Path::new(&format!("{path}-wal")).exists());

    let dst = format!("{path}.wal-mig");
    let result = sqlcipher::migrate_cipher_params(
        &path,
        &dst,
        4096,
        false,
        sqlcipher::CipherConfig {
            page_size: sqlcipher::CIPHER_PAGE_SIZE,
            hmac_off: false,
        },
    );
    assert!(
        result.is_err(),
        "pinned WAL reader must block TRUNCATE checkpoint"
    );
    assert_eq!(bak_count(&path), 0, "不得在 checkpoint 未完成时备份并替换");
    drop(reader);
    drop(writer);
    let conn = raw_conn(&path, 4096, false);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3, "两个已提交 WAL 写入仍可读取");
}

/// ⑤ HMAC 开关：默认关（= HMAC 开启）；设置后随迁移整库重写为 HMAC 关闭，重开可读。
#[tokio::test]
async fn hmac_off_config_rewrites_db_on_next_launch() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let db = Db::new(&path).await.unwrap();
    drop(db);
    set_hmac_off();
    // 未重启前 HMAC 未变（首次迁移只改页大小）。
    assert!(raw_hmac_enabled(&path));
    // 参数缓存进程级：清缓存模拟「下次启动」。
    sqlcipher::clear_params_cache();

    let db = Db::new(&path).await.unwrap();
    drop(db);
    assert!(!hmac_enabled_of(&path), "重启后 HMAC 应随迁移关闭");
    assert_eq!(page_size_of(&path), sqlcipher::CIPHER_PAGE_SIZE);
    // 关闭后数据可读 + 完整。
    let conn = raw_conn(&path, sqlcipher::CIPHER_PAGE_SIZE, true);
    let v: String = conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(v, "hello");
    let check: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
}

/// ⑤b sidecar 跟随主库 HMAC 设置：主库关 → log.db 同步迁移关闭且可读。
#[tokio::test]
async fn sidecar_follows_main_hmac_config() {
    let _home = HomeGuard::new();
    let (_dir, path) = seed_legacy_4096_db("aidog.db");
    let log_path = std::path::Path::new(&path)
        .parent()
        .unwrap()
        .join("log.db")
        .to_string_lossy()
        .into_owned();
    // 造一个 4096 的 log.db 存量。
    let conn = rusqlite::Connection::open(&log_path).unwrap();
    let key = crate::crypt::master_key().unwrap();
    conn.execute_batch(&format!(
        "PRAGMA key = \"x'{}'\"; PRAGMA cipher_page_size = 4096;
         CREATE TABLE t(v TEXT);",
        sqlcipher::hex_key(key)
    ))
    .unwrap();
    drop(conn);

    let db = Db::new(&path).await.unwrap();
    drop(db);
    set_hmac_off();
    // 参数缓存进程级：清缓存模拟「下次启动」。
    sqlcipher::clear_params_cache();

    let db = Db::new(&path).await.unwrap();
    drop(db);
    assert!(!hmac_enabled_of(&path), "主库 HMAC 关闭");
    assert!(!hmac_enabled_of(&log_path), "sidecar 跟随主库关闭");
    assert_eq!(page_size_of(&log_path), sqlcipher::CIPHER_PAGE_SIZE);
    let conn = raw_conn(&log_path, sqlcipher::CIPHER_PAGE_SIZE, true);
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(|x| x.ok())
        .collect();
    assert!(
        tables.contains(&"t".to_string()),
        "sidecar schema 随重写保留: {tables:?}"
    );
}

/// ⑥ 三文件库（platform/log/client_usage 分离路径）迁移后路由不破坏：写读回。
#[tokio::test]
async fn three_file_routing_survives_migration() {
    let _home = HomeGuard::new();
    let (db, dir) = three_file_db().await;
    let created = create_platform(&db, input("routing-db", "sk-test"))
        .await
        .unwrap();
    drop(db);
    // 重开句柄模拟重启，走完整迁移/探测链路后读回。
    let main = dir.path().join("aidog.db").to_string_lossy().into_owned();
    let db = Db::new(&main).await.unwrap();
    let got = get_platform(&db, created.id).await.unwrap().expect("row");
    assert_eq!(got.api_key, "sk-test");
    drop(db);
    for name in ["aidog.db", "log.db", "platform.db", "client_usage.db"] {
        let p = dir.path().join(name).to_string_lossy().into_owned();
        assert_eq!(page_size_of(&p), sqlcipher::CIPHER_PAGE_SIZE, "{name}");
    }
}

/// 平台建/读输入（与 test_crypt 同形，最小字段）。
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
