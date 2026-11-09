//! 可还原 mask 单测：模式反序列化、占位符替换、先后规则互不改动、作用域、fail-closed。

use super::test_mod::*;
use super::*;
use crate::placeholder::{restore, PLACEHOLDER_RE};
use aidog_db::models::{ActionKind, ActionParams, MaskMode};

fn rev_step(fields: &[&str]) -> aidog_db::models::ActionStep {
    step(
        ActionKind::Mask,
        ActionParams {
            mode: MaskMode::Reversible,
            fields: fields.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        },
    )
}

fn run(e: &MiddlewareEngine, sys: &str, msg: &str) -> (InboundTexts<'static>, Result<placeholder::ScrubMap, String>) {
    let mut t = InboundTexts::new("test-model");
    t.system.push(sys.to_string());
    t.messages.push(msg.to_string());
    let r = e.apply_reversible_texts(&settings_on(), &mut t, None, 1, None);
    (t, r)
}

#[test]
fn mask_mode_defaults_to_fixed_and_roundtrips() {
    let p: ActionParams = serde_json::from_str(r#"{"replacement":"x"}"#).unwrap();
    assert_eq!(p.mode, MaskMode::Fixed);
    let p: ActionParams = serde_json::from_str(r#"{"mode":"reversible"}"#).unwrap();
    assert_eq!(p.mode, MaskMode::Reversible);
}

#[test]
fn reversible_replaces_with_placeholder_and_restores() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "r", leaf(Target::RequestBody, r"[a-z]+@[a-z]+\.com"), vec![rev_step(&[])])]);
    let (t, r) = run(&e, "sys bob@example.com", "mail bob@example.com and amy@example.com");
    let map = r.unwrap();
    assert!(!t.messages[0].contains("example.com"), "{:?}", t.messages);
    assert!(!t.system[0].contains("example.com"));
    assert!(PLACEHOLDER_RE.is_match(&t.messages[0]));
    assert_eq!(map.len(), 2, "同值同占位符：bob 一条 + amy 一条");
    assert_eq!(restore(&t.messages[0], &map), "mail bob@example.com and amy@example.com");
}

#[test]
fn reversible_respects_fields() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "r", leaf(Target::RequestBody, "secret"), vec![rev_step(&["messages"])])]);
    let (t, _) = run(&e, "secret", "secret");
    assert_eq!(t.system[0], "secret");
    assert!(PLACEHOLDER_RE.is_match(&t.messages[0]));
}

#[test]
fn later_rule_does_not_touch_earlier_placeholder() {
    let e = MiddlewareEngine::new();
    let mut r1 = mk_rule(1, "first", leaf(Target::RequestBody, "secret"), vec![rev_step(&[])]);
    r1.priority = 1;
    // 第二条会命中占位符内部的 "PII"，但占位符片段必须对它不可见。
    let mut r2 = mk_rule(2, "second", leaf(Target::RequestBody, "PII|MASK"), vec![rev_step(&[])]);
    r2.priority = 2;
    e.rebuild_from_rules(vec![r1, r2]);
    let (t, r) = run(&e, "", "x secret y");
    let map = r.unwrap();
    assert_eq!(map.len(), 1, "后一条不应登记新映射: {map:?}");
    assert_eq!(restore(&t.messages[0], &map), "x secret y");
}

#[test]
fn validator_filters_reversible_hits() {
    let e = MiddlewareEngine::new();
    let mut c = leaf(Target::RequestBody, r"\d{16}");
    if let ConditionNode::Leaf(l) = &mut c {
        l.validator = "luhn".into();
    }
    e.rebuild_from_rules(vec![mk_rule(1, "card", c, vec![rev_step(&[])])]);
    let (t, _) = run(&e, "", "ok 4532015112830366 bad 4532015112830367");
    assert!(t.messages[0].contains("4532015112830367"));
    assert!(!t.messages[0].contains("4532015112830366"));
}

#[test]
fn chat_req_layer_skips_reversible_mask() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "r", leaf(Target::RequestBody, "secret"), vec![rev_step(&[])])]);
    let mut req = chat_req("", "a secret b");
    let mut inj = vec![];
    e.apply_inbound(&settings_on(), &mut req, None, None, &mut inj);
    let aidog_adapter::MessageContent::Text(t) = &req.messages[0].content else { panic!() };
    assert_eq!(t, "a secret b", "可还原规则不在 chat_req 层改写");
    let mut texts = InboundTexts::new("test-model");
    texts.messages.push("a secret b".into());
    e.apply_inbound_texts(&settings_on(), &mut texts, None, 1, None);
    assert_eq!(texts.messages[0], "a secret b", "Value 层固定 mask 路径同样跳过");
}

#[test]
fn fixed_mask_unchanged_by_reversible_pass() {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(vec![mk_rule(1, "f", leaf(Target::RequestBody, "secret"), vec![mask_step("****", &[])])]);
    let (t, r) = run(&e, "", "a secret b");
    assert_eq!(t.messages[0], "a secret b", "固定 mask 不在可还原趟执行");
    assert!(r.unwrap().is_empty());
}

#[test]
fn force_fail_only_when_reversible_rule_in_scope() {
    let e = MiddlewareEngine::new();
    let mut scoped = mk_rule(1, "r", leaf(Target::RequestBody, "secret"), vec![rev_step(&[])]);
    scoped.applies_to.groups = vec!["g1".into()];
    e.rebuild_from_rules(vec![scoped]);
    set_reversible_force_fail(true);
    let mut t = InboundTexts::new("test-model");
    t.messages.push("secret".into());
    let in_scope = e.apply_reversible_texts(&settings_on(), &mut t, Some("g1"), 1, None);
    let out_scope = e.apply_reversible_texts(&settings_on(), &mut t, Some("g2"), 1, None);
    set_reversible_force_fail(false);
    assert!(in_scope.is_err(), "作用域内应报错");
    assert!(out_scope.is_ok(), "作用域外不受影响");
}
