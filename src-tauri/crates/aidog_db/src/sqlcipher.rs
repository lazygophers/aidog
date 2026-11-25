//! SQLCipher 全库静态加密（db-sqlcipher-full 票 01）。
//!
//! 三个文件库（aidog.db / log.db / platform.db）open 时统一先 `PRAGMA key`（原始 key
//! 形式 `x'<hex>'`，key 来自 keychain 主密钥，src/crypt.rs 同源）。内存库不设 key
//! （SQLCipher 无 key 时行为等同标准 SQLite，既有测试透明）。存量明文库启动时经官方
//! `sqlcipher_export` 路径一次性迁移：rename 替换成功前明文库原样保留，任一步失败
//! 保留明文 + 报错，下次启动重试。
use rusqlite::{Connection, types::Value};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CIPHER_PAGE_SIZE: u32 = 16_384;
// Edit ~/.aidog/db-cipher.json before launch; hmac_off defaults to false (HMAC enabled).
const CIPHER_CONFIG_FILE: &str = "db-cipher.json";
const CIPHER_MIGRATION_LOCK_FILE: &str = "db-cipher-migration.lock";

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct CipherConfig {
    pub(crate) page_size: u32,
    pub(crate) hmac_off: bool,
}

impl Default for CipherConfig {
    fn default() -> Self {
        Self {
            page_size: CIPHER_PAGE_SIZE,
            hmac_off: false,
        }
    }
}

fn cipher_config_path() -> Result<std::path::PathBuf, String> {
    let home =
        std::env::var_os("HOME").ok_or_else(|| "HOME not set for db cipher config".to_string())?;
    Ok(std::path::PathBuf::from(home)
        .join(".aidog")
        .join(CIPHER_CONFIG_FILE))
}

fn cipher_migration_lock() -> Result<File, String> {
    let path = cipher_config_path()?.with_file_name(CIPHER_MIGRATION_LOCK_FILE);
    let parent = path
        .parent()
        .ok_or_else(|| "DB migration lock has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| {
        format!(
            "create DB migration lock directory {}: {e}",
            parent.display()
        )
    })?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| format!("open DB migration lock {}: {e}", path.display()))?;
    lock.lock()
        .map_err(|e| format!("lock DB migration {}: {e}", path.display()))?;
    Ok(lock)
}

fn read_cipher_config() -> Result<CipherConfig, String> {
    let path = cipher_config_path()?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(CipherConfig::default()),
        Err(e) => {
            return Err(format!(
                "read DB cipher config {}: {e}; repair or restore this JSON file before migrating plaintext databases",
                path.display()
            ));
        }
    };
    let config: CipherConfig = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "invalid DB cipher config {}: {e}; repair or restore this JSON file before migrating plaintext databases (page_size: 4096 or 16384; hmac_off: boolean)",
            path.display()
        )
    })?;
    if config.page_size != CIPHER_PAGE_SIZE && config.page_size != 4096 {
        return Err(format!(
            "invalid DB cipher config {}: unsupported page_size {}; use 4096 or 16384",
            path.display(),
            config.page_size
        ));
    }
    Ok(config)
}

pub(crate) fn write_cipher_config(config: CipherConfig) -> Result<(), String> {
    let path = cipher_config_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| "db cipher config has no parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("create db cipher config dir: {e}"))?;
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .map_err(|e| format!("write db cipher config {}: {e}", temp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("protect db cipher config {}: {e}", temp.display()))?;
    }
    file.write_all(&bytes)
        .map_err(|e| format!("write db cipher config {}: {e}", temp.display()))?;
    file.sync_all()
        .map_err(|e| format!("sync db cipher config {}: {e}", temp.display()))?;
    std::fs::rename(&temp, &path)
        .map_err(|e| format!("replace db cipher config {}: {e}", path.display()))?;
    #[cfg(unix)]
    sync_parent_directory(&path.to_string_lossy())?;
    Ok(())
}

/// 内存库判定（与 lib.rs `build_read_pool` 同口径：`:memory:` / mode=memory URI / 空路径）。
pub(crate) fn is_memory_path(path: &str) -> bool {
    path == ":memory:" || path.contains("mode=memory") || path.is_empty()
}

pub(crate) fn hex_key(k: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(k.len() * 2);
    for b in k {
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// key 不可得时必须在任何文件库 open 之前失败（不建明文库，spec Testing ④）。
/// 返回可直接拼进 pragma batch 首位的 SQL 片段（文件库 = `PRAGMA key` + 加密参数 +
/// 存量库整库迁移，内存库 = 空串）。
pub(crate) fn key_pragma_sql(path: &str) -> Result<String, String> {
    if is_memory_path(path) {
        return Ok(String::new());
    }
    crate::sqlcipher::migrate_plaintext_if_needed(path)?;
    let (page, hmac_off) = ensure_cipher_params(path)?;
    let k = crate::crypt::master_key()?;
    let mut sql = format!(
        "PRAGMA key = \"x'{}'\"; PRAGMA cipher_page_size = {page};",
        hex_key(k)
    );
    if hmac_off {
        sql.push_str(" PRAGMA cipher_use_hmac = OFF;");
    }
    Ok(sql)
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
/// 含加密参数迁移 + 页大小/HMAC pragma（同 `key_pragma_sql` 一套逻辑）。
pub fn open_keyed_conn(path: impl AsRef<std::path::Path>) -> Result<Connection, String> {
    let path_str = path.as_ref().to_string_lossy().into_owned();
    let key_sql = key_pragma_sql(&path_str)?;
    let conn = Connection::open(path.as_ref()).map_err(|e| e.to_string())?;
    conn.execute_batch(&key_sql).map_err(|e| e.to_string())?;
    Ok(conn)
}

/// 存量明文库迁移（官方 `sqlcipher_export` 路径）。幂等：内存库 / 文件不存在 / 已加密
/// → Ok(false) 不动。返回 Ok(true) = 本次完成迁移；Err = 迁移失败，**明文库原样保留**，
/// 下次启动重试（失败语义见模块注释）。
pub(crate) fn migrate_plaintext_if_needed(path: &str) -> Result<bool, String> {
    if is_memory_path(path) {
        return Ok(false);
    }
    let _migration_lock = cipher_migration_lock()?;
    recover_database_if_needed(path)?;
    if !std::path::Path::new(path).is_file() || !is_plaintext_sqlite(path) {
        return Ok(false);
    }
    // key 不可得：先于任何写动作失败，明文库不动、不建加密半成品。
    let key = crate::crypt::master_key()?;
    let hex = hex_key(key);

    let dst = format!("{path}.cipher-mig");
    let config = read_cipher_config()?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let bak = format!("{path}.aidog-cipher-backup-{ts}");
    match migrate_export(path, &dst, &hex, config, &bak) {
        Ok(()) => Ok(true),
        Err(e) => {
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{dst}{suffix}"));
            }
            Err(if std::path::Path::new(&bak).is_file() {
                format!("{e}（.bak 保留：{bak}）")
            } else {
                e
            })
        }
    }
}

/// 明文 conn 持独占锁至替换完成；替换前失败时源库仍可读。
fn migrate_export(
    src: &str,
    dst: &str,
    key_hex: &str,
    config: CipherConfig,
    backup: &str,
) -> Result<(), String> {
    sync_parent_directory(src)?;
    // 清理上次失败的半成品残留。
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{dst}{suffix}"));
    }
    let conn = Connection::open(src)
        .map_err(|e| format!("sqlcipher migration {src}: open plaintext failed: {e}"))?;
    checkpoint_truncate(&conn)
        .map_err(|e| format!("sqlcipher migration {src}: checkpoint failed: {e}"))?;
    copy_file_create_new(src, backup)
        .map_err(|e| format!("sqlcipher migration {src}: backup failed: {e}"))?;
    let backup_conn = Connection::open(backup)
        .map_err(|e| format!("sqlcipher migration backup open failed: {e}"))?;
    integrity_check(&backup_conn)?;
    verify_logical_copy(&conn, &backup_conn)?;
    write_migration_journal(src, backup, false)?;

    export_into(
        &conn,
        dst,
        key_hex,
        &cipher_target_pragmas(config.page_size, config.hmac_off),
    )
    .map_err(|e| format!("sqlcipher migration {src}: {e}"))?;
    sync_file(dst)?;
    sync_parent_directory(dst)?;

    let target_conn = open_with_cipher_params(dst, config.page_size, config.hmac_off)?;
    integrity_check(&target_conn)?;
    verify_logical_copy(&conn, &target_conn)?;
    drop(target_conn);
    remove_sidecars(src)?;
    std::fs::rename(dst, src).map_err(|e| {
        format!("sqlcipher migration {src}: replace failed: {e}（备份保留：{backup}）")
    })?;
    sync_parent_directory(src)?;
    drop(conn);

    let verification = (|| {
        let active = open_with_cipher_params(src, config.page_size, config.hmac_off)?;
        integrity_check(&active)?;
        verify_logical_copy(&backup_conn, &active)
    })();
    drop(backup_conn);
    if let Err(e) = verification {
        let restore = restore_database_from_backup(src, backup);
        return Err(format!(
            "sqlcipher migration {src}: replacement verification failed: {e}; restore result: {restore:?}"
        ));
    }
    write_migration_journal(src, backup, true)?;
    std::fs::remove_file(backup)
        .map_err(|e| format!("remove verified migration backup {backup}: {e}"))?;
    sync_parent_directory(src)?;
    remove_migration_journal(src)
}

pub(crate) fn checkpoint_truncate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("PRAGMA busy_timeout=5000;")
        .map_err(|e| e.to_string())?;
    let locking_mode: String = conn
        .query_row("PRAGMA locking_mode=EXCLUSIVE", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if !locking_mode.eq_ignore_ascii_case("exclusive") {
        return Err(format!(
            "could not lock database exclusively: {locking_mode}"
        ));
    }
    let (busy, log, checkpointed): (i64, i64, i64) = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(|e| e.to_string())?;
    if busy != 0 || (log >= 0 && checkpointed != log) {
        return Err(format!(
            "incomplete WAL checkpoint: busy={busy}, log={log}, checkpointed={checkpointed}"
        ));
    }
    Ok(())
}

fn remove_sidecars(path: &str) -> Result<(), String> {
    for suffix in ["-wal", "-shm"] {
        match std::fs::remove_file(format!("{path}{suffix}")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("remove database sidecar {path}{suffix}: {e}")),
        }
    }
    sync_parent_directory(path)
}

// ─── 加密参数迁移：页大小 + HMAC（sqlcipher_export 单链路） ────────────────────

/// 目标加密参数下打开文件库（`PRAGMA key` 首位 + 页大小 + 可选 HMAC 关闭）。
/// 对错误参数的库在真正读页时失败（SQLITE_NOTADB / HMAC 校验错），open 本身不炸。
fn open_with_cipher_params(
    path: impl AsRef<std::path::Path>,
    page: u32,
    hmac_off: bool,
) -> Result<Connection, String> {
    let conn = Connection::open(path.as_ref()).map_err(|e| e.to_string())?;
    let key = crate::crypt::master_key()?;
    let mut sql = format!(
        "PRAGMA key = \"x'{}'\"; PRAGMA cipher_page_size = {page};",
        hex_key(key)
    );
    if hmac_off {
        sql.push_str(" PRAGMA cipher_use_hmac = OFF;");
    }
    conn.execute_batch(&sql).map_err(|e| e.to_string())?;
    Ok(conn)
}

/// 加密库有效 `page_size`（SQLCipher 拦截该 pragma 返回 TEXT，同 maintenance.rs 口径）。
fn db_page_size(conn: &Connection, fallback: u32) -> u32 {
    conn.query_row("PRAGMA page_size", [], |r| {
        r.get::<_, rusqlite::types::Value>(0)
    })
    .ok()
    .and_then(|v| match v {
        rusqlite::types::Value::Integer(i) => u32::try_from(i).ok(),
        rusqlite::types::Value::Text(s) => s.parse().ok(),
        _ => None,
    })
    .unwrap_or(fallback)
}

/// 探测加密库当前 (页大小, HMAC 关闭?)：按已知组合逐一试开，`sqlite_master` 可读即命中
/// （错误参数读页即失败）。返回 conn 供调用方续读（如 setting 表）。
fn probe_cipher_params(path: &str) -> Result<(Connection, u32, bool), String> {
    const COMBOS: [(u32, bool); 4] = [
        (CIPHER_PAGE_SIZE, false),
        (4096, false),
        (CIPHER_PAGE_SIZE, true),
        (4096, true),
    ];
    for (page, hmac_off) in COMBOS {
        if let Ok(conn) = open_with_cipher_params(path, page, hmac_off)
            && conn
                .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
                    r.get::<_, i64>(0)
                })
                .is_ok()
        {
            let actual = db_page_size(&conn, page);
            return Ok((conn, actual, hmac_off));
        }
    }
    Err(format!(
        "sqlcipher probe {path}: 无法以任何已知页大小/HMAC 组合解密"
    ))
}

/// 配置缺失时使用默认；损坏或不可读时拒绝迁移，避免静默覆盖用户选择。
fn desired_cipher_config() -> Result<CipherConfig, String> {
    read_cipher_config()
}

fn is_main_db_path(path: &str) -> bool {
    std::path::Path::new(path)
        .file_name()
        .is_some_and(|n| n == std::ffi::OsStr::new("aidog.db"))
}

fn fallback_new_database_params(path: &str) -> Result<(u32, bool), String> {
    let database = std::path::Path::new(path);
    if !is_main_db_path(path)
        && let Some(main) = database.parent().map(|p| p.join("aidog.db"))
        && main.is_file()
        && !is_plaintext_sqlite(main.to_string_lossy().as_ref())
    {
        let (conn, page, hmac_off) = probe_cipher_params(&main.to_string_lossy())?;
        drop(conn);
        return Ok((page, hmac_off));
    }
    let config = CipherConfig::default();
    Ok((config.page_size, config.hmac_off))
}

/// 导出目标库 pragma（ATTACH 之后、export 之前对 cipher_mig 执行）。
fn cipher_target_pragmas(page: u32, hmac_off: bool) -> String {
    let mut s = format!("PRAGMA cipher_mig.cipher_page_size = {page};");
    if hmac_off {
        s.push_str(" PRAGMA cipher_mig.cipher_use_hmac = OFF;");
    }
    s
}

/// 在已打开的 src conn 上 ATTACH dst + 目标 pragma + `sqlcipher_export` + DETACH。
fn export_into(conn: &Connection, dst: &str, key_hex: &str, target: &str) -> Result<(), String> {
    let sql = format!(
        "ATTACH DATABASE '{}' AS cipher_mig KEY \"x'{key_hex}'\"; \
         {target} \
         SELECT sqlcipher_export('cipher_mig'); \
         DETACH DATABASE cipher_mig;",
        dst.replace('\'', "''")
    );
    conn.execute_batch(&sql)
        .map_err(|e| format!("sqlcipher_export failed: {e}"))
}

/// Verify SQLite page and b-tree integrity before comparing logical contents.
fn integrity_check(conn: &Connection) -> Result<(), String> {
    let check: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| format!("database integrity_check failed: {e}"))?;
    if check != "ok" {
        return Err(format!("database integrity_check = {check}"));
    }
    Ok(())
}

type SchemaObject = (String, String, String, Option<String>);

fn schema_objects(conn: &Connection) -> Result<Vec<SchemaObject>, String> {
    let mut stmt = conn
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name")
        .map_err(|e| e.to_string())?;
    stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('\"', "\"\""))
}

fn sync_file(path: &str) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|e| format!("sync database file {path}: {e}"))
}

#[cfg(unix)]
pub(crate) fn sync_parent_directory(path: &str) -> Result<(), String> {
    let file_path = std::path::Path::new(path);
    let parent = file_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|e| format!("sync database directory {}: {e}", parent.display()))
}

#[cfg(not(unix))]
fn sync_parent_directory(path: &str) -> Result<(), String> {
    Err(format!(
        "database migration refused: durable directory sync is unavailable for {path}"
    ))
}

fn copy_file_create_new(source: &str, target: &str) -> Result<(), String> {
    let mut input = File::open(source).map_err(|e| format!("open backup source {source}: {e}"))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options
        .open(target)
        .map_err(|e| format!("create unique database backup {target}: {e}"))?;
    std::io::copy(&mut input, &mut output)
        .map_err(|e| format!("copy database backup {source} to {target}: {e}"))?;
    output
        .sync_all()
        .map_err(|e| format!("sync database backup {target}: {e}"))?;
    sync_parent_directory(target)
}

fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Real(a), Value::Real(b)) => a.to_bits() == b.to_bits(),
        (Value::Text(a), Value::Text(b)) => a.as_bytes() == b.as_bytes(),
        (Value::Blob(a), Value::Blob(b)) => a == b,
        _ => false,
    }
}

fn implicit_rowid_alias(
    conn: &Connection,
    table: &str,
    columns: &[String],
) -> Result<Option<&'static str>, String> {
    let name = quote_identifier(table);
    for alias in ["rowid", "_rowid_", "oid"] {
        if columns
            .iter()
            .any(|column| column.eq_ignore_ascii_case(alias))
        {
            continue;
        }
        if conn
            .prepare(&format!("SELECT {alias} FROM {name} LIMIT 0"))
            .is_ok()
        {
            return Ok(Some(alias));
        }
    }
    let mut stmt = conn
        .prepare("PRAGMA table_list")
        .map_err(|e| e.to_string())?;
    let table_list = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let wr = table_list
        .into_iter()
        .find(|(schema, name, _)| schema == "main" && name == table)
        .map(|(_, _, wr)| wr == 1)
        .ok_or_else(|| format!("cannot determine rowid mode for table {table}"))?;
    if wr {
        Ok(None)
    } else {
        Err(format!("cannot access implicit rowid for table {table}"))
    }
}

fn primary_key_columns(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", quote_identifier(table)))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut columns = rows
        .into_iter()
        .filter(|(_, primary_key_order)| *primary_key_order > 0)
        .map(|(column, primary_key_order)| (primary_key_order, column))
        .collect::<Vec<_>>();
    columns.sort_by_key(|(order, _)| *order);
    if columns.is_empty() {
        return Err(format!("WITHOUT ROWID table {table} has no primary key"));
    }
    Ok(columns.into_iter().map(|(_, column)| column).collect())
}

fn compare_table_rows(left: &Connection, right: &Connection, table: &str) -> Result<(), String> {
    let name = quote_identifier(table);
    let columns = left
        .prepare(&format!("SELECT * FROM {name} LIMIT 0"))
        .map_err(|e| format!("inspect table {table}: {e}"))?
        .column_names()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let rowid = implicit_rowid_alias(left, table, &columns)?;
    let (selection, order, offset) = match rowid {
        Some(alias) => {
            let alias = quote_identifier(alias);
            (format!("{alias}, *"), alias, 1)
        }
        None => {
            let order = primary_key_columns(left, table)?
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", ");
            ("*".to_string(), order, 0)
        }
    };
    let query = format!("SELECT {selection} FROM {name} ORDER BY {order}");
    let mut left_stmt = left.prepare(&query).map_err(|e| e.to_string())?;
    let mut right_stmt = right.prepare(&query).map_err(|e| e.to_string())?;
    let mut left_rows = left_stmt.query([]).map_err(|e| e.to_string())?;
    let mut right_rows = right_stmt.query([]).map_err(|e| e.to_string())?;
    loop {
        match (
            left_rows.next().map_err(|e| e.to_string())?,
            right_rows.next().map_err(|e| e.to_string())?,
        ) {
            (None, None) => return Ok(()),
            (Some(a), Some(b)) => {
                for index in 0..(columns.len() + offset) {
                    let a: Value = a.get(index).map_err(|e| e.to_string())?;
                    let b: Value = b.get(index).map_err(|e| e.to_string())?;
                    if !same_value(&a, &b) {
                        return Err(format!("migration data differs in table {table}"));
                    }
                }
            }
            _ => return Err(format!("migration row count differs in table {table}")),
        }
    }
}

pub(crate) fn verify_logical_copy(source: &Connection, target: &Connection) -> Result<(), String> {
    if schema_objects(source)? != schema_objects(target)? {
        return Err("migration schema differs from source database".into());
    }
    let mut stmt = source
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .map_err(|e| e.to_string())?;
    let tables = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for table in tables {
        compare_table_rows(source, target, &table)?;
    }
    Ok(())
}

fn open_database(path: &str) -> Result<Connection, String> {
    let conn = if is_plaintext_sqlite(path) {
        Connection::open(path).map_err(|e| format!("open plaintext database {path}: {e}"))?
    } else {
        probe_cipher_params(path)?.0
    };
    integrity_check(&conn)?;
    Ok(conn)
}

fn migration_backup_paths(path: &str) -> Result<Vec<std::path::PathBuf>, String> {
    let database = std::path::Path::new(path);
    let name = database
        .file_name()
        .ok_or_else(|| format!("invalid database path: {path}"))?;
    let prefix = format!("{}.aidog-cipher-backup-", name.to_string_lossy());
    let dir = database
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read database directory {}: {e}", dir.display())),
    };
    let mut backups = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("read database directory {}: {e}", dir.display()))?;
        let filename = entry.file_name();
        let filename = filename.to_string_lossy();
        let Some(suffix) = filename.strip_prefix(&prefix) else {
            continue;
        };
        if let Ok(timestamp) = suffix.parse::<u128>() {
            backups.push((timestamp, entry.path()));
        }
    }
    backups.sort_by_key(|(timestamp, _)| std::cmp::Reverse(*timestamp));
    Ok(backups.into_iter().map(|(_, path)| path).collect())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MigrationJournal {
    backup_file: String,
    verified: bool,
}

fn migration_journal_path(path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{path}.aidog-cipher-migration.json"))
}

fn migration_journal_temp_path(path: &str) -> std::path::PathBuf {
    migration_journal_path(path).with_extension("json.tmp")
}

#[cfg(test)]
pub(crate) fn write_migration_journal_for_test(
    path: &str,
    backup: &str,
    verified: bool,
) -> Result<(), String> {
    write_migration_journal(path, backup, verified)
}

fn write_migration_journal(path: &str, backup: &str, verified: bool) -> Result<(), String> {
    let database = std::path::Path::new(path);
    let backup_path = std::path::Path::new(backup);
    if database.parent() != backup_path.parent() {
        return Err("migration journal backup must be in the database directory".into());
    }
    let backup_file = backup_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "migration journal backup name is invalid".to_string())?;
    if !migration_backup_paths(path)?
        .iter()
        .any(|candidate| candidate == backup_path)
    {
        return Err("migration journal backup name is not owned by this migration".into());
    }
    let journal_path = migration_journal_path(path);
    let temp_path = journal_path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(&MigrationJournal {
        backup_file: backup_file.to_string(),
        verified,
    })
    .map_err(|e| e.to_string())?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp_path)
        .map_err(|e| format!("write migration journal {}: {e}", temp_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("protect migration journal {}: {e}", temp_path.display()))?;
    }
    file.write_all(&bytes)
        .map_err(|e| format!("write migration journal {}: {e}", temp_path.display()))?;
    file.sync_all()
        .map_err(|e| format!("sync migration journal {}: {e}", temp_path.display()))?;
    std::fs::rename(&temp_path, &journal_path)
        .map_err(|e| format!("replace migration journal {}: {e}", journal_path.display()))?;
    sync_parent_directory(path)
}

fn read_migration_journal(
    path: &str,
) -> Result<Option<(MigrationJournal, std::path::PathBuf)>, String> {
    let journal_path = migration_journal_path(path);
    let bytes = match std::fs::read(&journal_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(format!(
                "read migration journal {}: {e}",
                journal_path.display()
            ));
        }
    };
    let journal: MigrationJournal = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid migration journal {}: {e}", journal_path.display()))?;
    let backup_path = journal_path
        .parent()
        .ok_or_else(|| {
            format!(
                "migration journal has no parent: {}",
                journal_path.display()
            )
        })?
        .join(&journal.backup_file);
    if backup_path.file_name().and_then(|name| name.to_str()) != Some(journal.backup_file.as_str())
        || !migration_backup_paths(path)?
            .iter()
            .any(|candidate| candidate == &backup_path)
    {
        return Err(format!(
            "migration journal {} names an invalid backup",
            journal_path.display()
        ));
    }
    Ok(Some((journal, backup_path)))
}

fn remove_migration_journal(path: &str) -> Result<(), String> {
    let mut removed = false;
    for journal_path in [
        migration_journal_path(path),
        migration_journal_temp_path(path),
    ] {
        match std::fs::remove_file(&journal_path) {
            Ok(()) => removed = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "remove migration journal {}: {e}",
                    journal_path.display()
                ));
            }
        }
    }
    if removed {
        sync_parent_directory(path)
    } else {
        Ok(())
    }
}

fn verify_same_database(source: &str, target: &str) -> Result<(), String> {
    let source_conn = open_database(source)?;
    let target_conn = open_database(target)?;
    verify_logical_copy(&source_conn, &target_conn)
}

fn restore_database_from_backup(path: &str, backup: &str) -> Result<(), String> {
    open_database(backup)?;
    remove_sidecars(path)?;
    std::fs::copy(backup, path)
        .map_err(|e| format!("restore database {path} from {backup}: {e}"))?;
    sync_file(path)?;
    sync_parent_directory(path)?;
    remove_sidecars(path)?;
    verify_same_database(backup, path)
}

fn remove_migration_backups(path: &str, backups: &[std::path::PathBuf]) -> Result<(), String> {
    for backup in backups {
        match std::fs::remove_file(backup) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!("remove migration backup {}: {e}", backup.display()));
            }
        }
    }
    sync_parent_directory(path)
}

fn recover_database_if_needed(path: &str) -> Result<(), String> {
    if let Some((journal, backup)) = read_migration_journal(path)? {
        if !backup.exists() {
            if journal.verified && open_database(path).is_ok() {
                return remove_migration_journal(path);
            }
            return Err(format!(
                "incomplete migration journal for {path} references missing backup {}; preserving database",
                backup.display()
            ));
        }
        open_database(&backup.to_string_lossy()).map_err(|e| {
            format!(
                "migration journal backup {} is not valid; preserving active database: {e}",
                backup.display()
            )
        })?;
        let active_matches = verify_same_database(&backup.to_string_lossy(), path).is_ok();
        if !active_matches {
            restore_database_from_backup(path, &backup.to_string_lossy())?;
        }
        remove_migration_backups(path, std::slice::from_ref(&backup))?;
        return remove_migration_journal(path);
    }

    let backups = migration_backup_paths(path)?;
    if backups.is_empty() {
        return remove_migration_journal(path);
    }
    if open_database(path).is_err() {
        return Err(format!(
            "database {path} is unreadable and orphaned migration backups have no journal; preserving both for manual recovery"
        ));
    }
    for backup in &backups {
        open_database(&backup.to_string_lossy()).map_err(|e| {
            format!(
                "orphaned migration backup {} cannot be validated; preserving both files: {e}",
                backup.display()
            )
        })?;
        verify_same_database(&backup.to_string_lossy(), path).map_err(|e| {
            format!(
                "healthy database {path} differs from orphaned backup {}; preserving both files and refusing rollback: {e}",
                backup.display()
            )
        })?;
    }
    remove_migration_backups(path, &backups)?;
    remove_migration_journal(path)
}

/// 测试入口：获取迁移锁并按库内实测参数执行重写。
#[cfg(test)]
pub(crate) fn migrate_cipher_params(
    path: &str,
    dst: &str,
    _cur_page: u32,
    _cur_hmac_off: bool,
    desired: CipherConfig,
) -> Result<(u32, bool), String> {
    let _migration_lock = cipher_migration_lock()?;
    recover_database_if_needed(path)?;
    migrate_cipher_params_locked(path, dst, desired)
}

fn migrate_cipher_params_locked(
    path: &str,
    dst: &str,
    desired: CipherConfig,
) -> Result<(u32, bool), String> {
    let (conn, cur_page, cur_hmac_off) = probe_cipher_params(path)?;
    drop(conn);
    if cur_page == desired.page_size && cur_hmac_off == desired.hmac_off {
        return Ok((cur_page, cur_hmac_off));
    }
    sync_parent_directory(path)?;
    let key = crate::crypt::master_key()?;
    let hex = hex_key(key);
    // 清理上次失败的半成品残留。
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{dst}{suffix}"));
    }
    let conn = open_with_cipher_params(path, cur_page, cur_hmac_off)
        .map_err(|e| format!("sqlcipher migration {path}: open failed: {e}"))?;
    checkpoint_truncate(&conn)
        .map_err(|e| format!("sqlcipher migration {path}: checkpoint failed: {e}"))?;
    // Checkpoint succeeds before the main-file backup; the backup includes committed WAL frames.
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let bak = format!("{path}.aidog-cipher-backup-{ts}");
    copy_file_create_new(path, &bak)
        .map_err(|e| format!("sqlcipher migration {path}: backup failed: {e}"))?;
    let backup_conn = open_with_cipher_params(&bak, cur_page, cur_hmac_off)
        .map_err(|e| format!("sqlcipher migration backup open failed: {e}"))?;
    integrity_check(&backup_conn)?;
    verify_logical_copy(&conn, &backup_conn)?;
    write_migration_journal(path, &bak, false)?;

    if let Err(e) = export_into(
        &conn,
        dst,
        &hex,
        &cipher_target_pragmas(desired.page_size, desired.hmac_off),
    ) {
        drop(conn);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{dst}{suffix}"));
        }
        return Err(format!(
            "sqlcipher migration {path}: {e}（.bak 保留：{bak}）"
        ));
    }
    sync_file(dst)?;
    sync_parent_directory(dst)?;
    let target_conn = open_with_cipher_params(dst, desired.page_size, desired.hmac_off)?;
    integrity_check(&target_conn)?;
    verify_logical_copy(&conn, &target_conn)?;
    drop(target_conn);
    remove_sidecars(path)?;
    std::fs::rename(dst, path).map_err(|e| {
        format!("sqlcipher migration {path}: replace failed: {e}（备份保留：{bak}）")
    })?;
    sync_parent_directory(path)?;
    drop(conn);
    let verification = verify_same_database(&bak, path);
    drop(backup_conn);
    if let Err(e) = verification {
        let restore = restore_database_from_backup(path, &bak);
        return Err(format!(
            "sqlcipher migration {path}: replacement verification failed: {e}; restore result: {restore:?}"
        ));
    }
    write_migration_journal(path, &bak, true)?;
    std::fs::remove_file(&bak)
        .map_err(|e| format!("remove verified migration backup {bak}: {e}"))?;
    sync_parent_directory(path)?;
    remove_migration_journal(path)?;
    Ok((desired.page_size, desired.hmac_off))
}

/// 进程级缓存：path → 有效 (页大小, hmac_off)。迁移每进程只尝试一次（失败即用当前参数，
/// 下次启动重试），避免每次 open 重做整库备份拷贝。
fn params_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, (u32, bool)>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, (u32, bool)>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 清空进程级参数缓存（测试模拟「下次启动」用；生产生命周期 = 进程）。
#[cfg(test)]
pub(crate) fn clear_params_cache() {
    params_cache().lock().expect("params cache lock").clear();
}

/// 缓存壳：锁只在查/插瞬间持有（uncached 内部可能递归 ensure 主库，不可持锁重入）。
fn ensure_cipher_params(path: &str) -> Result<(u32, bool), String> {
    if is_memory_path(path) {
        return Ok((CIPHER_PAGE_SIZE, false));
    }
    {
        let cache = params_cache().lock().expect("params cache lock");
        if let Some(v) = cache.get(path) {
            return Ok(*v);
        }
    }
    let v = ensure_cipher_params_uncached(path)?;
    params_cache()
        .lock()
        .expect("params cache lock")
        .insert(path.to_string(), v);
    Ok(v)
}

/// 探测当前参数，与 `~/.aidog/db-cipher.json` 目标核对；需要时整库重写。
/// 配置损坏时加密库按实测参数打开、不迁移、不覆写配置；明文库迁移会拒绝。
fn ensure_cipher_params_uncached(path: &str) -> Result<(u32, bool), String> {
    let _migration_lock = cipher_migration_lock()?;
    recover_database_if_needed(path)?;
    if !std::path::Path::new(path).is_file() {
        return match desired_cipher_config() {
            Ok(desired) => {
                if let Err(e) = write_cipher_config(desired) {
                    tracing::warn!(error = %e, "write db cipher config failed");
                }
                Ok((desired.page_size, desired.hmac_off))
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path, "invalid DB cipher config; creating database with fallback parameters without rewriting config");
                fallback_new_database_params(path)
            }
        };
    }
    let (conn, cur_page, cur_hmac_off) = probe_cipher_params(path)?;
    drop(conn);
    let desired = match desired_cipher_config() {
        Ok(config) => config,
        Err(e) => {
            tracing::warn!(error = %e, path = %path, "invalid DB cipher config; opening with detected database parameters without migration");
            return Ok((cur_page, cur_hmac_off));
        }
    };
    if cur_page == desired.page_size && cur_hmac_off == desired.hmac_off {
        if let Err(e) = write_cipher_config(desired) {
            tracing::warn!(error = %e, "write db cipher config failed");
        }
        return Ok((cur_page, cur_hmac_off));
    }
    match migrate_cipher_params_locked(path, &format!("{path}.cipher-mig2"), desired) {
        Ok(p) => {
            if let Err(e) = write_cipher_config(desired) {
                tracing::warn!(error = %e, "write db cipher config failed after migration");
            }
            Ok(p)
        }
        Err(e) => {
            tracing::warn!(error = %e, path = %path, "cipher params migration failed; retaining current database format");
            recover_database_if_needed(path)?;
            let (conn, actual_page, actual_hmac_off) =
                probe_cipher_params(path).map_err(|probe| {
                    format!("migration failed and current database is unreadable: {e}; {probe}")
                })?;
            drop(conn);
            Ok((actual_page, actual_hmac_off))
        }
    }
}
