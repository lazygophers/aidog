//! 逐次重算的尝试编排（请求尝试-逐次重算 票 02）：每次上游调用前，按当前调度状态对
//! 「尚未排除的（平台，模型）组合」重排取第一；失败按错误类排除——模型类 / 未知类只排除该组合（pair），
//! 服务类排除整个平台。失败组合在本次请求内不再选。
//!
//! 聊天、向量化 / 重排序、决策、搜索四类请求的循环共用 [`AttemptLoop`]（票 02 聊天、票 03 其余三类）。

use super::*;
use aidog_db::models::GroupPlatformDetail;
use std::time::Duration;
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
    let mut route = live.into_iter().next()?;
    let model_alt = !route.fallback_models.is_empty();
    route.fallback_models.clear();
    Some(Pick { route, model_alt, candidate_total })
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
        kind: RequestKind<'_>,
        client: ClientGate,
    ) -> Result<Vec<RouteResult>, String> {
        if self.platforms.is_none() {
            self.platforms = Some(aidog_db::get_group_platforms(db, group.id).await.map_err(|e| e.to_string())?);
        }
        let platforms = self.platforms.clone().unwrap_or_default();
        select_candidates_preloaded(db, group, Some(platforms), source_model, Some(ctx), kind, client)
            .await
            .map(|s| {
                self.saw_rate_limit = s.routing.entries.iter().any(|e| e.reason == aidog_db::models::RouteReason::RateLimited);
                s.candidates
            })
            .map_err(|e| e.to_string())
    }
}

/// 一次请求的尝试循环状态（四类请求共用）：每次调用前 [`next`](Self::next) 按当前调度状态重排取第一，
/// 失败后 [`failed`](Self::failed) 按错误类折进排除集。上游调用计数归 `budget`。
pub(crate) struct AttemptLoop<'k> {
    pub(crate) budget: CallBudget,
    kind: RequestKind<'k>,
    source_model: String,
    client: ClientGate,
    excl: Exclusions,
    planner: AttemptPlanner,
    first: Option<Vec<RouteResult>>,
    /// 本次请求内已 connect 失败的 base_url（R3：站级事实，同站兄弟条目直接跳过）。
    dead_base_urls: HashSet<String>,
    /// 当前这次选择：(平台 id, 模型, base_url)。
    cur: Option<(u64, String, String)>,
    /// 当前尝试轮（从 1 起）。
    round: u32,
    /// 开新一轮前的等待。
    pub(crate) wait: Duration,
}

/// 候选全排除后开新一轮前的等待（测试默认 0，免真实 sleep）。
pub(crate) const ROUND_WAIT: Duration = Duration::from_secs(1);

impl<'k> AttemptLoop<'k> {
    /// `first` = 请求入口已算好的首次排序；之后每次尝试重排。
    pub(crate) fn new(max_attempts: u32, kind: RequestKind<'k>, source_model: &str, client: ClientGate, first: Vec<RouteResult>) -> Self {
        Self {
            budget: CallBudget::new(max_attempts),
            kind,
            source_model: source_model.to_string(),
            client,
            excl: Exclusions::default(),
            planner: AttemptPlanner::default(),
            first: Some(first),
            dead_base_urls: HashSet::new(),
            cur: None,
            round: 1,
            wait: if cfg!(test) { Duration::ZERO } else { ROUND_WAIT },
        }
    }

    /// 下一次尝试的选择；None = 次数用尽 / 配置原因无组合可选 / 轮数用尽（调用方落终态 503）。
    /// 无可选组合但原因不是配置（失败排除、熔断 / 冷却 / 限频暂时挡掉）时，开新一轮：等 [`wait`](Self::wait)、
    /// 清空失败排除与已死 base_url 集，从全部候选重来（`attempts_len` = 目前已记的尝试条数，记进轮起点）。
    pub(crate) async fn next(
        &mut self,
        db: &Db,
        group: &Group,
        ctx: &ScheduleCtx<'_>,
        log: &mut ProxyLog,
        attempts_len: usize,
    ) -> Option<Pick> {
        let mut rate_limited = false;
        loop {
            if self.budget.exhausted() {
                return None;
            }
            let ranked = match self.first.take() {
                Some(c) => c,
                None => match self.planner.rerank(db, group, &self.source_model, ctx, self.kind, self.client.clone()).await {
                    Ok(c) => c,
                    Err(e) => {
                        rate_limited |= e == "rate_limit";
                        if is_config_empty(&e) || !self.new_round(log, attempts_len).await {
                            // 重排时剩余候选全被限频挡掉：终态 503 沿用限频审计标记
                            if rate_limited {
                                mark_rate_limited(log);
                            }
                            return None;
                        }
                        continue;
                    }
                },
            };
            if let Some(pick) = pick_first(self.excl.apply(ranked), &self.dead_base_urls, log) {
                self.cur = Some((pick.route.platform.id, pick.route.target_model.clone(), pick.route.platform.base_url.clone()));
                return Some(pick);
            }
            // 重排时有平台被限频挡掉、其余都已排除：终态 503 沿用限频审计标记
            rate_limited |= self.planner.saw_rate_limit;
            if !self.new_round(log, attempts_len).await {
                if rate_limited {
                    mark_rate_limited(log);
                }
                return None;
            }
        }
    }

    /// 开新一轮：总轮数 ≤ 最大尝试次数（空轮也计，防空转）。
    async fn new_round(&mut self, log: &mut ProxyLog, attempts_len: usize) -> bool {
        if self.round as usize >= self.budget.max() {
            return false;
        }
        if !self.wait.is_zero() {
            tokio::time::sleep(self.wait).await;
        }
        self.excl = Exclusions::default();
        self.dead_base_urls.clear();
        self.round += 1;
        if let Some(r) = log.routing.as_mut() {
            r.round_starts.get_or_insert_with(Default::default).push(attempts_len as u32);
        }
        true
    }

    /// 本次选择失败（`Next`）：折进排除集；`connect_failed` 把 base_url 记为已死。
    pub(crate) fn failed(&mut self, attempts: &mut [ProxyAttempt], connect_failed: bool) {
        if let Some((platform_id, model, base_url)) = self.cur.take() {
            if connect_failed {
                self.dead_base_urls.insert(base_url);
            }
            record_exclusion(&mut self.excl, attempts, platform_id, &model);
        }
    }

    /// 记一次上游调用（限频占位等不发请求的跳过不计）；返回 true = 配额内最后一次。
    pub(crate) fn begin_call(&mut self, log: &mut ProxyLog, attempts: &[ProxyAttempt], platform_id: u64) -> bool {
        let tried: Vec<u64> = attempts.iter().map(|a| a.platform_id).chain([platform_id]).collect();
        self.budget.begin_call_untried(log, &tried)
    }
}

/// 无可选组合的原因是否由配置造成（槽位关闭 / 平台禁用 / 客户端白名单 / 高峰禁用 / 未知协议 / 无该类请求的平台）：
/// 等多久都不会变，立即终态；其余（熔断 / 冷却 / 限频 / 额度 / 失败排除）都是暂时的，值得开新一轮。
/// 取值是 `router/candidates.rs` 阶段 5 的 `RouteFail.reason` 字面量。
pub(crate) fn is_config_empty(reason: &str) -> bool {
    matches!(
        reason,
        "unknown_protocol"
            | "peak_disabled"
            | "slot_disabled"
            | "client_not_allowed"
            | "client_not_allowed_mixed"
            | "no_chat_platform"
            | "no_decision_platform"
            | "no_embedding_platform"
            | "no_rerank_platform"
            | "no_tools_platform"
            | "group has no platforms"
            | "group's only platform is manually disabled"
            | "group's only platform is censorship-blocked"
    )
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

    #[test]
    fn config_caused_emptiness_is_terminal_others_are_not() {
        for r in [
            "unknown_protocol", "peak_disabled", "slot_disabled", "client_not_allowed", "client_not_allowed_mixed",
            "no_chat_platform", "no_decision_platform", "no_embedding_platform", "no_rerank_platform", "no_tools_platform",
            "group has no platforms", "group's only platform is manually disabled", "group's only platform is censorship-blocked",
        ] {
            assert!(is_config_empty(r), "{r} 应为配置原因（立即终态）");
        }
        for r in [
            "rate_limit",
            "quota_exhausted;cp=0",
            "no available platform (all disabled, backing off, or circuit-broken)",
        ] {
            assert!(!is_config_empty(r), "{r} 非配置原因（开新一轮）");
        }
    }

    fn lp(max: u32) -> AttemptLoop<'static> {
        AttemptLoop::new(max, RequestKind::Chat, "m", ClientGate::Off, vec![])
    }

    #[tokio::test]
    async fn rounds_are_bounded_by_max_attempts_and_record_starts() {
        let mut l = lp(3);
        let mut log = ProxyLog { routing: Some(Default::default()), ..Default::default() };
        assert!(l.new_round(&mut log, 2).await);
        assert!(l.new_round(&mut log, 2).await, "空轮（attempts 没增长）同样计轮");
        assert!(!l.new_round(&mut log, 2).await, "总轮数 ≤ 最大尝试次数 3");
        assert_eq!(log.routing.unwrap().round_starts, Some(Box::new(vec![2, 2])));
    }

    #[tokio::test]
    async fn new_round_clears_failure_exclusions_and_dead_base_urls() {
        let mut l = lp(5);
        let mut log = ProxyLog { routing: Some(Default::default()), ..Default::default() };
        l.excl.add(ExcludeScope::Platform, 1, "a");
        l.dead_base_urls.insert("http://x".into());
        assert!(l.new_round(&mut log, 1).await);
        assert_eq!(l.excl.apply(vec![rr(1, &["a"])]).len(), 1);
        assert!(l.dead_base_urls.is_empty());
    }

    #[test]
    fn round_wait_is_injectable_and_defaults_to_one_second() {
        assert_eq!(ROUND_WAIT, std::time::Duration::from_secs(1));
        let mut l = lp(2);
        assert_eq!(l.wait, Duration::ZERO, "测试默认不真等");
        l.wait = Duration::from_millis(7);
        assert_eq!(l.wait, Duration::from_millis(7));
    }
}
