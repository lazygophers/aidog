//! 协议适配器模块

pub mod converter;
pub mod dsml;
pub mod platform;
pub mod protocols;
pub mod quota;
pub mod reasoning_tags;
pub mod thinking;
pub mod thinking_strip;
pub mod types;

// 平台专属模块（原 66 个平台目录已删至 3 个：mock=模型测试/代理假上游，devin/newapi=quota 特例）
pub mod devin;
pub mod mock;
pub mod newapi;

pub use converter::{
    AnthropicSseState, ClientSseState, convert_request, convert_response, parse_incoming_request,
    parse_sse, decision_api_path, parse_upstream_sse, passthrough_api_path, response_model,
    response_model_or, split_stream_inline_reasoning, to_client_sse, to_client_sse_stateful,
};
pub use protocols::*;
pub use protocols::gemini::parse_gemini_usage_metadata;
pub use dsml::{DsmlSseRewriter, convert_dsml_in_body};
pub use reasoning_tags::InlineReasoningSplitter;
pub use thinking_strip::{SseThinkingStripper, strip_thinking_in_body};
pub use types::*;

/// reqwest is built with `rustls-no-provider`: the process entry points install the ring
/// provider at startup, so the unit-test binary installs it before any test runs.
#[cfg(test)]
ctor::declarative::ctor! {
    #[ctor(unsafe)]
    fn install_test_crypto_provider() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}
