use serde_json::Value;

use super::types::*;

/// Gemini API 请求格式
#[derive(Debug, Clone, serde::Serialize)]
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct GeminiContent {
    pub role: String,
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GeminiFunctionResponse {
    pub name: String,
    pub response: Value,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GeminiToolDecl {
    pub function_declarations: Vec<GeminiFunctionDecl>,
}

#[derive(Debug, Clone, serde::Serialize)]
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
            parts: vec![GeminiPart { text: Some(text), function_call: None, function_response: None }],
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
                vec![GeminiPart { text: Some(s.clone()), function_call: None, function_response: None }]
            }
            MessageContent::Blocks(blocks) => {
                blocks.iter().map(|b| match b {
                    ContentBlock::Text { text } => GeminiPart {
                        text: Some(text.clone()), function_call: None, function_response: None,
                    },
                    ContentBlock::ToolUse { name, input, .. } => GeminiPart {
                        text: None,
                        function_call: Some(GeminiFunctionCall {
                            name: name.clone(),
                            args: input.clone(),
                        }),
                        function_response: None,
                    },
                    ContentBlock::ToolResult { tool_use_id, content } => GeminiPart {
                        text: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse {
                            name: tool_use_id.clone(),
                            response: serde_json::json!({ "result": content }),
                        }),
                    },
                }).collect()
            }
        };

        contents.push(GeminiContent { role: role.to_string(), parts });
    }

    let tools = req.tools.as_ref().map(|ts| {
        vec![GeminiToolDecl {
            function_declarations: ts.iter().map(|t| GeminiFunctionDecl {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.input_schema.clone(),
            }).collect(),
        }]
    });

    let generation_config = if req.max_tokens.is_some() || req.temperature.is_some() || req.top_p.is_some() {
        Some(GeminiGenerationConfig {
            max_output_tokens: req.max_tokens,
            temperature: req.temperature,
            top_p: req.top_p,
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

/// 解析 Gemini SSE 格式的流式事件
pub fn parse_gemini_sse(data: &Value) -> Option<ChatStreamEvent> {
    let candidates = data.get("candidates")?.as_array()?;
    let candidate = candidates.first()?;
    let content = candidate.get("content")?;
    let parts = content.get("parts")?.as_array()?;
    let part = parts.first()?;

    // 文本 delta
    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
        return Some(ChatStreamEvent::Delta { text: text.to_string() });
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
    if let Some(reason) = finish_reason {
        if reason == "STOP" || reason == "MAX_TOKENS" {
            return Some(ChatStreamEvent::Stop {
                finish_reason: Some(reason.to_lowercase()),
            });
        }
    }

    None
}
