use crate::converter::NonStreamResponse;
use crate::converter::traits::ProtocolConverter;
use crate::types::*;
use serde_json::Value;

mod parse;
mod request;
mod response;
mod sse;

pub use parse::from_openai;
pub use request::to_openai;
pub use response::{parse_openai_response, render_openai_response};
pub use sse::{parse_openai_sse, to_openai_sse};

/// OpenAI 协议转换器实现
pub struct OpenAIConverter;

impl ProtocolConverter for OpenAIConverter {
    fn protocol_name(&self) -> &'static str {
        "openai"
    }

    fn parse_incoming(&self, body: &[u8]) -> Result<ChatRequest, String> {
        let value: Value =
            serde_json::from_slice(body).map_err(|e| format!("OpenAI parse error: {}", e))?;
        from_openai(&value).ok_or_else(|| "OpenAI parse failed".to_string())
    }

    fn serialize_request(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let openai_req = to_openai(req);
        let body = serde_json::to_value(openai_req)
            .map_err(|e| format!("OpenAI serialize error: {}", e))?;
        Ok((body, "/v1/chat/completions".to_string()))
    }

    fn parse_sse(&self, chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        let value: Value =
            serde_json::from_slice(chunk).map_err(|e| format!("OpenAI SSE parse error: {}", e))?;
        parse_openai_sse(&value)
            .map(|e| vec![e])
            .ok_or_else(|| "OpenAI SSE parse failed".to_string())
    }

    fn to_client_sse(&self, event: &ChatStreamEvent) -> Result<String, String> {
        to_openai_sse(event, "openai").ok_or_else(|| "OpenAI to_client_sse failed".to_string())
    }

    fn parse_response(&self, body: &[u8]) -> Result<NonStreamResponse, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("OpenAI response parse error: {}", e))?;
        parse_openai_response(&value, "").ok_or_else(|| "OpenAI response parse failed".to_string())
    }
}

/// OpenAI Chat Completions 请求格式（GLM/Kimi 也兼容）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 新版 OpenAI SDK / o 系列模型发的输出长度键（`max_tokens` 的官方继任者）。
    /// 入站在 `from_openai` 归一到 `ChatRequest::max_tokens`；出站恒为 `None`
    /// （`to_openai` 只写 `max_tokens`，官方 host 的键名改写在 forward 层做，票 05）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAITool {
    r#type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAIFunction {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAIToolCall {
    id: String,
    r#type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}
