//! 工具按需加载模拟（ADR 0012 决策 3）：Claude Code 的 `defer_loading` / `tool_reference`
//! 只有 Anthropic 官方上游认。非官方上游上，延迟工具被 `tool_reference` 点名前不下发，
//! 点名后去掉 `defer_loading` 下发；`tool_reference` 块降级为文字。官方上游原样不动。

use super::headers::is_official_anthropic_host;
use super::adapter::{ChatRequest, ContentBlock, MessageContent};
use serde_json::{Value, json};
use std::collections::HashSet;

/// 把 `blocks` 里的 `tool_reference` 换成文字块并记下被点名的工具；返回是否改过。
fn downgrade_refs(blocks: &mut [Value], loaded: &mut HashSet<String>) -> bool {
    let mut changed = false;
    for c in blocks {
        if c.get("type").and_then(Value::as_str) != Some("tool_reference") {
            continue;
        }
        let name = c.get("tool_name").and_then(Value::as_str).unwrap_or("").to_string();
        *c = json!({"type": "text", "text": format!("已加载工具 {name}")});
        loaded.insert(name);
        changed = true;
    }
    changed
}

/// 透传分支：改客户端原体 Value。
pub(crate) fn emulate_value(body: &mut Value, upstream_base_url: &str) {
    if is_official_anthropic_host(upstream_base_url) {
        return;
    }
    let Some(obj) = body.as_object_mut() else { return };
    let mut loaded = HashSet::new();
    for m in obj.get_mut("messages").and_then(Value::as_array_mut).into_iter().flatten() {
        for b in m.get_mut("content").and_then(Value::as_array_mut).into_iter().flatten() {
            if b.get("type").and_then(Value::as_str) == Some("tool_result")
                && let Some(inner) = b.get_mut("content").and_then(Value::as_array_mut)
            {
                downgrade_refs(inner, &mut loaded);
            }
        }
    }
    let Some(tools) = obj.get_mut("tools").and_then(Value::as_array_mut) else { return };
    let is_deferred = |t: &Value| t.get("defer_loading").and_then(Value::as_bool) == Some(true);
    let had_deferred = tools.iter().any(is_deferred);
    tools.retain(|t| {
        !is_deferred(t) || t.get("name").and_then(Value::as_str).is_some_and(|n| loaded.contains(n))
    });
    for t in tools.iter_mut().filter_map(Value::as_object_mut) {
        t.remove("defer_loading");
    }
    if had_deferred && tools.is_empty() {
        obj.remove("tools");
        obj.remove("tool_choice");
    }
}

/// 转换分支：改 ChatRequest（convert_request 之前）。
pub(crate) fn emulate_chat_req(req: &mut ChatRequest, upstream_base_url: &str) {
    if is_official_anthropic_host(upstream_base_url) {
        return;
    }
    let mut loaded = HashSet::new();
    for m in &mut req.messages {
        let MessageContent::Blocks(blocks) = &mut m.content else { continue };
        for b in blocks {
            if let ContentBlock::ToolResult { content, content_blocks: Some(cbs), .. } = b
                && downgrade_refs(cbs, &mut loaded)
            {
                *content = cbs.iter().map(super::adapter::block_text_view).collect();
            }
        }
    }
    let Some(tools) = req.tools.as_mut() else { return };
    let is_deferred = |t: &super::adapter::Tool| {
        t.extra.as_ref().and_then(|e| e.get("defer_loading")).and_then(Value::as_bool) == Some(true)
    };
    let had_deferred = tools.iter().any(is_deferred);
    tools.retain(|t| !is_deferred(t) || loaded.contains(&t.name));
    for t in tools.iter_mut() {
        if let Some(Value::Object(o)) = t.extra.as_mut() {
            o.remove("defer_loading");
        }
    }
    if had_deferred && tools.is_empty() {
        req.tools = None;
        req.tool_choice = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFICIAL: &str = "https://api.anthropic.com";
    const THIRD: &str = "https://open.bigmodel.cn/api/anthropic";

    fn body() -> Value {
        json!({
            "model": "m", "max_tokens": 8,
            "tools": [
                {"name": "ToolSearch", "description": "d", "input_schema": {"type":"object"}},
                {"name": "mcp__a__loaded", "description": "d", "input_schema": {"type":"object"}, "defer_loading": true},
                {"name": "mcp__a__idle", "description": "d", "input_schema": {"type":"object"}, "defer_loading": true}
            ],
            "messages": [
                {"role": "user", "content": "go"},
                {"role": "assistant", "content": [{"type":"tool_use","id":"t1","name":"ToolSearch","input":{"query":"x"}}]},
                {"role": "user", "content": [{"type":"tool_result","tool_use_id":"t1","content":[
                    {"type":"tool_reference","tool_name":"mcp__a__loaded"}
                ]}]}
            ]
        })
    }

    fn tool_names(v: &Value) -> Vec<&str> {
        v["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect()
    }

    #[test]
    fn value_unreferenced_deferred_dropped_referenced_kept_without_flag() {
        let mut b = body();
        emulate_value(&mut b, THIRD);
        assert_eq!(tool_names(&b), ["ToolSearch", "mcp__a__loaded"]);
        assert!(b["tools"].as_array().unwrap().iter().all(|t| t.get("defer_loading").is_none()));
    }

    #[test]
    fn value_tool_reference_becomes_text() {
        let mut b = body();
        emulate_value(&mut b, THIRD);
        let c = &b["messages"][2]["content"][0]["content"][0];
        assert_eq!(c, &json!({"type":"text","text":"已加载工具 mcp__a__loaded"}));
    }

    #[test]
    fn value_non_deferred_tools_unchanged() {
        let mut b = json!({"tools":[{"name":"Read","input_schema":{}}],"messages":[{"role":"user","content":"x"}]});
        let orig = b.clone();
        emulate_value(&mut b, THIRD);
        assert_eq!(b, orig);
    }

    #[test]
    fn value_official_untouched() {
        let mut b = body();
        let orig = b.clone();
        emulate_value(&mut b, OFFICIAL);
        assert_eq!(b, orig);
    }

    #[test]
    fn value_all_deferred_unreferenced_removes_tools_and_choice() {
        let mut b = json!({"tools":[{"name":"a","input_schema":{},"defer_loading":true}],"tool_choice":{"type":"auto"},"messages":[]});
        emulate_value(&mut b, THIRD);
        assert!(b.get("tools").is_none() && b.get("tool_choice").is_none());
    }

    #[test]
    fn value_with_builtin_tool_compat_both_apply() {
        let mut b = body();
        emulate_value(&mut b, THIRD);
        super::super::builtin_tools::apply_builtin_tool_compat(&mut b, "m", true);
        assert_eq!(tool_names(&b), ["mcp__a__loaded"]);
        assert_eq!(b["messages"][2]["content"][0]["content"][0]["type"], "text");
    }

    fn req_of(v: &Value) -> ChatRequest {
        let mut v = v.clone();
        v["stream"] = json!(false);
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn chat_req_matches_value_semantics() {
        let mut r = req_of(&body());
        emulate_chat_req(&mut r, THIRD);
        let names: Vec<&str> = r.tools.as_ref().unwrap().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["ToolSearch", "mcp__a__loaded"]);
        let out = serde_json::to_string(&r).unwrap();
        assert!(!out.contains("defer_loading") && !out.contains("tool_reference"), "{out}");
        assert!(out.contains("已加载工具 mcp__a__loaded"), "{out}");
    }

    #[test]
    fn chat_req_official_untouched() {
        let mut r = req_of(&body());
        let before = serde_json::to_string(&r).unwrap();
        emulate_chat_req(&mut r, OFFICIAL);
        assert_eq!(serde_json::to_string(&r).unwrap(), before);
    }
}
