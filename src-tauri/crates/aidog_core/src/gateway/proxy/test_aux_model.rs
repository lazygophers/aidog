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
    assert_eq!(f(r#"{"meta":{"billed_units":{"search_units":3}}}"#), 3);
    assert_eq!(f(r#"{"usage":{"total_tokens":9}}"#), 1);
    assert_eq!(f("not json"), 1);
    assert!(EMBEDDING.extract_units.is_none());
}
