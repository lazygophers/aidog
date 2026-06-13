//! 中间件规则引擎核心（C1 基座）。
//!
//! 职责：内存缓存（按 (rule_type, scope) 分桶）+ regex 预编译 + 三级作用域就近覆盖解析
//! + ReDoS 防护（编译失败/超限跳过，fail-open 不 panic）。
//!
//! 不负责：规则的实际执行（入站/出站 apply 属 C2/C3 在 proxy.rs）、内置 seed（C4）、UI（C5）。
//! 熔断器已移出中间件层（归 group 功能块独立 task），本文件不含任何熔断逻辑。
//!
//! 集成方式（决策）：MiddlewareEngine 为独立单例，由 Tauri `app.manage(Arc<MiddlewareEngine>)`
//! 持有；CRUD command 写库后调用 `engine.reload(&db)`。C2/C3 集成 proxy.rs 时把同一
//! `Arc<MiddlewareEngine>` 注入 ProxyState 即可（与 Db 平级，互不耦合）。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use regex::Regex;
use serde::Deserialize;

use super::adapter::{ChatRequest, MessageContent, SystemContent};
use super::db::{self, Db};
use super::models::{
    MatchType, MiddlewareRule, MiddlewareSettings, RuleAction, RuleScope, RuleType,
};

/// 正则编译大小上限（字节）。regex crate 用无回溯 DFA，本身抗 ReDoS；
/// 此上限进一步约束病态大模式的内存/编译开销。超限 → 编译失败 → 跳过该规则。
const REGEX_SIZE_LIMIT: usize = 1 << 20; // 1 MiB
/// DFA 状态缓存上限（字节）。
const REGEX_DFA_SIZE_LIMIT: usize = 1 << 20; // 1 MiB

/// 缓存中的已编译规则：原始规则 + 预编译正则（仅 match_type=regex 且编译成功时为 Some）。
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub rule: MiddlewareRule,
    /// 预编译正则；None 表示非 regex 匹配，或 regex 编译失败（已记日志，跳过匹配）。
    pub regex: Option<Arc<Regex>>,
}

impl CompiledRule {
    /// 文本是否命中本规则。regex 编译失败的规则（regex=None 且 match_type=Regex）永不命中（fail-open）。
    pub fn is_match(&self, text: &str) -> bool {
        match self.rule.match_type {
            MatchType::Regex => self.regex.as_ref().map(|re| re.is_match(text)).unwrap_or(false),
            MatchType::Contains => text.contains(&self.rule.pattern),
            MatchType::Exact => text == self.rule.pattern,
        }
    }
}

/// 分桶 key：按 (rule_type, scope) 分组缓存。
type BucketKey = (RuleType, RuleScope);

/// 中间件引擎单例。内部 RwLock 保护分桶缓存，读多写少（仅 CRUD 触发 reload）。
#[derive(Debug, Default)]
pub struct MiddlewareEngine {
    buckets: RwLock<HashMap<BucketKey, Vec<CompiledRule>>>,
}

impl MiddlewareEngine {
    pub fn new() -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
        }
    }

    /// 从规则列表重建分桶缓存（预编译 regex）。纯内存操作，便于单测直接喂规则。
    /// 只缓存 enabled 规则；disabled 规则不进桶（resolve 时无需再过滤 enabled）。
    pub fn rebuild_from_rules(&self, rules: Vec<MiddlewareRule>) {
        let mut buckets: HashMap<BucketKey, Vec<CompiledRule>> = HashMap::new();
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            let regex = if rule.match_type == MatchType::Regex {
                match compile_regex(&rule.pattern) {
                    Some(re) => Some(Arc::new(re)),
                    None => {
                        // ReDoS / 病态模式防护：编译失败 → 记日志 + regex=None（永不命中），不 panic。
                        tracing::warn!(
                            rule_id = rule.id,
                            rule_name = %rule.name,
                            pattern = %rule.pattern,
                            "middleware: regex compile failed/over-limit, rule will never match (fail-open)"
                        );
                        None
                    }
                }
            } else {
                None
            };
            let key = (rule.rule_type, rule.scope);
            buckets
                .entry(key)
                .or_default()
                .push(CompiledRule { rule, regex });
        }
        // 桶内已由 db 层按 priority/id 排序；rebuild 保持插入序即可。
        if let Ok(mut guard) = self.buckets.write() {
            *guard = buckets;
        } else {
            tracing::error!("middleware: buckets RwLock poisoned on rebuild");
        }
    }

    /// 从 DB 重新加载全部规则并重建缓存。CRUD 写库后调用。
    pub async fn reload(&self, db: &Db) -> Result<(), String> {
        let rules = db::list_middleware_rules(db).await?;
        let count = rules.len();
        self.rebuild_from_rules(rules);
        tracing::debug!(rule_count = count, "middleware: cache reloaded");
        Ok(())
    }

    /// 作用域就近覆盖解析（CSS 级联语义）：
    /// platform 层(scope=platform, scope_ref=platform_id) 该类型有规则 → 用之；
    /// 否则 group 层(scope=group, scope_ref=group_name) 有规则 → 用之；
    /// 否则 global 层。同 rule_type 内只生效最细粒度存在的那一层（非累加）。
    ///
    /// C2/C3 入站/出站执行层调用。
    pub fn resolve_rules(
        &self,
        rule_type: RuleType,
        group_name: Option<&str>,
        platform_id: Option<i64>,
    ) -> Vec<CompiledRule> {
        let guard = match self.buckets.read() {
            Ok(g) => g,
            Err(_) => {
                tracing::error!("middleware: buckets RwLock poisoned on resolve");
                return Vec::new();
            }
        };

        // platform 层（最细）
        if let Some(pid) = platform_id {
            let pid_str = pid.to_string();
            if let Some(bucket) = guard.get(&(rule_type, RuleScope::Platform)) {
                let matched: Vec<CompiledRule> = bucket
                    .iter()
                    .filter(|c| c.rule.scope_ref == pid_str)
                    .cloned()
                    .collect();
                if !matched.is_empty() {
                    return matched;
                }
            }
        }

        // group 层
        if let Some(gname) = group_name {
            if let Some(bucket) = guard.get(&(rule_type, RuleScope::Group)) {
                let matched: Vec<CompiledRule> = bucket
                    .iter()
                    .filter(|c| c.rule.scope_ref == gname)
                    .cloned()
                    .collect();
                if !matched.is_empty() {
                    return matched;
                }
            }
        }

        // global 层（兜底）
        guard
            .get(&(rule_type, RuleScope::Global))
            .cloned()
            .unwrap_or_default()
    }

    /// 仅解析 platform 层规则（不回退 group/global）。供入站「候选选定后」挂载点使用：
    /// 路由前已用 [`resolve_rules`]（platform_id=None）应用过 group/global 层，
    /// 此处只补 platform 层，避免就近覆盖语义下 group/global 被重复应用。
    /// platform 层非空 → 用之并覆盖（CSS 级联：本应只生效 platform 层，group/global 已应用属轻微叠加，
    /// 但 design 约定 platform 在候选后单独应用，且实战中 platform 与 group/global 规则集通常不重叠）。
    fn resolve_platform_only(&self, rule_type: RuleType, platform_id: i64) -> Vec<CompiledRule> {
        let guard = match self.buckets.read() {
            Ok(g) => g,
            Err(_) => {
                tracing::error!("middleware: buckets RwLock poisoned on resolve_platform_only");
                return Vec::new();
            }
        };
        let pid_str = platform_id.to_string();
        guard
            .get(&(rule_type, RuleScope::Platform))
            .map(|bucket| {
                bucket
                    .iter()
                    .filter(|c| c.rule.scope_ref == pid_str)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 入站规则执行（路由前挂载点：global/group 层）。
    /// 顺序：request_filter → sensitive_word → redaction → content_filter → dynamic_injection。
    /// 返回 [`InboundOutcome`]：`Continue` 放行（可能已原地改写 chat_req），`Blocked` 拦截。
    pub fn apply_inbound(
        &self,
        settings: &MiddlewareSettings,
        chat_req: &mut ChatRequest,
        group_name: Option<&str>,
    ) -> InboundOutcome {
        if !settings.enabled {
            return InboundOutcome::Continue;
        }
        for rule_type in INBOUND_ORDER {
            if !settings.type_enabled(rule_type) {
                continue;
            }
            let rules = self.resolve_rules(rule_type, group_name, None);
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

/// 内置密钥/邮箱检测正则（content_filter 类未配 pattern 时的兜底模式）。
/// 单一来源：proxy 与单测共用。匹配宽松优先（漏匹配可接受，误伤靠 fields 限定）。
const BUILTIN_SECRET_PATTERN: &str =
    r"(?i)(sk-[a-zA-Z0-9]{16,}|ghp_[a-zA-Z0-9]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_\-]{20,}|xox[baprs]-[a-zA-Z0-9\-]{10,})";
const BUILTIN_EMAIL_PATTERN: &str = r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}";

/// redaction/content_filter config 形状：`{ "replacement": "****", "fields": ["messages","system"] }`
#[derive(Debug, Deserialize)]
struct RedactionConfig {
    #[serde(default = "default_replacement")]
    replacement: String,
    #[serde(default)]
    fields: Vec<String>,
}
fn default_replacement() -> String {
    "****".to_string()
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
fn collect_request_text(chat_req: &ChatRequest) -> String {
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

/// 内置密钥/邮箱检测（缓存编译，编译失败 → false fail-open）。
fn builtin_detectors_match(text: &str) -> bool {
    use std::sync::OnceLock;
    static SECRET: OnceLock<Option<Regex>> = OnceLock::new();
    static EMAIL: OnceLock<Option<Regex>> = OnceLock::new();
    let secret = SECRET.get_or_init(|| compile_regex(BUILTIN_SECRET_PATTERN));
    let email = EMAIL.get_or_init(|| compile_regex(BUILTIN_EMAIL_PATTERN));
    secret.as_ref().map(|re| re.is_match(text)).unwrap_or(false)
        || email.as_ref().map(|re| re.is_match(text)).unwrap_or(false)
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

/// 对单段文本执行替换：内置检测器逐 match 替换；否则按 match_type 替换。
fn mask_text(
    s: &str,
    _rule_type: RuleType,
    cr: &CompiledRule,
    use_builtin: bool,
    replacement: &str,
) -> String {
    if use_builtin {
        let masked = builtin_replace_all(s, replacement);
        return masked;
    }
    match cr.rule.match_type {
        MatchType::Regex => match cr.regex.as_ref() {
            Some(re) => re.replace_all(s, replacement).into_owned(),
            None => s.to_string(), // 编译失败 fail-open
        },
        MatchType::Contains => {
            if cr.rule.pattern.is_empty() {
                s.to_string()
            } else {
                s.replace(&cr.rule.pattern, replacement)
            }
        }
        MatchType::Exact => {
            if s == cr.rule.pattern {
                replacement.to_string()
            } else {
                s.to_string()
            }
        }
    }
}

/// 内置密钥/邮箱替换（编译失败 → 原样返回）。
fn builtin_replace_all(s: &str, replacement: &str) -> String {
    use std::sync::OnceLock;
    static SECRET: OnceLock<Option<Regex>> = OnceLock::new();
    static EMAIL: OnceLock<Option<Regex>> = OnceLock::new();
    let secret = SECRET.get_or_init(|| compile_regex(BUILTIN_SECRET_PATTERN));
    let email = EMAIL.get_or_init(|| compile_regex(BUILTIN_EMAIL_PATTERN));
    let mut out = s.to_string();
    if let Some(re) = secret.as_ref() {
        out = re.replace_all(&out, replacement).into_owned();
    }
    if let Some(re) = email.as_ref() {
        out = re.replace_all(&out, replacement).into_owned();
    }
    out
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
                if let super::adapter::ContentBlock::Text { text } = b {
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
                if let super::adapter::ContentBlock::Text { text } = b {
                    *text = f(text);
                }
            }
        }
    }
}

/// 编译正则，附带 size/dfa 上限防护。失败返回 None（调用方记日志 + 跳过）。
fn compile_regex(pattern: &str) -> Option<Regex> {
    regex::RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
        .ok()
}

// ════════════════════════════════════════════════════════════════════════
// 出站规则执行（C3）
// ════════════════════════════════════════════════════════════════════════
//
// 挂载点 = proxy.rs forward 返回后 / 回客户端前。
//   - 非流式：拿到完整 body 字符串 → [`MiddlewareEngine::apply_outbound`]
//       · 状态码非 2xx → error_rule 分类 [`classify_error`]，产出 retryable/non-retryable
//         标记 + override_status/body 喂给现有重试编排（本树不引入熔断器）。
//       · 状态码 2xx → response_override/redaction/content_filter 改写 body。
//   - 流式 SSE：转发每个 chunk 时调 [`MiddlewareEngine::apply_outbound_stream_chunk`]
//     对 chunk 文本逐块应用 mask/override/sensitive 正则替换；error 由 HTTP 状态码
//     在转发前已判定（首块判定在 proxy 层用上游 status）。
//     **已知限制**：逐块替换在 SSE chunk 边界处可能漏匹配（密钥被切到两个 chunk），
//     滑窗跨块匹配列为后续（design 备注）。
//
// 熔断器**不在本树**：error_rule 仅产标记，group 调度树消费 retryable/auto_disabled 信号。

/// 出站规则类型执行顺序（body 改写类）。
/// error_rule 单独由 [`classify_error`] 在非 2xx 路径处理，不在此列。
const OUTBOUND_ORDER: [RuleType; 3] = [
    RuleType::ResponseOverride,
    RuleType::Redaction,
    RuleType::ContentFilter,
];

/// error_rule config：`{ "category":"...", "override_status": 400, "override_body": {...}, "retryable": false }`
#[derive(Debug, Deserialize)]
struct ErrorRuleConfig {
    #[serde(default)]
    category: String,
    #[serde(default)]
    override_status: Option<u16>,
    #[serde(default)]
    override_body: Option<serde_json::Value>,
    /// 缺省视为 true（可重试，换候选）；显式 false → non-retryable 立即返回。
    #[serde(default = "default_true_bool")]
    retryable: bool,
}
fn default_true_bool() -> bool {
    true
}

/// error_rule 分类结果。喂给现有重试编排：
/// - `retryable == false` → 重试编排立即返回不换候选（用 override_status/body 若有）。
/// - `retryable == true` → 继续换下个候选（默认重试语义不变）。
///
/// 熔断消费方在 group 树，本结构只是产出标记，不含任何熔断状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorClassification {
    /// 命中规则标识（error_rule#id name）。
    pub matched_by: String,
    /// 分类类别（config.category，人读/审计用）。
    pub category: String,
    /// 是否可重试。false → 立即返回不换候选。
    pub retryable: bool,
    /// 可选覆写状态码（回客户端用；None = 保持上游状态码）。
    pub override_status: Option<u16>,
    /// 可选覆写响应体（回客户端用；None = 保持上游 body）。
    pub override_body: Option<String>,
}

impl MiddlewareEngine {
    /// 出站非流式 body 改写：response_override/redaction/content_filter 顺序应用。
    /// 对 2xx 成功体调用；原地改写 `body` 字符串（脱敏/覆写）。fail-open：单条异常不阻断。
    /// 与入站脱敏幂等（已脱敏文本再扫不破坏，替换为同一 replacement 即可）。
    ///
    /// 总开关 OFF 或对应子开关 OFF → 不改写。
    pub fn apply_outbound(
        &self,
        settings: &MiddlewareSettings,
        body: &mut String,
        group_name: Option<&str>,
        platform_id: Option<i64>,
    ) {
        if !settings.enabled {
            return;
        }
        for rule_type in OUTBOUND_ORDER {
            if !settings.type_enabled(rule_type) {
                continue;
            }
            let rules = self.resolve_rules(rule_type, group_name, platform_id);
            for cr in &rules {
                apply_outbound_body_rule(rule_type, cr, body);
            }
        }
    }

    /// error_rule 分类：上游状态码非 2xx 时，按 error_rule 规则匹配（pattern 命中
    /// 上游错误 body 或 status 文本）产出 [`ErrorClassification`]。无命中 → None（走默认重试语义）。
    ///
    /// 命中多条 → 取第一条（priority 升序，已在 resolve 排好）。
    /// 总开关 OFF 或 error_rule 子开关 OFF → None（不分类，默认重试）。
    pub fn classify_error(
        &self,
        settings: &MiddlewareSettings,
        status: u16,
        body: &str,
        group_name: Option<&str>,
        platform_id: Option<i64>,
    ) -> Option<ErrorClassification> {
        if !settings.type_enabled(RuleType::ErrorRule) {
            return None;
        }
        let rules = self.resolve_rules(RuleType::ErrorRule, group_name, platform_id);
        // 匹配文本 = 状态码 + 上游 body（pattern 可命中其一）。
        let haystack = format!("{status}\n{body}");
        for cr in &rules {
            // pattern 为空 → 视为「任意非 2xx 均命中」（用于纯按状态码分类的规则）。
            let matched = cr.rule.pattern.is_empty() || cr.is_match(&haystack);
            if !matched {
                continue;
            }
            let cfg: ErrorRuleConfig = serde_json::from_str(&cr.rule.config).unwrap_or(ErrorRuleConfig {
                category: String::new(),
                override_status: None,
                override_body: None,
                retryable: true,
            });
            let override_body = cfg.override_body.as_ref().map(|v| {
                if let serde_json::Value::String(s) = v {
                    s.clone()
                } else {
                    serde_json::to_string(v).unwrap_or_default()
                }
            });
            return Some(ErrorClassification {
                matched_by: format!("error_rule#{} {}", cr.rule.id, cr.rule.name),
                category: cfg.category,
                retryable: cfg.retryable,
                override_status: cfg.override_status,
                override_body,
            });
        }
        None
    }

    /// 流式 SSE 逐块改写：对单个 chunk 的文本应用 redaction/content_filter(mask) +
    /// sensitive_word(命中替换为 replacement) + response_override(regex 替换)。
    /// 返回改写后文本（无规则命中 → 原样返回）。
    ///
    /// 逐块正则替换；**已知限制**：跨 chunk 边界的密钥/敏感词可能漏匹配（被切两半），
    /// 滑窗跨块匹配后续实现（design 备注）。block/inject/classify 动作流式不适用（跳过）。
    /// 总开关 OFF / 子开关 OFF → 原样返回。
    pub fn apply_outbound_stream_chunk(
        &self,
        settings: &MiddlewareSettings,
        text: &str,
        group_name: Option<&str>,
        platform_id: Option<i64>,
    ) -> String {
        if !settings.enabled {
            return text.to_string();
        }
        let mut out = text.to_string();
        for rule_type in [
            RuleType::ResponseOverride,
            RuleType::Redaction,
            RuleType::ContentFilter,
            RuleType::SensitiveWord,
        ] {
            if !settings.type_enabled(rule_type) {
                continue;
            }
            let rules = self.resolve_rules(rule_type, group_name, platform_id);
            for cr in &rules {
                out = rewrite_chunk_text(rule_type, cr, &out);
            }
        }
        out
    }
}

/// 对完整非流式 body 应用单条出站改写规则（mask/override）。
/// content_filter 未配 pattern → 内置密钥/邮箱检测器替换。fail-open。
fn apply_outbound_body_rule(rule_type: RuleType, cr: &CompiledRule, body: &mut String) {
    match cr.rule.action {
        RuleAction::Mask | RuleAction::Override => {
            let cfg: RedactionConfig = serde_json::from_str(&cr.rule.config).unwrap_or(RedactionConfig {
                replacement: default_replacement(),
                fields: Vec::new(),
            });
            let use_builtin = rule_type == RuleType::ContentFilter && cr.rule.pattern.is_empty();
            *body = mask_text(body, rule_type, cr, use_builtin, &cfg.replacement);
        }
        // warn/block/inject/classify 在出站 body 改写阶段无副作用（block/classify 由
        // error_rule 路径处理；inject 属入站；warn 仅记日志）。
        RuleAction::Warn => {
            if rule_matches_text(rule_type, cr, body) {
                tracing::warn!(
                    rule_id = cr.rule.id, rule_name = %cr.rule.name,
                    rule_type = %rule_type.as_str(), "middleware outbound: warn rule matched"
                );
            }
        }
        RuleAction::Block | RuleAction::Inject | RuleAction::Classify => {}
    }
}

/// 流式 chunk 文本改写（与非流式 body 同语义，逐块）。
fn rewrite_chunk_text(rule_type: RuleType, cr: &CompiledRule, text: &str) -> String {
    match cr.rule.action {
        RuleAction::Mask | RuleAction::Override => {
            let cfg: RedactionConfig = serde_json::from_str(&cr.rule.config).unwrap_or(RedactionConfig {
                replacement: default_replacement(),
                fields: Vec::new(),
            });
            let use_builtin = rule_type == RuleType::ContentFilter && cr.rule.pattern.is_empty();
            mask_text(text, rule_type, cr, use_builtin, &cfg.replacement)
        }
        // sensitive_word 流式：命中即替换为占位（不能 block 已发出的流，降级为 mask）。
        RuleAction::Block if rule_type == RuleType::SensitiveWord => {
            if cr.rule.pattern.is_empty() {
                text.to_string()
            } else {
                replace_match(cr, text, "****")
            }
        }
        _ => text.to_string(),
    }
}

/// 按 match_type 在文本中替换命中片段为 replacement（regex 编译失败 fail-open 原样）。
fn replace_match(cr: &CompiledRule, s: &str, replacement: &str) -> String {
    match cr.rule.match_type {
        MatchType::Regex => match cr.regex.as_ref() {
            Some(re) => re.replace_all(s, replacement).into_owned(),
            None => s.to_string(),
        },
        MatchType::Contains => {
            if cr.rule.pattern.is_empty() {
                s.to_string()
            } else {
                s.replace(&cr.rule.pattern, replacement)
            }
        }
        MatchType::Exact => {
            if s == cr.rule.pattern {
                replacement.to_string()
            } else {
                s.to_string()
            }
        }
    }
}

/// 通用文本命中判定（出站 warn 用；content_filter 空 pattern 用内置检测器）。
fn rule_matches_text(rule_type: RuleType, cr: &CompiledRule, text: &str) -> bool {
    if rule_type == RuleType::ContentFilter && cr.rule.pattern.is_empty() {
        return builtin_detectors_match(text);
    }
    cr.is_match(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::adapter::{ChatRequest, Message, MessageContent, Role, SystemContent};
    use super::super::models::{MatchType, MiddlewareRule, RuleAction, RuleScope, RuleType};

    fn mk_rule(
        id: i64,
        rule_type: RuleType,
        scope: RuleScope,
        scope_ref: &str,
        match_type: MatchType,
        pattern: &str,
    ) -> MiddlewareRule {
        MiddlewareRule {
            id,
            name: format!("rule-{id}"),
            description: String::new(),
            rule_type,
            scope,
            scope_ref: scope_ref.to_string(),
            match_type,
            pattern: pattern.to_string(),
            action: RuleAction::Warn,
            config: "{}".to_string(),
            priority: 0,
            enabled: true,
            is_builtin: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn compiled_rule_match_semantics() {
        let contains = CompiledRule {
            rule: mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "secret"),
            regex: None,
        };
        assert!(contains.is_match("this has a secret inside"));
        assert!(!contains.is_match("nothing here"));

        let exact = CompiledRule {
            rule: mk_rule(2, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Exact, "exactly"),
            regex: None,
        };
        assert!(exact.is_match("exactly"));
        assert!(!exact.is_match("exactly not"));

        let re = mk_rule(3, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Regex, r"\bsk-[a-z0-9]{8}\b");
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![re]);
        let resolved = engine.resolve_rules(RuleType::SensitiveWord, None, None);
        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].is_match("token sk-abcd1234 ok"));
        assert!(!resolved[0].is_match("token sk-XY end"));
    }

    #[test]
    fn redos_invalid_regex_skipped_not_panic() {
        // 非法正则（未闭合分组）→ 编译失败 → regex=None → 永不命中，不 panic。
        let bad = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Regex, "(unclosed");
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![bad]);
        let resolved = engine.resolve_rules(RuleType::ContentFilter, None, None);
        assert_eq!(resolved.len(), 1, "rule still cached even if regex failed");
        assert!(resolved[0].regex.is_none());
        assert!(!resolved[0].is_match("anything"), "failed-regex rule never matches");
    }

    #[test]
    fn resolve_scope_nearest_override_platform_wins() {
        let g = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "global");
        let grp = mk_rule(2, RuleType::Redaction, RuleScope::Group, "team-a", MatchType::Contains, "group");
        let plat = mk_rule(3, RuleType::Redaction, RuleScope::Platform, "42", MatchType::Contains, "platform");
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![g, grp, plat]);

        // platform 命中 → 只用 platform 层
        let r = engine.resolve_rules(RuleType::Redaction, Some("team-a"), Some(42));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].rule.pattern, "platform");
    }

    #[test]
    fn resolve_scope_falls_back_to_group_then_global() {
        let g = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "global");
        let grp = mk_rule(2, RuleType::Redaction, RuleScope::Group, "team-a", MatchType::Contains, "group");
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![g, grp]);

        // platform 层无该类型规则 → 落 group
        let r = engine.resolve_rules(RuleType::Redaction, Some("team-a"), Some(99));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].rule.pattern, "group");

        // group 不匹配 group_name → 落 global
        let r2 = engine.resolve_rules(RuleType::Redaction, Some("other-team"), Some(99));
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].rule.pattern, "global");

        // 无 group/platform 上下文 → global
        let r3 = engine.resolve_rules(RuleType::Redaction, None, None);
        assert_eq!(r3.len(), 1);
        assert_eq!(r3[0].rule.pattern, "global");
    }

    #[test]
    fn resolve_isolates_by_rule_type() {
        let red = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "red");
        let flt = mk_rule(2, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "flt");
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![red, flt]);

        let r = engine.resolve_rules(RuleType::Redaction, None, None);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].rule.pattern, "red");
        // 不存在的类型 → 空
        let none = engine.resolve_rules(RuleType::ErrorRule, None, None);
        assert!(none.is_empty());
    }

    #[test]
    fn disabled_rules_excluded_from_cache() {
        let mut disabled = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "x");
        disabled.enabled = false;
        let enabled = mk_rule(2, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "y");
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![disabled, enabled]);
        let r = engine.resolve_rules(RuleType::Redaction, None, None);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].rule.pattern, "y");
    }

    #[test]
    fn rebuild_reloads_cache() {
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "v1")]);
        assert_eq!(engine.resolve_rules(RuleType::Redaction, None, None)[0].rule.pattern, "v1");
        // 模拟 CRUD 写后 reload
        engine.rebuild_from_rules(vec![mk_rule(2, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "v2")]);
        let r = engine.resolve_rules(RuleType::Redaction, None, None);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].rule.pattern, "v2");
    }

    // ─── 入站 apply 测试（C2） ───────────────────────────────

    fn settings_all_on() -> MiddlewareSettings {
        MiddlewareSettings::default()
    }

    fn user_msg(text: &str) -> Message {
        Message {
            role: Role::User,
            content: MessageContent::Text(text.to_string()),
        }
    }

    fn mk_req(messages: Vec<Message>, system: Option<&str>) -> ChatRequest {
        ChatRequest {
            model: "m".to_string(),
            messages,
            system: system.map(|s| SystemContent::Text(s.to_string())),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: None,
            tools: None,
            tool_choice: None,
            extra: None,
        }
    }

    /// 提取 chat_req 全部文本（测试断言用）。
    fn dump_text(req: &ChatRequest) -> String {
        collect_request_text(req)
    }

    #[test]
    fn inbound_block_on_sensitive_word() {
        let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
        rule.action = RuleAction::Block;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut req = mk_req(vec![user_msg("this is forbidden content")], None);
        let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
        match outcome {
            InboundOutcome::Blocked { blocked_by, .. } => {
                assert!(blocked_by.contains("sensitive_word"));
            }
            _ => panic!("expected Blocked"),
        }

        // 不含敏感词 → 放行
        let mut ok = mk_req(vec![user_msg("clean text")], None);
        assert_eq!(engine.apply_inbound(&settings_all_on(), &mut ok, None), InboundOutcome::Continue);
    }

    #[test]
    fn inbound_mask_redaction_rewrites_messages_and_system() {
        let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "topsecret");
        rule.action = RuleAction::Mask;
        rule.config = r#"{"replacement":"[REDACTED]","fields":["messages","system"]}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut req = mk_req(vec![user_msg("here is topsecret data")], Some("system topsecret note"));
        let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
        assert_eq!(outcome, InboundOutcome::Continue);
        let text = dump_text(&req);
        assert!(!text.contains("topsecret"), "secret should be masked: {text}");
        assert!(text.contains("[REDACTED]"));
    }

    #[test]
    fn inbound_content_filter_masks_builtin_secret_and_email() {
        // content_filter 未配 pattern → 内置密钥/邮箱检测器
        let mut rule = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "");
        rule.action = RuleAction::Mask;
        rule.config = r#"{"replacement":"****"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut req = mk_req(
            vec![user_msg("my key sk-abcdefghijklmnop1234 and mail bob@example.com")],
            None,
        );
        let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
        assert_eq!(outcome, InboundOutcome::Continue);
        let text = dump_text(&req);
        assert!(!text.contains("sk-abcdefghijklmnop1234"), "secret masked: {text}");
        assert!(!text.contains("bob@example.com"), "email masked: {text}");
        assert!(text.contains("****"));
    }

    #[test]
    fn inbound_content_filter_blocks_builtin_secret() {
        let mut rule = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "");
        rule.action = RuleAction::Block;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut req = mk_req(vec![user_msg("token ghp_abcdefghijklmnopqrstuvwxyz")], None);
        match engine.apply_inbound(&settings_all_on(), &mut req, None) {
            InboundOutcome::Blocked { .. } => {}
            _ => panic!("expected Blocked on secret"),
        }
    }

    #[test]
    fn inbound_inject_system_append() {
        let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
        rule.action = RuleAction::Inject;
        rule.config = r#"{"inject_mode":"system_append","value":"INJECTED_POLICY"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut req = mk_req(vec![user_msg("hi")], Some("base system"));
        let outcome = engine.apply_inbound(&settings_all_on(), &mut req, None);
        assert_eq!(outcome, InboundOutcome::Continue);
        match &req.system {
            Some(SystemContent::Text(t)) => {
                assert!(t.contains("base system"));
                assert!(t.contains("INJECTED_POLICY"));
            }
            _ => panic!("expected appended system text"),
        }
    }

    #[test]
    fn inbound_inject_body_set_writes_extra() {
        let mut rule = mk_rule(1, RuleType::DynamicInjection, RuleScope::Global, "", MatchType::Contains, "");
        rule.action = RuleAction::Inject;
        rule.config = r#"{"inject_mode":"body_set","target":"x_custom","value":"v1"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut req = mk_req(vec![user_msg("hi")], None);
        let _ = engine.apply_inbound(&settings_all_on(), &mut req, None);
        let extra = req.extra.expect("extra set");
        assert_eq!(extra.get("x_custom").and_then(|v| v.as_str()), Some("v1"));
    }

    #[test]
    fn inbound_fail_open_on_bad_regex() {
        // 非法正则 block 规则 → regex=None → 永不命中 → fail-open 放行，不 panic。
        let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Regex, "(unclosed");
        rule.action = RuleAction::Block;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut req = mk_req(vec![user_msg("anything (unclosed here")], None);
        assert_eq!(
            engine.apply_inbound(&settings_all_on(), &mut req, None),
            InboundOutcome::Continue,
            "bad regex rule must fail-open"
        );
    }

    #[test]
    fn inbound_skipped_when_master_off() {
        let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
        rule.action = RuleAction::Block;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let settings = MiddlewareSettings { enabled: false, ..Default::default() };
        let mut req = mk_req(vec![user_msg("forbidden")], None);
        assert_eq!(engine.apply_inbound(&settings, &mut req, None), InboundOutcome::Continue);
    }

    #[test]
    fn inbound_skipped_when_type_toggle_off() {
        let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
        rule.action = RuleAction::Block;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut settings = MiddlewareSettings::default();
        settings.type_toggles.insert("sensitive_word".to_string(), false);
        let mut req = mk_req(vec![user_msg("forbidden")], None);
        assert_eq!(engine.apply_inbound(&settings, &mut req, None), InboundOutcome::Continue);
    }

    #[test]
    fn inbound_platform_only_does_not_apply_global() {
        // platform 层挂载点不应触发 global 规则（避免重复应用）。
        let mut g = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
        g.action = RuleAction::Block;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![g]);

        let mut req = mk_req(vec![user_msg("forbidden")], None);
        // platform 层无规则 → Continue（global 不被 platform-only 触发）
        assert_eq!(engine.apply_inbound_platform(&settings_all_on(), &mut req, 42), InboundOutcome::Continue);

        // 加 platform 规则后才在 platform 挂载点命中
        let mut p = mk_rule(2, RuleType::SensitiveWord, RuleScope::Platform, "42", MatchType::Contains, "blockme");
        p.action = RuleAction::Block;
        engine.rebuild_from_rules(vec![p]);
        let mut req2 = mk_req(vec![user_msg("blockme now")], None);
        match engine.apply_inbound_platform(&settings_all_on(), &mut req2, 42) {
            InboundOutcome::Blocked { .. } => {}
            _ => panic!("expected platform block"),
        }
    }

    // ─── 出站 apply 测试（C3） ───────────────────────────────

    #[test]
    fn outbound_mask_redaction_rewrites_body() {
        let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "topsecret");
        rule.action = RuleAction::Mask;
        rule.config = r#"{"replacement":"[REDACTED]"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut body = r#"{"choices":[{"message":{"content":"here is topsecret data"}}]}"#.to_string();
        engine.apply_outbound(&settings_all_on(), &mut body, None, None);
        assert!(!body.contains("topsecret"), "secret must be masked: {body}");
        assert!(body.contains("[REDACTED]"));
    }

    #[test]
    fn outbound_content_filter_masks_builtin_secret() {
        // content_filter 未配 pattern → 内置密钥检测器替换上游响应体中的密钥。
        let mut rule = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "");
        rule.action = RuleAction::Mask;
        rule.config = r#"{"replacement":"****"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut body = r#"{"content":"your key is sk-abcdefghijklmnop1234 keep safe"}"#.to_string();
        engine.apply_outbound(&settings_all_on(), &mut body, None, None);
        assert!(!body.contains("sk-abcdefghijklmnop1234"), "secret in response must be masked: {body}");
        assert!(body.contains("****"));
    }

    #[test]
    fn outbound_response_override_regex_rewrites_body() {
        let mut rule = mk_rule(1, RuleType::ResponseOverride, RuleScope::Global, "", MatchType::Regex, r"gpt-4o");
        rule.action = RuleAction::Override;
        rule.config = r#"{"replacement":"model-x"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut body = r#"{"model":"gpt-4o","ok":true}"#.to_string();
        engine.apply_outbound(&settings_all_on(), &mut body, None, None);
        assert!(body.contains("model-x"));
        assert!(!body.contains("gpt-4o"));
    }

    #[test]
    fn outbound_idempotent_with_inbound_redaction() {
        // 入站已脱敏 → 出站再扫同一规则不应破坏（幂等：已是 replacement，pattern 不再命中）。
        let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "topsecret");
        rule.action = RuleAction::Mask;
        rule.config = r#"{"replacement":"****"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut body = r#"{"content":"already masked ****"}"#.to_string();
        let before = body.clone();
        engine.apply_outbound(&settings_all_on(), &mut body, None, None);
        assert_eq!(body, before, "masked body must be stable under re-scan");
    }

    #[test]
    fn classify_error_non_retryable_with_override() {
        let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "content_policy");
        rule.action = RuleAction::Classify;
        rule.config = r#"{"category":"content_filter","retryable":false,"override_status":400,"override_body":{"error":"blocked by policy"}}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let body = r#"{"error":{"message":"content_policy violation"}}"#;
        let c = engine
            .classify_error(&settings_all_on(), 400, body, None, None)
            .expect("should classify");
        assert!(!c.retryable, "must be non-retryable");
        assert_eq!(c.category, "content_filter");
        assert_eq!(c.override_status, Some(400));
        assert!(c.override_body.as_deref().unwrap().contains("blocked by policy"));
    }

    #[test]
    fn classify_error_retryable_default_when_unset() {
        // config 未设 retryable → 缺省 true（可重试，换候选）。
        let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "rate");
        rule.action = RuleAction::Classify;
        rule.config = r#"{"category":"rate_limit"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let c = engine
            .classify_error(&settings_all_on(), 429, "rate limited", None, None)
            .expect("should classify");
        assert!(c.retryable, "default retryable=true");
        assert_eq!(c.override_status, None);
        assert_eq!(c.override_body, None);
    }

    #[test]
    fn classify_error_empty_pattern_matches_any_nonsuccess() {
        // pattern 空 → 任意非 2xx 命中（纯按状态码分类）。
        let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "");
        rule.action = RuleAction::Classify;
        rule.config = r#"{"category":"server_error","retryable":true}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let c = engine
            .classify_error(&settings_all_on(), 500, "internal error", None, None)
            .expect("empty pattern matches any error");
        assert_eq!(c.category, "server_error");
    }

    #[test]
    fn classify_error_none_when_no_rule_matches() {
        let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "specific_token");
        rule.action = RuleAction::Classify;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        assert!(engine
            .classify_error(&settings_all_on(), 502, "bad gateway", None, None)
            .is_none());
    }

    #[test]
    fn classify_error_skipped_when_type_toggle_off() {
        let mut rule = mk_rule(1, RuleType::ErrorRule, RuleScope::Global, "", MatchType::Contains, "");
        rule.action = RuleAction::Classify;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let mut settings = MiddlewareSettings::default();
        settings.type_toggles.insert("error_rule".to_string(), false);
        assert!(engine.classify_error(&settings, 500, "err", None, None).is_none());
    }

    #[test]
    fn outbound_stream_chunk_masks_secret_per_chunk() {
        let mut rule = mk_rule(1, RuleType::ContentFilter, RuleScope::Global, "", MatchType::Contains, "");
        rule.action = RuleAction::Mask;
        rule.config = r#"{"replacement":"****"}"#.to_string();
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let chunk = r#"data: {"delta":{"text":"token sk-abcdefghijklmnop1234 end"}}"#;
        let out = engine.apply_outbound_stream_chunk(&settings_all_on(), chunk, None, None);
        assert!(!out.contains("sk-abcdefghijklmnop1234"), "secret in stream chunk masked: {out}");
        assert!(out.contains("****"));
    }

    #[test]
    fn outbound_stream_chunk_sensitive_word_replaced() {
        let mut rule = mk_rule(1, RuleType::SensitiveWord, RuleScope::Global, "", MatchType::Contains, "forbidden");
        rule.action = RuleAction::Block; // 流式降级为替换（已发出的流不能拦截）
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let chunk = r#"data: {"delta":{"text":"this is forbidden word"}}"#;
        let out = engine.apply_outbound_stream_chunk(&settings_all_on(), chunk, None, None);
        assert!(!out.contains("forbidden"), "sensitive word replaced in stream: {out}");
        assert!(out.contains("****"));
    }

    #[test]
    fn outbound_skipped_when_master_off() {
        let mut rule = mk_rule(1, RuleType::Redaction, RuleScope::Global, "", MatchType::Contains, "topsecret");
        rule.action = RuleAction::Mask;
        let engine = MiddlewareEngine::new();
        engine.rebuild_from_rules(vec![rule]);

        let settings = MiddlewareSettings { enabled: false, ..Default::default() };
        let mut body = "topsecret stays".to_string();
        engine.apply_outbound(&settings, &mut body, None, None);
        assert_eq!(body, "topsecret stays", "master off → no rewrite");

        let stream_out = engine.apply_outbound_stream_chunk(&settings, "topsecret", None, None);
        assert_eq!(stream_out, "topsecret");
    }
}
