//! 客户端显式禁用思考（`disable_thinking: true`）时的**响应侧**思维链剥离。
//!
//! 出站请求侧已按目标协议写入显式禁用参数（见 `forward.rs::apply_disable_thinking`），
//! 但 MiniMax-M2 这类「思考内置、无法关闭」的上游照发思维链回来。此前响应不做处理，
//! 客户端仍能看到思考 → 用户视角「禁用没生效」（实测 request 3ed5a698：
//! `disable_thinking=true` + 上游 `thinking.type=disabled`，响应仍是纯 thinking 块）。
//!
//! 本模块在 aidog 自己的边界上兑现语义：按**客户端协议**剥掉响应里的思维链载体。
//! 上游算力照样花在思考上（无法追回），但客户端拿到的是干净正文。
//!
//! 两个入口：
//! - [`strip_thinking_in_body`]：非流式响应体（转换后 / 同协议透传均适用）。
//! - [`SseThinkingStripper`]：流式 SSE 逐帧剥离（透传分支用；转换分支直接丢
//!   `ReasoningDelta` 事件即可，不必过本状态机）。

use aidog_db::models::Protocol;
use serde_json::Value;

/// 非流式响应体剥离思维链，返回是否改动过。
///
/// 按客户端协议分派载体：
/// - Anthropic 系：`content[]` 里的 `thinking` / `redacted_thinking` 块
/// - OpenAI chat：`choices[].message.reasoning_content`
/// - OpenAI Responses：`output[]` 里的 `reasoning` 项
/// - Gemini：`candidates[].content.parts[]` 里 `thought: true` 的 part
pub fn strip_thinking_in_body(body: &mut Value, client_protocol: &Protocol) -> bool {
    match client_protocol {
        Protocol::OpenAI => strip_openai_chat(body),
        Protocol::OpenAIResponses => strip_openai_responses(body),
        Protocol::OpenAICompletions => false, // legacy completions 无独立思维链载体
        Protocol::Gemini => strip_gemini(body),
        // 其余全部是 Anthropic wire（含平台变体），与 to_client_sse 的分派口径一致
        _ => strip_anthropic(body),
    }
}

fn strip_anthropic(body: &mut Value) -> bool {
    let Some(content) = body.get_mut("content").and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let before = content.len();
    content.retain(|b| !is_thinking_block(b));
    if content.len() == before {
        return false;
    }
    // Anthropic 拒收空 content 数组：全被剔光（上游只回了思考）时补空 text 块兜底
    if content.is_empty() {
        content.push(serde_json::json!({ "type": "text", "text": "" }));
    }
    true
}

fn is_thinking_block(b: &Value) -> bool {
    matches!(
        b.get("type").and_then(|t| t.as_str()),
        Some("thinking") | Some("redacted_thinking")
    )
}

fn strip_openai_chat(body: &mut Value) -> bool {
    let Some(choices) = body.get_mut("choices").and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let mut changed = false;
    for c in choices {
        if let Some(msg) = c.get_mut("message").and_then(|v| v.as_object_mut())
            && msg.remove("reasoning_content").is_some()
        {
            changed = true;
        }
    }
    changed
}

fn strip_openai_responses(body: &mut Value) -> bool {
    let Some(output) = body.get_mut("output").and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let before = output.len();
    output.retain(|item| item.get("type").and_then(|t| t.as_str()) != Some("reasoning"));
    output.len() != before
}

fn strip_gemini(body: &mut Value) -> bool {
    let Some(candidates) = body.get_mut("candidates").and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let mut changed = false;
    for cand in candidates {
        if let Some(parts) = cand
            .get_mut("content")
            .and_then(|c| c.get_mut("parts"))
            .and_then(|p| p.as_array_mut())
        {
            let before = parts.len();
            parts.retain(|p| p.get("thought").and_then(|t| t.as_bool()) != Some(true));
            changed |= parts.len() != before;
        }
    }
    changed
}

/// 流式 SSE 思维链剥离状态机（同协议透传分支用）。
///
/// 逐块喂入上游 SSE 文本，返回剥掉思维链帧后的文本。内部按空行分帧缓冲，跨 chunk 被切断的
/// 帧留到下次拼接后再判（与 `SseLineReassembler` 同型 idiom）。
///
/// Anthropic wire 额外做 **block index 重编号**：剔掉 thinking 块后剩余块必须连续编号，
/// 否则客户端按 index 装配 content 数组会留空洞（`content_block_start index=1` 却没有 0）。
pub struct SseThinkingStripper {
    protocol: Protocol,
    buf: String,
    /// 被剔除的 thinking 块的原 index
    dropped: std::collections::HashSet<u32>,
    /// 保留块的原 index → 重编号后的 index
    remap: std::collections::HashMap<u32, u32>,
    next_out_index: u32,
}

impl SseThinkingStripper {
    pub fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            buf: String::new(),
            dropped: Default::default(),
            remap: Default::default(),
            next_out_index: 0,
        }
    }

    /// 喂入一段上游 SSE 文本，返回可下发的剥离后文本（不完整的尾帧留在内部缓冲）。
    pub fn push(&mut self, text: &str) -> String {
        self.buf.push_str(text);
        let mut out = String::new();
        // SSE 帧以空行分隔；最后一段若无结尾空行则不完整，留缓冲
        while let Some(pos) = self.buf.find("\n\n") {
            let frame: String = self.buf.drain(..pos + 2).collect();
            if let Some(kept) = self.filter_frame(&frame) {
                out.push_str(&kept);
            }
        }
        out
    }

    /// 冲刷残留（上游流结束时调，避免最后一帧无空行结尾被吞）。
    pub fn finish(&mut self) -> String {
        let rest = std::mem::take(&mut self.buf);
        if rest.is_empty() {
            return String::new();
        }
        self.filter_frame(&rest).unwrap_or_default()
    }

    /// 单帧决策：`None` = 整帧丢弃；`Some(s)` = 下发（可能是改写后的）。
    fn filter_frame(&mut self, frame: &str) -> Option<String> {
        let Some((prefix, data_json)) = split_data_line(frame) else {
            return Some(frame.to_string()); // 非 data 帧（注释 / 心跳）原样过
        };
        let Ok(mut json) = serde_json::from_str::<Value>(data_json) else {
            return Some(frame.to_string()); // `[DONE]` 等非 JSON 哨兵原样过
        };
        let keep = match self.protocol {
            Protocol::OpenAI | Protocol::OpenAICompletions => strip_openai_stream_delta(&mut json),
            Protocol::OpenAIResponses => strip_responses_stream_event(&json),
            Protocol::Gemini => {
                strip_gemini(&mut json);
                // parts 被剔空的帧无内容可下发
                !json
                    .get("candidates")
                    .and_then(|c| c.as_array())
                    .is_some_and(|arr| !arr.is_empty() && arr.iter().all(gemini_candidate_empty))
            }
            _ => return self.filter_anthropic_frame(&prefix, json),
        };
        if !keep {
            return None;
        }
        Some(format!("{prefix}data: {json}\n\n"))
    }

    /// Anthropic wire 帧：丢 thinking 块的 start/delta/stop，其余帧的 index 重编号。
    fn filter_anthropic_frame(&mut self, prefix: &str, mut json: Value) -> Option<String> {
        let ty = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let idx = json.get("index").and_then(|v| v.as_u64()).map(|v| v as u32);
        match ty {
            "content_block_start" => {
                let i = idx?;
                let is_thinking = json.get("content_block").is_some_and(is_thinking_block);
                if is_thinking {
                    self.dropped.insert(i);
                    return None;
                }
                let new_i = self.next_out_index;
                self.next_out_index += 1;
                self.remap.insert(i, new_i);
                set_index(&mut json, new_i);
            }
            "content_block_delta" | "content_block_stop" => {
                let i = idx?;
                if self.dropped.contains(&i) {
                    return None;
                }
                // 未见过 start（上游没发或已被过滤）时按原 index 下发，不臆造映射
                if let Some(&new_i) = self.remap.get(&i) {
                    set_index(&mut json, new_i);
                }
            }
            _ => {}
        }
        Some(format!("{prefix}data: {json}\n\n"))
    }
}

fn gemini_candidate_empty(c: &Value) -> bool {
    c.get("content")
        .and_then(|x| x.get("parts"))
        .and_then(|p| p.as_array())
        .is_some_and(|arr| arr.is_empty())
}

/// 拆一帧为 (data 行之前的部分, data 行的 JSON 文本)。无 data 行返回 None。
fn split_data_line(frame: &str) -> Option<(String, &str)> {
    let mut prefix = String::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("data: ") {
            return Some((prefix, rest));
        }
        if !line.is_empty() {
            prefix.push_str(line);
            prefix.push('\n');
        }
    }
    None
}

fn set_index(json: &mut Value, i: u32) {
    if let Some(obj) = json.as_object_mut() {
        obj.insert("index".to_string(), Value::from(i));
    }
}

/// OpenAI chat 流帧：剔 `delta.reasoning_content`；delta 因此变空则整帧丢弃。
/// 返回是否保留该帧。
fn strip_openai_stream_delta(json: &mut Value) -> bool {
    let Some(choices) = json.get_mut("choices").and_then(|v| v.as_array_mut()) else {
        return true;
    };
    let mut any_content = false;
    for c in choices.iter_mut() {
        if let Some(delta) = c.get_mut("delta").and_then(|v| v.as_object_mut()) {
            delta.remove("reasoning_content");
            if !delta.is_empty() {
                any_content = true;
            }
        } else {
            any_content = true; // 非 delta 帧（如 usage / finish 帧）保留
        }
        if c.get("finish_reason").is_some_and(|f| !f.is_null()) {
            any_content = true;
        }
    }
    any_content
}

/// OpenAI Responses 流帧：`response.reasoning*` 系列事件整帧丢弃。返回是否保留。
fn strip_responses_stream_event(json: &Value) -> bool {
    let ty = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
    !ty.contains("reasoning")
}

#[cfg(test)]
#[path = "test_thinking_strip.rs"]
mod test_thinking_strip;
