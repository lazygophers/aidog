//! 协议层 coding plan 套餐标记（`is_coding_plan: bool`）。
//!
//! 真值源同 `platform-presets.json`：标记整套餐协议（glm_coding / bailian_coding /
//! compshare_coding / kimi_coding / qianfan_coding / xiaomi_mimo_coding），与
//! endpoint 级 `coding_plan` flag（端点路由级，语义不同）并存。
//! 缺字段 / 解析失败 / protocol 未列 → false（向后兼容）。
//!
//! 与 TS `defaults.ts::isCodingPlanProtocol` 对称（跨层一致，见 cross-layer-rules.md）。

use serde_json::Value;
use std::sync::OnceLock;

/// bundled preset 缓存：首次访问解析一次 `platform-presets.json`，后续直接索引。
/// 解析失败（不应发生，JSON 已校验）回退空 Object → `default_is_coding_plan` 返 false。
static PRESETS: OnceLock<Value> = OnceLock::new();

const BUNDLED: &str = include_str!("../../defaults/platform-presets.json");

// 跨层对称：与 TS `isCodingPlanProtocol` 同义。当前无 Rust 路由消费（路由层仍用 endpoint
// 级 `coding_plan` flag，语义不同），保留供未来 protocol 级判定 + 编译期 JSON schema 自检。
#[allow(dead_code)]
fn presets() -> &'static Value {
    PRESETS.get_or_init(|| {
        serde_json::from_str(BUNDLED).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "platform-presets.json parse failed in coding_plan; flag defaults to false");
            Value::Object(serde_json::Map::new())
        })
    })
}

/// 按 protocol 名（serde rename 裸名，如 "glm_coding"）查 bundled preset 是否标记为
/// coding plan 套餐。缺失 / 非 bool / 解析失败 → false。
#[allow(dead_code)]
pub fn default_is_coding_plan(protocol: &str) -> bool {
    let doc = presets();
    let Some(proto_obj) = doc.get("protocols").and_then(|p| p.get(protocol)) else {
        return false;
    };
    proto_obj
        .get("is_coding_plan")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glm_coding_flagged() {
        assert!(default_is_coding_plan("glm_coding"));
    }

    #[test]
    fn bailian_coding_flagged() {
        assert!(default_is_coding_plan("bailian_coding"));
    }

    #[test]
    fn compshare_coding_flagged() {
        assert!(default_is_coding_plan("compshare_coding"));
    }

    #[test]
    fn kimi_coding_flagged() {
        assert!(default_is_coding_plan("kimi_coding"));
    }

    #[test]
    fn qianfan_coding_flagged() {
        assert!(default_is_coding_plan("qianfan_coding"));
    }

    #[test]
    fn xiaomi_mimo_coding_flagged() {
        assert!(default_is_coding_plan("xiaomi_mimo_coding"));
    }

    #[test]
    fn non_coding_protocol_not_flagged() {
        assert!(!default_is_coding_plan("anthropic"));
        assert!(!default_is_coding_plan("deepseek"));
        assert!(!default_is_coding_plan("glm"));
    }

    #[test]
    fn unknown_protocol_defaults_false() {
        assert!(!default_is_coding_plan("__never_exists__"));
    }

    #[test]
    fn field_absent_defaults_false_backward_compat() {
        // 旧 JSON 无 is_coding_plan 字段 → false（向后兼容；mock 不在 protocols 表内）
        assert!(!default_is_coding_plan("kimi"));
    }
}
