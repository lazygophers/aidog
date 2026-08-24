//! middleware.rs 单测（统一引擎模型：serde roundtrip / 默认值 / failed 兜底条件树）。

use super::*;

#[test]
fn target_match_action_kind_serde_snake_case() {
    let t: Target = serde_json::from_str("\"request_body\"").unwrap();
    assert_eq!(t, Target::RequestBody);
    assert_eq!(serde_json::to_string(&Target::ResponseHeaders).unwrap(), "\"response_headers\"");
    let m: MatchType = serde_json::from_str("\"contains\"").unwrap();
    assert_eq!(m, MatchType::Contains);
    let a: ActionKind = serde_json::from_str("\"classify\"").unwrap();
    assert_eq!(a, ActionKind::Classify);
    assert!(a.is_terminal());
    assert!(ActionKind::Block.is_terminal());
    assert!(!ActionKind::Mask.is_terminal());
}

#[test]
fn condition_node_tagged_serde_roundtrip() {
    let node = ConditionNode::Any {
        children: vec![
            ConditionNode::All {
                children: vec![ConditionNode::Leaf(ConditionLeaf {
                    target: Target::RequestBody,
                    field: "messages.0.content".to_string(),
                    match_type: MatchType::Regex,
                    pattern: "sk-\\w+".to_string(),
                })],
            },
            ConditionNode::Leaf(ConditionLeaf {
                target: Target::Status,
                field: String::new(),
                match_type: MatchType::Exact,
                pattern: "429".to_string(),
            }),
        ],
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"kind\":\"any\""));
    let back: ConditionNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, node);
}

#[test]
fn action_params_defaults_on_partial_json() {
    // 旧 config / 前端部分字段缺省 → serde default 填充。
    let p: ActionParams = serde_json::from_str("{\"category\":\"x\"}").unwrap();
    assert_eq!(p.replacement, "****");
    assert!(p.retryable);
    assert_eq!(p.category, "x");
    assert!(p.override_status.is_none());
}

#[test]
fn middleware_settings_default_enabled() {
    let s: MiddlewareSettings = serde_json::from_str("{}").unwrap();
    assert!(s.enabled);
    let s: MiddlewareSettings = serde_json::from_str("{\"enabled\":false}").unwrap();
    assert!(!s.enabled);
}

#[test]
fn applies_to_defaults_empty() {
    let a: AppliesTo = serde_json::from_str("{}").unwrap();
    assert!(a.platforms.is_empty() && a.groups.is_empty() && a.models.is_empty());
}

#[test]
fn validate_rule_phases_rejects_mixed() {
    // 同阶段通过
    let ok = ConditionNode::All {
        children: vec![
            ConditionNode::Leaf(ConditionLeaf { target: Target::RequestBody, field: String::new(), match_type: MatchType::Contains, pattern: "a".into() }),
            ConditionNode::Any {
                children: vec![ConditionNode::Leaf(ConditionLeaf { target: Target::Model, field: String::new(), match_type: MatchType::Exact, pattern: "m".into() })],
            },
        ],
    };
    assert!(validate_rule_phases(&ok).is_ok());
    // 混阶段拒绝（请求侧 + 响应侧）
    let mixed = ConditionNode::All {
        children: vec![
            ConditionNode::Leaf(ConditionLeaf { target: Target::RequestBody, field: String::new(), match_type: MatchType::Contains, pattern: "a".into() }),
            ConditionNode::Leaf(ConditionLeaf { target: Target::Status, field: String::new(), match_type: MatchType::Contains, pattern: "4".into() }),
        ],
    };
    let err = validate_rule_phases(&mixed).unwrap_err();
    assert!(err.contains("mixed-phase"), "{err}");
}
