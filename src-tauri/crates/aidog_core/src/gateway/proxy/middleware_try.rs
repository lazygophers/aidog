//! 中间件「试一试」：把样例文本包成单条 user 消息，走与生产透传路径同一组引擎函数
//! （`apply_middleware_body` 固定 mask → `apply_reversible_mask_body` 可还原 mask → 还原）。

use aidog_db::models::{MiddlewareRule, MiddlewareSettings, MiddlewareTryResult};
use aidog_middleware::{placeholder::restore, MiddlewareEngine};
use serde_json::json;

use super::middleware_body::{apply_middleware_body, apply_reversible_mask_body};
use crate::gateway::models::Protocol;

/// 命中规则 = 按优先级逐条累加规则后输出发生变化的那条（归因到「这条规则让文本变了」）。
pub(crate) fn try_run(
    rules: Vec<MiddlewareRule>,
    settings: &MiddlewareSettings,
    text: &str,
) -> Result<MiddlewareTryResult, String> {
    let mut hits = Vec::new();
    let mut prev = text.to_string();
    for i in 1..=rules.len() {
        let (masked, _) = run_once(rules[..i].to_vec(), settings, text)?;
        if masked != prev {
            hits.push(rules[i - 1].name.clone());
            prev = masked;
        }
    }
    let (masked, restored) = run_once(rules, settings, text)?;
    Ok(MiddlewareTryResult { masked, hits, restored })
}

fn run_once(
    rules: Vec<MiddlewareRule>,
    settings: &MiddlewareSettings,
    text: &str,
) -> Result<(String, String), String> {
    let engine = MiddlewareEngine::new();
    engine.rebuild_from_rules(rules);
    let wire = Protocol::Anthropic;
    let mut body = json!({"messages": [{"role": "user", "content": text}]});
    apply_middleware_body(&engine, settings, &mut body, &wire, "", None, 0, None);
    let map = apply_reversible_mask_body(&engine, settings, &mut body, &wire, "", None, 0, None)?;
    let masked = body["messages"][0]["content"].as_str().unwrap_or(text).to_string();
    let restored = restore(&masked, &map);
    Ok((masked, restored))
}

#[cfg(test)]
#[path = "test_middleware_try.rs"]
mod test_middleware_try;
