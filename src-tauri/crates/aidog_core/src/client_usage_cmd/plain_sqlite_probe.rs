//! 票 06 前提验证：SQLCipher 构建的 rusqlite 能只读打开**明文** sqlite 库
//! （opencode.db 是明文 sqlite；本仓 rusqlite 是 bundled-sqlcipher 构建）。
//! 夹具用系统 python3（真 sqlite 实现）生成明文库；python3 缺失时跳过并打印。
use std::process::Command;

fn make_plain_db(path: &std::path::Path) -> bool {
    if path.exists() {
        return true;
    }
    let py = r#"
import sqlite3, sys
c = sqlite3.connect(sys.argv[1])
c.execute("CREATE TABLE t (a TEXT)")
c.execute("INSERT INTO t VALUES ('hello')")
c.commit()
"#;
    Command::new("python3")
        .arg("-c")
        .arg(py)
        .arg(path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn opencode_db_plaintext_readable_by_sqlcipher_rusqlite() {
    let dir = std::env::temp_dir().join(format!("aidog-oc-plain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("plain.db");
    if !make_plain_db(&p) {
        eprintln!("SKIP: python3 unavailable, cannot build plain sqlite fixture");
        return;
    }
    let conn = rusqlite::Connection::open_with_flags(
        &p,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("open plain db read-only");
    let v: String = conn
        .query_row("SELECT a FROM t LIMIT 1", [], |r| r.get(0))
        .expect("query plain db");
    assert_eq!(v, "hello");
    std::fs::remove_dir_all(&dir).ok();
}
