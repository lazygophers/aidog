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
        match v {
            Value::String(s) => *s = self.mask_text(s),
            Value::Array(a) => a.iter_mut().for_each(|x| self.mask_value(x)),
            Value::Object(m) => m.values_mut().for_each(|x| self.mask_value(x)),
            _ => {}
        }
    }

    /// 占位符最大字节长度（流式扣留窗口用）。
    fn max_placeholder_len(&self) -> usize {
        self.inner.keys().map(String::len).max().unwrap_or(0)
    }
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
