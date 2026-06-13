use serde::{Deserialize, Serialize};

/// 统一内部消息格式（基于 Anthropic Messages API 扩展）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// 额外参数（协议特有字段透传）
    #[serde(flatten)]
    pub extra: Option<serde_json::Value>,
}

/// System content: can be a plain string or array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemContent {
    Text(String),
    Blocks(Vec<serde_json::Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

/// 消息内容：文本或多内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// 消息内容块。
///
/// 已知类型(text/tool_use/tool_result)走强类型；未覆盖类型(thinking/image/…)
/// 降级为 [`ContentBlock::Unknown`] 原样保留，避免 Anthropic 真实请求因个别 block
/// 类型缺失导致整条 [`ChatRequest`] 反序列化失败(→ 400 "failed to parse request")。
/// `Unknown` 透传/诊断时保留原值；转换到目标协议时由各 converter 决定降级策略。
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    /// 未覆盖的 block 类型，原样保留(透传/诊断用)。
    Unknown(serde_json::Value),
}

impl<'de> Deserialize<'de> for ContentBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        // 已知类型走强类型解析；任一字段缺失/类型不符 → 降级 Unknown 原样保留
        let parsed: Result<ContentBlock, ()> = match ty {
            "text" => {
                #[derive(Deserialize)]
                struct T {
                    text: String,
                }
                serde_json::from_value::<T>(v.clone())
                    .map(|t| ContentBlock::Text { text: t.text })
                    .map_err(|_| ())
            }
            "tool_use" => {
                #[derive(Deserialize)]
                struct TU {
                    id: String,
                    name: String,
                    input: serde_json::Value,
                }
                serde_json::from_value::<TU>(v.clone())
                    .map(|tu| ContentBlock::ToolUse {
                        id: tu.id,
                        name: tu.name,
                        input: tu.input,
                    })
                    .map_err(|_| ())
            }
            "tool_result" => {
                #[derive(Deserialize)]
                struct TR {
                    tool_use_id: String,
                    #[serde(default)]
                    content: serde_json::Value,
                }
                serde_json::from_value::<TR>(v.clone())
                    .map(|tr| {
                        // content 容错: string 原样; array 抽 text 拼接; 其他转字符串
                        let content = match tr.content {
                            serde_json::Value::String(s) => s,
                            serde_json::Value::Array(arr) => arr
                                .iter()
                                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join(""),
                            serde_json::Value::Null => String::new(),
                            other => other.to_string(),
                        };
                        ContentBlock::ToolResult {
                            tool_use_id: tr.tool_use_id,
                            content,
                        }
                    })
                    .map_err(|_| ())
            }
            _ => Err(()),
        };
        Ok(parsed.unwrap_or(ContentBlock::Unknown(v)))
    }
}

impl Serialize for ContentBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Unknown 原样输出(含原始 type 与全部字段)；已知类型按 Anthropic block 结构序列化
        let v = match self {
            ContentBlock::Unknown(v) => v.clone(),
            ContentBlock::Text { text } => {
                serde_json::json!({ "type": "text", "text": text })
            }
            ContentBlock::ToolUse { id, name, input } => {
                serde_json::json!({ "type": "tool_use", "id": id, "name": name, "input": input })
            }
            ContentBlock::ToolResult { tool_use_id, content } => {
                serde_json::json!({ "type": "tool_result", "tool_use_id": tool_use_id, "content": content })
            }
        };
        v.serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Auto,
    Any,
    None,
    Named { name: String },
}

// ─── Response ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

// ─── Streaming ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct StreamEvent {
    pub event: String,
    pub data: serde_json::Value,
}

/// 统一的流式事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatStreamEvent {
    /// 开始
    #[serde(rename = "start")]
    Start { id: String, model: String },
    /// 文本增量
    #[serde(rename = "delta")]
    Delta { text: String },
    /// 工具调用增量
    #[serde(rename = "tool_delta")]
    ToolDelta {
        index: u32,
        id: Option<String>,
        name: Option<String>,
        input: Option<String>,
    },
    /// 结束
    #[serde(rename = "stop")]
    Stop { finish_reason: Option<String> },
    /// 用量
    #[serde(rename = "usage")]
    Usage { usage: Usage },
}
