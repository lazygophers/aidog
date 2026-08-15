//! 协议转换器注册表

use super::traits::ProtocolConverter;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::gateway::adapter::protocols::{
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
pub fn get_converter(protocol: &str) -> Option<Box<dyn ProtocolConverter>> {
    // 注意：由于 trait object 不能 clone，这里返回 None
    // 实际使用时应直接从 registry 获取引用
    None
}
