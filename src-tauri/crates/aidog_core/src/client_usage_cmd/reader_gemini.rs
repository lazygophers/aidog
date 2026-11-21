//! Gemini CLI 会话 JSONL 读取器（issue #65 票 05）。
//!
//! 所核格式 = 官方源码 google-gemini/gemini-cli main 分支
//! `packages/core/src/services/chatRecordingService.ts` +
//! `chatRecordingTypes.ts`（2026-11-20 取证；开发机未装 Gemini CLI，
//! 靠官方源码核对，真机验收列人工清单）：
//! - 路径 `~/.gemini/tmp/<project_hash>/chats/` 下 `session-<ts>-<shortid>.jsonl`
//!   （subagent 文件 = `<sessionId>.jsonl`，位于 `chats/<parentSessionId>/` 子目录；
//!   `docs/cli/session-management.md:18` 同路径）。
//! - 追加写 JSONL：首行元数据对象（sessionId / projectHash / startTime /
//!   lastUpdated / kind? / summary?），后续行 = MessageRecord（id / timestamp /
//!   content / type）、`$set` 元数据更新、`$patch` / `$rewindTo` 标记行。
//! - `type:"gemini"` 消息带 `tokens`（TokensSummary，chatRecordingTypes.ts:19-26）：
//!   input / output / cached / thoughts? / tool? / total；input = promptTokenCount
//!   含 cached，计费口径 input_tokens = input - cached，cache_read = cached。
//! - `model` 字段随 gemini 消息（chatRecordingTypes.ts:81）；`kind:"subagent"`
//!   → is_sidechain=true；标题 = 元数据 `summary`，回落首条用户提示截 60 字。
//! - 增量扫描：[offset,end) 段外，另读文件头（≤64KB）取元数据 sessionId / kind，
//!   使增量行归属会话与 sidechain 判定不受截断影响；标题只取本次扫描段的 `$set`。
//! - 工作目录：`tmp/<slug>/.project_root`（票 06，见 [`project_root`]）。

use std::path::Path;

use super::reader::{ScanErr, ScanOut, UsageLine};
use aidog_db::truncate_name;

/// 文件头元数据预读上限：增量扫描时取 sessionId / kind 用。
const HEAD_PRELUDE_BYTES: u64 = 64 * 1024;

/// RFC3339 → 毫秒；解析失败 → 0。
fn ts_ms(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
}

/// 用户消息 content：字符串或 parts 数组（Gemini Part 带 `text` 字段）。
fn user_prompt_text(v: &serde_json::Value) -> Option<String> {
    let c = v.get("content")?;
    let text = match c {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(a) => {
            let joined: Vec<String> = a
                .iter()
                .filter_map(|b| {
                    b.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
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

/// 文件头预读：[0, min(offset, 64KB)) 段取首份元数据 sessionId 与 kind。
/// 消息行不携带会话信息，元数据只在文件首段；增量扫描必须补读才能归属。
fn read_prelude(path: &Path, offset: u64) -> (String, bool) {
    let (mut sid, mut sub) = (String::new(), false);
    let Ok(mut f) = std::fs::File::open(path) else {
        return (sid, sub);
    };
    use std::io::Read;
    let cap = offset.min(HEAD_PRELUDE_BYTES);
    let mut buf = Vec::new();
    if std::io::Read::by_ref(&mut f).take(cap).read_to_end(&mut buf).is_err() {
        return (sid, sub);
    }
    for line in buf.split(|b| *b == b'\n') {
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        let meta = v.get("$set").unwrap_or(&v);
        if let Some(id) = meta
            .get("sessionId")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            && sid.is_empty()
        {
            sid = id.to_string();
        }
        if meta.get("kind").and_then(|x| x.as_str()) == Some("subagent") {
            sub = true;
        }
        if !sid.is_empty() {
            break;
        }
    }
    (sid, sub)
}

/// 工作目录：会话文件最近一层 `chats` 目录的上一级 `tmp/<slug>/.project_root`
/// （官方 projectRegistry.ts:24 `PROJECT_ROOT_FILE`，内容为项目绝对路径）。
/// 子代理文件在 `chats/<parent>/` 下同样取到 `<slug>`；文件缺失（旧哈希目录）/ 读失败 → 空。
fn project_root(path: &Path) -> String {
    path.ancestors()
        .find(|a| a.file_name().is_some_and(|n| n == "chats"))
        .and_then(|chats| chats.parent())
        .and_then(|slug| std::fs::read_to_string(slug.join(".project_root")).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// 从 `offset` 起增量扫描一个会话 JSONL（追加写，无续行语义）。
/// 截断（size < offset）→ [`ScanErr::Truncated`]。行字节偏移即 `file_pos` 定位键。
pub fn scan_file(path: &Path, offset: u64, _last_msg_id: &str) -> Result<ScanOut, ScanErr> {
    let mut f = std::fs::File::open(path)?;
    let meta = f.metadata()?;
    if meta.len() < offset {
        return Err(ScanErr::Truncated);
    }
    let mut out = ScanOut {
        new_offset: offset,
        file_size: meta.len() as i64,
        mtime_ms: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
        ..Default::default()
    };
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(1 << 20);
    f.read_to_end(&mut buf)?;
    // 只消费到末个完整换行；末行已是合法 JSON（写完未换行）→ 也消费（与 claude / codex 读取器同逻辑）。
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
    // 增量扫描：先补读文件头元数据（sessionId / kind），归属不受 offset 截断影响；
    // 全量扫描（offset=0）时元数据在本扫描段内，读段内元数据行补齐。
    let (mut sid, mut sub) = read_prelude(path, offset);
    let mut pos = offset as i64;
    for line in buf[..end].split(|b| *b == b'\n') {
        let line_pos = pos;
        pos += line.len() as i64 + 1; // 含换行（末行无换行时多计 1，无后续行，无害）
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) else {
            out.parse_errors += 1;
            continue;
        };
        process_line(&v, line_pos, &mut sid, &mut sub, &mut out);
    }
    out.session_id = sid;
    out.workdir = project_root(path);
    out.new_offset = offset + end as u64;
    Ok(out)
}
/// 单行分类：首行元数据对象（sessionId + 无 id）、`$set` 元数据更新、
/// `$patch` / `$rewindTo` 标记跳过、消息行（含 id）。
fn process_line(
    v: &serde_json::Value,
    line_pos: i64,
    sid: &mut String,
    sub: &mut bool,
    out: &mut ScanOut,
) {
    let meta = v
        .get("$set")
        .or_else(|| (v.get("id").is_none() && v.get("sessionId").is_some()).then_some(v));
    if let Some(s) = meta {
        apply_metadata(s, sid, sub, out);
        return;
    }
    if v.get("$patch").is_some() || v.get("$rewindTo").is_some() {
        return;
    }
    let id = v.get("id").and_then(|i| i.as_str()).unwrap_or("");
    if id.is_empty() {
        return;
    }
    match v.get("type").and_then(|t| t.as_str()) {
        Some("user") => {
            if out.first_user_prompt.is_none()
                && let Some(t) = user_prompt_text(v)
            {
                out.first_user_prompt = Some(truncate_name(&t, 60));
            }
        }
        Some("gemini") => handle_gemini_msg(v, line_pos, *sub, sid, out),
        _ => {}
    }
}

/// `$set` 或首行元数据 → 合并：sessionId（非空才覆盖）、kind=subagent 置位、
/// summary 标题（本轮扫描段出现的才更新，空值不覆盖）。
fn apply_metadata(s: &serde_json::Value, sid: &mut String, sub: &mut bool, out: &mut ScanOut) {
    if let Some(id) = s.get("sessionId").and_then(|x| x.as_str()).filter(|t| !t.is_empty()) {
        *sid = id.to_string();
    }
    if s.get("kind").and_then(|x| x.as_str()) == Some("subagent") {
        *sub = true;
    }
    if let Some(t) = s.get("summary").and_then(|x| x.as_str()).filter(|t| !t.is_empty()) {
        out.title = Some(("summary".to_string(), truncate_name(t, 60)));
    }
}

/// gemini 消息行 → UsageLine（零用量或不带 tokens 的消息不计；
/// tokens.input 含 cached，计费口径 input = input − cached，cache_read = cached）。
fn handle_gemini_msg(v: &serde_json::Value, line_pos: i64, sub: bool, sid: &str, out: &mut ScanOut) {
    let Some(tokens) = v.get("tokens").filter(|t| t.is_object()) else {
        return;
    };
    let gi64 = |k: &str| tokens.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let (input, cached, output, thoughts) =
        (gi64("input"), gi64("cached"), gi64("output"), gi64("thoughts"));
    if input == 0 && output == 0 && cached == 0 && thoughts == 0 {
        return;
    }
    let ts = v.get("timestamp").and_then(|t| t.as_str()).map(ts_ms).unwrap_or(0);
    out.entries.push(UsageLine {
        session_id: sid.to_string(),
        is_sidechain: sub,
        call_time_ms: ts,
        model: v.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        input_tokens: (input - cached).max(0),
        output_tokens: output,
        cache_read_tokens: cached,
        cache_write_tokens: 0,
        reasoning_tokens: thoughts,
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
}
