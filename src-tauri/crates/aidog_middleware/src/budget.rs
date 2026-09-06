//! 成本预算闸门（票 06）：本自然月花费超过预算即拒绝请求。
//!
//! 为什么单独一趟、不并进 [`crate::MiddlewareEngine::apply_inbound`] 的动作链：
//! 判定要查库（异步），而入站动作链是同步的（在 proxy 热路径上原地改写 chat_req）。
//! 故 `ActionKind::BudgetGate` 在同步动作链里被忽略，只由本模块处理。
//!
//! 窗口固定自然月（用户拍板，不做滚动窗口 / QPS 限流）；超限行为固定直接拒绝
//! （不做降级到更便宜的候选——悄悄换模型会让「这次回答为什么变差」无法排查）。
//!
//! **无预算规则 = 零 DB 查询**：先按 applies_to + 条件树筛出带 budget_gate 的规则，
//! 一条都没有就直接返回，热路径不多一次 IO。
//!
//! ## `applies_to.models` 的口径（两侧必须一致，票 06 评审 F5）
//!
//! 规则选中侧（这里）与花费聚合侧（[`aidog_stats::window_spend`]）比对的是**同一个名字**：
//! **上游实际模型名**，也就是平台侧模型重映射之后、真正发给上游的那个名字
//! （聚合表 `stats_agg_hourly.model` 存的就是它）。
//! 调用方据此传 `model` 参数：
//! - platform 挂载点（`forward.rs`）已经路由完，传 `route.target_model`（= actual_model）；
//! - group 挂载点（`handler.rs`）路由还没发生，平台未知、重映射结果也未知，只能传客户端
//!   请求名。**未配模型重映射时两者相同**；配了重映射又要按模型限额，请把规则的作用范围
//!   同时限定到平台（那样规则归属到 platform 挂载点，拿得到真名）。

use aidog_adapter::ChatRequest;
use aidog_db::Db;
use aidog_db::models::{ActionKind, MiddlewareSettings};

use super::{EvalView, InboundOutcome, MiddlewareEngine, Mount, inbound::collect_request_text};

/// 单条预算闸门规则的当前窗口状态（前端展示已用 / 剩余用；也是拒绝判定的输入）。
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetState {
    pub rule_id: i64,
    pub rule_name: String,
    pub budget_usd: f64,
    pub spent_usd: f64,
}

impl BudgetState {
    /// 剩余额度（美元），已超限则为负。
    pub fn remaining_usd(&self) -> f64 {
        self.budget_usd - self.spent_usd
    }

    /// 是否已超限。等于预算即超限（「刚好花完」= 拦）。
    pub fn exceeded(&self) -> bool {
        self.spent_usd >= self.budget_usd
    }
}

/// 拦截原因值：落 `proxy_log.blocked_reason`，与普通 block（规则 description）、
/// 路由高峰禁用（`peak`）、观察模式（票 04）区分开。
pub const BUDGET_BLOCKED_REASON: &str = "budget_exceeded";

impl MiddlewareEngine {
    /// 取当前生效的预算闸门规则（applies_to 已过滤，budget_usd > 0）。
    /// 返回 `(rule_id, rule_name, budget_usd, applies_to)`，供查库与状态展示复用。
    fn budget_rules(
        &self,
        mount: Mount,
        group_key: Option<&str>,
        platform_id: Option<i64>,
        model: &str,
        view: Option<&EvalView>,
    ) -> Vec<(i64, String, f64, aidog_db::models::AppliesTo)> {
        self.request_rules(mount, group_key, platform_id, model)
            .into_iter()
            .filter(|cr| view.is_none_or(|v| cr.conditions.eval(v)))
            .flat_map(|cr| {
                cr.rule
                    .actions
                    .iter()
                    .filter(|s| s.kind == ActionKind::BudgetGate && s.params.budget_usd > 0.0)
                    .map(|s| {
                        (
                            cr.rule.id,
                            cr.rule.name.clone(),
                            s.params.budget_usd,
                            cr.rule.applies_to.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// 预算闸门判定（入站挂载点专用，group 层与 platform 层各调一次）。
    ///
    /// 命中条件树且带 budget_gate 动作的规则，逐条查本自然月已花费；
    /// 任一条 `已花费 >= 预算` → `Blocked`（调用方落审计日志 + 403，est_cost 记 0）。
    /// 查库失败 fail-open（放行 + warn）：预算查不出来不该把用户的请求全打死。
    ///
    /// 挂载点归属由 `platform_id` 派生（本函数只在两个入站挂载点被调用：group 层传 None、
    /// platform 层传 Some），保证同一条规则只查一次库。见 [`Mount`]。
    /// `model` 的口径见本模块头部注释。
    #[allow(clippy::too_many_arguments)]
    pub async fn check_budget(
        &self,
        settings: &MiddlewareSettings,
        db: &Db,
        chat_req: &ChatRequest,
        model: &str,
        group_key: Option<&str>,
        platform_id: Option<i64>,
        req_headers: Option<&str>,
    ) -> InboundOutcome {
        if !settings.enabled {
            return InboundOutcome::Continue;
        }
        let mount = if platform_id.is_some() {
            Mount::Platform
        } else {
            Mount::Group
        };
        // 先只按 applies_to 粗筛（不聚合请求文本）：常态是一条预算规则都没有，
        // 此时热路径连 `collect_request_text` 的整段拷贝都省掉，更不查库。
        if self
            .budget_rules(mount, group_key, platform_id, model, None)
            .is_empty()
        {
            return InboundOutcome::Continue;
        }
        let view = EvalView {
            req_text: collect_request_text(chat_req),
            req_headers,
            model,
            ..Default::default()
        };
        let gates = self.budget_rules(mount, group_key, platform_id, model, Some(&view));
        for (rule_id, rule_name, budget_usd, applies_to) in gates {
            let spent = match aidog_stats::month_spend(db, &applies_to).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(rule_id, error = %e, "middleware budget: spend query failed, fail-open");
                    continue;
                }
            };
            let state = BudgetState {
                rule_id,
                rule_name: rule_name.clone(),
                budget_usd,
                spent_usd: spent,
            };
            if state.exceeded() {
                tracing::warn!(
                    rule_id, rule_name = %rule_name, spent, budget_usd,
                    "middleware budget: monthly budget exceeded, request blocked"
                );
                return InboundOutcome::Blocked {
                    blocked_by: format!("rule#{rule_id} {rule_name}"),
                    blocked_reason: format!(
                        "{BUDGET_BLOCKED_REASON}: spent ${spent:.4} of ${budget_usd:.2} monthly budget"
                    ),
                };
            }
        }
        InboundOutcome::Continue
    }

    /// 全部预算闸门规则的当前窗口状态（前端展示用；不按单次请求过滤条件树 / 作用范围）。
    pub async fn budget_states(&self, db: &Db) -> Vec<BudgetState> {
        let mut out = Vec::new();
        for cr in self.snapshot() {
            for step in &cr.rule.actions {
                if step.kind != ActionKind::BudgetGate || step.params.budget_usd <= 0.0 {
                    continue;
                }
                let spent = aidog_stats::month_spend(db, &cr.rule.applies_to)
                    .await
                    .unwrap_or(0.0);
                out.push(BudgetState {
                    rule_id: cr.rule.id,
                    rule_name: cr.rule.name.clone(),
                    budget_usd: step.params.budget_usd,
                    spent_usd: spent,
                });
            }
        }
        out
    }
}
