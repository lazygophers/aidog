use serde_json::Value;

use super::types::*;

/// OpenAI Responses API (`/v1/responses`) 请求格式
/// 使用 `input` 而非 `messages`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: Vec<ResponsesInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResponsesInput {
    pub role: String,
    pub content: String,
}

/// 转为 Responses API 格式
pub fn to_responses(req: &ChatRequest) -> ResponsesRequest {
    let input: Vec<ResponsesInput> = req.messages.iter().map(|m| {
        let role_str = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        };
        let text = match &m.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => blocks.iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        };
        ResponsesInput { role: role_str.to_string(), content: text }
    }).collect();

    ResponsesRequest {
        model: req.model.clone(),
        input,
        max_output_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        stream: req.stream,
        tools: None,
    }
}

/// 从 Responses API 请求解析为内部 ChatRequest
pub fn from_responses(body: &Value) -> Option<ChatRequest> {
    let model = body.get("model")?.as_str()?.to_string();
    let input = body.get("input")?.as_array()?;

    let mut messages = Vec::new();
    for item in input {
        let role_str = item.get("role")?.as_str()?.to_lowercase();
        let role = match role_str.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            "tool" => Role::Tool,
            _ => Role::User,
        };
        let content = item.get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        messages.push(Message {
            role,
            content: MessageContent::Text(content),
        });
    }

    Some(ChatRequest {
        model,
        messages,
        system: None,
        max_tokens: body.get("max_output_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
        temperature: body.get("temperature").and_then(|v| v.as_f64()).map(|v| v as f32),
        top_p: body.get("top_p").and_then(|v| v.as_f64()).map(|v| v as f32),
        stream: body.get("stream").and_then(|v| v.as_bool()),
        tools: None,
        tool_choice: None,
        extra: None,
    })
}

/// Responses API SSE 解析（与 OpenAI Chat 兼容）
#[allow(dead_code)]
pub fn parse_responses_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}

/// 将 ChatStreamEvent 转为 Responses API SSE 格式
#[allow(dead_code)]
pub fn to_responses_sse(event: &ChatStreamEvent, model: &str) -> Option<String> {
    // Responses API SSE 与 OpenAI Chat Completions 格式相似
    super::openai::to_openai_sse(event, model)
}
