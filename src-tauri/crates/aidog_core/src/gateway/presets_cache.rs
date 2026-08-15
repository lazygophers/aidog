//! platform-presets bundled 解析单例：进程内多个消费方（peak_hours / coding_plan）
//! 此前各自解析同一份 107KB JSON，常驻内存 N 份 `serde_json::Value`。收敛为单一 `OnceLock`，
//! 首次访问解析一次，后续全部消费方共享同一份。
//!
//! 真值源 = `presets_const.rs` 内置常量（2026-08-16 内置化，原外部 JSON 已删除，禁改回）。

use serde_json::Value;
use std::sync::OnceLock;

const BUNDLED: &str = super::presets_const::BUNDLED;

static PRESETS: OnceLock<Value> = OnceLock::new();

/// bundled preset 唯一解析入口：首次访问解析一次，后续直接索引。
/// 解析失败（不应发生，JSON 已校验）回退空 Object → 各 caller 按自身语义退默认值。
pub(crate) fn presets() -> &'static Value {
    PRESETS.get_or_init(|| {
        serde_json::from_str(BUNDLED).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "platform-presets parse failed (presets_cache); defaults disabled");
            Value::Object(serde_json::Map::new())
        })
    })
}

/// bundled 原始文本（未解析）；供需要字符串本身的 caller 用（如 `defaults.rs` 的
/// `get_defaults_json` 兜底返回）。
pub(crate) const fn bundled_str() -> &'static str {
    BUNDLED
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证「单次解析」：各消费方共享同一 `OnceLock` 实例，取到的是同一份内存地址。
    #[test]
    fn single_parse_shared_across_consumers() {
        let a = presets() as *const Value;
        let _ = crate::gateway::peak_hours::default_peak_hours("anthropic");
        let c = presets() as *const Value;
        assert_eq!(a, c, "presets() 应恒返回同一静态实例地址");
    }
}
