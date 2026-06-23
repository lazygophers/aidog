use serde_json::Value;

use super::types::*;

/// DashScope (阿里百炼) 兼容 OpenAI Chat Completions 格式
/// 直接复用 OpenAI 的请求/响应结构
pub use super::openai::OpenAIRequest;

/// 从内部 ChatRequest 转为百炼 (OpenAI-compatible) 格式
#[allow(dead_code)]
pub fn to_bailian(req: &ChatRequest) -> OpenAIRequest {
    // DashScope 兼容 OpenAI 格式，逻辑一致
    super::openai::to_openai(req)
}

/// 解析百炼 SSE 格式的流式事件（与 OpenAI 格式一致）
#[allow(dead_code)]
pub fn parse_bailian_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_req() -> ChatRequest {
        ChatRequest {
            model: "qwen-plus".into(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text("hello".into()),
            }],
            system: None,
            max_tokens: Some(100),
            temperature: None,
            top_p: None,
            stream: Some(false),
            tools: None,
            tool_choice: None,
            extra: None,
        }
    }

    #[test]
    fn to_bailian_delegates_to_openai() {
        let req = sample_req();
        let result = to_bailian(&req);
        assert_eq!(result.model, "qwen-plus");
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.max_tokens, Some(100));
    }

    #[test]
    fn parse_bailian_sse_content_delta() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": "hello"},
                "finish_reason": null
            }]
        });
        let result = parse_bailian_sse(&data);
        assert!(result.is_some(), "should parse content delta");
        // Verify it's a Delta event with content
        if let Some(ChatStreamEvent::Delta { text }) = result {
            assert_eq!(text, "hello");
        } else {
            panic!("expected Delta event");
        }
    }

    #[test]
    fn parse_bailian_sse_none_on_missing_choices() {
        let data = serde_json::json!({"object": "chat.completion.chunk"});
        assert!(parse_bailian_sse(&data).is_none());
    }
}
