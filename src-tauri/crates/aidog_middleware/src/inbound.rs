//! 入站规则执行（请求侧条件，ADR 0003 统一引擎）。
//!
//! 挂载点：handler.rs 路由前（group 层）与 forward.rs 候选选定后（platform 层）。
//! 规则按 priority 升序堆叠执行；命中（条件树求值 true）后跑动作链，
//! block 终止一切；mask/override 替换请求文本中命中条件叶子 pattern 的片段；
//! inject 注入 system/body；warn 仅记日志；classify 属错误路径，入站忽略。
//!
//! 已知限制：chat_req 抽象层无请求 headers → request_headers 叶子恒不命中（文档化）。

use aidog_adapter::{ChatRequest, MessageContent, SystemContent};
use aidog_db::models::{ActionKind, MiddlewareSettings, Target};

use super::{collect_patterns, replace_match, CompiledRule, EvalView, MiddlewareEngine};

/// 入站执行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundOutcome {
    /// 放行（chat_req 可能已被 mask/inject 原地改写）。
    Continue,
    /// 拦截：写审计日志不计费，立即返回 4xx。
    Blocked {
        /// 命中规则标识（rule#id name）。
        blocked_by: String,
        /// 人读拦截原因。
        blocked_reason: String,
    },
}

impl MiddlewareEngine {
    /// 入站规则执行（路由前挂载点：group 层）。
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
        self.apply_inbound_inner(chat_req, group_key, None)
    }

    /// 入站规则执行（候选选定后挂载点：platform 层）。
    /// 与 [`apply_inbound`] 同一套规则堆叠（applies_to 已区分 group/platform 维度），
    /// 无旧三级 scope 概念，仅是第二个挂载点（platform id 此时才可用）。
    pub fn apply_inbound_platform(
        &self,
        settings: &MiddlewareSettings,
        chat_req: &mut ChatRequest,
        platform_id: i64,
    ) -> InboundOutcome {
        if !settings.enabled {
            return InboundOutcome::Continue;
        }
        self.apply_inbound_inner(chat_req, None, Some(platform_id))
    }

    fn apply_inbound_inner(
        &self,
        chat_req: &mut ChatRequest,
        group_key: Option<&str>,
        platform_id: Option<i64>,
    ) -> InboundOutcome {
        for cr in self.request_rules(group_key, platform_id, &chat_req.model) {
            // 每条规则求值前重新聚合文本（前序规则的 mask/inject 已改写请求）。
            let matched = {
                let view = EvalView {
                    req_text: collect_request_text(chat_req),
                    model: chat_req.model.as_str(),
                    ..Default::default()
                };
                cr.conditions.eval(&view)
            };
            if !matched {
                continue;
            }
            for step in &cr.rule.actions {
                match step.kind {
                    ActionKind::Block => {
                        return InboundOutcome::Blocked {
                            blocked_by: format!("rule#{} {}", cr.rule.id, cr.rule.name),
                            blocked_reason: if cr.rule.description.is_empty() {
                                "matched middleware rule".to_string()
                            } else {
                                cr.rule.description.clone()
                            },
                        };
                    }
                    ActionKind::Mask | ActionKind::Override => {
                        apply_rewrite_inbound(&cr, step, chat_req);
                    }
                    ActionKind::Inject => {
                        apply_inject(cr.rule.id, &step.params, chat_req);
                    }
                    ActionKind::Warn => {
                        tracing::warn!(
                            rule_id = cr.rule.id, rule_name = %cr.rule.name,
                            "middleware inbound: warn rule matched"
                        );
                    }
                    // classify 属错误路径（非 2xx 出站），入站忽略。
                    ActionKind::Classify => {}
                }
            }
        }
        InboundOutcome::Continue
    }
}

/// 注入指令：Value 层入口不碰 body 结构，由调用方（协议层）按 wire 协议写回。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundInject {
    /// 追加到 system / instructions / systemInstruction 侧。
    SystemAppend(String),
    /// 顶层 body 键赋值（键名由规则给定）。
    BodySet { target: String, value: String },
}

/// Value 层（同协议透传 body）入站改写的文本视图。
///
/// 协议知识留在调用方：调用方按 wire 协议把 body 内可读文本抽进 `system` / `messages`
/// 两组文本槽，引擎原地改写后再由调用方按同一顺序写回。
#[derive(Debug, Default)]
pub struct InboundTexts<'a> {
    /// 路由后的实际模型名（applies_to.models 与 model 叶子求值用）。
    pub model: &'a str,
    /// system 侧文本槽（anthropic `system` / responses `instructions` / gemini `systemInstruction`
    /// / openai role=system|developer 消息）。
    pub system: Vec<String>,
    /// 消息侧文本槽。
    pub messages: Vec<String>,
    /// 引擎产出的注入指令（调用方按协议写回）。
    pub injects: Vec<InboundInject>,
}

impl<'a> InboundTexts<'a> {
    pub fn new(model: &'a str) -> Self {
        Self { model, ..Default::default() }
    }

    /// 条件求值用的聚合文本（与 [`collect_request_text`] 同口径：system 在前，逐条换行）。
    fn aggregate(&self) -> String {
        let mut buf = String::new();
        for t in self.system.iter().chain(self.messages.iter()) {
            buf.push_str(t);
            buf.push('\n');
        }
        buf
    }
}

impl MiddlewareEngine {
    /// 入站规则执行（Value 层挂载点：同协议透传 body 的文本槽）。
    ///
    /// 只做 mask / override / inject —— **block 已在 chat_req 层（forward 分叉前）判定并返回**，
    /// 此处不重复拦截（同一套规则、同口径聚合文本，Value 层不会新命中一条 block）。
    ///
    /// 已知限制：本入口不带 body JSON path 上下文 → `request_body` 带 field 的叶子退化为
    /// 整文本匹配（与 chat_req 层同口径）。方向是「更容易命中」，不会漏掉用户配的脱敏规则。
    pub fn apply_inbound_texts(
        &self,
        settings: &MiddlewareSettings,
        texts: &mut InboundTexts<'_>,
        platform_id: i64,
    ) {
        if !settings.enabled {
            return;
        }
        for cr in self.request_rules(None, Some(platform_id), texts.model) {
            // 每条规则求值前重新聚合文本（前序规则的 mask 已改写）。
            let matched = {
                let view = EvalView {
                    req_text: texts.aggregate(),
                    model: texts.model,
                    ..Default::default()
                };
                cr.conditions.eval(&view)
            };
            if !matched {
                continue;
            }
            for step in &cr.rule.actions {
                match step.kind {
                    ActionKind::Mask | ActionKind::Override => {
                        rewrite_texts(&cr, step, texts);
                    }
                    ActionKind::Inject => {
                        collect_inject(cr.rule.id, &step.params, texts);
                    }
                    ActionKind::Warn => {
                        tracing::warn!(
                            rule_id = cr.rule.id, rule_name = %cr.rule.name,
                            "middleware inbound(body): warn rule matched"
                        );
                    }
                    // block 已在 chat_req 层返回，此处不可达；保守终止该规则剩余动作。
                    ActionKind::Block => break,
                    // classify 属错误路径（非 2xx 出站），入站忽略。
                    ActionKind::Classify => {}
                }
            }
        }
    }
}

/// Value 层 mask/override：与 [`apply_rewrite_inbound`] 同一套 pattern 来源与 fields 语义。
fn rewrite_texts(cr: &CompiledRule, step: &aidog_db::models::ActionStep, texts: &mut InboundTexts<'_>) {
    let leaves = collect_patterns(&cr.conditions, Target::RequestBody);
    if leaves.is_empty() {
        return;
    }
    let fields = &step.params.fields;
    let replacement = &step.params.replacement;
    let touch_system = fields.is_empty() || fields.iter().any(|f| f == "system");
    let touch_messages = fields.is_empty() || fields.iter().any(|f| f == "messages");
    let replace = |s: &str| -> String {
        let mut out = s.to_string();
        for p in &leaves {
            out = replace_match(p.match_type, &p.regex, &p.pattern, &out, replacement);
        }
        out
    };
    if touch_system {
        for t in texts.system.iter_mut() {
            *t = replace(t);
        }
    }
    if touch_messages {
        for t in texts.messages.iter_mut() {
            *t = replace(t);
        }
    }
}

/// Value 层 inject：只产出指令，写回由调用方按协议做（与 [`apply_inject`] 语义对齐）。
fn collect_inject(rule_id: i64, params: &aidog_db::models::ActionParams, texts: &mut InboundTexts<'_>) {
    match params.inject_mode.as_str() {
        "system_append" => texts.injects.push(InboundInject::SystemAppend(params.value.clone())),
        "body_set" => {
            if params.target.is_empty() {
                tracing::warn!(rule_id, "middleware inject body_set: empty target, skip");
                return;
            }
            texts.injects.push(InboundInject::BodySet {
                target: params.target.clone(),
                value: params.value.clone(),
            });
        }
        "header_set" => {
            tracing::debug!(rule_id, "middleware inject header_set: not supported at body layer, skipped");
        }
        other => {
            tracing::warn!(rule_id, mode = %other, "middleware inject: unknown inject_mode, skip");
        }
    }
}

/// mask/override 入站改写：把请求文本块中命中「条件树内 request_body 叶子 pattern」的
/// 片段替换为 replacement（regex 支持捕获组 $1）。fields 限定 messages/system（空 = 全部）。
fn apply_rewrite_inbound(cr: &CompiledRule, step: &aidog_db::models::ActionStep, chat_req: &mut ChatRequest) {
    let leaves = collect_patterns(&cr.conditions, Target::RequestBody);
    if leaves.is_empty() {
        return;
    }
    // fields 限定（mask 用；override 不限定）：空 = messages+system 全部。
    let fields = &step.params.fields;
    let replacement = &step.params.replacement;
    let touch_system = fields.is_empty() || fields.iter().any(|f| f == "system");
    let touch_messages = fields.is_empty() || fields.iter().any(|f| f == "messages");
    let replace = |s: &str| -> String {
        let mut out = s.to_string();
        for p in &leaves {
            out = replace_match(p.match_type, &p.regex, &p.pattern, &out, replacement);
        }
        out
    };
    if touch_system && let Some(sys) = chat_req.system.as_mut() {
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
    if touch_messages {
        for m in chat_req.messages.iter_mut() {
            map_text(&mut m.content, &replace);
        }
    }
}

/// inject 动作：按 inject_mode 注入。header_set 在入站无 HTTP 上下文 → 记日志跳过
///（入站 chat_req 抽象无 header；与旧引擎一致）。
fn apply_inject(rule_id: i64, params: &aidog_db::models::ActionParams, chat_req: &mut ChatRequest) {
    match params.inject_mode.as_str() {
        "system_append" => {
            let appended = match chat_req.system.take() {
                Some(SystemContent::Text(t)) => {
                    SystemContent::Text(format!("{t}\n{}", params.value))
                }
                Some(SystemContent::Blocks(mut blocks)) => {
                    blocks.push(serde_json::json!({ "type": "text", "text": params.value }));
                    SystemContent::Blocks(blocks)
                }
                None => SystemContent::Text(params.value.clone()),
            };
            chat_req.system = Some(appended);
        }
        "body_set" => {
            if params.target.is_empty() {
                tracing::warn!(rule_id, "middleware inject body_set: empty target, skip");
                return;
            }
            let extra = chat_req
                .extra
                .get_or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let Some(obj) = extra.as_object_mut() {
                obj.insert(params.target.clone(), serde_json::Value::String(params.value.clone()));
            }
        }
        "header_set" => {
            tracing::debug!(rule_id, "middleware inject header_set: not supported at inbound chat_req layer, skipped");
        }
        other => {
            tracing::warn!(rule_id, mode = %other, "middleware inject: unknown inject_mode, skip");
        }
    }
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

/// 遍历 MessageContent 内全部文本块（只读）。
fn for_each_text(content: &MessageContent, f: &mut dyn FnMut(&str)) {
    match content {
        MessageContent::Text(t) => f(t),
        MessageContent::Blocks(blocks) => {
            for b in blocks {
                if let aidog_adapter::ContentBlock::Text { text } = b {
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
                if let aidog_adapter::ContentBlock::Text { text } = b {
                    *text = f(text);
                }
            }
        }
    }
}
