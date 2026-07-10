use serde_json::Value;

use super::types::*;

/// OpenAI Legacy Completions API (`/v1/completions`) 请求格式
/// 使用 `prompt` 字段而非 `messages`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompletionsRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// 转为 Completions 格式：将 messages 拼接为 prompt
pub fn to_completions(req: &ChatRequest) -> CompletionsRequest {
    let prompt = req.messages.iter().map(|m| {
        let role_str = match m.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
            Role::Tool => "Tool",
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
        format!("{}: {}", role_str, text)
    }).collect::<Vec<_>>()
    .join("\n\n");

    CompletionsRequest {
        model: req.model.clone(),
        prompt,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        stream: req.stream,
        stop: None,
    }
}

/// 从 Completions API 请求解析为内部 ChatRequest
/// 将 prompt 字符串拆分为单条 User 消息
pub fn from_completions(body: &Value) -> Option<ChatRequest> {
    let model = body.get("model")?.as_str()?.to_string();
    let prompt = body.get("prompt")?.as_str().unwrap_or("").to_string();

    Some(ChatRequest {
        model,
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(prompt),
        }],
        system: None,
        max_tokens: body.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
        temperature: body.get("temperature").and_then(|v| v.as_f64()).map(|v| v as f32),
        top_p: body.get("top_p").and_then(|v| v.as_f64()).map(|v| v as f32),
        stream: body.get("stream").and_then(|v| v.as_bool()),
        tools: None,
        tool_choice: None,
        extra: None,
    })
}

/// Completions SSE 解析（与 Chat Completions 兼容）
#[allow(dead_code)]
pub fn parse_completions_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}

/// 将 ChatStreamEvent 转为 Completions SSE 格式
#[allow(dead_code)]
pub fn to_completions_sse(event: &ChatStreamEvent, model: &str) -> Option<String> {
    super::openai::to_openai_sse(event, model)
}

#[cfg(test)]
#[path = "test_openai_completions.rs"]
mod test_openai_completions;
