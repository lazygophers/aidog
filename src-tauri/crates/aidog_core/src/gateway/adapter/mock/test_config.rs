use super::*;
use serde_json::json;

/// 最小 ChatRequest（messages 用 Anthropic 兼容结构，role 仅 User/Assistant/System/Tool）。
fn chat_req(messages: Vec<Message>) -> ChatRequest {
    ChatRequest {
        model: "mock-model".to_string(),
        messages,
        system: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        stream: None,
        tools: None,
        tool_choice: None,
        extra: None,
    }
}

fn empty_req() -> ChatRequest {
    chat_req(vec![])
}

// ─── 三层覆盖优先级 ──────────────────────────────────────

#[test]
fn empty_extra_yields_defaults() {
    let cfg = resolve_mock_config("", &empty_req(), &json!({}));
    let def = MockConfig::default();
    assert_eq!(cfg.status_code, def.status_code);
    assert_eq!(cfg.input_tokens, def.input_tokens);
    assert_eq!(cfg.output_tokens, def.output_tokens);
    assert_eq!(cfg.cache_tokens, def.cache_tokens);
    assert_eq!(cfg.response_text, def.response_text);
    assert_eq!(cfg.error_mode, "none");
    assert_eq!(cfg.chunk_count, def.chunk_count);
}

#[test]
fn extra_layer_applied() {
    let extra = r#"{"mock":{"input_tokens":11,"output_tokens":22,"cache_tokens":33,"status_code":201,"response_text":"from-extra","error_mode":"http_error","chunk_count":3}}"#;
    let cfg = resolve_mock_config(extra, &empty_req(), &json!({}));
    assert_eq!(cfg.input_tokens, 11);
    assert_eq!(cfg.output_tokens, 22);
    assert_eq!(cfg.cache_tokens, 33);
    assert_eq!(cfg.status_code, 201);
    assert_eq!(cfg.response_text, "from-extra");
    assert_eq!(cfg.error_mode, "http_error");
    assert_eq!(cfg.chunk_count, 3);
}

#[test]
fn message_role_layer_overrides_extra() {
    // 第二层走原始 body messages 扫描（自定义 role 名当字段 key）。
    let extra = r#"{"mock":{"input_tokens":11,"output_tokens":22}}"#;
    let body = json!({
        "messages": [
            {"role": "input_tokens", "content": "555"},
            {"role": "status_code", "content": "503"},
        ]
    });
    let cfg = resolve_mock_config(extra, &empty_req(), &body);
    // 被 message role 覆盖
    assert_eq!(cfg.input_tokens, 555);
    assert_eq!(cfg.status_code, 503);
    // 未被覆盖字段回退 extra
    assert_eq!(cfg.output_tokens, 22);
}

#[test]
fn body_mock_overrides_all_layers() {
    let extra = r#"{"mock":{"input_tokens":11,"output_tokens":22,"status_code":201}}"#;
    let body = json!({
        "messages": [
            {"role": "input_tokens", "content": "555"},
        ],
        "mock": { "input_tokens": 999, "status_code": 429 }
    });
    let cfg = resolve_mock_config(extra, &empty_req(), &body);
    // body.mock 最高优先级
    assert_eq!(cfg.input_tokens, 999);
    assert_eq!(cfg.status_code, 429);
    // 未在 body.mock 出现的字段回退（message 层无，回退 extra）
    assert_eq!(cfg.output_tokens, 22);
}

#[test]
fn per_field_independent_fallback() {
    // body 只覆盖 output_tokens；input 回退 message 层；cache 回退 extra。
    let extra = r#"{"mock":{"input_tokens":1,"output_tokens":2,"cache_tokens":3}}"#;
    let body = json!({
        "messages": [ {"role": "input_tokens", "content": "100"} ],
        "mock": { "output_tokens": 200 }
    });
    let cfg = resolve_mock_config(extra, &empty_req(), &body);
    assert_eq!(cfg.input_tokens, 100); // message 层
    assert_eq!(cfg.output_tokens, 200); // body 层
    assert_eq!(cfg.cache_tokens, 3); // extra 兜底
}

/// body.mock 覆盖所有字段
#[test]
fn body_mock_all_fields() {
    let body = json!({
        "mock": {
            "status_code": 503,
            "delay_ms": 42,
            "stream_override": false,
            "response_text": "body-text",
            "finish_reason": "max_tokens",
            "input_tokens": 7,
            "output_tokens": 8,
            "cache_tokens": 9,
            "error_mode": "rate_limit_429",
            "chunk_count": 2
        }
    });
    let cfg = resolve_mock_config("", &empty_req(), &body);
    assert_eq!(cfg.status_code, 503);
    assert_eq!(cfg.delay_ms, 42);
    assert_eq!(cfg.stream_override, Some(false));
    assert_eq!(cfg.response_text, "body-text");
    assert_eq!(cfg.finish_reason, "max_tokens");
    assert_eq!(cfg.input_tokens, 7);
    assert_eq!(cfg.output_tokens, 8);
    assert_eq!(cfg.cache_tokens, 9);
    assert_eq!(cfg.error_mode, "rate_limit_429");
    assert_eq!(cfg.chunk_count, 2);
}

/// apply_field branches: response_text + error_mode via message role
#[test]
fn apply_field_response_text_and_error_mode_via_role() {
    let body = json!({
        "messages": [
            {"role": "response_text", "content": "from-role"},
            {"role": "error_mode", "content": "timeout"},
            {"role": "delay_ms", "content": "999"},
            {"role": "output_tokens", "content": "77"},
            {"role": "cache_tokens", "content": "3"},
        ]
    });
    let cfg = resolve_mock_config("", &empty_req(), &body);
    assert_eq!(cfg.response_text, "from-role");
    assert_eq!(cfg.error_mode, "timeout");
    assert_eq!(cfg.delay_ms, 999);
    assert_eq!(cfg.output_tokens, 77);
    assert_eq!(cfg.cache_tokens, 3);
}

/// Blocks content in message role scanning
#[test]
fn message_blocks_content_concatenated_for_role_field() {
    let req = chat_req(vec![Message {
        role: Role::User,
        content: MessageContent::Blocks(vec![
            ContentBlock::Text { text: "10".to_string() },
        ]),
    }]);
    // role=user does not match any field; no change to defaults
    let cfg = resolve_mock_config("", &req, &json!({}));
    let def = MockConfig::default();
    assert_eq!(cfg.input_tokens, def.input_tokens);
}

// ─── error_mode 字段解析（语义在 handle_mock，纯函数侧验证配置可控） ──

#[test]
fn error_mode_variants_parse() {
    for mode in ["none", "http_error", "rate_limit_429", "timeout"] {
        let extra = format!(r#"{{"mock":{{"error_mode":"{mode}"}}}}"#);
        let cfg = resolve_mock_config(&extra, &empty_req(), &json!({}));
        assert_eq!(cfg.error_mode, mode);
    }
}

#[test]
fn stream_override_and_delay_parse() {
    let extra = r#"{"mock":{"stream_override":true,"delay_ms":1500}}"#;
    let cfg = resolve_mock_config(extra, &empty_req(), &json!({}));
    assert_eq!(cfg.stream_override, Some(true));
    assert_eq!(cfg.delay_ms, 1500);
}

#[test]
fn parsed_message_role_layer_for_standard_roles() {
    // 标准 Role（user/assistant/system/tool）经 ChatRequest.messages 扫描，
    // 这些 role 名不匹配任何 mock 字段，不应改动配置。
    let req = chat_req(vec![Message {
        role: Role::User,
        content: MessageContent::Text("ignored".to_string()),
    }]);
    let cfg = resolve_mock_config("", &req, &json!({}));
    let def = MockConfig::default();
    assert_eq!(cfg.input_tokens, def.input_tokens);
    assert_eq!(cfg.output_tokens, def.output_tokens);
    assert_eq!(cfg.status_code, def.status_code);
    assert_eq!(cfg.response_text, def.response_text);
}
