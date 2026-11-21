//! 辅助模型请求单测：入站识别、endpoint 选择、usage 提取。端到端见 test_integration.rs。

use super::*;

// audio spec 的 multipart / binary_response 是编译期常量，const 块断言（运行期断言被 clippy 拒）。
const _: () = {
    assert!(AUDIO_SPEECH.binary_response && !AUDIO_SPEECH.multipart);
    assert!(!AUDIO_TRANSCRIPTIONS.binary_response && AUDIO_TRANSCRIPTIONS.multipart);
    assert!(!AUDIO_TRANSLATIONS.binary_response && AUDIO_TRANSLATIONS.multipart);
};

#[test]
fn embedding_endpoint_suffix_match() {
    for p in ["/v1/embeddings", "/embeddings", "/proxy/v1/embeddings", "/v1/embeddings/"] {
        assert!(is_embedding_endpoint(p), "{p}");
    }
    for p in ["/v1/embeddingsfoo", "/v1/embeddings/x", "/v1/chat/completions", "/v1/myembeddings"] {
        assert!(!is_embedding_endpoint(p), "{p}");
    }
}

#[test]
fn embedding_usage_only_input() {
    let (i, o) = (EMBEDDING.extract_usage)(r#"{"usage":{"prompt_tokens":8,"total_tokens":8}}"#);
    assert_eq!((i, o), (8, 0));
    assert_eq!((EMBEDDING.extract_usage)("not json"), (0, 0));
    assert_eq!((EMBEDDING.extract_usage)(r#"{"data":[]}"#), (0, 0));
}

#[test]
fn openai_family_endpoint_priority() {
    let ep = |protocol, url: &str| super::models::PlatformEndpoint {
        protocol,
        base_url: url.into(),
        client_type: "default".into(),
        coding_plan: false,
    };
    let eps = vec![
        ep(Protocol::Anthropic, "a"),
        ep(Protocol::OpenAIResponses, "r"),
        ep(Protocol::OpenAI, "o"),
    ];
    assert_eq!(select_endpoint_for_openai_family(&eps).unwrap().base_url, "o");
    assert!(select_endpoint_for_openai_family(&eps[..1]).is_none());
}

#[test]
fn rerank_endpoint_suffix_match() {
    for p in ["/v1/rerank", "/v2/rerank", "/rerank", "/proxy/v1/rerank", "/v1/rerank/"] {
        assert!(is_rerank_endpoint(p), "{p}");
    }
    for p in ["/v1/rerankx", "/v1/rerank/x", "/v1/embeddings", "/v1/myrerank"] {
        assert!(!is_rerank_endpoint(p), "{p}");
    }
    assert!(!is_embedding_endpoint("/v1/rerank"));
}

/// 样例取自一手文档（spec Further Notes）：Jina / SiliconFlow / Cohere / 空。
#[test]
fn rerank_usage_three_shapes() {
    let u = RERANK.extract_usage;
    assert_eq!(u(r#"{"model":"jina-reranker-v2","usage":{"total_tokens":815},"results":[]}"#), (815, 0));
    assert_eq!(u(r#"{"id":"x","results":[],"meta":{"tokens":{"input_tokens":12,"output_tokens":3}}}"#), (12, 3));
    assert_eq!(u(r#"{"id":"x","results":[],"meta":{"billed_units":{"input_tokens":7,"output_tokens":2}}}"#), (7, 2));
    // Cohere 按次计量：token 记 0
    assert_eq!(u(r#"{"id":"x","results":[],"meta":{"billed_units":{"search_units":1}}}"#), (0, 0));
    assert_eq!(u(""), (0, 0));
    assert_eq!(u("not json"), (0, 0));
    assert_eq!(u(r#"{"results":[]}"#), (0, 0));
}

#[test]
fn rerank_search_units_defaults_to_one() {
    let f = RERANK.extract_units.expect("rerank has units");
    let req = Value::Null;
    assert_eq!(f(&req, r#"{"meta":{"billed_units":{"search_units":3}}}"#), 3);
    assert_eq!(f(&req, r#"{"usage":{"total_tokens":9}}"#), 1);
    assert_eq!(f(&req, "not json"), 1);
    assert!(EMBEDDING.extract_units.is_none());
}

/// audio spec 形状与计量提取器（issue #167）。
#[test]
fn audio_spec_shape_and_extractors() {
    for p in ["/v1/audio/speech", "/proxy/v1/audio/speech", "/v1/audio/speech/"] {
        assert!(audio_spec(p).is_some(), "{p}");
    }
    for p in ["/v1/audio/speechx", "/v1/audio", "/v1/chat/completions", "/v1/audio/transcribe"] {
        assert!(audio_spec(p).is_none(), "{p}");
    }
    assert_eq!(audio_spec("/v1/audio/speech").map(|s| s.label), Some("audio_speech"));
    assert_eq!(audio_spec("/v1/audio/transcriptions").map(|s| s.label), Some("audio_transcriptions"));
    assert_eq!(audio_spec("/v1/audio/translations").map(|s| s.label), Some("audio_translations"));
    assert_eq!((AUDIO_SPEECH.extract_usage)(""), (0, 0));

    let sf = AUDIO_SPEECH.extract_units.expect("speech has units");
    assert_eq!(sf(&serde_json::json!({"input":"你好世界hello"}), ""), 9);
    assert_eq!(sf(&serde_json::json!({"input":""}), ""), 0);
    assert_eq!(sf(&Value::Null, ""), 0);

    let af = AUDIO_TRANSCRIPTIONS.extract_units.expect("asr has units");
    let req = Value::Null;
    assert_eq!(af(&req, r#"{"text":"hi","duration":12.4}"#), 12);
    assert_eq!(af(&req, r#"{"text":"hi"}"#), 0);
    assert_eq!(af(&req, "not json"), 0);
}
