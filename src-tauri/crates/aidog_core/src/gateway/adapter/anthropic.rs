use serde_json::Value;

use super::types::*;

/// Anthropic Messages API 请求格式
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Value>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnthropicTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

/// 从内部 ChatRequest 转为 Anthropic 格式
pub fn to_anthropic(req: &ChatRequest) -> AnthropicRequest {
    let messages: Vec<AnthropicMessage> = req
        .messages
        .iter()
        .filter(|m| !matches!(m.role, Role::System))
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "user", // Anthropic 没有 tool role
                _ => "user",
            };
            let content = match &m.content {
                MessageContent::Text(s) => Value::String(s.clone()),
                MessageContent::Blocks(blocks) => {
                    let arr: Vec<Value> = blocks
                        .iter()
                        .filter_map(|b| match b {
                            // 只保留已知类型;Unknown(thinking/redacted_thinking/image 等)跳过,
                            // 避免上游不支持 Anthropic 扩展类型导致 400 InvalidParameter
                            ContentBlock::Text { .. } | ContentBlock::ToolUse { .. } | ContentBlock::ToolResult { .. } => {
                                Some(serde_json::to_value(b).unwrap())
                            }
                            ContentBlock::Unknown(_) => None,
                        })
                        .collect();
                    Value::Array(arr)
                }
            };
            AnthropicMessage { role: role.to_string(), content }
        })
        .collect();

    let tools = req.tools.as_ref().map(|ts| {
        ts.iter()
            .map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect()
    });

    AnthropicRequest {
        model: req.model.clone(),
        messages,
        system: req.system.as_ref().map(|s| match s {
            SystemContent::Text(t) => Value::String(t.clone()),
            SystemContent::Blocks(blocks) => Value::Array(blocks.clone()),
        }),
        max_tokens: req.max_tokens.unwrap_or(4096),
        temperature: req.temperature,
        top_p: req.top_p,
        stream: req.stream,
        tools,
        tool_choice: req.tool_choice.as_ref().and_then(|tc| {
            match tc {
                ToolChoice::Auto => Some(serde_json::json!({"type": "auto"})),
                ToolChoice::Any => Some(serde_json::json!({"type": "any"})),
                ToolChoice::None => None,
                ToolChoice::Named { name } => Some(serde_json::json!({"type": "tool", "name": name})),
            }
        }),
    }
}

/// 从 Anthropic 响应格式转回内部格式（解析 Anthropic SSE event data）
pub fn parse_anthropic_sse(data: &Value) -> Option<ChatStreamEvent> {
    let event_type = data.get("type")?.as_str()?;
    match event_type {
        "message_start" => {
            let msg = data.get("message")?;
            Some(ChatStreamEvent::Start {
                id: msg.get("id")?.as_str()?.to_string(),
                model: msg.get("model")?.as_str()?.to_string(),
            })
        }
        "content_block_delta" => {
            let delta = data.get("delta")?;
            let delta_type = delta.get("type")?.as_str()?;
            match delta_type {
                "text_delta" => Some(ChatStreamEvent::Delta {
                    text: delta.get("text")?.as_str()?.to_string(),
                }),
                "input_json_delta" => Some(ChatStreamEvent::ToolDelta {
                    index: data.get("index")?.as_u64()? as u32,
                    id: None,
                    name: None,
                    input: delta.get("partial_json").and_then(|v| v.as_str()).map(|s| s.to_string()),
                }),
                _ => None,
            }
        }
        "content_block_start" => {
            let cb = data.get("content_block")?;
            match cb.get("type")?.as_str()? {
                "tool_use" => Some(ChatStreamEvent::ToolDelta {
                    index: data.get("index")?.as_u64()? as u32,
                    id: cb.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    name: cb.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    input: None,
                }),
                _ => None,
            }
        }
        "message_delta" => {
            let delta = data.get("delta")?;
            let stop_reason = delta.get("stop_reason").and_then(|v| v.as_str()).map(|s| s.to_string());
            Some(ChatStreamEvent::Stop {
                finish_reason: stop_reason,
            })
        }
        "message_stop" => Some(ChatStreamEvent::Stop {
            finish_reason: Some("stop".to_string()),
        }),
        _ => None,
    }
}

#[cfg(test)]
#[path = "test_anthropic.rs"]
mod test_anthropic;
