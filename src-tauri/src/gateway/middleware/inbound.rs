//! 入站规则执行（C2）。
//!
//! 挂载点：proxy.rs 路由前（global/group 层）与候选选定后（platform 层）。
//! 执行顺序：request_filter → sensitive_word → redaction → content_filter → dynamic_injection。
//! 动作语义：Block 拦截 / Mask 脱敏原地改写 / Inject 注入 / Warn 记日志；override/classify 属出站忽略。

use serde::Deserialize;

use super::super::adapter::{ChatRequest, MessageContent, SystemContent};
use super::super::models::{MiddlewareSettings, RuleAction, RuleType};
use super::{
    builtin_detectors_match, default_replacement, mask_text, CompiledRule, MiddlewareEngine,
    RedactionConfig,
};

/// 入站规则类型执行顺序。
const INBOUND_ORDER: [RuleType; 5] = [
    RuleType::RequestFilter,
    RuleType::SensitiveWord,
    RuleType::Redaction,
    RuleType::ContentFilter,
    RuleType::DynamicInjection,
];

/// 入站执行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundOutcome {
    /// 放行（chat_req 可能已被 mask/inject 原地改写）。
    Continue,
    /// 拦截：写审计日志不计费，立即返回 4xx。
    Blocked {
        /// 命中规则标识（rule_type#id name）。
        blocked_by: String,
        /// 人读拦截原因。
        blocked_reason: String,
    },
}

/// dynamic_injection config：`{ "inject_mode":"system_append|header_set|body_set", "target":"...", "value":"..." }`
#[derive(Debug, Deserialize)]
struct InjectionConfig {
    #[serde(default)]
    inject_mode: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    value: String,
}

impl MiddlewareEngine {
    /// 入站规则执行（路由前挂载点：global/group 层）。
    /// 顺序：request_filter → sensitive_word → redaction → content_filter → dynamic_injection。
    /// 返回 [`InboundOutcome`]：`Continue` 放行（可能已原地改写 chat_req），`Blocked` 拦截。
    pub fn apply_inbound(
        &self,
        settings: &MiddlewareSettings,
        chat_req: &mut ChatRequest,
        group_key: Option<&str>,
    ) -> InboundOutcome {
        if !settings.enabled {
            return InboundOutcome::Continue;
        }
        for rule_type in INBOUND_ORDER {
            if !settings.type_enabled(rule_type) {
                continue;
            }
            let rules = self.resolve_rules(rule_type, group_key, None);
            if let outcome @ InboundOutcome::Blocked { .. } =
                apply_rules_inbound(rule_type, &rules, chat_req)
            {
                return outcome;
            }
        }
        InboundOutcome::Continue
    }

    /// 入站规则执行（候选选定后挂载点：platform 层）。仅应用 platform 作用域规则，
    /// 不重复 group/global（已在 [`apply_inbound`] 应用）。
    pub fn apply_inbound_platform(
        &self,
        settings: &MiddlewareSettings,
        chat_req: &mut ChatRequest,
        platform_id: i64,
    ) -> InboundOutcome {
        if !settings.enabled {
            return InboundOutcome::Continue;
        }
        for rule_type in INBOUND_ORDER {
            if !settings.type_enabled(rule_type) {
                continue;
            }
            let rules = self.resolve_platform_only(rule_type, platform_id);
            if rules.is_empty() {
                continue;
            }
            if let outcome @ InboundOutcome::Blocked { .. } =
                apply_rules_inbound(rule_type, &rules, chat_req)
            {
                return outcome;
            }
        }
        InboundOutcome::Continue
    }
}

/// 对一组同类型规则按入站语义执行（fail-open：单条异常不阻断后续）。
fn apply_rules_inbound(
    rule_type: RuleType,
    rules: &[CompiledRule],
    chat_req: &mut ChatRequest,
) -> InboundOutcome {
    for cr in rules {
        match cr.rule.action {
            RuleAction::Block => {
                if rule_matches_request(rule_type, cr, chat_req) {
                    return InboundOutcome::Blocked {
                        blocked_by: format!("{}#{} {}", rule_type.as_str(), cr.rule.id, cr.rule.name),
                        blocked_reason: if cr.rule.description.is_empty() {
                            format!("matched pattern: {}", cr.rule.pattern)
                        } else {
                            cr.rule.description.clone()
                        },
                    };
                }
            }
            RuleAction::Mask => apply_mask(rule_type, cr, chat_req),
            RuleAction::Inject => apply_inject(cr, chat_req),
            RuleAction::Warn => {
                if rule_matches_request(rule_type, cr, chat_req) {
                    tracing::warn!(
                        rule_id = cr.rule.id, rule_name = %cr.rule.name,
                        rule_type = %rule_type.as_str(), pattern = %cr.rule.pattern,
                        "middleware inbound: warn rule matched"
                    );
                }
            }
            // override/classify 属出站(C3)语义，入站忽略。
            RuleAction::Override | RuleAction::Classify => {}
        }
    }
    InboundOutcome::Continue
}

/// 规则是否命中请求文本（聚合 messages + system 文本后判定）。
/// content_filter 类用内置密钥/邮箱检测器；其余按规则自身 match_type。
fn rule_matches_request(rule_type: RuleType, cr: &CompiledRule, chat_req: &ChatRequest) -> bool {
    let text = collect_request_text(chat_req);
    if rule_type == RuleType::ContentFilter && cr.rule.pattern.is_empty() {
        // 未配 pattern 的内容过滤 → 用内置密钥/邮箱检测兜底。
        return builtin_detectors_match(&text);
    }
    cr.is_match(&text)
}

/// 聚合请求中所有可读文本（messages 文本块 + system）。
pub(super) fn collect_request_text(chat_req: &ChatRequest) -> String {
    let mut buf = String::new();
    if let Some(sys) = &chat_req.system {
        match sys {
            SystemContent::Text(s) => {
                buf.push_str(s);
                buf.push('\n');
            }
            SystemContent::Blocks(blocks) => {
                for b in blocks {
                    if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                        buf.push_str(t);
                        buf.push('\n');
                    }
                }
            }
        }
    }
    for m in &chat_req.messages {
        for_each_text(&m.content, &mut |t| {
            buf.push_str(t);
            buf.push('\n');
        });
    }
    buf
}

/// mask 动作：按 config.fields 限定范围，将命中文本替换为 config.replacement。
/// content_filter 未配 pattern → 用内置密钥/邮箱检测器做替换。
fn apply_mask(rule_type: RuleType, cr: &CompiledRule, chat_req: &mut ChatRequest) {
    let cfg: RedactionConfig =
        serde_json::from_str(&cr.rule.config).unwrap_or(RedactionConfig {
            replacement: default_replacement(),
            fields: Vec::new(),
        });
    let touch_messages = cfg.fields.is_empty() || cfg.fields.iter().any(|f| f == "messages");
    let touch_system = cfg.fields.is_empty() || cfg.fields.iter().any(|f| f == "system");
    let replacement = cfg.replacement.clone();

    let use_builtin = rule_type == RuleType::ContentFilter && cr.rule.pattern.is_empty();
    let replace = |s: &str| -> String { mask_text(s, rule_type, cr, use_builtin, &replacement) };

    if touch_system {
        if let Some(sys) = chat_req.system.as_mut() {
            match sys {
                SystemContent::Text(t) => *t = replace(t),
                SystemContent::Blocks(blocks) => {
                    for b in blocks.iter_mut() {
                        if let Some(s) = b.get("text").and_then(|t| t.as_str()) {
                            let masked = replace(s);
                            if let Some(obj) = b.as_object_mut() {
                                obj.insert("text".to_string(), serde_json::Value::String(masked));
                            }
                        }
                    }
                }
            }
        }
    }
    if touch_messages {
        for m in chat_req.messages.iter_mut() {
            map_text(&mut m.content, &replace);
        }
    }
}

/// inject 动作：按 inject_mode 注入。header_set 在入站无 HTTP 上下文 → 仅 body/system 生效，
/// header_set 记日志跳过（入站 chat_req 抽象无 header；出站/转发层注入由 C3/proxy 处理）。
fn apply_inject(cr: &CompiledRule, chat_req: &mut ChatRequest) {
    let cfg: InjectionConfig = match serde_json::from_str(&cr.rule.config) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(rule_id = cr.rule.id, error = %e, "middleware inject: bad config, skip (fail-open)");
            return;
        }
    };
    match cfg.inject_mode.as_str() {
        "system_append" => {
            let appended = match chat_req.system.take() {
                Some(SystemContent::Text(t)) => {
                    SystemContent::Text(format!("{t}\n{}", cfg.value))
                }
                Some(SystemContent::Blocks(mut blocks)) => {
                    blocks.push(serde_json::json!({ "type": "text", "text": cfg.value }));
                    SystemContent::Blocks(blocks)
                }
                None => SystemContent::Text(cfg.value.clone()),
            };
            chat_req.system = Some(appended);
        }
        "body_set" => {
            // 写入 chat_req.extra（透传 flatten 字段）。target 为 JSON key。
            if cfg.target.is_empty() {
                tracing::warn!(rule_id = cr.rule.id, "middleware inject body_set: empty target, skip");
                return;
            }
            let extra = chat_req
                .extra
                .get_or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let Some(obj) = extra.as_object_mut() {
                obj.insert(cfg.target.clone(), serde_json::Value::String(cfg.value.clone()));
            }
        }
        "header_set" => {
            // 入站 chat_req 抽象层无 HTTP header；header 注入属转发层(后续/出站)能力。
            tracing::debug!(rule_id = cr.rule.id, "middleware inject header_set: not supported at inbound chat_req layer, skipped");
        }
        other => {
            tracing::warn!(rule_id = cr.rule.id, mode = %other, "middleware inject: unknown inject_mode, skip");
        }
    }
}

/// 遍历 MessageContent 内全部文本块（只读）。
fn for_each_text(content: &MessageContent, f: &mut dyn FnMut(&str)) {
    match content {
        MessageContent::Text(t) => f(t),
        MessageContent::Blocks(blocks) => {
            for b in blocks {
                if let super::super::adapter::ContentBlock::Text { text } = b {
                    f(text);
                }
            }
        }
    }
}

/// 原地映射 MessageContent 内全部文本块。
fn map_text(content: &mut MessageContent, f: &dyn Fn(&str) -> String) {
    match content {
        MessageContent::Text(t) => *t = f(t),
        MessageContent::Blocks(blocks) => {
            for b in blocks.iter_mut() {
                if let super::super::adapter::ContentBlock::Text { text } = b {
                    *text = f(text);
                }
            }
        }
    }
}

#[cfg(test)]
mod test_inbound;
