//! 调用作用域（issue 55 票 04 / spec M2）：一次上游调用的「传输失败记账」与「成功记账」收口。
//! 传输失败走 [`failure_verdict::judge`]（`Failure::Transport`）+ [`failure_verdict::apply`]；
//! 成功统一 [`commit_success`]（record_success + 清 last_error + AutoDisabled 恢复）。
//! 判定逻辑全在 failure_verdict（纯函数），本模块只做 I/O 组装；四类请求
//! （聊天 forward、aux_model、decision、tools）共用。

use super::*;
use super::failure_verdict::{apply, judge, Failure, HealthEffect, JudgeCtx};

/// 每请求公共入参打包（handle_* 入口参数 ≤7，免 clippy::too_many_arguments）：
/// 日志开关、起始时钟、语言、客户端闸。
pub(crate) struct ReqCtx<'a> {
    pub(crate) log_settings: &'a ProxyLogSettings,
    pub(crate) start: std::time::Instant,
    pub(crate) lang: Lang,
    pub(crate) client: ClientGate,
}

/// 一次上游调用的共享上下文（四类请求通用）：路由结果 + 尝试位次 + 熔断阈值 + 客户端配置。
/// 字段 pub(crate)：由各调用方直接构造结构体字面量（字段多，构造器会触发
/// clippy::too_many_arguments）。
pub(crate) struct CallScope<'a> {
    pub(crate) state: &'a Arc<ProxyState>,
    pub(crate) group: &'a Group,
    pub(crate) route: &'a RouteResult,
    pub(crate) log_settings: &'a ProxyLogSettings,
    pub(crate) lang: Lang,
    pub(crate) start: std::time::Instant,
    pub(crate) attempt_start: std::time::Instant,
    pub(crate) attempt_ts: i64,
    /// 配额内最后一次（true → 传输失败直返 502，不再换候选）。
    pub(crate) is_last_candidate: bool,
    /// 当前排序里的候选数（connect 失败的本地网络保护分母）。
    pub(crate) candidate_total: usize,
    /// 命中槽位的模型链上是否还有下一个模型（chain_stop「跳过」留痕）。
    pub(crate) has_next_model: bool,
    pub(crate) breaker_th: super::scheduling::BreakerThresholds,
    pub(crate) proxy_client: &'a super::models::ProxyClientSettings,
    pub(crate) req_timeout: u64,
    pub(crate) conn_timeout: u64,
}

impl CallScope<'_> {
    /// 构建该平台的上游 HTTP 客户端（四类请求同一套超时 / 代理 / 平台 extra 配置）。
    pub(crate) async fn build_client(&self) -> reqwest::Client {
        super::http_client::build_http_client(
            self.proxy_client,
            self.req_timeout,
            self.conn_timeout,
            Some(&self.route.platform.extra),
            None,
        )
        .await
    }

    /// 传输失败（连接失败 / 读超时 / 中途断连）：judge(`Failure::Transport`) + apply 记账 +
    /// attempt 留痕（status_code=0、服务类 chain_stop）；非末位换候选（connect 失败回报
    /// 供 R3 死站联动跳过同 base_url 兄弟），末位 502 终态。`url` 仅用于日志。
    pub(crate) async fn transport_failed(
        &self,
        log: &mut ProxyLog,
        attempts: &mut Vec<ProxyAttempt>,
        e: &reqwest::Error,
        url: &str,
    ) -> AttemptOutcome {
        let failure = Failure::Transport { is_connect: e.is_connect() };
        let v = judge(
            &failure,
            &JudgeCtx {
                health_aware: self.breaker_th.health_aware,
                has_next_model: false,
                is_last_candidate: self.is_last_candidate,
                rule_non_retryable: false,
                now_ms: aidog_db::now(),
            },
        );
        let detail = err_chain(e);
        tracing::error!(
            url = %url, platform = %self.route.platform.name, error = %detail,
            duration_ms = self.start.elapsed().as_millis() as i64,
            "upstream request failed (502)"
        );
        let upstream_err = format!("upstream error: {detail}");
        attempts.push(ProxyAttempt {
            // 传输错误属服务类：不换模型直接换平台，链上剩余模型记「跳过」
            chain_stop: self.has_next_model.then(|| "service".to_string()),
            error_class: Some(v.class.as_str().to_string()),
            platform_id: self.route.platform.id,
            platform_name: self.route.platform.name.clone(),
            status_code: 0,
            error: upstream_err.clone(),
            duration_ms: self.attempt_start.elapsed().as_millis() as i64,
            ts: self.attempt_ts,
            model: Some(self.route.target_model.clone()),
            ..Default::default()
        });
        let breaker_counted = apply(
            &v,
            self.state,
            self.route,
            &self.breaker_th,
            &failure,
            self.candidate_total,
        )
        .await;
        if let Some(a) = attempts.last_mut() {
            a.effects = v
                .effects
                .iter()
                // breaker_count 字符串只在熔断真的被计（本地网络保护放行时不留痕，对齐 forward 旧语义）
                .filter(|fx| !matches!(fx, HealthEffect::BreakerCount) || breaker_counted)
                .map(|fx| fx.as_str().to_string())
                .collect();
        }
        // 传输失败的 last_error 在此写（错误文本在 reqwest::Error 里，judge / apply 拿不到）；
        // 成功清空走 commit_success。
        let _ = aidog_db::set_platform_last_error(
            &self.state.db,
            self.route.platform.id,
            Some(upstream_err.clone()),
        )
        .await;
        if !self.is_last_candidate {
            return AttemptOutcome::Next { connect_failed: e.is_connect() };
        }
        let msg = format!("{}: {detail}", i18n::t(self.lang, ErrorKey::Upstream));
        AttemptOutcome::Respond(
            finalize_proxy_502(
                self.state,
                log,
                attempts,
                self.route.platform.id,
                upstream_err,
                msg,
                self.start,
                self.log_settings,
            )
            .await,
        )
    }

    /// 2xx 成功记账：record_success（延迟 EMA + 熔断 / 冷却 / 降权清零）+ 清 last_error +
    /// AutoDisabled 恢复 + attempt 留痕。委托 [`commit_success`]（聊天 forward 宏共用）。
    pub(crate) async fn succeeded(&self, attempts: &mut Vec<ProxyAttempt>, status: u16) {
        let attempt_latency_ms = self.attempt_start.elapsed().as_millis() as i64;
        commit_success(
            self.state,
            self.route,
            attempts,
            status,
            None,
            attempt_latency_ms,
            self.attempt_ts,
        )
        .await;
    }
}

/// 成功记账唯一落地点（聊天 forward 的 commit_2xx_success! 宏与 [`CallScope::succeeded`] 共用）：
/// record_success（延迟 EMA + breaker Closed/HalfOpen→Closed + inflight 配平）、清本平台
/// last_error（无错时 SQL 的 `last_error != ''` 条件使 UPDATE 零行）、AutoDisabled → Enabled
/// 恢复 + attempt 留痕。
pub(crate) async fn commit_success(
    state: &Arc<ProxyState>,
    route: &RouteResult,
    attempts: &mut Vec<ProxyAttempt>,
    status_code: u16,
    ttfb_ms: Option<i64>,
    attempt_latency_ms: i64,
    attempt_ts: i64,
) {
    state.scheduler.record_success(route.platform.id, attempt_latency_ms);
    let _ = aidog_db::set_platform_last_error(&state.db, route.platform.id, None).await;
    attempts.push(ProxyAttempt {
        chain_stop: None,
        ttfb_ms,
        platform_id: route.platform.id,
        platform_name: route.platform.name.clone(),
        status_code: status_code as i32,
        error: String::new(),
        duration_ms: attempt_latency_ms,
        ts: attempt_ts,
        model: Some(route.target_model.clone()),
        ..Default::default()
    });
    if route.platform.status == super::models::PlatformStatus::AutoDisabled {
        if let Err(e) = aidog_db::recover_platform_auto_disabled(&state.db, route.platform.id).await {
            tracing::error!(platform_id = route.platform.id, error = %e, "recover auto-disabled platform failed");
        } else {
            tracing::info!(platform = %route.platform.name, platform_id = route.platform.id, "platform recovered from auto-disabled (2xx)");
            // status 列变了（auto_disabled → enabled）：复用 proxy-log-updated 通知前端停留页面轻刷。
            aidog_ctx::emit("proxy-log-updated", route.platform.id.into());
        }
    }
}
