//! 统一引擎单测（缓存/条件树/动作链/applies_to/入站/出站/内置 pattern 样本）。

use super::*;
use aidog_adapter::{ChatRequest, Message, MessageContent, Role, SystemContent};
use aidog_db::models::{ActionKind, ActionParams, ActionStep, AppliesTo, ConditionLeaf, ConditionNode, MatchType, MiddlewareRule, MiddlewareSettings, Target};

// ─── 共享测试构造器 ─────────────────────────────────────────

pub(crate) fn mk_rule(id: i64, name: &str, conditions: ConditionNode, actions: Vec<ActionStep>) -> MiddlewareRule {
    MiddlewareRule {
        id,
        name: name.to_string(),
        description: String::new(),
        conditions,
        actions,
        applies_to: AppliesTo::default(),
        priority: 0,
        enabled: true,
        is_builtin: false,
        failed: false,
        created_at: 0,
        updated_at: 0,
    }
}

pub(crate) fn leaf(target: Target, pattern: &str) -> ConditionNode {
    ConditionNode::Leaf(ConditionLeaf {
        target,
        field: String::new(),
        match_type: MatchType::Regex,
        pattern: pattern.to_string(),
    })
}

pub(crate) fn contains_leaf(target: Target, pattern: &str) -> ConditionNode {
    ConditionNode::Leaf(ConditionLeaf {
        target,
        field: String::new(),
        match_type: MatchType::Contains,
        pattern: pattern.to_string(),
    })
}

pub(crate) fn step(kind: ActionKind, params: ActionParams) -> ActionStep {
    ActionStep { kind, params }
}

pub(crate) fn mask_step(replacement: &str, fields: &[&str]) -> ActionStep {
    step(ActionKind::Mask, ActionParams {
        replacement: replacement.to_string(),
        fields: fields.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    })
}

pub(crate) fn chat_req(system: &str, user: &str) -> ChatRequest {
    ChatRequest {
        model: "test-model".to_string(),
        system: if system.is_empty() { None } else { Some(SystemContent::Text(system.to_string())) },
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(user.to_string()),
        }],
        max_tokens: None,
        temperature: None,
        top_p: None,
        stream: None,
        tools: None,
        tool_choice: None,
        thinking_budget: None,
        extra: None,
        thinking_mode: None,
    }
}

pub(crate) fn settings_on() -> MiddlewareSettings {
    MiddlewareSettings { enabled: true }
}

// ─── 缓存 / 编译 ────────────────────────────────────────────

#[test]
fn rebuild_skips_disabled_and_failed_rules() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![
        mk_rule(1, "on", leaf(Target::RequestBody, "a"), vec![]),
        { let mut r = mk_rule(2, "off", leaf(Target::RequestBody, "b"), vec![]); r.enabled = false; r },
        { let mut r = mk_rule(3, "bad", leaf(Target::RequestBody, "c"), vec![]); r.failed = true; r },
    ]);
    assert_eq!(e.snapshot().len(), 1);
}

#[test]
fn invalid_regex_fail_open_never_matches() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "x", leaf(Target::RequestBody, "(["), vec![])]);
    let mut cr = chat_req("s", "hello");
    assert_eq!(
        e.apply_inbound(&settings_on(), &mut cr, None),
        InboundOutcome::Continue
    );
}

// ─── 条件树求值 ─────────────────────────────────────────────

#[test]
fn condition_tree_all_any_nesting() {
    let e = MiddlewareEngine::new();
    // (contains "foo" AND regex "b.r") OR exact "baz"
    let cond = ConditionNode::Any {
        children: vec![
            ConditionNode::All {
                children: vec![contains_leaf(Target::RequestBody, "foo"), leaf(Target::RequestBody, "b.r")],
            },
            ConditionNode::Leaf(ConditionLeaf {
                target: Target::RequestBody,
                field: String::new(),
                match_type: MatchType::Exact,
                // 聚合文本带尾部换行（collect_request_text 每段 push('\n')）
                pattern: "baz\n".to_string(),
            }),
        ],
    };
    e.rebuild_from_rules(vec![mk_rule(1, "blocker", cond, vec![step(ActionKind::Block, ActionParams::default())])]);
    let mut cr = chat_req("", "xxfooxxbarxx");
    assert!(matches!(e.apply_inbound(&settings_on(), &mut cr, None), InboundOutcome::Blocked { .. }));
    let mut cr = chat_req("", "baz");
    assert!(matches!(e.apply_inbound(&settings_on(), &mut cr, None), InboundOutcome::Blocked { .. }));
    let mut cr = chat_req("", "foo"); // AND 缺第二支
    assert_eq!(e.apply_inbound(&settings_on(), &mut cr, None), InboundOutcome::Continue);
}

#[test]
fn mixed_phase_rule_evaluates_on_response_side_only() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "resp", leaf(Target::ResponseBody, "err"), vec![mask_step("****", &[])])]);
    assert!(e.request_rules(None, None, "m").is_empty());
    assert_eq!(e.response_rules(None, None, "").len(), 1);
}

// ─── applies_to 过滤 ────────────────────────────────────────

#[test]
fn applies_to_filters_platform_group_model() {
    let e = MiddlewareEngine::new();
    let mut r = mk_rule(1, "scoped", leaf(Target::RequestBody, "x"), vec![]);
    r.applies_to = AppliesTo {
        platforms: vec![7],
        groups: vec!["g1".to_string()],
        models: vec!["m-a".to_string()],
    };
    e.rebuild_from_rules(vec![r]);
    assert_eq!(e.request_rules(Some("g1"), Some(7), "m-a").len(), 1);
    assert!(e.request_rules(None, Some(8), "m-a").is_empty());
    assert!(e.request_rules(Some("g2"), Some(7), "m-a").is_empty());
    assert!(e.request_rules(Some("g1"), Some(7), "m-b").is_empty());
    // 空 = 不限
    let mut r2 = mk_rule(2, "wild", leaf(Target::RequestBody, "y"), vec![]);
    r2.applies_to = AppliesTo::default();
    e.rebuild_from_rules(vec![r2]);
    assert_eq!(e.request_rules(None, None, "anything").len(), 1);
}

// ─── 入站动作 ───────────────────────────────────────────────

#[test]
fn inbound_mask_rewrites_message_and_system() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "mask", leaf(Target::RequestBody, "sk-[a-zA-Z0-9]{16,}"), vec![mask_step("****", &["messages", "system"])])]);
    let mut cr = chat_req("secret sk-abcdefghijklmnopqrst in system", "key sk-abcdefghijklmnopqrst here");
    e.apply_inbound(&settings_on(), &mut cr, None);
    assert!(!collect_request_text(&cr).contains("sk-"));
    assert!(collect_request_text(&cr).contains("****"));
}

#[test]
fn inbound_mask_fields_limit_to_messages() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "m", leaf(Target::RequestBody, "secret"), vec![mask_step("[gone]", &["messages"])])]);
    let mut cr = chat_req("secret in system", "secret in msg");
    e.apply_inbound(&settings_on(), &mut cr, None);
    let text = collect_request_text(&cr);
    assert!(text.contains("[gone]"), "messages masked");
    assert!(text.contains("secret in system"), "system untouched");
}

#[test]
fn inbound_override_regex_capture_backrefs() {
    // 票 03 内置「日期格式改写」同款：YYYY/MM/DD → YYYY-MM-DD（$1-$2-$3）。
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "date", leaf(Target::RequestBody, r"(\d{4})/(\d{1,2})/(\d{1,2})"), vec![step(ActionKind::Override, ActionParams {
        replacement: "$1-$2-$3".to_string(),
        ..Default::default()
    })])]);
    let mut cr = chat_req("", "today is 2026/08/24 ok");
    e.apply_inbound(&settings_on(), &mut cr, None);
    assert!(collect_request_text(&cr).contains("2026-08-24"));
}

#[test]
fn inbound_inject_system_append() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "always", ConditionNode::All { children: vec![] }, vec![step(ActionKind::Inject, ActionParams {
        inject_mode: "system_append".to_string(),
        value: "INJECTED".to_string(),
        ..Default::default()
    })])]);
    let mut cr = chat_req("base", "u");
    e.apply_inbound(&settings_on(), &mut cr, None);
    assert!(matches!(&cr.system, Some(SystemContent::Text(t)) if t.contains("INJECTED")));
}

#[test]
fn terminal_block_stops_later_rules() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![
        mk_rule(1, "low", leaf(Target::RequestBody, "x"), vec![step(ActionKind::Block, ActionParams::default())]),
        mk_rule(2, "high", leaf(Target::RequestBody, "x"), vec![mask_step("NEVER", &[])]),
    ]);
    let mut cr = chat_req("", "x");
    assert!(matches!(e.apply_inbound(&settings_on(), &mut cr, None), InboundOutcome::Blocked { .. }));
    assert!(!collect_request_text(&cr).contains("NEVER"));
}

#[test]
fn master_switch_off_disables_everything() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "b", leaf(Target::RequestBody, "."), vec![step(ActionKind::Block, ActionParams::default())])]);
    let mut cr = chat_req("", "x");
    let off = MiddlewareSettings { enabled: false };
    assert_eq!(e.apply_inbound(&off, &mut cr, None), InboundOutcome::Continue);
}

// ─── 出站 / 错误分类 ────────────────────────────────────────

#[test]
fn outbound_mask_rewrites_body() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "m", leaf(Target::ResponseBody, "sk-[a-zA-Z0-9]{16,}"), vec![mask_step("****", &[])])]);
    let mut body = "leak sk-abcdefghijklmnopqrst end".to_string();
    e.apply_outbound(&settings_on(), &mut body, None, None);
    assert_eq!(body, "leak **** end");
}

#[test]
fn classify_error_returns_first_match() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![
        mk_rule(1, "prompt", leaf(Target::ResponseBody, "(?i)context length"), vec![step(ActionKind::Classify, ActionParams {
            category: "prompt_limit".to_string(),
            retryable: false,
            ..Default::default()
        })]),
        mk_rule(2, "catchall", leaf(Target::Status, "[0-9]+"), vec![step(ActionKind::Classify, ActionParams {
            category: "other".to_string(),
            ..Default::default()
        })]),
    ]);
    let c = e.classify_error(&settings_on(), 400, "Request too large: context length exceeded", None, None).unwrap();
    assert_eq!(c.category, "prompt_limit");
    assert!(!c.retryable);
    let c2 = e.classify_error(&settings_on(), 500, "boom", None, None).unwrap();
    assert_eq!(c2.category, "other");
    assert!(c2.retryable);
    // catchall（status 叶子 [0-9]+）覆盖任意非 2xx——「任意非 2xx 命中」的显式翻译语义。
    assert!(e.classify_error(&settings_on(), 400, "nothing here", None, None).is_some());
}

#[test]
fn stream_chunk_masking() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "m", leaf(Target::ResponseBody, "secret"), vec![mask_step("****", &[])])]);
    let out = e.apply_outbound_stream_chunk(&settings_on(), "a secret b", None, None);
    assert_eq!(out, "a **** b");
}

// ─── 票 03：内置 pattern 命中/排除样本 ───────────────────────

fn pat_matches(pat: &str, text: &str) -> bool {
    Regex::new(pat).map(|re| re.is_match(text)).unwrap_or(false)
}

#[test]
fn builtin_db_uri_pattern_samples() {
    let p = aidog_db::BUILTIN_DB_URI_PATTERN;
    assert!(pat_matches(p, "mysql://root:p4ssw0rd@localhost:3306/db"));
    assert!(pat_matches(p, "postgresql://admin:hunter2@db.example.com:5432/prod"));
    assert!(pat_matches(p, "redis://:my_strong_pw@127.0.0.1:6379/0"));
    assert!(pat_matches(p, "mongodb+srv://user:pass@cluster.mongodb.net"));
    // 排除：无凭据连接串
    assert!(!pat_matches(p, "https://example.com/path?user=bob"));
    assert!(!pat_matches(p, "postgres://localhost:5432/db"));
}

#[test]
fn builtin_key_value_pattern_samples() {
    let p = aidog_db::BUILTIN_KEY_VALUE_PATTERN;
    assert!(pat_matches(p, "password=SuperSecret1"));
    assert!(pat_matches(p, "db password: hunter2pass"));
    assert!(pat_matches(p, "\"api_key\": \"ak_live_abcdef12\""));
    assert!(pat_matches(p, "secret = 'correcthorsebattery'"));
    // 排除：普通赋值 / 短值
    assert!(!pat_matches(p, "name=alice"));
    assert!(!pat_matches(p, "password=abc"));
    assert!(!pat_matches(p, "timeout=30000"));
}

#[test]
fn builtin_secret_email_phone_samples() {
    assert!(pat_matches(aidog_db::BUILTIN_SECRET_PATTERN, "token sk-abcdefghijklmnopqrst leaked"));
    assert!(pat_matches(aidog_db::BUILTIN_SECRET_PATTERN, "AKIAIOSFODNN7EXAMPLE"));
    assert!(!pat_matches(aidog_db::BUILTIN_SECRET_PATTERN, "sk-short"));
    assert!(pat_matches(aidog_db::BUILTIN_EMAIL_PATTERN, "contact bob.smith@example.com now"));
    assert!(!pat_matches(aidog_db::BUILTIN_EMAIL_PATTERN, "no-at-sign"));
    assert!(pat_matches(aidog_db::BUILTIN_PHONE_PATTERN, "call 13812345678 please"));
    // 宽松国际号段（\+\d{6,15}）按 spec 排除：带 + 的 7-16 位数字不再命中（防订单号/时间戳误伤）。
    assert!(!pat_matches(aidog_db::BUILTIN_PHONE_PATTERN, "+41791234567"));
    assert!(!pat_matches(aidog_db::BUILTIN_PHONE_PATTERN, "order +86123456789012x"));
    assert!(!pat_matches(aidog_db::BUILTIN_PHONE_PATTERN, "12345"));
}

/// 聚合请求文本（测试侧内联同逻辑：inbound 的实现是 pub(super) 不可跨 mod 引）。
pub(crate) fn collect_request_text(chat_req: &ChatRequest) -> String {
    let mut buf = String::new();
    if let Some(SystemContent::Text(s)) = &chat_req.system {
        buf.push_str(s);
        buf.push('\n');
    }
    for m in &chat_req.messages {
        if let MessageContent::Text(t) = &m.content {
            buf.push_str(t);
            buf.push('\n');
        }
    }
    buf
}
