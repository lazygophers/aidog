//! 统一引擎单测（缓存/条件树/动作链/applies_to/入站/出站/内置 pattern 样本）。

use super::*;
use aidog_adapter::{ChatRequest, Message, MessageContent, Role, SystemContent};
use aidog_db::models::{
    ActionKind, ActionParams, ActionStep, AppliesTo, ConditionLeaf, ConditionNode, MatchType,
    MiddlewareRule, MiddlewareSettings, Target,
};

// ─── 共享测试构造器 ─────────────────────────────────────────

pub(crate) fn mk_rule(
    id: i64,
    name: &str,
    conditions: ConditionNode,
    actions: Vec<ActionStep>,
) -> MiddlewareRule {
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
        validator: String::new(),
    })
}

pub(crate) fn contains_leaf(target: Target, pattern: &str) -> ConditionNode {
    ConditionNode::Leaf(ConditionLeaf {
        target,
        field: String::new(),
        match_type: MatchType::Contains,
        pattern: pattern.to_string(),
        validator: String::new(),
    })
}

pub(crate) fn step(kind: ActionKind, params: ActionParams) -> ActionStep {
    ActionStep { kind, params }
}

pub(crate) fn mask_step(replacement: &str, fields: &[&str]) -> ActionStep {
    step(
        ActionKind::Mask,
        ActionParams {
            replacement: replacement.to_string(),
            fields: fields.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        },
    )
}

pub(crate) fn chat_req(system: &str, user: &str) -> ChatRequest {
    ChatRequest {
        model: "test-model".to_string(),
        system: if system.is_empty() {
            None
        } else {
            Some(SystemContent::Text(system.to_string()))
        },
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
        {
            let mut r = mk_rule(2, "off", leaf(Target::RequestBody, "b"), vec![]);
            r.enabled = false;
            r
        },
        {
            let mut r = mk_rule(3, "bad", leaf(Target::RequestBody, "c"), vec![]);
            r.failed = true;
            r
        },
    ]);
    assert_eq!(e.snapshot().len(), 1);
}

#[test]
fn invalid_regex_fail_open_never_matches() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "x",
        leaf(Target::RequestBody, "(["),
        vec![],
    )]);
    let mut cr = chat_req("s", "hello");
    assert_eq!(
        e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new()),
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
                children: vec![
                    contains_leaf(Target::RequestBody, "foo"),
                    leaf(Target::RequestBody, "b.r"),
                ],
            },
            ConditionNode::Leaf(ConditionLeaf {
                target: Target::RequestBody,
                field: String::new(),
                match_type: MatchType::Exact,
                // 聚合文本带尾部换行（collect_request_text 每段 push('\n')）
                pattern: "baz\n".to_string(),
                validator: String::new(),
            }),
        ],
    };
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "blocker",
        cond,
        vec![step(ActionKind::Block, ActionParams::default())],
    )]);
    let mut cr = chat_req("", "xxfooxxbarxx");
    assert!(matches!(
        e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new()),
        InboundOutcome::Blocked { .. }
    ));
    let mut cr = chat_req("", "baz");
    assert!(matches!(
        e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new()),
        InboundOutcome::Blocked { .. }
    ));
    let mut cr = chat_req("", "foo"); // AND 缺第二支
    assert_eq!(
        e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new()),
        InboundOutcome::Continue
    );
}

#[test]
fn mixed_phase_rule_evaluates_on_response_side_only() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "resp",
        leaf(Target::ResponseBody, "err"),
        vec![mask_step("****", &[])],
    )]);
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
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "mask",
        leaf(Target::RequestBody, "sk-[a-zA-Z0-9]{16,}"),
        vec![mask_step("****", &["messages", "system"])],
    )]);
    let mut cr = chat_req(
        "secret sk-abcdefghijklmnopqrst in system",
        "key sk-abcdefghijklmnopqrst here",
    );
    e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new());
    assert!(!collect_request_text(&cr).contains("sk-"));
    assert!(collect_request_text(&cr).contains("****"));
}

#[test]
fn inbound_mask_fields_limit_to_messages() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "m",
        leaf(Target::RequestBody, "secret"),
        vec![mask_step("[gone]", &["messages"])],
    )]);
    let mut cr = chat_req("secret in system", "secret in msg");
    e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new());
    let text = collect_request_text(&cr);
    assert!(text.contains("[gone]"), "messages masked");
    assert!(text.contains("secret in system"), "system untouched");
}

#[test]
fn inbound_override_regex_capture_backrefs() {
    // 票 03 内置「日期格式改写」同款：YYYY/MM/DD → YYYY-MM-DD（$1-$2-$3）。
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "date",
        leaf(Target::RequestBody, r"(\d{4})/(\d{1,2})/(\d{1,2})"),
        vec![step(
            ActionKind::Override,
            ActionParams {
                replacement: "$1-$2-$3".to_string(),
                ..Default::default()
            },
        )],
    )]);
    let mut cr = chat_req("", "today is 2026/08/24 ok");
    e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new());
    assert!(collect_request_text(&cr).contains("2026-08-24"));
}

#[test]
fn inbound_inject_system_append() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "always",
        ConditionNode::All { children: vec![] },
        vec![step(
            ActionKind::Inject,
            ActionParams {
                inject_mode: "system_append".to_string(),
                value: "INJECTED".to_string(),
                ..Default::default()
            },
        )],
    )]);
    let mut cr = chat_req("base", "u");
    e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new());
    assert!(matches!(&cr.system, Some(SystemContent::Text(t)) if t.contains("INJECTED")));
}

// ─── 入站 inject / header_set 收集（票 03） ─────────────────

/// 只有一条恒命中的 inject/header_set 规则的引擎。
fn header_set_engine(target: &str, value: &str) -> MiddlewareEngine {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "hs",
        ConditionNode::All { children: vec![] },
        vec![step(
            ActionKind::Inject,
            ActionParams {
                inject_mode: "header_set".to_string(),
                target: target.to_string(),
                value: value.to_string(),
                ..Default::default()
            },
        )],
    )]);
    e
}

#[test]
fn inbound_inject_header_set_collected() {
    let e = header_set_engine("X-Custom", "v1");
    let mut cr = chat_req("base", "u");
    let mut injects = Vec::new();
    e.apply_inbound(&settings_on(), &mut cr, None, None, &mut injects);
    assert_eq!(injects, vec![("X-Custom".to_string(), "v1".to_string())]);
    // body 不受影响（header_set 不碰 system / extra）
    assert!(matches!(&cr.system, Some(SystemContent::Text(t)) if t == "base"));

    // platform 层挂载点同样收集
    let mut cr2 = chat_req("base", "u");
    let mut injects2 = Vec::new();
    e.apply_inbound_platform(&settings_on(), &mut cr2, 7, None, &mut injects2);
    assert_eq!(injects2, vec![("X-Custom".to_string(), "v1".to_string())]);
}

#[test]
fn inbound_inject_header_set_empty_target_skipped() {
    let e = header_set_engine("", "v1");
    let mut cr = chat_req("base", "u");
    let mut injects = Vec::new();
    e.apply_inbound(&settings_on(), &mut cr, None, None, &mut injects);
    assert!(injects.is_empty(), "空 header 名不收集");
}

#[test]
fn terminal_block_stops_later_rules() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![
        mk_rule(
            1,
            "low",
            leaf(Target::RequestBody, "x"),
            vec![step(ActionKind::Block, ActionParams::default())],
        ),
        mk_rule(
            2,
            "high",
            leaf(Target::RequestBody, "x"),
            vec![mask_step("NEVER", &[])],
        ),
    ]);
    let mut cr = chat_req("", "x");
    assert!(matches!(
        e.apply_inbound(&settings_on(), &mut cr, None, None, &mut Vec::new()),
        InboundOutcome::Blocked { .. }
    ));
    assert!(!collect_request_text(&cr).contains("NEVER"));
}

// ─── 观察模式（票 04）────────────────────────────────────────

fn block_step(observe: bool) -> ActionStep {
    step(
        ActionKind::Block,
        ActionParams {
            observe,
            ..Default::default()
        },
    )
}

#[test]
fn observe_block_allows_request_and_reports_hit() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        7,
        "watcher",
        leaf(Target::RequestBody, "secret"),
        vec![block_step(true)],
    )]);
    let mut cr = chat_req("", "a secret here");
    match e.apply_inbound(&settings_on(), &mut cr, None, None) {
        InboundOutcome::Observed { blocked_by } => assert_eq!(blocked_by, "rule#7 watcher"),
        other => panic!("expected Observed, got {other:?}"),
    }
    // 请求原文未被改动（观察模式不碰 body）。
    assert!(collect_request_text(&cr).contains("a secret here"));
    // 不命中 → 仍是 Continue。
    let mut clean = chat_req("", "nothing here");
    assert_eq!(
        e.apply_inbound(&settings_on(), &mut clean, None, None),
        InboundOutcome::Continue
    );
}

#[test]
fn observe_false_still_blocks() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "hard",
        leaf(Target::RequestBody, "secret"),
        vec![block_step(false)],
    )]);
    let mut cr = chat_req("", "a secret here");
    assert!(matches!(
        e.apply_inbound(&settings_on(), &mut cr, None, None),
        InboundOutcome::Blocked { .. }
    ));
}

#[test]
fn observe_does_not_stop_later_rules() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![
        mk_rule(1, "watch", leaf(Target::RequestBody, "x"), vec![block_step(true)]),
        mk_rule(
            2,
            "later",
            leaf(Target::RequestBody, "x"),
            vec![mask_step("MASKED", &[])],
        ),
    ]);
    let mut cr = chat_req("", "x");
    match e.apply_inbound(&settings_on(), &mut cr, None, None) {
        InboundOutcome::Observed { blocked_by } => assert_eq!(blocked_by, "rule#1 watch"),
        other => panic!("expected Observed, got {other:?}"),
    }
    // 观察模式只中和 block，后续规则照常生效。
    assert!(collect_request_text(&cr).contains("MASKED"));
}

#[test]
fn multiple_observe_hits_are_joined() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![
        mk_rule(1, "a", leaf(Target::RequestBody, "x"), vec![block_step(true)]),
        mk_rule(2, "b", leaf(Target::RequestBody, "x"), vec![block_step(true)]),
    ]);
    let mut cr = chat_req("", "x");
    match e.apply_inbound(&settings_on(), &mut cr, None, None) {
        InboundOutcome::Observed { blocked_by } => {
            assert_eq!(blocked_by, "rule#1 a; rule#2 b")
        }
        other => panic!("expected Observed, got {other:?}"),
    }
}

#[test]
fn legacy_action_params_json_without_observe_defaults_false() {
    // 旧规则的 actions JSON 列里没有 observe 字段 → serde default false，读取不报错。
    let steps: Vec<ActionStep> =
        serde_json::from_str(r#"[{"kind":"block","params":{"replacement":"****"}}]"#).unwrap();
    assert_eq!(steps.len(), 1);
    assert!(!steps[0].params.observe);
    // params 整体缺省同样成立。
    let steps: Vec<ActionStep> = serde_json::from_str(r#"[{"kind":"block"}]"#).unwrap();
    assert!(!steps[0].params.observe);
}

#[test]
fn master_switch_off_disables_everything() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "b",
        leaf(Target::RequestBody, "."),
        vec![step(ActionKind::Block, ActionParams::default())],
    )]);
    let mut cr = chat_req("", "x");
    let off = MiddlewareSettings { enabled: false };
    assert_eq!(
        e.apply_inbound(&off, &mut cr, None, None, &mut Vec::new()),
        InboundOutcome::Continue
    );
}

// ─── 入站 request_headers 条件（票 01） ─────────────────────

/// 以 header 名为 field 的 contains 叶子。
fn header_leaf(name: &str, pattern: &str) -> ConditionNode {
    ConditionNode::Leaf(ConditionLeaf {
        target: Target::RequestHeaders,
        field: name.to_string(),
        match_type: MatchType::Contains,
        pattern: pattern.to_string(),
        validator: String::new(),
    })
}

/// 只有 header 条件的 block 规则引擎。
fn header_block_engine(field: &str, pattern: &str) -> MiddlewareEngine {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "ua",
        header_leaf(field, pattern),
        vec![step(ActionKind::Block, ActionParams::default())],
    )]);
    e
}

#[test]
fn inbound_header_condition_matches() {
    let e = header_block_engine("user-agent", "claude-cli");
    let headers = r#"{"user-agent":"claude-cli/2.0.1","accept":"*/*"}"#;
    let mut cr = chat_req("", "hi");
    assert!(matches!(
        e.apply_inbound(
            &settings_on(),
            &mut cr,
            None,
            Some(headers),
            &mut Vec::new()
        ),
        InboundOutcome::Blocked { .. }
    ));
    // platform 层挂载点同样生效
    let mut cr2 = chat_req("", "hi");
    assert!(matches!(
        e.apply_inbound_platform(&settings_on(), &mut cr2, 7, Some(headers), &mut Vec::new()),
        InboundOutcome::Blocked { .. }
    ));
}

#[test]
fn inbound_header_condition_not_matched_when_value_differs() {
    let e = header_block_engine("user-agent", "claude-cli");
    let mut cr = chat_req("", "hi");
    assert_eq!(
        e.apply_inbound(
            &settings_on(),
            &mut cr,
            None,
            Some(r#"{"user-agent":"codex_cli_rs/0.9"}"#),
            &mut Vec::new()
        ),
        InboundOutcome::Continue
    );
}

#[test]
fn inbound_header_condition_absent_header_does_not_match() {
    let e = header_block_engine("anthropic-beta", "oauth");
    let mut cr = chat_req("", "hi");
    // header 不存在
    assert_eq!(
        e.apply_inbound(
            &settings_on(),
            &mut cr,
            None,
            Some(r#"{"accept":"*/*"}"#),
            &mut Vec::new()
        ),
        InboundOutcome::Continue
    );
    // 无 HTTP 上下文（挂载点未传 headers）
    let mut cr2 = chat_req("", "hi");
    assert_eq!(
        e.apply_inbound(&settings_on(), &mut cr2, None, None, &mut Vec::new()),
        InboundOutcome::Continue
    );
}

#[test]
fn inbound_header_name_matching_is_case_insensitive() {
    // 规则写混合大小写、实际请求头小写
    let e = header_block_engine("User-Agent", "claude-cli");
    let mut cr = chat_req("", "hi");
    assert!(matches!(
        e.apply_inbound(
            &settings_on(),
            &mut cr,
            None,
            Some(r#"{"user-agent":"claude-cli/2.0.1"}"#),
            &mut Vec::new()
        ),
        InboundOutcome::Blocked { .. }
    ));
    // 规则写小写、实际请求头混合大小写
    let e2 = header_block_engine("anthropic-beta", "oauth");
    let mut cr2 = chat_req("", "hi");
    assert!(matches!(
        e2.apply_inbound(
            &settings_on(),
            &mut cr2,
            None,
            Some(r#"{"Anthropic-Beta":"oauth-2025-04-20"}"#),
            &mut Vec::new()
        ),
        InboundOutcome::Blocked { .. }
    ));
}

// ─── 出站 / 错误分类 ────────────────────────────────────────

#[test]
fn outbound_mask_rewrites_body() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "m",
        leaf(Target::ResponseBody, "sk-[a-zA-Z0-9]{16,}"),
        vec![mask_step("****", &[])],
    )]);
    let mut body = "leak sk-abcdefghijklmnopqrst end".to_string();
    e.apply_outbound(&settings_on(), &mut body, None, None, "", None);
    assert_eq!(body, "leak **** end");
}

#[test]
fn classify_error_returns_first_match() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![
        mk_rule(
            1,
            "prompt",
            leaf(Target::ResponseBody, "(?i)context length"),
            vec![step(
                ActionKind::Classify,
                ActionParams {
                    category: "prompt_limit".to_string(),
                    retryable: false,
                    ..Default::default()
                },
            )],
        ),
        mk_rule(
            2,
            "catchall",
            leaf(Target::Status, "[0-9]+"),
            vec![step(
                ActionKind::Classify,
                ActionParams {
                    category: "other".to_string(),
                    ..Default::default()
                },
            )],
        ),
    ]);
    let c = e
        .classify_error(
            &settings_on(),
            400,
            "Request too large: context length exceeded",
            None,
            None,
            "",
            None,
        )
        .unwrap();
    assert_eq!(c.category, "prompt_limit");
    assert!(!c.retryable);
    let c2 = e
        .classify_error(&settings_on(), 500, "boom", None, None, "", None)
        .unwrap();
    assert_eq!(c2.category, "other");
    assert!(c2.retryable);
    // catchall（status 叶子 [0-9]+）覆盖任意非 2xx——「任意非 2xx 命中」的显式翻译语义。
    assert!(
        e.classify_error(&settings_on(), 400, "nothing here", None, None, "", None)
            .is_some()
    );
}

// ─── 票 02：出站上下文（applies_to.models + response_headers）────────────────

/// applies_to.models 限定 "gpt-4o" 的响应侧规则：出站传入该 model 才改写。
#[test]
fn outbound_applies_to_models_filters_by_request_model() {
    let e = MiddlewareEngine::new();
    let mut rule = mk_rule(
        1,
        "only-gpt4o",
        leaf(Target::ResponseBody, "secret"),
        vec![mask_step("****", &[])],
    );
    rule.applies_to.models = vec!["gpt-4o".to_string()];
    e.rebuild_from_rules(vec![rule]);

    let mut hit = "a secret b".to_string();
    e.apply_outbound(&settings_on(), &mut hit, None, None, "gpt-4o", None);
    assert_eq!(hit, "a **** b", "限定模型命中时必须改写");

    let mut miss = "a secret b".to_string();
    e.apply_outbound(&settings_on(), &mut miss, None, None, "claude-3-opus", None);
    assert_eq!(miss, "a secret b", "非限定模型不得改写");

    // 空 model（无请求模型上下文）同样不命中非空 models 限定。
    let mut blank = "a secret b".to_string();
    e.apply_outbound(&settings_on(), &mut blank, None, None, "", None);
    assert_eq!(blank, "a secret b");
}

/// 错误分类路径同样按 applies_to.models 过滤。
#[test]
fn classify_error_applies_to_models_filters_by_request_model() {
    let e = MiddlewareEngine::new();
    let mut rule = mk_rule(
        1,
        "only-gpt4o",
        leaf(Target::Status, "[0-9]+"),
        vec![step(
            ActionKind::Classify,
            ActionParams {
                category: "scoped".to_string(),
                ..Default::default()
            },
        )],
    );
    rule.applies_to.models = vec!["gpt-4o".to_string()];
    e.rebuild_from_rules(vec![rule]);

    let hit = e
        .classify_error(&settings_on(), 500, "boom", None, None, "gpt-4o", None)
        .expect("限定模型命中");
    assert_eq!(hit.category, "scoped");
    assert!(
        e.classify_error(&settings_on(), 500, "boom", None, None, "claude-3-opus", None)
            .is_none(),
        "非限定模型不得命中"
    );
}

/// response_headers 叶子按 header 名匹配，且大小写不敏感（上游头名常已被规范成小写）。
#[test]
fn classify_error_response_headers_condition_case_insensitive() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "hdr",
        ConditionNode::Leaf(ConditionLeaf {
            target: Target::ResponseHeaders,
            field: "X-Upstream-Flag".to_string(),
            match_type: MatchType::Contains,
            pattern: "tripped".to_string(),
            validator: String::new(),
        }),
        vec![step(
            ActionKind::Classify,
            ActionParams {
                category: "upstream_flag".to_string(),
                ..Default::default()
            },
        )],
    )]);

    // 规则里写的是 X-Upstream-Flag，实际头名小写 → 仍须命中。
    let headers = r#"{"x-upstream-flag":"tripped","content-type":"application/json"}"#;
    let c = e
        .classify_error(&settings_on(), 503, "err", None, None, "", Some(headers))
        .expect("header 名大小写不敏感命中");
    assert_eq!(c.category, "upstream_flag");

    // 头存在但值不含 pattern → 不命中。
    let other = r#"{"x-upstream-flag":"ok"}"#;
    assert!(
        e.classify_error(&settings_on(), 503, "err", None, None, "", Some(other))
            .is_none()
    );
    // 调用方未传响应头 → 该叶子不命中（fail-open，不误判）。
    assert!(
        e.classify_error(&settings_on(), 503, "err", None, None, "", None)
            .is_none()
    );
}

/// 出站 2xx 路径也能读到响应头：header 条件成立时才对 body 应用改写。
#[test]
fn outbound_response_headers_condition_gates_rewrite() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "hdr-gated",
        ConditionNode::All {
            children: vec![
                ConditionNode::Leaf(ConditionLeaf {
                    target: Target::ResponseHeaders,
                    field: "X-Redact".to_string(),
                    match_type: MatchType::Exact,
                    pattern: "1".to_string(),
                    validator: String::new(),
                }),
                leaf(Target::ResponseBody, "secret"),
            ],
        },
        vec![mask_step("****", &[])],
    )]);

    let mut hit = "a secret b".to_string();
    e.apply_outbound(
        &settings_on(),
        &mut hit,
        None,
        None,
        "",
        Some(r#"{"x-redact":"1"}"#),
    );
    assert_eq!(hit, "a **** b");

    let mut miss = "a secret b".to_string();
    e.apply_outbound(
        &settings_on(),
        &mut miss,
        None,
        None,
        "",
        Some(r#"{"x-redact":"0"}"#),
    );
    assert_eq!(miss, "a secret b");
}

#[test]
fn stream_chunk_masking() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "m",
        leaf(Target::ResponseBody, "secret"),
        vec![mask_step("****", &[])],
    )]);
    let out = e.apply_outbound_stream_chunk(&settings_on(), "a secret b", None, None);
    assert_eq!(out, "a **** b");
}

// ─── 票 05：流式滑窗（跨 chunk 脱敏）─────────────────────────

/// 建一个「literal 命中即 mask」的引擎 + 滑窗器。
fn masker_for(pattern: &str, regex: bool) -> crate::StreamMasker {
    let e = std::sync::Arc::new(MiddlewareEngine::new());
    let cond = if regex {
        leaf(Target::ResponseBody, pattern)
    } else {
        contains_leaf(Target::ResponseBody, pattern)
    };
    e.rebuild_from_rules(vec![mk_rule(1, "m", cond, vec![mask_step("****", &[])])]);
    e.stream_masker(&settings_on(), None, None)
}

/// 逐块喂 + 流末冲刷，返回客户端实际收到的完整文本。
fn drive(m: &mut crate::StreamMasker, chunks: &[&str]) -> String {
    let mut out = String::new();
    for c in chunks {
        out.push_str(&m.push(c));
    }
    out.push_str(&m.finish());
    out
}

#[test]
fn stream_window_masks_secret_split_across_two_chunks() {
    // 本票核心：敏感串被 SSE 分块切成两半，滑窗拼接后仍被脱敏。
    let mut m = masker_for("topsecret", false);
    let out = drive(&mut m, &["prefix tops", "ecret suffix"]);
    assert!(!out.contains("topsecret"), "cross-chunk secret leaked: {out}");
    assert_eq!(out, "prefix **** suffix");
}

#[test]
fn stream_window_masks_secret_split_across_many_tiny_chunks() {
    // 单块极小（1 字节），敏感串横跨 9 个 chunk。
    let mut m = masker_for("topsecret", false);
    let chunks: Vec<&str> = vec!["a", "t", "o", "p", "s", "e", "c", "r", "e", "t", "b"];
    let out = drive(&mut m, &chunks);
    assert_eq!(out, "a****b", "cross-chunk secret leaked: {out}");
}

#[test]
fn stream_window_flushes_tail_at_end_of_stream() {
    // 尾窗残留必须在 finish 时下发，不能吞（无敏感串也一样）。
    let mut m = masker_for("topsecret", false);
    let out = drive(&mut m, &["hello ", "world"]);
    assert_eq!(out, "hello world");
    // finish 幂等：再调返回空串。
    assert_eq!(m.finish(), "");
}

#[test]
fn stream_window_drop_without_finish_leaks_nothing() {
    // 客户端断连 = masker 直接 Drop：扣住的字节从未下发（不泄漏），也不卡住其他状态。
    let mut m = masker_for("topsecret", false);
    let emitted = m.push("head tops");
    drop(m);
    assert!(!emitted.contains("tops"), "tail must be withheld: {emitted}");
}

#[test]
fn stream_window_regex_rule_uses_constant_upper_bound() {
    // 正则叶子 → 保守常数窗口；跨块的密钥同样命中。
    let mut m = masker_for(r"sk-[A-Za-z0-9]{20}", true);
    assert!(m.is_active());
    let out = drive(&mut m, &["key sk-abcdefghij", "klmnopqrst end"]);
    assert!(!out.contains("sk-abcdefghijklmnopqrst"), "leaked: {out}");
    assert_eq!(out, "key **** end");
}

#[test]
fn stream_window_inactive_without_mask_rules() {
    // 无 mask/override 响应规则 → window=0，零延迟透传（首字延迟不受影响）。
    let e = std::sync::Arc::new(MiddlewareEngine::new());
    e.rebuild_from_rules(vec![]);
    let mut m = e.stream_masker(&settings_on(), None, None);
    assert!(!m.is_active());
    assert_eq!(m.push("abc"), "abc");
    assert_eq!(m.finish(), "");
}

#[test]
fn stream_window_inactive_when_master_off() {
    let e = std::sync::Arc::new(MiddlewareEngine::new());
    e.rebuild_from_rules(vec![mk_rule(
        1,
        "m",
        contains_leaf(Target::ResponseBody, "topsecret"),
        vec![mask_step("****", &[])],
    )]);
    let m = e.stream_masker(&MiddlewareSettings { enabled: false }, None, None);
    assert!(!m.is_active(), "master off → 不进滑窗，不加延迟");
}

// ─── 票 03：内置 pattern 命中/排除样本 ───────────────────────

fn pat_matches(pat: &str, text: &str) -> bool {
    Regex::new(pat).map(|re| re.is_match(text)).unwrap_or(false)
}

#[test]
fn builtin_db_uri_pattern_samples() {
    let p = aidog_db::BUILTIN_DB_URI_PATTERN;
    assert!(pat_matches(p, "mysql://root:p4ssw0rd@localhost:3306/db"));
    assert!(pat_matches(
        p,
        "postgresql://admin:hunter2@db.example.com:5432/prod"
    ));
    assert!(pat_matches(p, "redis://:my_strong_pw@127.0.0.1:6379/0"));
    assert!(pat_matches(
        p,
        "mongodb+srv://user:pass@cluster.mongodb.net"
    ));
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
    assert!(pat_matches(
        aidog_db::BUILTIN_SECRET_PATTERN,
        "token sk-abcdefghijklmnopqrst leaked"
    ));
    assert!(pat_matches(
        aidog_db::BUILTIN_SECRET_PATTERN,
        "AKIAIOSFODNN7EXAMPLE"
    ));
    assert!(!pat_matches(aidog_db::BUILTIN_SECRET_PATTERN, "sk-short"));
    assert!(pat_matches(
        aidog_db::BUILTIN_EMAIL_PATTERN,
        "contact bob.smith@example.com now"
    ));
    assert!(!pat_matches(aidog_db::BUILTIN_EMAIL_PATTERN, "no-at-sign"));
    assert!(pat_matches(
        aidog_db::BUILTIN_PHONE_PATTERN,
        "call 13812345678 please"
    ));
    // 宽松国际号段（\+\d{6,15}）按 spec 排除：带 + 的 7-16 位数字不再命中（防订单号/时间戳误伤）。
    assert!(!pat_matches(
        aidog_db::BUILTIN_PHONE_PATTERN,
        "+41791234567"
    ));
    assert!(!pat_matches(
        aidog_db::BUILTIN_PHONE_PATTERN,
        "order +86123456789012x"
    ));
    assert!(!pat_matches(aidog_db::BUILTIN_PHONE_PATTERN, "12345"));
}

/// 密钥模式跨平台覆盖：各厂商前缀形态 + 未知厂商长 token 兜底。
#[test]
fn builtin_secret_pattern_covers_vendor_key_shapes() {
    let p = aidog_db::BUILTIN_SECRET_PATTERN;
    for s in [
        // 分节 sk-（点/连字符分段，旧 `sk-[a-zA-Z0-9]{16,}` 会漏）
        "sk-ws-H.EYIMLMI.abcdefghijklmnop",
        "sk-ant-api03-abcdefghijklmnop1234",
        "sk-or-v1-abcdefghijklmnopqrstuv",
        // 短前缀厂商 token
        "bfl_a1b2c3d4e5f6g7h8",
        "gsk_abcdefghij123456",
        "hf_abcdefghijklmnopqrst",
        "r8_Abcdef123456789012",
        "nvapi-abcdefghij1234567",
        "ark-0f1e2d3c4b5a69788796",
        // 通用词前缀 + 含数字长尾
        "key_b5eaca1f899ac96abcdefghij",
        // Google OAuth / JWT / GitHub / GitLab
        "AQ.Ab8RN6LDH_kdGabcdefgh",
        "ya29.a0AfB_abcdefghijklmnop",
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
        "github_pat_11ABCDEFG0abcdefghijklmno",
        "glpat-abcdefghij1234567890",
        // 未知厂商兜底（前缀不在名单内，靠「长尾 + 含数字」命中）
        "zzcloud_9f8e7d6c5b4a39281706",
    ] {
        assert!(pat_matches(p, s), "应命中密钥样例: {s}");
    }
}

/// 密钥模式防误伤：普通标识符/路径/版本号不得被当密钥抹掉。
#[test]
fn builtin_secret_pattern_rejects_ordinary_text() {
    let p = aidog_db::BUILTIN_SECRET_PATTERN;
    for s in [
        "sk-short",
        "token_expiration_check",
        "run_migrations_proxy_log_late",
        "some-file-name-that-is-long.tsx",
        "migration-20260824-02",
        "node_modules/.bin/vitest",
        "v1.2.3-beta.4",
        "claude-opus-5",
    ] {
        assert!(!pat_matches(p, s), "不应命中普通文本: {s}");
    }
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

// ─── 票 09：NOT 算子 + 内置识别器（正则 + 校验位）────────────────────────

/// 带校验器的 regex 叶子。
fn checked_leaf(pattern: &str, validator: &str) -> ConditionNode {
    ConditionNode::Leaf(ConditionLeaf {
        target: Target::RequestBody,
        field: String::new(),
        match_type: MatchType::Regex,
        pattern: pattern.to_string(),
        validator: validator.to_string(),
    })
}

fn not(child: ConditionNode) -> ConditionNode {
    ConditionNode::Not {
        child: Box::new(child),
    }
}

/// 对请求文本求值一棵条件树（走真实 compile → eval 路径）。
fn eval_req(tree: &ConditionNode, text: &str) -> bool {
    let compiled = compile_node(tree);
    let view = EvalView {
        req_text: text.to_string(),
        model: "test-model",
        ..Default::default()
    };
    compiled.eval(&view)
}

#[test]
fn not_node_negates_leaf() {
    let tree = not(contains_leaf(Target::RequestBody, "secret"));
    assert!(!eval_req(&tree, "this has a secret"));
    assert!(eval_req(&tree, "nothing here"));
}

#[test]
fn not_node_nested_double_negation_and_whitelist() {
    // NOT(NOT(x)) == x
    let double = not(not(contains_leaf(Target::RequestBody, "abc")));
    assert!(eval_req(&double, "xx abc yy"));
    assert!(!eval_req(&double, "xx yy"));

    // 白名单：ALL(含 "req", NOT(ANY(allow1, allow2))) —— 不在白名单里才命中。
    let tree = ConditionNode::All {
        children: vec![
            contains_leaf(Target::RequestBody, "req"),
            not(ConditionNode::Any {
                children: vec![
                    contains_leaf(Target::RequestBody, "allow1"),
                    contains_leaf(Target::RequestBody, "allow2"),
                ],
            }),
        ],
    };
    assert!(eval_req(&tree, "req from somewhere"));
    assert!(!eval_req(&tree, "req from allow2"));
    assert!(!eval_req(&tree, "req from allow1"));
    // 前半不满足 → 整体不命中（NOT 不会把它救回来）
    assert!(!eval_req(&tree, "nothing"));
}

#[test]
fn not_subtree_patterns_excluded_from_mask() {
    // NOT 里的 pattern 表达「不该出现的东西」，不能当改写模式，否则会抹掉要保留的文本。
    let tree = ConditionNode::All {
        children: vec![
            leaf(Target::RequestBody, "keep-me"),
            not(leaf(Target::RequestBody, "do-not-mask")),
        ],
    };
    let compiled = compile_node(&tree);
    let pats: Vec<String> = collect_patterns(&compiled, Target::RequestBody)
        .into_iter()
        .map(|p| p.pattern)
        .collect();
    assert_eq!(pats, vec!["keep-me".to_string()]);
}

#[test]
fn credit_card_checksum_rejects_regex_only_false_positive() {
    let tree = checked_leaf(aidog_db::BUILTIN_CREDIT_CARD_PATTERN, "luhn");
    // 正例：Luhn 合法的测试卡号（含分隔符形式）。
    assert!(eval_req(&tree, "card 4111111111111111 on file"));
    assert!(eval_req(&tree, "card 4111 1111 1111 1111 on file"));
    // 反例：16 位数字，正则照样命中，Luhn 校验挂 → 不算命中。
    assert!(
        pat_matches(aidog_db::BUILTIN_CREDIT_CARD_PATTERN, "1234567812345678"),
        "前提：正则确实命中这串数字（证明拦截来自 checksum 而非正则）"
    );
    assert!(!eval_req(&tree, "order id 1234567812345678"));
    assert!(!eval_req(&tree, "trace 4111111111111112"));
}

#[test]
fn credit_card_mask_leaves_checksum_failures_intact() {
    let tree = checked_leaf(aidog_db::BUILTIN_CREDIT_CARD_PATTERN, "luhn");
    let compiled = compile_node(&tree);
    let pats = collect_patterns(&compiled, Target::RequestBody);
    let p = &pats[0];
    let out = replace_match(
        p.match_type,
        &p.regex,
        &p.pattern,
        &p.validator,
        "card 4111111111111111 order 1234567812345678",
        "****",
    );
    assert_eq!(out, "card **** order 1234567812345678");
}

#[test]
fn iban_and_cn_id_checksum_reject_false_positives() {
    let iban = checked_leaf(aidog_db::BUILTIN_IBAN_PATTERN, "iban");
    assert!(eval_req(&iban, "pay to GB82WEST12345698765432 please"));
    assert!(
        pat_matches(aidog_db::BUILTIN_IBAN_PATTERN, "GB82WEST12345698765433"),
        "前提：正则命中这串（拦截来自 mod-97 校验）"
    );
    assert!(!eval_req(&iban, "pay to GB82WEST12345698765433 please"));

    let cn = checked_leaf(aidog_db::BUILTIN_CN_ID_PATTERN, "cn_id");
    assert!(eval_req(&cn, "id 11010519491231002X here"));
    assert!(
        pat_matches(aidog_db::BUILTIN_CN_ID_PATTERN, "110105194912310021"),
        "前提：正则命中 18 位数字（拦截来自 GB 11643 校验位）"
    );
    assert!(!eval_req(&cn, "id 110105194912310021 here"));
}

#[test]
fn ip_mac_and_ssn_pattern_samples() {
    let ipmac = checked_leaf(aidog_db::BUILTIN_IP_MAC_PATTERN, "");
    assert!(eval_req(&ipmac, "host 192.168.1.10"));
    assert!(eval_req(&ipmac, "mac 3C:22:FB:0A:1B:2C"));
    assert!(eval_req(
        &ipmac,
        "v6 2001:0db8:85a3:0000:0000:8a2e:0370:7334"
    ));
    assert!(!eval_req(&ipmac, "version 999.999.999.999"));
    assert!(!eval_req(&ipmac, "just words"));

    let ssn = checked_leaf(aidog_db::BUILTIN_US_SSN_PATTERN, "");
    assert!(eval_req(&ssn, "ssn 078-05-1120"));
    assert!(eval_req(&ssn, "ssn 078051120"));
    // 官方无效段：area 000 / 666 / 9xx，group 00，serial 0000。
    assert!(!eval_req(&ssn, "ssn 000-12-3456"));
    assert!(!eval_req(&ssn, "ssn 666-12-3456"));
    assert!(!eval_req(&ssn, "ssn 900-12-3456"));
    assert!(!eval_req(&ssn, "ssn 078-00-1120"));
    assert!(!eval_req(&ssn, "ssn 078-05-0000"));
}

#[test]
fn cloud_secret_pattern_is_registry_driven() {
    // 平台前缀来自 registry key_prefixes（禁代码硬编码）：抽两个 registry 里确有的前缀验证，
    // 并确认 seed 出来的 conditions JSON 里带着它们。
    let conds = aidog_db::builtin_rule_specs()
        .iter()
        .find(|s| s.name == "内置·云厂商密钥脱敏")
        .expect("云厂商密钥内置规则存在")
        .conditions;
    for prefix in ["sk-ant-", "ark-"] {
        assert!(
            conds.contains(prefix),
            "conditions 应包含 registry 前缀 {prefix}"
        );
    }
    let node: ConditionNode = serde_json::from_str(conds).expect("conditions JSON 可解析");
    assert!(eval_req(&node, "AKIAIOSFODNN7EXAMPLE"));
    assert!(eval_req(&node, "key sk-ant-api03-abcdefghijklmnop"));
    assert!(!eval_req(&node, "just a normal sentence"));
}

#[test]
fn pii_recognizers_seed_disabled_by_default() {
    for spec in aidog_db::builtin_rule_specs() {
        let expect_off = spec.name.contains("云厂商密钥")
            || spec.name.contains("IP/MAC")
            || spec.name.contains("信用卡")
            || spec.name.contains("IBAN")
            || spec.name.contains("身份证")
            || spec.name.contains("社保号");
        assert_eq!(
            spec.default_enabled,
            !expect_off,
            "内置识别器默认关闭、历史内置规则默认开启：{}",
            spec.name
        );
    }
}
