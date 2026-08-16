//! middleware.rs 单测（原 models.rs `middleware_model_tests` 的中间件相关用例）。

use super::*;

#[test]
fn rule_type_serde_snake_case_roundtrip() {
    for (variant, lit) in [
        (RuleType::RequestFilter, "\"request_filter\""),
        (RuleType::SensitiveWord, "\"sensitive_word\""),
        (RuleType::Redaction, "\"redaction\""),
        (RuleType::ContentFilter, "\"content_filter\""),
        (RuleType::DynamicInjection, "\"dynamic_injection\""),
        (RuleType::ResponseOverride, "\"response_override\""),
        (RuleType::Rectifier, "\"rectifier\""),
        (RuleType::ErrorRule, "\"error_rule\""),
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, lit, "serialize {variant:?}");
        let back: RuleType = serde_json::from_str(lit).unwrap();
        assert_eq!(back, variant, "deserialize {lit}");
        // as_str / from_db_str 与 serde 字面量一致
        assert_eq!(format!("\"{}\"", variant.as_str()), lit);
        assert_eq!(RuleType::from_db_str(variant.as_str()), Some(variant));
    }
    assert_eq!(RuleType::from_db_str("nope"), None);
}

#[test]
fn scope_match_action_serde_snake_case() {
    assert_eq!(serde_json::to_string(&RuleScope::Global).unwrap(), "\"global\"");
    assert_eq!(serde_json::to_string(&RuleScope::Group).unwrap(), "\"group\"");
    assert_eq!(serde_json::to_string(&RuleScope::Platform).unwrap(), "\"platform\"");
    assert_eq!(serde_json::to_string(&MatchType::Regex).unwrap(), "\"regex\"");
    assert_eq!(serde_json::to_string(&MatchType::Contains).unwrap(), "\"contains\"");
    assert_eq!(serde_json::to_string(&MatchType::Exact).unwrap(), "\"exact\"");
    assert_eq!(serde_json::to_string(&RuleAction::Mask).unwrap(), "\"mask\"");
    assert_eq!(serde_json::to_string(&RuleAction::Classify).unwrap(), "\"classify\"");
    // from_db_str 兜底
    assert_eq!(RuleScope::from_db_str("xxx"), RuleScope::Global);
    assert_eq!(MatchType::from_db_str("xxx"), MatchType::Contains);
    assert_eq!(RuleAction::from_db_str("xxx"), RuleAction::Warn);
}

#[test]
fn middleware_rule_serde_roundtrip() {
    let rule = MiddlewareRule {
        id: 7,
        name: "mask-keys".into(),
        description: "redact api keys".into(),
        rule_type: RuleType::Redaction,
        scope: RuleScope::Group,
        scope_ref: "team-a".into(),
        match_type: MatchType::Regex,
        pattern: r"sk-\w+".into(),
        action: RuleAction::Mask,
        config: "{\"replacement\":\"****\"}".into(),
        priority: 3,
        enabled: true,
        is_builtin: false,
        created_at: 100,
        updated_at: 200,
    };
    let json = serde_json::to_string(&rule).unwrap();
    let back: MiddlewareRule = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, rule.id);
    assert_eq!(back.rule_type, rule.rule_type);
    assert_eq!(back.scope, rule.scope);
    assert_eq!(back.match_type, rule.match_type);
    assert_eq!(back.action, rule.action);
    assert_eq!(back.pattern, rule.pattern);
}

#[test]
fn middleware_settings_default_and_type_enabled() {
    let s = MiddlewareSettings::default();
    assert!(s.enabled);
    assert!(s.type_toggles.is_empty());
    // 缺省键 → true
    assert!(s.type_enabled(RuleType::Redaction));

    // 显式关某类型
    let mut s2 = MiddlewareSettings::default();
    s2.type_toggles.insert("redaction".into(), false);
    assert!(!s2.type_enabled(RuleType::Redaction));
    assert!(s2.type_enabled(RuleType::SensitiveWord));

    // 总开关关 → 全 false
    let s3 = MiddlewareSettings { enabled: false, ..Default::default() };
    assert!(!s3.type_enabled(RuleType::SensitiveWord));
}

#[test]
fn middleware_settings_serde_partial_fills_default() {
    // 旧/部分 JSON（无 type_toggles）→ default 填充
    let s: MiddlewareSettings = serde_json::from_str("{\"enabled\":true}").unwrap();
    assert!(s.enabled);
    assert!(s.type_toggles.is_empty());
    // 空对象 → enabled default true
    let s2: MiddlewareSettings = serde_json::from_str("{}").unwrap();
    assert!(s2.enabled);
}
