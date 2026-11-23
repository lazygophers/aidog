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

/// 剥离后响应是否已无可下发内容（正文与工具调用皆无，只剩被剔光的思考载体）。
/// 按客户端协议分派载体；未知块类型一律视为可下发（不误报）。
pub fn response_contentless_after_strip(body: &Value, protocol: &Protocol) -> bool {
    match protocol {
        Protocol::OpenAI => chat_contentless(body),
        Protocol::OpenAIResponses => responses_contentless(body),
        Protocol::OpenAICompletions => false, // legacy completions 无独立思维链载体
        Protocol::Gemini => gemini_contentless(body),
        _ => body
            .get("content")
            .and_then(|v| v.as_array())
            .is_some_and(|content| !content.iter().any(is_deliverable_block)),
    }
}

/// OpenAI chat：choices 全空 content（空白也不算）且无 tool_calls。
fn chat_contentless(body: &Value) -> bool {
    body.get("choices")
        .and_then(|v| v.as_array())
        .is_some_and(|choices| {
            choices.iter().all(|c| {
                let m = c.get("message");
                let has_text = m
                    .and_then(|m| m.get("content"))
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.trim().is_empty());
                let has_tools = m
                    .and_then(|m| m.get("tool_calls"))
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| !a.is_empty());
                let has_fn_call = m
                    .and_then(|m| m.get("function_call"))
                    .is_some_and(|v| !v.is_null());
                // content 非字符串形态（数组 / 对象）视作未知载体，不误报
                let unknown_shape = m
                    .and_then(|m| m.get("content"))
                    .is_some_and(|v| !v.is_null() && !v.is_string());
                !has_text && !has_tools && !has_fn_call && !unknown_shape
            })
        })
}

/// OpenAI Responses：output 无任何非 reasoning 交付项。
fn responses_contentless(body: &Value) -> bool {
    body.get("output")
        .and_then(|v| v.as_array())
        .is_some_and(|out| {
            !out.iter()
                .any(|i| i.get("type").and_then(|t| t.as_str()) != Some("reasoning"))
        })
}

/// Gemini：每个 candidate 的 parts 剥完都无交付载荷（text / functionCall / inlineData）。
fn gemini_contentless(body: &Value) -> bool {
    body.get("candidates")
        .and_then(|v| v.as_array())
        .is_some_and(|cands| cands.iter().all(|c| !cand_has_deliverable(c)))
}

/// candidate 剥后 parts 是否还有交付载荷；content/parts 缺失按无可交付算。
fn cand_has_deliverable(cand: &Value) -> bool {
    cand.get("content")
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .is_some_and(|parts| {
            parts.iter().any(|p| {
                if p.get("thought").and_then(|t| t.as_bool()) == Some(true) {
                    return false;
                }
                p.get("text")
                    .and_then(|t| t.as_str())
                    .is_some_and(|s| !s.trim().is_empty())
                    || p.get("functionCall").is_some()
                    || p.get("inlineData").is_some()
            })
        })
}

/// 块是否有可下发载荷：非思考载体且非空 text。未知块类型一律视为可下发（不误报）。
fn is_deliverable_block(b: &Value) -> bool {
    match b.get("type").and_then(|t| t.as_str()) {
        Some("thinking") | Some("redacted_thinking") => false,
        Some("text") => b
            .get("text")
            .and_then(|t| t.as_str())
            .is_some_and(|s| !s.trim().is_empty()),
        _ => true,
    }
}

fn strip_openai_chat(body: &mut Value) -> bool {
    let Some(choices) = body.get_mut("choices").and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let mut changed = false;
    for c in choices {
        if let Some(msg) = c.get_mut("message").and_then(|v| v.as_object_mut()) {
            // StepFun 默认 reasoning_format=general 用 `reasoning` 字段（reasoning_content 是
            // deepseek-style 别名），两字段都要剥。
            if msg.remove("reasoning_content").is_some() {
                changed = true;
            }
            if msg.remove("reasoning").is_some() {
                changed = true;
            }
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
    /// 剥离后见过可下发内容块（text / tool_use / 其它非思考块）
    saw_content: bool,
    /// 上游 message_delta 携带的 stop_reason（保留到错误信息里）
    stop_reason: Option<String>,
    /// 非 Anthropic 臂跟踪的上游终止原因（openai finish_reason / responses status / gemini finishReason）
    stream_finish_reason: Option<String>,
    /// 已把终止帧换成 error 帧（后续帧丢弃）
    error_emitted: bool,
}

impl SseThinkingStripper {
    pub fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            buf: String::new(),
            dropped: Default::default(),
            remap: Default::default(),
            next_out_index: 0,
            saw_content: false,
            stop_reason: None,
            stream_finish_reason: None,
            error_emitted: false,
        }
    }

    /// 剥离后无内容（thinking-only 上游）且已把 message_stop 换成 error 帧时为 true；
    /// finish 层凭它把日志终态从 200 改记 502。
    pub fn converted_to_error(&self) -> bool {
        self.error_emitted
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
        if self.error_emitted {
            return None;
        }
        let Some((prefix, data_json)) = split_data_line(frame) else {
            return Some(frame.to_string()); // 非 data 帧（注释 / 心跳）原样过
        };
        // OpenAI 系终止哨兵：过滤后无内容 → 换 error 帧（finish_reason 帧已换则此处已被 error_emitted 拦下）
        if matches!(
            self.protocol,
            Protocol::OpenAI | Protocol::OpenAICompletions | Protocol::OpenAIResponses
        ) && data_json.trim() == "[DONE]"
        {
            if self.saw_content {
                return Some(frame.to_string());
            }
            self.error_emitted = true;
            return Some(wire_error_frame(&self.protocol, "api_error", &thinking_only_msg(self.stream_finish_reason.as_deref())));
        }
        let Ok(mut json) = serde_json::from_str::<Value>(data_json) else {
            return Some(frame.to_string()); // 其它非 JSON 哨兵原样过
        };
        match self.protocol {
            Protocol::OpenAI | Protocol::OpenAICompletions => {
                if !strip_openai_stream_delta(&mut json) {
                    return None;
                }
                self.note_openai_chat_frame(&json)
            }
            Protocol::OpenAIResponses => {
                if !strip_responses_stream_event(&json) {
                    return None;
                }
                self.note_responses_frame(&json)
            }
            Protocol::Gemini => {
                strip_gemini(&mut json);
                // parts 被剔空的帧无内容可下发
                let has_parts = !json
                    .get("candidates")
                    .and_then(|c| c.as_array())
                    .is_some_and(|arr| !arr.is_empty() && arr.iter().all(gemini_candidate_empty));
                if has_parts {
                    self.note_gemini_content(&json);
                }
                // finishReason 帧且剥后无交付内容 → 换 error
                if self.candidates_finished(&json) && !self.saw_content {
                    return Some(self.convert_to_error());
                }
                if has_parts {
                    Some(format!("{prefix}data: {json}\n\n"))
                } else {
                    None
                }
            }
            _ => self.filter_anthropic_frame(&prefix, json),
        }
    }

    /// Gemini 帧（含 finishReason 的终止帧也算）是否宣告上游终止。
    fn candidates_finished(&self, json: &Value) -> bool {
        json.get("candidates")
            .and_then(|c| c.as_array())
            .is_some_and(|arr| {
                arr.iter().any(|c| {
                    c.get("finishReason").is_some_and(|f| !f.is_null())
                })
            })
    }

    /// Gemini 帧：跟踪剥后 parts 的交付载荷与终止原因。
    fn note_gemini_content(&mut self, json: &Value) {
        if let Some(cands) = json.get("candidates").and_then(|c| c.as_array()) {
            for c in cands {
                if let Some(fr) = c.get("finishReason").and_then(|f| f.as_str()) {
                    self.stream_finish_reason = Some(fr.to_string());
                }
                if cand_has_deliverable(c) {
                    self.saw_content = true;
                }
            }
        }
    }

    /// OpenAI chat 帧：跟踪 delta 正文 / tool_calls 与 finish_reason；终止帧无内容 → 换 error。
    fn note_openai_chat_frame(&mut self, json: &Value) -> Option<String> {
        if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
            for c in choices {
                if let Some(delta) = c.get("delta") {
                    let has_text = delta
                        .get("content")
                        .and_then(|v| v.as_str())
                        .is_some_and(|s| !s.is_empty());
                    let has_tools = delta
                        .get("tool_calls")
                        .and_then(|v| v.as_array())
                        .is_some_and(|a| !a.is_empty());
                    let has_fn_call = delta.get("function_call").is_some();
                    if has_text || has_tools || has_fn_call {
                        self.saw_content = true;
                    }
                }
                if let Some(fr) = c.get("finish_reason").filter(|f| !f.is_null()) {
                    self.stream_finish_reason =
                        Some(fr.as_str().unwrap_or("unknown").to_string());
                    if !self.saw_content {
                        return Some(self.convert_to_error());
                    }
                }
            }
        }
        Some(serde_json::to_string(&json).map_or_else(
            |_| "<openai frame>".to_string(),
            |s| format!("data: {s}\n\n"),
        ))
    }

    /// OpenAI Responses 帧：跟踪交付事件；response.completed 无内容 → 换 error。
    fn note_responses_frame(&mut self, json: &Value) -> Option<String> {
        let ty = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "response.output_text.delta" | "response.function_call_arguments.delta" => {
                self.saw_content |= json
                    .get("delta")
                    .and_then(|d| d.as_str())
                    .is_some_and(|s| !s.is_empty());
            }
            "response.output_item.added" => {
                if json.pointer("/item/type").and_then(|t| t.as_str()) != Some("reasoning") {
                    self.saw_content = true;
                }
            }
            "response.completed" => {
                if let Some(status) =
                    json.pointer("/response/status").and_then(|s| s.as_str())
                {
                    self.stream_finish_reason = Some(status.to_string());
                }
                if !self.saw_content {
                    return Some(self.convert_to_error());
                }
            }
            _ => {}
        }
        Some(serde_json::to_string(&json).map_or_else(
            |_| "<responses frame>".to_string(),
            |s| format!("data: {s}\n\n"),
        ))
    }

    /// 过滤后无内容的终止：换 error 帧（按客户端 wire 分派形状），保留上游终止原因。
    fn convert_to_error(&mut self) -> String {
        self.error_emitted = true;
        wire_error_frame(
            &self.protocol,
            "api_error",
            &thinking_only_msg(self.stream_finish_reason.as_deref()),
        )
    }

    /// Anthropic wire 帧：丢 thinking 块的 start/delta/stop，其余帧的 index 重编号。
    /// 过滤后无任何可下发内容（thinking-only 上游）时，把 message_stop 换成 error 帧
    /// 报错（保留上游 stop_reason），不发给客户端一个空 content 的「成功」流。
    fn filter_anthropic_frame(&mut self, prefix: &str, mut json: Value) -> Option<String> {
        let ty = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let idx = json.get("index").and_then(|v| v.as_u64()).map(|v| v as u32);
        match ty {
            "content_block_start" => {
                // 缺 index 的畸形帧原样过（`?` 静默丢帧会让客户端装配留洞）
                let Some(i) = idx else {
                    return Some(format!("{prefix}data: {json}\n\n"));
                };
                let is_thinking = json.get("content_block").is_some_and(is_thinking_block);
                if is_thinking {
                    self.dropped.insert(i);
                    return None;
                }
                // 空 text block 本身不构成可下发正文；tool_use 与未知非思考块仍算内容。
                self.saw_content |= json
                    .get("content_block")
                    .and_then(|b| b.get("type"))
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| t != "text");
                let new_i = self.next_out_index;
                self.next_out_index += 1;
                self.remap.insert(i, new_i);
                set_index(&mut json, new_i);
            }
            "message_delta" => {
                if let Some(r) = json.pointer("/delta/stop_reason").and_then(|v| v.as_str()) {
                    self.stop_reason = Some(r.to_string());
                }
            }
            "message_stop" if !self.saw_content => {
                self.error_emitted = true;
                return Some(thinking_only_error_frame(self.stop_reason.as_deref()));
            }
            "content_block_delta" | "content_block_stop" => {
                let Some(i) = idx else {
                    return Some(format!("{prefix}data: {json}\n\n"));
                };
                if self.dropped.contains(&i) {
                    return None;
                }
                if ty == "content_block_delta"
                    && json
                        .pointer("/delta/type")
                        .and_then(|t| t.as_str())
                        .is_some_and(|t| t == "text_delta" || t == "input_json_delta")
                    && json
                        .pointer("/delta/text")
                        .and_then(|t| t.as_str())
                        .is_some_and(|t| !t.is_empty())
                {
                    self.saw_content = true;
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

/// thinking-only 上游响应被剥空后的统一错误文案（非流式 502 体与流式 error 帧共用）。
fn thinking_only_msg(stop_reason: Option<&str>) -> String {
    format!(
        "upstream returned thinking-only response (disable_thinking=true), nothing deliverable after filter; upstream stop_reason: {}",
        stop_reason.unwrap_or("none")
    )
}

/// thinking-only 上游响应被剥空后的终止错误帧（Anthropic Messages SSE 规范的终端
/// `event: error` 事件）。保留上游 stop_reason；不声称额度耗尽，不泄漏任何思考文本。
fn thinking_only_error_frame(stop_reason: Option<&str>) -> String {
    wire_error_frame(&Protocol::Anthropic, "api_error", &thinking_only_msg(stop_reason))
}

/// 按客户端 wire 协议合成 SSE 错误帧（通用形状；`anthropic_error_type` 仅 anthropic 臂用）。
/// openai 系错误 chunk 自带 `[DONE]` 终止哨兵。
pub fn wire_error_frame(
    protocol: &Protocol,
    anthropic_error_type: &str,
    message: &str,
) -> String {
    let body = wire_error_body(protocol, anthropic_error_type, message);
    match protocol {
        Protocol::OpenAI | Protocol::OpenAIResponses | Protocol::OpenAICompletions => {
            format!("data: {body}\n\ndata: [DONE]\n\n")
        }
        Protocol::Gemini => format!("data: {body}\n\n"),
        _ => format!("event: error\ndata: {body}\n\n"),
    }
}

/// 按客户端 wire 协议的非流式 JSON 错误体（SSE 错误帧 [`wire_error_frame`] 与之同形状）。
pub fn wire_error_body(
    protocol: &Protocol,
    anthropic_error_type: &str,
    message: &str,
) -> Value {
    match protocol {
        Protocol::OpenAI | Protocol::OpenAIResponses | Protocol::OpenAICompletions => {
            serde_json::json!({
                "error": { "message": message, "type": "server_error", "code": 502 }
            })
        }
        Protocol::Gemini => {
            serde_json::json!({
                "error": { "code": 502, "message": message, "status": "UNAVAILABLE" }
            })
        }
        _ => {
            serde_json::json!({
                "type": "error",
                "error": { "type": anthropic_error_type, "message": message }
            })
        }
    }
}

/// OpenAI chat 流帧：剔 `delta.reasoning_content` / `delta.reasoning`；delta 因此变空则整帧丢弃。
/// 返回是否保留该帧。
fn strip_openai_stream_delta(json: &mut Value) -> bool {
    let Some(choices) = json.get_mut("choices").and_then(|v| v.as_array_mut()) else {
        return true;
    };
    let mut any_content = false;
    for c in choices.iter_mut() {
        if let Some(delta) = c.get_mut("delta").and_then(|v| v.as_object_mut()) {
            delta.remove("reasoning_content");
            delta.remove("reasoning");
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
        && !(ty == "response.output_item.added"
            && json.pointer("/item/type").and_then(|t| t.as_str()) == Some("reasoning"))
}

#[cfg(test)]
#[path = "test_thinking_strip.rs"]
mod test_thinking_strip;
