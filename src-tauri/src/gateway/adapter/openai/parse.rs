use serde_json::Value;

use super::super::types::*;
use super::OpenAIRequest;

/// 从 OpenAI 格式请求解析为内部 ChatRequest
pub fn from_openai(body: &serde_json::Value) -> Option<ChatRequest> {
    let openai_req: OpenAIRequest = serde_json::from_value(body.clone()).ok()?;

    let mut messages = Vec::new();
    let mut system = None;

    for m in &openai_req.messages {
        let role = match m.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => {
                // Extract system message
                if let Some(content) = &m.content {
                    system = Some(SystemContent::Text(
                        content.as_str().unwrap_or("").to_string()
                    ));
                }
                continue;
            }
            "tool" => Role::Tool,
            _ => Role::User,
        };

        // Check for tool_calls (assistant messages with tool calls)
        if let Some(tool_calls) = &m.tool_calls {
            let mut blocks: Vec<ContentBlock> = Vec::new();
            // Add text content if present
            if let Some(content) = &m.content {
                if let Some(text) = content.as_str() {
                    if !text.is_empty() {
                        blocks.push(ContentBlock::Text { text: text.to_string() });
                    }
                }
            }
            for tc in tool_calls {
                let input: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Object(Default::default()));
                blocks.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input,
                });
            }
            messages.push(Message {
                role,
                content: MessageContent::Blocks(blocks),
            });
            continue;
        }

        // tool_call_id → tool_result
        if let Some(tool_call_id) = &m.tool_call_id {
            let content = m.content.as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("");
            messages.push(Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: tool_call_id.clone(),
                    content: content.to_string(),
                }]),
            });
            continue;
        }

        // Regular text message
        let content = match &m.content {
            Some(Value::String(s)) => MessageContent::Text(s.clone()),
            Some(Value::Array(parts)) => {
                let texts: Vec<ContentBlock> = parts.iter()
                    .filter_map(|p| p.as_str().map(|s| ContentBlock::Text { text: s.to_string() }))
                    .collect();
                if texts.len() == 1 {
                    if let ContentBlock::Text { text } = &texts[0] {
                        MessageContent::Text(text.clone())
                    } else {
                        MessageContent::Blocks(texts)
                    }
                } else {
                    MessageContent::Blocks(texts)
                }
            }
            Some(v) => MessageContent::Text(v.to_string()),
            None => MessageContent::Text(String::new()),
        };
        messages.push(Message { role, content });
    }

    let tools = openai_req.tools.map(|ts| {
        ts.into_iter()
            .map(|t| Tool {
                name: t.function.name,
                description: t.function.description,
                input_schema: t.function.parameters,
            })
            .collect()
    });

    let tool_choice = openai_req.tool_choice.and_then(|tc| {
        if tc.is_string() {
            match tc.as_str()? {
                "auto" => Some(ToolChoice::Auto),
                "required" => Some(ToolChoice::Any),
                "none" => Some(ToolChoice::None),
                _ => None,
            }
        } else if tc.is_object() {
            let name = tc.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())?;
            Some(ToolChoice::Named { name: name.to_string() })
        } else {
            None
        }
    });

    Some(ChatRequest {
        model: openai_req.model,
        messages,
        system,
        max_tokens: openai_req.max_tokens,
        temperature: openai_req.temperature,
        top_p: openai_req.top_p,
        stream: openai_req.stream,
        tools,
        tool_choice,
        extra: None,
    })
}
