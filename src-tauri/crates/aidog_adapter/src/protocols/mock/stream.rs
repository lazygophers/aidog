//! mock 流式 SSE 序列构造（复用 converter::to_client_sse_stateful 按协议格式化）。

use super::config::MockConfig;
use crate::converter;
use crate::types::*;
use aidog_db::models::Protocol;

/// 生成 mock 流式 SSE 字符串序列：Start → N×Delta → Stop。
/// 整条流共用一份 ClientSseState（issue 70 评审项 3：Responses 等有状态协议不得逐事件重建状态）。
pub fn build_sse_chunks(cfg: &MockConfig, source_protocol: &Protocol, model: &str) -> Vec<String> {
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

    let mut state = converter::ClientSseState::default();
    events
        .iter()
        .filter_map(|e| converter::to_client_sse_stateful(e, &mut state, source_protocol, model))
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
mod tests {
    use super::*;

    #[test]
    fn responses_client_chunks_single_created_via_state() {
        // issue 70 评审项 3：Responses 多事件流必须整体走一份状态机——
        // 整个 mock 流恰好一个 response.created，且以 response.completed 收尾。
        let cfg = MockConfig {
            response_text: "你好世界".into(),
            chunk_count: 2,
            ..Default::default()
        };
        let chunks = build_sse_chunks(&cfg, &Protocol::OpenAIResponses, "mock-model");
        let all = chunks.join("");
        assert_eq!(
            all.matches("event: response.created").count(),
            1,
            "mock Responses 流须恰好 1 个 response.created: {all:?}"
        );
        assert!(all.contains("response.completed"), "缺 response.completed: {all:?}");
        assert!(all.contains("response.output_text.done"), "缺 output_text.done: {all:?}");
        // sequence_number 跨 chunk 连续
        let mut seqs: Vec<u64> = Vec::new();
        for line in all.lines() {
            if let Some(d) = line.strip_prefix("data: ")
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(d)
                && let Some(s) = v.get("sequence_number").and_then(|x| x.as_u64())
            {
                seqs.push(s);
            }
        }
        for w in seqs.windows(2) {
            assert_eq!(w[1], w[0] + 1, "sequence_number 不连续: {seqs:?}");
        }
    }

    #[test]
    fn openai_client_chunks_unchanged() {
        let cfg = MockConfig {
            response_text: "hello".into(),
            chunk_count: 1,
            ..Default::default()
        };
        let chunks = build_sse_chunks(&cfg, &Protocol::OpenAI, "mock-model");
        let all = chunks.join("");
        assert!(all.contains("chat.completion.chunk"), "openai 回归: {all:?}");
        assert!(all.contains("[DONE]"), "openai 回归: {all:?}");
    }
}

#[cfg(test)]
#[path = "test_stream.rs"]
mod test_stream;
