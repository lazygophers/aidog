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
//!
//! 模块划分（纯结构搬移）：
//! - 本文件（mod）：引擎单例 + 缓存/解析 + regex 编译 + 共享脱敏/检测原语（mask/replace/builtin）。
//! - [`inbound`]：入站规则执行（C2，apply_inbound/apply_inbound_platform）。
//! - [`outbound`]：出站规则执行（C3，apply_outbound/classify_error/apply_outbound_stream_chunk）。

mod inbound;
mod outbound;

#[cfg(test)]
pub(crate) mod test_mod;

// 对外路径保持不变：`gateway::middleware::{InboundOutcome, ErrorClassification, ...}`。
pub use inbound::InboundOutcome;
// `classify_error` 返回此类型，属公开 API 表面；保留 `gateway::middleware::ErrorClassification`
// 路径不变（当前无外部命名引用，故 allow 以免 unused_imports）。
#[allow(unused_imports)]
pub use outbound::ErrorClassification;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use regex::Regex;
use serde::Deserialize;

use super::db::{self, Db};
use super::models::{MatchType, MiddlewareRule, RuleScope, RuleType};

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
    /// 否则 group 层(scope=group, scope_ref=group_key) 有规则 → 用之；
    /// 否则 global 层。同 rule_type 内只生效最细粒度存在的那一层（非累加）。
    ///
    /// C2/C3 入站/出站执行层调用。
    pub fn resolve_rules(
        &self,
        rule_type: RuleType,
        group_key: Option<&str>,
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
        if let Some(gname) = group_key
            && let Some(bucket) = guard.get(&(rule_type, RuleScope::Group)) {
                let matched: Vec<CompiledRule> = bucket
                    .iter()
                    .filter(|c| c.rule.scope_ref == gname)
                    .cloned()
                    .collect();
                if !matched.is_empty() {
                    return matched;
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
    pub(super) fn resolve_platform_only(
        &self,
        rule_type: RuleType,
        platform_id: i64,
    ) -> Vec<CompiledRule> {
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
}

// ════════════════════════════════════════════════════════════════════════
// 共享原语：regex 编译 + 内置密钥/邮箱检测/替换 + redaction config + mask/replace
// （入站 C2 与出站 C3 共用，集中于此单一来源）
// ════════════════════════════════════════════════════════════════════════

/// 内置密钥/邮箱检测正则（content_filter 类未配 pattern 时的兜底模式）。
/// 单一来源：proxy 与单测共用。匹配宽松优先（漏匹配可接受，误伤靠 fields 限定）。
const BUILTIN_SECRET_PATTERN: &str =
    r"(?i)(sk-[a-zA-Z0-9]{16,}|ghp_[a-zA-Z0-9]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_\-]{20,}|xox[baprs]-[a-zA-Z0-9\-]{10,})";
const BUILTIN_EMAIL_PATTERN: &str = r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}";

/// redaction/content_filter config 形状：`{ "replacement": "****", "fields": ["messages","system"] }`
#[derive(Debug, Deserialize)]
pub(super) struct RedactionConfig {
    #[serde(default = "default_replacement")]
    pub(super) replacement: String,
    #[serde(default)]
    pub(super) fields: Vec<String>,
}
pub(super) fn default_replacement() -> String {
    "****".to_string()
}

/// 内置密钥/邮箱检测（缓存编译，编译失败 → false fail-open）。
pub(super) fn builtin_detectors_match(text: &str) -> bool {
    use std::sync::OnceLock;
    static SECRET: OnceLock<Option<Regex>> = OnceLock::new();
    static EMAIL: OnceLock<Option<Regex>> = OnceLock::new();
    let secret = SECRET.get_or_init(|| compile_regex(BUILTIN_SECRET_PATTERN));
    let email = EMAIL.get_or_init(|| compile_regex(BUILTIN_EMAIL_PATTERN));
    secret.as_ref().map(|re| re.is_match(text)).unwrap_or(false)
        || email.as_ref().map(|re| re.is_match(text)).unwrap_or(false)
}

/// 对单段文本执行替换：内置检测器逐 match 替换；否则按 match_type 替换。
pub(super) fn mask_text(
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

/// 按 match_type 在文本中替换命中片段为 replacement（regex 编译失败 fail-open 原样）。
pub(super) fn replace_match(cr: &CompiledRule, s: &str, replacement: &str) -> String {
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

/// 编译正则，附带 size/dfa 上限防护。失败返回 None（调用方记日志 + 跳过）。
fn compile_regex(pattern: &str) -> Option<Regex> {
    regex::RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
        .ok()
}
