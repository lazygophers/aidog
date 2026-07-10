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
    })
}
