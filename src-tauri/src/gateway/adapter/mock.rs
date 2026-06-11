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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 最小 ChatRequest（messages 用 Anthropic 兼容结构，role 仅 User/Assistant/System/Tool）。
    fn chat_req(messages: Vec<Message>) -> ChatRequest {
        ChatRequest {
            model: "mock-model".to_string(),
            messages,
            system: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: None,
            tools: None,
            tool_choice: None,
            extra: None,
        }
    }

    fn empty_req() -> ChatRequest {
        chat_req(vec![])
    }

    // ─── 三层覆盖优先级 ──────────────────────────────────────

    #[test]
    fn empty_extra_yields_defaults() {
        let cfg = resolve_mock_config("", &empty_req(), &json!({}));
        let def = MockConfig::default();
        assert_eq!(cfg.status_code, def.status_code);
        assert_eq!(cfg.input_tokens, def.input_tokens);
        assert_eq!(cfg.output_tokens, def.output_tokens);
        assert_eq!(cfg.cache_tokens, def.cache_tokens);
        assert_eq!(cfg.response_text, def.response_text);
        assert_eq!(cfg.error_mode, "none");
        assert_eq!(cfg.chunk_count, def.chunk_count);
    }

    #[test]
    fn extra_layer_applied() {
        let extra = r#"{"mock":{"input_tokens":11,"output_tokens":22,"cache_tokens":33,"status_code":201,"response_text":"from-extra","error_mode":"http_error","chunk_count":3}}"#;
        let cfg = resolve_mock_config(extra, &empty_req(), &json!({}));
        assert_eq!(cfg.input_tokens, 11);
        assert_eq!(cfg.output_tokens, 22);
        assert_eq!(cfg.cache_tokens, 33);
        assert_eq!(cfg.status_code, 201);
        assert_eq!(cfg.response_text, "from-extra");
        assert_eq!(cfg.error_mode, "http_error");
        assert_eq!(cfg.chunk_count, 3);
    }

    #[test]
    fn message_role_layer_overrides_extra() {
        // 第二层走原始 body messages 扫描（自定义 role 名当字段 key）。
        let extra = r#"{"mock":{"input_tokens":11,"output_tokens":22}}"#;
        let body = json!({
            "messages": [
                {"role": "input_tokens", "content": "555"},
                {"role": "status_code", "content": "503"},
            ]
        });
        let cfg = resolve_mock_config(extra, &empty_req(), &body);
        // 被 message role 覆盖
        assert_eq!(cfg.input_tokens, 555);
        assert_eq!(cfg.status_code, 503);
        // 未被覆盖字段回退 extra
        assert_eq!(cfg.output_tokens, 22);
    }

    #[test]
    fn body_mock_overrides_all_layers() {
        let extra = r#"{"mock":{"input_tokens":11,"output_tokens":22,"status_code":201}}"#;
        let body = json!({
            "messages": [
                {"role": "input_tokens", "content": "555"},
            ],
            "mock": { "input_tokens": 999, "status_code": 429 }
        });
        let cfg = resolve_mock_config(extra, &empty_req(), &body);
        // body.mock 最高优先级
        assert_eq!(cfg.input_tokens, 999);
        assert_eq!(cfg.status_code, 429);
        // 未在 body.mock 出现的字段回退（message 层无，回退 extra）
        assert_eq!(cfg.output_tokens, 22);
    }

    #[test]
    fn per_field_independent_fallback() {
        // body 只覆盖 output_tokens；input 回退 message 层；cache 回退 extra。
        let extra = r#"{"mock":{"input_tokens":1,"output_tokens":2,"cache_tokens":3}}"#;
        let body = json!({
            "messages": [ {"role": "input_tokens", "content": "100"} ],
            "mock": { "output_tokens": 200 }
        });
        let cfg = resolve_mock_config(extra, &empty_req(), &body);
        assert_eq!(cfg.input_tokens, 100); // message 层
        assert_eq!(cfg.output_tokens, 200); // body 层
        assert_eq!(cfg.cache_tokens, 3); // extra 兜底
    }

    // ─── 5 协议非流式 build_response shape ─────────────────────

    fn cfg_with_tokens() -> MockConfig {
        MockConfig {
            input_tokens: 100,
            output_tokens: 50,
            cache_tokens: 7,
            response_text: "hello-mock".to_string(),
            finish_reason: "end_turn".to_string(),
            ..MockConfig::default()
        }
    }

    #[test]
    fn build_response_anthropic_shape() {
        let v = build_response(&cfg_with_tokens(), "anthropic", "claude-x");
        assert_eq!(v["type"], "message");
        assert_eq!(v["role"], "assistant");
        assert_eq!(v["model"], "claude-x");
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["content"][0]["text"], "hello-mock");
        assert_eq!(v["stop_reason"], "end_turn");
        assert_eq!(v["usage"]["input_tokens"], 100);
        assert_eq!(v["usage"]["output_tokens"], 50);
        assert_eq!(v["usage"]["cache_read_input_tokens"], 7);
    }

    #[test]
    fn build_response_openai_shape() {
        let v = build_response(&cfg_with_tokens(), "openai", "gpt-x");
        assert_eq!(v["object"], "chat.completion");
        assert_eq!(v["model"], "gpt-x");
        assert_eq!(v["choices"][0]["message"]["role"], "assistant");
        assert_eq!(v["choices"][0]["message"]["content"], "hello-mock");
        // end_turn → stop 归一化
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["prompt_tokens"], 100);
        assert_eq!(v["usage"]["completion_tokens"], 50);
        assert_eq!(v["usage"]["total_tokens"], 150);
        assert_eq!(v["usage"]["prompt_tokens_details"]["cached_tokens"], 7);
    }

    #[test]
    fn build_response_openai_completions_shape() {
        let v = build_response(&cfg_with_tokens(), "openai_completions", "gpt-x");
        assert_eq!(v["object"], "text_completion");
        assert_eq!(v["choices"][0]["text"], "hello-mock");
        assert_eq!(v["choices"][0]["index"], 0);
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["prompt_tokens"], 100);
        assert_eq!(v["usage"]["completion_tokens"], 50);
        assert_eq!(v["usage"]["total_tokens"], 150);
    }

    #[test]
    fn build_response_openai_responses_shape() {
        let v = build_response(&cfg_with_tokens(), "openai_responses", "gpt-x");
        assert_eq!(v["object"], "response");
        assert_eq!(v["status"], "completed");
        assert_eq!(v["output"][0]["type"], "message");
        assert_eq!(v["output"][0]["content"][0]["type"], "output_text");
        assert_eq!(v["output"][0]["content"][0]["text"], "hello-mock");
        assert_eq!(v["usage"]["input_tokens"], 100);
        assert_eq!(v["usage"]["output_tokens"], 50);
        assert_eq!(v["usage"]["total_tokens"], 150);
    }

    #[test]
    fn build_response_gemini_shape() {
        let v = build_response(&cfg_with_tokens(), "gemini", "gemini-x");
        assert_eq!(v["candidates"][0]["content"]["parts"][0]["text"], "hello-mock");
        assert_eq!(v["candidates"][0]["content"]["role"], "model");
        assert_eq!(v["candidates"][0]["finishReason"], "STOP");
        assert_eq!(v["usageMetadata"]["promptTokenCount"], 100);
        assert_eq!(v["usageMetadata"]["candidatesTokenCount"], 50);
        assert_eq!(v["usageMetadata"]["cachedContentTokenCount"], 7);
        assert_eq!(v["usageMetadata"]["totalTokenCount"], 150);
    }

    #[test]
    fn build_response_unknown_protocol_falls_back_anthropic() {
        let v = build_response(&cfg_with_tokens(), "weird-proto", "m");
        assert_eq!(v["type"], "message");
        assert_eq!(v["content"][0]["text"], "hello-mock");
    }

    // ─── SSE build_sse_chunks 序列 ───────────────────────────

    #[test]
    fn sse_anthropic_start_delta_stop_sequence() {
        let cfg = MockConfig {
            response_text: "abcdef".to_string(),
            chunk_count: 3,
            finish_reason: "end_turn".to_string(),
            ..MockConfig::default()
        };
        let chunks = build_sse_chunks(&cfg, "anthropic", "claude-x");
        // 1 Start + 3 Delta + 1 Stop = 5
        assert_eq!(chunks.len(), 5);
        assert!(chunks[0].contains("message_start"));
        assert!(chunks[1].contains("content_block_delta"));
        assert!(chunks[2].contains("content_block_delta"));
        assert!(chunks[3].contains("content_block_delta"));
        assert!(chunks[4].contains("message_stop"));
        // chunk_count=3，6 字符 → 每块 2 字符
        assert!(chunks[1].contains("\"text\":\"ab\""));
        assert!(chunks[2].contains("\"text\":\"cd\""));
        assert!(chunks[3].contains("\"text\":\"ef\""));
    }

    #[test]
    fn sse_openai_sequence_has_done() {
        let cfg = MockConfig {
            response_text: "xy".to_string(),
            chunk_count: 2,
            ..MockConfig::default()
        };
        let chunks = build_sse_chunks(&cfg, "openai", "gpt-x");
        // Start + 2 Delta + Stop
        assert_eq!(chunks.len(), 4);
        assert!(chunks[0].contains("chat.completion.chunk"));
        assert!(chunks.last().unwrap().contains("[DONE]"));
    }

    #[test]
    fn sse_chunk_count_capped_to_text_len() {
        // chunk_count 大于文本长度时按字符数封顶。
        let cfg = MockConfig {
            response_text: "ab".to_string(),
            chunk_count: 10,
            ..MockConfig::default()
        };
        let chunks = build_sse_chunks(&cfg, "anthropic", "m");
        // 2 字符 → 最多 2 Delta：Start + 2 Delta + Stop = 4
        assert_eq!(chunks.len(), 4);
    }

    #[test]
    fn sse_empty_text_yields_single_delta() {
        let cfg = MockConfig {
            response_text: String::new(),
            chunk_count: 5,
            ..MockConfig::default()
        };
        let chunks = build_sse_chunks(&cfg, "anthropic", "m");
        // 空文本 → split_text 返单块 → Start + 1 Delta + Stop = 3
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn split_text_basic() {
        assert_eq!(split_text("abcd", 2), vec!["ab", "cd"]);
        assert_eq!(split_text("abc", 2), vec!["ab", "c"]);
        assert_eq!(split_text("", 5), vec![String::new()]);
        // n=0 视为 1 块
        assert_eq!(split_text("abc", 0), vec!["abc"]);
    }

    // ─── build_error_body 各协议 shape ───────────────────────

    #[test]
    fn error_body_anthropic_shape() {
        let v = build_error_body("anthropic", 500, "boom");
        assert_eq!(v["type"], "error");
        assert_eq!(v["error"]["type"], "mock_error");
        assert_eq!(v["error"]["message"], "boom");
    }

    #[test]
    fn error_body_openai_shape() {
        for proto in ["openai", "openai_responses", "openai_completions"] {
            let v = build_error_body(proto, 429, "rate limited");
            assert_eq!(v["error"]["message"], "rate limited", "proto {proto}");
            assert_eq!(v["error"]["type"], "mock_error", "proto {proto}");
            assert_eq!(v["error"]["code"], 429, "proto {proto}");
        }
    }

    #[test]
    fn error_body_gemini_shape() {
        let v = build_error_body("gemini", 503, "unavailable");
        assert_eq!(v["error"]["code"], 503);
        assert_eq!(v["error"]["message"], "unavailable");
        assert_eq!(v["error"]["status"], "MOCK_ERROR");
    }

    // ─── error_mode 字段解析（语义在 handle_mock，纯函数侧验证配置可控） ──

    #[test]
    fn error_mode_variants_parse() {
        for mode in ["none", "http_error", "rate_limit_429", "timeout"] {
            let extra = format!(r#"{{"mock":{{"error_mode":"{mode}"}}}}"#);
            let cfg = resolve_mock_config(&extra, &empty_req(), &json!({}));
            assert_eq!(cfg.error_mode, mode);
        }
    }

    #[test]
    fn stream_override_and_delay_parse() {
        let extra = r#"{"mock":{"stream_override":true,"delay_ms":1500}}"#;
        let cfg = resolve_mock_config(extra, &empty_req(), &json!({}));
        assert_eq!(cfg.stream_override, Some(true));
        assert_eq!(cfg.delay_ms, 1500);
    }

    #[test]
    fn parsed_message_role_layer_for_standard_roles() {
        // 标准 Role（user/assistant/system/tool）经 ChatRequest.messages 扫描，
        // 这些 role 名不匹配任何 mock 字段，不应改动配置。
        let req = chat_req(vec![Message {
            role: Role::User,
            content: MessageContent::Text("ignored".to_string()),
        }]);
        let cfg = resolve_mock_config("", &req, &json!({}));
        let def = MockConfig::default();
        assert_eq!(cfg.input_tokens, def.input_tokens);
        assert_eq!(cfg.output_tokens, def.output_tokens);
        assert_eq!(cfg.status_code, def.status_code);
        assert_eq!(cfg.response_text, def.response_text);
    }
}
