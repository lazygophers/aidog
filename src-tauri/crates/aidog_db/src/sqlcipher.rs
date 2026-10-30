//! SQLCipher 全库静态加密（db-sqlcipher-full 票 01）。
//!
//! 三个文件库（aidog.db / log.db / platform.db）open 时统一先 `PRAGMA key`（原始 key
//! 形式 `x'<hex>'`，key 来自 keychain 主密钥，src/crypt.rs 同源）。内存库不设 key
//! （SQLCipher 无 key 时行为等同标准 SQLite，既有测试透明）。存量明文库启动时经官方
//! `sqlcipher_export` 路径一次性迁移：rename 替换成功前明文库原样保留，任一步失败
//! 保留明文 + 报错，下次启动重试。
use rusqlite::Connection;
use std::io::Read;

/// 内存库判定（与 lib.rs `build_read_pool` 同口径：`:memory:` / mode=memory URI / 空路径）。
pub(crate) fn is_memory_path(path: &str) -> bool {
    path == ":memory:" || path.contains("mode=memory") || path.is_empty()
}

fn hex_key(k: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(k.len() * 2);
    for b in k {
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// key 不可得时必须在任何文件库 open 之前失败（不建明文库，spec Testing ④）。
/// 返回可直接拼进 pragma batch 首位的 SQL 片段（文件库 = `PRAGMA key = ...;`，内存库 = 空串）。
pub(crate) fn key_pragma_sql(path: &str) -> Result<String, String> {
    if is_memory_path(path) {
        return Ok(String::new());
    }
    let k = crate::crypt::master_key()?;
    Ok(format!("PRAGMA key = \"x'{}'\";", hex_key(k)))
}

/// ATTACH 加密库用的 `KEY "x'hex'"` 子句（同主库 master key；log.db / platform.db 与主库同 key）。
pub(crate) fn attach_key_clause() -> Result<String, String> {
    Ok(format!("KEY \"x'{}'\"", hex_key(crate::crypt::master_key()?)))
}

/// 明文 SQLite 文件头魔数（前 16 字节）。SQLCipher 下 page 1 连头一起密文 → 头部随机
/// 字节；非该魔数 = 已加密（或非 SQLite 文件，交给后续 open 报错）。
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

fn is_plaintext_sqlite(path: &str) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 16];
    f.read_exact(&mut buf).is_ok() && &buf == SQLITE_HEADER
}

/// 带 key 打开文件库（迁移自检 + 测试直读列值共用；`PRAGMA key` 须为首个语句）。
pub fn open_keyed_conn(path: impl AsRef<std::path::Path>) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    let key = crate::crypt::master_key()?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex_key(key)))
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// 存量明文库迁移（官方 `sqlcipher_export` 路径）。幂等：内存库 / 文件不存在 / 已加密
/// → Ok(false) 不动。返回 Ok(true) = 本次完成迁移；Err = 迁移失败，**明文库原样保留**，
/// 下次启动重试（失败语义见模块注释）。
pub(crate) fn migrate_plaintext_if_needed(path: &str) -> Result<bool, String> {
    if is_memory_path(path) || !std::path::Path::new(path).is_file() || !is_plaintext_sqlite(path) {
        return Ok(false);
    }
    // key 不可得：先于任何写动作失败，明文库不动、不建加密半成品。
    let key = crate::crypt::master_key()?;
    let hex = hex_key(key);

    let dst = format!("{path}.cipher-mig");
    let result = migrate_export(path, &dst, &hex);
    if let Err(e) = result {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{dst}{suffix}"));
        }
        return Err(e);
    }
    // 原子替换明文库（同目录 rename），随后清明文 WAL/SHM 旁文件。
    std::fs::rename(&dst, path).map_err(|e| format!("sqlcipher migration {path}: replace failed: {e}"))?;
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path}{suffix}"));
    }
    Ok(true)
}

/// 明文 conn 上 ATTACH 新加密库 + `sqlcipher_export` + 导出产物带 key 自检（integrity_check
/// 过了才让 rename 落地）。任一步失败 → Err（临时文件由调用方清理，明文库未动）。
fn migrate_export(src: &str, dst: &str, key_hex: &str) -> Result<(), String> {
    // 清理上次失败的半成品残留。
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{dst}{suffix}"));
    }
    let conn = Connection::open(src)
        .map_err(|e| format!("sqlcipher migration {src}: open plaintext failed: {e}"))?;
    // WAL 先落盘，导出的主文件才含全部已提交数据。
    conn.execute_batch("PRAGMA busy_timeout=5000; PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| format!("sqlcipher migration {src}: checkpoint failed: {e}"))?;
    // hex 字符集 [0-9a-f] 无注入面；dst 路径来自调用方，单引号翻倍转义兜底。
    let sql = format!(
        "ATTACH DATABASE '{}' AS cipher_mig KEY \"x'{key_hex}'\"; \
         SELECT sqlcipher_export('cipher_mig'); \
         DETACH DATABASE cipher_mig;",
        dst.replace('\'', "''")
    );
    conn.execute_batch(&sql)
        .map_err(|e| format!("sqlcipher migration {src}: sqlcipher_export failed: {e}"))?;
    drop(conn);

    // 导出产物自检：带 key 打开 + integrity_check，过了才允许替换明文库。
    let verify = open_keyed_conn(dst)
        .map_err(|e| format!("sqlcipher migration {src}: open exported db failed: {e}"))?;
    let check: String = verify
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| format!("sqlcipher migration {src}: verify read failed: {e}"))?;
    if check != "ok" {
        return Err(format!("sqlcipher migration {src}: integrity_check = {check}"));
    }
    Ok(())
}
