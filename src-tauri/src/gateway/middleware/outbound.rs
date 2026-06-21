// 出站规则执行（C3）。
//
// 挂载点 = proxy.rs forward 返回后 / 回客户端前。
//   - 非流式：拿到完整 body 字符串 → MiddlewareEngine::apply_outbound
//       · 状态码非 2xx → error_rule 分类 classify_error，产出 retryable/non-retryable
//         标记 + override_status/body 喂给现有重试编排（本树不引入熔断器）。
//       · 状态码 2xx → response_override/redaction/content_filter 改写 body。
//   - 流式 SSE：转发每个 chunk 时调 MiddlewareEngine::apply_outbound_stream_chunk
//     对 chunk 文本逐块应用 mask/override/sensitive 正则替换；error 由 HTTP 状态码
//     在转发前已判定（首块判定在 proxy 层用上游 status）。
//     **已知限制**：逐块替换在 SSE chunk 边界处可能漏匹配（密钥被切到两个 chunk），
//     滑窗跨块匹配列为后续（design 备注）。
//
// 熔断器**不在本树**：error_rule 仅产标记，group 调度树消费 retryable/auto_disabled 信号。

use serde::Deserialize;

use super::super::models::{MiddlewareSettings, RuleAction, RuleType};
use super::{
    builtin_detectors_match, default_replacement, mask_text, replace_match, CompiledRule,
    MiddlewareEngine, RedactionConfig,
};

/// 出站规则类型执行顺序（body 改写类）。
/// error_rule 单独由 [`MiddlewareEngine::classify_error`] 在非 2xx 路径处理，不在此列。
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
        group_key: Option<&str>,
        platform_id: Option<i64>,
    ) {
        if !settings.enabled {
            return;
        }
        for rule_type in OUTBOUND_ORDER {
            if !settings.type_enabled(rule_type) {
                continue;
            }
            let rules = self.resolve_rules(rule_type, group_key, platform_id);
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
        group_key: Option<&str>,
        platform_id: Option<i64>,
    ) -> Option<ErrorClassification> {
        if !settings.type_enabled(RuleType::ErrorRule) {
            return None;
        }
        let rules = self.resolve_rules(RuleType::ErrorRule, group_key, platform_id);
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
        group_key: Option<&str>,
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
            let rules = self.resolve_rules(rule_type, group_key, platform_id);
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

/// 通用文本命中判定（出站 warn 用；content_filter 空 pattern 用内置检测器）。
fn rule_matches_text(rule_type: RuleType, cr: &CompiledRule, text: &str) -> bool {
    if rule_type == RuleType::ContentFilter && cr.rule.pattern.is_empty() {
        return builtin_detectors_match(text);
    }
    cr.is_match(text)
}

#[cfg(test)]
mod test_outbound;
