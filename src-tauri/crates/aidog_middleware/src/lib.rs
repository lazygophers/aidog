//! 中间件规则引擎核心（统一引擎，ADR 0003）。
//!
//! 一条规则 = 嵌套条件树 + 有序动作链 + Applies To 过滤器。职责：
//! 内存缓存（enabled 且非 failed 规则，priority 升序）+ regex 预编译（ReDoS 防护，
//! 编译失败 fail-open 跳过）+ 条件树求值 + 动作链执行（block/classify 终止一切）。
//!
//! 不负责：CRUD 落库（aidog_db）、内置 seed（aidog_db::schema）、UI。
//! 熔断器不在中间件层（归 group 功能块）。
//!
//! 集成方式：MiddlewareEngine 独立单例，Tauri `app.manage(Arc<MiddlewareEngine>)`；
//! CRUD 写库后 `engine.reload(&db)`；ProxyState 注入同一 Arc。

mod inbound;
mod outbound;

#[cfg(test)]
pub(crate) mod test_mod;

pub use inbound::{InboundInject, InboundOutcome, InboundTexts};
#[allow(unused_imports)]
pub use outbound::ErrorClassification;

use std::sync::Arc;
use std::sync::RwLock;

use regex::Regex;
use serde_json::Value;

use aidog_db::Db;
use aidog_db::models::{
    ConditionLeaf, ConditionNode, MatchType, MiddlewareRule, Target,
};

/// 正则编译大小上限（字节）。regex crate 无回溯 DFA 本身抗 ReDoS；
/// 此上限进一步约束病态大模式。超限 → 编译失败 → 跳过该叶子（fail-open）。
const REGEX_SIZE_LIMIT: usize = 1 << 20; // 1 MiB
/// DFA 状态缓存上限（字节）。
const REGEX_DFA_SIZE_LIMIT: usize = 1 << 20; // 1 MiB

/// 编译后的条件树：叶子带预编译正则（仅 match_type=regex 且编译成功时 Some）。
#[derive(Debug, Clone)]
pub enum CompiledNode {
    All(Vec<CompiledNode>),
    Any(Vec<CompiledNode>),
    Leaf {
        leaf: ConditionLeaf,
        regex: Option<Arc<Regex>>,
    },
}

impl CompiledNode {
    /// 树内任一叶子是否响应侧（决定规则在出站阶段求值）。
    fn any_leaf<F: Fn(&ConditionLeaf) -> bool>(&self, f: &F) -> bool {
        match self {
            CompiledNode::All(cs) | CompiledNode::Any(cs) => cs.iter().any(|c| c.any_leaf(f)),
            CompiledNode::Leaf { leaf, .. } => f(leaf),
        }
    }

    fn is_response_side(&self) -> bool {
        self.any_leaf(&|l| l.target.is_response_side())
    }
}

/// 缓存中的已编译规则。
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub rule: MiddlewareRule,
    pub conditions: CompiledNode,
}

impl CompiledRule {
    /// Applies To 过滤：三维各自空 = 不限；多值 = 任一命中；三维间 AND。
    fn applies(&self, group_key: Option<&str>, platform_id: Option<i64>, model: &str) -> bool {
        let at = &self.rule.applies_to;
        let p_ok = at.platforms.is_empty()
            || platform_id.is_some_and(|pid| at.platforms.contains(&pid));
        let g_ok = at.groups.is_empty() || group_key.is_some_and(|gk| at.groups.iter().any(|g| g == gk));
        let m_ok = at.models.is_empty() || at.models.iter().any(|m| m == model);
        p_ok && g_ok && m_ok
    }
}

/// 条件求值的世界视图：按 target 取叶子文本。request 侧字段仅在入站有值，
/// response 侧仅在出站 / 错误路径有值（混阶段规则在保存时已被校验拒绝）。
#[derive(Default)]
pub(crate) struct EvalView<'a> {
    /// 请求侧聚合文本（messages+system 或透传原始 body）
    pub req_text: String,
    /// 请求 body 原始 JSON（JSON path 定位用；透传分支可为 None）
    pub req_body_json: Option<Value>,
    /// 请求 headers（JSON 对象字符串；chat_req 抽象层无 → None，header 叶子恒不命中）
    pub req_headers: Option<&'a str>,
    pub model: &'a str,
    pub resp_body: Option<&'a str>,
    pub resp_body_json: Option<Value>,
    pub resp_headers: Option<&'a str>,
    pub status: Option<u16>,
}

/// 按点分 JSON path 从 body 取值并字符串化。不存在 → None。
fn json_path<'v>(root: &'v Value, path: &str) -> Option<&'v Value> {
    if path.is_empty() {
        return Some(root);
    }
    let mut cur = root;
    for seg in path.split('.') {
        cur = match cur {
            Value::Object(m) => m.get(seg)?,
            Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// 从 headers JSON 字符串取 header 值（大小写不敏感；值非字符串则 to_string）。
fn header_value(headers_json: &str, name: &str) -> Option<String> {
    let m: Value = serde_json::from_str(headers_json).ok()?;
    let m = m.as_object()?;
    for (k, v) in m {
        if k.eq_ignore_ascii_case(name) {
            return Some(match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            });
        }
    }
    None
}

impl CompiledNode {
    /// 叶子文本命中判定（regex 编译失败 fail-open = 不命中）。
    fn leaf_matches(leaf: &ConditionLeaf, regex: &Option<Arc<Regex>>, view: &EvalView) -> bool {
        let text: Option<String> = match leaf.target {
            Target::RequestBody => {
                if leaf.field.is_empty() {
                    Some(view.req_text.clone())
                } else if let Some(json) = &view.req_body_json {
                    json_path(json, &leaf.field).map(value_to_text)
                } else {
                    // 无原始 body JSON（chat_req 抽象层）→ 聚合文本内无法定位 path，退化为整文本
                    Some(view.req_text.clone())
                }
            }
            Target::RequestHeaders => {
                view.req_headers.and_then(|h| header_value(h, &leaf.field))
            }
            Target::ResponseBody => {
                if leaf.field.is_empty() {
                    view.resp_body.map(|s| s.to_string())
                } else {
                    view.resp_body_json
                        .as_ref()
                        .and_then(|j| json_path(j, &leaf.field))
                        .map(value_to_text)
                }
            }
            Target::ResponseHeaders => {
                view.resp_headers.and_then(|h| header_value(h, &leaf.field))
            }
            Target::Status => view.status.map(|s| s.to_string()),
            Target::Model => Some(view.model.to_string()),
        };
        let Some(text) = text else { return false };
        match leaf.match_type {
            MatchType::Regex => regex.as_ref().map(|re| re.is_match(&text)).unwrap_or(false),
            MatchType::Contains => text.contains(&leaf.pattern),
            MatchType::Exact => text == leaf.pattern,
        }
    }

    /// 条件树求值。空组：All([]) = true（vacuous），Any([]) = false。
    pub(crate) fn eval(&self, view: &EvalView) -> bool {
        match self {
            CompiledNode::All(cs) => cs.iter().all(|c| c.eval(view)),
            CompiledNode::Any(cs) => cs.iter().any(|c| c.eval(view)),
            CompiledNode::Leaf { leaf, regex } => Self::leaf_matches(leaf, regex, view),
        }
    }
}

/// 中间件引擎单例。读多写少（仅 CRUD 触发 rebuild），RwLock 保护。
#[derive(Debug, Default)]
pub struct MiddlewareEngine {
    rules: RwLock<Vec<CompiledRule>>,
}

impl MiddlewareEngine {
    pub fn new() -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
        }
    }

    /// 从规则列表重建缓存（预编译 regex）。只收 enabled 且非 failed 规则，priority 升序。
    pub fn rebuild_from_rules(&self, rules: Vec<MiddlewareRule>) {
        let mut compiled = Vec::new();
        for rule in rules {
            if !rule.enabled || rule.failed {
                continue;
            }
            let conditions = compile_node(&rule.conditions);
            compiled.push(CompiledRule { rule, conditions });
        }
        if let Ok(mut guard) = self.rules.write() {
            *guard = compiled;
        } else {
            tracing::error!("middleware: rules RwLock poisoned on rebuild");
        }
    }

    /// 从 DB 重新加载全部规则并重建缓存。CRUD 写库后调用。
    pub async fn reload(&self, db: &Db) -> Result<(), String> {
        let rules = aidog_db::list_middleware_rules(db).await?;
        let count = rules.len();
        self.rebuild_from_rules(rules);
        tracing::debug!(rule_count = count, "middleware: cache reloaded");
        Ok(())
    }

    fn snapshot(&self) -> Vec<CompiledRule> {
        match self.rules.read() {
            Ok(g) => g.clone(),
            Err(_) => {
                tracing::error!("middleware: rules RwLock poisoned on snapshot");
                Vec::new()
            }
        }
    }

    /// 取按 applies_to 过滤后、指定阶段的规则（priority 已升序）。
    /// phase: false = 请求侧（入站），true = 响应侧（出站/错误路径）。
    fn phase_rules(
        &self,
        phase: bool,
        group_key: Option<&str>,
        platform_id: Option<i64>,
        model: &str,
    ) -> Vec<CompiledRule> {
        self.snapshot()
            .into_iter()
            .filter(|c| c.conditions.is_response_side() == phase)
            .filter(|c| c.applies(group_key, platform_id, model))
            .collect()
    }

    /// 入站侧规则（请求侧条件）。
    pub(crate) fn request_rules(
        &self,
        group_key: Option<&str>,
        platform_id: Option<i64>,
        model: &str,
    ) -> Vec<CompiledRule> {
        self.phase_rules(false, group_key, platform_id, model)
    }

    /// 出站侧规则（响应侧条件）。
    pub(crate) fn response_rules(
        &self,
        group_key: Option<&str>,
        platform_id: Option<i64>,
        model: &str,
    ) -> Vec<CompiledRule> {
        self.phase_rules(true, group_key, platform_id, model)
    }
}

/// 递归编译条件树（预编译叶子 regex；失败 fail-open = None，永不命中）。
fn compile_node(node: &ConditionNode) -> CompiledNode {
    match node {
        ConditionNode::All { children } => CompiledNode::All(children.iter().map(compile_node).collect()),
        ConditionNode::Any { children } => CompiledNode::Any(children.iter().map(compile_node).collect()),
        ConditionNode::Leaf(leaf) => CompiledNode::Leaf {
            leaf: leaf.clone(),
            regex: if leaf.match_type == MatchType::Regex {
                compile_regex(&leaf.pattern)
            } else {
                None
            },
        },
    }
}

/// 按 match_type 在文本中替换命中片段为 replacement（regex 支持捕获组 $1；编译失败 fail-open）。
pub(crate) fn replace_match(
    match_type: MatchType,
    regex: &Option<Arc<Regex>>,
    pattern: &str,
    s: &str,
    replacement: &str,
) -> String {
    match match_type {
        MatchType::Regex => match regex.as_ref() {
            Some(re) => re.replace_all(s, replacement).into_owned(),
            None => s.to_string(),
        },
        MatchType::Contains => {
            if pattern.is_empty() {
                s.to_string()
            } else {
                s.replace(pattern, replacement)
            }
        }
        MatchType::Exact => {
            if s == pattern {
                replacement.to_string()
            } else {
                s.to_string()
            }
        }
    }
}

/// 改写动作（mask/override）用的命中模式：条件树内指定 target 的叶子集合。
pub(crate) struct RewriteLeaf {
    pub match_type: MatchType,
    pub regex: Option<Arc<Regex>>,
    pub pattern: String,
}

/// 编译正则，附带 size/dfa 上限防护。失败返回 None（调用方记日志 + 跳过）。
fn compile_regex(pattern: &str) -> Option<Arc<Regex>> {
    regex::RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
        .ok()
        .map(Arc::new)
}

/// 收集条件树内指定 target 的叶子（mask/override 按这些 pattern 替换文本命中片段）。
pub(crate) fn collect_patterns(node: &CompiledNode, target: Target) -> Vec<RewriteLeaf> {
    fn walk(node: &CompiledNode, target: Target, out: &mut Vec<RewriteLeaf>) {
        match node {
            CompiledNode::All(cs) | CompiledNode::Any(cs) => cs.iter().for_each(|c| walk(c, target, out)),
            CompiledNode::Leaf { leaf, regex } => {
                if leaf.target == target {
                    out.push(RewriteLeaf {
                        match_type: leaf.match_type,
                        regex: regex.clone(),
                        pattern: leaf.pattern.clone(),
                    });
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(node, target, &mut out);
    out
}
