//! mock 配置解析：三层覆盖（body.mock > message role > platform.extra）。

use serde::Deserialize;
use serde_json::Value;

use crate::gateway::adapter::types::*;

/// mock 场景配置。全字段 `#[serde(default)]`，空 extra → 全默认。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MockConfig {
    pub status_code: u16,
    pub delay_ms: u64,
    /// null=跟随请求 stream；Some(true/false)=强制
    pub stream_override: Option<bool>,
    pub response_text: String,
    pub finish_reason: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    /// none | http_error | rate_limit_429 | timeout
    pub error_mode: String,
    /// 流式时 response_text 切 N 块
    pub chunk_count: usize,
}

impl Default for MockConfig {
    fn default() -> Self {
        MockConfig {
            status_code: 200,
            delay_ms: 0,
            stream_override: None,
            response_text: "Hello from mock".to_string(),
            finish_reason: "end_turn".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cache_tokens: 0,
            error_mode: "none".to_string(),
            chunk_count: 5,
        }
    }
}

/// 解析最终生效的 mock 配置：extra 默认 → message role 覆盖 → body.mock 覆盖。
/// 每字段独立覆盖（缺省回退下层）。
pub fn resolve_mock_config(extra: &str, chat_req: &ChatRequest, body_json: &Value) -> MockConfig {
    // 第三层（兜底）：platform.extra 的 .mock
    let mut cfg: MockConfig = serde_json::from_str::<Value>(extra)
        .ok()
        .and_then(|v| v.get("mock").cloned())
        .and_then(|m| serde_json::from_value(m).ok())
        .unwrap_or_default();

    // 第二层：messages 的 role 映射（role ∈ 已知字段名时，content 为值）
    for msg in &chat_req.messages {
        let role = format!("{:?}", msg.role).to_lowercase();
        let content = match &msg.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        };
        apply_field(&mut cfg, &role, &content);
    }
    // role 映射也兼容原始 body messages（部分自定义 role 不在 Role enum 内，
    // 会被 parse_incoming_request 归一化丢失），直接从原始 body 再扫一遍。
    if let Some(messages) = body_json.get("messages").and_then(|v| v.as_array()) {
        for m in messages {
            if let (Some(role), Some(content)) = (
                m.get("role").and_then(|v| v.as_str()),
                m.get("content").and_then(|v| v.as_str()),
            ) {
                apply_field(&mut cfg, role, content);
            }
        }
    }

    // 第一层（最高）：body 顶层 mock 对象
    if let Some(mock_obj) = body_json.get("mock").and_then(|v| v.as_object()) {
        if let Some(v) = mock_obj.get("status_code").and_then(|v| v.as_u64()) {
            cfg.status_code = v as u16;
        }
        if let Some(v) = mock_obj.get("delay_ms").and_then(|v| v.as_u64()) {
            cfg.delay_ms = v;
        }
        if let Some(v) = mock_obj.get("stream_override").and_then(|v| v.as_bool()) {
            cfg.stream_override = Some(v);
        }
        if let Some(v) = mock_obj.get("response_text").and_then(|v| v.as_str()) {
            cfg.response_text = v.to_string();
        }
        if let Some(v) = mock_obj.get("finish_reason").and_then(|v| v.as_str()) {
            cfg.finish_reason = v.to_string();
        }
        if let Some(v) = mock_obj.get("input_tokens").and_then(|v| v.as_i64()) {
            cfg.input_tokens = v as i32;
        }
        if let Some(v) = mock_obj.get("output_tokens").and_then(|v| v.as_i64()) {
            cfg.output_tokens = v as i32;
        }
        if let Some(v) = mock_obj.get("cache_tokens").and_then(|v| v.as_i64()) {
            cfg.cache_tokens = v as i32;
        }
        if let Some(v) = mock_obj.get("error_mode").and_then(|v| v.as_str()) {
            cfg.error_mode = v.to_string();
        }
        if let Some(v) = mock_obj.get("chunk_count").and_then(|v| v.as_u64()) {
            cfg.chunk_count = v as usize;
        }
    }

    cfg
}

/// 按 role(=字段名) / content(=值) 覆盖单个字段。
fn apply_field(cfg: &mut MockConfig, field: &str, value: &str) {
    match field {
        "input_tokens" => {
            if let Ok(v) = value.trim().parse() {
                cfg.input_tokens = v;
            }
        }
        "output_tokens" => {
            if let Ok(v) = value.trim().parse() {
                cfg.output_tokens = v;
            }
        }
        "cache_tokens" => {
            if let Ok(v) = value.trim().parse() {
                cfg.cache_tokens = v;
            }
        }
        "status_code" => {
            if let Ok(v) = value.trim().parse() {
                cfg.status_code = v;
            }
        }
        "delay_ms" => {
            if let Ok(v) = value.trim().parse() {
                cfg.delay_ms = v;
            }
        }
        "response_text" => {
            cfg.response_text = value.to_string();
        }
        "error_mode" => {
            cfg.error_mode = value.trim().to_string();
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "test_config.rs"]
mod test_config;
