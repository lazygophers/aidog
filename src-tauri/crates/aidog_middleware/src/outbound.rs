// 出站规则执行（响应侧条件，ADR 0003 统一引擎）。
//
// 挂载点 = forward 返回后 / 回客户端前。
//   - 非流式 2xx：apply_outbound 对完整 body 应用 mask/override（替换命中条件叶子
//     response_body pattern 的片段）+ warn。block 在响应已到达后无法收回 → 忽略；
//     classify 属错误路径，不在此。
//   - 非 2xx：classify_error 求值响应侧规则，取链内 classify 步骤产出
//     ErrorClassification 喂现有重试编排（本层不引入熔断器）。
//   - 流式 SSE：apply_outbound_stream_chunk 逐块应用 mask/override。
//     **已知限制**：逐块替换在 chunk 边界处可能漏匹配（密钥被切到两个 chunk），
//     滑窗跨块匹配列为后续（design 备注）。
//
// response_headers 叶子挂载点无上游 header JSON（调用方未传）→ 恒不命中（文档化）。

use aidog_db::models::{ActionKind, MiddlewareSettings, Target};

use super::{EvalView, MiddlewareEngine, collect_patterns, replace_match};

/// error classify 结果。喂给现有重试编排：
/// - `retryable == false` → 重试编排立即返回不换候选（用 override_status/body 若有）。
/// - `retryable == true` → 继续换下个候选（默认重试语义不变）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorClassification {
    /// 命中规则标识（rule#id name）。
    pub matched_by: String,
    /// 分类类别（params.category，人读/审计用）。
    pub category: String,
    /// 是否可重试。false → 立即返回不换候选。
    pub retryable: bool,
    /// 可选覆写状态码（回客户端用；None = 保持上游状态码）。
    pub override_status: Option<u16>,
    /// 可选覆写响应体（回客户端用；None = 保持上游 body）。
    pub override_body: Option<String>,
}

impl MiddlewareEngine {
    /// 出站非流式 body 改写（2xx 路径）：按 priority 堆叠应用 mask/override/warn。
    /// 原地改写 `body` 字符串。fail-open：单条异常不阻断。
    /// 与入站脱敏幂等（已脱敏文本再扫替换为同一 replacement 即不破坏）。
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
        // 出站挂载点无请求 model 可用（chat_req 已消费）→ model 空串，
        // applies_to.models 非空的响应侧规则不在此命中（文档化限制）。
        for cr in self.response_rules(group_key, platform_id, "") {
            let view = EvalView {
                resp_body: Some(body.as_str()),
                ..Default::default()
            };
            if !cr.conditions.eval(&view) {
                continue;
            }
            for step in &cr.rule.actions {
                match step.kind {
                    ActionKind::Mask | ActionKind::Override => {
                        for leaf in collect_patterns(&cr.conditions, Target::ResponseBody) {
                            *body = replace_match(
                                leaf.match_type,
                                &leaf.regex,
                                &leaf.pattern,
                                body,
                                &step.params.replacement,
                            );
                        }
                    }
                    ActionKind::Warn => {
                        tracing::warn!(
                            rule_id = cr.rule.id, rule_name = %cr.rule.name,
                            "middleware outbound: warn rule matched"
                        );
                    }
                    // block：响应已到达无法收回；inject 属入站；classify 属非 2xx 路径。
                    ActionKind::Block | ActionKind::Inject | ActionKind::Classify => {}
                }
            }
        }
    }

    /// 错误分类（非 2xx 路径）：求值响应侧规则（status/response_body 条件），
    /// 取链内 classify 步骤产出 [`ErrorClassification`]。无命中 → None（走默认重试语义）。
    /// 命中多条 → 取第一条（priority 升序已在缓存排序）。
    pub fn classify_error(
        &self,
        settings: &MiddlewareSettings,
        status: u16,
        body: &str,
        group_key: Option<&str>,
        platform_id: Option<i64>,
    ) -> Option<ErrorClassification> {
        if !settings.enabled {
            return None;
        }
        let view = EvalView {
            resp_body: Some(body),
            status: Some(status),
            ..Default::default()
        };
        for cr in self.response_rules(group_key, platform_id, "") {
            if !cr.conditions.eval(&view) {
                continue;
            }
            let Some(step) = cr
                .rule
                .actions
                .iter()
                .find(|a| a.kind == ActionKind::Classify)
            else {
                continue;
            };
            return Some(ErrorClassification {
                matched_by: format!("rule#{} {}", cr.rule.id, cr.rule.name),
                category: step.params.category.clone(),
                retryable: step.params.retryable,
                override_status: step.params.override_status,
                override_body: step.params.override_body.clone(),
            });
        }
        None
    }

    /// 流式 SSE 逐块改写：对单个 chunk 文本应用 mask/override（与非流式同语义，逐块）。
    /// 返回改写后文本（无命中 → 原样返回）。
    ///
    /// **已知限制**：跨 chunk 边界的密钥/敏感词可能漏匹配（被切两半），滑窗后续实现。
    /// block/inject/classify 流式不适用（block 已发字节无法收回，由首块前的入站层负责）。
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
        for cr in self.response_rules(group_key, platform_id, "") {
            // 逐块无法用条件树做整体求值（条件可能依赖完整 body）——退化：
            // 命中叶子 pattern 的片段直接按链内 mask/override 替换（块内匹配）。
            let leaves = collect_patterns(&cr.conditions, Target::ResponseBody);
            if leaves.is_empty() {
                continue;
            }
            for step in &cr.rule.actions {
                match step.kind {
                    ActionKind::Mask | ActionKind::Override => {
                        for leaf in &leaves {
                            out = replace_match(
                                leaf.match_type,
                                &leaf.regex,
                                &leaf.pattern,
                                &out,
                                &step.params.replacement,
                            );
                        }
                    }
                    // warn/inject/classify 流式不适用：仅记日志（spec：disabled with a log line）。
                    other => {
                        tracing::debug!(
                            rule_id = cr.rule.id, action = %other.as_str(),
                            "middleware stream chunk: action not applicable, skipped"
                        );
                    }
                }
            }
        }
        out
    }
}
