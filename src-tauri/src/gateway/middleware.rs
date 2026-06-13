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

use super::db::{self, Db};
use super::models::{MatchType, MiddlewareRule, RuleScope, RuleType};

/// 正则编译大小上限（字节）。regex crate 用无回溯 DFA，本身抗 ReDoS；
/// 此上限进一步约束病态大模式的内存/编译开销。超限 → 编译失败 → 跳过该规则。
const REGEX_SIZE_LIMIT: usize = 1 << 20; // 1 MiB
/// DFA 状态缓存上限（字节）。
const REGEX_DFA_SIZE_LIMIT: usize = 1 << 20; // 1 MiB

/// 缓存中的已编译规则：原始规则 + 预编译正则（仅 match_type=regex 且编译成功时为 Some）。
///
/// 注：`rule`/`regex` 字段与 `is_match`/`resolve_rules` 是 C1 基座为 C2/C3 预留的消费面，
/// C1 先行落地（执行层入站/出站 apply 在 proxy.rs，属 C2/C3），故 lib 层暂无调用方。
/// 已有单测覆盖；保留 allow(dead_code) 待 C2/C3 接入后移除。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub rule: MiddlewareRule,
    /// 预编译正则；None 表示非 regex 匹配，或 regex 编译失败（已记日志，跳过匹配）。
    pub regex: Option<Arc<Regex>>,
}

impl CompiledRule {
    /// 文本是否命中本规则。regex 编译失败的规则（regex=None 且 match_type=Regex）永不命中（fail-open）。
    #[allow(dead_code)]
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
    /// C2/C3 入站/出站执行层调用；C1 先行，lib 层暂无调用方（单测已覆盖）。
    #[allow(dead_code)]
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
}

/// 编译正则，附带 size/dfa 上限防护。失败返回 None（调用方记日志 + 跳过）。
fn compile_regex(pattern: &str) -> Option<Regex> {
    regex::RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
