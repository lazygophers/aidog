use super::*;

// ─── SSE build_sse_chunks 序列 ───────────────────────────

#[test]
fn sse_anthropic_start_delta_stop_sequence() {
    let cfg = MockConfig {
        response_text: "abcdef".to_string(),
        chunk_count: 3,
        finish_reason: "end_turn".to_string(),
        ..MockConfig::default()
    };
    let chunks = build_sse_chunks(&cfg, &Protocol::Anthropic, "claude-x");
    // 1 Start + 3 Delta + 1 Stop(closes) + 1 flush(message_delta+message_stop) = 6
    // issue 71：Stop 只关块，终帧压到流末 flush
    assert_eq!(chunks.len(), 6);
    assert!(chunks[0].contains("message_start"));
    assert!(chunks[1].contains("content_block_delta"));
    assert!(chunks[2].contains("content_block_delta"));
    assert!(chunks[3].contains("content_block_delta"));
    assert!(chunks[4].contains("content_block_stop"));
    assert!(chunks[5].contains("message_stop"));
    // chunk_count=3，6 字符 → 每块 2 字符
    assert!(chunks[1].contains("\"text\":\"ab\""));
    assert!(chunks[2].contains("\"text\":\"cd\""));
    assert!(chunks[3].contains("\"text\":\"ef\""));
}

#[test]
fn sse_openai_sequence_has_done() {
    let cfg = MockConfig {
        response_text: "xy".to_string(),
        chunk_count: 2,
        ..MockConfig::default()
    };
    let chunks = build_sse_chunks(&cfg, &Protocol::OpenAI, "gpt-x");
    // Start + 2 Delta + Stop（finish+usage+[DONE]；issue 71）
    assert_eq!(chunks.len(), 4);
    assert!(chunks[0].contains("chat.completion.chunk"));
    assert!(chunks.last().unwrap().contains("[DONE]"));
}

#[test]
fn sse_chunk_count_capped_to_text_len() {
    // chunk_count 大于文本长度时按字符数封顶。
    let cfg = MockConfig {
        response_text: "ab".to_string(),
        chunk_count: 10,
        ..MockConfig::default()
    };
    let chunks = build_sse_chunks(&cfg, &Protocol::Anthropic, "m");
    // 2 字符 → 最多 2 Delta：Start + 2 Delta + Stop(closes) + flush = 5（issue 71）
    assert_eq!(chunks.len(), 5);
}

#[test]
fn sse_empty_text_yields_single_delta() {
    let cfg = MockConfig {
        response_text: String::new(),
        chunk_count: 5,
        ..MockConfig::default()
    };
    let chunks = build_sse_chunks(&cfg, &Protocol::Anthropic, "m");
    // 空文本 → split_text 返单块 → Start + 1 Delta + Stop(closes) + flush = 4（issue 71）
    assert_eq!(chunks.len(), 4);
}

#[test]
fn split_text_basic() {
    assert_eq!(split_text("abcd", 2), vec!["ab", "cd"]);
    assert_eq!(split_text("abc", 2), vec!["ab", "c"]);
    assert_eq!(split_text("", 5), vec![String::new()]);
    // n=0 视为 1 块
    assert_eq!(split_text("abc", 0), vec!["abc"]);
}
