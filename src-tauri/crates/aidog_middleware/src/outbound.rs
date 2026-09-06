// 出站规则执行（响应侧条件，ADR 0003 统一引擎）。
//
// 挂载点 = forward 返回后 / 回客户端前。
//   - 非流式 2xx：apply_outbound 对完整 body 应用 mask/override（替换命中条件叶子
//     response_body pattern 的片段）+ warn。block 在响应已到达后无法收回 → 忽略；
//     classify 属错误路径，不在此。
//   - 非 2xx：classify_error 求值响应侧规则，取链内 classify 步骤产出
//     ErrorClassification 喂现有重试编排（本层不引入熔断器）。
//   - 流式 SSE：StreamMasker（滑窗）逐块应用 mask/override，跨 chunk 边界不漏匹配
//     （尾窗延后一块下发，票 05）。流式挂载点仍不传 model / 上游响应头，故
//     applies_to.models 与 response_headers 条件在流式路径尚未生效。
//
// 非流式 2xx 与非 2xx 两条路径均传入请求 model 与上游响应头 JSON：
//   - model 取**客户端请求的模型名**（remap 前），与入站主挂载点 handler.rs 同口径；
//     平台侧 remap 后的 actual_model 属路由内部量，不作为 applies_to.models 的匹配对象。
//   - resp_headers 为 `{name: value}` JSON 字符串，header 名匹配大小写不敏感
//     （见 lib.rs::header_value）。

use std::sync::Arc;

use aidog_db::models::{ActionKind, MatchType, MiddlewareSettings, Target};

use super::{EvalView, MiddlewareEngine, collect_patterns, replace_match};

/// 含正则叶子时的尾窗保守上界（字节）。正则的最大匹配长度不可静态求（`.*` 等无界），
/// 取一个覆盖常见密钥/证件号长度的常数；literal 叶子按自身长度动态取，不吃这个上界。
const REGEX_WINDOW_BYTES: usize = 256;

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
        model: &str,
        resp_headers: Option<&str>,
    ) {
        if !settings.enabled {
            return;
        }
        for cr in self.response_rules(group_key, platform_id, model) {
            let view = EvalView {
                model,
                resp_body: Some(body.as_str()),
                resp_headers,
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
                                &leaf.validator,
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
    #[allow(clippy::too_many_arguments)]
    pub fn classify_error(
        &self,
        settings: &MiddlewareSettings,
        status: u16,
        body: &str,
        group_key: Option<&str>,
        platform_id: Option<i64>,
        model: &str,
        resp_headers: Option<&str>,
    ) -> Option<ErrorClassification> {
        if !settings.enabled {
            return None;
        }
        let view = EvalView {
            model,
            resp_body: Some(body),
            resp_headers,
            status: Some(status),
            ..Default::default()
        };
        for cr in self.response_rules(group_key, platform_id, model) {
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

    /// 流式 SSE 逐块改写：对单段文本应用 mask/override（与非流式同语义）。
    /// 返回改写后文本（无命中 → 原样返回）。跨 chunk 边界由 [`StreamMasker`] 的尾窗保证，
    /// 本函数只做块内替换，调用方须经 StreamMasker 而非直接逐 chunk 调用。
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
                                &leaf.validator,
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

    /// 建流式滑窗脱敏器（每条流一个，见 [`StreamMasker`]）。
    /// 尾窗大小按当前生效的响应侧 mask/override 规则动态取：literal 叶子取 pattern 字节长，
    /// 正则叶子取 [`REGEX_WINDOW_BYTES`]。无此类规则（或总开关 OFF）→ 窗口 0，纯透传零延迟。
    pub fn stream_masker(
        self: &Arc<Self>,
        settings: &MiddlewareSettings,
        group_key: Option<&str>,
        platform_id: Option<i64>,
    ) -> StreamMasker {
        let mut window = 0usize;
        if settings.enabled {
            for cr in self.response_rules(group_key, platform_id, "") {
                if !cr
                    .rule
                    .actions
                    .iter()
                    .any(|a| matches!(a.kind, ActionKind::Mask | ActionKind::Override))
                {
                    continue;
                }
                for leaf in collect_patterns(&cr.conditions, Target::ResponseBody) {
                    let w = match leaf.match_type {
                        MatchType::Regex => REGEX_WINDOW_BYTES,
                        MatchType::Contains | MatchType::Exact => leaf.pattern.len(),
                    };
                    window = window.max(w);
                }
            }
        }
        StreamMasker {
            engine: self.clone(),
            settings: settings.clone(),
            group_key: group_key.map(str::to_string),
            platform_id,
            window,
            pending: String::new(),
        }
    }
}

/// 流式脱敏滑窗：把每个 chunk 的**尾部 window 字节**扣住延到下一块再下发，
/// 于是被 SSE 分块切成两半的密钥/敏感词在拼接后仍能被同一套规则命中。
///
/// 首字延迟代价有明确上界：任一字节最多**延后一个 chunk**下发（不是延后到流末），
/// 且只有落在尾窗内的至多 `window` 字节会被延；window ≤ 256 字节（正则上界）或最长
/// literal pattern 长度。无 mask/override 响应规则时 window = 0，退化为零拷贝透传。
///
/// 生命周期 = 一条流。客户端断连时本结构随 stream 一起 Drop，扣住的字节从未下发 → 不泄漏；
/// 上游中断 / 流正常结束都必须调 [`StreamMasker::finish`] 把残留冲刷出去，不能吞。
pub struct StreamMasker {
    engine: Arc<MiddlewareEngine>,
    settings: MiddlewareSettings,
    group_key: Option<String>,
    platform_id: Option<i64>,
    window: usize,
    pending: String,
}

impl StreamMasker {
    /// 本流是否需要滑窗（无生效 mask/override 规则 → false，调用方可整段跳过）。
    pub fn is_active(&self) -> bool {
        self.window > 0
    }

    /// 吃进一个 chunk 文本，返回本次可安全下发的（已脱敏）文本。
    fn mask(&self, text: &str) -> String {
        self.engine.apply_outbound_stream_chunk(
            &self.settings,
            text,
            self.group_key.as_deref(),
            self.platform_id,
        )
    }

    pub fn push(&mut self, text: &str) -> String {
        if self.window == 0 {
            return self.mask(text);
        }
        self.pending.push_str(text);
        let mut masked = self.mask(&self.pending);
        // 扣住尾窗：不足一窗则整段扣住（跨 3+ 个极小 chunk 的敏感串也能拼齐）。
        let mut cut = masked.len().saturating_sub(self.window);
        while cut > 0 && !masked.is_char_boundary(cut) {
            cut -= 1;
        }
        self.pending = masked.split_off(cut);
        masked
    }

    /// 流末冲刷：返回尾窗内残留（已脱敏）。幂等，再调返回空串。
    pub fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }
        let out = self.mask(&self.pending);
        self.pending.clear();
        out
    }
}
