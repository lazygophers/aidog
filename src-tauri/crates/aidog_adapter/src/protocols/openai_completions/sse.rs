//! legacy Completions 流式出站渲染器（issue 70）：`text_completion` 分块。

use crate::types::ChatStreamEvent;

/// 将中立事件渲染为 legacy Completions SSE 分块（只有 `data:` 行，无 `event:`）。
/// legacy wire 无 reasoning / tools 槽位，对应事件不渲染（返回 None）。
pub fn to_completions_sse(event: &ChatStreamEvent, model: &str) -> Option<String> {
    match event {
        ChatStreamEvent::Start { id, model } => Some(format!(
            "data: {}\n\n",
            serde_json::json!({
                "id": id,
                "object": "text_completion",
                "model": model,
                "choices": [{"index": 0, "text": "", "logprobs": null, "finish_reason": null}]
            })
        )),
        ChatStreamEvent::Delta { text } => Some(format!(
            "data: {}\n\n",
            serde_json::json!({
                "id": "",
                "object": "text_completion",
                "model": model,
                "choices": [{"index": 0, "text": text, "logprobs": null, "finish_reason": null}]
            })
        )),
        ChatStreamEvent::Stop { finish_reason } => {
            let reason = match finish_reason.as_deref().unwrap_or("end_turn") {
                "max_tokens" | "length" => "length",
                _ => "stop", // legacy 无 tool_calls，工具回合同样按 stop 收尾
            };
            Some(format!(
                "data: {}\n\ndata: [DONE]\n\n",
                serde_json::json!({
                    "id": "",
                    "object": "text_completion",
                    "model": model,
                    "choices": [{"index": 0, "text": "", "logprobs": null, "finish_reason": reason}]
                })
            ))
        }
        ChatStreamEvent::ReasoningDelta { .. } | ChatStreamEvent::ToolDelta { .. } => None,
        ChatStreamEvent::Usage { .. } => None,
        ChatStreamEvent::Ping => None,
    }
}

#[cfg(test)]
#[path = "test_sse.rs"]
mod test_sse;
