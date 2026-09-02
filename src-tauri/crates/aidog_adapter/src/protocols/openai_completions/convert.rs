use serde_json::Value;

use crate::types::*;

/// OpenAI Legacy Completions API (`/v1/completions`) 请求格式
/// 使用 `prompt` 字段而非 `messages`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompletionsRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// 转为 Completions 格式：将 messages 拼接为 prompt
pub fn to_completions(req: &ChatRequest) -> CompletionsRequest {
    let prompt = req
        .messages
        .iter()
        .map(|m| {
            let role_str = match m.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "System",
                Role::Tool => "Tool",
            };
            let text = match &m.content {
                MessageContent::Text(t) => t.clone(),
                MessageContent::Blocks(blocks) => blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.clone()),
                        // legacy `/v1/completions` 没有 tools API（无 tools / tool_choice / tool 角色），
                        // 工具回合只能落进 prompt 文本。此前整块跳过 → 工具调用与结果在 prompt 里
                        // 凭空消失，模型看到一段断裂的对话；失败标记同样无从体现（票 11）。
                        ContentBlock::ToolUse { name, input, .. } => {
                            Some(format!("\n[tool_use {name}] {input}"))
                        }
                        ContentBlock::ToolResult {
                            content, is_error, ..
                        } => Some(format!(
                            "\n[tool_result] {}",
                            mark_tool_error(content, *is_error)
                        )),
                        ContentBlock::Unknown(_) => None,
                    })
                    .collect::<Vec<_>>()
                    .join(""),
            };
            format!("{}: {}", role_str, text)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    CompletionsRequest {
        model: req.model.clone(),
        prompt,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        stream: req.stream,
        // `ChatRequest` 无强类型 stop 字段，客户端原值落在 flatten 的 `extra` 里
        // （anthropic 入站写 stop_sequences，openai 族写 stop）。出站 forward 层还有一道
        // 按目标协议的白名单兜底，本处已写出时那道会因 key 已存在而跳过（票 01）。
        stop: req.extra.as_ref().and_then(extra_stop),
    }
}

/// 从 `ChatRequest.extra` 取停止序列，兼容 `stop` / `stop_sequences` 两种写法与
/// 字符串 / 数组两种值形态；取不到或形态不合法返回 None。
fn extra_stop(extra: &Value) -> Option<Vec<String>> {
    let v = extra.get("stop").or_else(|| extra.get("stop_sequences"))?;
    match v {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(items) => {
            let out: Vec<String> = items
                .iter()
                .filter_map(|i| i.as_str().map(String::from))
                .collect();
            (!out.is_empty()).then_some(out)
        }
        _ => None,
    }
}

/// 解析 OpenAI Legacy Completions API 非流式响应为归一 NonStreamResponse
pub fn parse_completions_response(
    body: &Value,
    fallback_model: &str,
) -> Option<crate::converter::NonStreamResponse> {
    // Legacy /v1/completions 格式：choices[0].text
    // 部分提供商可能复用 chat 格式 choices[0].message.content
    let choices = body.get("choices")?.as_array()?;
    let choice = choices.first()?;

    // 优先取 message（chat 格式），回退 text（legacy 格式）
    let (text, reasoning) = if let Some(message) = choice.get("message") {
        let t = message
            .get("content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let r = message
            .get("reasoning_content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        (t, r)
    } else {
        let t = choice
            .get("text")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        (t, None)
    };

    let finish_reason = choice
        .get("finish_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("stop");
    let stop_reason = match finish_reason {
        "length" => "max_tokens",
        "stop" => "end_turn",
        _ => "end_turn",
    }
    .to_string();

    let usage = body.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_read_tokens = usage
        .and_then(|u| u.get("prompt_tokens_details"))
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let id = body
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_model)
        .to_string();

    Some(crate::converter::NonStreamResponse {
        id,
        model,
        text,
        tool_uses: Vec::new(), // legacy completions 不支持 tool_calls
        stop_reason,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        reasoning,
    })
}

/// 渲染归一响应为 OpenAI Legacy Completions 非流式响应体。
///
/// 映射规则（legacy 格式，无 tool_calls）：
/// - choices[0].text: reasoning + text 拼接（reasoning 排前缀）
/// - choices[0].finish_reason: 映射 stop_reason
/// - usage: prompt_tokens/completion_tokens
pub fn render_completions_response(r: &crate::converter::NonStreamResponse) -> Option<Value> {
    // 拼接 reasoning + text（legacy 格式）
    let mut combined = String::new();
    if let Some(reasoning) = &r.reasoning
        && !reasoning.is_empty()
    {
        combined.push_str(reasoning);
    }
    if let Some(text) = &r.text
        && !text.is_empty()
    {
        combined.push_str(text);
    }

    // 映射 finish_reason
    let finish_reason = match r.stop_reason.as_str() {
        "tool_use" => "stop", // legacy 无 tool_calls，按 stop 处理
        "max_tokens" => "length",
        "end_turn" | "stop_sequence" => "stop",
        _ => "stop",
    };

    Some(serde_json::json!({
        "id": r.id,
        "model": r.model,
        "choices": [{
            "index": 0,
            "text": combined,
            "finish_reason": finish_reason,
        }],
        "usage": {
            "prompt_tokens": r.input_tokens,
            "completion_tokens": r.output_tokens,
            "total_tokens": r.input_tokens + r.output_tokens,
        }
    }))
}

/// 从 Completions API 请求解析为内部 ChatRequest
/// 将 prompt 字符串拆分为单条 User 消息
pub fn from_completions(body: &Value) -> Option<ChatRequest> {
    let model = body.get("model")?.as_str()?.to_string();
    let prompt = body.get("prompt")?.as_str().unwrap_or("").to_string();

    Some(ChatRequest {
        thinking_budget: None,
        model,
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(prompt),
        }],
        system: None,
        max_tokens: body
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        temperature: body
            .get("temperature")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32),
        top_p: body.get("top_p").and_then(|v| v.as_f64()).map(|v| v as f32),
        stream: body.get("stream").and_then(|v| v.as_bool()),
        tools: None,
        tool_choice: None,
        // 未建模顶层字段进 `extra`（票 11）：与 anthropic 入站的 `#[serde(flatten)]` 行为对齐，
        // 否则 completions → gemini 路径上 `stop` / `top_k` 在中立层就没了。
        extra: rest_keys(
            body,
            &[
                "model",
                "prompt",
                "max_tokens",
                "temperature",
                "top_p",
                "stream",
            ],
        ),
        thinking_mode: None,
    })
}

/// 解析 legacy Completions SSE chunk（`choices[].text` 增量）为统一 ChatStreamEvent。
pub fn parse_completions_sse(data: &Value) -> Option<ChatStreamEvent> {
    let choice = data.get("choices")?.as_array()?.first()?;
    if let Some(text) = choice.get("text").and_then(Value::as_str)
        && !text.is_empty()
    {
        return Some(ChatStreamEvent::Delta {
            text: text.to_string(),
        });
    }
    if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str)
        && (reason == "stop" || reason == "length")
    {
        return Some(ChatStreamEvent::Stop {
            finish_reason: Some(reason.to_string()),
        });
    }
    None
}

#[cfg(test)]
#[path = "test_openai_completions.rs"]
mod test_openai_completions;
