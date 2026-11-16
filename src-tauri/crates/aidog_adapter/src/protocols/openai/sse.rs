use serde_json::Value;

use crate::types::*;

/// 解析 OpenAI SSE 格式的流式事件
pub fn parse_openai_sse(data: &Value) -> Option<ChatStreamEvent> {
    let choices = data.get("choices")?.as_array()?;
    let choice = choices.first()?;
    let index = choice.get("index")?.as_u64()? as u32;
    let delta = choice.get("delta")?;

    // 检查是否有 tool_calls
    if let Some(tool_calls_val) = delta.get("tool_calls")
        && let Some(tool_calls) = tool_calls_val.as_array()
        && let Some(tc) = tool_calls.first()
    {
        let id = tc.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
        let func = tc.get("function");
        let name = func
            .and_then(|f| f.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
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

    // 文本 delta
    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
        if content.is_empty() {
            return None;
        }
        return Some(ChatStreamEvent::Delta {
            text: content.to_string(),
        });
    }

    // reasoning_content delta（GLM/deepseek/商汤思维链流式增量）
    if let Some(reasoning) = delta.get("reasoning_content").and_then(|v| v.as_str())
        && !reasoning.is_empty()
    {
        return Some(ChatStreamEvent::ReasoningDelta {
            text: reasoning.to_string(),
        });
    }

    // 结束
    if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str())
        && (reason == "stop" || reason == "tool_calls" || reason == "length")
    {
        return Some(ChatStreamEvent::Stop {
            finish_reason: Some(reason.to_string()),
        });
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
        ChatStreamEvent::ReasoningDelta { text } => Some(format!(
            "data: {}\n\n",
            serde_json::json!({
                "id": "",
                "object": "chat.completion.chunk",
                "choices": [{"index": 0, "delta": {"reasoning_content": text}, "finish_reason": null}]
            })
        )),
        ChatStreamEvent::ToolDelta {
            index,
            id,
            name,
            input,
        } => {
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
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(""))
            }
        }
        ChatStreamEvent::Stop { finish_reason } => {
            Some(format!("{}\ndata: [DONE]\n\n", chat_finish_chunk(finish_reason.as_deref())))
        }
        ChatStreamEvent::Usage { .. } => None,
        // openai wire 没有 ping 事件，无处安放；keep-alive 由网关侧自发（见 aidog_core::gateway::proxy::stream::with_idle_ping）。
        ChatStreamEvent::Ping => None,
    }
}

/// finish_reason → chat.completion.chunk 终止块（仅 `data:` 行，不含 [DONE] 哨兵）。
fn chat_finish_chunk(finish_reason: Option<&str>) -> String {
    let reason = match finish_reason.unwrap_or("end_turn") {
        "end_turn" => "stop",
        other => other,
    };
    format!(
        "data: {}\n\n",
        serde_json::json!({
            "id": "",
            "object": "chat.completion.chunk",
            "choices": [{"index": 0, "delta": {}, "finish_reason": reason}]
        })
    )
}

/// usage → usage 终块（issue 71）：官方形态 choices 为空数组，usage 挂顶层；
/// 缓存读走 `prompt_tokens_details.cached_tokens`（仅上游提供时出现）。
fn chat_usage_chunk(usage: &Usage) -> String {
    let mut u = serde_json::json!({
        "prompt_tokens": usage.prompt_tokens.unwrap_or(0),
        "completion_tokens": usage.completion_tokens.unwrap_or(0),
        "total_tokens": usage.effective_total().unwrap_or(0),
    });
    if let Some(cr) = usage.cache_read_tokens {
        u["prompt_tokens_details"] = serde_json::json!({ "cached_tokens": cr });
    }
    format!(
        "data: {}\n\n",
        serde_json::json!({
            "id": "",
            "object": "chat.completion.chunk",
            "choices": [],
            "usage": u,
        })
    )
}

/// OpenAI Chat 客户端流式状态机（issue 71）：usage 块仅当客户端请求带
/// `stream_options.include_usage` 时下发，且 `[DONE]` 哨兵压到 usage 块之后
/// （官方顺序 finish 块 → usage 块 → [DONE]；标准 SDK 读到 [DONE] 即停止迭代，
/// 先发的 usage 会丢）。终帧（finish + usage + [DONE]）照 anthropic 侧同构压到
/// 流末 [`OpenAiSseState::flush`] 才发：finish 帧与 usage 帧分属两个网络 chunk
/// （openai 官方分帧形态）时，立即发终帧会把 usage 定格成 0。
pub struct OpenAiSseState {
    include_usage: bool,
    pending_usage: Usage,
    finish_seen: bool,
    finish_reason: Option<String>,
    done_sent: bool,
}

impl OpenAiSseState {
    pub fn new(include_usage: bool) -> Self {
        Self {
            include_usage,
            pending_usage: Usage::default(),
            finish_seen: false,
            finish_reason: None,
            done_sent: false,
        }
    }

    pub fn push(&mut self, event: &ChatStreamEvent, model: &str) -> Option<String> {
        match event {
            ChatStreamEvent::Usage { usage } => {
                if !self.include_usage {
                    return None;
                }
                self.pending_usage.merge_partial(usage);
                None // usage 随终帧一起在 flush 发
            }
            ChatStreamEvent::Stop { finish_reason } => {
                if self.finish_seen {
                    return None; // 重复 Stop（anthropic message_delta + message_stop / [DONE] 哨兵等）
                }
                self.finish_seen = true;
                self.finish_reason = finish_reason.clone();
                None // finish 块压到流末 flush，等 usage 到齐
            }
            other => to_openai_sse(other, model),
        }
    }

    /// 流末冲刷：Stop 已见时在此发 finish 块 + usage 块 + [DONE]（usage 帧晚于
    /// finish 帧到达甚至跨 chunk 时，这里拿到的才是终值）；未见 Stop 仅补 [DONE]。
    pub fn flush(&mut self) -> Option<String> {
        if self.done_sent {
            return None;
        }
        self.done_sent = true;
        if !self.finish_seen {
            return Some("data: [DONE]\n\n".to_string());
        }
        let mut out = chat_finish_chunk(self.finish_reason.as_deref());
        if self.include_usage {
            out.push_str(&chat_usage_chunk(&self.pending_usage));
        }
        out.push_str("data: [DONE]\n\n");
        Some(out)
    }

    /// 流被错误帧提前终止：错误帧自带 [DONE]，不再补发。
    pub fn mark_done(&mut self) {
        self.done_sent = true;
    }
}

#[cfg(test)]
#[path = "test_sse.rs"]
mod test_sse;
