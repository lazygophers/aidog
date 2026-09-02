//! Mock 平台类型：本地生成可控假响应，不转发真实上游。
//!
//! 配置三层覆盖（逐字段，优先级高 → 低）：
//! 1. 请求 body 顶层 `mock` 对象
//! 2. 请求 messages 的 role 映射（role 当 key，content 当 value）
//! 3. platform.extra JSON 的 `mock` 对象（兜底默认）

use crate::converter::NonStreamResponse;
use crate::converter::traits::ProtocolConverter;
use crate::types::*;
use serde_json::Value;

mod config;
mod response;
mod stream;

#[allow(unused_imports)]
pub use config::MockConfig;
pub use config::resolve_mock_config;
pub use response::{build_error_body, build_response};
pub use stream::build_sse_chunks;

/// Mock 协议转换器实现
pub struct MockConverter;

impl ProtocolConverter for MockConverter {
    fn protocol_name(&self) -> &'static str {
        "mock"
    }

    fn parse_incoming(&self, body: &[u8]) -> Result<ChatRequest, String> {
        serde_json::from_slice(body).map_err(|e| format!("Mock parse error: {}", e))
    }

    fn serialize_request(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        // Mock 不需要真实上游请求
        Ok((Value::Null, String::new()))
    }

    fn parse_sse(&self, _chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        // Mock SSE 由本地生成
        Ok(Vec::new())
    }

    fn to_client_sse(&self, event: &ChatStreamEvent) -> Result<String, String> {
        // Mock 直接输出事件
        Ok(format!(
            "data: {}\n\n",
            serde_json::to_string(event).unwrap_or_default()
        ))
    }

    fn parse_response(&self, body: &[u8]) -> Result<NonStreamResponse, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("Mock response parse error: {}", e))?;
        Ok(NonStreamResponse {
            id: value
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            model: value
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            text: value
                .get("content")
                .and_then(|v| v.as_str())
                .map(String::from),
            tool_uses: Vec::new(),
            stop_reason: value
                .get("finish_reason")
                .and_then(|v| v.as_str())
                .unwrap_or("end_turn")
                .to_string(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            reasoning: None,
        })
    }
}
