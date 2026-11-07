//! 脱敏 / 还原引擎：确定性占位符生成与文本替换。

use super::detect::collect_spans;
use super::{PiiKind, ScrubMap, ScrubOptions};

/// FNV-1a 32 位，占位符 hex 由原值确定性派生（同原值恒同占位符）。
fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &b in bytes {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn placeholder_for(kind: PiiKind, original: &str) -> String {
    format!(
        "{{{{PII_{}_{:08x}}}}}",
        kind.as_str(),
        fnv1a32(original.as_bytes())
    )
}

/// 检测并脱敏文本中的敏感数据。
///
/// 返回（脱敏后文本，映射表）。同原值恒得同占位符。
pub fn scrub(text: &str, opts: &ScrubOptions) -> (String, ScrubMap) {
    let spans = collect_spans(text, |k| opts.enabled(k));
    let mut map = ScrubMap::new();
    if spans.is_empty() {
        return (text.to_string(), map);
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for span in spans {
        let original = &text[span.start..span.end];
        let placeholder = placeholder_for(span.kind, original);
        out.push_str(&text[last..span.start]);
        out.push_str(&placeholder);
        map.insert(&placeholder, original);
        last = span.end;
    }
    out.push_str(&text[last..]);
    (out, map)
}

/// 把 `text` 中登记过的占位符换回原值。
pub fn restore(text: &str, map: &ScrubMap) -> String {
    let mut out = text.to_string();
    for (placeholder, original) in map.iter() {
        out = out.replace(placeholder, original);
    }
    out
}
