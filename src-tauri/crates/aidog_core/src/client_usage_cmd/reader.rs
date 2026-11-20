//! Claude Code transcript JSONL 读取器（issue #65 票 02）。
//!
//! 所核格式 = 本机真实会话文件抽样（Claude Code 2.1.327，2026-11-20）：
//! - `type:"assistant"` 行：`message.usage`（`input_tokens` / `output_tokens` /
//!   `cache_read_input_tokens` / `cache_creation_input_tokens`（= cache write 总量）/
//!   `output_tokens_details.thinking_tokens`）、`message.model`、`message.id`、行级
//!   `timestamp`（RFC3339 → 毫秒）、`sessionId`、`isSidechain`、`cwd`。
//! - 同一 `message.id` 可分多行（流式续行追加），首个 usage 行计一次，后续同 id 跳过：
//!   跨扫描靠 `client_file_progress.last_msg_id` 持久化「上一已计 id」（延续行紧随已计行）。
//! - 不存对话原文：只落 token / 成本 / 定位键元数据（schema_client_usage.rs 头注释同口径）。
//! - title 行（custom-title / ai-title）与首条用户提示（标题回落，截 60 字仅内存截取）同文件扫描。

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use aidog_db::truncate_name;

/// 截断 / 替换信号：文件比已读偏移小（调用方删状态重读）。
#[derive(Debug)]
pub enum ScanErr {
    Truncated,
    Io(std::io::Error),
}

impl From<std::io::Error> for ScanErr {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            ScanErr::Truncated
        } else {
            ScanErr::Io(e)
        }
    }
}

impl std::fmt::Display for ScanErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanErr::Truncated => write!(f, "file truncated"),
            ScanErr::Io(e) => write!(f, "{e}"),
        }
    }
}

/// 一行 assistant usage 的解析产物（file_pos 由扫描器按字节偏移盖章）。
#[derive(Debug, Clone)]
pub struct UsageLine {
    pub session_id: String,
    pub is_sidechain: bool,
    pub call_time_ms: i64,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
}

/// 单文件扫描产物。
#[derive(Debug, Default)]
pub struct ScanOut {
    pub entries: Vec<UsageLine>,
    pub positions: Vec<i64>,
    pub new_offset: u64,
    pub last_msg_id: String,
    pub parse_errors: u64,
    pub file_size: i64,
    pub mtime_ms: i64,
    pub session_id: String,
    pub workdir: String,
    pub title: Option<(String, String)>,
    pub first_user_prompt: Option<String>,
    pub first_ms: i64,
    pub last_ms: i64,
}

fn json_i64(v: &serde_json::Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

/// RFC3339 → 毫秒；解析失败 → 0（落库后由 now() 盖章，误差可容忍）。
fn ts_ms(v: &serde_json::Value) -> i64 {
    v.get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
}

/// 用户行可作标题的提示文本：`message.content` 为字符串或 text 块数组。
fn user_prompt_text(v: &serde_json::Value) -> Option<String> {
    let m = v.get("message")?;
    let c = m.get("content")?;
    let text = match c {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(a) => {
            let joined: Vec<String> = a
                .iter()
                .filter_map(|b| {
                    (b.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .then(|| b.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string())
                        .filter(|s| !s.is_empty())
                })
                .collect();
            (!joined.is_empty()).then(|| joined.join("\n"))
        }
        _ => None,
    }?;
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// 从 `offset` 起增量扫描一个 transcript 文件（末行未写完则不消费，同通知模块 idiom）。
/// 截断（size < offset）→ [`ScanErr::Truncated`]。行字节偏移即 `file_pos` 定位键。
pub fn scan_file(path: &Path, offset: u64, last_msg_id: &str) -> Result<ScanOut, ScanErr> {
    let mut f = std::fs::File::open(path)?;
    let meta = f.metadata()?;
    if meta.len() < offset {
        return Err(ScanErr::Truncated);
    }
    let mut out = ScanOut {
        new_offset: offset,
        last_msg_id: last_msg_id.to_string(),
        file_size: meta.len() as i64,
        mtime_ms: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
        ..Default::default()
    };
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(1 << 20);
    f.read_to_end(&mut buf)?;
    // 只消费到末个完整换行；末行无换行但已是合法 JSON（写完未换行的静默尾行）→ 也消费。
    let end = if buf.ends_with(b"\n")
        || buf
            .rsplit(|b| *b == b'\n')
            .next()
            .is_some_and(|t| serde_json::from_slice::<serde_json::Value>(t).is_ok())
    {
        buf.len()
    } else {
        buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1)
    };
    let mut pos = offset as i64;
    for line in buf[..end].split(|b| *b == b'\n') {
        let line_pos = pos;
        pos += line.len() as i64 + 1; // 含换行（末行无换行时多计 1，无后续行，无害）
        // 空行跳过不计错；非法 JSON 才 parse_errors += 1。
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) else {
            out.parse_errors += 1;
            continue;
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("assistant") => handle_assistant(&v, line_pos, &mut out),
            Some("user") => {
                if out.first_user_prompt.is_none()
                    && v.get("isSidechain").and_then(|b| b.as_bool()) != Some(true)
                    && v.get("isMeta").and_then(|b| b.as_bool()) != Some(true)
                    && let Some(t) = user_prompt_text(&v)
                {
                    out.first_user_prompt = Some(truncate_name(&t, 60));
                }
            }
            _ => {}
        }
        // custom-title > ai-title（与行序无关：custom 覆盖已落的 ai）。
        if let Some((source, name)) = aidog_db::session_name::extract_transcript_title(&v)
            && (out.title.is_none()
                || (source == "custom_title"
                    && out.title.as_ref().is_some_and(|(s, _)| s != "custom_title")))
        {
            out.title = Some((source.to_string(), name));
        }
    }
    out.new_offset = offset + end as u64;
    Ok(out)
}

/// assistant 行 → UsageLine（同 id 续行跳过：id == 当前 last_msg_id 时不计）。
fn handle_assistant(v: &serde_json::Value, line_pos: i64, out: &mut ScanOut) {
    let m = match v.get("message") {
        Some(m) => m,
        None => return,
    };
    let usage = match m.get("usage") {
        Some(u) if u.is_object() => u,
        _ => return,
    }
    .clone();
    let msg_id = m
        .get("id")
        .and_then(|i| i.as_str())
        .unwrap_or("")
        .to_string();
    // 同 message.id 多行：首个 usage 行计一次。跨扫描状态 = last_msg_id（续行紧随已计行）。
    if !msg_id.is_empty() && msg_id == out.last_msg_id {
        return;
    }
    let empty = String::new();
    let sid = v
        .get("sessionId")
        .and_then(|s| s.as_str())
        .unwrap_or(&empty)
        .to_string();
    out.session_id = sid.clone();
    out.workdir = v
        .get("cwd")
        .and_then(|c| c.as_str())
        .unwrap_or(&empty)
        .to_string();
    let ts = ts_ms(v);
    let u = &usage;
    out.entries.push(UsageLine {
        session_id: sid,
        is_sidechain: v.get("isSidechain").and_then(|b| b.as_bool()).unwrap_or(false),
        call_time_ms: ts,
        model: m.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        input_tokens: json_i64(u, "input_tokens"),
        output_tokens: json_i64(u, "output_tokens"),
        cache_read_tokens: json_i64(u, "cache_read_input_tokens"),
        cache_write_tokens: json_i64(u, "cache_creation_input_tokens"),
        reasoning_tokens: u
            .get("output_tokens_details")
            .and_then(|d| d.get("thinking_tokens"))
            .and_then(|t| t.as_i64())
            .unwrap_or(0),
    });
    out.positions.push(line_pos);
    if ts > 0 {
        if out.first_ms == 0 || ts < out.first_ms {
            out.first_ms = ts;
        }
        if ts > out.last_ms {
            out.last_ms = ts;
        }
    }
    out.last_msg_id = msg_id;
}
