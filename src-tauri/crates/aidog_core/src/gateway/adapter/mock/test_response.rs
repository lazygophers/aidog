use super::*;

fn cfg_with_tokens() -> MockConfig {
    MockConfig {
        input_tokens: 100,
        output_tokens: 50,
        cache_tokens: 7,
        response_text: "hello-mock".to_string(),
        finish_reason: "end_turn".to_string(),
        ..MockConfig::default()
    }
}

// ─── 5 协议非流式 build_response shape ─────────────────────

#[test]
fn build_response_anthropic_shape() {
    let v = build_response(&cfg_with_tokens(), "anthropic", "claude-x");
    assert_eq!(v["type"], "message");
    assert_eq!(v["role"], "assistant");
    assert_eq!(v["model"], "claude-x");
    assert_eq!(v["content"][0]["type"], "text");
    assert_eq!(v["content"][0]["text"], "hello-mock");
    assert_eq!(v["stop_reason"], "end_turn");
    assert_eq!(v["usage"]["input_tokens"], 100);
    assert_eq!(v["usage"]["output_tokens"], 50);
    assert_eq!(v["usage"]["cache_read_input_tokens"], 7);
}

#[test]
fn build_response_openai_shape() {
    let v = build_response(&cfg_with_tokens(), "openai", "gpt-x");
    assert_eq!(v["object"], "chat.completion");
    assert_eq!(v["model"], "gpt-x");
    assert_eq!(v["choices"][0]["message"]["role"], "assistant");
    assert_eq!(v["choices"][0]["message"]["content"], "hello-mock");
    // end_turn → stop 归一化
    assert_eq!(v["choices"][0]["finish_reason"], "stop");
    assert_eq!(v["usage"]["prompt_tokens"], 100);
    assert_eq!(v["usage"]["completion_tokens"], 50);
    assert_eq!(v["usage"]["total_tokens"], 150);
    assert_eq!(v["usage"]["prompt_tokens_details"]["cached_tokens"], 7);
}

#[test]
fn build_response_openai_completions_shape() {
    let v = build_response(&cfg_with_tokens(), "openai_completions", "gpt-x");
    assert_eq!(v["object"], "text_completion");
    assert_eq!(v["choices"][0]["text"], "hello-mock");
    assert_eq!(v["choices"][0]["index"], 0);
    assert_eq!(v["choices"][0]["finish_reason"], "stop");
    assert_eq!(v["usage"]["prompt_tokens"], 100);
    assert_eq!(v["usage"]["completion_tokens"], 50);
    assert_eq!(v["usage"]["total_tokens"], 150);
}

#[test]
fn build_response_openai_responses_shape() {
    let v = build_response(&cfg_with_tokens(), "openai_responses", "gpt-x");
    assert_eq!(v["object"], "response");
    assert_eq!(v["status"], "completed");
    assert_eq!(v["output"][0]["type"], "message");
    assert_eq!(v["output"][0]["content"][0]["type"], "output_text");
    assert_eq!(v["output"][0]["content"][0]["text"], "hello-mock");
    assert_eq!(v["usage"]["input_tokens"], 100);
    assert_eq!(v["usage"]["output_tokens"], 50);
    assert_eq!(v["usage"]["total_tokens"], 150);
}

#[test]
fn build_response_gemini_shape() {
    let v = build_response(&cfg_with_tokens(), "gemini", "gemini-x");
    assert_eq!(v["candidates"][0]["content"]["parts"][0]["text"], "hello-mock");
    assert_eq!(v["candidates"][0]["content"]["role"], "model");
    assert_eq!(v["candidates"][0]["finishReason"], "STOP");
    assert_eq!(v["usageMetadata"]["promptTokenCount"], 100);
    assert_eq!(v["usageMetadata"]["candidatesTokenCount"], 50);
    assert_eq!(v["usageMetadata"]["cachedContentTokenCount"], 7);
    assert_eq!(v["usageMetadata"]["totalTokenCount"], 150);
}

#[test]
fn build_response_unknown_protocol_falls_back_anthropic() {
    let v = build_response(&cfg_with_tokens(), "weird-proto", "m");
    assert_eq!(v["type"], "message");
    assert_eq!(v["content"][0]["text"], "hello-mock");
}

// ─── build_error_body 各协议 shape ───────────────────────

#[test]
fn error_body_anthropic_shape() {
    let v = build_error_body("anthropic", 500, "boom");
    assert_eq!(v["type"], "error");
    assert_eq!(v["error"]["type"], "mock_error");
    assert_eq!(v["error"]["message"], "boom");
}

#[test]
fn error_body_openai_shape() {
    for proto in ["openai", "openai_responses", "openai_completions"] {
        let v = build_error_body(proto, 429, "rate limited");
        assert_eq!(v["error"]["message"], "rate limited", "proto {proto}");
        assert_eq!(v["error"]["type"], "mock_error", "proto {proto}");
        assert_eq!(v["error"]["code"], 429, "proto {proto}");
    }
}

#[test]
fn error_body_gemini_shape() {
    let v = build_error_body("gemini", 503, "unavailable");
    assert_eq!(v["error"]["code"], 503);
    assert_eq!(v["error"]["message"], "unavailable");
    assert_eq!(v["error"]["status"], "MOCK_ERROR");
}
