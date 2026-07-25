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

/// 解析 OpenAI Responses API 非流式响应为归一 NonStreamResponse
pub fn parse_responses_response(body: &Value, fallback_model: &str) -> Option<super::converter::NonStreamResponse> {
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

/// 渲染归一响应为 OpenAI Responses API 非流式响应体。
///
/// 映射规则：
/// - output[]: text item（type=text） + reasoning summary item（type=summary） + function_call item
/// - status: completed
/// - usage: prompt_tokens/completion_tokens
pub fn render_responses_response(r: &super::converter::NonStreamResponse) -> Option<Value> {
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
///   复杂字段（tools / reasoning / tool 调用回合）暂不转换（TODO），保证基本文本对话不 400。
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
        // 数组形态：逐 item 解析 role + content
        Some(Value::Array(items)) => {
            for item in items {
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
        _ => return None,
    }

    // instructions → system prompt（Codex 用 instructions 传系统提示）
    let system = body
        .get("instructions")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| SystemContent::Text(s.to_string()));

    Some(ChatRequest {
        model,
        messages,
        system,
        max_tokens: body.get("max_output_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
        temperature: body.get("temperature").and_then(|v| v.as_f64()).map(|v| v as f32),
        top_p: body.get("top_p").and_then(|v| v.as_f64()).map(|v| v as f32),
        stream: body.get("stream").and_then(|v| v.as_bool()),
        // TODO: Responses tools / tool_choice / reasoning 转换暂未实现（与内部 Tool schema 形态不一致）
        tools: None,
        tool_choice: None,
        extra: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        use super::super::types::{MessageContent, Role};
        let req = ChatRequest {
            model: "gpt-5".into(),
            messages: vec![
                crate::gateway::adapter::types::Message {
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
        };
        let out = to_responses(&req);
        assert_eq!(out.model, "gpt-5");

        assert_eq!(out.max_output_tokens, Some(1024));
        assert_eq!(out.stream, Some(true));
    }

    // ── render_responses_response 测试 ──
    #[test]
    fn render_responses_text_only() {
        use super::super::converter::NonStreamResponse;
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
        use super::super::converter::NonStreamResponse;
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
        use super::super::converter::NonStreamResponse;
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
        use super::super::converter::NonStreamResponse;
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
    fn render_responses_empty_message() {
        use super::super::converter::NonStreamResponse;
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
        // 兜底空 text item
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
