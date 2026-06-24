//! 入站 apply 测试（C2）。

use super::*;
use super::super::super::adapter::SystemContent;
use super::super::super::models::{MatchType, RuleAction, RuleScope, RuleType};
use crate::gateway::middleware::test_mod::{dump_text, mk_req, mk_rule, settings_all_on, user_msg};
use crate::gateway::middleware::MiddlewareEngine;
use super::super::super::models::MiddlewareSettings;

#[test]
fn inbound_block_on_sensitive_word() {
    let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
    rule.action = RuleAction::Block;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("this is forbidden content")], None);
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    match outcome {
        InboundOutcome::Blocked { blocked_by, .. } => {
            assert!(blocked_by.contains("sensitive_word"));
        }
        _ => panic!("expected Blocked"),
    }
    // 不含敏感词 → 放行
    let mut ok = mk_req(vec![user_msg("clean text")], None);
    assert_eq!(engine.apply_inbound(&settings_all_on(), &mut ok, None), InboundOutcome::Continue);
}

#[test]
fn inbound_mask_redaction_rewrites_messages_and_system() {
    let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "topsecret");
    rule.action = RuleAction::Mask;
    rule.config = r#"{"replacement":"[REDACTED]","fields":["messages","system"]}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("here is topsecret data")], Some("system topsecret note"));
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue);
    let text = dump_text(&req);
    assert!(!text.contains("topsecret"), "secret should be masked: {text}");
    assert!(text.contains("[REDACTED]"));
}

#[test]
fn inbound_content_filter_masks_builtin_secret_and_email() {
    // content_filter 未配 pattern → 内置密钥/邮箱检测器
    let mut rule = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Mask;
    rule.config = r#"{"replacement":"****"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(
        vec![user_msg("my key sk-abcdefghijklmnop1234 and mail bob@example.com")],
        None,
    );
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue);
    let text = dump_text(&req);
    assert!(!text.contains("sk-abcdefghijklmnop1234"), "secret masked: {text}");
    assert!(!text.contains("bob@example.com"), "email masked: {text}");
    assert!(text.contains("****"));
}

#[test]
fn inbound_content_filter_blocks_builtin_secret() {
    let mut rule = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Block;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("token ghp_abcdefghijklmnopqrstuvwxyz")], None);
    match engine.apply_inbound(&settings_all_on(), &mut req, None) {
        InboundOutcome::Blocked { .. } => {}
        _ => panic!("expected Blocked on secret"),
    }
}

#[test]
fn inbound_inject_system_append() {
    let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Inject;
    rule.config = r#"{"inject_mode":"system_append","value":"INJECTED_POLICY"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("hi")], Some("base system"));
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue);
    match &req.system {
        Some(SystemContent::Text(t)) => {
            assert!(t.contains("base system"));
            assert!(t.contains("INJECTED_POLICY"));
        }
        _ => panic!("expected appended system text"),
    }
}

#[test]
fn inbound_inject_body_set_writes_extra() {
    let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Inject;
    rule.config = r#"{"inject_mode":"body_set","target":"x_custom","value":"v1"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("hi")], None);
    let _ = engine.apply_inbound(&settings_all_on(), &mut req, None);
    let extra = req.extra.expect("extra set");
    assert_eq!(extra.get("x_custom").and_then(|v| v.as_str()), Some("v1"));
}

#[test]
fn inbound_fail_open_on_bad_regex() {
    // 非法正则 block 规则 → regex=None → 永不命中 → fail-open 放行，不 panic。
    let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Regex, "(unclosed");
    rule.action = RuleAction::Block;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("anything (unclosed here")], None);
    assert_eq!(
        engine.apply_inbound(&settings_all_on(), &mut req, None),
        InboundOutcome::Continue,
        "bad regex rule must fail-open"
    );
}

#[test]
fn inbound_skipped_when_master_off() {
    let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
    rule.action = RuleAction::Block;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let settings = MiddlewareSettings { enabled: false, ..Default::default() };
    let mut req = mk_req(vec![user_msg("forbidden")], None);
    assert_eq!(engine.apply_inbound(&settings, &mut req, None), InboundOutcome::Continue);
}

#[test]
fn inbound_skipped_when_type_toggle_off() {
    let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
    rule.action = RuleAction::Block;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut settings = MiddlewareSettings::default();
    settings.type_toggles.insert("sensitive_word".to_string(), false);
    let mut req = mk_req(vec![user_msg("forbidden")], None);
    assert_eq!(engine.apply_inbound(&settings, &mut req, None), InboundOutcome::Continue);
}

#[test]
fn inbound_platform_only_does_not_apply_global() {
    // platform 层挂载点不应触发 global 规则（避免重复应用）。
    let mut g = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
    g.action = RuleAction::Block;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![g]);

    let mut req = mk_req(vec![user_msg("forbidden")], None);
    // platform 层无规则 → Continue（global 不被 platform-only 触发）
    assert_eq!(engine.apply_inbound_platform(&settings_all_on(), &mut req, 42), InboundOutcome::Continue);

    // 加 platform 规则后才在 platform 挂载点命中
    let mut p = mk_rule(2, RuleType::SensitiveWord, RuleScope::Platform, "42", MatchType::Contains, "blockme");
    p.action = RuleAction::Block;
    engine.rebuild_from_rules(vec![p]);
    let mut req2 = mk_req(vec![user_msg("blockme now")], None);
    match engine.apply_inbound_platform(&settings_all_on(), &mut req2, 42) {
        InboundOutcome::Blocked { .. } => {}
        _ => panic!("expected platform block"),
    }
}

/// inject system_append on None system.
#[test]
fn inbound_inject_system_append_when_no_system() {
    let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Inject;
    rule.config = r#"{"inject_mode":"system_append","value":"POLICY"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("hi")], None); // no system
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue);
    match &req.system {
        Some(SystemContent::Text(t)) => assert!(t.contains("POLICY"), "got: {t}"),
        _ => panic!("expected system text with injected policy"),
    }
}

/// inject system_append on system Blocks.
#[test]
fn inbound_inject_system_append_blocks_system() {
    let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Inject;
    rule.config = r#"{"inject_mode":"system_append","value":"APPENDED"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("hi")], None);
    // set Blocks system manually
    req.system = Some(SystemContent::Blocks(vec![
        serde_json::json!({"type":"text","text":"initial system"}),
    ]));
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue);
    match &req.system {
        Some(SystemContent::Blocks(blocks)) => {
            let last = blocks.last().unwrap();
            assert_eq!(last.get("text").and_then(|v| v.as_str()), Some("APPENDED"));
        }
        _ => panic!("expected Blocks system"),
    }
}

/// inject body_set with empty target → logs warn but does not fail.
#[test]
fn inbound_inject_body_set_empty_target_skips() {
    let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Inject;
    rule.config = r#"{"inject_mode":"body_set","target":"","value":"v"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("hi")], None);
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue);
    // extra should not be set (empty target → skip)
    assert!(req.extra.is_none() || req.extra.as_ref().map(|v| v.as_object().map(|o| o.is_empty()).unwrap_or(true)).unwrap_or(true));
}

/// inject header_set → skipped at inbound layer.
#[test]
fn inbound_inject_header_set_skipped() {
    let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Inject;
    rule.config = r#"{"inject_mode":"header_set","target":"X-Custom","value":"val"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("hi")], None);
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue, "header_set skipped = continue");
}

/// inject unknown mode → skipped.
#[test]
fn inbound_inject_unknown_mode_skipped() {
    let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Inject;
    rule.config = r#"{"inject_mode":"unknown_mode","target":"k","value":"v"}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("hi")], None);
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue);
}

/// inject with bad config JSON → fail-open (skip), no panic.
#[test]
fn inbound_inject_bad_config_fails_open() {
    let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
    rule.action = RuleAction::Inject;
    rule.config = "NOT VALID JSON".to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("hi")], None);
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue);
}

/// collect_request_text includes system Blocks text.
#[test]
fn collect_request_text_with_system_blocks() {
    use super::super::super::adapter::SystemContent;
    let mut req = mk_req(vec![user_msg("msg text")], None);
    req.system = Some(SystemContent::Blocks(vec![
        serde_json::json!({"type":"text","text":"block system text"}),
        serde_json::json!({"type":"other_type"}), // no text field → skip
    ]));
    let text = collect_request_text(&req);
    assert!(text.contains("block system text"), "got: {text}");
    assert!(text.contains("msg text"));
}

/// Warn action: matched rule logs but does not block.
#[test]
fn inbound_warn_action_does_not_block() {
    let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "warn_word");
    rule.action = RuleAction::Warn;
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("warn_word present")], None);
    let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
    assert_eq!(outcome, InboundOutcome::Continue, "Warn should not block");
}

/// Block rule with non-empty description uses description as reason.
#[test]
fn inbound_block_with_description_uses_it() {
    let mut rule = mk_rule(1, RuleType::RequestFilter, RuleScope::Global, "", MatchType::Contains, "block_this");
    rule.action = RuleAction::Block;
    rule.description = "Custom block reason".to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("block_this present")], None);
    match engine.apply_inbound(&settings_all_on(), &mut req, None) {
        InboundOutcome::Blocked { blocked_reason, .. } => {
            assert!(blocked_reason.contains("Custom block reason"), "got: {blocked_reason}");
        }
        _ => panic!("expected Blocked"),
    }
}

/// apply_mask with fields=["system"] only masks system, not messages.
#[test]
fn inbound_mask_system_field_only() {
    let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "secret");
    rule.action = RuleAction::Mask;
    rule.config = r#"{"replacement":"[X]","fields":["system"]}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("secret in message")], Some("secret in system"));
    let _ = engine.apply_inbound(&settings_all_on(), &mut req, None);
    let text = dump_text(&req);
    // system masked
    assert!(!text.contains("secret in system"), "system should be masked: {text}");
    // message should still contain secret (fields=["system"] only)
    assert!(text.contains("secret in message"), "message should not be masked: {text}");
}

/// apply_mask with system Blocks.
#[test]
fn inbound_mask_system_blocks() {
    let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "secret");
    rule.action = RuleAction::Mask;
    rule.config = r#"{"replacement":"[M]","fields":["system"]}"#.to_string();
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![rule]);

    let mut req = mk_req(vec![user_msg("no secret here")], None);
    req.system = Some(SystemContent::Blocks(vec![
        serde_json::json!({"type":"text","text":"secret block text"}),
    ]));
    let _ = engine.apply_inbound(&settings_all_on(), &mut req, None);
    match &req.system {
        Some(SystemContent::Blocks(blocks)) => {
            let text = blocks[0].get("text").and_then(|v| v.as_str()).unwrap_or("");
            assert!(!text.contains("secret"), "system block should be masked: {text}");
            assert!(text.contains("[M]"), "replacement should appear: {text}");
        }
        _ => panic!("expected Blocks system"),
    }
}
