//! OpenCode 读取器（issue #65 票 06）：SQLite database 与 legacy JSON store 两形态。
//!
//! 格式核对（官方源码，2026-11-20 取证，开发机未装 OpenCode）：
//! - 数据根 = `$XDG_DATA_HOME/opencode`（缺省 `~/.local/share/opencode`）
//!   （sst/opencode v2.3.2 `packages/util/src/global-roots.ts`）。
//! - db 形态：`<root>/opencode.db`（v2.3.2 `packages/cli/src/database-path.ts:5-11`，WAL）。
//!   表两代并存，按代取用：
//!   - 新代：`session_v2`（id / parent_id / directory / title）+ `session_message`
//!     （id / session_id / type / seq / data=消息 JSON）（v2.3.2 `schema.gen.ts:224-290`）。
//!   - 旧代（1.x sqlite 时代）：`session` / `message`（id / session_id / data）。
//!   - 消息 JSON 两代同构 = Assistant：`type:"assistant"`、`model:{id,providerID}`、
//!     `tokens:{input,output,reasoning,cache:{read,write}}`、`time:{created}`；
//!     `time.created` 为 ISO 字符串（DateTimeUtcFromMillis）或旧版毫秒数，两收。
//!   - 子会话 = session `parent_id` 非空，调用归属根会话（父链行走，防环）。
//!   - 代判定：`session_v2` 表存在 → 新代；否则旧代。
//! - JSON store（1.x 及更早；**有 db 时只读 db 不双读**——db 形态版本不再写 JSON store，
//!   双读会重复计数）：
//!   - 全局布局：`storage/session/<projectID>/<sessionID>.json`（会话信息）+
//!     `storage/message/<sessionID>/<msgID>.json`（消息）
//!     （v1.19.6 `packages/opencode/src/storage/storage.ts:154,196`）。
//!   - per-project 布局：`project/<dir>/storage/session/info/<sessionID>.json` +
//!     `project/<dir>/storage/session/message/<sessionID>/<msgID>.json`
//!     （v1.19.6 storage.ts 迁移 glob）。
//!   - 会话信息 JSON：`id` / `parentID`（旧 `parent_id` 也收）/ `directory` / `title`。
//! - 折算：模型按名匹配（`model_entry_for_billing` 平台 miss → any-platform 官方优先），
//!   平台码传空串即走该链；条目缺失或无有效价 → None = 未定价，不回落兜底单价。
//! - sqlite 只读打开（`SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`），零写入零写侧 PRAGMA。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::reader::ScanErr;

/// 生产根：XDG 数据目录下的 opencode 目录；`opencode.db` 存在时返回 db 文件
/// （单文件形态走同一文件级进度 / 对齐语义）。
pub fn preferred_root() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local").join("share"));
    let dir = data.join("opencode");
    let db = dir.join("opencode.db");
    Some(if db.is_file() { db } else { dir })
}

/// 一次扫描的产物：调用明细（positions[i] 与 entries[i] 一一对应）+ 涉及的会话元数据。
#[derive(Default)]
pub struct ScanOutOc {
    pub entries: Vec<super::reader::UsageLine>,
    pub positions: Vec<i64>,
    pub sessions: Vec<SessionMeta>,
    pub parse_errors: u64,
    pub file_size: i64,
    pub mtime_ms: i64,
    pub first_ms: i64,
    pub last_ms: i64,
}

/// 会话元数据（入库 upsert_client_session 用；first/last 由本扫描段消息时刻填充）。
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub workdir: String,
    pub first_ms: i64,
    pub last_ms: i64,
}

fn file_meta_ms(path: &Path, out: &mut ScanOutOc) -> Result<(), std::io::Error> {
    let meta = std::fs::metadata(path)?;
    out.file_size = meta.len() as i64;
    out.mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Ok(())
}

/// RFC3339 ISO 字符串或毫秒数 → 毫秒；都解析失败 → 0。
fn ts_ms(v: &serde_json::Value) -> i64 {
    match v {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
        serde_json::Value::String(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map(|d| d.timestamp_millis())
            .unwrap_or(0),
        _ => 0,
    }
}

/// 父链行走取根会话（防环）。
fn root_session(id: &str, parents: &HashMap<String, String>) -> String {
    let mut cur = id;
    let mut seen = std::collections::HashSet::new();
    while seen.insert(cur.to_string()) && parents.get(cur).is_some_and(|p| !p.is_empty()) {
        cur = parents[cur].as_str();
    }
    cur.to_string()
}

/// Assistant 消息 JSON → UsageLine（非 assistant / 无 tokens / 全零用量不计）。
fn parse_message(v: &serde_json::Value, sid: &str, sidechain: bool, pos: i64, out: &mut ScanOutOc) {
    if v.get("type").and_then(|x| x.as_str()) != Some("assistant") {
        return;
    }
    let Some(tokens) = v.get("tokens").filter(|t| t.is_object()) else {
        return;
    };
    let get = |ks: &[&str]| -> i64 {
        ks.iter()
            .try_fold(tokens, |v, k| v.get(*k))
            .and_then(|x| x.as_i64())
            .unwrap_or(0)
    };
    let (input, output, reasoning, cr, cw) = (
        get(&["input"]),
        get(&["output"]),
        get(&["reasoning"]),
        get(&["cache", "read"]),
        get(&["cache", "write"]),
    );
    if input + output + reasoning + cr + cw == 0 {
        return;
    }
    // 模型名：`model:{id,providerID}`（1.x/2.x）或旧版顶层 `modelID`。
    let model = v
        .pointer("/model/id")
        .or_else(|| v.get("modelID"))
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let time = v.pointer("/time/created").map(ts_ms).unwrap_or(0);
    out.entries.push(super::reader::UsageLine {
        session_id: sid.to_string(),
        is_sidechain: sidechain,
        call_time_ms: time,
        model: model.to_string(),
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: cr,
        cache_write_tokens: cw,
        reasoning_tokens: reasoning,
    });
    out.positions.push(pos);
    if time > 0 {
        if out.first_ms == 0 || time < out.first_ms {
            out.first_ms = time;
        }
        out.last_ms = out.last_ms.max(time);
    }
}

fn push_json_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            push_json_files(&p, files);
        } else if p.extension().and_then(|x| x.to_str()) == Some("json") {
            files.push(p);
        }
    }
}

/// 枚举消息文件（两种 JSON 布局）：
/// - 全局：`storage/message/<sid>/<msg>.json`
/// - per-project：`project/<dir>/storage/session/message/<sid>/<msg>.json`
pub fn enumerate_json_messages(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    push_json_files(&root.join("storage").join("message"), &mut files);
    if let Ok(rd) = std::fs::read_dir(root.join("project")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                push_json_files(&p.join("storage").join("session").join("message"), &mut files);
            }
        }
    }
    files.sort();
    files
}

/// 枚举会话信息文件：
/// - 全局：`storage/session/<projectID>/<sessionID>.json`
/// - per-project：`project/<dir>/storage/session/info/<sessionID>.json`
fn enumerate_session_infos(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    push_json_files(&root.join("storage").join("session"), &mut files);
    if let Ok(rd) = std::fs::read_dir(root.join("project")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                push_json_files(&p.join("storage").join("session").join("info"), &mut files);
            }
        }
    }
    files.sort();
    files
}

/// 会话信息 JSON → (id, parentID, title, directory)；parentID 旧键 parent_id 兼容。
fn session_info(v: &serde_json::Value) -> (String, String, String, String) {
    let g = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    };
    let parent = g("parentID");
    let parent = if parent.is_empty() { g("parent_id") } else { parent };
    (g("id"), parent, g("title"), g("directory"))
}

/// 读全部会话信息构建 父链 + 会话元数据表。
fn load_session_map(
    root: &Path,
) -> (
    HashMap<String, String>,
    HashMap<String, (String, String)>,
) {
    let mut parents = HashMap::new();
    let mut meta = HashMap::new();
    for p in enumerate_session_infos(root) {
        let Some(v) = std::fs::read(&p)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        else {
            continue;
        };
        let (id, parent, title, dir) = session_info(&v);
        if !id.is_empty() {
            parents.insert(id.clone(), parent);
            meta.insert(id, (title, dir));
        }
    }
    (parents, meta)
}

/// JSON store 扫描：单个消息文件 = 单条消息（file_pos 恒 0）。
pub fn scan_json_file(path: &Path, root: &Path) -> Result<ScanOutOc, std::io::Error> {
    let mut out = ScanOutOc::default();
    file_meta_ms(path, &mut out)?;
    let sid = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    if sid.is_empty() {
        return Err(std::io::Error::other("message file has no session dir"));
    }
    let (parents, meta) = load_session_map(root);
    let root_sid = root_session(&sid, &parents);
    let bytes = std::fs::read(path)?;
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        out.parse_errors += 1;
        return Ok(out);
    };
    parse_message(
        &v,
        &root_sid,
        parents.get(&sid).is_some_and(|p| !p.is_empty()),
        0,
        &mut out,
    );
    if let Some((title, dir)) = meta.get(&root_sid) {
        out.sessions.push(SessionMeta {
            id: root_sid,
            title: title.clone(),
            workdir: dir.clone(),
            first_ms: out.first_ms,
            last_ms: out.last_ms,
        });
    }
    Ok(out)
}

/// db 形态扫描：整库只读单连接读取（本次扫描段覆盖全部会话）。
/// 表代判定 `session_v2` 表存在 → 新代；否则旧代 `session` / `message`。
pub fn scan_db_file(path: &Path) -> Result<ScanOutOc, ScanErr> {
    let mut out = ScanOutOc::default();
    file_meta_ms(path, &mut out)?;
    scan_db_inner(path, &mut out)
        .map_err(|e| ScanErr::Io(std::io::Error::other(e.to_string())))?;
    Ok(out)
}

/// db 形态内层（rusqlite 错误面；外层统一转 ScanErr）。
fn scan_db_inner(path: &Path, out: &mut ScanOutOc) -> rusqlite::Result<()> {
    use rusqlite::OpenFlags;
    let conn = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let has_v2 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='session_v2'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    let (session_tbl, message_tbl) = if has_v2 {
        ("session_v2", "session_message")
    } else {
        ("session", "message")
    };
    // 会话父链 + 元数据。
    let sql = format!(
        "SELECT id, parent_id, title, directory FROM {session_tbl} ORDER BY id"
    );
    let mut parents: HashMap<String, String> = HashMap::new();
    let mut meta: HashMap<String, (String, String)> = HashMap::new();
    {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            ))
        })?;
        for row in rows {
            let (id, parent, title, dir) = row?;
            parents.insert(id.clone(), parent);
            meta.insert(id, (title, dir));
        }
    }
    // 会话 first/last 聚合（仅由消息时刻填充，与其他读取器同口径）。
    let mut span: HashMap<String, (i64, i64)> = HashMap::new();
    let sql = format!("SELECT rowid, session_id, data FROM {message_tbl} ORDER BY rowid");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (rowid, sid, data) = row?;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) else {
            out.parse_errors += 1;
            continue;
        };
        let root_sid = root_session(&sid, &parents);
        let sidechain = parents.get(&sid).is_some_and(|p| !p.is_empty());
        parse_message(&v, &root_sid, sidechain, rowid, out);
        if let Some(t) = v.pointer("/time/created").map(ts_ms).filter(|t| *t > 0) {
            let e = span.entry(root_sid).or_insert((t, t));
            e.0 = e.0.min(t);
            e.1 = e.1.max(t);
        }
    }
    // 只为有消息的根会话产出会话元数据（无消息会话不成壳，与其他读取器同口径）。
    let mut sids: Vec<&String> = span.keys().collect();
    sids.sort();
    for sid in sids {
        let (first_ms, last_ms) = span[sid];
        let (title, dir) = meta.get(sid).cloned().unwrap_or_default();
        out.sessions.push(SessionMeta {
            id: sid.clone(),
            title,
            workdir: dir,
            first_ms,
            last_ms,
        });
    }
    Ok(())
}

/// 枚举该客户端本次要读的「文件」：db 形态 = [opencode.db]（root 为 db 文件或其父目录
/// 均可，db 存在即优先）；JSON store = 消息文件集。
pub fn enumerate(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }
    let db = root.join("opencode.db");
    if db.is_file() {
        return vec![db];
    }
    enumerate_json_messages(root)
}

/// 会话是否存在（票 07：`opencode -s <id>` 在 id 不存在时会静默新建会话，继续前先确认）。
/// root 同 [`enumerate`]：db 文件或其父目录；db 形态按代查 `session_v2` / `session`，
/// JSON store 查会话信息文件；只读，打不开 / 查不到一律 false。
pub fn session_exists(root: &Path, session_id: &str) -> bool {
    let db = if root.is_file() { root.to_path_buf() } else { root.join("opencode.db") };
    if db.is_file() {
        return db_session_exists(&db, session_id).unwrap_or(false);
    }
    enumerate_session_infos(root).iter().any(|p| {
        std::fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .is_some_and(|v| session_info(&v).0 == session_id)
    })
}

fn db_session_exists(path: &Path, session_id: &str) -> rusqlite::Result<bool> {
    use rusqlite::OpenFlags;
    let conn = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let has_v2: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='session_v2'",
        [],
        |r| r.get(0),
    )?;
    let tbl = if has_v2 > 0 { "session_v2" } else { "session" };
    let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {tbl} WHERE id = ?1"), [session_id], |r| r.get(0))?;
    Ok(n > 0)
}

/// 按形态分派扫描（spawn_blocking 内调用的同步面）。
pub fn scan(path: &Path, root: &Path) -> Result<ScanOutOc, String> {
    if path.extension().and_then(|x| x.to_str()) == Some("db") {
        scan_db_file(path).map_err(|e| e.to_string())
    } else {
        scan_json_file(path, root).map_err(|e| e.to_string())
    }
}
