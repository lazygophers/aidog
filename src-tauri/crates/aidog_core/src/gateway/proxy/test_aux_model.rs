//! 辅助模型请求单测：入站识别、endpoint 选择、usage 提取。端到端见 test_integration.rs。

use super::*;

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
