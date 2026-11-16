//! legacy Completions 流式出站渲染器（issue 70）：`text_completion` 分块。

use crate::types::{ChatStreamEvent, Usage};

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
            Some(format!("{}\ndata: [DONE]\n\n", completions_finish_chunk(finish_reason.as_deref(), model)))
        }
        ChatStreamEvent::ReasoningDelta { .. } | ChatStreamEvent::ToolDelta { .. } => None,
        ChatStreamEvent::Usage { .. } => None,
        ChatStreamEvent::Ping => None,
    }
}

/// finish_reason → text_completion 终止块（仅 `data:` 行，不含 [DONE] 哨兵）。
fn completions_finish_chunk(finish_reason: Option<&str>, model: &str) -> String {
    let reason = match finish_reason.unwrap_or("end_turn") {
        "max_tokens" | "length" => "length",
        _ => "stop", // legacy 无 tool_calls，工具回合同样按 stop 收尾
    };
    format!(
        "data: {}\n\n",
        serde_json::json!({
            "id": "",
            "object": "text_completion",
            "model": model,
            "choices": [{"index": 0, "text": "", "logprobs": null, "finish_reason": reason}]
        })
    )
}

/// usage → usage 终块（issue 71，`stream_options.include_usage` 门控同 OpenAI Chat）。
fn completions_usage_chunk(usage: &Usage, model: &str) -> String {
    format!(
        "data: {}\n\n",
        serde_json::json!({
            "id": "",
            "object": "text_completion",
            "model": model,
            "choices": [],
            "usage": {
                "prompt_tokens": usage.prompt_tokens.unwrap_or(0),
                "completion_tokens": usage.completion_tokens.unwrap_or(0),
                "total_tokens": usage.effective_total().unwrap_or(0),
            },
        })
    )
}

/// legacy Completions 客户端流式状态机（issue 71）：与 [`super::super::openai::OpenAiSseState`]
/// 同一时序模型——usage 块仅当 `stream_options.include_usage` 时下发，[DONE] 压到 usage 之后。
pub struct CompletionsSseState {
    include_usage: bool,
    pending_usage: Usage,
    finish_emitted: bool,
    done_sent: bool,
}

impl CompletionsSseState {
    pub fn new(include_usage: bool) -> Self {
        Self {
            include_usage,
            pending_usage: Usage::default(),
            finish_emitted: false,
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
                if self.finish_emitted {
                    let mut out = completions_usage_chunk(&self.pending_usage, model);
                    out.push_str("data: [DONE]\n\n");
                    self.done_sent = true;
                    Some(out)
                } else {
                    None
                }
            }
            ChatStreamEvent::Stop { finish_reason } => {
                if self.finish_emitted {
                    return None;
                }
                self.finish_emitted = true;
                let mut out = completions_finish_chunk(finish_reason.as_deref(), model);
                if self.include_usage {
                    out.push_str(&completions_usage_chunk(&self.pending_usage, model));
                }
                out.push_str("data: [DONE]\n\n");
                self.done_sent = true;
                Some(out)
            }
            other => to_completions_sse(other, model),
        }
    }

    /// 流末冲刷：等 usage 未等到时补发 [DONE]。
    pub fn flush(&mut self) -> Option<String> {
        if self.done_sent {
            return None;
        }
        self.done_sent = true;
        Some("data: [DONE]\n\n".to_string())
    }

    /// 流被错误帧提前终止：不再补发 [DONE]。
    pub fn mark_done(&mut self) {
        self.done_sent = true;
    }
}

#[cfg(test)]
#[path = "test_sse.rs"]
mod test_sse;
