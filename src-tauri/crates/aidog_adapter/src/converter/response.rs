//! 上游响应解析与转换：SSE 解析 / 非流式响应转换 / 按客户端协议渲染 SSE。

use aidog_db::models::Protocol;
use serde_json::Value;

use super::super::types::*;

/// 将目标协议的 SSE event data 解析为统一的 ChatStreamEvent。
/// SSE 响应格式由 wire protocol（endpoint 协议）决定。
/// 从上游响应 JSON 提取服务端实际使用的模型名。
///
/// 不同协议把模型放在不同字段（非流式顶层 `model`；OpenAI Responses 嵌在 `response.model`；
/// Anthropic 流式 message_start 嵌在 `message.model`；Gemini 用 `modelVersion`）；
/// 空字符串和非字符串都视为缺失，调用方负责回退请求模型。
pub fn response_model(body: &Value) -> Option<String> {
    [
        body.get("model"),
        body.get("response").and_then(|response| response.get("model")),
        body.get("message").and_then(|message| message.get("model")),
        body.get("modelVersion"),
    ]
    .into_iter()
    .flatten()
    .find_map(|value| {
        value
            .as_str()
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(str::to_owned)
    })
}

/// 从上游响应 JSON 提取模型名；缺失时回退到调用方提供的模型名。
pub fn response_model_or(body: &Value, fallback_model: &str) -> String {
    response_model(body).unwrap_or_else(|| fallback_model.to_string())
}

/// 上游单帧 `data:` JSON → 中立事件流（0..n 个）。
///
/// issue 71：帧内携带 usage 时（anthropic message_start/message_delta、openai 末块、
/// gemini usageMetadata、responses response.completed、completions 末块）先产出
/// `ChatStreamEvent::Usage`，再产出该帧的原有事件（Stop 等）——渲染侧靠 Usage 先于 Stop
/// 把 usage 合并进终止帧。官方 openai 的 usage 与 finish 分帧发送（finish 块先到），
/// 此时 Usage 晚于 Stop 单独产出，渲染状态机兜住两种顺序。
pub fn parse_sse(data: &Value, wire_protocol: &Protocol) -> Vec<ChatStreamEvent> {
    let mut events = Vec::new();
    if let Some(usage) = wire_usage(data, wire_protocol) {
        events.push(ChatStreamEvent::Usage { usage });
    }
    if let Some(event) = parse_sse_single(data, wire_protocol) {
        events.push(event);
    }
    events
}

/// 解析上游帧内 usage 为中立 [`Usage`]（issue 71）。None = 该帧 / 该协议不携带。
fn wire_usage(data: &Value, wire_protocol: &Protocol) -> Option<Usage> {
    let (prompt, completion, total, cache_read, cache_write) = match wire_protocol {
        // anthropic：message_start 在 message.usage 带输入与缓存；message_delta 在
        // usage 只带输出（部分帧），由 Usage::merge_partial 跨帧合并。
        Protocol::Anthropic => {
            let usage = data
                .get("usage")
                .or_else(|| data.get("message").and_then(|m| m.get("usage")))?;
            (
                usage.get("input_tokens"),
                usage.get("output_tokens"),
                None,
                usage.get("cache_read_input_tokens"),
                usage.get("cache_creation_input_tokens"),
            )
        }
        // gemini：promptTokenCount 原样保留（含缓存），输出 = candidates + thoughts（与
        // 非流式 parse_gemini_usage_metadata 同口径）；缓存读 = cachedContentTokenCount。
        Protocol::Gemini => {
            let usage = data.get("usageMetadata")?;
            let completion = usage
                .get("candidatesTokenCount")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_add(
                    usage
                        .get("thoughtsTokenCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                );
            return Some(Usage {
                prompt_tokens: usage.get("promptTokenCount").and_then(Value::as_u64).map(|v| v as u32),
                completion_tokens: Some(completion as u32),
                total_tokens: usage.get("totalTokenCount").and_then(Value::as_u64).map(|v| v as u32),
                cache_read_tokens: usage.get("cachedContentTokenCount").and_then(Value::as_u64).map(|v| v as u32),
                cache_write_tokens: None,
            });
        }
        // responses：usage 嵌在 response 对象内。
        Protocol::OpenAIResponses => {
            let usage = data.get("response")?.get("usage")?;
            (
                usage.get("input_tokens"),
                usage.get("output_tokens"),
                usage.get("total_tokens"),
                usage
                    .get("input_tokens_details")
                    .and_then(|d| d.get("cached_tokens")),
                None,
            )
        }
        // openai chat 与 legacy completions：顶层 usage。
        _ => {
            let usage = data.get("usage")?;
            (
                usage.get("prompt_tokens"),
                usage.get("completion_tokens"),
                usage.get("total_tokens"),
                usage
                    .get("prompt_tokens_details")
                    .and_then(|d| d.get("cached_tokens")),
                None,
            )
        }
    };
    let num = |v: Option<&Value>| v.and_then(Value::as_u64).map(|n| n as u32);
    Some(Usage {
        prompt_tokens: num(prompt),
        completion_tokens: num(completion),
        total_tokens: num(total),
        cache_read_tokens: num(cache_read),
        cache_write_tokens: num(cache_write),
    })
}

/// 上游单帧原有事件解析（issue 71 前的 parse_sse 行为）。
fn parse_sse_single(data: &Value, wire_protocol: &Protocol) -> Option<ChatStreamEvent> {
    match wire_protocol {
        Protocol::Anthropic => super::super::anthropic::parse_anthropic_sse(data),
        Protocol::Gemini => super::super::gemini::parse_gemini_sse(data),
        Protocol::OpenAIResponses => super::super::openai_responses::parse_responses_sse(data),
        Protocol::OpenAICompletions => {
            super::super::openai_completions::parse_completions_sse(data)
        }
        // 剩余 OpenAI 系（chat completions 格式及其平台变体）共用 OpenAI SSE 解析
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
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        if data.trim() == "[DONE]" {
            events.push(ChatStreamEvent::Stop {
                finish_reason: Some("end_turn".to_string()),
            });
            continue;
        }
        if let Ok(json) = serde_json::from_str::<Value>(data) {
            events.extend(parse_sse(&json, wire_protocol));
        }
    }
    hoist_usage_before_stop(events)
}

/// 把晚于 Stop 的 Usage 事件上提到最近的 Stop 之前（issue 71）。openai 官方分帧
/// finish 块 → usage 块，解析出的事件序为 Stop, Usage；各客户端渲染状态机按
/// 「Usage 先于 Stop → 合并进终止帧」处理，故渲染前统一上提。
pub fn hoist_usage_before_stop(events: Vec<ChatStreamEvent>) -> Vec<ChatStreamEvent> {
    let mut out: Vec<ChatStreamEvent> = Vec::with_capacity(events.len());
    for e in events {
        if let ChatStreamEvent::Usage { .. } = e
            && let Some(pos) = out.iter().rposition(|o| matches!(o, ChatStreamEvent::Stop { .. }))
        {
            out.insert(pos, e);
        } else {
            out.push(e);
        }
    }
    out
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
    // 只有响应形态完全相同才跳过转换。
    if wire_protocol == client_protocol {
        return None;
    }

    // parse 阶段：wire_protocol → NonStreamResponse
    let parsed = match wire_protocol {
        Protocol::Anthropic => super::super::anthropic::parse_anthropic_response(body, model),
        Protocol::OpenAI => super::super::openai::parse_openai_response(body, model),
        Protocol::OpenAIResponses => {
            super::super::openai_responses::parse_responses_response(body, model)
        }
        Protocol::OpenAICompletions => {
            super::super::openai_completions::parse_completions_response(body, model)
        }
        Protocol::Gemini => super::super::gemini::parse_gemini_response(body, model),
        _ => None, // 非目标协议回退透传
    };

    let parsed = normalize_inline_reasoning(parsed?);

    // render 阶段：NonStreamResponse → client_protocol
    match client_protocol {
        Protocol::Anthropic => Some(render_anthropic_response(&parsed)),
        Protocol::OpenAI => Some(super::super::openai::render_openai_response(&parsed)?),
        Protocol::OpenAIResponses => Some(
            super::super::openai_responses::render_responses_response(&parsed)?,
        ),
        Protocol::OpenAICompletions => {
            Some(super::super::openai_completions::render_completions_response(&parsed)?)
        }
        Protocol::Gemini => Some(super::super::gemini::render_gemini_response(&parsed)?),
        _ => None, // 未知客户端协议回退透传
    }
}

/// 把上游写在正文里的行内思维链标签（`<thinking>` / `<think>`）归一到 `reasoning` 字段。
///
/// 不识别这类标签时，标签连同思考内容会作为普通 text 块下发，Claude Code 按正文渲染
/// → 用户看到裸 `<thinking>…</thinking>`。识别后与结构化思维链通道
/// （`reasoning_content` / thinking 块 / Gemini `thought`）汇到同一字段，
/// 由各协议 render 按各自标准语义写出（Anthropic → thinking 块）。
///
/// 上游同时给了结构化 reasoning 与行内标签时，结构化的排前，行内的追加在后（出现顺序）。
fn normalize_inline_reasoning(mut r: NonStreamResponse) -> NonStreamResponse {
    let Some(text) = r.text.as_deref() else {
        return r;
    };
    if !super::super::reasoning_tags::has_inline_reasoning_tag(text) {
        return r;
    }
    let (inline_reasoning, clean_text) = super::super::reasoning_tags::split_inline_reasoning(text);
    r.text = Some(clean_text);
    if inline_reasoning.is_empty() {
        return r;
    }
    r.reasoning = Some(match r.reasoning.take() {
        Some(existing) if !existing.is_empty() => format!("{existing}\n\n{inline_reasoning}"),
        _ => inline_reasoning,
    });
    r
}

/// 渲染归一响应为 Anthropic Messages 非流式响应体。
///
/// 思维链走标准 `thinking` 块（与流式 `thinking_delta` 同语义），不降级成 text 块：
/// 降级会让客户端把思考当正式回答回灌历史（实测 comet gpt-5.5 reasoning-only 自锁循环），
/// 也让上游写在正文里的 `<thinking>` 标签原样透到界面上。
/// 不带 `signature`：签名是 Anthropic 官方模型对自己思考内容的签发凭据，
/// 非 Anthropic 上游的思维链没有、也不能伪造（伪造签名会被真 Anthropic 上游判 400）。
pub fn render_anthropic_response(r: &NonStreamResponse) -> Value {
    let mut content: Vec<Value> = Vec::new();
    // thinking 块排首位（Anthropic 规定 thinking 必须在同一条消息的正文块之前）
    if let Some(reasoning) = &r.reasoning
        && !reasoning.is_empty()
    {
        content.push(serde_json::json!({ "type": "thinking", "thinking": reasoning }));
    }
    if let Some(text) = &r.text
        && !text.is_empty()
    {
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
/// Responses 客户端走 [`ResponsesSseState`]（issue 70），OpenAI / Completions / Gemini
/// 走各自轻量状态机（issue 71：usage 累积与 [DONE]/终止帧时序）。
///
/// `include_usage` 只对 OpenAI Chat / legacy Completions 客户端生效（官方契约：usage 块
/// 仅当请求带 `stream_options.include_usage` 时下发）；Anthropic / Gemini / Responses
/// 客户端恒回传 usage，该参数忽略。
pub fn to_client_sse_stateful(
    event: &ChatStreamEvent,
    state: &mut ClientSseState,
    source_protocol: &Protocol,
    model: &str,
    include_usage: bool,
) -> Option<String> {
    use Protocol::*;
    match source_protocol {
        OpenAI => state.as_openai(include_usage).push(event, model),
        OpenAIResponses => state.as_responses().push(event, model),
        OpenAICompletions => state.as_completions(include_usage).push(event, model),
        Gemini => state.as_gemini().push(event, model),
        _ => state.as_anthropic().push(event),
    }
}

pub fn to_client_sse(
    event: &ChatStreamEvent,
    source_protocol: &Protocol,
    model: &str,
) -> Option<String> {
    use Protocol::*;
    match source_protocol {
        OpenAI => super::super::openai::to_openai_sse(event, model),
        OpenAIResponses => {
            // Responses 是有状态协议（response.created / sequence_number / item 生命周期），
            // 多事件流必须走 to_client_sse_stateful + ClientSseState；无状态入口只服务
            // 单发终态帧（错误帧路径的 Stop）：输出 created → completed 自包含收尾序列。
            // 非终态事件经此入口会重复 created / 重置 sequence_number / item 永不闭合 → None。
            match event {
                ChatStreamEvent::Stop { .. } => {
                    let mut st = super::super::openai_responses::ResponsesSseState::default();
                    st.push(event, model)
                }
                _ => None,
            }
        }
        OpenAICompletions => super::super::openai_completions::to_completions_sse(event, model),
        Gemini => super::super::gemini::to_gemini_sse(event, model),
        Anthropic | Mock | ClaudeCode | Glm | GlmCoding | GlmEn | GlmCodingEn | Kimi | KimiEn
        | KimiCoding | InternInkstone | MiniMax | MiniMaxEn | MinimaxCoding | Codex | Cline | ClinePass | Bailian | BailianCoding
        | BailianEn | BailianCodingEn | DeepSeek | StepFun | StepFunEn | TencentTokenhub | TencentTokenhubIntl | TencentCoding | Doubao | BytePlus
        | QianFan | QianfanCoding | XiaomiMimo | XiaomiMimoCoding | XiaomiMimoCodingEn
        | Longcat | SenseNova | SenseNovaEn | OpenRouter | SiliconFlow | SiliconFlowEn
        | AiHubMix | DmxApi | ModelScope | ShengSuanYun | AtlasCloud | Novita | TheRouter
        | CherryIn | PackyCode | Cubence | AiGoCode | RightCode | AiCodeMirror | Nvidia
        | Pateway | CcSub | ApiKeyFun | SudoCode | ClaudeApi | ClaudeCN | RunApi | RelaxyCode
        | CrazyRouter | SssAiCode | Compshare | CompshareCoding | Micu | CTok | EFlowCode
        | LemonData | PipeLlm | OpenCode | OpenCodeZen | TokenRhythm | NewApi | Devin
        | TypeSafe | CommandCode | LiteLLM | Meta | Mistral | XAI | Magpie
        // Passthrough 非 wire 协议（透传平台在 handler 拦截，不经 converter）；编译完备性挂 anthropic 渲染
        // Unknown 同理（票 06：未知协议平台被 router 排除，正常路径不经 converter）
        | Passthrough | Unknown => to_anthropic_sse(event),

    }
}

/// 上游 finish_reason → Anthropic `stop_reason` 词表。
///
/// 中立事件 `ChatStreamEvent::Stop` 携带的是上游原词（openai `stop`/`tool_calls`/`length`，
/// gemini 小写后的 `stop`/`max_tokens`），Anthropic wire 只认 end_turn / tool_use /
/// max_tokens / stop_sequence。严格校验该枚举的客户端（pi 会抛
/// `Unhandled stop reason: stop`）要求出站前翻译。与非流式路径
/// `protocols/openai/response.rs` 的映射同表。
fn anthropic_stop_reason(finish_reason: Option<&str>) -> &'static str {
    match finish_reason {
        Some("tool_calls") | Some("tool_use") => "tool_use",
        Some("length") | Some("max_tokens") => "max_tokens",
        Some("stop_sequence") => "stop_sequence",
        _ => "end_turn",
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
        ChatStreamEvent::ToolDelta {
            index,
            id,
            name,
            input,
        } => {
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
                    "stop_reason": anthropic_stop_reason(finish_reason.as_deref()),
                    "stop_sequence": null
                },
                "usage": { "output_tokens": 0 }
            })
        )),
        ChatStreamEvent::Usage { .. } => None,
        // 上游的心跳原样转给 Anthropic 系客户端：它按字节计数，静默 300 秒就掐断整条流。
        ChatStreamEvent::Ping => Some("event: ping\ndata: {\"type\":\"ping\"}\n\n".to_string()),
    }
}

/// 流式：把一个中立事件里写在正文中的行内思维链标签分流为 `ReasoningDelta`。
///
/// 与非流式 [`normalize_inline_reasoning`] 同一判定，只是按增量做：`Delta` 文本经
/// [`InlineReasoningSplitter`] 拆成正文段 / 思维链段，分别产出 `Delta` / `ReasoningDelta`；
/// `Stop` 前先冲刷跨 chunk 残留（未闭合 `<thinking>` 按思维链收尾），其余事件原样透过。
///
/// 返回 0..n 个事件，调用方按序渲染。
pub fn split_stream_inline_reasoning(
    event: ChatStreamEvent,
    splitter: &mut super::super::reasoning_tags::InlineReasoningSplitter,
) -> Vec<ChatStreamEvent> {
    use super::super::reasoning_tags::Segment;
    let to_events = |segs: Vec<Segment>| -> Vec<ChatStreamEvent> {
        segs.into_iter()
            .map(|s| match s {
                Segment::Text(text) => ChatStreamEvent::Delta { text },
                Segment::Reasoning(text) => ChatStreamEvent::ReasoningDelta { text },
            })
            .collect()
    };
    match event {
        ChatStreamEvent::Delta { text } => to_events(splitter.push(&text)),
        ChatStreamEvent::Stop { .. } => {
            let mut out = to_events(splitter.finish());
            out.push(event);
            out
        }
        other => vec![other],
    }
}

#[cfg(test)]
mod response_model_tests {
    use super::{response_model, response_model_or};
    use serde_json::json;

    #[test]
    fn response_model_prefers_top_level_and_supports_protocol_fields() {
        assert_eq!(
            response_model(&json!({"model": "served-model"})).as_deref(),
            Some("served-model")
        );
        assert_eq!(
            response_model(&json!({"response": {"model": "responses-model"}})).as_deref(),
            Some("responses-model")
        );
        assert_eq!(
            response_model(&json!({"modelVersion": "gemini-model"})).as_deref(),
            Some("gemini-model")
        );
        assert_eq!(
            response_model(&json!({"message": {"model": "anthropic-stream-model"}})).as_deref(),
            Some("anthropic-stream-model")
        );
    }

    #[test]
    fn response_model_falls_back_for_missing_or_empty_fields() {
        assert_eq!(response_model(&json!({"model": ""})), None);
        assert_eq!(
            response_model_or(&json!({"model": ""}), "requested-model"),
            "requested-model"
        );
    }
}

#[cfg(test)]
#[path = "test_response.rs"]
mod test_response;

#[cfg(test)]
#[path = "test_usage_matrix.rs"]
mod test_usage_matrix;

/// 客户端 SSE 渲染状态（issue 70 / issue 71）：按 client_protocol 持有对应状态机。
/// Anthropic 系走 [`AnthropicSseState`]；Responses 客户端走
/// [`ResponsesSseState`](super::super::openai_responses::ResponsesSseState)；
/// OpenAI / Completions / Gemini 走各自轻量状态机（issue 71）。
pub enum ClientSseState {
    Anthropic(AnthropicSseState),
    Responses(super::super::openai_responses::ResponsesSseState),
    OpenAi(super::super::openai::OpenAiSseState),
    Completions(super::super::openai_completions::CompletionsSseState),
    Gemini(super::super::gemini::GeminiSseState),
}

impl Default for ClientSseState {
    fn default() -> Self {
        ClientSseState::Anthropic(AnthropicSseState::default())
    }
}

impl ClientSseState {
    /// 按需换轨到 Responses 状态（调用方通常按默认构造了 Anthropic 状态）。
    fn as_responses(&mut self) -> &mut super::super::openai_responses::ResponsesSseState {
        if !matches!(self, ClientSseState::Responses(_)) {
            *self = ClientSseState::Responses(Default::default());
        }
        match self {
            ClientSseState::Responses(s) => s,
            _ => unreachable!(),
        }
    }

    fn as_anthropic(&mut self) -> &mut AnthropicSseState {
        if !matches!(self, ClientSseState::Anthropic(_)) {
            *self = ClientSseState::Anthropic(Default::default());
        }
        match self {
            ClientSseState::Anthropic(s) => s,
            _ => unreachable!(),
        }
    }

    fn as_openai(&mut self, include_usage: bool) -> &mut super::super::openai::OpenAiSseState {
        if !matches!(self, ClientSseState::OpenAi(_)) {
            *self = ClientSseState::OpenAi(super::super::openai::OpenAiSseState::new(include_usage));
        }
        match self {
            ClientSseState::OpenAi(s) => s,
            _ => unreachable!(),
        }
    }

    fn as_completions(
        &mut self,
        include_usage: bool,
    ) -> &mut super::super::openai_completions::CompletionsSseState {
        if !matches!(self, ClientSseState::Completions(_)) {
            *self = ClientSseState::Completions(
                super::super::openai_completions::CompletionsSseState::new(include_usage),
            );
        }
        match self {
            ClientSseState::Completions(s) => s,
            _ => unreachable!(),
        }
    }

    fn as_gemini(&mut self) -> &mut super::super::gemini::GeminiSseState {
        if !matches!(self, ClientSseState::Gemini(_)) {
            *self = ClientSseState::Gemini(Default::default());
        }
        match self {
            ClientSseState::Gemini(s) => s,
            _ => unreachable!(),
        }
    }

    /// 流末冲刷（issue 71）：OpenAI Chat / Completions 客户端把 `[DONE]` 哨兵压到 usage 块
    /// 之后（官方顺序 finish → usage → [DONE]），流结束仍无 usage 时在此补发；其余协议
    /// 无哨兵 / 无待冲刷内容 → None。
    pub fn flush(&mut self) -> Option<String> {
        match self {
            ClientSseState::Anthropic(s) => s.flush(),
            ClientSseState::OpenAi(s) => s.flush(),
            ClientSseState::Completions(s) => s.flush(),
            ClientSseState::Gemini(s) => s.flush(),
            ClientSseState::Responses(_) => None,
        }
    }

    /// 流被错误帧提前终止时调用：错误帧已含各协议自身的终止哨兵 / 终态载荷，标记后
    /// [`ClientSseState::flush`] 不再补发（openai 系免双 `[DONE]`、anthropic 免双
    /// message_stop、gemini 免终帧重复）。
    pub fn mark_done(&mut self) {
        match self {
            ClientSseState::Anthropic(s) => {
                s.stop_seen = true;
                s.finish_reason = None;
                s.terminated = true;
            }
            ClientSseState::OpenAi(s) => s.mark_done(),
            ClientSseState::Completions(s) => s.mark_done(),
            ClientSseState::Gemini(s) => s.mark_done(),
            ClientSseState::Responses(_) => {}
        }
    }
}

/// Anthropic SSE 出站状态机：跨 chunk 维护 block index 分配与开块表。
///
/// 中立事件流的 tool index 是「第几个工具」；Anthropic wire 的 content block index
/// 是消息内全局序（text/thinking/tool 混排连续编号）。本结构把中立 index 映射到
/// wire index，并在首个块事件时发 content_block_start、Stop 前统一补 content_block_stop。
#[derive(Default)]
pub struct AnthropicSseState {
    /// 下一个可用 wire block index
    next_index: u32,
    /// text 块是否已开 + 其 wire index（thinking 先于 text 出现时 text 不再恒占 0）
    text_index: Option<u32>,
    /// thinking 块是否已开 + 其 wire index
    thinking_index: Option<u32>,
    /// thinking 首帧标记（首个 thinking_delta 前须发 content_block_start）
    thinking_just_opened: Option<bool>,
    /// 中立 tool index → wire block index（已开块）
    tool_wire: std::collections::HashMap<u32, u32>,
    /// 跨帧累积的 usage（issue 71）：anthropic 上游 message_start 带输入与缓存、
    /// message_delta 只带输出，`Usage::merge_partial` 合并后在 message_delta 终帧回传。
    pending_usage: Usage,
    /// Stop 已见标记：anthropic 上游 message_delta 与 message_stop 各产出一个 Stop，
    /// 第二次重复不再关块 / 不再改写终帧内容。
    stop_seen: bool,
    /// Stop 携带的 finish_reason（终帧内容压到 flush 才渲染，issue 71）。
    finish_reason: Option<String>,
    /// 流被错误帧提前终止（mark_done）：flush 不再补终帧。
    terminated: bool,
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
                let idx = match self.text_index {
                    Some(i) => i,
                    None => {
                        // 思维链先于正文出现时 thinking 已占 0，text 顺延；不可再写死 0
                        // （两块同 index 会让客户端把 text_delta 灌进 thinking 块 → 解析错乱）。
                        let i = self.alloc_block();
                        self.text_index = Some(i);
                        parts.push(format!(
                            "event: content_block_start\ndata: {}\n\n",
                            serde_json::json!({ "type": "content_block_start", "index": i, "content_block": { "type": "text", "text": "" } })
                        ));
                        i
                    }
                };
                parts.push(format!(
                    "event: content_block_delta\ndata: {}\n\n",
                    serde_json::json!({ "type": "content_block_delta", "index": idx, "delta": { "type": "text_delta", "text": text } })
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
            ChatStreamEvent::ToolDelta {
                index,
                id,
                name,
                input,
            } => {
                let mut parts = Vec::new();
                // 首帧（带 id/name）→ wire index 分配 + content_block_start
                if !self.tool_wire.contains_key(index)
                    && let (Some(id), Some(name)) = (id, name)
                {
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
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join(""))
                }
            }
            ChatStreamEvent::Stop { finish_reason } => {
                // 重复 Stop（anthropic message_delta + message_stop / [DONE] 哨兵）只处理一次
                if self.stop_seen {
                    return None;
                }
                self.stop_seen = true;
                self.finish_reason = finish_reason.clone();
                // issue 71：message_delta（含 usage 终值）压到流末 flush 才发——openai 系上游
                // 的 usage 帧晚于 finish 帧到达（甚至跨 chunk），立即发终帧会把 usage 定格成 0。
                // 此处只关全部开块（text / thinking / tool × n），wire index 升序。
                let mut parts = Vec::new();
                let mut closes: Vec<u32> = Vec::new();
                if let Some(i) = self.text_index {
                    closes.push(i);
                }
                if let Some(i) = self.thinking_index {
                    closes.push(i);
                }
                closes.extend(self.tool_wire.values().copied());
                closes.sort();
                for c in closes {
                    parts.push(format!(
                        "event: content_block_stop\ndata: {}\n\n",
                        serde_json::json!({ "type": "content_block_stop", "index": c })
                    ));
                }
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join(""))
                }
            }
            ChatStreamEvent::Usage { usage } => {
                self.pending_usage.merge_partial(usage);
                None
            }
            ChatStreamEvent::Ping => Some("event: ping\ndata: {\"type\":\"ping\"}\n\n".to_string()),
        }
    }

    /// 流末冲刷（issue 71）：Stop 已见时在此发 message_delta（stop_reason + 全量 usage）+
    /// message_stop。usage 帧晚于 finish 帧到达时，这里拿到的才是终值。
    pub fn flush(&mut self) -> Option<String> {
        if !self.stop_seen || self.terminated {
            return None;
        }
        let reason = self.finish_reason.take().unwrap_or_default();
        Some(format!(
            "event: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            serde_json::json!({
                "type": "message_delta",
                "delta": { "stop_reason": anthropic_stop_reason(Some(reason.as_str())), "stop_sequence": null },
                "usage": {
                    "input_tokens": self.pending_usage.prompt_tokens.unwrap_or(0),
                    "output_tokens": self.pending_usage.completion_tokens.unwrap_or(0),
                    "cache_read_input_tokens": self.pending_usage.cache_read_tokens.unwrap_or(0),
                    "cache_creation_input_tokens": self.pending_usage.cache_write_tokens.unwrap_or(0),
                }
            })
        ))
    }

    fn alloc_block(&mut self) -> u32 {
        // 按出现顺序连续编号（text / thinking / tool 共用同一计数器）
        let i = self.next_index;
        self.next_index += 1;
        i
    }
}
