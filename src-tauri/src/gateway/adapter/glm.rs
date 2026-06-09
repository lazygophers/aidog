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
pub fn parse_glm_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}
