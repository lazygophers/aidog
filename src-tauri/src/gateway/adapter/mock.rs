//! Mock 平台类型：本地生成可控假响应，不转发真实上游。
//!
//! 配置三层覆盖（逐字段，优先级高 → 低）：
//! 1. 请求 body 顶层 `mock` 对象
//! 2. 请求 messages 的 role 映射（role 当 key，content 当 value）
//! 3. platform.extra JSON 的 `mock` 对象（兜底默认）

use serde::Deserialize;
use serde_json::{json, Value};

use super::types::*;

/// mock 场景配置。全字段 `#[serde(default)]`，空 extra → 全默认。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MockConfig {
    pub status_code: u16,
    pub delay_ms: u64,
    /// null=跟随请求 stream；Some(true/false)=强制
    pub stream_override: Option<bool>,
    pub response_text: String,
    pub finish_reason: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    /// none | http_error | rate_limit_429 | timeout
    pub error_mode: String,
    /// 流式时 response_text 切 N 块
    pub chunk_count: usize,
}

impl Default for MockConfig {
    fn default() -> Self {
        MockConfig {
            status_code: 200,
            delay_ms: 0,
            stream_override: None,
            response_text: "Hello from mock".to_string(),
            finish_reason: "end_turn".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cache_tokens: 0,
            error_mode: "none".to_string(),
            chunk_count: 5,
        }
    }
}

/// 解析最终生效的 mock 配置：extra 默认 → message role 覆盖 → body.mock 覆盖。
/// 每字段独立覆盖（缺省回退下层）。
pub fn resolve_mock_config(extra: &str, chat_req: &ChatRequest, body_json: &Value) -> MockConfig {
    // 第三层（兜底）：platform.extra 的 .mock
    let mut cfg: MockConfig = serde_json::from_str::<Value>(extra)
        .ok()
        .and_then(|v| v.get("mock").cloned())
        .and_then(|m| serde_json::from_value(m).ok())
        .unwrap_or_default();

    // 第二层：messages 的 role 映射（role ∈ 已知字段名时，content 为值）
    for msg in &chat_req.messages {
        let role = format!("{:?}", msg.role).to_lowercase();
        let content = match &msg.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        };
        apply_field(&mut cfg, &role, &content);
    }
    // role 映射也兼容原始 body messages（部分自定义 role 不在 Role enum 内，
    // 会被 parse_incoming_request 归一化丢失），直接从原始 body 再扫一遍。
    if let Some(messages) = body_json.get("messages").and_then(|v| v.as_array()) {
        for m in messages {
            if let (Some(role), Some(content)) = (
                m.get("role").and_then(|v| v.as_str()),
                m.get("content").and_then(|v| v.as_str()),
            ) {
                apply_field(&mut cfg, role, content);
            }
        }
    }

    // 第一层（最高）：body 顶层 mock 对象
    if let Some(mock_obj) = body_json.get("mock").and_then(|v| v.as_object()) {
        if let Some(v) = mock_obj.get("status_code").and_then(|v| v.as_u64()) {
            cfg.status_code = v as u16;
        }
        if let Some(v) = mock_obj.get("delay_ms").and_then(|v| v.as_u64()) {
            cfg.delay_ms = v;
        }
        if let Some(v) = mock_obj.get("stream_override").and_then(|v| v.as_bool()) {
            cfg.stream_override = Some(v);
        }
        if let Some(v) = mock_obj.get("response_text").and_then(|v| v.as_str()) {
            cfg.response_text = v.to_string();
        }
        if let Some(v) = mock_obj.get("finish_reason").and_then(|v| v.as_str()) {
            cfg.finish_reason = v.to_string();
        }
        if let Some(v) = mock_obj.get("input_tokens").and_then(|v| v.as_i64()) {
            cfg.input_tokens = v as i32;
        }
        if let Some(v) = mock_obj.get("output_tokens").and_then(|v| v.as_i64()) {
            cfg.output_tokens = v as i32;
        }
        if let Some(v) = mock_obj.get("cache_tokens").and_then(|v| v.as_i64()) {
            cfg.cache_tokens = v as i32;
        }
        if let Some(v) = mock_obj.get("error_mode").and_then(|v| v.as_str()) {
            cfg.error_mode = v.to_string();
        }
        if let Some(v) = mock_obj.get("chunk_count").and_then(|v| v.as_u64()) {
            cfg.chunk_count = v as usize;
        }
    }

    cfg
}

/// 按 role(=字段名) / content(=值) 覆盖单个字段。
fn apply_field(cfg: &mut MockConfig, field: &str, value: &str) {
    match field {
        "input_tokens" => {
            if let Ok(v) = value.trim().parse() {
                cfg.input_tokens = v;
            }
        }
        "output_tokens" => {
            if let Ok(v) = value.trim().parse() {
                cfg.output_tokens = v;
            }
        }
        "cache_tokens" => {
            if let Ok(v) = value.trim().parse() {
                cfg.cache_tokens = v;
            }
        }
        "status_code" => {
            if let Ok(v) = value.trim().parse() {
                cfg.status_code = v;
            }
        }
        "delay_ms" => {
            if let Ok(v) = value.trim().parse() {
                cfg.delay_ms = v;
            }
        }
        "response_text" => {
            cfg.response_text = value.to_string();
        }
        "error_mode" => {
            cfg.error_mode = value.trim().to_string();
        }
        _ => {}
    }
}

/// 按 source_protocol 构造非流式假响应 JSON body。
/// 假 token 注入各协议各自的 usage 字段。
pub fn build_response(cfg: &MockConfig, source_protocol: &str, model: &str) -> Value {
    let id = format!("mock-{}", uuid::Uuid::new_v4().simple());
    let text = cfg.response_text.clone();
    let input = cfg.input_tokens;
    let output = cfg.output_tokens;
    let cache = cfg.cache_tokens;

    match source_protocol {
        "openai" => json!({
            "id": id,
            "object": "chat.completion",
            "model": model,
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": text },
                "finish_reason": openai_finish(&cfg.finish_reason)
            }],
            "usage": {
                "prompt_tokens": input,
                "completion_tokens": output,
                "total_tokens": input + output,
                "prompt_tokens_details": { "cached_tokens": cache }
            }
        }),
        "openai_completions" => json!({
            "id": id,
            "object": "text_completion",
            "model": model,
            "choices": [{
                "text": text,
                "index": 0,
                "logprobs": null,
                "finish_reason": openai_finish(&cfg.finish_reason)
            }],
            "usage": {
                "prompt_tokens": input,
                "completion_tokens": output,
                "total_tokens": input + output
            }
        }),
        "openai_responses" => json!({
            "id": id,
            "object": "response",
            "model": model,
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{ "type": "output_text", "text": text }]
            }],
            "usage": {
                "input_tokens": input,
                "output_tokens": output,
                "total_tokens": input + output
            }
        }),
        "gemini" => json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": text }],
                    "role": "model"
                },
                "finishReason": gemini_finish(&cfg.finish_reason),
                "index": 0
            }],
            "modelVersion": model,
            "usageMetadata": {
                "promptTokenCount": input,
                "candidatesTokenCount": output,
                "cachedContentTokenCount": cache,
                "totalTokenCount": input + output
            }
        }),
        // 默认 Anthropic 格式
        _ => json!({
            "id": id,
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [{ "type": "text", "text": text }],
            "stop_reason": cfg.finish_reason,
            "stop_sequence": null,
            "usage": {
                "input_tokens": input,
                "output_tokens": output,
                "cache_read_input_tokens": cache
            }
        }),
    }
}

/// 构造 mock 错误响应 body（按协议错误格式）。
pub fn build_error_body(source_protocol: &str, status_code: u16, message: &str) -> Value {
    match source_protocol {
        "gemini" => json!({
            "error": { "code": status_code, "message": message, "status": "MOCK_ERROR" }
        }),
        "openai" | "openai_responses" | "openai_completions" => json!({
            "error": { "message": message, "type": "mock_error", "code": status_code }
        }),
        // anthropic
        _ => json!({
            "type": "error",
            "error": { "type": "mock_error", "message": message }
        }),
    }
}

/// 生成 mock 流式 SSE 字符串序列：Start → N×Delta → Stop。
/// 复用 `to_client_sse` 按 source_protocol 转格式。
pub fn build_sse_chunks(cfg: &MockConfig, source_protocol: &str, model: &str) -> Vec<String> {
    let id = format!("mock-{}", uuid::Uuid::new_v4().simple());
    let mut events: Vec<ChatStreamEvent> = Vec::new();
    events.push(ChatStreamEvent::Start {
        id,
        model: model.to_string(),
    });
    for piece in split_text(&cfg.response_text, cfg.chunk_count) {
        events.push(ChatStreamEvent::Delta { text: piece });
    }
    events.push(ChatStreamEvent::Stop {
        finish_reason: Some(cfg.finish_reason.clone()),
    });

    events
        .iter()
        .filter_map(|e| super::converter::to_client_sse(e, source_protocol, model))
        .collect()
}

/// 将文本切成 n 块（n<=1 或文本空则单块）。
fn split_text(text: &str, n: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let chars: Vec<char> = text.chars().collect();
    let n = n.max(1).min(chars.len());
    let chunk_size = chars.len().div_ceil(n);
    chars
        .chunks(chunk_size)
        .map(|c| c.iter().collect::<String>())
        .collect()
}

fn openai_finish(reason: &str) -> &str {
    match reason {
        "end_turn" => "stop",
        other => other,
    }
}

fn gemini_finish(reason: &str) -> &str {
    match reason {
        "end_turn" | "stop" => "STOP",
        "max_tokens" => "MAX_TOKENS",
        _ => "STOP",
    }
}
