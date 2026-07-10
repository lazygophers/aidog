//! 模板占位变量替换。
//!
//! 不依赖正则：线性扫描 `{key}`，键查 vars。缺失占位两种策略：
//! - `substitute_vars`：保留原文 `{x}`（type 路径，与历史一致）。
//! - `substitute_vars_fill_empty`：替换为空串（event 路径，避免残留裸占位）。

use std::collections::HashMap;

/// 替换模板中的 `{key}` 占位为 vars 对应值；未知占位保留原文。
///
/// 不依赖正则：线性扫描，遇 `{` 找配对 `}`，取键查 vars。
/// 键含非占位字符（空格等）或缺失 → 整段 `{...}` 原样保留。
pub fn substitute_vars(template: &str, vars: &HashMap<String, String>) -> String {
    substitute_vars_impl(template, vars, false)
}

/// 同 `substitute_vars`，但缺失占位 → **替换为空串**（不保留 `{x}` 字面）。
///
/// 用于 event 路径：每事件默认模板用其专属入参（`{tool_name}` 等），但脚本通用透传时
/// 该事件实际 stdin 可能缺该可选字段；为避免残留裸 `{x}` 难看，event 路径采用「缺失→空串」。
/// type 路径仍用 `substitute_vars`（保留未知占位，与历史一致）。
pub fn substitute_vars_fill_empty(template: &str, vars: &HashMap<String, String>) -> String {
    substitute_vars_impl(template, vars, true)
}

/// 占位替换核心。`fill_empty=true` 时缺失/未知占位替换为空串，否则保留原文。
fn substitute_vars_impl(template: &str, vars: &HashMap<String, String>, fill_empty: bool) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            // 找配对 }
            if let Some(rel) = template[i + 1..].find('}') {
                let key = &template[i + 1..i + 1 + rel];
                // 键须为合法占位名（非空 + 仅 [a-zA-Z0-9_]）
                let valid = !key.is_empty()
                    && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
                if valid {
                    if let Some(v) = vars.get(key) {
                        out.push_str(v);
                    } else if !fill_empty {
                        // 未知占位保留原文
                        out.push('{');
                        out.push_str(key);
                        out.push('}');
                    }
                    // fill_empty 且缺失 → 不输出任何内容（替换为空串）
                    i = i + 1 + rel + 1;
                    continue;
                }
            }
            // 无配对 / 非法键：原样输出 '{'
            out.push('{');
            i += 1;
        } else {
            // 推进一个 UTF-8 字符（避免切断多字节）
            let ch_len = utf8_char_len(bytes[i]);
            out.push_str(&template[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

/// UTF-8 首字节判断字符字节数。
fn utf8_char_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        1
    }
}
