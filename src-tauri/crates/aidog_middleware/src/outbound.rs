// 出站规则执行（响应侧条件，ADR 0003 统一引擎）。
//
// 挂载点 = forward 返回后 / 回客户端前。
//   - 非流式 2xx：apply_outbound 对完整 body 应用 mask/override（替换命中条件叶子
//     response_body pattern 的片段）+ warn。block 在响应已到达后无法收回 → 忽略；
//     classify 属错误路径，不在此。
//   - 非 2xx：classify_error 求值响应侧规则，取链内 classify 步骤产出
//     ErrorClassification 喂现有重试编排（本层不引入熔断器）。
//   - 流式 SSE：StreamMasker（滑窗）逐块应用 mask/override，跨 chunk 边界不漏匹配
//     （尾窗延后一块下发，票 05）。model 与两条非流式路径同口径传入（评审 F4），
//     故 applies_to.models 在流式下同样生效、窗口大小也把 models 限定的规则算进去。
//     上游响应头条件流式仍不生效（建流时逐块求值拿不到整段 body 语义，见下方注释）。
//
// 三条路径均传入请求 model 与上游响应头 JSON（流式无响应头）：
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
                    // block：响应已到达无法收回；inject 属入站；classify 属非 2xx 路径；
                    // budget_gate 属入站（check_budget）。
                    ActionKind::Block
                    | ActionKind::Inject
                    | ActionKind::Classify
                    | ActionKind::BudgetGate => {}
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
    /// 本函数只做块内替换，**只许经 [`StreamMasker`] 调用**（私有即为强制）。
    /// block/inject/classify 流式不适用（block 已发字节无法收回，由首块前的入站层负责）。
    pub(crate) fn apply_outbound_stream_chunk(
        &self,
        settings: &MiddlewareSettings,
        text: &str,
        group_key: Option<&str>,
        platform_id: Option<i64>,
        model: &str,
    ) -> String {
        if !settings.enabled {
            return text.to_string();
        }
        let mut out = text.to_string();
        for cr in self.response_rules(group_key, platform_id, model) {
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
        model: &str,
    ) -> StreamMasker {
        let mut window = 0usize;
        if settings.enabled {
            // model 必须参与筛选：漏传会让 models 限定的规则不进窗口计算 → 漏窗（评审 F4）。
            for cr in self.response_rules(group_key, platform_id, model) {
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
            model: model.to_string(),
            window,
            pending: String::new(),
            byte_tail: Vec::new(),
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
    model: String,
    window: usize,
    pending: String,
    /// UTF-8 边界缓冲：chunk 末尾**不完整的多字节序列**（1–3 字节）留到下一块再拼。
    byte_tail: Vec<u8>,
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
            &self.model,
        )
    }

    /// 吃进一个 chunk 的**原始字节**（流式挂载点的正式入口）。
    ///
    /// SSE 传输层可以在任意字节处切块，一个中文字符（3 字节）完全可能被切成两半。
    /// 这里先做 UTF-8 边界缓冲：末尾不完整的多字节序列扣下来留到下一块，与尾窗同一套
    /// 「延后下发」机制。**不用 `from_utf8_lossy` 兜底**——那会把半个汉字变成 `U+FFFD`，
    /// 拼回来的另一半也废了，正文被永久破坏（评审 F1）。
    ///
    /// 真正非法的字节（`error_len` 有值，不是被切断的前缀）不缓冲：留着也拼不回来，
    /// 缓冲区会无限涨；照旧 lossy 放行。
    pub fn push_bytes(&mut self, bytes: &[u8]) -> String {
        let buf = if self.byte_tail.is_empty() {
            bytes.to_vec()
        } else {
            let mut b = std::mem::take(&mut self.byte_tail);
            b.extend_from_slice(bytes);
            b
        };
        let split = match std::str::from_utf8(&buf) {
            Ok(_) => buf.len(),
            // error_len = None：输入到此为止，尾部是被切断的多字节序列前缀 → 留到下一块。
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            Err(_) => buf.len(),
        };
        self.byte_tail.extend_from_slice(&buf[split..]);
        if split == 0 {
            return String::new();
        }
        let text = String::from_utf8_lossy(&buf[..split]).into_owned();
        self.push(&text)
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
    /// UTF-8 边界缓冲里的残字节也一并放行：流已结束，另一半永远不会来了，此时
    /// lossy 是唯一选择（比静默吞掉字节好）。
    pub fn finish(&mut self) -> String {
        if !self.byte_tail.is_empty() {
            let tail = std::mem::take(&mut self.byte_tail);
            self.pending
                .push_str(&String::from_utf8_lossy(&tail));
        }
        if self.pending.is_empty() {
            return String::new();
        }
        let out = self.mask(&self.pending);
        self.pending.clear();
        out
    }
}
