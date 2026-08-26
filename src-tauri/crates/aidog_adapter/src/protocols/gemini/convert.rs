use serde_json::Value;
use crate::types::*;

/// Gemini API 请求格式
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<GeminiToolDecl>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeminiContent {
    pub role: String,
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// thought=true 标记 reasoning part（Gemini 思考链）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
    /// inlineData / fileData 等动态 part 字段（多模态图片）
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<GeminiThinkingConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiThinkingConfig {
    pub thinking_budget: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiToolDecl {
    pub function_declarations: Vec<GeminiFunctionDecl>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionDecl {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: Value,
}

/// 从内部 ChatRequest 转为 Gemini 格式
pub fn to_gemini(req: &ChatRequest) -> GeminiRequest {
    let system_instruction = req.system.as_ref().map(|s| {
        let text = match s {
            SystemContent::Text(t) => t.clone(),
            SystemContent::Blocks(blocks) => blocks.iter()
                .filter_map(|b| b.get("text").and_then(|v| v.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
        };
        GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart { text: Some(text), thought: None, function_call: None, function_response: None, extra: None }],
        }
    });

    let mut contents: Vec<GeminiContent> = Vec::new();

    for m in &req.messages {
        let role = match m.role {
            Role::User | Role::System | Role::Tool => "user",
            Role::Assistant => "model",
        };

        let parts: Vec<GeminiPart> = match &m.content {
            MessageContent::Text(s) => {
                vec![GeminiPart { text: Some(s.clone()), thought: None, function_call: None, function_response: None, extra: None }]
            }
            MessageContent::Blocks(blocks) => {
                blocks.iter().map(|b| match b {
                    ContentBlock::Text { text, .. } => GeminiPart {
                        text: Some(text.clone()), function_call: None, function_response: None,
                        thought: None,
                    extra: None,
                    },
                    ContentBlock::ToolUse { name, input, .. } => GeminiPart {
                        text: None,
                        function_call: Some(GeminiFunctionCall {
                            name: name.clone(),
                            args: input.clone(),
                        }),
                        function_response: None,
                        thought: None,
                    extra: None,
                    },
                    ContentBlock::ToolResult { tool_use_id, content, name, is_error, .. } => GeminiPart {
                        text: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse {
                            // Gemini 靠 name 关联 functionResponse ↔ functionCall；中立 name 缺时退 tool_use_id
                            name: name.clone().unwrap_or_else(|| tool_use_id.clone()),
                            // response 是自由 object，没有官方 error 键约定；且多模态 functionResponse
                            // 会被上游拒（"Multimodal function responses are not supported"），
                            // 所以失败与非文本 block 一律走文本标注/占位
                            response: serde_json::json!({ "result": mark_tool_error(content, *is_error) }),
                        }),
                        thought: None,
                    extra: None,
                    },
                    // thinking block → thought part（signature 丢，Gemini 无此概念）
                    ContentBlock::Unknown(v)
                        if v.get("type").and_then(|t| t.as_str()) == Some("thinking") => GeminiPart {
                        text: v.get("thinking").and_then(|t| t.as_str()).map(|s| s.to_string()),
                        thought: Some(true),
                        function_call: None,
                        function_response: None,
                    extra: None,
                    },
                    // image block → inlineData(base64) / fileData(url)
                    ContentBlock::Unknown(v)
                        if v.get("type").and_then(|t| t.as_str()) == Some("image") => {
                            let src = v.get("source").cloned().unwrap_or(serde_json::Value::Null);
                            match src.get("type").and_then(|t| t.as_str()) {
                                Some("base64") => GeminiPart {
                                    text: None, thought: None, function_call: None, function_response: None,
                                    extra: Some(serde_json::json!({
                                        "inlineData": {
                                            "mimeType": src.get("media_type").cloned().unwrap_or(serde_json::json!("application/octet-stream")),
                                            "data": src.get("data").cloned().unwrap_or(serde_json::Value::String(String::new())),
                                        }
                                    })),
                                },
                                Some("url") => GeminiPart {
                                    text: None, thought: None, function_call: None, function_response: None,
                                    extra: Some(serde_json::json!({
                                        "fileData": {
                                            "fileUri": src.get("url").cloned().unwrap_or(serde_json::Value::Null),
                                        }
                                    })),
                                },
                                _ => GeminiPart { text: None, thought: None, function_call: None, function_response: None, extra: None },
                            }
                        }
                    // 未覆盖 block(…): 尝试取 text，否则空 part(保留消息位)
                    ContentBlock::Unknown(v) => GeminiPart {
                        text: v.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()),
                        function_call: None,
                        function_response: None,
                        thought: None,
                    extra: None,
                    },
                }).collect()
            }
        };

        contents.push(GeminiContent { role: role.to_string(), parts });
    }

    // 服务端工具在 Gemini 侧无执行方，整条不下发；全被丢弃时不写 tools 键
    let tools = req.tools.as_ref().and_then(|ts| {
        let decls: Vec<GeminiFunctionDecl> = client_tools(ts, "gemini").into_iter().map(|t| GeminiFunctionDecl {
            name: t.name.clone(),
            description: t.description.clone(),
            parameters: t.input_schema.clone(),
        }).collect();
        (!decls.is_empty()).then(|| vec![GeminiToolDecl { function_declarations: decls }])
    });

    let generation_config = if req.max_tokens.is_some() || req.temperature.is_some() || req.top_p.is_some()
        || req.thinking_budget.is_some() {
        Some(GeminiGenerationConfig {
            max_output_tokens: req.max_tokens,
            temperature: req.temperature,
            top_p: req.top_p,
            thinking_config: req.thinking_budget.map(|b| GeminiThinkingConfig { thinking_budget: b }),
        })
    } else {
        None
    };

    GeminiRequest {
        contents,
        system_instruction,
        generation_config,
        tools,
    }
}

/// 解析 Gemini API 非流式响应为归一 NonStreamResponse
pub fn parse_gemini_response(body: &Value, fallback_model: &str) -> Option<crate::converter::NonStreamResponse> {
    let candidates = body.get("candidates")?.as_array()?;
    let candidate = candidates.first()?;

    let id = body.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let model = body.get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_model)
        .to_string();

    let content = candidate.get("content")?;
    let parts = content.get("parts")?.as_array()?;

    let mut text_parts: Vec<String> = Vec::new();
    let mut reasoning_parts: Vec<String> = Vec::new();
    let mut tool_uses: Vec<(String, String, Value)> = Vec::new();

    for part in parts {
        // 提取 text（非 thought 标记的普通文本）
        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
            // 检查是否有 thought 标记，有则归入 reasoning
            if part.get("thought").and_then(|v| v.as_bool()).unwrap_or(false) {
                reasoning_parts.push(text.to_string());
            } else {
                text_parts.push(text.to_string());
            }
        }

        // 提取 function_call（tool_use）
        if let Some(fc) = part.get("functionCall") {
            let name = fc.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = fc.get("args").cloned().unwrap_or_else(|| Value::Object(Default::default()));
            let id = format!("tool_{}", tool_uses.len()); // Gemini 无 id，生成一个
            tool_uses.push((id, name, args));
        }
    }

    let text = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };

    let reasoning = if reasoning_parts.is_empty() {
        None
    } else {
        Some(reasoning_parts.join("\n\n"))
    };

    // finishReason 映射
    let finish_reason = candidate.get("finishReason")
        .and_then(|v| v.as_str())
        .unwrap_or("STOP");
    let stop_reason = match finish_reason {
        "STOP" => "end_turn",
        "MAX_TOKENS" => "max_tokens",
        "SAFETY" | "RECITATION" | "OTHER" => "end_turn",
        _ => "end_turn",
    }.to_string();

    // usageMetadata
    let usage = body.get("usageMetadata");
    let input_tokens = usage
        .and_then(|u| u.get("promptTokenCount"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|u| u.get("candidatesTokenCount"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_read_tokens = usage
        .and_then(|u| u.get("cachedContentTokenCount"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    Some(crate::converter::NonStreamResponse {
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

/// 渲染归一响应为 Gemini API 非流式响应体。
///
/// 映射规则：
/// - candidates[0].content.parts[]: {text} + {thought:true,text:reasoning} + {functionCall}
/// - usageMetadata: promptTokenCount/completionTokenTotal/totalTokenCount
pub fn render_gemini_response(r: &crate::converter::NonStreamResponse) -> Option<Value> {
    let mut parts = Vec::new();

    // 添加文本 part
    if let Some(text) = &r.text
        && !text.is_empty() {
            parts.push(serde_json::json!({
                "text": text,
            }));
        }

    // 添加 reasoning part（thought 格式）
    if let Some(reasoning) = &r.reasoning
        && !reasoning.is_empty() {
            parts.push(serde_json::json!({
                "thought": true,
                "text": reasoning,
            }));
        }

    // 添加 functionCall parts
    for (_id, name, input) in &r.tool_uses {
        parts.push(serde_json::json!({
            "functionCall": {
                "name": name,
                "args": input,
            },
        }));
    }

    // 兜底：既无 text 也无 tool_use（异常上游）→ 空 text part，保证 parts 非空
    if parts.is_empty() {
        parts.push(serde_json::json!({
            "text": "",
        }));
    }

    // 映射 finishReason
    let finish_reason = match r.stop_reason.as_str() {
        "tool_use" => "STOP",
        "max_tokens" => "MAX_TOKENS",
        "end_turn" | "stop_sequence" => "STOP",
        _ => "STOP",
    };

    Some(serde_json::json!({
        "candidates": [{
            "content": {
                "parts": parts,
                "role": "model",
            },
            "finishReason": finish_reason,
            "index": 0,
        }],
        "usageMetadata": {
            "promptTokenCount": r.input_tokens,
            "completionTokenCount": r.output_tokens,
            "totalTokenCount": r.input_tokens + r.output_tokens,
        },
        "modelVersion": r.model,
    }))
}

// Gemini functionCall 无 id：自生成 `gemini-fc-<n>`（functionResponse 按 name 回填同规则 id）。
// ponytail: 进程内原子自增即可，跨请求重复无害（id 仅在中立模型内部关联用）。
fn next_fc_id() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(1);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

/// 从 Gemini API 请求格式解析为内部 ChatRequest
pub fn from_gemini(body: &Value) -> Option<ChatRequest> {
    let contents = body.get("contents")?.as_array()?;
    let mut messages = Vec::new();
    // name → 最近同名 functionCall 的自生成 id（跨 contents 顺序扫描，functionResponse 配对用）
    let mut call_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // System instruction
    let system = body.get("systemInstruction")
        .and_then(|si| si.get("parts"))
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    for c in contents {
        let role_str = c.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let role = match role_str {
            "model" => Role::Assistant,
            _ => Role::User,
        };

        let parts = c.get("parts")?.as_array()?;
        let mut text_parts = Vec::new();
        let mut thinking_blocks: Vec<ContentBlock> = Vec::new();
        let mut image_blocks: Vec<ContentBlock> = Vec::new();
        for p in parts {
            // inlineData / fileData → 中立 image block
            if let Some(inline) = p.get("inlineData") {
                image_blocks.push(ContentBlock::Unknown(serde_json::json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": inline.get("mimeType").cloned().unwrap_or(serde_json::json!("application/octet-stream")),
                        "data": inline.get("data").cloned().unwrap_or(serde_json::Value::String(String::new())),
                    }
                })));
                continue;
            }
            if let Some(file) = p.get("fileData")
                && let Some(uri) = file.get("fileUri").and_then(|v| v.as_str()) {
                    image_blocks.push(ContentBlock::Unknown(serde_json::json!({
                        "type": "image",
                        "source": { "type": "url", "url": uri }
                    })));
                    continue;
                }
            if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
                if p.get("thought").and_then(|v| v.as_bool()).unwrap_or(false) {
                    // thought part → 中立 thinking block（无 signature；回传 Anthropic 侧降级不回传）
                    thinking_blocks.push(ContentBlock::Unknown(serde_json::json!({
                        "type": "thinking", "thinking": t,
                    })));
                } else {
                    text_parts.push(t.to_string());
                }
            }
        }
        // 工具 part：functionCall(Gemini 无 id，按序自生成并记 name→id) /
        // functionResponse(按 name 查最近同名 Call 的生成 id 完成关联)
        let mut tool_blocks: Vec<ContentBlock> = Vec::new();
        for p in parts {
            if let Some(fc) = p.get("functionCall") {
                let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = fc.get("args").cloned().unwrap_or(serde_json::json!({}));
                let id = format!("gemini-fc-{}", next_fc_id());
                call_ids.insert(name.to_string(), id.clone());
                tool_blocks.push(ContentBlock::ToolUse {
                    id,
                    name: name.to_string(),
                    input: args, extra: None
                });
            }
            if let Some(fr) = p.get("functionResponse") {
                let name = fr.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let response = fr.get("response").cloned().unwrap_or(serde_json::json!({}));
                tool_blocks.push(ContentBlock::ToolResult {
                    tool_use_id: call_ids.get(name).cloned().unwrap_or_else(|| format!("gemini-fc-{name}")),
                    content: response.to_string(),
                    name: Some(name.to_string()), is_error: None, content_blocks: None, extra: None
                });
            }
        }
        if !tool_blocks.is_empty() || !thinking_blocks.is_empty() || !image_blocks.is_empty() {
            let mut blocks: Vec<ContentBlock> = text_parts.into_iter()
                .map(|t| ContentBlock::Text { text: t, extra: None })
                .collect();
            blocks.extend(thinking_blocks);
            blocks.extend(image_blocks);
            blocks.extend(tool_blocks);
            messages.push(Message { role, content: MessageContent::Blocks(blocks) });
            continue;
        }
        let content = if text_parts.len() == 1 {
            MessageContent::Text(text_parts.into_iter().next().unwrap())
        } else if text_parts.is_empty() {
            MessageContent::Text(String::new())
        } else {
            MessageContent::Text(text_parts.join("\n"))
        };
        messages.push(Message { role, content });
    }

    let gen_config = body.get("generationConfig");
    let thinking_budget = gen_config
        .and_then(|g| g.get("thinkingConfig"))
        .and_then(|t| t.get("thinkingBudget"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let max_tokens = gen_config.and_then(|g| g.get("maxOutputTokens")).and_then(|v| v.as_u64()).map(|v| v as u32);
    let temperature = gen_config.and_then(|g| g.get("temperature")).and_then(|v| v.as_f64()).map(|v| v as f32);
    let top_p = gen_config.and_then(|g| g.get("topP")).and_then(|v| v.as_f64()).map(|v| v as f32);

    let tools = body.get("tools")
        .and_then(|t| t.as_array())
        .map(|ts| {
            ts.iter()
                .filter_map(|t| t.get("functionDeclarations").and_then(|d| d.as_array()))
                .flatten()
                .map(|d| Tool {
                    name: d.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    description: d.get("description").and_then(|v| v.as_str()).map(str::to_string),
                    input_schema: d.get("parameters").cloned().unwrap_or(serde_json::json!({})),
                tool_type: None,
                cache_control: None,
                extra: None,
                })
                .collect::<Vec<_>>()
        })
        .filter(|t| !t.is_empty());
    Some(ChatRequest {
        model: String::new(),
        thinking_budget,
        messages,
        system: system.map(SystemContent::Text),
        max_tokens,
        temperature,
        top_p,
        stream: None,
        tools,
        tool_choice: None,
        extra: None,
        thinking_mode: None,
    })
}

/// 解析 Gemini SSE 格式的流式事件
pub fn parse_gemini_sse(data: &Value) -> Option<ChatStreamEvent> {
    let candidates = data.get("candidates")?.as_array()?;
    let candidate = candidates.first()?;

    // 结束（finishReason 帧可能无 content/parts，须先判）
    if let Some(reason) = candidate.get("finishReason").and_then(|v| v.as_str())
        && (reason == "STOP" || reason == "MAX_TOKENS") {
            return Some(ChatStreamEvent::Stop {
                finish_reason: Some(reason.to_lowercase()),
            });
        }

    let content = candidate.get("content")?;
    let parts = content.get("parts")?.as_array()?;
    let part = parts.first()?;

    // 文本 delta（含 thought 标记检查）
    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
        // 检查是否有 thought 标记，有则归入 reasoning
        if part.get("thought").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Some(ChatStreamEvent::ReasoningDelta {
                text: text.to_string(),
            });
        } else {
            return Some(ChatStreamEvent::Delta { text: text.to_string() });
        }
    }

    // function call
    if let Some(fc) = part.get("functionCall") {
        let name = fc.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let args = fc.get("args");
        let input = args.map(|a| serde_json::to_string(a).unwrap_or_default());
        return Some(ChatStreamEvent::ToolDelta {
            index: 0,
            id: name.clone(),
            name,
            input,
        });
    }

    // 结束
    let finish_reason = candidate.get("finishReason").and_then(|v| v.as_str());
    if let Some(reason) = finish_reason
        && (reason == "STOP" || reason == "MAX_TOKENS") {
            return Some(ChatStreamEvent::Stop {
                finish_reason: Some(reason.to_lowercase()),
            });
        }

    None
}

/// 将统一的 ChatStreamEvent 转为 Gemini SSE wire 帧（用于返回给 Gemini 客户端）。
///
/// 实测 Gemini `streamGenerateContent?alt=sse` 真实 wire 格式：每帧 `data: {json}\n\n`，
/// 无 `[DONE]` 终止帧（流结束由 finishReason 承载，见上方 `parse_gemini_sse`）。
/// 与 `to_openai_sse` / `to_anthropic_sse` 同一约定：`to_*_sse` 一律返回完整 wire 帧。
pub fn to_gemini_sse(event: &ChatStreamEvent, model: &str) -> Option<String> {
    let json = match event {
        ChatStreamEvent::Start { .. } => return None,
        ChatStreamEvent::Delta { text } => serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": text }],
                    "role": "model"
                }
            }],
            "modelVersion": model,
        }),
        ChatStreamEvent::ReasoningDelta { text } => serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{ "thought": true, "text": text }],
                    "role": "model"
                }
            }],
            "modelVersion": model,
        }),
        ChatStreamEvent::Stop { finish_reason } => {
            let reason = match finish_reason.as_deref() {
                Some("end_turn") | Some("stop") => "STOP",
                Some("max_tokens") => "MAX_TOKENS",
                _ => "STOP",
            };
            serde_json::json!({
                "candidates": [{
                    "finishReason": reason,
                    "content": { "parts": [], "role": "model" }
                }]
            })
        }
        ChatStreamEvent::Usage { .. } => return None,
        ChatStreamEvent::ToolDelta { name, input, .. } => {
            let args: Value = input.as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({}));
            serde_json::json!({
                "candidates": [{
                    "content": {
                        "parts": [{
                            "functionCall": { "name": name, "args": args }
                        }],
                        "role": "model"
                    }
                }]
            })
        }
    };
    Some(format!("data: {}\n\n", json))
}

#[cfg(test)]
#[path = "test_gemini.rs"]
mod test_gemini;
