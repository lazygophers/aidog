//! 语义事件模型（通知体系重设计票 01）：Envelope 统一事件形状 + proxy 侧旁路发射入口。
//! 规则引擎（规则/条件表达式/渠道集合/去重窗口）在票 02；本模块只做形状 + 发射。

use std::collections::HashMap;
use std::sync::Arc;

use aidog_ctx::AppCtx;
use aidog_db::Db;

/// 语义类型目录（3 CC 语义 + 3 proxy 语义）。
/// **跨层镜像**：`src/domains/notifications/semantic.ts` 的 `SEMANTIC_TYPES` 逐字镜像。
pub const SEMANTIC_TYPES: &[&str] = &[
    "task_complete",
    "waiting_input",
    "error",
    "breaker_on",
    "breaker_off",
    "quota_exhaust_forecast",
];

/// 事件来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// CC hooks（/api/notify event 路径）。
    Hook,
    /// 代理内核（熔断/配额预测发射点）。
    Proxy,
    /// 外部脚本（/api/notify type 路径）。
    Api,
}

/// 事件级别（票 02 规则引擎的匹配维度之一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// 语义类型 → 默认级别（与 spec「语义类型目录」一致）。
pub fn severity_for_type(typ: &str) -> Severity {
    match typ {
        "error" | "breaker_on" => Severity::Error,
        "waiting_input" | "quota_exhaust_forecast" => Severity::Warn,
        _ => Severity::Info,
    }
}

/// 语义事件信封：体系内一切通知的统一形状（票 02 规则引擎的输入）。
#[derive(Debug, Clone)]
pub struct Envelope {
    /// 语义类型字面量（`SEMANTIC_TYPES` 之一；或透传 raw 事件名）。
    pub typ: String,
    pub source: Source,
    pub severity: Severity,
    /// 模板变量（`request_id` 由 dispatch 注入）。
    pub vars: HashMap<String, String>,
    /// 去重键（票 02 去重窗口用；票 01 由发射点自带频控）。
    pub dedup_key: String,
    /// 事件时刻（unix ms）。
    pub ts: i64,
}

impl Envelope {
    pub fn new(
        typ: &str,
        source: Source,
        vars: HashMap<String, String>,
        dedup_key: String,
        ts: i64,
    ) -> Self {
        Self {
            typ: typ.to_string(),
            source,
            severity: severity_for_type(typ),
            vars,
            dedup_key,
            ts,
        }
    }
}

/// 旁路发射：构造 Envelope 后走既有 dispatch 的 event 路径（per_event / 内置默认控制通道）。
/// 只 emit 不改变调用方行为（票 01 验收）；fire-and-forget 由调用方 tokio::spawn 决定。
pub async fn emit(
    db: &Arc<Db>,
    ctx: Option<&dyn AppCtx>,
    typ: &str,
    vars: HashMap<String, String>,
) {
    let dedup_key = format!(
        "{}:{}",
        typ,
        vars.get("platform_id").map(String::as_str).unwrap_or("")
    );
    let env = Envelope::new(typ, Source::Proxy, vars, dedup_key, aidog_db::now());
    tracing::info!(typ = %env.typ, dedup_key = %env.dedup_key, "notify: semantic emit");
    super::dispatch(db, ctx, Some(&env.typ), "", None, &env.vars).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_mapping() {
        assert_eq!(severity_for_type("error"), Severity::Error);
        assert_eq!(severity_for_type("breaker_on"), Severity::Error);
        assert_eq!(severity_for_type("waiting_input"), Severity::Warn);
        assert_eq!(severity_for_type("quota_exhaust_forecast"), Severity::Warn);
        assert_eq!(severity_for_type("task_complete"), Severity::Info);
        assert_eq!(severity_for_type("breaker_off"), Severity::Info);
        // 透传 raw 事件名 → info 兜底。
        assert_eq!(severity_for_type("WhateverRaw"), Severity::Info);
    }

    #[test]
    fn envelope_new_fills_severity() {
        let env = Envelope::new(
            "breaker_on",
            Source::Proxy,
            HashMap::new(),
            "breaker_on:7".to_string(),
            123,
        );
        assert_eq!(env.severity, Severity::Error);
        assert_eq!(env.typ, "breaker_on");
    }

    #[test]
    fn semantic_types_cover_cc_and_proxy() {
        assert_eq!(SEMANTIC_TYPES.len(), 6);
        assert!(SEMANTIC_TYPES.contains(&"breaker_on"));
        assert!(SEMANTIC_TYPES.contains(&"quota_exhaust_forecast"));
    }
}
