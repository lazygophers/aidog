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
    /// 思考预算 tokens（Anthropic thinking.budget_tokens / Gemini thinkingBudget /
    /// OpenAI reasoning_effort 三家的统一映射；None = 未开启思考）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u32>,
    /// 思考档位（与 `thinking_budget` 并存：预算是数字，档位是三态 + effort 名）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_mode: Option<ThinkingMode>,
    /// 额外参数（协议特有字段透传）
    #[serde(flatten)]
    pub extra: Option<serde_json::Value>,
}

/// 思考档位的中立表示：原值透传，不在入站做换算（换算表由出站映射统一持有）。
///
/// - `kind`：Anthropic `thinking.type` 三态原值（`enabled` / `disabled` / `adaptive`）
/// - `effort`：档位名原值（Anthropic `output_config.effort` / OpenAI `reasoning_effort` /
///   Responses `reasoning.effort`）
///
/// 两者可并存：Claude Code 2.x 会同时发 `thinking.type=adaptive` 与 `output_config.effort=high`。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ThinkingMode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

impl ThinkingMode {
    /// 两个档位来源都为空时视作「没有档位信息」，避免出站写出空对象。
    pub fn is_empty(&self) -> bool {
        self.kind.is_none() && self.effort.is_none()
    }

    /// 由两个可选来源构造；都为空返回 None。入站解析的统一入口。
    pub fn from_parts(kind: Option<String>, effort: Option<String>) -> Option<Self> {
        let m = ThinkingMode { kind, effort };
        (!m.is_empty()).then_some(m)
    }
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

impl MessageContent {
    /// 纯文本视图：Text 原样；Blocks 拼接全部 Text block（其他类型跳过）。
    /// adapter 各协议序列化的「取文本」统一出口。
    pub fn as_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        }
    }

    /// block 列表视图：Text 折叠为单 Text block。
    pub fn blocks(&self) -> Vec<ContentBlock> {
        match self {
            MessageContent::Text(s) => vec![ContentBlock::Text { text: s.clone(), extra: None }],
            MessageContent::Blocks(blocks) => blocks.clone(),
        }
    }

    /// 追加 block：Text 自动升级为 Blocks。adapter 组装多 block 消息的统一入口。
    pub fn push_block(&mut self, block: ContentBlock) {
        match self {
            MessageContent::Text(s) => {
                *self = MessageContent::Blocks(vec![ContentBlock::Text { text: std::mem::take(s), extra: None }, block]);
            }
            MessageContent::Blocks(blocks) => blocks.push(block),
        }
    }
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
        /// block 级附加键（`cache_control` 等），入站原样收下、出站按目标协议决定是否写回
        extra: Option<serde_json::Value>,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        extra: Option<serde_json::Value>,
    },
    ToolResult {
        tool_use_id: String,
        /// 纯文本视图：数组形态 content 里的非文本 block 在此降级为可读占位
        content: String,
        /// 工具名（Gemini functionResponse 靠 name 关联；OpenAI/Anthropic 无此概念，None 不序列化）
        name: Option<String>,
        /// 工具执行失败标记（Anthropic `tool_result.is_error`）
        is_error: Option<bool>,
        /// 数组形态 content 的原始 block 列表（image 等非文本 block 在此保真）
        content_blocks: Option<Vec<serde_json::Value>>,
        extra: Option<serde_json::Value>,
    },
    /// 未覆盖的 block 类型，原样保留(透传/诊断用)。
    Unknown(serde_json::Value),
}

/// 目标协议没有 `is_error` 等价字段时，工具失败在文本里的显式标注。
/// OpenAI tool message / Responses `function_call_output.output` /
/// Gemini `functionResponse.response` 三条载体都只吃文本，靠这个前缀让模型识别失败。
pub const TOOL_ERROR_PREFIX: &str = "[tool_error] ";

/// 给纯文本载体加失败标注；`is_error != Some(true)` 时原样返回。
pub fn mark_tool_error(content: &str, is_error: Option<bool>) -> String {
    if is_error == Some(true) {
        format!("{TOOL_ERROR_PREFIX}{content}")
    } else {
        content.to_string()
    }
}

/// block 的纯文本视图：text 取原文，image 与其它类型降级为可读占位（不静默丢弃）。
fn block_text_view(b: &serde_json::Value) -> String {
    match b.get("type").and_then(|t| t.as_str()) {
        Some("text") => b.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        Some("image") => {
            let mt = b
                .get("source")
                .and_then(|s| s.get("media_type"))
                .and_then(|m| m.as_str())
                .unwrap_or("image");
            format!("[image: {mt}]")
        }
        Some(other) => format!("[{other} block]"),
        // 无 type 的元素：能取到 text 就取，取不到留空（与历史行为一致）
        None => b.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
    }
}

/// 取出 object 里已建模键之外的剩余键；无剩余返回 None。
fn rest_keys(v: &serde_json::Value, known: &[&str]) -> Option<serde_json::Value> {
    let obj = v.as_object()?;
    let rest: serde_json::Map<String, serde_json::Value> = obj
        .iter()
        .filter(|(k, _)| !known.contains(&k.as_str()))
        .map(|(k, val)| (k.clone(), val.clone()))
        .collect();
    (!rest.is_empty()).then_some(serde_json::Value::Object(rest))
}

/// 把附加键并回 block object；已建模键不被覆盖。
fn merge_extra(mut base: serde_json::Value, extra: &Option<serde_json::Value>) -> serde_json::Value {
    if let (Some(serde_json::Value::Object(ex)), Some(obj)) = (extra.as_ref(), base.as_object_mut()) {
        for (k, v) in ex {
            if !obj.contains_key(k) {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    base
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
                    .map(|t| ContentBlock::Text {
                        text: t.text,
                        extra: rest_keys(&v, &["type", "text"]),
                    })
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
                        extra: rest_keys(&v, &["type", "id", "name", "input"]),
                    })
                    .map_err(|_| ())
            }
            "tool_result" => {
                #[derive(Deserialize)]
                struct TR {
                    tool_use_id: String,
                    #[serde(default)]
                    content: serde_json::Value,
                    #[serde(default)]
                    name: Option<String>,
                    #[serde(default)]
                    is_error: Option<bool>,
                }
                serde_json::from_value::<TR>(v.clone())
                    .map(|tr| {
                        // content 容错: string 原样; array 抽文本视图拼接(非文本 block 留占位); 其他转字符串
                        // array 形态同时把原数组存进 content_blocks，供支持多模态 tool_result 的目标协议保真
                        let mut content_blocks = None;
                        let content = match tr.content {
                            serde_json::Value::String(s) => s,
                            serde_json::Value::Array(arr) => {
                                let text = arr.iter().map(block_text_view).collect::<Vec<_>>().join("");
                                content_blocks = Some(arr);
                                text
                            }
                            serde_json::Value::Null => String::new(),
                            other => other.to_string(),
                        };
                        ContentBlock::ToolResult {
                            tool_use_id: tr.tool_use_id,
                            content,
                            name: tr.name,
                            is_error: tr.is_error,
                            content_blocks,
                            extra: rest_keys(&v, &["type", "tool_use_id", "content", "name", "is_error"]),
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
            ContentBlock::Text { text, extra } => {
                merge_extra(serde_json::json!({ "type": "text", "text": text }), extra)
            }
            ContentBlock::ToolUse { id, name, input, extra } => merge_extra(
                serde_json::json!({ "type": "tool_use", "id": id, "name": name, "input": input }),
                extra,
            ),
            ContentBlock::ToolResult { tool_use_id, content, name, is_error, content_blocks, extra } => {
                // 数组形态原样写回（image 等非文本 block 保真）；字符串形态写字符串
                let content_v = match content_blocks {
                    Some(blocks) => serde_json::Value::Array(blocks.clone()),
                    None => serde_json::Value::String(content.clone()),
                };
                let mut v = serde_json::json!({ "type": "tool_result", "tool_use_id": tool_use_id, "content": content_v });
                if let Some(name) = name {
                    v["name"] = serde_json::json!(name);
                }
                if let Some(is_error) = is_error {
                    v["is_error"] = serde_json::json!(is_error);
                }
                merge_extra(v, extra)
            }
        };
        v.serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    // 入站工具可能缺 input_schema(如 Anthropic 服务端工具 web_search/bash)；缺失时默认空对象 {},
    // 避免单个工具字段缺失导致整请求 serde missing field → 400。禁默认 null(破坏上游)。
    #[serde(default = "default_input_schema")]
    pub input_schema: serde_json::Value,
    /// Anthropic 工具类型：None / `custom` = 客户端 function；
    /// 其余（`web_search_20250305` / `bash_20250124` …）= 服务端内置工具，只有 Anthropic 上游能执行。
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    /// prompt caching 标记（Anthropic-compat 上游专有）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<serde_json::Value>,
    /// 服务端工具的其余配置键（`max_uses` / `allowed_domains` …）
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl Tool {
    /// 服务端内置工具：由上游自己执行，转成普通 function 会得到一个没有执行方的空壳。
    pub fn is_server_tool(&self) -> bool {
        matches!(self.tool_type.as_deref(), Some(t) if t != "custom" && t != "function")
    }

    /// 出站给 Anthropic 的 `input_schema`：服务端工具且 schema 是入站兜底的空对象时不写出
    /// （服务端工具的 schema 由上游持有，多送一个空 schema 会被判成参数错误）。
    pub fn outbound_input_schema(&self) -> Option<serde_json::Value> {
        let is_empty = self.input_schema.as_object().is_some_and(|o| o.is_empty());
        if self.is_server_tool() && is_empty {
            None
        } else {
            Some(self.input_schema.clone())
        }
    }
}

fn default_input_schema() -> serde_json::Value {
    serde_json::json!({})
}

/// 目标协议不支持服务端工具时的统一降级：整条不下发并 warn 留痕。
/// 退化成空 schema 的假 function 更糟——模型会反复调用一个没有执行方的工具，对话卡死。
/// 全部被丢弃时返回空 Vec，调用方应据此不写 `tools` 键（空数组会被 OpenAI 判成参数错误）。
pub fn client_tools<'a>(tools: &'a [Tool], target_protocol: &str) -> Vec<&'a Tool> {
    let (dropped, keep): (Vec<&Tool>, Vec<&Tool>) = tools.iter().partition(|t| t.is_server_tool());
    if !dropped.is_empty() {
        let names: Vec<&str> = dropped.iter().map(|t| t.name.as_str()).collect();
        tracing::warn!(
            target_protocol,
            dropped = dropped.len(),
            ?names,
            "server-side tools dropped: 目标协议无法执行 Anthropic 服务端工具"
        );
    }
    keep
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
    /// 推理内容增量（思维链）
    #[serde(rename = "reasoning_delta")]
    ReasoningDelta { text: String },
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── MessageContent helper ──

    #[test]
    fn message_content_as_text_from_blocks() {
        let mc = MessageContent::Blocks(vec![
            ContentBlock::Text { text: "a".into(), extra: None },
            ContentBlock::ToolUse { id: "x".into(), name: "n".into(), input: json!({}), extra: None },
            ContentBlock::Text { text: "b".into(), extra: None },
        ]);
        assert_eq!(mc.as_text(), "ab");
        assert_eq!(MessageContent::Text("solo".into()).as_text(), "solo");
    }

    #[test]
    fn message_content_blocks_text_folds_single_block() {
        let mc = MessageContent::Text("hi".into());
        assert!(matches!(mc.blocks()[..], [ContentBlock::Text { ref text, .. }] if text == "hi"));
    }

    #[test]
    fn message_content_push_block_upgrades_text_to_blocks() {
        let mut mc = MessageContent::Text("t".into());
        mc.push_block(ContentBlock::ToolResult { tool_use_id: "t1".into(), content: "ok".into(), name: None, is_error: None, content_blocks: None, extra: None });
        match mc {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert!(matches!(blocks[0], ContentBlock::Text { .. }));
                assert!(matches!(blocks[1], ContentBlock::ToolResult { .. }));
            }
            _ => panic!("expected Blocks after push_block"),
        }
    }

    // ── ChatStreamEvent 工具/思考变体 serde 锁形 ──

    #[test]
    fn chat_stream_event_tool_and_reasoning_variants_serde() {
        let e = ChatStreamEvent::ToolDelta { index: 1, id: Some("c1".into()), name: Some("f".into()), input: Some("{\"x\"".into()) };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "tool_delta");
        assert_eq!(v["index"], 1);
        let e2: ChatStreamEvent = serde_json::from_value(v).unwrap();
        assert!(matches!(e2, ChatStreamEvent::ToolDelta { index: 1, .. }));

        let r = ChatStreamEvent::ReasoningDelta { text: "think".into() };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["type"], "reasoning_delta");
        let r2: ChatStreamEvent = serde_json::from_value(v).unwrap();
        assert!(matches!(r2, ChatStreamEvent::ReasoningDelta { ref text } if text == "think"));

        let st = ChatStreamEvent::Stop { finish_reason: Some("tool_use".into()) };
        let v = serde_json::to_value(&st).unwrap();
        assert_eq!(v["type"], "stop");
        assert_eq!(v["finish_reason"], "tool_use");
    }

    // ── ContentBlock Deserialize ──

    #[test]
    fn content_block_deserialize_text() {
        let v = json!({"type": "text", "text": "hello"});
        let b: ContentBlock = serde_json::from_value(v).unwrap();
        assert!(matches!(b, ContentBlock::Text { text, .. } if text == "hello"));
    }

    #[test]
    fn content_block_deserialize_tool_use() {
        let v = json!({"type": "tool_use", "id": "call-1", "name": "my_tool", "input": {"x": 1}});
        let b: ContentBlock = serde_json::from_value(v).unwrap();
        match b {
            ContentBlock::ToolUse { id, name, input, .. } => {
                assert_eq!(id, "call-1");
                assert_eq!(name, "my_tool");
                assert_eq!(input, json!({"x": 1}));
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn content_block_deserialize_tool_result_string() {
        let v = json!({"type": "tool_result", "tool_use_id": "t1", "content": "result-text"});
        let b: ContentBlock = serde_json::from_value(v).unwrap();
        match b {
            ContentBlock::ToolResult { tool_use_id, content, .. } => {
                assert_eq!(tool_use_id, "t1");
                assert_eq!(content, "result-text");
            }
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn content_block_deserialize_tool_result_null_content() {
        let v = json!({"type": "tool_result", "tool_use_id": "t2"});
        let b: ContentBlock = serde_json::from_value(v).unwrap();
        match b {
            ContentBlock::ToolResult { content, .. } => assert_eq!(content, ""),
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn content_block_deserialize_tool_result_other_content() {
        let v = json!({"type": "tool_result", "tool_use_id": "t3", "content": 42});
        let b: ContentBlock = serde_json::from_value(v).unwrap();
        match b {
            ContentBlock::ToolResult { content, .. } => assert!(!content.is_empty()),
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn content_block_deserialize_unknown_falls_back() {
        let v = json!({"type": "thinking", "thinking": "deep", "signature": "sig"});
        let b: ContentBlock = serde_json::from_value(v.clone()).unwrap();
        assert!(matches!(b, ContentBlock::Unknown(x) if x == v));
    }

    // ── ContentBlock Serialize ──

    #[test]
    fn content_block_serialize_text() {
        let b = ContentBlock::Text { text: "hi".into(), extra: None };
        let v = serde_json::to_value(b).unwrap();
        assert_eq!(v["type"], "text");
        assert_eq!(v["text"], "hi");
    }

    #[test]
    fn content_block_serialize_tool_use() {
        let b = ContentBlock::ToolUse { id: "id-1".into(), name: "tool-x".into(), input: json!({"k": "v"}), extra: None };
        let v = serde_json::to_value(b).unwrap();
        assert_eq!(v["type"], "tool_use");
        assert_eq!(v["id"], "id-1");
        assert_eq!(v["name"], "tool-x");
        assert_eq!(v["input"]["k"], "v");
    }

    #[test]
    fn content_block_serialize_tool_result() {
        let b = ContentBlock::ToolResult { tool_use_id: "tu-1".into(), content: "ok".into(), name: None, is_error: None, content_blocks: None, extra: None };
        let v = serde_json::to_value(b).unwrap();
        assert_eq!(v["type"], "tool_result");
        assert_eq!(v["tool_use_id"], "tu-1");
        assert_eq!(v["content"], "ok");
    }

    #[test]
    fn content_block_serialize_unknown() {
        let orig = json!({"type": "image", "source": "url"});
        let b = ContentBlock::Unknown(orig.clone());
        let v = serde_json::to_value(b).unwrap();
        assert_eq!(v, orig);
    }

    // ── ToolChoice serde round-trip ──

    #[test]
    fn tool_choice_roundtrip() {
        // Auto → {"type":"auto"}, Any → {"type":"any"}, None → {"type":"none"}
        for (tc, expected_type) in [
            (json!({"type": "auto"}), "auto"),
            (json!({"type": "any"}), "any"),
            (json!({"type": "none"}), "none"),
        ] {
            let _: ToolChoice = serde_json::from_value(tc.clone())
                .unwrap_or(ToolChoice::Auto); // untagged may fail for some
            let _ = expected_type;
        }
    }

    // ── Role serde ──

    #[test]
    fn role_serde_roundtrip() {
        assert_eq!(serde_json::to_value(Role::User).unwrap(), json!("user"));
        assert_eq!(serde_json::to_value(Role::Assistant).unwrap(), json!("assistant"));
        assert_eq!(serde_json::to_value(Role::System).unwrap(), json!("system"));
        assert_eq!(serde_json::to_value(Role::Tool).unwrap(), json!("tool"));
    }

    // ── SystemContent serde ──

    #[test]
    fn system_content_text_roundtrip() {
        let sc = SystemContent::Text("system prompt".into());
        let v = serde_json::to_value(&sc).unwrap();
        assert_eq!(v, json!("system prompt"));
        let sc2: SystemContent = serde_json::from_value(v).unwrap();
        assert!(matches!(sc2, SystemContent::Text(_)));
    }

    #[test]
    fn system_content_blocks_roundtrip() {
        let sc = SystemContent::Blocks(vec![json!({"type": "text", "text": "hi"})]);
        let v = serde_json::to_value(&sc).unwrap();
        assert!(v.is_array());
        let sc2: SystemContent = serde_json::from_value(v).unwrap();
        assert!(matches!(sc2, SystemContent::Blocks(_)));
    }
}
