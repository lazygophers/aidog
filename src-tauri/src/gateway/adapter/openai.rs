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
                        ContentBlock::Text { text } => Some(text.clone()),
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

                // 处理 tool_result 块 → tool message(每个 tool_result 单独成 tool message)
                let has_tool_result = blocks.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                if has_tool_result {
                    for b in blocks {
                        if let ContentBlock::ToolResult { tool_use_id, content } = b {
                            messages.push(OpenAIMessage {
                                role: "tool".to_string(),
                                content: Some(Value::String(content.clone())),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn user_blocks(blocks: Vec<ContentBlock>) -> Message {
        Message { role: Role::User, content: MessageContent::Blocks(blocks) }
    }
    fn assistant_blocks(blocks: Vec<ContentBlock>) -> Message {
        Message { role: Role::Assistant, content: MessageContent::Blocks(blocks) }
    }
    fn base_req(messages: Vec<Message>) -> ChatRequest {
        ChatRequest {
            model: "kimi-k2".into(),
            messages,
            system: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: None,
            tools: None,
            tool_choice: None,
            extra: None,
        }
    }

    // 缺陷 C: assistant 消息仅含 thinking(Unknown) 块,不得产出空 {"role":"assistant"} message
    #[test]
    fn thinking_only_assistant_message_is_skipped() {
        let thinking = ContentBlock::Unknown(serde_json::json!({
            "type": "thinking", "thinking": "let me think", "signature": "sig"
        }));
        let req = base_req(vec![
            Message { role: Role::User, content: MessageContent::Text("hi".into()) },
            assistant_blocks(vec![thinking]),
        ]);
        let out = to_openai(&req);
        // 只应保留 user 消息,thinking-only assistant 被跳过(否则 Kimi 400)
        assert_eq!(out.messages.len(), 1, "thinking-only assistant 不应产出空 message");
        assert_eq!(out.messages[0].role, "user");
    }

    // 缺陷 C 变体: thinking + tool_use 混排 → 保留 tool_calls,丢 thinking,非空
    #[test]
    fn thinking_plus_tool_use_keeps_tool_calls() {
        let req = base_req(vec![assistant_blocks(vec![
            ContentBlock::Unknown(serde_json::json!({"type":"thinking","thinking":"t"})),
            ContentBlock::ToolUse {
                id: "call_1".into(),
                name: "read_file".into(),
                input: serde_json::json!({"path":"/a"}),
            },
        ])]);
        let out = to_openai(&req);
        assert_eq!(out.messages.len(), 1);
        let m = &out.messages[0];
        assert_eq!(m.role, "assistant");
        let tcs = m.tool_calls.as_ref().expect("tool_calls present");
        assert_eq!(tcs.len(), 1);
        assert!(m.content.is_none(), "无 text 时 content 应省略而非空字符串");
    }

    // 缺陷 A: user 消息含 tool_result + text 混排,文本不得丢失
    #[test]
    fn tool_result_plus_text_preserves_text() {
        let req = base_req(vec![user_blocks(vec![
            ContentBlock::ToolResult { tool_use_id: "call_1".into(), content: "file content".into() },
            ContentBlock::Text { text: "now do X".into() },
        ])]);
        let out = to_openai(&req);
        // tool message + 残余 user 文本 message
        assert_eq!(out.messages.len(), 2);
        assert_eq!(out.messages[0].role, "tool");
        assert_eq!(out.messages[0].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(out.messages[1].role, "user");
        assert_eq!(out.messages[1].content, Some(Value::String("now do X".into())));
    }

    // 缺陷 A 变体: 多个 tool_result(并行工具)各自成 tool message,tool_call_id 对应
    #[test]
    fn multiple_tool_results_each_become_tool_message() {
        let req = base_req(vec![user_blocks(vec![
            ContentBlock::ToolResult { tool_use_id: "c1".into(), content: "r1".into() },
            ContentBlock::ToolResult { tool_use_id: "c2".into(), content: "r2".into() },
        ])]);
        let out = to_openai(&req);
        assert_eq!(out.messages.len(), 2);
        assert_eq!(out.messages[0].tool_call_id.as_deref(), Some("c1"));
        assert_eq!(out.messages[1].tool_call_id.as_deref(), Some("c2"));
    }

    // 多段 text 块拼成单一字符串(非 array),避免 Kimi 拒多模态结构
    #[test]
    fn multiple_text_blocks_join_into_string() {
        let req = base_req(vec![user_blocks(vec![
            ContentBlock::Text { text: "part1".into() },
            ContentBlock::Text { text: "part2".into() },
        ])]);
        let out = to_openai(&req);
        assert_eq!(out.messages.len(), 1);
        assert_eq!(out.messages[0].content, Some(Value::String("part1\npart2".into())));
    }
}

/// 将 ChatStreamEvent 转为 OpenAI SSE 格式
pub fn to_openai_sse(event: &ChatStreamEvent, model: &str) -> Option<String> {
    match event {
        ChatStreamEvent::Start { id, .. } => Some(format!(
            "data: {}\n\n",
            serde_json::json!({
                "id": id,
                "object": "chat.completion.chunk",
                "model": model,
                "choices": [{"index": 0, "delta": {"role": "assistant", "content": ""}, "finish_reason": null}]
            })
        )),
        ChatStreamEvent::Delta { text } => Some(format!(
            "data: {}\n\n",
            serde_json::json!({
                "id": "",
                "object": "chat.completion.chunk",
                "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]
            })
        )),
        ChatStreamEvent::ToolDelta { index, id, name, input } => {
            let mut parts = Vec::new();
            if let (Some(id), Some(name)) = (id, name) {
                parts.push(format!(
                    "data: {}\n\n",
                    serde_json::json!({
                        "id": "",
                        "object": "chat.completion.chunk",
                        "choices": [{"index": 0, "delta": {"tool_calls": [{"index": index, "id": id, "type": "function", "function": {"name": name, "arguments": ""}}]}, "finish_reason": null}]
                    })
                ));
            }
            if let Some(input) = input {
                parts.push(format!(
                    "data: {}\n\n",
                    serde_json::json!({
                        "id": "",
                        "object": "chat.completion.chunk",
                        "choices": [{"index": 0, "delta": {"tool_calls": [{"index": index, "function": {"arguments": input}}]}, "finish_reason": null}]
                    })
                ));
            }
            if parts.is_empty() { None } else { Some(parts.join("")) }
        },
        ChatStreamEvent::Stop { finish_reason } => {
            let reason = match finish_reason.as_deref().unwrap_or("end_turn") {
                "end_turn" => "stop",
                other => other,
            };
            Some(format!(
                "data: {}\n\ndata: [DONE]\n\n",
                serde_json::json!({
                    "id": "",
                    "object": "chat.completion.chunk",
                    "choices": [{"index": 0, "delta": {}, "finish_reason": reason}]
                })
            ))
        },
        ChatStreamEvent::Usage { .. } => None,
    }
}
