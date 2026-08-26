use serde_json::Value;

use crate::types::*;
use super::{OpenAIFunction, OpenAIFunctionCall, OpenAIMessage, OpenAIRequest, OpenAITool, OpenAIToolCall};

/// 从内部 ChatRequest 转为 OpenAI 格式
pub fn to_openai(req: &ChatRequest) -> OpenAIRequest {
    let mut messages: Vec<OpenAIMessage> = Vec::new();

    // system 消息放在 messages 数组开头
    if let Some(system) = &req.system {
        let content = match system {
            SystemContent::Text(t) => Value::String(t.clone()),
            SystemContent::Blocks(blocks) => {
                // Extract text from blocks for OpenAI compatibility
                let texts: Vec<&str> = blocks.iter()
                    .filter_map(|b| b.get("text").and_then(|v| v.as_str()))
                    .collect();
                Value::String(texts.join("\n"))
            }
        };
        messages.push(OpenAIMessage {
            role: "system".to_string(),
            content: Some(content),
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
                // 提取文本块(Text;Unknown/thinking/image 等跳过,避免泄漏 Anthropic 专属结构)
                let text_parts: Vec<String> = blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    })
                    .collect();
                // OpenAI/Kimi content 用单一字符串(拼接多段),避免 array 多模态结构被 Kimi 拒
                let text_content = if text_parts.is_empty() {
                    None
                } else {
                    Some(Value::String(text_parts.join("\n")))
                };

                // 处理 tool_use 块 → assistant message 的 tool_calls
                let tool_calls: Vec<OpenAIToolCall> = blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::ToolUse { id, name, input, .. } => Some(OpenAIToolCall {
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

                // 处理 tool_result 块 → tool message(每个 tool_result 单独成 tool message)
                let has_tool_result = blocks.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                if has_tool_result {
                    for b in blocks {
                        if let ContentBlock::ToolResult { tool_use_id, content, is_error, .. } = b {
                            messages.push(OpenAIMessage {
                                role: "tool".to_string(),
                                // OpenAI tool message 只吃文本：失败标注与非文本 block 的占位都在文本里
                                content: Some(Value::String(mark_tool_error(content, *is_error))),
                                tool_calls: None,
                                tool_call_id: Some(tool_use_id.clone()),
                            });
                        }
                    }
                    // tool_result 与 text 混排时,残余文本另起一条 user message,避免静默丢内容
                    if let Some(tc) = text_content {
                        messages.push(OpenAIMessage {
                            role: "user".to_string(),
                            content: Some(tc),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                    continue;
                }

                // image block → OpenAI 多模态数组 content（text 段 + image_url 段）
                let image_parts: Vec<Value> = blocks.iter()
                    .filter_map(|b| match b {
                        ContentBlock::Unknown(v) if v.get("type").and_then(|t| t.as_str()) == Some("image") => {
                            let src = v.get("source")?;
                            let url = match src.get("type").and_then(|t| t.as_str()) {
                                // base64 source 重组 data URL
                                Some("base64") => format!(
                                    "data:{};base64,{}",
                                    src.get("media_type").and_then(|m| m.as_str()).unwrap_or("application/octet-stream"),
                                    src.get("data").and_then(|d| d.as_str()).unwrap_or(""),
                                ),
                                Some("url") => src.get("url").and_then(|u| u.as_str())?.to_string(),
                                _ => return None,
                            };
                            Some(serde_json::json!({ "type": "image_url", "image_url": { "url": url } }))
                        }
                        _ => None,
                    })
                    .collect();
                if !image_parts.is_empty() {
                    let mut content_arr: Vec<Value> = Vec::new();
                    if let Some(tc) = text_content.clone()
                        && let Value::String(t) = &tc && !t.is_empty() {
                            content_arr.push(serde_json::json!({ "type": "text", "text": t }));
                        }
                    content_arr.extend(image_parts);
                    messages.push(OpenAIMessage {
                        role: role.to_string(),
                        content: Some(Value::Array(content_arr)),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                    continue;
                }

                // 普通文本块。若全是 Unknown(thinking 等)致 text_content 为空,
                // 跳过该消息,避免产出既无 content 又无 tool_calls 的空 message
                // (OpenAI/Kimi 强校验拒绝空消息 → 400 "Invalid request Error")。
                if let Some(tc) = text_content {
                    messages.push(OpenAIMessage {
                        role: role.to_string(),
                        content: Some(tc),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
            }
        }
    }

    // 服务端工具在 OpenAI 侧无执行方，整条不下发；全被丢弃时不写 tools 键
    let tools = req.tools.as_ref().and_then(|ts| {
        let kept: Vec<OpenAITool> = client_tools(ts, "openai")
            .into_iter()
            .map(|t| OpenAITool {
                r#type: "function".to_string(),
                function: OpenAIFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect();
        (!kept.is_empty()).then_some(kept)
    });

    let tool_choice = req.tool_choice.as_ref().map(|tc| match tc {
        ToolChoice::Auto => serde_json::json!("auto"),
        ToolChoice::Any => serde_json::json!("required"),
        ToolChoice::None => serde_json::json!("none"),
        ToolChoice::Named { name } => serde_json::json!({
            "type": "function",
            "function": { "name": name }
        }),
    });

    OpenAIRequest {
        model: req.model.clone(),
        messages,
        max_tokens: req.max_tokens,
        // 出站统一写 `max_tokens`（第三方 OpenAI 兼容端点只认它）；官方 api.openai.com
        // 需要的 `max_completion_tokens` 由 forward 层按 host 改键（票 05）。
        max_completion_tokens: None,
        temperature: req.temperature,
        top_p: req.top_p,
        stream: req.stream,
        tools,
        tool_choice,
        // budget → effort 阈值映射（与 from_openai 反向映射共用档位）
        reasoning_effort: req.thinking_budget.map(|b| match b {
            0..=4096 => "low",
            4097..=8192 => "medium",
            _ => "high",
        }.to_string()),
    }
}

#[cfg(test)]
#[path = "test_request.rs"]
mod test_request;
