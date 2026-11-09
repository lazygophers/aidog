//! 「试一试」单测：结果必须等于生产转发引擎函数对同一规则/同一 body 的产出。

use super::*;
use crate::gateway::models::Protocol;
use crate::gateway::proxy::middleware_body::{apply_middleware_body, apply_reversible_mask_body};
use aidog_db::models::{
    ActionKind, ActionParams, ActionStep, AppliesTo, ConditionLeaf, ConditionNode, MaskMode,
    MatchType, Target,
};
use aidog_middleware::{placeholder::restore, MiddlewareEngine};
use serde_json::json;

fn on() -> MiddlewareSettings {
    MiddlewareSettings { enabled: true }
}

fn rule(id: i64, name: &str, pat: &str, mode: MaskMode, repl: &str) -> MiddlewareRule {
    MiddlewareRule {
        id,
        name: name.to_string(),
        description: String::new(),
        conditions: ConditionNode::Leaf(ConditionLeaf {
            target: Target::RequestBody,
            field: String::new(),
            match_type: MatchType::Contains,
            pattern: pat.to_string(),
            validator: String::new(),
        }),
        actions: vec![ActionStep {
            kind: ActionKind::Mask,
            params: ActionParams { mode, replacement: repl.to_string(), ..Default::default() },
        }],
        applies_to: AppliesTo::default(),
        priority: id,
        enabled: true,
        is_builtin: false,
        failed: false,
        created_at: 0,
        updated_at: 0,
    }
}

/// 生产路径基准：同一引擎、同一 body 形状，直接调转发用的两个函数。
fn production(rules: Vec<MiddlewareRule>, text: &str) -> (String, String) {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(rules);
    let mut body = json!({"messages":[{"role":"user","content":text}]});
    apply_middleware_body(&e, &on(), &mut body, &Protocol::Anthropic, "", None, 0, None);
    let map = apply_reversible_mask_body(&e, &on(), &mut body, &Protocol::Anthropic, "", None, 0, None).unwrap();
    let masked = body["messages"][0]["content"].as_str().unwrap().to_string();
    let restored = restore(&masked, &map);
    (masked, restored)
}

#[test]
fn stacked_fixed_and_reversible_equals_production() {
    let text = "key=AAA mail=bob@x.com tail";
    let rules = vec![
        rule(1, "fixed-key", "AAA", MaskMode::Fixed, "[K]"),
        rule(2, "rev-mail", "bob@x.com", MaskMode::Reversible, ""),
        rule(3, "miss", "zzz-not-there", MaskMode::Fixed, "[Z]"),
    ];
    let (m, r) = production(rules.clone(), text);
    assert!(m.contains("[K]") && !m.contains("bob@x.com"), "基准自检: {m}");
    let got = try_run(rules, &on(), text).unwrap();
    assert_eq!(got.masked, m);
    assert_eq!(got.restored, r);
    assert_eq!(got.restored, "key=[K] mail=bob@x.com tail");
    assert_eq!(got.hits, vec!["fixed-key".to_string(), "rev-mail".to_string()]);
}

#[test]
fn disabled_settings_is_noop() {
    let rules = vec![rule(1, "fixed-key", "AAA", MaskMode::Fixed, "[K]")];
    let got = try_run(rules, &MiddlewareSettings { enabled: false }, "AAA").unwrap();
    assert_eq!(got, MiddlewareTryResult { masked: "AAA".into(), hits: vec![], restored: "AAA".into() });
}

#[test]
fn disabled_rule_not_hit() {
    let mut r = rule(1, "off", "AAA", MaskMode::Fixed, "[K]");
    r.enabled = false;
    let got = try_run(vec![r], &on(), "AAA").unwrap();
    assert_eq!(got.masked, "AAA");
    assert!(got.hits.is_empty());
}
