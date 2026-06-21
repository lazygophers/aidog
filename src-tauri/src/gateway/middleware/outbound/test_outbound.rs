//! 出站 apply 测试（C3）。

use super::super::super::models::{MatchType, MiddlewareSettings, RuleAction, RuleScope, RuleType};
use crate::gateway::middleware::test_mod::{mk_rule, settings_all_on};
use crate::gateway::middleware::MiddlewareEngine;

#[test]
fn outbound_mask_redaction_rewrites_body() {
    let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "topsecret");
    rule.action = RuleAction::Mask;
    rule.config = r#"{"replacement":"[REDACTED]"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut body = r#"{"choices":[{"message":{"content":"here is topsecret data"}}]}"#.to_string();
    engine.apply_outbound(&settings_all_on(), &mut body, None, None);
    assert!(!body.contains("topsecret"), "secret must be masked: {body}");
    assert!(body.contains("[REDACTED]"));
}

#[test]
fn outbound_content_filter_masks_builtin_secret() {
    // content_filter 未配 pattern → 内置密钥检测器替换上游响应体中的密钥。
    let mut rule = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Mask;
    rule.config = r#"{"replacement":"****"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut body = r#"{"content":"your key is sk-abcdefghijklmnop1234 keep safe"}"#.to_string();
    engine.apply_outbound(&settings_all_on(), &mut body, None, None);
    assert!(!body.contains("sk-abcdefghijklmnop1234"), "secret in response must be masked: {body}");
    assert!(body.contains("****"));
}

#[test]
fn outbound_response_override_regex_rewrites_body() {
    let mut rule = mk_rule(1, RuleType::ResponseOverride, RuleScope::Global, "", MatchType::Regex, r"gpt-4o");
    rule.action = RuleAction::Override;
    rule.config = r#"{"replacement":"model-x"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut body = r#"{"model":"gpt-4o","ok":true}"#.to_string();
    engine.apply_outbound(&settings_all_on(), &mut body, None, None);
    assert!(body.contains("model-x"));
    assert!(!body.contains("gpt-4o"));
}

#[test]
fn outbound_idempotent_with_inbound_redaction() {
    // 入站已脱敏 → 出站再扫同一规则不应破坏（幂等：已是 replacement，pattern 不再命中）。
    let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "topsecret");
    rule.action = RuleAction::Mask;
    rule.config = r#"{"replacement":"****"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut body = r#"{"content":"already masked ****"}"#.to_string();
    let before = body.clone();
    engine.apply_outbound(&settings_all_on(), &mut body, None, None);
    assert_eq!(body, before, "masked body must be stable under re-scan");
}

#[test]
fn classify_error_non_retryable_with_override() {
    let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "content_policy");
    rule.action = RuleAction::Classify;
    rule.config = r#"{"category":"content_filter","retryable":false,"override_status":400,"override_body":{"error":"blocked by policy"}}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let body = r#"{"error":{"message":"content_policy violation"}}"#;
    let c = engine
        .classify_error(&settings_all_on(), 400, body, None, None)
        .expect("should classify");
    assert!(!c.retryable, "must be non-retryable");
    assert_eq!(c.category, "content_filter");
    assert_eq!(c.override_status, Some(400));
    assert!(c.override_body.as_deref().unwrap().contains("blocked by policy"));
}

#[test]
fn classify_error_retryable_default_when_unset() {
    // config 未设 retryable → 缺省 true（可重试，换候选）。
    let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "rate");
    rule.action = RuleAction::Classify;
    rule.config = r#"{"category":"rate_limit"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let c = engine
        .classify_error(&settings_all_on(), 429, "rate limited", None, None)
        .expect("should classify");
    assert!(c.retryable, "default retryable=true");
    assert_eq!(c.override_status, None);
    assert_eq!(c.override_body, None);
}

#[test]
fn classify_error_empty_pattern_matches_any_nonsuccess() {
    // pattern 空 → 任意非 2xx 命中（纯按状态码分类）。
    let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Classify;
    rule.config = r#"{"category":"server_error","retryable":true}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let c = engine
        .classify_error(&settings_all_on(), 500, "internal error", None, None)
        .expect("empty pattern matches any error");
    assert_eq!(c.category, "server_error");
}

#[test]
fn classify_error_none_when_no_rule_matches() {
    let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "specific_token");
    rule.action = RuleAction::Classify;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    assert!(engine
        .classify_error(&settings_all_on(), 502, "bad gateway", None, None)
        .is_none());
}

#[test]
fn classify_error_skipped_when_type_toggle_off() {
    let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Classify;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut settings = MiddlewareSettings::default();
    settings.type_toggles.insert("error_rule".to_string(), false);
    assert!(engine.classify_error(&settings, 500, "err", None, None).is_none());
}

#[test]
fn outbound_stream_chunk_masks_secret_per_chunk() {
    let mut rule = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Mask;
    rule.config = r#"{"replacement":"****"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let chunk = r#"data: {"delta":{"text":"token sk-abcdefghijklmnop1234 end"}}"#;
    let out = engine.apply_outbound_stream_chunk(&settings_all_on(), chunk, None, None);
    assert!(!out.contains("sk-abcdefghijklmnop1234"), "secret in stream chunk masked: {out}");
    assert!(out.contains("****"));
}

#[test]
fn outbound_stream_chunk_sensitive_word_replaced() {
    let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
    rule.action = RuleAction::Block; // 流式降级为替换（已发出的流不能拦截）
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let chunk = r#"data: {"delta":{"text":"this is forbidden word"}}"#;
    let out = engine.apply_outbound_stream_chunk(&settings_all_on(), chunk, None, None);
    assert!(!out.contains("forbidden"), "sensitive word replaced in stream: {out}");
    assert!(out.contains("****"));
}

#[test]
fn outbound_skipped_when_master_off() {
    let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "topsecret");
    rule.action = RuleAction::Mask;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let settings = MiddlewareSettings { enabled: false, ..Default::default() };
    let mut body = "topsecret stays".to_string();
    engine.apply_outbound(&settings, &mut body, None, None);
    assert_eq!(body, "topsecret stays", "master off → no rewrite");

    let stream_out = engine.apply_outbound_stream_chunk(&settings, "topsecret", None, None);
    assert_eq!(stream_out, "topsecret");
}
