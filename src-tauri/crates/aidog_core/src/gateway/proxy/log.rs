use super::*;

/// Read proxy log settings from DB
pub(crate) async fn get_log_settings(db: &Db) -> ProxyLogSettings {
    super::db::get_setting(db, "proxy", "logging")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// 单 writer 有界队列消息（s1 异步日志）。热路径只构造 + 入队，落库逻辑全在 writer 侧
/// （见 `spawn_log_writer`），snapshot 读-改-写/remove 串行化于同一 consumer，消除竞态。
pub(crate) enum LogMsg {
    /// 渐进式/终态 upsert（原 upsert_log 全部落库逻辑，见 `process_upsert`）。
    Upsert(Box<ProxyLog>, ProxyLogSettings),
    /// CONNECT 隧道一次性终态 INSERT（原 upsert_connect_log，见 `process_connect_log`）。
    Connect {
        id: String,
        group_key: String,
        platform_id: u64,
        request_url: String,
        status_code: i32,
        duration_ms: i32,
    },
    /// 测试用同步屏障：writer 处理到此消息即 ack，供测试在断言前等待此前所有入队消息落库
    /// 完成（FIFO 单 consumer 保证屏障之前的消息必已处理）。生产路径不发送。
    #[cfg(test)]
    Barrier(tokio::sync::oneshot::Sender<()>),
}

/// 终态判定（与 `process_upsert` 内 is_terminal 同判定）：决定背压分支——中间态队满即丢，
/// 终态队满则阻塞等待腾位，保证最终结果 / 统计 / cost / emit 不丢。
fn is_terminal_log(log: &ProxyLog) -> bool {
    log.status_code != 0 && log.response_body != "[stream]"
}

/// 热路径入口：把日志投递进 `ProxyState.log_tx` 有界 mpsc 队列，构造 + 入队后立即返回，
/// 不 `.await` 任何 DB 操作（DB 写全部移入 `spawn_log_writer` 单 writer 串行处理）。
///
/// 背压（硬约束，s1 设计）：中间态（status==0 / response_body=="[stream]" 占位）队满即用
/// `try_send` 静默丢弃——不影响最终数据，终态 upsert 会覆盖写全部列；终态（真实 HTTP 结果）
/// 队满则退化为阻塞 `send().await` 等待 writer 腾位，保证不丢失最终结果 / 统计 / cost / emit。
pub(crate) async fn upsert_log(state: &Arc<ProxyState>, log: &ProxyLog, settings: &ProxyLogSettings) {
    let msg = LogMsg::Upsert(Box::new(log.clone()), settings.clone());
    if is_terminal_log(log) {
        if state.log_tx.send(msg).await.is_err() {
            tracing::warn!(id = %log.id, "log writer channel closed, terminal log dropped");
        }
    } else if let Err(e) = state.log_tx.try_send(msg) {
        match e {
            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                tracing::debug!(id = %log.id, "log queue full, non-terminal log dropped (backpressure)");
            }
            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                tracing::debug!(id = %log.id, "log writer channel closed, non-terminal log dropped");
            }
        }
    }
}

/// 单 writer 后台任务：串行消费 `rx`，逐条落库。保序（单 consumer FIFO）替代原「caller 串行
/// `.await`」；snapshot 读-改-写 + 终态 remove 全在本任务的处理函数内完成，消除多请求并发
/// diff 同一 id 快照的竞态。`start_proxy` 启动时 spawn，与 `ProxyState` 同生命周期。
///
/// ponytail: 关机 drain 走「等 rx 自然排空」这条设计允许的简化路径——writer 持有自己的
/// `Arc<ProxyState>` 克隆，`proxy_stop` abort 的只是 axum serve 任务，本 writer 不受影响，
/// 继续消费所有已入队消息直至队列见底再空闲等待；仅在整进程被杀（非 graceful）时会连同
/// buffer 中未处理的终态一起丢失（与旧同步实现下"写到一半被杀"的既有风险同级，未劣化）。
/// 如未来需要严格保证 graceful proxy_stop 也不丢日志，可加 oneshot shutdown 信号 + `rx.close()`
/// 显式 drain-then-join，再由 commands_proxy::proxy_stop 触发。
pub(crate) fn spawn_log_writer(
    state: Arc<ProxyState>,
    mut rx: tokio::sync::mpsc::Receiver<LogMsg>,
) -> tokio::task::JoinHandle<()> {
    crate::logging::spawn_traced("log_writer", async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                LogMsg::Upsert(log, settings) => process_upsert(&state, &log, &settings).await,
                LogMsg::Connect { id, group_key, platform_id, request_url, status_code, duration_ms } => {
                    process_connect_log(&state, id, group_key, platform_id, request_url, status_code, duration_ms).await;
                }
                #[cfg(test)]
                LogMsg::Barrier(ack) => {
                    let _ = ack.send(());
                }
            }
        }
        tracing::info!("log writer: channel closed (all senders dropped), exiting");
    })
}

/// 测试专用：等待此前所有已入队消息被 writer 处理完（FIFO 屏障），供断言前同步。
#[cfg(test)]
pub(crate) async fn flush_log_queue(state: &Arc<ProxyState>) {
    let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
    if state.log_tx.send(LogMsg::Barrier(ack_tx)).await.is_ok() {
        let _ = ack_rx.await;
    }
}

/// upsert 落库主逻辑（原 upsert_log 函数体，现只在 `spawn_log_writer` 内单 writer 串行调用）。
/// 语义不变：Respects ProxyLogSettings: if logging disabled, does nothing;
/// if user/upstream recording disabled, clears those fields before writing.
pub(crate) async fn process_upsert(state: &Arc<ProxyState>, log: &ProxyLog, settings: &ProxyLogSettings) {
    // ── 聚合统计写入（解耦于日志开关）──
    // 必须在 `!settings.enabled` 早退之前：关日志时统计仍需写。仅终态请求计入
    // （status!=0 且非流式占位 "[stream]"，与下方 is_terminal 同判定，避免占位/中间节点重复计）。
    // est_cost：log 已带则用；否则（关日志路径不会经下方计算）就地走 calc_est_cost 回退链。
    // 失败非致命：warn 不中断请求。eff_pid 回溯在 upsert_stats_agg 的 SQL 内做。
    //
    // 去重：upsert_log 在单个请求生命周期内被多次调用（insert + 多次 update + 流式 flush），
    // 终态后每次调用 gate 仍为真。HashSet::insert 返回 false 表示该 id 已聚合过 → 跳过，
    // 保证每请求只 +1 一次（id 在 remove_log_snapshot 清理，见下）。
    // count_tokens 子端点（/v1/messages/count_tokens）是纯计数调用、不发生推理，不该计入
    // stats_agg 聚合/总统计（否则 Stats 页/托盘成本虚高，实测占全库 17.6%）。
    // proxy_log 单行照旧保留 input_tokens + est_cost（供单行审计可见），仅聚合路径跳过。
    // 识别复用 request_url 判定，避免加列迁移；与 is_count_tokens_endpoint 同款尾段匹配。
    let is_count_tokens = is_count_tokens_endpoint(&log.request_url);
    let first_agg = log.status_code != 0
        && log.response_body != "[stream]"
        && !is_count_tokens
        && agg_mark_first(state, &log.id);

    // est_cost 统一计算（两分支复用，避免重复 get_platform + calc_est_cost 调用）。
    // 结果存局部变量，first_agg 分支用值，日志写入分支用 Option（后续覆盖）。
    //
    // ponytail: 仅在终态（status_code != 0）计算 cost。upsert_log 单请求生命周期被调 40+ 次
    // （见 mod.rs 注释），中间态（status=0 / 流式聚合中）每次重复 get_platform + calc_est_cost
    // 是 CPU/DB 浪费 —— 中间态的 est_cost 列写入会被后续 upsert 覆盖，最终值由终态 upsert 决定；
    // first_agg 也仅在 status_code != 0 时触发，二者天然对齐。无终态 upsert 的请求（被 abort）
    // 由 RequestLogGuard Drop 兜底写 status=499，仍会命中此分支计算。统计正确性不变。
    let est_cost_value = if log.est_cost == 0.0
        && (log.input_tokens > 0 || log.output_tokens > 0)
        && log.status_code != 0
    {
        let model_name = if log.actual_model.is_empty() {
            log.model.clone()
        } else {
            log.actual_model.clone()
        };
        let platform_type = super::db::get_platform(&state.db, log.platform_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.platform_type.wire_str())
            .unwrap_or_default();
        Some(super::db::calc_est_cost(
            &state.db,
            &model_name,
            &platform_type,
            log.input_tokens,
            log.output_tokens,
            log.cache_tokens,
            log.platform_id as i64,
            log.created_at,
        )
        .await)
    } else {
        None
    };

    if first_agg {
        let cost = est_cost_value.unwrap_or(log.est_cost);
        let agg_input = super::db::StatsAggInput {
            created_at: log.created_at,
            model: if log.actual_model.is_empty() { log.model.clone() } else { log.actual_model.clone() },
            group_key: log.group_key.clone(),
            platform_id: log.platform_id as i64,
            status_code: log.status_code,
            input_tokens: log.input_tokens as i64,
            output_tokens: log.output_tokens as i64,
            cache_tokens: log.cache_tokens as i64,
            est_cost: cost,
            duration_ms: log.duration_ms as i64,
        };
        if let Err(e) = super::db::upsert_stats_agg(&state.db, agg_input).await {
            tracing::warn!(error = %e, "stats_agg upsert failed (non-fatal)");
        }

        // (A) 最终日志汇总条：每请求仅一条（复用 agg_mark_first 的一次性 gate，绝不重复）。
        // request_id = proxy_log.id（完整 32-hex，可串回 proxy_log / req span 的 request_id 字段）。
        // 同时作为 notification 渲染上下文的 vars 口径来源（request_id 唯一 key + status + tokens + cost）。
        tracing::info!(
            target: "final",
            request_id = %log.id,
            status = log.status_code,
            input_tokens = log.input_tokens,
            output_tokens = log.output_tokens,
            cache_tokens = log.cache_tokens,
            est_cost = cost,
            duration_ms = log.duration_ms,
            "request final"
        );
    }

    if !settings.enabled {
        return;
    }
    // 按 settings 就地脱敏构造入库列快照（仅克隆受影响 String 字段，不再 clone 整 ProxyLog 结构）。
    let strip_user = !settings.log_user_request;
    let strip_upstream = !settings.log_upstream_request;
    let mut cols = super::db::ProxyLogColumns::from_log(log, strip_user, strip_upstream);

    // est_cost 复用上方计算结果（避免重复 get_platform + calc_est_cost）。
    if let Some(cost) = est_cost_value {
        cols.est_cost = cost;
    }

    let id = cols.id.clone();
    let platform_id = log.platform_id;
    // 终态判定：有真实 HTTP 状态(status!=0)。唯一例外是流式占位写（response_body=="[stream]"，
    // 终态由 guard.flush 后显式 remove，不在此误删以免 guard 再 INSERT 撞主键）。
    // 覆盖流式请求在占位前就出错(如 502)的分支，避免快照泄漏。
    let is_terminal = cols.status_code != 0 && cols.response_body != "[stream]";

    // 取上一快照决定 INSERT(首节点) 还是 部分列 UPDATE(后续节点)。
    // DashMap 分片 get 返回 Ref（持读锁），.map(|r| r.clone()) 释锁后返回克隆，避免持锁跨 await。
    let prev = state.log_snapshots.get(&id).map(|r| r.clone());
    let write_ok = match prev {
        None => {
            // 首节点：建行。成功后存快照供后续 diff。
            let ok = super::db::insert_proxy_log_columns(&state.db, cols.clone()).await.is_ok();
            if ok {
                // OOM 止血：快照表只留 meta（清空 body/headers 大字段），N 并发不累积大 String。
                state.log_snapshots.insert(id.clone(), cols.into_snapshot_meta());
            }
            ok
        }
        Some(prev) => {
            // 后续节点：仅 UPDATE 变化列；成功后刷新快照。
            let ok = super::db::update_proxy_log_columns(&state.db, cols.clone(), &prev).await.is_ok();
            if ok {
                state.log_snapshots.insert(id.clone(), cols.into_snapshot_meta());
            }
            ok
        }
    };

    // 终态写完移除快照，防 in-flight map 无限增长（流式占位写除外，由 guard 显式移除）。
    if is_terminal {
        remove_log_snapshot(state, &id);
    }

    // ponytail: emit 节流 —— 仅终态触发前端 + 托盘事件，中间态 upsert（占位写 / 无 status 的
    // 流式中间 chunk）静默写库。upsert_log 单请求生命周期被调用 40+ 次（见 mod.rs 注释），
    // emit 从 40+ 次/请求 降到 1-2 次/请求（终态后少数重复调用）。前端 listener 各自 debounce
    // 兜底，丢失中间态刷新对 UI 无感（用户关心的是请求结束后的累计值）。
    if write_ok
        && is_terminal
        && let Some(app) = &state.app
    {
        use tauri::Emitter;
        let _ = app.emit("proxy-log-updated", platform_id);
        let _ = app.emit("tray-refresh", ());
    }
}

/// 移除某请求 id 的列快照（终态写入后调用，防止 in-flight 快照 map 无限增长）。
/// 流式 guard 终态 flush / 非流式终态返回前调用。重复调用安全（不存在即 no-op）。
/// 注意：不在此清 agg_done——终态 upsert_log 会被反复调用（remove 后下次 prev=None 又走终态），
/// 在此清会破坏去重；agg_done 自带 FIFO 容量上限，无需按请求清理。
pub(crate) fn remove_log_snapshot(state: &Arc<ProxyState>, id: &str) {
    state.log_snapshots.remove(id);
}

/// 中间件入站拦截：写审计日志（blocked_by/blocked_reason，不计费）并立即返回 403。
/// 参照现有 parse 错误返回模式；body 为结构化 JSON，便于客户端识别拦截。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn block_inbound(
    state: &Arc<ProxyState>,
    mut log: ProxyLog,
    log_settings: &ProxyLogSettings,
    lang: Lang,
    blocked_by: String,
    blocked_reason: String,
    start: std::time::Instant,
) -> Response {
    let body = serde_json::json!({
        "error": {
            "type": "middleware_blocked",
            "message": i18n::t(lang, ErrorKey::MiddlewareBlocked),
            "blocked_by": blocked_by,
            "blocked_reason": blocked_reason,
        }
    })
    .to_string();
    tracing::warn!(blocked_by = %blocked_by, reason = %blocked_reason, "middleware inbound: request blocked (403)");
    log.status_code = 403;
    log.blocked_by = blocked_by;
    log.blocked_reason = blocked_reason;
    log.response_body = body.clone();
    log.user_response_body = body.clone();
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    // est_cost 保持 0（不计费）；不调用 spawn_estimate。
    upsert_log(state, &log, log_settings).await;
    let mut r = (
        StatusCode::FORBIDDEN,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response();
    inject_trace_header(&mut r);
    r
}

/// 在后台 tokio::spawn 中执行请求驱动的 quota 预估（不阻塞响应）。
/// 余额平台扣金额 / coding plan 平台更新利用率，并按阈值触发真查校准。
/// platform_type 传入 serde rename 裸名（如 "deepseek"），供 resolve_price 查 pricing key。
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_estimate(
    state: &Arc<ProxyState>,
    platform_id: u64,
    platform_type: &Protocol,
    quota_base_url: String,
    api_key: String,
    model: String,
    extra: String,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    is_coding_plan: bool,
    span: tracing::Span,
) {
    // 无 token（请求失败 / 无 usage）则跳过
    if input_tokens <= 0 && output_tokens <= 0 && cache_tokens <= 0 {
        return;
    }
    let ptype = platform_type.wire_str();
    let db = state.db.clone();
    let app = state.app.clone();
    tokio::spawn(async move {
        super::estimate::estimate_after_request(
            &db,
            platform_id,
            &ptype,
            &quota_base_url,
            &api_key,
            &model,
            &extra,
            input_tokens as i64,
            output_tokens as i64,
            cache_tokens as i64,
            is_coding_plan,
        )
        .await;
        // 预估更新后通知主线程刷新托盘（emit 事件，避免后台线程直接操作 tray）
        if let Some(app) = app {
            use tauri::Emitter;
            let _ = app.emit("tray-refresh", ());
        }
    }.instrument(span));
}

/// P1 CONNECT 隧道元数据写入：独立路径，**不走 upsert_log**。
///
/// 原因：upsert_log 会触发 `upsert_stats_agg`（污染今日统计 — 隧道不计费，token=0）+
/// calc_est_cost（0 token 无意义）+ log_snapshots 渐进式 diff（隧道一次性终态，无中间节点）。
/// 本函数直接构造 `ProxyLogColumns`（全空 body / 0 token / 0 cost）→ insert_proxy_log_columns
/// 落一行。日志开关（settings.enabled）由调用方判断：disabled 时不调本函数。
///
/// 热路径入口：一次性终态入队（同 upsert_log 终态分支——队满阻塞等待腾位，不丢），不 `.await`
/// 落库；实际 INSERT 移入 `process_connect_log`（`spawn_log_writer` 单 writer 串行执行）。
pub(crate) async fn upsert_connect_log(
    state: &Arc<ProxyState>,
    id: String,
    group_key: String,
    platform_id: u64,
    request_url: String,
    status_code: i32,
    duration_ms: i32,
) {
    let msg = LogMsg::Connect { id: id.clone(), group_key, platform_id, request_url, status_code, duration_ms };
    if state.log_tx.send(msg).await.is_err() {
        tracing::warn!(id = %id, "log writer channel closed, connect log dropped");
    }
}

/// connect log 落库主逻辑（原 upsert_connect_log 函数体，现只在 writer 内单 writer 串行调用）。
///
/// 字段语义（PRD 锁）:
/// - `source_protocol`/`target_protocol` = `"http-connect"`（Logs 页区分隧道请求）
/// - `platform_id` = host 命中平台 else 0
/// - `request_url` = CONNECT target（`host:port`）
/// - `status_code` = 200（隧道建立成功）/ 502（上游连不上）/ 499（客户端断）
/// - tokens/cost = 0（P1 不解析 body）
async fn process_connect_log(
    state: &Arc<ProxyState>,
    id: String,
    group_key: String,
    platform_id: u64,
    request_url: String,
    status_code: i32,
    duration_ms: i32,
) {
    let now = super::db::now();
    let cols = super::db::ProxyLogColumns {
        id,
        group_key,
        model: String::new(),
        actual_model: String::new(),
        source_protocol: "http-connect".to_string(),
        target_protocol: "http-connect".to_string(),
        platform_id: platform_id as i64,
        request_headers: String::new(),
        request_body: String::new(),
        upstream_request_headers: String::new(),
        upstream_request_body: String::new(),
        response_body: String::new(),
        request_url,
        upstream_request_url: String::new(),
        upstream_response_headers: String::new(),
        upstream_status_code: 0,
        user_response_headers: String::new(),
        user_response_body: String::new(),
        status_code,
        duration_ms,
        input_tokens: 0,
        output_tokens: 0,
        cache_tokens: 0,
        est_cost: 0.0,
        is_stream: 0,
        attempts: String::new(),
        retry_count: 0,
        blocked_by: String::new(),
        blocked_reason: String::new(),
        created_at: now,
        updated_at: now,
        deleted_at: 0,
        cli_proxy_provider_id: None,
    };
    if let Err(e) = super::db::insert_proxy_log_columns(&state.db, cols).await {
        tracing::warn!(error = %e, "connect log insert failed (non-fatal)");
        return;
    }
    // 通知前端 Platforms/Stats 刷新（platform_id 可能为 0，前端按需处理）。
    if let Some(app) = &state.app {
        use tauri::Emitter;
        let _ = app.emit("proxy-log-updated", platform_id);
        let _ = app.emit("tray-refresh", ());
    }
}

#[cfg(test)]
#[path = "test_log.rs"]
mod test_log;
