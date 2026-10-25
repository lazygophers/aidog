//! 规则引擎（票 02 建于此，票 04 迁至 aidog_db::models::notification_rules——
//! 规则随 NotificationSettings 持久化 + TS 导出同处）。本模块保路径兼容 re-export。
pub use aidog_db::models::{
    Channel, CmpOp, DedupGate, Expr, ExprError, MatchInput, Rule, RuleAction, compile_expr,
    dedup_gate, eval_expr, evaluate, preset_rules, rule_matches,
};
