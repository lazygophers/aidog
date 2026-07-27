use serde_json::Value;

/// 将 OpenAI Chat Completion **非流式**响应解析为归一 [`NonStreamResponse`]。
///
/// 处理 `choices[0].message` 的 `content`(文本) + `tool_calls`(函数调用) 并存：
/// - content 文本 → text 段
/// - 每个 tool_call → tool_use(id/name/input)，input 由 function.arguments(JSON 字符串)解析
/// - finish_reason 映射为 anthropic stop_reason: tool_calls→tool_use / length→max_tokens
///   / stop→end_turn / 其它→end_turn
/// - usage: prompt_tokens→input_tokens / completion_tokens→output_tokens
///   / prompt_tokens_details.cached_tokens→cache_read
///
/// `reasoning_content`(GLM 思维链等非标准字段)被忽略，不影响 content/tool_use 产出。
pub fn parse_openai_response(body: &Value, fallback_model: &str) -> Option<super::super::converter::NonStreamResponse> {
    let choices = body.get("choices")?.as_array()?;
    let choice = choices.first()?;
    let message = choice.get("message")?;

    let text = message
        .get("content")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // 提取 reasoning_content（GLM/deepseek/商汤思维链等非标准字段）
    let reasoning = message
        .get("reasoning_content")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let mut tool_uses: Vec<(String, String, Value)> = Vec::new();
    if let Some(tcs) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for tc in tcs {
            let id = tc
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let func = tc.get("function");
            let name = func
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // arguments 是 JSON 字符串；解析失败兜底空对象（Anthropic input 必须是对象）
            let input = func
                .and_then(|f| f.get("arguments"))
                .and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or_else(|| Value::Object(Default::default()));
            tool_uses.push((id, name, input));
        }
    }

    let finish_reason = choice
        .get("finish_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("stop");
    let stop_reason = match finish_reason {
        "tool_calls" => "tool_use",
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

    Some(super::super::converter::NonStreamResponse {
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

/// 渲染归一响应为 OpenAI Chat Completions 非流式响应体。
///
/// 映射规则：
/// - message.content: 文本内容（reasoning 放入 reasoning_content 字段，不拼 content）
/// - message.tool_calls: 工具调用数组（id/name/arguments）
/// - message.reasoning_content: 思维链字段（独立字段，与 content 并列）
/// - finish_reason: tool_use→tool_calls / max_tokens→length / end_turn→stop
/// - usage: input_tokens→prompt_tokens / output_tokens→completion_tokens
pub fn render_openai_response(r: &super::super::converter::NonStreamResponse) -> Option<Value> {
    // 构建 message 对象
    let mut message = serde_json::json!({
        "role": "assistant",
    });

    // 添加 content（仅文本，不含 reasoning）
    if let Some(text) = &r.text
        && !text.is_empty() {
            message["content"] = serde_json::json!(text);
        }

    // 添加 reasoning_content（独立字段）
    if let Some(reasoning) = &r.reasoning
        && !reasoning.is_empty() {
            message["reasoning_content"] = serde_json::json!(reasoning);
        }

    // 添加 tool_calls
    if !r.tool_uses.is_empty() {
        let tool_calls: Vec<Value> = r.tool_uses.iter().map(|(id, name, input)| {
            serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string()),
                }
            })
        }).collect();
        message["tool_calls"] = serde_json::json!(tool_calls);
    }

    // 映射 finish_reason
    let finish_reason = match r.stop_reason.as_str() {
        "tool_use" => "tool_calls",
        "max_tokens" => "length",
        "end_turn" | "stop_sequence" => "stop",
        _ => "stop",
    };

    Some(serde_json::json!({
        "id": r.id,
        "model": r.model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason,
        }],
        "usage": {
            "prompt_tokens": r.input_tokens,
            "completion_tokens": r.output_tokens,
            "total_tokens": r.input_tokens + r.output_tokens,
            "prompt_tokens_details": {
                "cached_tokens": r.cache_read_tokens,
            }
        }
    }))
}

#[cfg(test)]
#[path = "test_response.rs"]
mod test_response;
