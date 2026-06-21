//! mock 非流式响应 / 错误 body 构造（按 source_protocol 分形态）。

use serde_json::{json, Value};

use super::config::MockConfig;

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

#[cfg(test)]
#[path = "test_response.rs"]
mod test_response;
