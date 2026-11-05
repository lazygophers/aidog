//! 会话名解析（Claude Code）：从 transcript JSONL 取 `/rename` 名或自动标题，结果缓存进主库。
//!
//! 行格式为本机观察（未文档化）：`{"type":"custom-title","customTitle":"…"}` /
//! `{"type":"ai-title","aiTitle":"…"}`。优先级 custom-title > ai-title > 无。
//! 任何一步失败 → 空名（通知照发），只记 debug 日志。

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use aidog_db::Db;
use serde::{Deserialize, Serialize};

pub(crate) const CLIENT_CLAUDE: &str = "claude_code";
pub(crate) const CLIENT_CODEX: &str = "codex";
const MAX_CHARS: usize = 24;

/// 缓存 value（JSON；读取方忽略未知键）。`offset` 为已消费的 transcript 字节数。
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ClaudeCache {
    #[serde(default)]
    pub name: String,
    /// `custom_title` / `ai_title` / 空。
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub offset: u64,
}

/// 首行、去首尾空白，按 Unicode 字符计 24，超出截断加「…」。
/// 先整体 trim 再取首行，避免首行为空时丢掉后面的有效内容。
pub(crate) fn truncate_name(raw: &str) -> String {
    let line = raw.trim().lines().next().unwrap_or("").trim();
    match line.char_indices().nth(MAX_CHARS) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

/// 从 `prev.offset` 起增量读 transcript，返回更新后的缓存。文件变短则从头重读；坏行跳过。
/// 末行无换行且不是合法 JSON（可能写了一半）→ 不消费，留待下次。
pub(crate) fn scan_transcript(path: &Path, prev: ClaudeCache) -> std::io::Result<ClaudeCache> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut c = if len < prev.offset { ClaudeCache::default() } else { prev };
    if len == c.offset {
        return Ok(c);
    }
    f.seek(SeekFrom::Start(c.offset))?;
    let mut buf = Vec::with_capacity((len - c.offset) as usize);
    f.read_to_end(&mut buf)?;
    let end = if buf.ends_with(b"\n")
        || buf.rsplit(|b| *b == b'\n').next().is_some_and(|t| serde_json::from_slice::<serde_json::Value>(t).is_ok())
    {
        buf.len()
    } else {
        buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1)
    };
    for line in buf[..end].split(|b| *b == b'\n') {
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) else { continue };
        let (key, source) = match v.get("type").and_then(|t| t.as_str()) {
            Some("custom-title") => ("customTitle", "custom_title"),
            Some("ai-title") if c.source != "custom_title" => ("aiTitle", "ai_title"),
            _ => continue,
        };
        let name = truncate_name(v.get(key).and_then(|t| t.as_str()).unwrap_or(""));
        if !name.is_empty() {
            c.name = name;
            c.source = source.to_string();
        }
    }
    c.offset += end as u64;
    Ok(c)
}

/// 解析会话名并刷新缓存；`vars` 缺 `session_id` / `transcript_path` 时返回已有 `session_name` 或空串。
/// 每次都重新检查来源（增量读），缓存只省重复全量读。失败 → 空串 + debug 日志。
pub(crate) async fn resolve(db: &Db, vars: &HashMap<String, String>) -> String {
    resolve_in(db, vars, &projects_root()).await
}

fn projects_root() -> std::path::PathBuf {
    // 测试：fixture 在 tempdir，不碰真 ~/.claude。
    #[cfg(test)]
    return std::env::temp_dir();
    #[cfg(not(test))]
    dirs::home_dir().unwrap_or_default().join(".claude").join("projects")
}

/// `root` 为允许读取的 transcript 根目录（生产 = `~/.claude/projects`，测试注入 tempdir）。
pub(crate) async fn resolve_in(db: &Db, vars: &HashMap<String, String>, root: &Path) -> String {
    let get = |k: &str| vars.get(k).map(|s| s.trim()).filter(|s| !s.is_empty());
    // hook 脚本把 stdin 的 session_id 以 `session` 键发出（scripts.rs 通用透传跳过 session_id），
    // 直接带 session_id 的调用方（测试 / 其它脚本）同样认。
    let (Some(sid), Some(path)) = (get("session_id").or_else(|| get("session")), get("transcript_path")) else {
        if get("thread_id").is_some()
            && let Some(home) = codex_home()
        {
            return resolve_codex(db, vars, &home).await;
        }
        return vars.get("session_name").cloned().unwrap_or_default();
    };
    let cached_json = aidog_db::get_session_name_cache(db, CLIENT_CLAUDE, sid)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "session_name: read cache failed");
            None
        });
    let prev: ClaudeCache = cached_json
        .as_deref()
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_default();
    let path = std::path::PathBuf::from(path);
    // /api/notify 的 vars 来自本机任意调用方：只读 root 下、无 `..` 的文件，其余当无名。
    if !path.starts_with(root) || path.components().any(|c| c == std::path::Component::ParentDir) {
        tracing::debug!("session_name: transcript_path outside root, skipped");
        return String::new();
    }
    let prev2 = prev.clone();
    let scanned = tokio::task::spawn_blocking(move || scan_transcript(&path, prev2)).await;
    let next = match scanned {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            tracing::debug!(error = %e, "session_name: read transcript failed");
            return prev.name;
        }
        Err(e) => {
            tracing::debug!(error = %e, "session_name: scan task failed");
            return prev.name;
        }
    };
    if next != prev
        && let Ok(json) = serde_json::to_string(&next)
        && let Err(e) = aidog_db::put_session_name_cache(db, CLIENT_CLAUDE, sid, &json).await
    {
        tracing::debug!(error = %e, "session_name: write cache failed");
    }
    next.name
}

/// Codex 家目录：`CODEX_HOME` 优先（openai/codex `utils/home-dir`），否则 `~/.codex`。
fn codex_home() -> Option<std::path::PathBuf> {
    std::env::var_os("CODEX_HOME")
        .filter(|v| !v.is_empty())
        .map(Into::into)
        .or_else(|| dirs::home_dir().map(|h| h.join(".codex")))
}

/// `session_index.jsonl`（追加写，字段 `id` / `thread_name`）里该线程最后一条非空名；坏行跳过。
fn codex_rename(home: &Path, tid: &str) -> Option<String> {
    let text = std::fs::read_to_string(home.join("session_index.jsonl")).ok()?;
    text.lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v.get("id").and_then(|x| x.as_str()) == Some(tid))
        .filter_map(|v| v.get("thread_name").and_then(|x| x.as_str()).map(truncate_name))
        .find(|n| !n.is_empty())
}

/// 只读打开版本号最大的 `state_N.sqlite`（WAL 库），取 `threads.title`（首句提问原文）。
fn codex_title(home: &Path, tid: &str) -> Option<String> {
    let db = std::fs::read_dir(home)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n = e.file_name().into_string().ok()?;
            let v: u32 = n.strip_prefix("state_")?.strip_suffix(".sqlite")?.parse().ok()?;
            Some((v, e.path()))
        })
        .max_by_key(|(v, _)| *v)?
        .1;
    let c = rusqlite::Connection::open_with_flags(
        db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    c.busy_timeout(std::time::Duration::from_millis(200)).ok()?;
    let t: String = c
        .query_row("SELECT title FROM threads WHERE id = ?1", [tid], |r| r.get(0))
        .ok()?;
    Some(truncate_name(&t)).filter(|n| !n.is_empty())
}

/// Codex：`/rename` 名（`session_index.jsonl`）> 线程标题（`state_*.sqlite` `threads.title`）> 无。
/// 每次重查来源，名字变了才写缓存；任何来源缺失 / 打不开当无名。
pub(crate) async fn resolve_codex(db: &Db, vars: &HashMap<String, String>, home: &Path) -> String {
    let Some(tid) = vars.get("thread_id").map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let (h, t) = (home.to_path_buf(), tid.to_string());
    let found = tokio::task::spawn_blocking(move || {
        codex_rename(&h, &t)
            .map(|n| (n, "codex_rename"))
            .or_else(|| codex_title(&h, &t).map(|n| (n, "codex_title")))
    })
    .await
    .ok()
    .flatten();
    let Some((name, source)) = found else {
        tracing::debug!("session_name: codex name not found");
        return String::new();
    };
    let json = serde_json::json!({ "name": name, "source": source }).to_string();
    // 缓存只写不读：仅用于「名字没变就不重写」的比较与排查，运行时不据此回落（表结构由 spec 定）。
    let same = aidog_db::get_session_name_cache(db, CLIENT_CODEX, tid)
        .await
        .ok()
        .flatten()
        .is_some_and(|c| c == json);
    if !same
        && let Err(e) = aidog_db::put_session_name_cache(db, CLIENT_CODEX, tid, &json).await
    {
        tracing::debug!(error = %e, "session_name: write codex cache failed");
    }
    name
}
