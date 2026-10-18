
use serde_json::Value;

mod parse;
mod request;
mod response;
mod sse;

pub use parse::from_openai;
pub use request::to_openai;
pub use response::{parse_openai_response, render_openai_response};
pub use sse::{parse_openai_sse, to_openai_sse};

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
