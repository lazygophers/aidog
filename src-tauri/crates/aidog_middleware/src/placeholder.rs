//! 可还原占位符引擎（唯一实现）：确定性占位符 `{{PII_<TYPE>_<8hex>}}`、映射表、
//! JSON 转义还原、流式还原窗口。映射表只在内存，原值不进 `Debug` / `Display`。

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, LazyLock};

use regex::Regex;
use serde_json::Value;

/// FNV-1a 32 位，占位符 hex 由原值确定性派生（同原值恒同占位符）。
pub fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &b in bytes {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// `kind` 为占位符 TYPE 段（大写字母 / 数字 / 下划线）。
pub fn placeholder_for(kind: &str, original: &str) -> String {
    format!("{{{{PII_{kind}_{:08x}}}}}", fnv1a32(original.as_bytes()))
}

/// 占位符形态匹配（已被替换的片段：后续规则不得再匹配 / 改写）。
pub static PLACEHOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{PII_[A-Z0-9_]+_[0-9a-f]{8}\}\}").expect("placeholder regex"));

/// 脱敏映射表：占位符 → 原值。原值不进 `Debug` / `Display`。
#[derive(Default)]
pub struct ScrubMap {
    inner: HashMap<String, String>,
}

impl ScrubMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, placeholder: &str, original: &str) {
        self.inner
            .insert(placeholder.to_string(), original.to_string());
    }

    /// 登记 `original` 并返回其占位符。FNV-1a-32 碰撞（键同值异）时 salt 重散列，
    /// 两条映射独立、往返无损。
    pub fn register(&mut self, kind: &str, original: &str) -> String {
        let mut placeholder = placeholder_for(kind, original);
        let mut salt = 0u32;
        while self.get(&placeholder).is_some_and(|prev| prev != original) {
            salt += 1;
            placeholder = placeholder_for(kind, &format!("{salt}:{original}"));
        }
        self.insert(&placeholder, original);
        placeholder
    }

    /// 占位符 → 原值，未登记返回 `None`。
    pub fn get(&self, placeholder: &str) -> Option<&str> {
        self.inner.get(placeholder).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.inner.iter()
    }

    /// 并入另一张表。
    pub fn extend(&mut self, other: &ScrubMap) {
        for (p, o) in other.iter() {
            self.insert(p, o);
        }
    }

    /// 反向：把文本中的原值换回占位符（日志落库用）；长原值优先，避免短原值吃掉长原值的子串。
    pub fn mask_text(&self, text: &str) -> String {
        let mut pairs: Vec<(&String, &String)> = self.inner.iter().collect();
        pairs.sort_by_key(|(_, o)| std::cmp::Reverse(o.len()));
        let mut out = text.to_string();
        for (p, o) in pairs {
            if !o.is_empty() && out.contains(o.as_str()) {
                out = out.replace(o.as_str(), p);
            }
        }
        out
    }

    /// 日志用：`body` 若是 JSON，对每个字符串叶子 [`mask_text`]（不依赖原值在 JSON 里的转义形态），
    /// 重序列化为紧凑 JSON；非 JSON 则整串 [`mask_text`] 并补一遍转义形态。
    pub fn mask_log_body(&self, body: &str) -> String {
        if self.is_empty() || body.is_empty() {
            return body.to_string();
        }
        match serde_json::from_str::<Value>(body) {
            Ok(mut v) => {
                self.mask_value(&mut v);
                serde_json::to_string(&v).unwrap_or_else(|_| self.mask_text(body))
            }
            Err(_) => self.mask_text(body),
        }
    }

    fn mask_value(&self, v: &mut Value) {
        self.mask_value_at(v, true, None);
    }

    fn mask_value_at(&self, v: &mut Value, top: bool, parent_key: Option<&str>) {
        match v {
            Value::String(s) => *s = self.mask_text(s),
            Value::Array(a) => a.iter_mut().for_each(|x| self.mask_value_at(x, false, parent_key)),
            Value::Object(m) => {
                for (k, x) in m.iter_mut() {
                    if is_session_field(top, parent_key, k) {
                        // sessionId 硬边界：session 字段叶子原样保留。
                        continue;
                    }
                    self.mask_value_at(x, false, Some(k));
                }
            }
            _ => {}
        }
    }

    /// 占位符最大字节长度（流式扣留窗口用）。
    fn max_placeholder_len(&self) -> usize {
        self.inner.keys().map(String::len).max().unwrap_or(0)
    }
}

/// sessionId 硬边界（用户 2026-11-11）：session id 不在中间件过滤范围。
/// ① 顶层 `session_id`；② `metadata.user_id` / `metadata.session_id`
/// （anthropic `metadata.user_id` 内含 `session_<uuid>`，同样保留）。
/// 出站「全部内容」mask（`visit_all_node`）与日志 body 掩码（`mask_log_body`）共用，
/// 保证出站 body 与日志副本口径一致。
pub fn is_session_field(top: bool, parent_key: Option<&str>, key: &str) -> bool {
    (top && key == "session_id")
        || (parent_key == Some("metadata") && matches!(key, "user_id" | "session_id"))
}

impl fmt::Debug for ScrubMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(
                self.inner
                    .iter()
                    .map(|(p, o)| (p, format!("[REDACTED {}B]", o.len()))),
            )
            .finish()
    }
}

/// 把 `text` 中登记过的占位符换回原值（纯文本）。
pub fn restore(text: &str, map: &ScrubMap) -> String {
    let mut out = text.to_string();
    for (placeholder, original) in map.iter() {
        out = out.replace(placeholder, original);
    }
    out
}

/// JSON 字符串字面量转义（不加外层引号）。
fn json_escape_inner(original: &str) -> String {
    let mut esc = String::with_capacity(original.len());
    for c in original.chars() {
        match c {
            '"' => esc.push_str("\\\""),
            '\\' => esc.push_str("\\\\"),
            '\n' => esc.push_str("\\n"),
            '\r' => esc.push_str("\\r"),
            '\t' => esc.push_str("\\t"),
            c if (c as u32) < 0x20 => esc.push_str(&format!("\\u{:04x}", c as u32)),
            c => esc.push(c),
        }
    }
    esc
}

/// 占位符起始标记（全部占位符的公共 ASCII 前缀）。
const PLACEHOLDER_HEAD_STR: &str = "{{PII_";
const PLACEHOLDER_HEAD: &[u8] = PLACEHOLDER_HEAD_STR.as_bytes();
/// 流式扣留窗口下限（当前最长类型 BEARER_TOKEN = 29 字节）。
const MIN_PLACEHOLDER_LEN: usize = 29;

/// 还原（JSON / SSE 文本上下文安全形态）：替换值先做 JSON 字符串转义，
/// 原值含引号 / 反斜杠 / 换行（PEM）时不打破外层 JSON 结构。
pub fn restore_json_escaped(text: &str, map: &ScrubMap) -> String {
    if !text.contains(PLACEHOLDER_HEAD_STR) {
        return text.to_string();
    }
    let mut out = text.to_string();
    for (placeholder, original) in map.iter() {
        if !out.contains(placeholder.as_str()) {
            continue;
        }
        out = out.replace(placeholder.as_str(), &json_escape_inner(original));
    }
    out
}

/// 流式还原窗口：每次 `push_bytes` 扣留尾部可能的半截占位符，攒齐再还原下发，
/// 占位符永不被 chunk 边界切断；流末 / 断连 / 终止符时 `finish()` 冲刷残留。
pub struct StreamRestorer {
    map: Arc<ScrubMap>,
    max_len: usize,
    pending: Vec<u8>,
}

fn starts_with_registered_placeholder(haystack: &[u8], i: usize, map: &ScrubMap) -> bool {
    map.iter()
        .any(|(p, _)| haystack[i..].starts_with(p.as_bytes()))
}

fn tail_head_prefix_len(haystack: &[u8]) -> usize {
    let max = PLACEHOLDER_HEAD.len() - 1;
    let start = haystack.len().saturating_sub(max);
    let tail = &haystack[start..];
    for k in (1..=tail.len()).rev() {
        if PLACEHOLDER_HEAD[..k] == tail[tail.len() - k..] {
            return k;
        }
    }
    0
}

impl StreamRestorer {
    pub fn new(map: Arc<ScrubMap>) -> Self {
        let max_len = map.max_placeholder_len().max(MIN_PLACEHOLDER_LEN);
        Self { map, max_len, pending: Vec::new() }
    }

    /// 喂一段（未还原）字节，返回可安全下发的还原后文本（UTF-8 无损）。
    pub fn push_bytes(&mut self, data: &[u8]) -> String {
        self.pending.extend_from_slice(data);
        let mut emit_len = self.pending.len();
        if let Some(i) = last_head_occurrence(&self.pending) {
            let remaining = self.pending.len() - i;
            if remaining < self.max_len
                && !starts_with_registered_placeholder(&self.pending, i, &self.map)
            {
                emit_len = i;
            }
        } else {
            emit_len -= tail_head_prefix_len(&self.pending);
        }
        while emit_len > 0 && !is_char_boundary(&self.pending, emit_len) {
            emit_len -= 1;
        }
        if emit_len == 0 {
            return String::new();
        }
        let text = String::from_utf8_lossy(&self.pending[..emit_len]).to_string();
        self.pending.drain(..emit_len);
        restore_json_escaped(&text, &self.map)
    }

    /// 冲刷扣留的残留（还原后返回；幂等）。
    pub fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }
        let text = String::from_utf8_lossy(&self.pending).to_string();
        self.pending.clear();
        restore_json_escaped(&text, &self.map)
    }
}

struct DeltaTextRestorer {
    pending: String,
    map: Arc<ScrubMap>,
}

impl DeltaTextRestorer {
    fn new(map: Arc<ScrubMap>) -> Self {
        Self { pending: String::new(), map }
    }

    fn push(&mut self, text: &str) -> String {
        self.pending.push_str(text);
        let bytes = self.pending.as_bytes();
        let mut emit_len = bytes.len() - tail_head_prefix_len(bytes);
        if let Some(i) = last_head_occurrence(bytes)
            && bytes.len() - i < self.map.max_placeholder_len().max(MIN_PLACEHOLDER_LEN)
            && !starts_with_registered_placeholder(bytes, i, &self.map)
        {
            emit_len = emit_len.min(i);
        }
        let safe = self.pending[..emit_len].to_string();
        self.pending.drain(..emit_len);
        restore(&safe, &self.map)
    }

    fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

struct DeferredSseFrame {
    original: String,
    restored: String,
}

/// SSE-aware counterpart to `StreamRestorer`; each logical delta field keeps separate state.
pub struct SseStreamRestorer {
    map: Arc<ScrubMap>,
    bytes: Vec<u8>,
    fields: HashMap<String, DeltaTextRestorer>,
    deferred: Vec<DeferredSseFrame>,
}

impl SseStreamRestorer {
    pub fn new(map: Arc<ScrubMap>) -> Self {
        Self { map, bytes: Vec::new(), fields: HashMap::new(), deferred: Vec::new() }
    }

    pub fn push_bytes(&mut self, data: &[u8]) -> String {
        self.bytes.extend_from_slice(data);
        let mut output = String::new();
        while let Some((end, delimiter_len)) = sse_frame_delimiter(&self.bytes) {
            let frame = String::from_utf8_lossy(&self.bytes[..end + delimiter_len]).into_owned();
            self.bytes.drain(..end + delimiter_len);
            output.push_str(&self.process_frame(frame));
        }
        output
    }

    pub fn finish(&mut self) -> String {
        let mut output = String::new();
        if !self.bytes.is_empty() {
            let frame = String::from_utf8_lossy(&self.bytes).into_owned();
            self.bytes.clear();
            output.push_str(&self.process_frame(frame));
        }
        if self.fields.values().any(DeltaTextRestorer::has_pending) {
            output.push_str(&self.deferred.drain(..).map(|f| f.original).collect::<String>());
        } else {
            output.push_str(&self.deferred.drain(..).map(|f| f.restored).collect::<String>());
        }
        output
    }

    fn process_frame(&mut self, original: String) -> String {
        let mut restored = original.clone();
        let body = original.trim_end_matches(['\r', '\n']);
        let lines = body.lines().collect::<Vec<_>>();
        let data_lines = lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                line.strip_prefix("data:")
                    .map(|data| data.strip_prefix(' ').unwrap_or(data))
                    .or_else(|| (*line == "data").then_some(""))
                    .map(|data| (index, data))
            })
            .collect::<Vec<_>>();
        let json = data_lines.iter().map(|(_, data)| *data).collect::<Vec<_>>().join("\n");
        if !json.is_empty() && json != "[DONE]"
            && let Ok(mut event) = serde_json::from_str::<Value>(&json)
        {
            self.restore_event_text(&mut event);
            if let Ok(json) = serde_json::to_string(&event) {
                let first_data_line = data_lines[0].0;
                restored = lines
                    .iter()
                    .enumerate()
                    .filter_map(|(index, line)| {
                        if index == first_data_line {
                            Some(format!("data: {json}"))
                        } else if data_lines.iter().any(|(data_index, _)| *data_index == index) {
                            None
                        } else {
                            Some((*line).to_string())
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if original.ends_with("\r\n\r\n") {
                    restored.push_str("\r\n\r\n");
                } else if original.ends_with("\n\n") {
                    restored.push_str("\n\n");
                } else if original.ends_with('\n') {
                    restored.push('\n');
                }
            }
        }
        if self.fields.values().any(DeltaTextRestorer::has_pending) {
            self.deferred.push(DeferredSseFrame { original, restored });
            String::new()
        } else if self.deferred.is_empty() {
            restored
        } else {
            self.deferred.push(DeferredSseFrame { original, restored });
            self.deferred.drain(..).map(|f| f.restored).collect()
        }
    }

    fn restore_event_text(&mut self, event: &mut Value) {
        if let Some(choices) = event.get_mut("choices").and_then(Value::as_array_mut) {
            for (choice_index, choice) in choices.iter_mut().enumerate() {
                let choice_id = choice.get("index").and_then(Value::as_u64).unwrap_or(choice_index as u64);
                if let Some(delta) = choice.get_mut("delta") {
                    if let Some(text) = delta.get_mut("content").and_then(|value| value.as_str()).map(str::to_owned) {
                        let key = format!("choice:{choice_id}:content");
                        *delta.get_mut("content").unwrap() = Value::String(self.field_push(key, &text));
                    }
                    if let Some(calls) = delta.get_mut("tool_calls").and_then(Value::as_array_mut) {
                        for (call_index, call) in calls.iter_mut().enumerate() {
                            let index = call.get("index").and_then(Value::as_u64).unwrap_or(call_index as u64);
                            if let Some(arguments) = call.pointer_mut("/function/arguments")
                                && let Some(text) = arguments.as_str().map(str::to_owned)
                            {
                                let key = format!("choice:{choice_id}:tool:{index}:arguments");
                                *arguments = Value::String(self.field_push(key, &text));
                            }
                        }
                    }
                }
            }
        }
        if let Some(index) = event.get("index").and_then(Value::as_u64)
            && let Some(delta) = event.get_mut("delta")
        {
            let (field, key) = match delta.get("type").and_then(Value::as_str) {
                Some("text_delta") => ("text", "text"),
                Some("input_json_delta") => ("partial_json", "tool_json"),
                _ => ("", ""),
            };
            if !field.is_empty()
                && let Some(text) = delta.get(field).and_then(|value| value.as_str()).map(str::to_owned)
            {
                let path = format!("block:{index}:{key}");
                *delta.get_mut(field).unwrap() = Value::String(self.field_push(path, &text));
            }
        }
    }

    fn field_push(&mut self, key: String, text: &str) -> String {
        self.fields.entry(key).or_insert_with(|| DeltaTextRestorer::new(self.map.clone())).push(text)
    }
}

fn sse_frame_delimiter(bytes: &[u8]) -> Option<(usize, usize)> {
    let lf = bytes.windows(2).position(|window| window == b"\n\n");
    let crlf = bytes.windows(4).position(|window| window == b"\r\n\r\n");
    match (lf, crlf) {
        (Some(lf), Some(crlf)) if crlf < lf => Some((crlf, 4)),
        (Some(lf), _) => Some((lf, 2)),
        (None, Some(crlf)) => Some((crlf, 4)),
        (None, None) => None,
    }
}

fn is_char_boundary(bytes: &[u8], cut: usize) -> bool {
    cut == 0 || cut >= bytes.len() || (bytes[cut] & 0b1100_0000) != 0b1000_0000
}

fn last_head_occurrence(haystack: &[u8]) -> Option<usize> {
    if haystack.len() < PLACEHOLDER_HEAD.len() {
        return None;
    }
    (0..=haystack.len() - PLACEHOLDER_HEAD.len())
        .rev()
        .find(|&i| &haystack[i..i + PLACEHOLDER_HEAD.len()] == PLACEHOLDER_HEAD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_with(kind: &str, orig: &str) -> (ScrubMap, String) {
        let mut m = ScrubMap::new();
        let p = m.register(kind, orig);
        (m, p)
    }

    #[test]
    fn mask_log_body_preserves_session_fields() {
        // 硬边界（用户 2026-11-11）：sessionId 不在中间件过滤范围。日志 body 掩码时
        // session 字段叶子原样保留（消息内容照常替换占位符）。
        let uuid = "3f2b8c9e-1111-2222-3333-444455556666";
        let (m, p) = map_with("SECRET", uuid);
        let body = serde_json::json!({
            "session_id": uuid,
            "metadata": {
                "user_id": format!("user_x_account__session_{uuid}"),
                "session_id": uuid,
            },
            "messages": [{ "role": "user", "content": format!("msg {uuid} end") }],
        })
        .to_string();
        let masked = m.mask_log_body(&body);
        let v: Value = serde_json::from_str(&masked).unwrap();
        assert_eq!(v["session_id"], serde_json::json!(uuid), "根级 session_id 被抹");
        assert_eq!(
            v["metadata"]["user_id"],
            serde_json::json!(format!("user_x_account__session_{uuid}")),
            "metadata.user_id 被抹"
        );
        assert_eq!(v["metadata"]["session_id"], serde_json::json!(uuid), "metadata.session_id 被抹");
        let c = v["messages"][0]["content"].as_str().unwrap();
        assert!(c.contains(&p) && !c.contains(uuid), "消息内容应替换占位符，实际 {c}");
    }

    #[test]
    fn restore_json_escaped_keeps_json_frame() {
        let (m, p) = map_with("EMAIL", "a\"b\nc");
        let json = serde_json::to_string(&serde_json::json!({ "c": format!("x {p} y") })).unwrap();
        let back: Value = serde_json::from_str(&restore_json_escaped(&json, &m)).unwrap();
        assert_eq!(back["c"], "x a\"b\nc y");
    }

    #[test]
    fn stream_restorer_reassembles_split_placeholder() {
        let (m, p) = map_with("EMAIL", "bob@example.com");
        let text = format!("mail me: {p} thanks");
        let mut r = StreamRestorer::new(Arc::new(m));
        let mut out = String::new();
        for chunk in text.as_bytes().chunks(5) {
            out.push_str(&r.push_bytes(chunk));
        }
        out.push_str(&r.finish());
        assert_eq!(out, "mail me: bob@example.com thanks");
    }

    #[test]
    fn stream_restorer_reassembles_placeholder_across_sse_deltas() {
        let mut map = ScrubMap::new();
        let placeholder = map.register("EMAIL", "restored-address");
        let split = 12;
        let first = serde_json::json!({"choices":[{"index":0,"delta":{"content":&placeholder[..split]}}]});
        let second = serde_json::json!({"choices":[{"index":0,"delta":{"content":&placeholder[split..]}}]});
        let frames = format!("data: {first}\r\n\r\ndata: {second}\r\n\r\ndata: [DONE]\r\n\r\n");
        let mut restorer = SseStreamRestorer::new(Arc::new(map));
        let mut restored = restorer.push_bytes(frames.as_bytes());
        restored.push_str(&restorer.finish());
        let contents = restored
            .split("\r\n\r\n")
            .filter_map(|frame| frame.strip_prefix("data: "))
            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
            .filter_map(|event| event["choices"][0]["delta"]["content"].as_str().map(str::to_owned))
            .collect::<String>();
        assert_eq!(contents, "restored-address");
    }

    #[test]
    fn sse_restorer_reassembles_placeholder_prefix_across_deltas() {
        let mut map = ScrubMap::new();
        let placeholder = map.register("EMAIL", "a\"b\nc");
        let map = Arc::new(map);
        for split in 1..placeholder.len() {
            let first = serde_json::json!({"choices":[{"index":0,"delta":{"content":&placeholder[..split]}}]});
            let second = serde_json::json!({"choices":[{"index":0,"delta":{"content":&placeholder[split..]}}]});
            let frames = format!("data: {first}\n\ndata: {second}\n\n");
            let mut restorer = SseStreamRestorer::new(map.clone());
            let mut restored = restorer.push_bytes(frames.as_bytes());
            restored.push_str(&restorer.finish());
            let contents = restored
                .split("\n\n")
                .filter_map(|frame| frame.strip_prefix("data: "))
                .map(|data| serde_json::from_str::<Value>(data).unwrap())
                .filter_map(|event| event["choices"][0]["delta"]["content"].as_str().map(str::to_owned))
                .collect::<String>();
            assert_eq!(contents, "a\"b\nc", "split={split}");
        }
    }

    #[test]
    fn sse_restorer_preserves_anthropic_metadata_and_restores_isolated_blocks() {
        let mut map = ScrubMap::new();
        let text_placeholder = map.register("EMAIL", "restored-text");
        let tool_placeholder = map.register("SECRET", "restored-tool");
        let split = 12;
        let first = serde_json::json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": &text_placeholder[..split]}
        });
        let second = serde_json::json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": &text_placeholder[split..]}
        });
        let tool = serde_json::json!({
            "type": "content_block_delta",
            "index": 1,
            "delta": {"type": "input_json_delta", "partial_json": &tool_placeholder}
        });
        let first_lines = serde_json::to_string_pretty(&first).unwrap();
        let first_data = first_lines.lines().enumerate().map(|(index, line)| {
            if index % 2 == 0 { format!("data:{line}\n") } else { format!("data: {line}\n") }
        }).collect::<String>();
        let frames = format!(
            "event: content_block_delta\nid: 7\nretry: 2500\n{first_data}\nevent: content_block_delta\ndata: {tool}\n\nevent: content_block_delta\ndata: {second}\n\nevent: message_stop\ndata: {{}}\n\n"
        );
        let mut restorer = SseStreamRestorer::new(Arc::new(map));
        let mut restored = restorer.push_bytes(frames.as_bytes());
        restored.push_str(&restorer.finish());

        let frame = restored
            .split("\n\n")
            .find(|frame| frame.contains("id: 7"))
            .unwrap();
        assert!(frame.contains("event: content_block_delta"));
        assert!(frame.contains("id: 7"));
        assert!(frame.contains("retry: 2500"));
        let event = frame
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .collect::<Vec<_>>()
            .join("\n");
        let event: Value = serde_json::from_str(&event).unwrap();
        assert_eq!(event["type"], "content_block_delta");
        let text = restored
            .split("\n\n")
            .filter(|frame| frame.contains("content_block_delta"))
            .filter_map(|frame| frame.lines().find_map(|line| line.strip_prefix("data: ")))
            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
            .filter(|event| event["index"] == 0)
            .filter_map(|event| event["delta"]["text"].as_str().map(str::to_owned))
            .collect::<String>();
        assert_eq!(text, "restored-text");

        let tool_event = restored
            .split("\n\n")
            .filter(|frame| frame.contains("content_block_delta"))
            .filter_map(|frame| frame.lines().find_map(|line| line.strip_prefix("data: ")))
            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
            .find(|event| event["index"] == 1)
            .unwrap();
        assert_eq!(tool_event["delta"]["partial_json"], "restored-tool");
    }

    #[test]
    fn sse_restorer_flushes_unfinished_placeholder_before_terminal_event() {
        let mut map = ScrubMap::new();
        let placeholder = map.register("EMAIL", "restored-address");
        let partial = &placeholder[..12];
        let frame = format!(
            "event: content_block_delta\ndata: {}\n\nevent: message_stop\ndata: {{}}\n\n",
            serde_json::json!({"index":0,"delta":{"type":"text_delta","text":partial}})
        );
        let mut restorer = SseStreamRestorer::new(Arc::new(map));
        let mut restored = restorer.push_bytes(frame.as_bytes());
        restored.push_str(&restorer.finish());
        assert!(restored.find(partial).unwrap() < restored.find("event: message_stop").unwrap());
        assert!(restored.contains("event: message_stop"));
    }

    #[test]
    fn stream_restorer_passthrough_multibyte() {
        let mut r = StreamRestorer::new(Arc::new(ScrubMap::new()));
        let text = "没有 placeholder，多字节完整下发";
        let mut out = r.push_bytes(text.as_bytes());
        out.push_str(&r.finish());
        assert_eq!(out, text);
    }

    #[test]
    fn mask_log_body_replaces_in_json_leaves_regardless_of_escape() {
        let (m, p) = map_with("EMAIL", "bob@example.com");
        // 客户端把 @ 写成 @：纯文本替换抓不到，叶子级替换可以。
        let body = r#"{"m":"mail bob@example.com"}"#;
        let masked = m.mask_log_body(body);
        assert!(masked.contains(&p) && !masked.contains("example.com"), "{masked}");
    }

    #[test]
    fn register_is_deterministic_and_collision_safe() {
        let mut m = ScrubMap::new();
        assert_eq!(m.register("X", "v"), m.register("X", "v"));
        assert_eq!(m.len(), 1);
    }
}
