//! Claude Code 内置工具兼容（builtin-tool-compat spec，`.scratch/builtin-tool-compat/`）。
//!
//! Claude Code 内置工具（ToolSearch / deferred tools / Read / Bash / Agent 等）由客户端
//! 执行，仅以普通 tools 定义随请求下发。第三方模型/端点不支持时（4xx 拒收或不会调用），
//! 按 `platform.extra.builtin_tool_compat`（默认关闭）在转发层出站 body 上做字段级剔除。
//!
//! 本模块与 ADR 0003 middleware 规则引擎分工：middleware 管用户自定义规则，
//! 本模块管路由级内置 per-model 策略，互不叠加重复改写。

use crate::gateway::models::ProxyLog;
use serde_json::Value;

/// Claude Code 内置工具名单（供剔除匹配与日志归因）。与 `STATIC_MODEL_IDS` 同理，
/// 随 Claude Code 版本演进存在月级腐化，需手工核对。
pub const BUILTIN_TOOL_NAMES: &[&str] = &[
    "ToolSearch",
    "Agent",
    "Skill",
    "Read",
    "Edit",
    "Write",
    "Bash",
    "Glob",
    "Grep",
    "NotebookEdit",
    "WebFetch",
    "WebSearch",
    "AskUserQuestion",
    "TodoWrite",
    "TaskCreate",
    "TaskGet",
    "TaskList",
    "TaskOutput",
    "TaskStop",
    "TaskUpdate",
    "EnterPlanMode",
    "ExitPlanMode",
    "EnterWorktree",
    "ExitWorktree",
    "SendMessage",
    "ScheduleWakeup",
];

/// 工具定义是否命中剔除名单。`strip` 空 = 剔除全部内置工具；非空 = 精确按名剔除
/// （允许列非内置名，即用户自定义客户端工具）。
fn in_strip_set(name: &str, strip: &[String]) -> bool {
    if strip.is_empty() {
        BUILTIN_TOOL_NAMES.contains(&name)
    } else {
        strip.iter().any(|s| s == name)
    }
}

/// 取工具定义名：anthropic 扁平 `{name}` 优先，openai `{function:{name}}` 兜底。
fn tool_def_name(t: &Value) -> Option<&str> {
    t.get("name")
        .and_then(Value::as_str)
        .or_else(|| t.get("function").and_then(|f| f.get("name")).and_then(Value::as_str))
}

/// 从出站 body 的 `tools` 数组剔除命中定义；数组清空时连 `tool_choice` 一并移除
/// （tools 为空时 tool_choice 残留会触发上游 400）。返回剔除数。
pub fn strip_tools(body: &mut Value, strip: &[String]) -> usize {
    let Some(arr) = body.get("tools").and_then(Value::as_array) else { return 0 };
    let keep: Vec<Value> = arr
        .iter()
        .filter(|t| tool_def_name(t).is_none_or(|n| !in_strip_set(n, strip)))
        .cloned()
        .collect();
    let removed = arr.len() - keep.len();
    if removed == 0 {
        return 0;
    }
    let obj = body.as_object_mut().expect("tools container is object");
    if keep.is_empty() {
        obj.remove("tools");
        obj.remove("tool_choice");
    } else {
        obj.insert("tools".to_string(), Value::Array(keep));
    }
    removed
}

/// 转发层出站 seam 入口：按 `platform.extra.builtin_tool_compat` 对出站 body 剔除工具定义。
/// 开关关闭（默认）/ 模型不命中名单 → 零改写（含透传与转换两分支）。
pub fn apply_builtin_tool_compat(body: &mut Value, extra: &str, model: &str) {
    let cfg = aidog_db::models::parse_builtin_tool_compat(extra);
    if !cfg.enabled {
        return;
    }
    // models 空 = 平台全部模型生效；非空 = 仅名单内模型
    if !cfg.models.is_empty() && !cfg.models.iter().any(|m| m == model) {
        return;
    }
    let removed = strip_tools(body, &cfg.strip_tools);
    if removed > 0 {
        tracing::info!(model, removed, "builtin-tool-compat: stripped builtin tool defs");
    }
}

/// 运行时审计：上游 4xx 且出站请求含非空 tools 定义 → proxy_log 标记
/// blocked_by="upstream" / blocked_reason="tools_4xx"，供 UI 按模型筛选定位
/// 疑似不支持内置工具的模型。已有 blocked 标记（router / middleware）不覆盖。
pub fn mark_tools_4xx(log: &mut ProxyLog, outbound_body: &Value, status: u16) {
    if !(400..=499).contains(&status) || !log.blocked_reason.is_empty() {
        return;
    }
    let has_tools = outbound_body
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    if has_tools {
        log.blocked_by = "upstream".to_string();
        log.blocked_reason = "tools_4xx".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn anthropic_body() -> Value {
        json!({
            "model": "m",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [
                {"name": "ToolSearch", "description": "d", "input_schema": {}},
                {"name": "Bash", "description": "d", "input_schema": {}},
                {"name": "get_weather", "description": "d", "input_schema": {}}
            ]
        })
    }

    fn openai_body() -> Value {
        json!({
            "model": "m",
            "messages": [],
            "tools": [
                {"type": "function", "function": {"name": "Bash", "parameters": {}}},
                {"type": "function", "function": {"name": "get_weather", "parameters": {}}}
            ]
        })
    }

    #[test]
    fn strip_empty_list_removes_all_builtin_keeps_custom() {
        let mut b = anthropic_body();
        assert_eq!(strip_tools(&mut b, &[]), 2);
        let names: Vec<&str> = b["tools"].as_array().unwrap().iter()
            .map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["get_weather"]);
    }

    #[test]
    fn strip_named_list_exact_match() {
        let mut b = anthropic_body();
        assert_eq!(strip_tools(&mut b, &["ToolSearch".to_string()]), 1);
        assert_eq!(b["tools"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn strip_openai_function_shape() {
        let mut b = openai_body();
        assert_eq!(strip_tools(&mut b, &[]), 1);
        let names: Vec<&str> = b["tools"].as_array().unwrap().iter()
            .map(|t| t["function"]["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["get_weather"]);
    }

    #[test]
    fn strip_all_removes_tool_choice_too() {
        let mut b = openai_body();
        b["tool_choice"] = json!("auto");
        // 自定义工具也点名剔除 → 数组清空 → tool_choice 一并移除
        strip_tools(&mut b, &["Bash".to_string(), "get_weather".to_string()]);
        assert!(b.get("tools").is_none());
        assert!(b.get("tool_choice").is_none());
    }

    #[test]
    fn strip_no_tools_is_noop() {
        let mut b = json!({"model": "m", "messages": []});
        assert_eq!(strip_tools(&mut b, &[]), 0);
        assert!(b.get("tools").is_none());
    }

    #[test]
    fn compat_disabled_by_default_is_noop() {
        let mut b = anthropic_body();
        let before = b.clone();
        apply_builtin_tool_compat(&mut b, "", "any");
        assert_eq!(b, before);
        apply_builtin_tool_compat(&mut b, r#"{"builtin_tool_compat":{"enabled":false}}"#, "any");
        assert_eq!(b, before);
    }

    #[test]
    fn compat_enabled_strips_but_model_scope_gates() {
        let extra = r#"{"builtin_tool_compat":{"enabled":true,"models":["glm-4.7"]}}"#;
        // 不在名单 → 不动
        let mut b = anthropic_body();
        apply_builtin_tool_compat(&mut b, extra, "kimi-k2");
        assert_eq!(b["tools"].as_array().unwrap().len(), 3);
        // 在名单 → 剔除
        let mut b = anthropic_body();
        apply_builtin_tool_compat(&mut b, extra, "glm-4.7");
        assert_eq!(b["tools"].as_array().unwrap().len(), 1);
        // 名单空 → 全模型生效
        let extra_all = r#"{"builtin_tool_compat":{"enabled":true}}"#;
        let mut b = anthropic_body();
        apply_builtin_tool_compat(&mut b, extra_all, "glm-4.7");
        assert_eq!(b["tools"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn mark_tools_4xx_only_when_tools_present_and_4xx() {
        let mut log = ProxyLog::default();
        mark_tools_4xx(&mut log, &anthropic_body(), 400);
        assert_eq!(log.blocked_by, "upstream");
        assert_eq!(log.blocked_reason, "tools_4xx");

        // 2xx / 5xx 不标
        let mut log = ProxyLog::default();
        mark_tools_4xx(&mut log, &anthropic_body(), 200);
        mark_tools_4xx(&mut log, &anthropic_body(), 502);
        assert_eq!(log.blocked_reason, "");

        // 无 tools 不标
        let mut log = ProxyLog::default();
        mark_tools_4xx(&mut log, &json!({"model": "m"}), 400);
        assert_eq!(log.blocked_reason, "");

        // 已有 blocked 标记不覆盖
        let mut log = ProxyLog { blocked_reason: "peak_hours".into(), ..Default::default() };
        mark_tools_4xx(&mut log, &anthropic_body(), 400);
        assert_eq!(log.blocked_reason, "peak_hours");
    }
}
