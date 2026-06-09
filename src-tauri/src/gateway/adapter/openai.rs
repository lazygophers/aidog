use serde_json::Value;

use super::types::*;

/// OpenAI Chat Completions 请求格式（GLM/Kimi 也兼容）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
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

/// 从内部 ChatRequest 转为 OpenAI 格式
pub fn to_openai(req: &ChatRequest) -> OpenAIRequest {
    let mut messages: Vec<OpenAIMessage> = Vec::new();

    // system 消息放在 messages 数组开头
    if let Some(system) = &req.system {
        messages.push(OpenAIMessage {
            role: "system".to_string(),
            content: Some(Value::String(system.clone())),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for m in &req.messages {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        };

        match &m.content {
            MessageContent::Text(s) => {
                messages.push(OpenAIMessage {
                    role: role.to_string(),
                    content: Some(Value::String(s.clone())),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            MessageContent::Blocks(blocks) => {
                // 提取文本块
                let text_parts: Vec<Value> = blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(Value::String(text.clone())),
                        _ => None,
                    })
                    .collect();

                let text_content = if text_parts.len() == 1 {
                    text_parts.into_iter().next()
                } else if text_parts.is_empty() {
                    None
                } else {
                    Some(Value::Array(text_parts))
                };

                // 处理 tool_use 块 → assistant message 的 tool_calls
                let tool_calls: Vec<OpenAIToolCall> = blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::ToolUse { id, name, input } => Some(OpenAIToolCall {
                            id: id.clone(),
                            r#type: "function".to_string(),
                            function: OpenAIFunctionCall {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).unwrap_or_default(),
                            },
                        }),
                        _ => None,
                    })
                    .collect();

                if !tool_calls.is_empty() {
                    messages.push(OpenAIMessage {
                        role: "assistant".to_string(),
                        content: text_content,
                        tool_calls: Some(tool_calls),
                        tool_call_id: None,
                    });
                    continue;
                }

                // 处理 tool_result 块 → tool message
                let mut pushed_result = false;
                for b in blocks {
                    if let ContentBlock::ToolResult { tool_use_id, content } = b {
                        messages.push(OpenAIMessage {
                            role: "tool".to_string(),
                            content: Some(Value::String(content.clone())),
                            tool_calls: None,
                            tool_call_id: Some(tool_use_id.clone()),
                        });
                        pushed_result = true;
                    }
                }
                if pushed_result {
                    continue;
                }

                // 普通文本块
                messages.push(OpenAIMessage {
                    role: role.to_string(),
                    content: text_content,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }
    }

    let tools = req.tools.as_ref().map(|ts| {
        ts.iter()
            .map(|t| OpenAITool {
                r#type: "function".to_string(),
                function: OpenAIFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect()
    });

    let tool_choice = req.tool_choice.as_ref().and_then(|tc| match tc {
        ToolChoice::Auto => Some(serde_json::json!("auto")),
        ToolChoice::Any => Some(serde_json::json!("required")),
        ToolChoice::None => Some(serde_json::json!("none")),
        ToolChoice::Named { name } => Some(serde_json::json!({
            "type": "function",
            "function": { "name": name }
        })),
    });

    OpenAIRequest {
        model: req.model.clone(),
        messages,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        stream: req.stream,
        tools,
        tool_choice,
    }
}

/// 解析 OpenAI SSE 格式的流式事件
pub fn parse_openai_sse(data: &Value) -> Option<ChatStreamEvent> {
    let choices = data.get("choices")?.as_array()?;
    let choice = choices.first()?;
    let index = choice.get("index")?.as_u64()? as u32;
    let delta = choice.get("delta")?;

    // 检查是否有 tool_calls
    if let Some(tool_calls_val) = delta.get("tool_calls") {
        if let Some(tool_calls) = tool_calls_val.as_array() {
            if let Some(tc) = tool_calls.first() {
                let id = tc.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                let func = tc.get("function");
                let name = func.and_then(|f| f.get("name")).and_then(|v| v.as_str()).map(|s| s.to_string());
                let input = func
                    .and_then(|f| f.get("arguments"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                return Some(ChatStreamEvent::ToolDelta {
                    index,
                    id,
                    name,
                    input,
                });
            }
        }
    }

    // 文本 delta
    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
        if content.is_empty() {
            return None;
        }
        return Some(ChatStreamEvent::Delta {
            text: content.to_string(),
        });
    }

    // 结束
    if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
        if reason == "stop" || reason == "tool_calls" || reason == "length" {
            return Some(ChatStreamEvent::Stop {
                finish_reason: Some(reason.to_string()),
            });
        }
    }

    None
}
