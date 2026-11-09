//! 逐次重算的尝试编排（请求尝试-逐次重算 票 02）：每次上游调用前，按当前调度状态对
//! 「尚未排除的（平台，模型）组合」重排取第一；失败按错误类排除——模型类 / 未知类只排除该组合（pair），
//! 服务类排除整个平台。失败组合在本次请求内不再选。
//!
//! 聊天循环（handler.rs）使用；向量化 / 重排序 / 决策 / tools 循环（票 03）复用同一套
//! [`Exclusions`] + [`AttemptPlanner`]。

use super::*;
use aidog_db::models::GroupPlatformDetail;
use std::collections::HashSet;

/// 排除范围，字符串形态即 `ProxyAttempt.exclusion` 的取值。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExcludeScope {
    Pair,
    Platform,
}

impl ExcludeScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pair => "pair",
            Self::Platform => "platform",
        }
    }
    fn parse(s: &str) -> Self {
        if s == "pair" { Self::Pair } else { Self::Platform }
    }
}

/// 本次请求内已排除的平台 / 组合。
#[derive(Default)]
pub(crate) struct Exclusions {
    platforms: HashSet<u64>,
    pairs: HashSet<(u64, String)>,
}

impl Exclusions {
    pub(crate) fn add(&mut self, scope: ExcludeScope, platform_id: u64, model: &str) {
        match scope {
            ExcludeScope::Platform => {
                self.platforms.insert(platform_id);
            }
            ExcludeScope::Pair => {
                self.pairs.insert((platform_id, model.to_string()));
            }
        }
    }

    /// 剔除已排除的平台，并把每个候选的模型链里已排除的模型去掉（链首随之顺延）；链空则整个候选丢弃。
    pub(crate) fn apply(&self, cands: Vec<RouteResult>) -> Vec<RouteResult> {
        cands
            .into_iter()
            .filter_map(|mut c| {
                let pid = c.platform.id;
                if self.platforms.contains(&pid) {
                    return None;
                }
                if self.pairs.is_empty() {
                    return Some(c);
                }
                let mut chain: Vec<String> = std::iter::once(std::mem::take(&mut c.target_model))
                    .chain(std::mem::take(&mut c.fallback_models))
                    .filter(|m| !self.pairs.contains(&(pid, m.clone())))
                    .collect();
                if chain.is_empty() {
                    return None;
                }
                c.target_model = chain.remove(0);
                c.fallback_models = chain;
                Some(c)
            })
            .collect()
    }
}

/// 下一次调用的选择：单个（平台，模型）组合 + 之后是否还有备选。
pub(crate) struct Pick {
    /// `fallback_models` 已清空：一次调用只发一个模型。
    pub route: RouteResult,
    /// 排除本平台后是否还有别的平台可选。
    pub platform_alt: bool,
    /// 本平台当前排序下是否还有下一个模型。
    pub model_alt: bool,
    /// 当前排序里的候选数（日志用）。
    pub candidate_total: usize,
}

/// 从（已应用排除的）排序结果里取第一个 base_url 未判死的候选；被判死的补记进快照。
pub(crate) fn pick_first(
    cands: Vec<RouteResult>,
    dead_base_urls: &HashSet<String>,
    log: &mut ProxyLog,
) -> Option<Pick> {
    let mut live = Vec::with_capacity(cands.len());
    for c in cands {
        if dead_base_urls.contains(&c.platform.base_url) {
            tracing::warn!(
                platform = %c.platform.name, base_url = %c.platform.base_url,
                "same-base_url sibling already failed to connect this request, skipping (R3)"
            );
            if let Some(r) = log.routing.as_mut() {
                r.mark_dead_upstream(c.platform.id);
            }
        } else {
            live.push(c);
        }
    }
    let candidate_total = live.len();
    let mut it = live.into_iter();
    let mut route = it.next()?;
    let platform_alt = it.any(|c| c.platform.id != route.platform.id);
    let model_alt = !route.fallback_models.is_empty();
    route.fallback_models.clear();
    Some(Pick { route, platform_alt, model_alt, candidate_total })
}

/// 把刚失败的一次调用折进排除集：范围取自最后一条尝试记录的 `exclusion`，缺省按平台（传输错误 / 超时 /
/// 连接失败 / 限频占位失败等服务类）；缺省时把范围回写进记录，日志详情据此显示。
pub(crate) fn record_exclusion(
    excl: &mut Exclusions,
    attempts: &mut [ProxyAttempt],
    platform_id: u64,
    model: &str,
) {
    let scope = match attempts.last_mut() {
        Some(a) => ExcludeScope::parse(a.exclusion.get_or_insert_with(|| ExcludeScope::Platform.as_str().to_string())),
        None => ExcludeScope::Platform,
    };
    excl.add(scope, platform_id, model);
}

/// 重排器：分组平台列表首次重排时读一次库，之后各次重排复用（一次请求只读一次）。
#[derive(Default)]
pub(crate) struct AttemptPlanner {
    platforms: Option<Vec<GroupPlatformDetail>>,
    /// 最近一次重排里是否有平台因限频被路由层挡掉（终态 503 据此沿用限频审计标记）。
    pub(crate) saw_rate_limit: bool,
}

impl AttemptPlanner {
    /// 以当前调度状态（`ctx`）重新排序；Err = 没有可选组合（路由失败原因 / 读库失败）。
    pub(crate) async fn rerank(
        &mut self,
        db: &Db,
        group: &Group,
        source_model: &str,
        ctx: &ScheduleCtx<'_>,
        client: ClientGate,
    ) -> Result<Vec<RouteResult>, String> {
        if self.platforms.is_none() {
            self.platforms = Some(aidog_db::get_group_platforms(db, group.id).await.map_err(|e| e.to_string())?);
        }
        let platforms = self.platforms.clone().unwrap_or_default();
        select_candidates_preloaded(db, group, Some(platforms), source_model, Some(ctx), RequestKind::Chat, client)
            .await
            .map(|s| {
                self.saw_rate_limit = s.routing.entries.iter().any(|e| e.reason == aidog_db::models::RouteReason::RateLimited);
                s.candidates
            })
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rr(pid: u64, models: &[&str]) -> RouteResult {
        let platform = crate::gateway::router::test_mod::mk_platform_id(pid);
        RouteResult {
            platform,
            target_model: models[0].to_string(),
            fallback_models: models[1..].iter().map(|s| s.to_string()).collect(),
            mapping: None,
        }
    }

    #[test]
    fn platform_scope_drops_all_models_of_platform() {
        let mut e = Exclusions::default();
        e.add(ExcludeScope::Platform, 1, "a");
        let out = e.apply(vec![rr(1, &["a", "b"]), rr(2, &["c"])]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].platform.id, 2);
    }

    #[test]
    fn pair_scope_promotes_next_model_and_drops_empty_chain() {
        let mut e = Exclusions::default();
        e.add(ExcludeScope::Pair, 1, "a");
        e.add(ExcludeScope::Pair, 2, "c");
        let out = e.apply(vec![rr(1, &["a", "b"]), rr(2, &["c"])]);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].platform.id, out[0].target_model.as_str()), (1, "b"));
        assert!(out[0].fallback_models.is_empty());
    }
}
