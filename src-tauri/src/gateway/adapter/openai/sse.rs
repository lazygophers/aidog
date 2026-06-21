use serde_json::Value;

use super::super::types::*;

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
