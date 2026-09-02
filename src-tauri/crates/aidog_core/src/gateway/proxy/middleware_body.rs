//! 同协议透传 body 的 middleware 文本改写（`.scratch/field-adapt/` 票 02）。
//!
//! **为什么在协议层而不是引擎里**：middleware 引擎的改写实现只认 `ChatRequest` 抽象
//! （`aidog_middleware/src/inbound.rs`），同协议透传分支用的是客户端原体 `Value`，不经
//! `parse_incoming_request`，因此用户配的 mask/override/inject 规则历史上在透传路径上
//! **完全不生效**（脱敏被绕过 = 安全缺口）。修法是给引擎加一条 `Value` 层入口
//! （`MiddlewareEngine::apply_inbound_texts`），协议知识留在本模块：
//! 本模块按 wire 协议把 body 内可读文本抽成文本槽 → 引擎做条件求值与片段替换 →
//! 本模块按同一遍历顺序写回。middleware crate 不引入任何协议知识。
//!
//! **只作用于透传分支**：转换分支的 mask/inject 已在分叉前作用于 `chat_req`
//! （`forward.rs` 的 platform 层挂载点），两处都跑会把 `system_append` 注入两遍。
//!
//! **block 不在本模块**：block 在分叉前判定并直接返回 403，两条路径本来就都生效。

use aidog_db::models::MiddlewareSettings;
use aidog_middleware::{InboundInject, InboundTexts, MiddlewareEngine};
use serde_json::Value;

use crate::gateway::models::Protocol;

/// 文本槽所属侧：与 `ChatRequest` 的 `system` / `messages` 二分对齐（规则 `fields` 按此限定）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    System,
    Messages,
}

/// 透传 body 上应用 middleware 入站改写。返回是否真的改写了 body（调用方记日志）。
///
/// 无命中规则时**一个字节都不动**（不写回、不注入），保证「无规则透传逐字节不变」。
pub(crate) fn apply_middleware_body(
    engine: &MiddlewareEngine,
    settings: &MiddlewareSettings,
    body: &mut Value,
    wire: &Protocol,
    model: &str,
    platform_id: i64,
) -> bool {
    let mut texts = InboundTexts::new(model);
    for side in [Side::System, Side::Messages] {
        let mut sink = |s: &str| -> Option<String> {
            match side {
                Side::System => texts_push(&mut texts.system, s),
                Side::Messages => texts_push(&mut texts.messages, s),
            }
            None
        };
        visit_texts(body, wire, side, &mut sink);
    }
    let before = (texts.system.clone(), texts.messages.clone());

    engine.apply_inbound_texts(settings, &mut texts, platform_id);

    let rewritten = before.0 != texts.system || before.1 != texts.messages;
    if !rewritten && texts.injects.is_empty() {
        return false;
    }
    if rewritten {
        for side in [Side::System, Side::Messages] {
            let slots = match side {
                Side::System => &texts.system,
                Side::Messages => &texts.messages,
            };
            let mut i = 0usize;
            let mut writer = |_: &str| -> Option<String> {
                let next = slots.get(i).cloned();
                i += 1;
                next
            };
            visit_texts(body, wire, side, &mut writer);
        }
    }
    for op in &texts.injects {
        match op {
            InboundInject::SystemAppend(v) => append_system(body, wire, v),
            InboundInject::BodySet { target, value } => {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert(target.clone(), Value::String(value.clone()));
                }
            }
        }
    }
    true
}

fn texts_push(dst: &mut Vec<String>, s: &str) {
    dst.push(s.to_string());
}

// ───────────────────────── 文本槽遍历（wire 协议分叉） ─────────────────────────

/// 按 wire 协议遍历 body 内某一侧的可读文本槽。`f` 返回 `Some(new)` 即原地改写，
/// `None` 表示只读。收集与写回共用本函数，保证两趟遍历顺序严格一致。
fn visit_texts(
    body: &mut Value,
    wire: &Protocol,
    side: Side,
    f: &mut dyn FnMut(&str) -> Option<String>,
) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    match wire {
        Protocol::Anthropic => match side {
            Side::System => {
                if let Some(s) = obj.get_mut("system") {
                    visit_content(s, f);
                }
            }
            Side::Messages => {
                if let Some(Value::Array(msgs)) = obj.get_mut("messages") {
                    for m in msgs.iter_mut() {
                        if let Some(c) = m.get_mut("content") {
                            visit_content(c, f);
                        }
                    }
                }
            }
        },
        // OpenAI 无独立 system 字段：role=system|developer 的消息即 system 侧
        // （与 parse_incoming_request 把它们提到 chat_req.system 的口径一致）。
        Protocol::OpenAI => {
            if let Some(Value::Array(msgs)) = obj.get_mut("messages") {
                for m in msgs.iter_mut() {
                    let is_sys = matches!(
                        m.get("role").and_then(|r| r.as_str()),
                        Some("system") | Some("developer")
                    );
                    if is_sys != (side == Side::System) {
                        continue;
                    }
                    if let Some(c) = m.get_mut("content") {
                        visit_content(c, f);
                    }
                }
            }
        }
        // completions 无 system 概念，正文只有 prompt（字符串或字符串数组）。
        Protocol::OpenAICompletions => {
            if side == Side::Messages
                && let Some(p) = obj.get_mut("prompt")
            {
                match p {
                    Value::Array(items) => items.iter_mut().for_each(|i| visit_str(i, f)),
                    other => visit_str(other, f),
                }
            }
        }
        Protocol::OpenAIResponses => match side {
            Side::System => {
                if let Some(s) = obj.get_mut("instructions") {
                    visit_content(s, f);
                }
            }
            Side::Messages => {
                if let Some(input) = obj.get_mut("input") {
                    match input {
                        Value::Array(items) => {
                            for it in items.iter_mut() {
                                match it.get_mut("content") {
                                    Some(c) => visit_content(c, f),
                                    None => visit_str(it, f),
                                }
                            }
                        }
                        other => visit_str(other, f),
                    }
                }
            }
        },
        Protocol::Gemini => match side {
            Side::System => {
                // camelCase 为 REST 规范形态，snake_case 为部分 SDK 写法，两者都认。
                for k in ["systemInstruction", "system_instruction"] {
                    if let Some(si) = obj.get_mut(k) {
                        visit_parts(si, f);
                    }
                }
            }
            Side::Messages => {
                if let Some(Value::Array(items)) = obj.get_mut("contents") {
                    for c in items.iter_mut() {
                        visit_parts(c, f);
                    }
                }
            }
        },
        // 其余枚举值不是 wire 协议（平台类型），不会作为 endpoint 协议出现
        _ => {}
    }
}

/// content 形态：字符串，或 block/part 数组（凡带字符串 `text` 字段的块都算可读文本，
/// 覆盖 anthropic text block / openai text part / responses input_text|output_text）。
/// thinking / tool_use / image 等无 `text` 字段的块天然跳过。
fn visit_content(v: &mut Value, f: &mut dyn FnMut(&str) -> Option<String>) {
    match v {
        Value::Array(blocks) => {
            for b in blocks.iter_mut() {
                if let Some(t) = b.get_mut("text") {
                    visit_str(t, f);
                }
            }
        }
        other => visit_str(other, f),
    }
}

/// Gemini Content 节点：`parts[].text`。
fn visit_parts(node: &mut Value, f: &mut dyn FnMut(&str) -> Option<String>) {
    if let Some(Value::Array(parts)) = node.get_mut("parts") {
        for p in parts.iter_mut() {
            if let Some(t) = p.get_mut("text") {
                visit_str(t, f);
            }
        }
    }
}

fn visit_str(v: &mut Value, f: &mut dyn FnMut(&str) -> Option<String>) {
    if let Value::String(s) = v
        && let Some(new) = f(s)
    {
        *s = new;
    }
}

// ───────────────────────── inject: system_append 写回 ─────────────────────────

/// `system_append` 按 wire 协议写回。缺 system 结构时按协议规范形态补建。
fn append_system(body: &mut Value, wire: &Protocol, value: &str) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    match wire {
        Protocol::Anthropic => match obj.get_mut("system") {
            Some(Value::String(s)) => *s = format!("{s}\n{value}"),
            Some(Value::Array(blocks)) => {
                blocks.push(serde_json::json!({"type": "text", "text": value}))
            }
            _ => {
                obj.insert("system".to_string(), Value::String(value.to_string()));
            }
        },
        Protocol::OpenAI => {
            let msgs = obj
                .entry("messages".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            let Some(arr) = msgs.as_array_mut() else {
                return;
            };
            let existing = arr.iter_mut().find(|m| {
                matches!(
                    m.get("role").and_then(|r| r.as_str()),
                    Some("system") | Some("developer")
                ) && m.get("content").map(|c| c.is_string()).unwrap_or(false)
            });
            match existing {
                Some(m) => {
                    if let Some(Value::String(s)) = m.get_mut("content") {
                        *s = format!("{s}\n{value}");
                    }
                }
                None => arr.insert(0, serde_json::json!({"role": "system", "content": value})),
            }
        }
        Protocol::OpenAIResponses => match obj.get_mut("instructions") {
            Some(Value::String(s)) => *s = format!("{s}\n{value}"),
            _ => {
                obj.insert("instructions".to_string(), Value::String(value.to_string()));
            }
        },
        Protocol::Gemini => {
            let key = if obj.contains_key("system_instruction") {
                "system_instruction"
            } else {
                "systemInstruction"
            };
            let si = obj
                .entry(key.to_string())
                .or_insert_with(|| serde_json::json!({"parts": []}));
            match si.get_mut("parts").and_then(|p| p.as_array_mut()) {
                Some(parts) => parts.push(serde_json::json!({"text": value})),
                None => *si = serde_json::json!({"parts": [{"text": value}]}),
            }
        }
        // completions 协议无 system 角色，无处可注入（inject 对其为 no-op）。
        _ => {
            tracing::debug!(
                ?wire,
                "middleware inject system_append: protocol has no system slot, skipped"
            );
        }
    }
}

#[cfg(test)]
#[path = "test_middleware_body.rs"]
mod test_middleware_body;
