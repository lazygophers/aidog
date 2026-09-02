//! 透传 body 的 middleware 改写单测（票 02）。
//!
//! 断言口径：给定一份入站 body + 一条规则 → 出站 body 长什么样。不断言函数被调用过。

use super::*;
use aidog_db::models::{
    ActionKind, ActionParams, ActionStep, AppliesTo, ConditionLeaf, ConditionNode, MatchType,
    MiddlewareRule, MiddlewareSettings, Target,
};
use serde_json::json;

const SECRET: &str = "sk-live-abc123";

fn settings_on() -> MiddlewareSettings {
    MiddlewareSettings { enabled: true }
}

fn rule(conditions: ConditionNode, actions: Vec<ActionStep>) -> MiddlewareRule {
    MiddlewareRule {
        id: 1,
        name: "t".to_string(),
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

fn body_contains_leaf(pattern: &str) -> ConditionNode {
    ConditionNode::Leaf(ConditionLeaf {
        target: Target::RequestBody,
        field: String::new(),
        match_type: MatchType::Contains,
        pattern: pattern.to_string(),
    })
}

fn mask_step(replacement: &str, fields: &[&str]) -> ActionStep {
    ActionStep {
        kind: ActionKind::Mask,
        params: ActionParams {
            replacement: replacement.to_string(),
            fields: fields.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        },
    }
}

fn inject_step(mode: &str, target: &str, value: &str) -> ActionStep {
    ActionStep {
        kind: ActionKind::Inject,
        params: ActionParams {
            inject_mode: mode.to_string(),
            target: target.to_string(),
            value: value.to_string(),
            ..Default::default()
        },
    }
}

/// 建一个只装了给定规则的引擎。
fn engine_with(rules: Vec<MiddlewareRule>) -> MiddlewareEngine {
    let e = MiddlewareEngine::new();
    e.rebuild_from_rules(rules);
    e
}

/// 跑一遍透传改写，返回是否改写。
fn run(engine: &MiddlewareEngine, body: &mut Value, wire: &Protocol) -> bool {
    apply_middleware_body(engine, &settings_on(), body, wire, "test-model", 7)
}

// ─── mask / override：敏感串不因「恰好同协议」原样上送 ───────────────

/// anthropic 透传：system 与 messages 两侧的敏感串都被替换。
#[test]
fn anthropic_passthrough_masks_secret_in_system_and_messages() {
    let engine = engine_with(vec![rule(
        body_contains_leaf(SECRET),
        vec![mask_step("[REDACTED]", &[])],
    )]);
    let mut body = json!({
        "model": "claude-3", "max_tokens": 100,
        "system": format!("key is {SECRET}"),
        "messages": [
            {"role": "user", "content": format!("use {SECRET} please")},
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "keep"},
                {"type": "text", "text": format!("echo {SECRET}")}
            ]}
        ]
    });
    assert!(run(&engine, &mut body, &Protocol::Anthropic));
    assert_eq!(body["system"], json!("key is [REDACTED]"));
    assert_eq!(
        body["messages"][0]["content"],
        json!("use [REDACTED] please")
    );
    assert_eq!(
        body["messages"][1]["content"][1]["text"],
        json!("echo [REDACTED]")
    );
    assert_eq!(
        body["messages"][1]["content"][0]["thinking"],
        json!("keep"),
        "无 text 字段的块不动"
    );
    assert_eq!(body["max_tokens"], json!(100), "同级其它字段不动");
    assert!(!serde_json::to_string(&body).unwrap().contains(SECRET));
}

/// openai 透传：role=system|developer 归 system 侧，其余归 messages 侧，两侧都脱敏。
#[test]
fn openai_passthrough_masks_system_role_and_user_parts() {
    let engine = engine_with(vec![rule(
        body_contains_leaf(SECRET),
        vec![mask_step("***", &[])],
    )]);
    let mut body = json!({
        "model": "gpt", "messages": [
            {"role": "system", "content": format!("sys {SECRET}")},
            {"role": "user", "content": [{"type": "text", "text": format!("u {SECRET}")}]}
        ]
    });
    assert!(run(&engine, &mut body, &Protocol::OpenAI));
    assert_eq!(body["messages"][0]["content"], json!("sys ***"));
    assert_eq!(body["messages"][1]["content"][0]["text"], json!("u ***"));
}

/// gemini 透传：systemInstruction.parts 与 contents[].parts 都脱敏。
#[test]
fn gemini_passthrough_masks_parts() {
    let engine = engine_with(vec![rule(
        body_contains_leaf(SECRET),
        vec![mask_step("***", &[])],
    )]);
    let mut body = json!({
        "systemInstruction": {"parts": [{"text": format!("s {SECRET}")}]},
        "contents": [{"role": "user", "parts": [{"text": format!("c {SECRET}")}, {"inlineData": {"data": "x"}}]}],
        "generationConfig": {"maxOutputTokens": 8}
    });
    assert!(run(&engine, &mut body, &Protocol::Gemini));
    assert_eq!(
        body["systemInstruction"]["parts"][0]["text"],
        json!("s ***")
    );
    assert_eq!(body["contents"][0]["parts"][0]["text"], json!("c ***"));
    assert_eq!(
        body["generationConfig"]["maxOutputTokens"],
        json!(8),
        "同级其它字段不动"
    );
}

/// openai_responses 透传：instructions（system 侧）与 input items（messages 侧）都脱敏。
#[test]
fn responses_passthrough_masks_instructions_and_input() {
    let engine = engine_with(vec![rule(
        body_contains_leaf(SECRET),
        vec![mask_step("***", &[])],
    )]);
    let mut body = json!({
        "model": "gpt-5", "instructions": format!("i {SECRET}"),
        "input": [{"role": "user", "content": [{"type": "input_text", "text": format!("in {SECRET}")}]}]
    });
    assert!(run(&engine, &mut body, &Protocol::OpenAIResponses));
    assert_eq!(body["instructions"], json!("i ***"));
    assert_eq!(body["input"][0]["content"][0]["text"], json!("in ***"));
}

/// openai_completions 透传：prompt（字符串数组形态）脱敏。
#[test]
fn completions_passthrough_masks_prompt() {
    let engine = engine_with(vec![rule(
        body_contains_leaf(SECRET),
        vec![mask_step("***", &[])],
    )]);
    let mut body =
        json!({"model": "davinci", "prompt": [format!("p {SECRET}"), "clean".to_string()]});
    assert!(run(&engine, &mut body, &Protocol::OpenAICompletions));
    assert_eq!(body["prompt"], json!(["p ***", "clean"]));
}

/// fields 限定：只写 system 时 messages 侧原样保留。
#[test]
fn mask_fields_scope_limits_rewrite_to_system() {
    let engine = engine_with(vec![rule(
        body_contains_leaf(SECRET),
        vec![mask_step("***", &["system"])],
    )]);
    let mut body = json!({
        "system": format!("s {SECRET}"),
        "messages": [{"role": "user", "content": format!("u {SECRET}")}]
    });
    assert!(run(&engine, &mut body, &Protocol::Anthropic));
    assert_eq!(body["system"], json!("s ***"));
    assert_eq!(
        body["messages"][0]["content"],
        json!(format!("u {SECRET}")),
        "fields=system 时消息侧不动"
    );
}

// ─── 无规则 / 未命中：body 逐字节不变 ────────────────────────────

/// 无规则：body 一个字节都不动（票 02 硬要求）。
#[test]
fn no_rules_leaves_body_untouched() {
    let engine = engine_with(vec![]);
    let original = json!({
        "model": "claude-3", "max_tokens": 100,
        "system": [{"type": "text", "text": "hello"}],
        "messages": [{"role": "user", "content": "hi"}],
        "metadata": {"user_id": "u1"}
    });
    let mut body = original.clone();
    assert!(!run(&engine, &mut body, &Protocol::Anthropic));
    assert_eq!(body, original, "无规则时透传 body 必须逐字节不变");
}

/// 规则条件不命中：body 同样不动。
#[test]
fn unmatched_rule_leaves_body_untouched() {
    let engine = engine_with(vec![rule(
        body_contains_leaf("nope"),
        vec![mask_step("***", &[])],
    )]);
    let original = json!({"system": "hello", "messages": [{"role": "user", "content": "hi"}]});
    let mut body = original.clone();
    assert!(!run(&engine, &mut body, &Protocol::Anthropic));
    assert_eq!(body, original);
}

/// 引擎总开关关闭：规则存在也不改写。
#[test]
fn disabled_settings_leave_body_untouched() {
    let engine = engine_with(vec![rule(
        body_contains_leaf(SECRET),
        vec![mask_step("***", &[])],
    )]);
    let original = json!({"system": format!("s {SECRET}"), "messages": []});
    let mut body = original.clone();
    let changed = apply_middleware_body(
        &engine,
        &MiddlewareSettings { enabled: false },
        &mut body,
        &Protocol::Anthropic,
        "test-model",
        7,
    );
    assert!(!changed);
    assert_eq!(body, original);
}

// ─── inject ──────────────────────────────────────────────────

/// system_append：anthropic 字符串 system 追加一行。
#[test]
fn inject_system_append_anthropic_string() {
    let engine = engine_with(vec![rule(
        body_contains_leaf("hi"),
        vec![inject_step("system_append", "", "POLICY")],
    )]);
    let mut body = json!({"system": "base", "messages": [{"role": "user", "content": "hi"}]});
    assert!(run(&engine, &mut body, &Protocol::Anthropic));
    assert_eq!(body["system"], json!("base\nPOLICY"));
}

/// system_append：anthropic blocks 形态追加一个 text block。
#[test]
fn inject_system_append_anthropic_blocks() {
    let engine = engine_with(vec![rule(
        body_contains_leaf("hi"),
        vec![inject_step("system_append", "", "POLICY")],
    )]);
    let mut body = json!({
        "system": [{"type": "text", "text": "base"}],
        "messages": [{"role": "user", "content": "hi"}]
    });
    assert!(run(&engine, &mut body, &Protocol::Anthropic));
    assert_eq!(body["system"][1], json!({"type": "text", "text": "POLICY"}));
}

/// system_append：openai 无 system 消息时在队首插一条。
#[test]
fn inject_system_append_openai_inserts_system_message() {
    let engine = engine_with(vec![rule(
        body_contains_leaf("hi"),
        vec![inject_step("system_append", "", "POLICY")],
    )]);
    let mut body = json!({"messages": [{"role": "user", "content": "hi"}]});
    assert!(run(&engine, &mut body, &Protocol::OpenAI));
    assert_eq!(
        body["messages"][0],
        json!({"role": "system", "content": "POLICY"})
    );
    assert_eq!(body["messages"][1]["role"], json!("user"));
}

/// system_append：gemini 追加到 systemInstruction.parts（缺省时补建）。
#[test]
fn inject_system_append_gemini_creates_system_instruction() {
    let engine = engine_with(vec![rule(
        body_contains_leaf("hi"),
        vec![inject_step("system_append", "", "POLICY")],
    )]);
    let mut body = json!({"contents": [{"role": "user", "parts": [{"text": "hi"}]}]});
    assert!(run(&engine, &mut body, &Protocol::Gemini));
    assert_eq!(
        body["systemInstruction"],
        json!({"parts": [{"text": "POLICY"}]})
    );
}

/// system_append：responses 写 instructions。
#[test]
fn inject_system_append_responses_instructions() {
    let engine = engine_with(vec![rule(
        body_contains_leaf("hi"),
        vec![inject_step("system_append", "", "POLICY")],
    )]);
    let mut body = json!({"instructions": "base", "input": "hi"});
    assert!(run(&engine, &mut body, &Protocol::OpenAIResponses));
    assert_eq!(body["instructions"], json!("base\nPOLICY"));
}

/// body_set：顶层键赋值，与协议无关。
#[test]
fn inject_body_set_writes_top_level_key() {
    let engine = engine_with(vec![rule(
        body_contains_leaf("hi"),
        vec![inject_step("body_set", "user_tag", "v1")],
    )]);
    let mut body = json!({"messages": [{"role": "user", "content": "hi"}]});
    assert!(run(&engine, &mut body, &Protocol::Anthropic));
    assert_eq!(body["user_tag"], json!("v1"));
}

/// body_set 缺 target：跳过，body 不动。
#[test]
fn inject_body_set_without_target_is_noop() {
    let engine = engine_with(vec![rule(
        body_contains_leaf("hi"),
        vec![inject_step("body_set", "", "v1")],
    )]);
    let original = json!({"messages": [{"role": "user", "content": "hi"}]});
    let mut body = original.clone();
    assert!(!run(&engine, &mut body, &Protocol::Anthropic));
    assert_eq!(body, original);
}
