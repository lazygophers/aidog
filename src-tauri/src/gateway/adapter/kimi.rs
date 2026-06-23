use serde_json::Value;

use super::types::*;

/// Kimi (Moonshot) API 请求格式
/// 完全兼容 OpenAI Chat Completions API
/// 直接复用 OpenAI 转换逻辑
#[allow(dead_code)]
pub fn to_kimi(req: &ChatRequest) -> super::openai::OpenAIRequest {
    super::openai::to_openai(req)
}

/// Kimi SSE 与 OpenAI 完全兼容
#[allow(dead_code)]
pub fn parse_kimi_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_req() -> ChatRequest {
        ChatRequest {
            model: "moonshot-v1-8k".into(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text("test".into()),
            }],
            system: None,
            max_tokens: Some(200),
            temperature: Some(0.7),
            top_p: None,
            stream: Some(true),
            tools: None,
            tool_choice: None,
            extra: None,
        }
    }

    #[test]
    fn to_kimi_delegates_to_openai() {
        let req = sample_req();
        let result = to_kimi(&req);
        assert_eq!(result.model, "moonshot-v1-8k");
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.stream, Some(true));
        assert_eq!(result.temperature, Some(0.7));
    }

    #[test]
    fn parse_kimi_sse_stop_event() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        let result = parse_kimi_sse(&data);
        assert!(result.is_some());
        assert!(matches!(result, Some(ChatStreamEvent::Stop { .. })));
    }
}
