//! Codex rollout JSONL 读取器（issue #65 票 04）。
//!
//! 所核格式 = 本机真实会话文件抽样（session_meta.cli_version 0.135.0，2026-11-20，
//! 45 文件全量抽样）：
//! - `session_meta` 行（文件首行）：`payload.id`（会话 id，与 rollout 文件名 uuid 段一致）、
//!   `payload.cwd`、`payload.forked_from_id`（仅分叉文件带，指向父会话 id）。
//! - `event_msg`/`token_count` 行：`payload.info.last_token_usage` =
//!   `{input_tokens, cached_input_tokens, output_tokens, reasoning_output_tokens, total_tokens}`。
//!   单次 API 调用的增量用量；`total_token_usage` 是会话累计，禁用（会双重计数）。
//!   同值 token_count 事件会重复发射（UI 刷新），实测「跳过连续同值事件」后
//!   sum(last_token_usage) == 终值 total_token_usage（45 文件最大样本比率 1.0000）。
//! - 模型：token_count 无 model 字段，模型来自最近的 `turn_context.payload.model`。
//! - 每行顶层 `timestamp`（RFC3339）。
//! - **分叉去重**（0.135.0 实测）：分叉子文件的复制段 = 与父会话 token 序列逐条一致的
//!   头部前缀（42 对父子抽样：17 对有复制前缀，前缀起点恒为父序列起点 0，复制段全部
//!   位于自有首事件之前；复制段内无 turn_context / session_meta）。父文件按
//!   `forked_from_id` 在 sessions 根递归找文件名以 `-<id>.jsonl` 结尾者；找不到父文件
//!   → 按普通会话读。仅在 offset==0 全量扫描时做（增量续读时复制段早已越过）。
//! - 时间戳重写：复制段的行时间戳是子文件写入时刻，无法按时间区分复制段（实测 17/17
//!   全部晚于子会话起点）。

use std::path::{Path, PathBuf};

use super::reader::{ScanErr, ScanOut, UsageLine};
use aidog_db::truncate_name;

/// 单个 token_count 事件的解析缓存（签名去重与分叉前缀匹配都在签名串上做）。
struct TokenEvent {
    pos: i64,
    ts_ms: i64,
    /// "i,o,cr,ro" 用量签名。
    sig: String,
    line: UsageLine,
}

/// 一行 token_count → 事件（无时间戳或非数值 → None，不计）。
fn parse_token_event(v: &serde_json::Value, pos: i64) -> Option<TokenEvent> {
    let info = v.get("payload")?.get("info")?;
    let u = info.get("last_token_usage")?;
    let input = u.get("input_tokens")?.as_i64()?;
    let cached = u.get("cached_input_tokens")?.as_i64()?;
    let output = u.get("output_tokens")?.as_i64()?;
    let reasoning = u.get("reasoning_output_tokens")?.as_i64()?;
    if input == 0 && output == 0 && cached == 0 && reasoning == 0 {
        return None; // 零用量占位事件（初始发射），不计。
    }
    let ts = top_ts_ms(v);
    Some(TokenEvent {
        pos,
        ts_ms: ts,
        sig: format!("{input},{output},{cached},{reasoning}"),
        line: UsageLine {
            session_id: String::new(),
            is_sidechain: false,
            call_time_ms: ts,
            model: String::new(),
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cached,
            cache_write_tokens: 0,
            reasoning_tokens: reasoning,
        },
    })
}

/// 行顶层 `timestamp`（RFC3339）→ 毫秒；解析失败 → 0。
fn top_ts_ms(v: &serde_json::Value) -> i64 {
    v.get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
}
/// 在 sessions 根递归找文件名以 `-<parent_id>.jsonl` 结尾的 rollout 文件。
fn find_parent_rollout(root: &Path, parent_id: &str) -> Option<PathBuf> {
    let rd = std::fs::read_dir(root).ok()?;
    let suffix = format!("-{parent_id}.jsonl");
    let mut out = None;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if out.is_none()
                && let Some(f) = find_parent_rollout(&p, parent_id)
            {
                out = Some(f);
            }
            if out.is_some() {
                break;
            }
        } else if e.file_name().to_string_lossy().ends_with(&suffix) {
            return Some(p);
        }
    }
    out
}

/// 父会话文件的 token 签名序列（只解析 token_count 行）。
fn parent_token_sigs(path: &Path) -> Vec<String> {
    let mut buf = Vec::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(line.as_bytes()) else {
                continue;
            };
            if v.get("type").and_then(|t| t.as_str()) == Some("event_msg")
                && v.get("payload")
                    .and_then(|p| p.get("type"))
                    .and_then(|t| t.as_str())
                    == Some("token_count")
            {
                if let Some(ev) = parse_token_event(&v, 0) {
                    buf.push(ev.sig);
                }
                continue;
            }
        }
    }
    buf
}

/// 文件名 uuid 段 → 会话 id 回落（rollout-<时间>-<uuid>.jsonl）。
fn session_id_from_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = name.strip_suffix(".jsonl").unwrap_or(&name);
    let uuid_re = |s: &str| s.len() == 36 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    if stem.len() >= 36 && uuid_re(&stem[stem.len() - 36..]) {
        stem[stem.len() - 36..].to_string()
    } else {
        String::new()
    }
}

/// 从 offset 起增量扫描一个 rollout 文件。签名去重状态 = last_msg_id（上一事件用量签名）；
/// 分叉去重仅在 offset==0 全量扫描时做：复制段 = 与父 token 序列逐条一致的头部前缀。
pub fn scan_file(
    path: &Path,
    offset: u64,
    last_msg_id: &str,
    sessions_root: Option<&Path>,
) -> Result<ScanOut, ScanErr> {
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
    // 只消费到末个完整换行；末行已是合法 JSON（写完未换行）→ 也消费（与 claude 读取器同逻辑）。
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(1 << 20);
    f.read_to_end(&mut buf)?;
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
    let mut events: Vec<TokenEvent> = Vec::new();
    let mut cur_model = String::new();
    let mut forked_from: Option<String> = None;
    for line in buf[..end].split(|b| *b == b'\n') {
        let line_pos = pos;
        pos += line.len() as i64 + 1;
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) else {
            out.parse_errors += 1;
            continue;
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("session_meta") if offset == 0 => {
                let p = v.get("payload");
                out.session_id = p
                    .and_then(|p| p.get("id"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                out.workdir = p
                    .and_then(|p| p.get("cwd"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                forked_from = p
                    .and_then(|p| p.get("forked_from_id"))
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
            }
            Some("turn_context") => {
                cur_model = v
                    .get("payload")
                    .and_then(|p| p.get("model"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            Some("event_msg") => {
                let pt = v
                    .get("payload")
                    .and_then(|p| p.get("type"))
                    .and_then(|t| t.as_str());
                if pt == Some("token_count") {
                    if let Some(mut ev) = parse_token_event(&v, line_pos) {
                        ev.line.model = cur_model.clone();
                        events.push(ev);
                    }
                    continue;
                }
                if pt == Some("user_message")
                    && out.first_user_prompt.is_none()
                {
                    let t = v
                        .get("payload")
                        .and_then(|p| p.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !t.is_empty() {
                        out.first_user_prompt = Some(truncate_name(&t, 60));
                    }
                }
            }
            _ => {}
        }
    }
    out.new_offset = offset + end as u64;
    // 会话 id：meta 优先，回落文件名 uuid 段。
    if out.session_id.is_empty() {
        out.session_id = session_id_from_name(path);
    }
    let sid = out.session_id.clone();
    // 分叉去重：仅全量扫描（offset==0）且父文件可寻得时，跳过与父 token 序列逐条一致的头部前缀。
    let mut skip = 0usize;
    if offset == 0
        && let Some(parent_id) = forked_from
        && let Some(root) = sessions_root
        && let Some(parent_path) = find_parent_rollout(root, &parent_id)
    {
        let psigs = parent_token_sigs(&parent_path);
        while skip < events.len() && skip < psigs.len() && events[skip].sig == psigs[skip] {
            skip += 1;
        }
    }
    // 组装：跳过复制段后，连续同签名事件只计一次（prev 初值 = skip 段末签名或持久化签名）。
    let mut prev = if skip > 0 {
        events[skip - 1].sig.clone()
    } else {
        last_msg_id.to_string()
    };
    for ev in events.into_iter().skip(skip) {
        out.last_msg_id = ev.sig.clone();
        if ev.sig == prev {
            continue;
        }
        prev = ev.sig.clone();
        let (ev_pos, ev_ts) = (ev.pos, ev.ts_ms);
        let mut e = ev.line;
        e.session_id = sid.clone();
        out.entries.push(e);
        out.positions.push(ev_pos);
        if ev_ts > 0 {
            if out.first_ms == 0 || ev_ts < out.first_ms {
                out.first_ms = ev_ts;
            }
            if ev_ts > out.last_ms {
                out.last_ms = ev_ts;
            }
        }
    }
    Ok(out)
}
