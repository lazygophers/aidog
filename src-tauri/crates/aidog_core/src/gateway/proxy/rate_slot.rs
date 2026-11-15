use super::*;
use crate::gateway::rate_limit::{Access, RateLimitsCfg};

/// 一次候选的限频占位参数。
pub(crate) struct RateSlotCtx<'a> {
    pub(crate) route: &'a RouteResult,
    pub(crate) requested_model: &'a str,
    pub(crate) req_value: &'a Value,
    pub(crate) model_alt: bool,
    pub(crate) label: &'a str,
}

/// 限频占位跨调用保存：同平台复用平台级名额，换平台时释放旧名额。
#[derive(Default)]
pub(crate) struct RateSlot(Option<(u64, Option<Arc<Access>>)>);

impl RateSlot {
    /// 占平台与模型名额。`Err(())` 表示未发上游请求，调用方只需排除当前候选。
    pub(crate) fn enter(
        &mut self,
        state: &ProxyState,
        log: &mut ProxyLog,
        attempts: &mut Vec<ProxyAttempt>,
        ctx: RateSlotCtx<'_>,
    ) -> Result<Option<Arc<Access>>, ()> {
        let RateSlotCtx { route, requested_model, req_value, model_alt, label } = ctx;
        let attempt_ts = aidog_db::now();
        if log.blocked_reason == "rate_limit" {
            log.blocked_by.clear();
            log.blocked_reason.clear();
        }
        let cfg = RateLimitsCfg::parse(&route.platform.extra);
        let estimate = if cfg.has_tpm() {
            estimate_input_tokens(req_value, requested_model)
        } else {
            0
        };
        if self.0.as_ref().map(|(pid, _)| *pid) != Some(route.platform.id) {
            self.0 = None;
            let access = if cfg.is_empty() {
                None
            } else {
                let now = aidog_db::now();
                match state.scheduler.rate.begin(
                    route.platform.id,
                    &cfg.platform_limits(now),
                    now,
                    estimate,
                ) {
                    Some(access) => Some(access),
                    None => {
                        tracing::warn!(platform = %route.platform.name, kind = label, "rate limit reached at acquire, skipping candidate");
                        attempts.push(ProxyAttempt {
                            kind: Some("rate_limit_platform".to_string()),
                            chain_stop: model_alt.then(|| "rate_limit".to_string()),
                            platform_id: route.platform.id,
                            platform_name: route.platform.name.clone(),
                            error: "platform rate limit reached".to_string(),
                            ts: attempt_ts,
                            model: Some(route.target_model.clone()),
                            exclusion: Some(ExcludeScope::Platform),
                            ..Default::default()
                        });
                        mark_rate_limited(log);
                        return Err(());
                    }
                }
            };
            self.0 = Some((route.platform.id, access));
        }
        let access = self.0.as_ref().and_then(|(_, access)| access.clone());
        if let Some(access) = &access {
            let model = route.target_model.clone();
            let now = aidog_db::now();
            if !access.enter_model(&model, &cfg.model_limits(&model, now), now, estimate) {
                tracing::warn!(model = %model, kind = label, "model-level rate limit reached, excluding pair");
                attempts.push(ProxyAttempt {
                    kind: Some("rate_limit_model".to_string()),
                    platform_id: route.platform.id,
                    platform_name: route.platform.name.clone(),
                    error: "model rate limit reached".to_string(),
                    ts: attempt_ts,
                    model: Some(model),
                    exclusion: Some(ExcludeScope::Pair),
                    ..Default::default()
                });
                mark_rate_limited(log);
                return Err(());
            }
        }
        Ok(access)
    }
}

/// 限频占位失败：末候选走「全部耗尽」503 时据此落 blocked_reason。
pub(crate) fn mark_rate_limited(log: &mut ProxyLog) {
    log.blocked_by = "router".to_string();
    log.blocked_reason = "rate_limit".to_string();
}
