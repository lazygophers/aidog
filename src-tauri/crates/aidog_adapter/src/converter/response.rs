//! 上游响应解析与转换：SSE 解析 / 非流式响应转换 / 按客户端协议渲染 SSE。

use aidog_db::models::Protocol;
use serde_json::Value;

use super::super::types::*;

/// 将目标协议的 SSE event data 解析为统一的 ChatStreamEvent。
/// SSE 响应格式由 wire protocol（endpoint 协议）决定。
pub fn parse_sse(data: &Value, wire_protocol: &Protocol) -> Option<ChatStreamEvent> {
    match wire_protocol {
        Protocol::Anthropic => super::super::anthropic::parse_anthropic_sse(data),
        Protocol::Gemini => super::super::gemini::parse_gemini_sse(data),
        // 所有 OpenAI 系列共用 OpenAI SSE 解析
        _ => super::super::openai::parse_openai_sse(data),
    }
}

/// 从上游原始响应文本块中按 wire 协议分帧提取 ChatStreamEvent（上游帧格式知识收敛于此，
/// 不下沉到 proxy 层）。三协议共用同一 SSE 分帧规则：`data: ` 前缀 + 空行分隔（实测确认，
/// 见 gemini.rs::to_gemini_sse 注释）。协议差异仅在有无显式终止哨兵：OpenAI 系发送
/// `data: [DONE]`；Anthropic / Gemini 无哨兵，流关闭即结束——`[DONE]` 字面量对它们不会出现，
/// 故检测不必按协议门控，统一映射为 Stop 事件即可（无害 no-op）。
pub fn parse_upstream_sse(text: &str, wire_protocol: &Protocol) -> Vec<ChatStreamEvent> {
    let mut events = Vec::new();
    for line in text.lines() {
        let Some(data) = line.strip_prefix("data: ") else { continue };
        if data.trim() == "[DONE]" {
            events.push(ChatStreamEvent::Stop { finish_reason: Some("end_turn".to_string()) });
            continue;
        }
        if let Ok(json) = serde_json::from_str::<Value>(data)
            && let Some(event) = parse_sse(&json, wire_protocol) {
                events.push(event);
            }
    }
    events
}

/// 非流式响应内部归一表示（基于 Anthropic 语义：text + tool_use 块 + stop_reason + usage）。
///
/// 上游响应（openai chat completion / anthropic messages / …）先 parse 为本结构，
/// 再按客户端协议 render，避免把上游原生格式直接透回致客户端解析失败。
pub struct NonStreamResponse {
    pub id: String,
    pub model: String,
    /// 文本段（按出现顺序，通常单段）
    pub text: Option<String>,
    /// 工具调用块：(id, name, input)
    pub tool_uses: Vec<(String, String, Value)>,
    /// 统一 stop_reason（anthropic 语义：end_turn / tool_use / max_tokens / stop_sequence）
    pub stop_reason: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    /// 思维链文本（按 wire 协议语义提取，anthropic thinking 块 / openai reasoning_content 等）
    pub reasoning: Option<String>,
}

/// 非流式上游响应 → 客户端协议响应体转换。
///
/// - `wire_protocol`: 上游响应体格式（endpoint 协议）。
/// - `client_protocol`: 客户端期望协议（入站 source_protocol）。
///
/// 返回 `Some(Value)` 表示已转换为客户端格式；`None` 表示无需 / 无法转换
/// （调用方应原样透传上游 body，保持既有行为）。
///
/// 转换路径：parse_<wire>(body) → NonStreamResponse → render_<client>(parsed) → Value
/// wire == client 时透传；缺实现时回退透传（向后兼容）。
pub fn convert_response(
    body: &Value,
    wire_protocol: &Protocol,
    client_protocol: &Protocol,
    model: &str,
) -> Option<Value> {
    // 同 wire family（wire == client 语义协议）→ 跳过 parse/render（避免不必要转换）
    if wire_protocol.same_wire_family(client_protocol) {
        return None;
    }

    // parse 阶段：wire_protocol → NonStreamResponse
    let parsed = match wire_protocol {
        Protocol::Anthropic => super::super::anthropic::parse_anthropic_response(body, model),
        Protocol::OpenAI => super::super::openai::parse_openai_response(body, model),
        Protocol::OpenAIResponses => super::super::openai_responses::parse_responses_response(body, model),
        Protocol::OpenAICompletions => super::super::openai_completions::parse_completions_response(body, model),
        Protocol::Gemini => super::super::gemini::parse_gemini_response(body, model),
        _ => None, // 非目标协议回退透传
    };

    let parsed = parsed?;

    // render 阶段：NonStreamResponse → client_protocol
    match client_protocol {
        Protocol::Anthropic => Some(render_anthropic_response(&parsed)),
        Protocol::OpenAI => Some(super::super::openai::render_openai_response(&parsed)?),
        Protocol::OpenAIResponses => Some(super::super::openai_responses::render_responses_response(&parsed)?),
        Protocol::OpenAICompletions => Some(super::super::openai_completions::render_completions_response(&parsed)?),
        Protocol::Gemini => Some(super::super::gemini::render_gemini_response(&parsed)?),
        _ => None, // 未知客户端协议回退透传
    }
}

/// 渲染归一响应为 Anthropic Messages 非流式响应体。
pub fn render_anthropic_response(r: &NonStreamResponse) -> Value {
    let mut content: Vec<Value> = Vec::new();
    // reasoning 排首位（方案 B：禁 thinking 块避 signature 风险）
    if let Some(reasoning) = &r.reasoning
        && !reasoning.is_empty() {
            content.push(serde_json::json!({ "type": "text", "text": reasoning }));
        }
    if let Some(text) = &r.text
        && !text.is_empty() {
            content.push(serde_json::json!({ "type": "text", "text": text }));
        }
    for (id, name, input) in &r.tool_uses {
        content.push(serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }));
    }
    // 兜底：既无 text 也无 tool_use（异常上游）→ 空 text 块，保证 content 非空数组（Anthropic 合法）。
    if content.is_empty() {
        content.push(serde_json::json!({ "type": "text", "text": "" }));
    }
    serde_json::json!({
        "id": r.id,
        "type": "message",
        "role": "assistant",
        "model": r.model,
        "content": content,
        "stop_reason": r.stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": r.input_tokens,
            "output_tokens": r.output_tokens,
            "cache_read_input_tokens": r.cache_read_tokens,
        }
    })
}

/// 将统一的 ChatStreamEvent 按客户端协议格式化为 SSE。
///
/// 穷尽 match（无 `_ =>` 兜底）：非 wire 协议的平台变体在此仍统一走 Anthropic 格式
/// （历史行为不变，见旧版 `_ =>` 分支），但显式列出而非通配，Protocol 新增 wire 变体时
/// 编译器强制在此补处理，不会静默落入错误分支。
/// 带状态的客户端 SSE 渲染：Anthropic 系客户端走 [`AnthropicSseState`] 状态机
/// （block index 分配 / content_block_start·stop / thinking block 完整序列），
/// OpenAI / Gemini 出站无状态，与 [`to_client_sse`] 一致。
pub fn to_client_sse_stateful(
    event: &ChatStreamEvent,
    state: &mut AnthropicSseState,
    source_protocol: &Protocol,
    _model: &str,
) -> Option<String> {
    use Protocol::*;
    match source_protocol {
        OpenAI | OpenAIResponses | OpenAICompletions => super::super::openai::to_openai_sse(event, _model),
        Gemini => super::super::gemini::to_gemini_sse(event, _model),
        _ => state.push(event),
    }
}

pub fn to_client_sse(event: &ChatStreamEvent, source_protocol: &Protocol, model: &str) -> Option<String> {
    use Protocol::*;
    match source_protocol {
        OpenAI | OpenAIResponses | OpenAICompletions => super::super::openai::to_openai_sse(event, model),
        Gemini => super::super::gemini::to_gemini_sse(event, model),
        Anthropic
        | Mock
        | ClaudeCode
        | Glm
        | GlmCoding
        | GlmEn
        | Kimi
        | KimiCoding
        | MiniMax
        | MiniMaxEn
        | Codex
        | Bailian
        | BailianCoding
        | DeepSeek
        | StepFun
        | StepFunEn
        | Doubao
        | BytePlus
        | QianFan
        | QianfanCoding
        | XiaomiMimo
        | XiaomiMimoCoding
        | BaiLing
        | Longcat
        | SenseNova
        | OpenRouter
        | SiliconFlow
        | SiliconFlowEn
        | AiHubMix
        | DmxApi
        | ModelScope
        | ShengSuanYun
        | AtlasCloud
        | Novita
        | TheRouter
        | CherryIn
        | PackyCode
        | Cubence
        | AiGoCode
        | RightCode
        | AiCodeMirror
        | Nvidia
        | Pateway
        | CcSub
        | ApiKeyFun
        | ApiNebula
        | SudoCode
        | ClaudeApi
        | ClaudeCN
        | RunApi
        | RelaxyCode
        | CrazyRouter
        | SssAiCode
        | Compshare
        | CompshareCoding
        | Micu
        | CTok
        | EFlowCode
        | LemonData
        | PipeLlm
        | OpenCode
        | OpenCodeZen
        | NewApi
        | CliProxy
        | Devin => to_anthropic_sse(event),
    }
}

/// 将统一的 ChatStreamEvent 转为 Anthropic SSE 格式（用于返回给 Claude Code 客户端）
pub fn to_anthropic_sse(event: &ChatStreamEvent) -> Option<String> {
    match event {
        ChatStreamEvent::Start { id, model } => Some(format!(
            "event: message_start\ndata: {}\n\n",
            serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": id,
                    "type": "message",
                    "role": "assistant",
                    "model": model,
                    "content": [],
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": { "input_tokens": 0, "output_tokens": 0 }
                }
            })
        )),
        ChatStreamEvent::Delta { text } => Some(format!(
            "event: content_block_delta\ndata: {}\n\n",
            serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {
                    "type": "text_delta",
                    "text": text
                }
            })
        )),
        ChatStreamEvent::ReasoningDelta { text } => Some(format!(
            "event: content_block_delta\ndata: {}\n\n",
            serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {
                    "type": "text_delta",
                    "text": text
                }
            })
        )),
        ChatStreamEvent::ToolDelta { index, id, name, input } => {
            let mut parts = Vec::new();

            // tool_use 开始
            if let (Some(id), Some(name)) = (id, name) {
                parts.push(format!(
                    "event: content_block_start\ndata: {}\n\n",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index,
                        "content_block": {
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": {}
                        }
                    })
                ));
            }

            // tool input delta
            if let Some(input) = input {
                parts.push(format!(
                    "event: content_block_delta\ndata: {}\n\n",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": input
                        }
                    })
                ));
            }

            if parts.is_empty() {
                None
            } else {
                Some(parts.join(""))
            }
        }
        ChatStreamEvent::Stop { finish_reason } => Some(format!(
            "event: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            serde_json::json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": finish_reason.as_deref().unwrap_or("end_turn"),
                    "stop_sequence": null
                },
                "usage": { "output_tokens": 0 }
            })
        )),
        ChatStreamEvent::Usage { .. } => None,
    }
}

#[cfg(test)]
#[path = "test_response.rs"]
mod test_response;

/// Anthropic SSE 出站状态机：跨 chunk 维护 block index 分配与开块表。
///
/// 中立事件流的 tool index 是「第几个工具」；Anthropic wire 的 content block index
/// 是消息内全局序（text/thinking/tool 混排连续编号）。本结构把中立 index 映射到
/// wire index，并在首个块事件时发 content_block_start、Stop 前统一补 content_block_stop。
#[derive(Default)]
pub struct AnthropicSseState {
    /// 下一个可用 wire block index
    next_index: u32,
    /// text 块是否已开
    text_open: bool,
    /// thinking 块是否已开 + 其 wire index
    thinking_index: Option<u32>,
    /// thinking 首帧标记（首个 thinking_delta 前须发 content_block_start）
    thinking_just_opened: Option<bool>,
    /// 中立 tool index → wire block index（已开块）
    tool_wire: std::collections::HashMap<u32, u32>,
}

impl AnthropicSseState {
    /// 处理一个中立事件，产出 0..n 个 wire 帧（拼好的 SSE 文本，含 event: 行）。
    pub fn push(&mut self, event: &ChatStreamEvent) -> Option<String> {
        match event {
            ChatStreamEvent::Start { id, model } => Some(format!(
                "event: message_start\ndata: {}\n\n",
                serde_json::json!({
                    "type": "message_start",
                    "message": {
                        "id": id, "type": "message", "role": "assistant", "model": model,
                        "content": [], "stop_reason": null, "stop_sequence": null,
                        "usage": { "input_tokens": 0, "output_tokens": 0 }
                    }
                })
            )),
            ChatStreamEvent::Delta { text } => {
                let mut parts = Vec::new();
                if !self.text_open {
                    self.text_open = true;
                    parts.push(format!(
                        "event: content_block_start\ndata: {}\n\n",
                        serde_json::json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })
                    ));
                }
                parts.push(format!(
                    "event: content_block_delta\ndata: {}\n\n",
                    serde_json::json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": text } })
                ));
                Some(parts.join(""))
            }
            ChatStreamEvent::ReasoningDelta { text } => {
                let idx = match self.thinking_index {
                    Some(i) => i,
                    None => {
                        let i = self.alloc_block();
                        self.thinking_index = Some(i);
                        self.thinking_just_opened = Some(true);
                        i
                    }
                };
                let mut parts = Vec::new();
                if self.thinking_just_opened.take().is_some() {
                    parts.push(format!(
                        "event: content_block_start\ndata: {}\n\n",
                        serde_json::json!({ "type": "content_block_start", "index": idx, "content_block": { "type": "thinking", "thinking": "" } })
                    ));
                }
                parts.push(format!(
                    "event: content_block_delta\ndata: {}\n\n",
                    serde_json::json!({ "type": "content_block_delta", "index": idx, "delta": { "type": "thinking_delta", "thinking": text } })
                ));
                Some(parts.join(""))
            }
            ChatStreamEvent::ToolDelta { index, id, name, input } => {
                let mut parts = Vec::new();
                // 首帧（带 id/name）→ wire index 分配 + content_block_start
                if !self.tool_wire.contains_key(index)
                    && let (Some(id), Some(name)) = (id, name) {
                        // text 块后续不再有；开 tool 块前不必关 text（Anthropic 允许交错块？
                        // 实际须按序——文本块先 close。简化：首个 tool 块出现时 close text 块。）
                        let wire = self.alloc_block();
                        self.tool_wire.insert(*index, wire);
                        parts.push(format!(
                            "event: content_block_start\ndata: {}\n\n",
                            serde_json::json!({ "type": "content_block_start", "index": wire, "content_block": { "type": "tool_use", "id": id, "name": name, "input": {} } })
                        ));
                    }
                if let Some(input) = input {
                    let wire = self.tool_wire.get(index).copied().unwrap_or(*index);
                    parts.push(format!(
                        "event: content_block_delta\ndata: {}\n\n",
                        serde_json::json!({ "type": "content_block_delta", "index": wire, "delta": { "type": "input_json_delta", "partial_json": input } })
                    ));
                }
                if parts.is_empty() { None } else { Some(parts.join("")) }
            }
            ChatStreamEvent::Stop { finish_reason } => {
                let mut parts = Vec::new();
                // 关全部开块（text / thinking / tool × n），wire index 升序
                let mut closes: Vec<u32> = Vec::new();
                if self.text_open { closes.push(0); }
                if let Some(i) = self.thinking_index { closes.push(i); }
                closes.extend(self.tool_wire.values().copied());
                closes.sort();
                for c in closes {
                    parts.push(format!(
                        "event: content_block_stop\ndata: {}\n\n",
                        serde_json::json!({ "type": "content_block_stop", "index": c })
                    ));
                }
                parts.push(format!(
                    "event: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
                    serde_json::json!({
                        "type": "message_delta",
                        "delta": { "stop_reason": finish_reason.as_deref().unwrap_or("end_turn"), "stop_sequence": null },
                        "usage": { "output_tokens": 0 }
                    })
                ));
                Some(parts.join(""))
            }
            ChatStreamEvent::Usage { .. } => None,
        }
    }

    fn alloc_block(&mut self) -> u32 {
        // text 块恒占 wire 0；已开 text 时后续块从 1 起编
        if self.text_open && self.next_index == 0 {
            self.next_index = 1;
        }
        let i = self.next_index;
        self.next_index += 1;
        i
    }
}
