//! SQLCipher 全库加密外部行为测试（db-sqlcipher-full 票 01，spec Testing Decisions）。
//!
//! 覆盖：① 加密库标准 sqlite 打开失败（静态加密生效）② 存量明文迁移零丢失 + 幂等
//! ③ 迁移失败注入 → 明文库完好保留 ④ key 不可得 → 清晰错误且不建明文库。
//! 内存库透明性由既有套件全绿承载（不设 key，行为等同标准 SQLite）。
use crate::test_support::{ensure_test_master_key, three_file_db};
use crate::Db;
use rusqlite::Connection;

/// 明文 SQLite 文件头（前 16 字节）——加密库读出来必不是它。
fn plaintext_header(path: &str) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 16];
    f.read_exact(&mut buf).is_ok() && &buf == b"SQLite format 3\0"
}

/// 造一个含数据的明文库：表 mig_probe + 3 行已知内容。
fn build_plaintext_db(path: &str) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE mig_probe(id INTEGER PRIMARY KEY, v TEXT);
         INSERT INTO mig_probe(v) VALUES ('alpha'), ('beta'), ('gamma');",
    )
    .unwrap();
}

/// 票面验收 1：三库文件头非明文 + 无 key 的普通连接读 sqlite_master 报
/// `file is not a database`（静态加密生效，spec Testing ②）。
#[tokio::test]
async fn encrypted_files_reject_standard_sqlite_open() {
    let (_db, dir) = three_file_db().await;
    for name in ["aidog.db", "log.db", "platform.db"] {
        let path = dir.path().join(name);
        let path = path.to_str().unwrap();
        assert!(path_exist(path), "{name} exists");
        assert!(!plaintext_header(path), "{name} header must not be plaintext SQLite");
        let conn = Connection::open(path).unwrap(); // 打开成功（惰性），首条读才炸
        let err = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
            .expect_err("standard sqlite must fail on encrypted db");
        assert!(
            err.to_string().contains("not a database"),
            "{name}: unexpected error: {err}"
        );
    }
}

fn path_exist(path: &str) -> bool {
    std::path::Path::new(path).is_file()
}

/// 票面验收 2：三库各有数据的明文库 → Db::new 一次性迁移 → 逐表行数 + 内容比对 +
/// 文件头非明文 + 二次打开幂等（已加密不再迁移）。
#[tokio::test]
async fn plaintext_dbs_migrate_with_zero_loss() {
    ensure_test_master_key();
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("aidog.db");
    let main_path = main.to_str().unwrap();
    build_plaintext_db(main_path);
    build_plaintext_db(dir.path().join("log.db").to_str().unwrap());
    build_plaintext_db(dir.path().join("platform.db").to_str().unwrap());

    let db = Db::new(main_path).await.expect("migrate & open");
    assert!(!plaintext_header(main_path), "main migrated to encrypted");
    assert!(!plaintext_header(dir.path().join("log.db").to_str().unwrap()));
    assert!(!plaintext_header(dir.path().join("platform.db").to_str().unwrap()));

    // 逐库读回：行数 + 内容比对（主库 call_traced / log.db call_proxy_log_traced /
    // platform.db call_platform_traced）。
    let expected: Vec<(i64, String)> = vec![
        (1, "alpha".into()),
        (2, "beta".into()),
        (3, "gamma".into()),
    ];
    for (label, rows) in [
        ("main", read_probe(&db, CallSite::Main).await),
        ("log", read_probe(&db, CallSite::ProxyLog).await),
        ("platform", read_probe(&db, CallSite::Platform).await),
    ] {
        assert_eq!(rows, expected, "{label}: zero loss + content match");
    }

    // 二次打开：已加密（幂等不重迁），数据仍在。
    let db2 = Db::new(main_path).await.expect("reopen encrypted");
    assert_eq!(
        read_probe(&db2, CallSite::Main).await,
        expected,
        "reopen keeps data"
    );
}

enum CallSite {
    Main,
    ProxyLog,
    Platform,
}

async fn read_probe(db: &Db, site: CallSite) -> Vec<(i64, String)> {
    let collect = move |conn: &mut Connection| {
        let mut rows: Vec<(i64, String)> = Vec::new();
        let mut stmt = conn.prepare("SELECT id, v FROM mig_probe ORDER BY id")?;
        let mut it = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in it.by_ref() {
            rows.push(row?);
        }
        Ok(rows)
    };
    match site {
        CallSite::Main => db
            .call_traced(None, std::panic::Location::caller(), collect)
            .await
            .unwrap(),
        CallSite::ProxyLog => db
            .call_proxy_log_traced(None, std::panic::Location::caller(), collect)
            .await
            .unwrap(),
        CallSite::Platform => db
            .call_platform_traced(None, std::panic::Location::caller(), collect)
            .await
            .unwrap(),
    }
}

/// 票面验收 2 失败分支：导出目标路径被目录占位 → ATTACH 失败 → Db::new 报错且
/// 明文库完好保留（可继续用普通 sqlite 读，下次启动可重试）。
#[tokio::test]
async fn migration_failure_preserves_plaintext() {
    ensure_test_master_key();
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("aidog.db");
    let main_path = main.to_str().unwrap();
    build_plaintext_db(main_path);
    // 占位：导出临时文件路径已存在且是目录 → ATTACH 建库失败。
    std::fs::create_dir(format!("{main_path}.cipher-mig")).unwrap();

    let err = match Db::new(main_path).await {
        Err(e) => e,
        Ok(_) => panic!("migration must fail"),
    };
    assert!(
        err.contains("sqlcipher migration"),
        "clear error, got: {err}"
    );
    // 明文库完好：仍是明文头 + 内容原样可读。
    assert!(plaintext_header(main_path), "plaintext preserved on failure");
    let conn = Connection::open(main_path).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM mig_probe", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 3, "plaintext data intact after failed migration");
}

/// 票面验收 4（子进程注入）：`AIDOG_DB_MASTER_KEY_FILE` 指向非法 key 文件（长度 ≠ 32B）
/// → Db::new 清晰报错且**不创建任何明文库文件**。经子进程跑：master_key 的 OnceLock
/// 进程级缓存使主进程内无法在其它测试并行时可靠注入失败。
#[test]
fn key_unavailable_no_plaintext_db_created() {
    let bad_key = std::env::temp_dir().join(format!("aidog_bad_key_{}.bin", std::process::id()));
    std::fs::write(&bad_key, b"short").unwrap();
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "test_sqlcipher::key_unavailable_child",
        ])
        .env("AIDOG_DB_MASTER_KEY_FILE", &bad_key)
        .output()
        .expect("spawn test binary");
    let _ = std::fs::remove_file(&bad_key);
    assert!(
        out.status.success(),
        "child failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[ignore = "subprocess child of key_unavailable_no_plaintext_db_created（依赖注入的坏 key env）"]
fn key_unavailable_child() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("aidog.db");
    let main_path = main.to_str().unwrap().to_string();
    let res = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(Db::new(&main_path));
    let err = match res {
        Err(e) => e,
        Ok(_) => panic!("db open must fail when master key unavailable"),
    };
    assert!(
        err.contains("master key"),
        "clear error mentioning master key, got: {err}"
    );
    assert!(
        !std::path::Path::new(&main_path).exists(),
        "no plaintext db created when key unavailable"
    );
}
