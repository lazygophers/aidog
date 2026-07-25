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

/// 解析 Anthropic Messages API 非流式响应为归一 NonStreamResponse
pub fn parse_anthropic_response(body: &Value, fallback_model: &str) -> Option<super::converter::NonStreamResponse> {
    let id = body.get("id")?.as_str()?.to_string();
    let model = body.get("model")?.as_str().unwrap_or(fallback_model).to_string();

    let content = body.get("content")?.as_array()?;
    let mut text_parts: Vec<String> = Vec::new();
    let mut reasoning_parts: Vec<String> = Vec::new();
    let mut tool_uses: Vec<(String, String, Value)> = Vec::new();

    for block in content {
        let block_type = block.get("type")?.as_str()?;
        match block_type {
            "text" => {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    text_parts.push(t.to_string());
                }
            }
            "thinking" => {
                // 提取 thinking 文本，剥离 signature（只保留思维链内容）
                if let Some(t) = block.get("thinking").and_then(|v| v.as_str()) {
                    reasoning_parts.push(t.to_string());
                }
            }
            "tool_use" => {
                let id = block.get("id")?.as_str()?.to_string();
                let name = block.get("name")?.as_str()?.to_string();
                let input = block.get("input").cloned().unwrap_or_else(|| Value::Object(Default::default()));
                tool_uses.push((id, name, input));
            }
            _ => {} // 跳过未知类型（如 redacted_thinking 等）
        }
    }

    let stop_reason = body.get("stop_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("end_turn")
        .to_string();

    let usage = body.get("usage")?;
    let input_tokens = usage.get("input_tokens")?.as_i64()?;
    let output_tokens = usage.get("output_tokens")?.as_i64()?;
    let cache_read_tokens = usage.get("cache_read_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let text = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };

    let reasoning = if reasoning_parts.is_empty() {
        None
    } else {
        Some(reasoning_parts.join("\n\n"))
    };

    Some(super::converter::NonStreamResponse {
        id,
        model,
        text,
        tool_uses,
        stop_reason,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        reasoning,
    })
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
                "thinking_delta" => {
                    // Anth Claude thinking 流式增量（reasoning_content）
                    delta.get("thinking").and_then(|v| v.as_str()).filter(|t| !t.is_empty()).map(|thinking| ChatStreamEvent::ReasoningDelta {
                        text: thinking.to_string(),
                    })
                }
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
