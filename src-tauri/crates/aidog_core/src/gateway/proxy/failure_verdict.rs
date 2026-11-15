//! 失败裁决（spec M1 / issue 55 票 03）：「判定」抽成纯函数 [`judge`]，「落地」收口到
//! [`apply`]。judge 无 I/O、无时钟（now 走入参），同一输入恒产同一 Verdict；
//! apply 是 scheduler / DB 记账副作用的唯一落地点。本票只处理 HTTP 非 2xx 输入；
//! 传输失败（`Failure::Transport`）在票 04 并入。

use super::*;

/// 上游失败的输入形态。
pub(crate) enum Failure<'a> {
    Http {
        code: u16,
        body: &'a str,
        /// `Retry-After` 头（须在 resp body 被消费前取，调用方负责）。
        retry_after: Option<&'a str>,
        /// registry 码义（`rate_limit` / `overloaded`），调用方按平台 wire 协议查出
        /// （`registry_code_meaning`）；码义优先于通用状态码分类。
        code_meaning: Option<&'a str>,
    },
}

/// 判定所需的请求上下文快照（纯数据，无状态句柄、无时钟）。
pub(crate) struct JudgeCtx {
    /// 分组是否健康感知路由（401/403 计熔断 vs 固定 auth 冷却的分界）。
    pub health_aware: bool,
    /// 命中槽位的模型链上是否还有下一个模型。
    pub has_next_model: bool,
    pub is_last_candidate: bool,
    /// 中间件 error_rule 把本错误标成 non-retryable（用户显式意图：不换模型也不换平台）。
    pub rule_non_retryable: bool,
    pub now_ms: i64,
}

/// 记账效果：枚举替换旧 `effects: Vec<String>`；`as_str()` 保持旧字符串，
/// 历史 proxy_log.attempts JSON 与前端 RoutingCanvas 渲染（`EFFECT_KEY` 9 键）不变。
/// 注意无 `Ignored` 变体：旧实现 record_ignored（inflight 配平）从不落 effects 字符串。
pub(crate) enum HealthEffect {
    BreakerCount,
    Penalty(super::scheduling::PenaltyTier),
    CooldownMinute {
        until_ms: i64,
    },
    CooldownQuota {
        until_ms: i64,
    },
    CooldownRateLimit {
        hint_ms: Option<i64>,
    },
    CooldownBalance,
    CooldownAuth,
    DisableCensorship,
}

impl HealthEffect {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::BreakerCount => "breaker_count",
            Self::Penalty(super::scheduling::PenaltyTier::Server) => "penalty_server",
            Self::Penalty(super::scheduling::PenaltyTier::Client) => "penalty_client",
            Self::CooldownMinute { .. } => "cooldown_minute",
            Self::CooldownQuota { .. } => "cooldown_quota",
            Self::CooldownRateLimit { .. } => "cooldown_rate_limit",
            Self::CooldownBalance => "cooldown_balance",
            Self::CooldownAuth => "cooldown_auth",
            Self::DisableCensorship => "disabled_censorship",
        }
    }
}

/// 裁决后的走向。
pub(crate) enum Next {
    /// 同平台换下一个模型重发（只留痕不记账）。
    SwitchModel,
    /// 换下个候选（排除范围见 [`Verdict::scope`]）。
    SwitchPlatform,
    /// 终态：把上游错误回客户端。
    Return,
}

/// 纯判定结果。
pub(crate) struct Verdict {
    pub class: UpstreamErrorClass,
    /// 排除范围：Service → 整个平台，其余（模型类 / 未知类）→ 只该组合。
    pub scope: ExcludeScope,
    /// true = 平台背锅：写 last_error（模型类失败错在模型不在平台，不写、不降权）。
    pub blame: bool,
    /// 持久化到 `ProxyAttempt.effects` 的效果名单（字符串经 `HealthEffect::as_str`）。
    pub effects: Vec<HealthEffect>,
    pub next: Next,
    /// 本次尝试的终态错误是否为连接建立失败（票 04 的 Transport 分支起才有 true）。
    pub connect_failed: bool,
}

/// 纯函数：对一次上游非 2xx 失败做全部裁决。不读状态、不写任何东西、不取时钟。
///
/// 判定序（与抽取前 `handle_non_success` 逐行为对应）：
/// 1. 类别：registry 码义（rate_limit / overloaded）→ Service；否则 `classify_upstream_error`
///    （429/402/401/403 与 censorship_blocked 直接 Service；其余读 body 特征）。
/// 2. 同平台换模型（spec D5）：非 Service 且规则未拦且链上有下一个 → 只留痕不记账。
/// 3. 模型类链尽且非末位 → 换平台，仅 inflight 配平（record_ignored），不碰平台健康。
/// 4. 其余进入主裁决：429 三形态（配额耗尽带/不带重置时间、每分钟限流、其余）、529、
///    registry 码 → 各自冷却通道；401/403 × 健康感知 → 熔断 vs auth 冷却；402 → 余额冷却；
///    降权（censorship 与每分钟限流例外）；censorship → DB 自动禁用；
///    error_rule non-retryable / 400·422 硬错 → 直返。
pub(crate) fn judge(f: &Failure, c: &JudgeCtx) -> Verdict {
    let Failure::Http {
        code,
        body,
        retry_after,
        code_meaning,
    } = f;
    let class = match code_meaning.as_deref() {
        Some("rate_limit" | "overloaded") => UpstreamErrorClass::Service,
        _ => classify_upstream_error(*code, body),
    };
    let rule_blocks = c.rule_non_retryable;
    // ── 同平台换模型（票 06，spec D5）：先于 is_status_retryable（400/422 不换平台）与全部记账。
    //   Service 类（含 censorship_blocked）→ 不换模型；error_rule 标 non-retryable → 用户显式意图，
    //   不换模型也不换平台；其余（Model / Unknown）且链上还有下一个模型 → 只留痕不记账，换模型重发。
    if c.has_next_model && class != UpstreamErrorClass::Service && !rule_blocks {
        return Verdict {
            class,
            scope: ExcludeScope::Pair,
            blame: false,
            effects: Vec::new(),
            next: Next::SwitchModel,
            connect_failed: false,
        };
    }
    // 模型类错误且无同平台备选：错在模型而非平台。还有别的平台 → 换平台（含 400/422），只留痕
    // 不记账（不写 last_error / 降权 / 熔断 / 冷却，仅 inflight-1）；已是末位 → 落到主裁决把
    // 上游错误回客户端，同样不记账。error_rule 标 non-retryable 是用户显式意图，不在此放行。
    if class == UpstreamErrorClass::Model && !c.has_next_model && !rule_blocks && !c.is_last_candidate {
        return Verdict {
            class,
            scope: ExcludeScope::Pair,
            blame: false,
            effects: Vec::new(),
            next: Next::SwitchPlatform,
            connect_failed: false,
        };
    }

    let extracted_msg = extract_error_message(body);

    // ── 429 分类（只看 message 文本，禁按 error.type）：配额耗尽 vs 每分钟限流 vs 其余限流 ──
    //   分类用于记账选路，不再触发 auto_disable：429 统一走 failover 换下个候选。
    let registry_rate_limited = code_meaning.as_deref() == Some("rate_limit");
    let registry_overloaded = code_meaning.as_deref() == Some("overloaded");
    let quota_exhausted = *code == 429
        && !registry_rate_limited
        && classify_429(extracted_msg.as_deref().unwrap_or(body));
    // ── 429 每分钟限流（2026-10-03 用户裁决）：窗口是自然分钟——"rate limit 40/min exceeded"
    //    在 59s 报错，下个整分边界（1s 后）额度就回来了。冷却到边界即可复用，不套固定
    //    熔断/降权窗口。marker 只认显式 "/min" / "per minute" / "每分钟"，不按裸
    //    "rate limit" 猜（小时/日级限流误判成分钟级会提前放行）。registry rate_limit 码
    //    归同一桶（整分边界冷却，不进翻倍通道）。
    let per_minute = registry_rate_limited
        || (*code == 429
            && !quota_exhausted
            && is_per_minute_rate_limit(extracted_msg.as_deref().unwrap_or(body)));
    // ── 票 04（spec 决策 4）：其余 429（含配额耗尽但解析不到重置时间）、529、registry
    //    overloaded 码 → 独立限频冷却通道（1s 起翻倍封顶 10 分钟、成功清零、不计熔断）。
    let quota_reset_until = quota_exhausted
        .then(|| parse_quota_reset_at(retry_after.as_deref(), body, c.now_ms))
        .flatten();
    let rate_limited_now = *code == 529
        || registry_overloaded
        || (*code == 429 && !per_minute && quota_reset_until.is_none());

    // 审核拒绝是平台级不可用：自动禁用一小时，并立即换下个候选。
    // 400 通常是请求硬错，但结构化 censorship_blocked 明确表示上游审核机制拒绝，换平台可能成功。
    let censorship_blocked = is_censorship_blocked(body);

    // 401 / 403 鉴权失败（区域封锁除外）：健康感知分组计入熔断按时间翻倍，其余分组走固定 5 分钟鉴权冷却。
    let auth_failed = matches!(*code, 401 | 403)
        && !is_region_blocked(extracted_msg.as_deref().unwrap_or(body));

    // 模型类失败（到这里 = 链尽且无处可换，或规则拦截）错在模型不在平台：不写 last_error、不降权。
    let no_blame = class == UpstreamErrorClass::Model && !rule_blocks;

    // ── 副作用名单（顺序与抽取前 effects.push 一致，回写前已过换模型早退）──
    let mut effects: Vec<HealthEffect> = Vec::new();
    if auth_failed && c.health_aware {
        effects.push(HealthEffect::BreakerCount);
    }
    // ── 失败降权（health-aware，2026-09-29）：全部非 2xx 计入排序降权（含 400/422），
    //    censorship 例外（已 DB 禁用 1h，不叠加）、每分钟限流例外（降权窗口会越过分边界，
    //    到点该平台只沉不踢，多平台组里排序靠后等于事实上的不可用）。档位：429/5xx → Server，
    //    其余 4xx → Client。──
    if !no_blame && !censorship_blocked && !per_minute {
        let tier = if *code == 429 || *code >= 500 {
            super::scheduling::PenaltyTier::Server
        } else {
            super::scheduling::PenaltyTier::Client
        };
        effects.push(HealthEffect::Penalty(tier));
    }
    if per_minute {
        effects.push(HealthEffect::CooldownMinute {
            until_ms: (c.now_ms / 60_000 + 1) * 60_000,
        });
    }
    if let Some(until) = quota_reset_until {
        effects.push(HealthEffect::CooldownQuota { until_ms: until });
    }
    if rate_limited_now {
        // 票 04 补充口径：正文 / Retry-After 明说的等待时长优先，解析不出才默认翻倍。
        let hint = retry_after
            .and_then(|v| v.trim().parse::<i64>().ok())
            .map(|secs| secs * 1000)
            .or_else(|| parse_rate_limit_retry_ms(body));
        effects.push(HealthEffect::CooldownRateLimit { hint_ms: hint });
    }
    if *code == 402 {
        effects.push(HealthEffect::CooldownBalance);
    } else if auth_failed && !c.health_aware {
        effects.push(HealthEffect::CooldownAuth);
    }
    if censorship_blocked {
        effects.push(HealthEffect::DisableCensorship);
    }

    // ── 中间件 error_rule 分类（出站）：non-retryable → 立即返回不换候选；
    //   retryable / 无命中 → 默认重试语义（is_last_candidate 决定）。
    // ── 决策 A：状态码硬错圈定 ──
    //   400 / 422（请求体本身非法）→ 不重试；结构化 censorship_blocked 与 registry 码义
    //   overloaded 是例外，换候选。其余非 2xx → 默认可重试（换下个候选）。
    let status_retryable = is_status_retryable(*code) || censorship_blocked || registry_overloaded;
    let mw_non_retryable = !censorship_blocked && rule_blocks;
    let non_retryable = !status_retryable || mw_non_retryable;
    let next = if !non_retryable && !c.is_last_candidate {
        Next::SwitchPlatform
    } else {
        Next::Return
    };

    Verdict {
        class,
        scope: if class == UpstreamErrorClass::Service {
            ExcludeScope::Platform
        } else {
            ExcludeScope::Pair
        },
        blame: !no_blame,
        effects,
        next,
        connect_failed: false,
    }
}

/// 唯一记账落地点：把 [`Verdict`] 落到 scheduler / DB。DB 写失败沿用现状
/// 「记 tracing::error 不上抛」。
///
/// 熔断 / inflight 配平由 effects 推导：换模型路径不碰 scheduler（旧早退语义）；
/// 计 BreakerCount → record_failure（健康感知 401/403）；否则 record_ignored
/// （inflight-1：链尽换平台、非 auth 的主路径。旧实现 record_ignored 从不落字符串）。
pub(crate) async fn apply(
    v: &Verdict,
    state: &Arc<ProxyState>,
    route: &RouteResult,
    th: &super::scheduling::BreakerThresholds,
    f: &Failure<'_>,
) {
    let Failure::Http { code, body, .. } = f;
    let pid = route.platform.id;
    let now = aidog_db::now();

    // 熔断计数（2026-09-15 用户裁决：网络类故障不降权）：仅健康感知分组的 401/403 计一次；
    // 429 / 529 / registry 限频码 / 5xx / 402 / 其他 4xx 一律不计（仅 inflight-1）。
    if !matches!(v.next, Next::SwitchModel) {
        if v.effects.iter().any(|e| matches!(e, HealthEffect::BreakerCount)) {
            state.scheduler.record_failure(pid, th, now);
        } else {
            state.scheduler.record_ignored(pid);
        }
    }

    // 记本平台最近一次错误（卡片展示）。本平台失败即覆盖，自身下次成功时清空（commit_2xx）。
    // 换候选成功不清失败平台的 last_error。模型类失败不写（blame=false）。
    if v.blame {
        let detail = extract_error_message(body).unwrap_or_else(|| truncate_attempt_error(body));
        let _ = aidog_db::set_platform_last_error(
            &state.db,
            pid,
            Some(format!("HTTP {code}: {detail}")),
        )
        .await;
    }

    for e in &v.effects {
        match e {
            HealthEffect::BreakerCount => {}
            HealthEffect::Penalty(tier) => {
                state.scheduler.record_penalty(pid, *tier, now);
            }
            // ── 429 每分钟限流 → 冷却到下一个自然分钟边界（复用配额冷却通道）。──
            HealthEffect::CooldownMinute { until_ms } => {
                state.scheduler.set_quota_cooldown(pid, *until_ms);
                tracing::warn!(
                    platform = %route.platform.name, platform_id = pid,
                    cooldown_until_ms = until_ms,
                    "platform per-minute rate limited; cooling down until next minute boundary"
                );
            }
            // ── 429 配额耗尽 + 上游给出明确恢复时间 → 冷却该平台到那个时刻。解析不到就
            //    不冷却（不猜时长）。纯内存维度：配额用完不是平台故障，不写 DB status。──
            HealthEffect::CooldownQuota { until_ms } => {
                state.scheduler.set_quota_cooldown(pid, *until_ms);
                super::estimate::spawn_refresh_at(
                    state.db.clone(),
                    pid,
                    route.platform.base_url.clone(),
                    route.platform.api_key.clone(),
                    *until_ms,
                );
                tracing::warn!(
                    platform = %route.platform.name, platform_id = pid,
                    quota_cooldown_until = until_ms,
                    "platform out of scheduling until upstream quota reset (not disabled)"
                );
            }
            // ── 限频冷却（票 04）：其余 429 / 529 / registry 限频码 → 1s 起翻倍封顶 10 分钟，
            //    该平台任意一次成功清零；不计熔断。纯内存态。──
            HealthEffect::CooldownRateLimit { hint_ms } => {
                state.scheduler.set_rate_limit_cooldown(pid, now, *hint_ms);
                tracing::warn!(
                    platform = %route.platform.name, platform_id = pid,
                    cooldown_until_ms = state.scheduler.rate_limit_cooldown_until_ms(pid),
                    "platform rate limited; rate-limit cooldown with doubling backoff (not counted as breaker)"
                );
            }
            // ── 权限/余额冷却（内存固定 5 分钟，2026-09-15 取代旧 DB auto_disabled 指数退避）。
            //    不写 DB status：平台 UI 仍是启用态，到点自动回调度。──
            HealthEffect::CooldownBalance => {
                state.scheduler.set_balance_cooldown(pid, now);
                tracing::warn!(
                    platform = %route.platform.name, platform_id = pid, status = code,
                    cooldown_ms = super::scheduling::AUTH_COOLDOWN_MS,
                    "platform out of scheduling for balance cooldown (memory-only, not disabled)"
                );
            }
            HealthEffect::CooldownAuth => {
                state.scheduler.set_auth_cooldown(pid, now);
                tracing::warn!(
                    platform = %route.platform.name, platform_id = pid, status = code,
                    cooldown_ms = super::scheduling::AUTH_COOLDOWN_MS,
                    "platform out of scheduling for fixed auth cooldown (memory-only, not disabled)"
                );
            }
            // 审核拒绝：自动禁用一小时，并立即换下个候选。
            HealthEffect::DisableCensorship => {
                if let Err(e) = aidog_db::disable_platform_for_censorship(&state.db, pid).await {
                    tracing::error!(
                        platform_id = pid, error = %e,
                        "failed to auto-disable censorship-blocked platform"
                    );
                }
                tracing::warn!(
                    platform = %route.platform.name, platform_id = pid,
                    "upstream censorship blocked request; platform auto-disabled for one hour"
                );
                aidog_ctx::emit("proxy-log-updated", pid.into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY_PLAIN: &str = r#"{"error":{"message":"boom"}}"#;

    fn ctx(health_aware: bool, has_next: bool, last: bool, rule: bool) -> JudgeCtx {
        JudgeCtx {
            health_aware,
            has_next_model: has_next,
            is_last_candidate: last,
            rule_non_retryable: rule,
            now_ms: 1_000_000,
        }
    }

    fn http(code: u16, body: &'static str) -> Failure<'static> {
        Failure::Http { code, body, retry_after: None, code_meaning: None }
    }

    // ── 表驱动：形态 → (class, scope, blame, effects 字符串, next) ──
    #[test]
    fn judge_table() {
        use Next as N;
        use UpstreamErrorClass as C;
        type Row = (
            &'static str,
            Failure<'static>,
            JudgeCtx,
            C,
            ExcludeScope,
            bool,
            &'static [&'static str],
            N,
        );
        let rows: Vec<Row> = vec![
            // 401 健康感知：熔断 + 降权（Client 档），末位直返
            ("401 HA last", http(401, r#"{"error":"unauthorized"}"#), ctx(true, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["breaker_count", "penalty_client"], N::Return),
            // 401 非健康感知：auth 冷却
            ("401 non-HA", http(401, r#"{"error":"unauthorized"}"#), ctx(false, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["penalty_client", "cooldown_auth"], N::Return),
            // 402：余额冷却（两种模式同形）
            ("402", http(402, r#"{"error":"balance"}"#), ctx(true, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["penalty_client", "cooldown_balance"], N::Return),
            // 429 配额耗尽 + 无重置时间：限频冷却通道
            ("429 quota no-reset", http(429, r#"{"error":{"message":"quota exhausted"}}"#), ctx(true, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["penalty_server", "cooldown_rate_limit"], N::Return),
            // 429 每分钟限流：只冷却到整分边界，不降权
            ("429 per-minute", http(429, r#"{"error":{"message":"rate limit 40/min exceeded"}}"#), ctx(true, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["cooldown_minute"], N::Return),
            // 529：限频冷却
            ("529", http(529, r#"{"error":{"message":"overloaded"}}"#), ctx(true, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["penalty_server", "cooldown_rate_limit"], N::Return),
            // 5xx：服务类，只降权
            ("500", http(500, BODY_PLAIN), ctx(true, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["penalty_server"], N::Return),
            // 区域封锁 401：不熔断不冷却，只降权
            ("region 401", http(401, r#"{"error":{"message":"not available in your country"}}"#), ctx(true, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["penalty_client"], N::Return),
            // censorship：禁用 + 不降权，状态可重试
            ("censorship", http(400, r#"{"error":{"message":"x","type":"censorship_blocked"}}"#), ctx(true, false, true, false),
             C::Service, ExcludeScope::Platform, true, &["disabled_censorship"], N::Return),
            // 模型类 × 有下一个模型：换模型，零记账零效果
            ("model switch", http(404, r#"{"type":"error","error":{"type":"not_found_error","message":"model: m-a"}}"#), ctx(true, true, false, false),
             C::Model, ExcludeScope::Pair, false, &[], N::SwitchModel),
            // 模型类链尽非末位：换平台不背锅
            ("model chain switch platform", http(404, r#"{"type":"error","error":{"type":"not_found_error","message":"model: m-a"}}"#), ctx(true, false, false, false),
             C::Model, ExcludeScope::Pair, false, &[], N::SwitchPlatform),
            // 模型类链尽末位：直返，不背锅无效果
            ("model chain last", http(404, r#"{"type":"error","error":{"type":"not_found_error","message":"model: m-a"}}"#), ctx(true, false, true, false),
             C::Model, ExcludeScope::Pair, false, &[], N::Return),
            // 400 硬错：直返 + Client 降权
            ("400 hard", http(400, BODY_PLAIN), ctx(true, false, true, false),
             C::Unknown, ExcludeScope::Pair, true, &["penalty_client"], N::Return),
        ];
        for (name, f, c, class, scope, blame, effects, next) in rows {
            let v = judge(&f, &c);
            assert_eq!(v.class, class, "{name}: class");
            assert_eq!(v.scope, scope, "{name}: scope");
            assert_eq!(v.blame, blame, "{name}: blame");
            let got: Vec<&str> = v.effects.iter().map(|e| e.as_str()).collect();
            assert_eq!(got.as_slice(), effects, "{name}: effects");
            assert!(
                matches!(
                    (&v.next, next),
                    (N::SwitchModel, N::SwitchModel)
                        | (N::SwitchPlatform, N::SwitchPlatform)
                        | (N::Return, N::Return)
                ),
                "{name}: next"
            );
            assert!(!v.connect_failed, "{name}: http 失败 connect_failed 恒 false");
        }
    }

    // 不变量（spec M1）：模型类且规则未拦 → blame=false 且 effects 不含降权/熔断/冷却；
    // Service ⇒ scope=Platform，其余 ⇒ Pair。
    #[test]
    fn invariants() {
        const BODIES: [&str; 2] = [
            r#"{"type":"error","error":{"type":"not_found_error","message":"model: m-a"}}"#,
            r#"{"error":{"message":"无可用渠道"}}"#,
        ];
        for body in BODIES {
            for ha in [true, false] {
                for last in [true, false] {
                    let v = judge(&http(404, body), &ctx(ha, false, last, false));
                    assert_eq!(v.class, UpstreamErrorClass::Model, "404 模型特征");
                    assert!(!v.blame, "模型类不背锅");
                    assert!(
                        v.effects.is_empty(),
                        "模型类（规则未拦）零记账效果: {:?}",
                        v.effects.iter().map(|e| e.as_str()).collect::<Vec<_>>()
                    );
                }
            }
        }
        // Service ⇒ Platform
        for code in [401u16, 402, 403, 429, 500, 529] {
            let v = judge(&http(code, r#"{"error":"x"}"#), &ctx(false, false, true, false));
            if v.class == UpstreamErrorClass::Service {
                assert_eq!(v.scope, ExcludeScope::Platform, "{code}");
            } else {
                assert_eq!(v.scope, ExcludeScope::Pair, "{code}");
            }
        }
    }

    // R1：HealthEffect::as_str() 穷举 == 旧 effects 字符串集合（历史日志 / RoutingCanvas EFFECT_KEY）。
    #[test]
    fn health_effect_as_str_covers_legacy_set() {
        let all = [
            HealthEffect::BreakerCount.as_str(),
            HealthEffect::Penalty(super::scheduling::PenaltyTier::Server).as_str(),
            HealthEffect::Penalty(super::scheduling::PenaltyTier::Client).as_str(),
            HealthEffect::CooldownMinute { until_ms: 0 }.as_str(),
            HealthEffect::CooldownQuota { until_ms: 0 }.as_str(),
            HealthEffect::CooldownRateLimit { hint_ms: None }.as_str(),
            HealthEffect::CooldownBalance.as_str(),
            HealthEffect::CooldownAuth.as_str(),
            HealthEffect::DisableCensorship.as_str(),
        ];
        assert_eq!(
            all,
            [
                "breaker_count",
                "penalty_server",
                "penalty_client",
                "cooldown_minute",
                "cooldown_quota",
                "cooldown_rate_limit",
                "cooldown_balance",
                "cooldown_auth",
                "disabled_censorship",
            ]
        );
        let mut uniq = all.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), all.len(), "as_str 两两不同");
    }
}
