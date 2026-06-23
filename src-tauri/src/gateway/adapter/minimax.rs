use serde_json::Value;

use super::types::*;

/// MiniMax API 请求格式
/// 兼容 OpenAI Chat Completions API，复用 OpenAI 消息转换
#[allow(dead_code)]
pub fn to_minimax(req: &ChatRequest) -> super::openai::OpenAIRequest {
    super::openai::to_openai(req)
}

/// MiniMax SSE 与 OpenAI 兼容，直接复用
#[allow(dead_code)]
pub fn parse_minimax_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_req() -> ChatRequest {
        ChatRequest {
            model: "abab6.5s-chat".into(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text("hi".into()),
            }],
            system: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: Some(false),
            tools: None,
            tool_choice: None,
            extra: None,
        }
    }

    #[test]
    fn to_minimax_delegates_to_openai() {
        let req = sample_req();
        let result = to_minimax(&req);
        assert_eq!(result.model, "abab6.5s-chat");
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.stream, Some(false));
    }

    #[test]
    fn parse_minimax_sse_content_delta() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": "world"},
                "finish_reason": null
            }]
        });
        let result = parse_minimax_sse(&data);
        assert!(result.is_some());
        assert!(matches!(result, Some(ChatStreamEvent::Delta { .. })));
    }
}
