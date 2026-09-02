use serde_json::Value;

use super::OpenAIRequest;
use crate::types::*;

/// 从 OpenAI 格式请求解析为内部 ChatRequest
pub fn from_openai(body: &serde_json::Value) -> Option<ChatRequest> {
    let openai_req: OpenAIRequest = serde_json::from_value(body.clone()).ok()?;

    let mut messages = Vec::new();
    let mut system = None;
    // tool_call id → 函数名（tool 消息回填 ToolResult.name 用；Gemini 出站靠 name 关联）
    let mut call_names: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for m in &openai_req.messages {
        let role = match m.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => {
                // Extract system message
                if let Some(content) = &m.content {
                    system = Some(SystemContent::Text(
                        content.as_str().unwrap_or("").to_string(),
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
            if let Some(content) = &m.content
                && let Some(text) = content.as_str()
                && !text.is_empty()
            {
                blocks.push(ContentBlock::Text {
                    text: text.to_string(),
                    extra: None,
                });
            }
            for tc in tool_calls {
                let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::Value::Object(Default::default()));
                call_names.insert(tc.id.clone(), tc.function.name.clone());
                blocks.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input,
                    extra: None,
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
            let content = m.content.as_ref().and_then(|v| v.as_str()).unwrap_or("");
            messages.push(Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: tool_call_id.clone(),
                    content: content.to_string(),
                    name: call_names.get(tool_call_id).cloned(),
                    is_error: None,
                    content_blocks: None,
                    extra: None,
                }]),
            });
            continue;
        }

        // Regular text message
        let content = match &m.content {
            Some(Value::String(s)) => MessageContent::Text(s.clone()),
            Some(Value::Array(parts)) => {
                let mut texts: Vec<ContentBlock> = parts
                    .iter()
                    .filter_map(|p| {
                        // 纯字符串元素 或 {type:"text"} object
                        if let Some(s) = p.as_str() {
                            Some(ContentBlock::Text {
                                text: s.to_string(),
                                extra: None,
                            })
                        } else {
                            p.get("text")
                                .and_then(|t| t.as_str())
                                .map(|s| ContentBlock::Text {
                                    text: s.to_string(),
                                    extra: None,
                                })
                        }
                    })
                    .collect();
                // image_url → 中立 image block（data URL 拆 media_type + base64）
                let images: Vec<ContentBlock> = parts.iter()
                    .filter_map(|p| {
                        if p.get("type").and_then(|t| t.as_str()) != Some("image_url") { return None; }
                        let url = p.get("image_url")?.get("url")?.as_str()?;
                        let source = if let Some(rest) = url.strip_prefix("data:") {
                            let (media_type, data) = rest.split_once(";base64,")?;
                            serde_json::json!({ "type": "base64", "media_type": media_type, "data": data })
                        } else {
                            serde_json::json!({ "type": "url", "url": url })
                        };
                        Some(ContentBlock::Unknown(serde_json::json!({ "type": "image", "source": source })))
                    })
                    .collect();
                texts.extend(images);
                let texts = texts;
                if texts.len() == 1 {
                    if let ContentBlock::Text { text, .. } = &texts[0] {
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
                tool_type: None,
                cache_control: None,
                extra: None,
            })
            .collect()
    });

    // reasoning_effort → 思考预算（换算表统一在 `crate::thinking`，票 03；此前这里的 low=2048
    // 与出站表的 low<=4096 不自洽，openai → responses 往返会把 low 抬成 medium）
    let thinking_budget = openai_req
        .reasoning_effort
        .as_deref()
        .and_then(crate::thinking::effort_to_budget);

    let tool_choice = openai_req.tool_choice.and_then(|tc| {
        if tc.is_string() {
            match tc.as_str()? {
                "auto" => Some(ToolChoice::Auto),
                "required" => Some(ToolChoice::Any),
                "none" => Some(ToolChoice::None),
                _ => None,
            }
        } else if tc.is_object() {
            let name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())?;
            Some(ToolChoice::Named {
                name: name.to_string(),
            })
        } else {
            None
        }
    });

    Some(ChatRequest {
        thinking_budget,
        model: openai_req.model,
        messages,
        system,
        // 输出长度归一（票 05）：`max_completion_tokens` 是官方对 `max_tokens` 的继任键，
        // 新版 SDK 与 o 系列模型只发前者；此前它被 serde 静默忽略，anthropic 目标随后
        // 落到 `to_anthropic` 的默认 4096，长输出被截断且无痕。
        // **两键同时出现时取 `max_completion_tokens`**：新键是客户端有意设置的那个，
        // 旧键多为 SDK 为兼容老服务端保留的镜像值；官方本身也把 `max_tokens` 标为 deprecated。
        max_tokens: openai_req.max_completion_tokens.or(openai_req.max_tokens),
        temperature: openai_req.temperature,
        top_p: openai_req.top_p,
        stream: openai_req.stream,
        tools,
        tool_choice,
        // 未建模顶层字段进 `extra`（票 11）：与 anthropic 入站的 `#[serde(flatten)]` 行为对齐。
        // 不收就等于 `stop` / `top_k` / `response_format` 在中立层蒸发，openai → gemini
        // （值落 `generationConfig`，forward 层的顶层白名单管不到）与 openai → completions
        // 两条链路上这些参数必丢。收下不等于出站——出站仍由各 `to_*` 的白名单决定写不写。
        extra: rest_keys(
            body,
            &[
                "model",
                "messages",
                "max_tokens",
                "max_completion_tokens",
                "temperature",
                "top_p",
                "stream",
                "tools",
                "tool_choice",
                "reasoning_effort",
            ],
        ),
        // 档位原值与 thinking_budget 并存：预算是换算后的数字，档位保留客户端原字面量
        thinking_mode: crate::thinking::mode_from_effort(openai_req.reasoning_effort.as_deref()),
    })
}

#[cfg(test)]
#[path = "test_parse.rs"]
mod test_parse;
