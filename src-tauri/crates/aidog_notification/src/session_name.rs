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
    let get = |k: &str| vars.get(k).map(|s| s.trim()).filter(|s| !s.is_empty());
    // hook 脚本把 stdin 的 session_id 以 `session` 键发出（scripts.rs 通用透传跳过 session_id），
    // 直接带 session_id 的调用方（测试 / 其它脚本）同样认。
    let (Some(sid), Some(path)) = (get("session_id").or_else(|| get("session")), get("transcript_path")) else {
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
    let prev2 = prev.clone();
    let scanned = tokio::task::spawn_blocking(move || scan_transcript(&path, prev2)).await;
    let next = match scanned {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            tracing::debug!(error = %e, "session_name: read transcript failed");
            return String::new();
        }
        Err(e) => {
            tracing::debug!(error = %e, "session_name: scan task failed");
            return String::new();
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
