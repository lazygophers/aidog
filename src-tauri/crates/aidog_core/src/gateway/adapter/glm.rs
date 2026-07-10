use serde_json::Value;

use super::types::*;

/// GLM API 请求格式
/// 与 OpenAI 高度兼容，但有以下差异：
/// - tools 字段格式略有不同
/// - 部分模型支持 web_search 等特有功能
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GlmRequest {
    pub model: String,
    pub messages: Vec<super::openai::OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<super::openai::OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    /// GLM 特有：是否启用 web_search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search: Option<bool>,
}

/// 转为 GLM 格式（复用 OpenAI 消息转换，附加 GLM 特有字段）
#[allow(dead_code)]
pub fn to_glm(req: &ChatRequest) -> GlmRequest {
    let openai_req = super::openai::to_openai(req);
    GlmRequest {
        model: openai_req.model,
        messages: openai_req.messages,
        max_tokens: openai_req.max_tokens,
        temperature: openai_req.temperature,
        top_p: openai_req.top_p,
        stream: openai_req.stream,
        tools: openai_req.tools,
        tool_choice: openai_req.tool_choice,
        web_search: None,
    }
}

/// GLM SSE 与 OpenAI 兼容，直接复用
#[allow(dead_code)]
pub fn parse_glm_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_req() -> ChatRequest {
        ChatRequest {
            model: "glm-4-plus".into(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text("你好".into()),
            }],
            system: None,
            max_tokens: Some(1024),
            temperature: Some(0.9),
            top_p: Some(0.95),
            stream: Some(false),
            tools: None,
            tool_choice: None,
            extra: None,
        }
    }

    #[test]
    fn to_glm_produces_correct_fields() {
        let req = sample_req();
        let result = to_glm(&req);
        assert_eq!(result.model, "glm-4-plus");
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.max_tokens, Some(1024));
        assert_eq!(result.temperature, Some(0.9));
        assert_eq!(result.top_p, Some(0.95));
        assert_eq!(result.stream, Some(false));
        assert!(result.web_search.is_none(), "web_search defaults to None");
    }

    #[test]
    fn to_glm_serializes_without_null_fields() {
        let req = sample_req();
        let result = to_glm(&req);
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("\"web_search\":null"), "web_search=None must be skipped");
    }

    #[test]
    fn parse_glm_sse_content_delta() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": "GLM响应"},
                "finish_reason": null
            }]
        });
        let result = parse_glm_sse(&data);
        assert!(result.is_some(), "should parse GLM content delta");
        if let Some(ChatStreamEvent::Delta { text }) = result {
            assert_eq!(text, "GLM响应");
        } else {
            panic!("expected Delta event");
        }
    }

    #[test]
    fn parse_glm_sse_finish_reason_stop() {
        let data = serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        let result = parse_glm_sse(&data);
        assert!(result.is_some(), "should parse stop event");
        assert!(matches!(result, Some(ChatStreamEvent::Stop { .. })));
    }
}
