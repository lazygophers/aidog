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
    apply_spans(text, spans)
}

/// 把已收集的 span 应用到文本：区间替换为占位符并登记映射。
/// 规则引擎路径（scrub）与模型合并路径（wire 层）共用。
pub(crate) fn apply_spans(text: &str, spans: Vec<super::detect::SpanKind>) -> (String, ScrubMap) {
    let mut map = ScrubMap::new();
    if spans.is_empty() {
        return (text.to_string(), map);
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for span in spans {
        let original = &text[span.start..span.end];
        let mut placeholder = placeholder_for(span.kind, original);
        // review #7：FNV-1a-32 碰撞时同键异值会覆盖映射（restore 不可逆）→
        // 键同值异 → salt 重散列出新占位符，两条映射独立、往返无损。
        let mut salt = 0u32;
        while map.get(&placeholder).is_some_and(|prev| prev != original) {
            salt += 1;
            placeholder = placeholder_for(span.kind, &format!("{salt}:{original}"));
        }
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

#[cfg(test)]
mod tests {
    use super::super::detect::SpanKind;
    use super::*;
    use std::collections::HashMap;

    /// review #7 红测试：FNV-1a-32 碰撞时 `map.insert` 覆盖映射 → 后值的占位符指回前值，
    /// restore 不可逆。修后：键同值异 → salt 重散列出新占位符，两条映射独立、往返无损。
    #[test]
    fn fnv_collision_keeps_both_mappings() {
        // 32 位空间 ~2^16 样本期望撞出一对：暴力找一对 fnv1a32 相等的不同短串。
        let mut seen: HashMap<u32, String> = HashMap::new();
        let mut pair: Option<(String, String)> = None;
        for i in 0..1_000_000u32 {
            let s = format!("t{i}");
            let h = fnv1a32(s.as_bytes());
            if let Some(prev) = seen.insert(h, s.clone())
                && prev != s
            {
                pair = Some((prev, s));
                break;
            }
        }
        let (a, b) = pair.expect("FNV-32 空间撞不出一对碰撞串");
        assert_ne!(a, b);
        let text = format!("{a}|{b}");
        let spans = vec![
            SpanKind { start: 0, end: a.len(), kind: PiiKind::Email },
            SpanKind { start: a.len() + 1, end: a.len() + 1 + b.len(), kind: PiiKind::Email },
        ];
        let (out, map) = apply_spans(&text, spans);
        assert_eq!(map.len(), 2, "碰撞对必须各得独立映射: {map:?}");
        assert_eq!(restore(&out, &map), text, "碰撞对往返必须无损");
    }
}
