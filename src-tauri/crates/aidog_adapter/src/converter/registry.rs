//! 协议转换器注册表

use super::traits::ProtocolConverter;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::protocols::{
    anthropic::AnthropicConverter,
    gemini::GeminiConverter,
    openai::OpenAIConverter,
    openai_completions::OpenAICompletionsConverter,
    openai_responses::OpenAIResponsesConverter,
    mock::MockConverter,
};

/// 协议转换器注册表（单例）
pub fn converter_registry() -> &'static HashMap<String, Box<dyn ProtocolConverter>> {
    static REGISTRY: OnceLock<HashMap<String, Box<dyn ProtocolConverter>>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert("anthropic".to_string(), Box::new(AnthropicConverter));
        map.insert("openai".to_string(), Box::new(OpenAIConverter));
        map.insert("openai_responses".to_string(), Box::new(OpenAIResponsesConverter));
        map.insert("openai_completions".to_string(), Box::new(OpenAICompletionsConverter));
        map.insert("gemini".to_string(), Box::new(GeminiConverter));
        map.insert("mock".to_string(), Box::new(MockConverter));
        map
    })
}

/// 获取指定协议的转换器
pub fn get_converter(protocol: &str) -> Option<&'static dyn ProtocolConverter> {
    converter_registry().get(protocol).map(|b| b.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_converter_returns_registry_entries() {
        for name in ["anthropic", "openai", "openai_responses", "openai_completions", "gemini", "mock"] {
            assert!(get_converter(name).is_some(), "missing converter: {name}");
        }
        assert!(get_converter("nope").is_none());
    }

    #[test]
    fn responses_converter_roundtrip_via_trait() {
        let conv = get_converter("openai_responses").unwrap();
        let body = serde_json::json!({
            "model": "gpt-5",
            "instructions": "be brief",
            "input": [
                {"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}
            ]
        });
        let req = conv
            .parse_incoming(serde_json::to_vec(&body).unwrap())
            .expect("parse_incoming");
        let (out, path) = conv.serialize_request(&req).expect("serialize_request");
        assert_eq!(path, "/v1/responses");
        assert_eq!(out["instructions"], "be brief");
    }

    #[test]
    fn completions_and_gemini_converters_wired() {
        let conv = get_converter("openai_completions").unwrap();
        let body = serde_json::json!({"model":"davinci","prompt":"hello"});
        let req = conv.parse_incoming(serde_json::to_vec(&body).unwrap()).expect("parse_incoming");
        let (_, path) = conv.serialize_request(&req).expect("serialize_request");
        assert_eq!(path, "/v1/completions");

        let gem = get_converter("gemini").unwrap();
        let gbody = serde_json::json!({
            "contents": [{"role":"user","parts":[{"text":"hi"}]}]
        });
        let greq = gem.parse_incoming(serde_json::to_vec(&gbody).unwrap()).expect("parse_incoming");
        let (_, gpath) = gem.serialize_request(&greq).expect("serialize_request");
        assert!(gpath.contains(":generateContent"));
    }
}
