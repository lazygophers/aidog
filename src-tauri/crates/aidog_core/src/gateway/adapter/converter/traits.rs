//! 协议转换统一接口

use crate::gateway::adapter::types::*;
use serde_json::Value;

/// 协议转换器 trait：定义各协议的入站解析和出站序列化接口
pub trait ProtocolConverter {
    /// 协议标识
    fn protocol_name(&self) -> &'static str;

    /// 解析入站请求 body → ChatRequest
    fn parse_incoming(&self, body: &[u8]) -> Result<ChatRequest, String>;

    /// 序列化 ChatRequest → 该协议请求格式 (body_json, api_path)
    fn serialize_request(&self, req: &ChatRequest) -> Result<(Value, String), String>;

    /// 解析流式响应 SSE → ChatStreamEvent
    fn parse_sse(&self, chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String>;

    /// 序列化 ChatStreamEvent → 客户端 SSE 格式
    fn to_client_sse(&self, event: &ChatStreamEvent) -> Result<String, String>;

    /// 解析非流式响应（返回 converter::response::NonStreamResponse）
    fn parse_response(&self, body: &[u8]) -> Result<super::response::NonStreamResponse, String>;
}
