use serde_json::Value;

use super::types::*;

/// Codex (OpenAI) API 请求格式
/// 完全兼容 OpenAI Chat Completions API
#[allow(dead_code)]
pub fn to_codex(req: &ChatRequest) -> super::openai::OpenAIRequest {
    super::openai::to_openai(req)
}

/// Codex SSE 与 OpenAI 完全兼容
#[allow(dead_code)]
pub fn parse_codex_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_req() -> ChatRequest {
        ChatRequest {
            model: "gpt-4o".into(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text("hello codex".into()),
            }],
            system: None,
            max_tokens: Some(512),
            temperature: Some(0.5),
            top_p: None,
            stream: Some(true),
            tools: None,
            tool_choice: None,
            extra: None,
        }
    }

    #[test]
    fn to_codex_delegates_to_openai() {
        let req = sample_req();
        let result = to_codex(&req);
        assert_eq!(result.model, "gpt-4o");
        assert_eq!(result.max_tokens, Some(512));
        assert_eq!(result.stream, Some(true));
    }

    #[test]
    fn parse_codex_sse_content_delta() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": "test response"},
                "finish_reason": null
            }]
        });
        let result = parse_codex_sse(&data);
        assert!(result.is_some(), "should parse content delta");
        assert!(matches!(result, Some(ChatStreamEvent::Delta { .. })));
    }

    #[test]
    fn parse_codex_sse_finish() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        let result = parse_codex_sse(&data);
        assert!(result.is_some(), "should parse stop event");
        assert!(matches!(result, Some(ChatStreamEvent::Stop { .. })));
    }
}
