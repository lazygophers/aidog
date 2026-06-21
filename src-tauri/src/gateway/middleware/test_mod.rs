//! 引擎核心单测（缓存/解析/regex 编译）+ 跨入站/出站共享的测试构造器。

use super::*;
use super::super::adapter::{ChatRequest, Message, MessageContent, Role, SystemContent};
use super::super::models::{MatchType, MiddlewareRule, MiddlewareSettings, RuleAction, RuleScope, RuleType};

// ─── 共享测试构造器（inbound/outbound 测试复用）────────────────────────

pub(crate) fn mk_rule(
    id: i64,
    rule_type: RuleType,
    scope: RuleScope,
    scope_ref: &str,
    match_type: MatchType,
    pattern: &str,
) -> MiddlewareRule {
    MiddlewareRule {
        id,
        name: format!("rule-{id}"),
        description: String::new(),
        rule_type,
        scope,
        scope_ref: scope_ref.to_string(),
        match_type,
        pattern: pattern.to_string(),
        action: RuleAction::Warn,
        config: "{}".to_string(),
        priority: 0,
        enabled: true,
        is_builtin: false,
        created_at: 0,
        updated_at: 0,
    }
}

pub(crate) fn settings_all_on() -> MiddlewareSettings {
    MiddlewareSettings::default()
}

pub(crate) fn user_msg(text: &str) -> Message {
    Message {
        role: Role::User,
        content: MessageContent::Text(text.to_string()),
    }
}

pub(crate) fn mk_req(messages: Vec<Message>, system: Option<&str>) -> ChatRequest {
    ChatRequest {
        model: "m".to_string(),
        messages,
        system: system.map(|s| SystemContent::Text(s.to_string())),
        max_tokens: None,
        temperature: None,
        top_p: None,
        stream: None,
        tools: None,
        tool_choice: None,
        extra: None,
    }
}

/// 提取 chat_req 全部文本（测试断言用）。
pub(crate) fn dump_text(req: &ChatRequest) -> String {
    super::inbound::collect_request_text(req)
}

// ─── 引擎核心测试 ──────────────────────────────────────────────────────

#[test]
fn compiled_rule_match_semantics() {
    let contains = CompiledRule {
        rule: mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "secret"),
        regex: None,
    };
    assert!(contains.is_match("this has a secret inside"));
    assert!(!contains.is_match("nothing here"));

    let exact = CompiledRule {
        rule: mk_rule(2, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Exact, "exactly"),
        regex: None,
    };
    assert!(exact.is_match("exactly"));
    assert!(!exact.is_match("exactly not"));

    let re = mk_rule(3, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Regex, r"\bsk-[a-z0-9]{8}\b");
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![re]);
    let resolved = engine.resolve_rules(RuleType::SensitiveWord, None, None);
    assert_eq!(resolved.len(), 1);
    assert!(resolved[0].is_match("token sk-abcd1234 ok"));
    assert!(!resolved[0].is_match("token sk-XY end"));
}

#[test]
fn redos_invalid_regex_skipped_not_panic() {
    // 非法正则（未闭合分组）→ 编译失败 → regex=None → 永不命中，不 panic。
    let bad = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Regex, "(unclosed");
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![bad]);
    let resolved = engine.resolve_rules(RuleType::ContentFilter, None, None);
    assert_eq!(resolved.len(), 1, "rule still cached even if regex failed");
    assert!(resolved[0].regex.is_none());
    assert!(!resolved[0].is_match("anything"), "failed-regex rule never matches");
}

#[test]
fn resolve_scope_nearest_override_platform_wins() {
    let g = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "global");
    let grp = mk_rule(2, RuleType::Redaction, RuleScope::Group, "team-a", MatchType::Contains, "group");
    let plat = mk_rule(3, RuleType::Redaction, RuleScope::Platform, "42", MatchType::Contains, "platform");
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![g, grp, plat]);

    // platform 命中 → 只用 platform 层
    let r = engine.resolve_rules(RuleType::Redaction, Some("team-a"), Some(42));
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].rule.pattern, "platform");
}

#[test]
fn resolve_scope_falls_back_to_group_then_global() {
    let g = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "global");
    let grp = mk_rule(2, RuleType::Redaction, RuleScope::Group, "team-a", MatchType::Contains, "group");
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![g, grp]);

    // platform 层无该类型规则 → 落 group
    let r = engine.resolve_rules(RuleType::Redaction, Some("team-a"), Some(99));
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].rule.pattern, "group");

    // group 不匹配 group_key → 落 global
    let r2 = engine.resolve_rules(RuleType::Redaction, Some("other-team"), Some(99));
    assert_eq!(r2.len(), 1);
    assert_eq!(r2[0].rule.pattern, "global");

    // 无 group/platform 上下文 → global
    let r3 = engine.resolve_rules(RuleType::Redaction, None, None);
    assert_eq!(r3.len(), 1);
    assert_eq!(r3[0].rule.pattern, "global");
}

#[test]
fn resolve_isolates_by_rule_type() {
    let red = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "red");
    let flt = mk_rule(2, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "flt");
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![red, flt]);

    let r = engine.resolve_rules(RuleType::Redaction, None, None);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].rule.pattern, "red");
    // 不存在的类型 → 空
    let none = engine.resolve_rules(RuleType::ErrorRule, None, None);
    assert!(none.is_empty());
}

#[test]
fn disabled_rules_excluded_from_cache() {
    let mut disabled = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "x");
    disabled.enabled = false;
    let enabled = mk_rule(2, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "y");
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![disabled, enabled]);
    let r = engine.resolve_rules(RuleType::Redaction, None, None);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].rule.pattern, "y");
}

#[test]
fn rebuild_reloads_cache() {
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(vec![mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "v1")]);
    assert_eq!(engine.resolve_rules(RuleType::Redaction, None, None)[0].rule.pattern, "v1");
    // 模拟 CRUD 写后 reload
    engine.rebuild_from_rules(vec![mk_rule(2, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "v2")]);
    let r = engine.resolve_rules(RuleType::Redaction, None, None);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].rule.pattern, "v2");
}
