use serde_json::Value;

use crate::types::*;

/// OpenAI Responses API (`/v1/responses`) 请求格式
/// 使用 `input` 而非 `messages`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResponsesInput {
    pub role: String,
    pub content: String,
}

/// 转为 Responses API 格式
pub fn to_responses(req: &ChatRequest) -> ResponsesRequest {
    let input: Vec<Value> = req.messages.iter().flat_map(|m| {
        let role_str = match m.role { Role::User => "user", Role::Assistant => "assistant", Role::System => "developer", Role::Tool => "tool" };
        // Responses 的 part 类型按角色分：助手轮是模型产出（output_text），其余轮是输入（input_text）
        let part_type = if matches!(m.role, Role::Assistant) { "output_text" } else { "input_text" };
        m.content.blocks().into_iter().filter_map(move |b| match b {
            ContentBlock::Text { text, .. } => Some(serde_json::json!({"type":"message","role":role_str,"content":[{"type":part_type,"text":text}]})),
            ContentBlock::ToolUse { id, name, input, .. } => Some(serde_json::json!({"type":"function_call","call_id":id,"name":name,"arguments":input.to_string()})),
            // function_call_output.output 只吃文本：失败标注与非文本 block 的占位都在文本里
            ContentBlock::ToolResult { tool_use_id, content, is_error, .. } => Some(serde_json::json!({"type":"function_call_output","call_id":tool_use_id,"output":mark_tool_error(&content, is_error)})),
            ContentBlock::Unknown(_) => None,
        })
    }).collect();
    let instructions = req.system.as_ref().map(|system| match system {
        SystemContent::Text(text) => text.clone(),
        SystemContent::Blocks(blocks) => blocks.iter().filter_map(|b| b.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n"),
    });
    // 服务端工具在 Responses 侧无执行方，整条不下发；全被丢弃时不写 tools 键
    let tools = req.tools.as_ref().and_then(|tools| {
        let kept: Vec<Value> = client_tools(tools, "openai_responses").into_iter().map(|tool| serde_json::json!({"type":"function","name":tool.name,"description":tool.description,"parameters":tool.input_schema})).collect();
        (!kept.is_empty()).then_some(Value::Array(kept))
    });
    // tool_choice：Responses 侧是扁平形态 {type:"function",name}，与 from_responses 的解析互逆
    let tool_choice = req.tool_choice.as_ref().map(|tc| match tc {
        ToolChoice::Auto => serde_json::json!("auto"),
        ToolChoice::Any => serde_json::json!("required"),
        ToolChoice::None => serde_json::json!("none"),
        ToolChoice::Named { name } => serde_json::json!({"type":"function","name":name}),
    });
    ResponsesRequest { model: req.model.clone(), input, instructions, max_output_tokens: req.max_tokens, temperature: req.temperature, top_p: req.top_p, stream: req.stream, tools, tool_choice }
}

/// 解析 OpenAI Responses API 非流式响应为归一 NonStreamResponse
pub fn parse_responses_response(body: &Value, fallback_model: &str) -> Option<crate::converter::NonStreamResponse> {
    let id = body.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let model = body.get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_model)
        .to_string();

    let output = body.get("output")?.as_array()?;
    let mut text_parts: Vec<String> = Vec::new();
    let mut reasoning_parts: Vec<String> = Vec::new();
    let mut tool_uses: Vec<(String, String, Value)> = Vec::new();

    for item in output {
        // 提取 reasoning：summary 数组或 reasoning 类型的 content
        if let Some(summary) = item.get("summary").and_then(|v| v.as_array()) {
            for s in summary {
                if let Some(text) = s.get("text").and_then(|v| v.as_str()) {
                    reasoning_parts.push(text.to_string());
                }
            }
        } else if let Some(content) = item.get("content").and_then(|v| v.as_array()) {
            for c in content {
                let ctype = c.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if ctype == "reasoning" {
                    if let Some(text) = c.get("text").and_then(|v| v.as_str()) {
                        reasoning_parts.push(text.to_string());
                    }
                } else if ctype == "text"
                    && let Some(text) = c.get("text").and_then(|v| v.as_str()) {
                        text_parts.push(text.to_string());
                    }
            }
        }

        // 提取 function_call（tool_use）：兼容两种形态
        //   ① 嵌套：item: {function_call: {id, name, arguments}}
        //   ② 扁平：item: {type:"function_call", id, name, arguments}
        if let Some(fc) = item.get("function_call") {
            let id = fc.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = fc.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = fc.get("arguments").cloned().unwrap_or_else(|| Value::Object(Default::default()));
            if !id.is_empty() || !name.is_empty() {
                tool_uses.push((id, name, args));
            }
        } else if item.get("type").and_then(|v| v.as_str()) == Some("function_call") {
            let id = item.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = item.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = item.get("arguments").cloned().unwrap_or_else(|| Value::Object(Default::default()));
            if !id.is_empty() || !name.is_empty() {
                tool_uses.push((id, name, args));
            }
        }

        // 提取普通文本
        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
            text_parts.push(text.to_string());
        }
    }

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

    // Responses API 的 status 映射到 stop_reason
    let status = body.get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("completed");
    let stop_reason = match status {
        "completed" => "end_turn",
        "failed" | "incomplete" => "end_turn",
        _ => "end_turn",
    }.to_string();

    // usage 信息
    let usage = body.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("input_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|u| u.get("output_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_read_tokens = 0; // Responses API 暂不支持缓存读取

    Some(crate::converter::NonStreamResponse {
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

/// 渲染归一响应为 OpenAI Responses API 非流式响应体。
///
/// 映射规则：
/// - output[]: text item（type=text） + reasoning summary item（type=summary） + function_call item
/// - status: completed
/// - usage: prompt_tokens/completion_tokens
pub fn render_responses_response(r: &crate::converter::NonStreamResponse) -> Option<Value> {
    let mut output = Vec::new();

    // 添加文本 item
    if let Some(text) = &r.text
        && !text.is_empty() {
            output.push(serde_json::json!({
                "type": "text",
                "text": text,
            }));
        }

    // 添加 reasoning summary item
    if let Some(reasoning) = &r.reasoning
        && !reasoning.is_empty() {
            output.push(serde_json::json!({
                "type": "summary",
                "text": reasoning,
            }));
        }

    // 添加 function_call item（每个 tool_use 一个 item）
    for (id, name, input) in &r.tool_uses {
        output.push(serde_json::json!({
            "type": "function_call",
            "id": id,
            "name": name,
            "arguments": input,
        }));
    }

    // 兜底：既无 text 也无 tool_use（异常上游）→ 空 text item，保证 output 非空
    if output.is_empty() {
        output.push(serde_json::json!({
            "type": "text",
            "text": "",
        }));
    }

    Some(serde_json::json!({
        "id": r.id,
        "model": r.model,
        "status": "completed",
        "output": output,
        "usage": {
            "prompt_tokens": r.input_tokens,
            "completion_tokens": r.output_tokens,
            "total_tokens": r.input_tokens + r.output_tokens,
        }
    }))
}

/// 从 Responses API 请求解析为内部 ChatRequest。
///
/// 兼容 Codex / OpenAI Responses 的多种 `input` 形态：
/// - `input` 为字符串（如 `{"input":"hi"}`）→ 单条 user 文本消息
/// - `input` 为数组，每个 item 的 `content`：
///   - 字符串 → 直接文本
///   - 数组（typed parts，如 `input_text` / `output_text` / `text`）→ 拼接各 part 的 `text`
/// - `instructions` → system（system prompt）
/// - `tools` / `tool_choice` / `reasoning.effort` 与 tool 调用回合（function_call / function_call_output）均转换。
pub fn from_responses(body: &Value) -> Option<ChatRequest> {
    let model = body.get("model")?.as_str()?.to_string();

    let mut messages = Vec::new();
    match body.get("input") {
        // 字符串形态：单条 user 消息
        Some(Value::String(s)) => {
            messages.push(Message {
                role: Role::User,
                content: MessageContent::Text(s.clone()),
            });
        }
        // 数组形态：逐 item 解析（typed parts：message / function_call / function_call_output）
        Some(Value::Array(items)) => {
            for item in items {
                let item_type = item.get("type").and_then(Value::as_str);
                match item_type {
                    // 工具调用回合：assistant 发起（call_id/name/arguments）
                    Some("function_call") => {
                        let input_json = item
                            .get("arguments")
                            .and_then(Value::as_str)
                            .and_then(|s| serde_json::from_str::<Value>(s).ok())
                            .unwrap_or(Value::Null);
                        messages.push(Message {
                            role: Role::Assistant,
                            content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                                id: item.get("call_id").and_then(Value::as_str).unwrap_or_default().to_string(),
                                name: item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                                input: input_json, extra: None
                            }]),
                        });
                    }
                    // 工具结果回传（call_id/output）
                    Some("function_call_output") => {
                        messages.push(Message {
                            role: Role::Tool,
                            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                                tool_use_id: item.get("call_id").and_then(Value::as_str).unwrap_or_default().to_string(),
                                content: item
                                    .get("output")
                                    .map(|o| match o {
                                        Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    })
                                    .unwrap_or_default(),
                                name: None, is_error: None, content_blocks: None, extra: None
                            }]),
                        });
                    }
                    _ => {
                        let role_str = item
                            .get("role")
                            .and_then(|v| v.as_str())
                            .unwrap_or("user")
                            .to_lowercase();
                        let role = match role_str.as_str() {
                            "assistant" => Role::Assistant,
                            "system" | "developer" => Role::System,
                            "tool" => Role::Tool,
                            _ => Role::User,
                        };
                        let content = extract_content_text(item.get("content"));
                        messages.push(Message {
                            role,
                            content: MessageContent::Text(content),
                        });
                    }
                }
            }
        }
        _ => return None,
    }

    // instructions → system prompt（Codex 用 instructions 传系统提示）
    let system = body
        .get("instructions")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| SystemContent::Text(s.to_string()));

    // tools: Responses 扁平格式 {type:"function", name, description, parameters} → 内部 Tool
    let tools = body.get("tools").and_then(Value::as_array).map(|ts| {
        ts.iter()
            .filter(|t| t.get("type").and_then(Value::as_str) == Some("function"))
            .map(|t| crate::types::Tool {
                name: t.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                description: t.get("description").and_then(Value::as_str).map(str::to_string),
                input_schema: t.get("parameters").cloned().unwrap_or_else(|| serde_json::json!({})),
                tool_type: None,
                cache_control: None,
                extra: None,
            })
            .collect::<Vec<_>>()
    });
    // tool_choice: "auto"/"none"/"required"/{type:"function",name}
    let tool_choice = body.get("tool_choice").and_then(|v| match v {
        Value::String(s) => match s.as_str() {
            "auto" => Some(ToolChoice::Auto),
            "none" => Some(ToolChoice::None),
            "required" => Some(ToolChoice::Any),
            _ => None,
        },
        Value::Object(o) => o.get("name").and_then(Value::as_str).map(|name| ToolChoice::Named { name: name.to_string() }),
        _ => None,
    });
    // reasoning.effort → thinking_budget（与 to_openai budget→effort 档位映射互逆）
    let thinking_budget = body
        .get("reasoning")
        .and_then(|r| r.get("effort"))
        .and_then(Value::as_str)
        .map(|effort| match effort {
            "low" => 4096,
            "medium" => 8192,
            _ => 10000,
        });

    Some(ChatRequest {
        thinking_budget,
        model,
        messages,
        system,
        max_tokens: body.get("max_output_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
        temperature: body.get("temperature").and_then(|v| v.as_f64()).map(|v| v as f32),
        top_p: body.get("top_p").and_then(|v| v.as_f64()).map(|v| v as f32),
        stream: body.get("stream").and_then(|v| v.as_bool()),
        tools,
        tool_choice,
        extra: None,
        // 档位原值与 thinking_budget 并存：预算是换算后的数字，档位保留客户端原字面量
        thinking_mode: ThinkingMode::from_parts(
            None,
            body.get("reasoning").and_then(|r| r.get("effort")).and_then(Value::as_str).map(str::to_string),
        ),
    })
}

/// 提取一个 Responses input item 的 `content` 文本：
/// 支持字符串、或 typed parts 数组（`input_text` / `output_text` / `text` 的 `text` 字段）。
fn extract_content_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| {
                // 优先 part.text；兼容 {"type":"input_text","text":"..."}
                p.get("text").and_then(|v| v.as_str()).map(|s| s.to_string())
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// 解析 OpenAI Responses SSE 事件（`response.*` 事件流）为统一 ChatStreamEvent。
/// 单事件帧：一个 data JSON 只产出一个事件（与 parse_openai_sse 同粒度）。
pub fn parse_responses_sse(data: &Value) -> Option<ChatStreamEvent> {
    let event_type = data.get("type")?.as_str()?;
    match event_type {
        "response.output_text.delta" => {
            let delta = data.get("delta")?.as_str()?;
            if delta.is_empty() {
                None
            } else {
                Some(ChatStreamEvent::Delta { text: delta.to_string() })
            }
        }
        "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
            let delta = data.get("delta")?.as_str()?;
            if delta.is_empty() {
                None
            } else {
                Some(ChatStreamEvent::ReasoningDelta { text: delta.to_string() })
            }
        }
        "response.output_item.added" => {
            let item = data.get("item")?;
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                return None;
            }
            Some(ChatStreamEvent::ToolDelta {
                index: data.get("output_index").and_then(Value::as_u64)? as u32,
                id: item.get("id").and_then(Value::as_str).map(str::to_string),
                name: item.get("name").and_then(Value::as_str).map(str::to_string),
                input: None,
            })
        }
        "response.function_arguments.delta" => Some(ChatStreamEvent::ToolDelta {
            index: data.get("output_index").and_then(Value::as_u64)? as u32,
            id: None,
            name: None,
            input: data.get("delta").and_then(Value::as_str).map(str::to_string),
        }),
        "response.completed" => Some(ChatStreamEvent::Stop { finish_reason: Some("stop".to_string()) }),
        "response.incomplete" => Some(ChatStreamEvent::Stop { finish_reason: Some("length".to_string()) }),
        "response.failed" => Some(ChatStreamEvent::Stop { finish_reason: Some("stop".to_string()) }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_responses_sse_text_delta() {
        let ev = parse_responses_sse(&json!({"type":"response.output_text.delta","delta":"你好"}));
        assert!(matches!(ev, Some(ChatStreamEvent::Delta { ref text }) if text == "你好"));
    }

    #[test]
    fn parse_responses_sse_function_call_flow() {
        let added = parse_responses_sse(&json!({
            "type":"response.output_item.added","output_index":1,
            "item":{"type":"function_call","id":"call_1","name":"get_weather"}
        }));
        match added {
            Some(ChatStreamEvent::ToolDelta { index, id, name, input }) => {
                assert_eq!(index, 1);
                assert_eq!(id.as_deref(), Some("call_1"));
                assert_eq!(name.as_deref(), Some("get_weather"));
                assert!(input.is_none());
            }
            other => panic!("expected ToolDelta, got {:?}", other.map(|_| ())),
        }
        let args = parse_responses_sse(&json!({
            "type":"response.function_arguments.delta","output_index":1,"delta":"{\"city\":"
        }));
        assert!(matches!(args, Some(ChatStreamEvent::ToolDelta { input: Some(ref s), .. }) if s == "{\"city\":"));
    }

    #[test]
    fn parse_responses_sse_terminal_events() {
        let done = parse_responses_sse(&json!({"type":"response.completed","response":{}}));
        assert!(matches!(done, Some(ChatStreamEvent::Stop { finish_reason: Some(ref r) }) if r == "stop"));
        let truncated = parse_responses_sse(&json!({"type":"response.incomplete","response":{}}));
        assert!(matches!(truncated, Some(ChatStreamEvent::Stop { finish_reason: Some(ref r) }) if r == "length"));
        let ignored = parse_responses_sse(&json!({"type":"response.created","response":{}}));
        assert!(ignored.is_none());
    }

    #[test]
    fn from_responses_tools_tool_choice_reasoning() {
        let body = json!({
            "model": "gpt-5",
            "instructions": "sys",
            "input": "hi",
            "tools": [
                {"type":"function","name":"f","description":"do f","parameters":{"type":"object"}},
                {"type":"web_search"} // 非 function 工具忽略
            ],
            "tool_choice": {"type":"function","name":"f"},
            "reasoning": {"effort":"medium"}
        });
        let req = from_responses(&body).expect("parse");
        assert_eq!(req.tools.as_ref().unwrap().len(), 1);
        assert_eq!(req.tools.as_ref().unwrap()[0].name, "f");
        assert!(matches!(req.tool_choice, Some(ToolChoice::Named { ref name }) if name == "f"));
        assert_eq!(req.thinking_budget, Some(8192));
    }

    #[test]
    fn from_responses_typed_tool_turns_roundtrip() {
        let body = json!({
            "model": "gpt-5",
            "input": [
                {"type":"message","role":"user","content":[{"type":"input_text","text":"weather?"}]},
                {"type":"function_call","call_id":"call_1","name":"get_weather","arguments":"{\"city\":\"苏黎世\"}"},
                {"type":"function_call_output","call_id":"call_1","output":"15C sunny"}
            ]
        });
        let req = from_responses(&body).expect("parse");
        // 出站方向应还原出同样的 typed items（上下文跨协议接续的关键路径）
        let out = to_responses(&req);
        let types: Vec<&str> = out.input.iter().filter_map(|i| i.get("type").and_then(Value::as_str)).collect();
        assert_eq!(types, vec!["message", "function_call", "function_call_output"]);
        let fc = &out.input[1];
        assert_eq!(fc["call_id"], "call_1");
        assert_eq!(fc["name"], "get_weather");
        assert_eq!(fc["arguments"], "{\"city\":\"苏黎世\"}");
        let fco = &out.input[2];
        assert_eq!(fco["call_id"], "call_1");
        assert_eq!(fco["output"], "15C sunny");
    }

    #[test]
    fn from_responses_string_input() {
        // Codex 最简请求体：input 为字符串
        let body = json!({ "model": "gpt-5", "input": "say hi" });
        let req = from_responses(&body).expect("string input should parse");
        assert_eq!(req.model, "gpt-5");
        assert_eq!(req.messages.len(), 1);
        assert!(matches!(req.messages[0].role, Role::User));
        match &req.messages[0].content {
            MessageContent::Text(t) => assert_eq!(t, "say hi"),
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn from_responses_array_typed_parts() {
        // Codex 实际请求：input 为数组，content 为 typed parts
        let body = json!({
            "model": "gpt-5",
            "instructions": "you are helpful",
            "input": [
                { "role": "user", "content": [
                    { "type": "input_text", "text": "hello " },
                    { "type": "input_text", "text": "world" }
                ]},
                { "role": "assistant", "content": "hi there" }
            ],
            "max_output_tokens": 256,
            "stream": true
        });
        let req = from_responses(&body).expect("array input should parse");
        assert_eq!(req.messages.len(), 2);
        match &req.messages[0].content {
            MessageContent::Text(t) => assert_eq!(t, "hello world"),
            _ => panic!("expected joined text"),
        }
        assert!(matches!(req.messages[1].role, Role::Assistant));
        assert_eq!(req.max_tokens, Some(256));
        assert_eq!(req.stream, Some(true));
        match req.system {
            Some(SystemContent::Text(s)) => assert_eq!(s, "you are helpful"),
            _ => panic!("instructions should map to system"),
        }
    }

    #[test]
    fn from_responses_missing_model_or_input() {
        assert!(from_responses(&json!({ "input": "hi" })).is_none());
        assert!(from_responses(&json!({ "model": "x" })).is_none());
    }

    #[test]
    fn from_responses_system_developer_role() {
        let body = json!({
            "model": "gpt-5",
            "input": [
                { "role": "developer", "content": "system instruction" },
                { "role": "user", "content": "hello" }
            ]
        });
        let req = from_responses(&body).unwrap();
        assert_eq!(req.messages.len(), 2);
        assert!(matches!(req.messages[0].role, Role::System));
        assert!(matches!(req.messages[1].role, Role::User));
    }

    #[test]
    fn from_responses_tool_role() {
        let body = json!({
            "model": "gpt-5",
            "input": [
                { "role": "tool", "content": "tool result" }
            ]
        });
        let req = from_responses(&body).unwrap();
        assert!(matches!(req.messages[0].role, Role::Tool));
    }

    #[test]
    fn from_responses_temperature_top_p() {
        let body = json!({
            "model": "gpt-5",
            "input": "hi",
            "temperature": 0.7,
            "top_p": 0.9
        });
        let req = from_responses(&body).unwrap();
        assert!((req.temperature.unwrap() - 0.7f32).abs() < 0.01);
        assert!((req.top_p.unwrap() - 0.9f32).abs() < 0.01);
    }

    #[test]
    fn from_responses_array_content_with_none_content() {
        // item without content → empty string
        let body = json!({
            "model": "gpt-5",
            "input": [
                { "role": "user" }
            ]
        });
        let req = from_responses(&body).unwrap();
        assert!(matches!(&req.messages[0].content, MessageContent::Text(t) if t.is_empty()));
    }

    #[test]
    fn to_responses_basic() {
        use crate::types::{MessageContent, Role};
        let req = ChatRequest {
            thinking_budget: None,
            model: "gpt-5".into(),
            messages: vec![
                crate::types::Message {
                    role: Role::User,
                    content: MessageContent::Text("hello".into()),
                }
            ],
            system: None,
            max_tokens: Some(1024),
            temperature: Some(0.5),
            top_p: None,
            stream: Some(true),
            tools: None,
            tool_choice: None,
            extra: None,
        thinking_mode: None,
        };
        let out = to_responses(&req);
        assert_eq!(out.model, "gpt-5");

        assert_eq!(out.max_output_tokens, Some(1024));
        assert_eq!(out.stream, Some(true));
    }

    #[test]
    fn to_responses_text_part_type_by_role() {
        // user + assistant + user 三轮：助手轮的 part 必须是 output_text，用户轮是 input_text
        let body = json!({
            "model": "gpt-5",
            "input": [
                { "role": "user", "content": [{"type":"input_text","text":"q1"}] },
                { "role": "assistant", "content": [{"type":"output_text","text":"a1"}] },
                { "role": "user", "content": [{"type":"input_text","text":"q2"}] }
            ]
        });
        let req = from_responses(&body).expect("parse");
        let out = to_responses(&req);
        let parts: Vec<(&str, &str)> = out
            .input
            .iter()
            .map(|i| (i["role"].as_str().unwrap(), i["content"][0]["type"].as_str().unwrap()))
            .collect();
        assert_eq!(
            parts,
            vec![("user", "input_text"), ("assistant", "output_text"), ("user", "input_text")]
        );
    }

    #[test]
    fn to_responses_single_user_turn_unchanged() {
        // 回归防线：单轮 user 请求仍然是 input_text
        let req = from_responses(&json!({ "model": "gpt-5", "input": "say hi" })).expect("parse");
        let out = to_responses(&req);
        assert_eq!(out.input.len(), 1);
        assert_eq!(out.input[0]["type"], "message");
        assert_eq!(out.input[0]["role"], "user");
        assert_eq!(out.input[0]["content"][0]["type"], "input_text");
        assert_eq!(out.input[0]["content"][0]["text"], "say hi");
    }

    #[test]
    fn to_responses_tool_choice_roundtrip() {
        // tool_choice 曾在 Responses→Responses 路径静默消失
        for (inbound, expected) in [
            (json!("auto"), json!("auto")),
            (json!("required"), json!("required")),
            (json!("none"), json!("none")),
            (json!({"type":"function","name":"f"}), json!({"type":"function","name":"f"})),
        ] {
            let req = from_responses(&json!({
                "model": "gpt-5", "input": "hi", "tool_choice": inbound
            }))
            .expect("parse");
            assert_eq!(to_responses(&req).tool_choice, Some(expected));
        }
        // 未指定时不写出该键
        let req = from_responses(&json!({ "model": "gpt-5", "input": "hi" })).expect("parse");
        assert!(to_responses(&req).tool_choice.is_none());
    }

    // ── render_responses_response 测试 ──
    #[test]
    fn render_responses_text_only() {
        use crate::converter::NonStreamResponse;
        use super::render_responses_response;

        let r = NonStreamResponse {
            id: "test".to_string(),
            model: "gpt-5".to_string(),
            text: Some("Hello world".to_string()),
            tool_uses: vec![],
            stop_reason: "end_turn".to_string(),
            input_tokens: 10,
            output_tokens: 5,
            cache_read_tokens: 0,
            reasoning: None,
        };

        let out = render_responses_response(&r).unwrap();
        assert_eq!(out["id"], "test");
        assert_eq!(out["model"], "gpt-5");
        assert_eq!(out["status"], "completed");
        assert_eq!(out["output"].as_array().unwrap().len(), 1);
        assert_eq!(out["output"][0]["type"], "text");
        assert_eq!(out["output"][0]["text"], "Hello world");
    }

    #[test]
    fn render_responses_with_reasoning() {
        use crate::converter::NonStreamResponse;
        use super::render_responses_response;

        let r = NonStreamResponse {
            id: "test".to_string(),
            model: "gpt-5".to_string(),
            text: Some("Answer".to_string()),
            tool_uses: vec![],
            stop_reason: "end_turn".to_string(),
            input_tokens: 20,
            output_tokens: 10,
            cache_read_tokens: 0,
            reasoning: Some("Thinking...".to_string()),
        };

        let out = render_responses_response(&r).unwrap();
        assert_eq!(out["output"].as_array().unwrap().len(), 2);
        assert_eq!(out["output"][0]["type"], "text");
        assert_eq!(out["output"][0]["text"], "Answer");
        assert_eq!(out["output"][1]["type"], "summary");
        assert_eq!(out["output"][1]["text"], "Thinking...");
    }

    #[test]
    fn render_responses_with_function_call() {
        use crate::converter::NonStreamResponse;
        use super::render_responses_response;

        let r = NonStreamResponse {
            id: "test".to_string(),
            model: "gpt-5".to_string(),
            text: Some("Let me check".to_string()),
            tool_uses: vec![
                ("tool-1".to_string(), "read_file".to_string(), json!({"path": "/tmp"})),
            ],
            stop_reason: "tool_use".to_string(),
            input_tokens: 15,
            output_tokens: 8,
            cache_read_tokens: 0,
            reasoning: None,
        };

        let out = render_responses_response(&r).unwrap();
        assert_eq!(out["output"].as_array().unwrap().len(), 2);
        assert_eq!(out["output"][0]["type"], "text");
        assert_eq!(out["output"][1]["type"], "function_call");
        assert_eq!(out["output"][1]["id"], "tool-1");
        assert_eq!(out["output"][1]["name"], "read_file");
        assert_eq!(out["output"][1]["arguments"]["path"], "/tmp");
    }

    #[test]
    fn render_responses_with_all() {
        use crate::converter::NonStreamResponse;
        use super::render_responses_response;

        let r = NonStreamResponse {
            id: "test".to_string(),
            model: "gpt-5".to_string(),
            text: Some("Result".to_string()),
            tool_uses: vec![
                ("tool-2".to_string(), "write".to_string(), json!({"content": "data"})),
            ],
            stop_reason: "tool_use".to_string(),
            input_tokens: 25,
            output_tokens: 12,
            cache_read_tokens: 0,
            reasoning: Some("Analyzing...".to_string()),
        };

        let out = render_responses_response(&r).unwrap();
        // text + summary + function_call
        assert_eq!(out["output"].as_array().unwrap().len(), 3);
        assert_eq!(out["output"][0]["type"], "text");
        assert_eq!(out["output"][1]["type"], "summary");
        assert_eq!(out["output"][2]["type"], "function_call");
    }

    #[test]
    fn to_responses_preserves_system_tools_and_tool_turns() {
        let req = ChatRequest {
            thinking_budget: None,
            model: "gpt-5".into(),
            messages: vec![
                Message { role: Role::Assistant, content: MessageContent::Blocks(vec![
                    ContentBlock::ToolUse { id: "call_1".into(), name: "lookup".into(), input: json!({"q": "x"}), extra: None },
                ]) },
                Message { role: Role::Tool, content: MessageContent::Blocks(vec![
                    ContentBlock::ToolResult { tool_use_id: "call_1".into(), content: "ok".into(), name: Some("lookup".into()), is_error: None, content_blocks: None, extra: None },
                ]) },
            ],
            system: Some(SystemContent::Text("system prompt".into())),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: Some(true),
            tools: Some(vec![Tool { name: "lookup".into(), description: Some("look up".into()), input_schema: json!({"type":"object"}), tool_type: None, cache_control: None, extra: None }]),
            tool_choice: Some(ToolChoice::Auto),
            extra: None,
        thinking_mode: None,
        };

        let out = to_responses(&req);
        assert_eq!(out.instructions.as_deref(), Some("system prompt"));
        assert!(out.tools.is_some());
        assert!(out.input.iter().any(|item| item["type"] == "function_call"));
        assert!(out.input.iter().any(|item| item["type"] == "function_call_output"));
    }

    #[test]
    fn render_responses_empty_message() {
        use crate::converter::NonStreamResponse;
        use super::render_responses_response;

        let r = NonStreamResponse {
            id: "empty".to_string(),
            model: "gpt-5".to_string(),
            text: None,
            tool_uses: vec![],
            stop_reason: "end_turn".to_string(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            reasoning: None,
        };

        let out = render_responses_response(&r).unwrap();
        assert_eq!(out["output"].as_array().unwrap().len(), 1);
        assert_eq!(out["output"][0]["type"], "text");
        assert_eq!(out["output"][0]["text"], "");
    }

    // --- parse_responses_response 测试 ---
    #[test]
    fn parse_responses_response_with_summary() {
        // Responses API reasoning via summary[]
        let body = json!({
            "id": "resp_123",
            "status": "completed",
            "model": "gpt-5",
            "output": [
                {
                    "type": "summary",
                    "summary": [
                        {"text": "Let me analyze this.\n\nStep 1: Understand."},
                        {"text": "Step 2: Solve."}
                    ]
                },
                {
                    "type": "text",
                    "text": "Final answer here."
                }
            ],
            "usage": {
                "input_tokens": 25,
                "output_tokens": 35,
                "total_tokens": 60
            }
        });

        let parsed = parse_responses_response(&body, "gpt-5").expect("should parse");
        assert_eq!(parsed.id, "resp_123");
        assert_eq!(parsed.model, "gpt-5");
        assert_eq!(parsed.text.as_deref(), Some("Final answer here."));
        assert_eq!(parsed.reasoning.as_deref(), Some("Let me analyze this.\n\nStep 1: Understand.\n\nStep 2: Solve."));
        assert_eq!(parsed.stop_reason, "end_turn");
        assert_eq!(parsed.input_tokens, 25);
        assert_eq!(parsed.output_tokens, 35);
    }

    #[test]
    fn parse_responses_response_with_function_call() {
        let body = json!({
            "id": "resp_456",
            "status": "completed",
            "model": "gpt-5",
            "output": [
                {"type": "text", "text": "I'll call a function."},
                {
                    "type": "function_call",
                    "id": "call_abc",
                    "name": "search",
                    "arguments": {"query": "weather"}
                }
            ],
            "usage": {
                "input_tokens": 15,
                "output_tokens": 20,
                "total_tokens": 35
            }
        });

        let parsed = parse_responses_response(&body, "gpt-5").expect("should parse");
        assert_eq!(parsed.text.as_deref(), Some("I'll call a function."));
        assert_eq!(parsed.tool_uses.len(), 1);
        assert_eq!(parsed.tool_uses[0].0, "call_abc");
        assert_eq!(parsed.tool_uses[0].1, "search");
        assert_eq!(parsed.tool_uses[0].2, serde_json::json!({"query": "weather"}));
    }

    #[test]
    fn parse_responses_response_minimal() {
        // 最简情况：只有文本输出，无 reasoning
        let body = json!({
            "id": "resp_789",
            "status": "completed",
            "model": "gpt-5",
            "output": [
                {"type": "text", "text": "Simple response"}
            ],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            }
        });

        let parsed = parse_responses_response(&body, "gpt-5").expect("should parse");
        assert_eq!(parsed.text.as_deref(), Some("Simple response"));
        assert!(parsed.reasoning.is_none());
        assert!(parsed.tool_uses.is_empty());
    }
}
