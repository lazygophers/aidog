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
}
