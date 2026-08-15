//! 平台转换器注册表

use super::traits::PlatformConverter;
use std::collections::HashMap;
use std::sync::OnceLock;

/// 平台转换器注册表（单例）
pub fn converter_registry() -> &'static HashMap<String, Box<dyn PlatformConverter>> {
    static REGISTRY: OnceLock<HashMap<String, Box<dyn PlatformConverter>>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let map = HashMap::new();
        // 注册各平台转换器（TODO: 逐步添加）
        // map.insert("glm".to_string(), Box::new(glm::GlmConverter));
        // map.insert("glm_coding".to_string(), Box::new(glm_coding::GlmCodingConverter));
        // ...
        map
    })
}
