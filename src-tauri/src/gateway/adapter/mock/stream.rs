//! mock 流式 SSE 序列构造（复用 converter::to_client_sse 转协议格式）。

use super::config::MockConfig;
use crate::gateway::adapter::converter;
use crate::gateway::adapter::types::*;

/// 生成 mock 流式 SSE 字符串序列：Start → N×Delta → Stop。
/// 复用 `to_client_sse` 按 source_protocol 转格式。
pub fn build_sse_chunks(cfg: &MockConfig, source_protocol: &str, model: &str) -> Vec<String> {
    let id = format!("mock-{}", uuid::Uuid::new_v4().simple());
    let mut events: Vec<ChatStreamEvent> = Vec::new();
    events.push(ChatStreamEvent::Start {
        id,
        model: model.to_string(),
    });
    for piece in split_text(&cfg.response_text, cfg.chunk_count) {
        events.push(ChatStreamEvent::Delta { text: piece });
    }
    events.push(ChatStreamEvent::Stop {
        finish_reason: Some(cfg.finish_reason.clone()),
    });

    events
        .iter()
        .filter_map(|e| converter::to_client_sse(e, source_protocol, model))
        .collect()
}

/// 将文本切成 n 块（n<=1 或文本空则单块）。
fn split_text(text: &str, n: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let chars: Vec<char> = text.chars().collect();
    let n = n.max(1).min(chars.len());
    let chunk_size = chars.len().div_ceil(n);
    chars
        .chunks(chunk_size)
        .map(|c| c.iter().collect::<String>())
        .collect()
}

#[cfg(test)]
#[path = "test_stream.rs"]
mod test_stream;
