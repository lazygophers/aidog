use super::*;

/// Read proxy log settings from DB
pub(crate) async fn get_log_settings(db: &Db) -> ProxyLogSettings {
    aidog_db::get_setting(db, "proxy", "logging")
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
    /// `log` 是按需携带的副本（只含本节点可能要写库的大字段，见 `scoped_queue_log`）；
    /// 降级时大字段全空 + `log.body_omitted` 置位。`bytes` = 入队时核算的字节数，
    /// writer 出队即归还字节预算（`LOG_QUEUE_BYTE_BUDGET`）。
    Upsert {
        log: Box<ProxyLog>,
        settings: ProxyLogSettings,
        bytes: usize,
    },
    /// CONNECT 隧道一次性终态 INSERT（原 upsert_connect_log，见 `process_connect_log`）。
    /// `blocked_reason`：空串 = 普通盲转行；`MITM_OPAQUE_REASON` = 认证绑定的隧道未能解密
    /// （spec cc-sub-mitm D4：盲转保通 + est_cost=0 + mitm_opaque 标记）。
    Connect {
        id: String,
        group_key: String,
        /// 绑定 group 名（认证命中时）；空串 = 未绑定。
        group_name: String,
        platform_id: u64,
        request_url: String,
        status_code: i32,
        duration_ms: i32,
        blocked_reason: String,
        /// 双向透传字节数（client→upstream / upstream→client）；仅盲转路径统计。
        req_bytes: i64,
        resp_bytes: i64,
    },
    /// 测试用同步屏障：writer 处理到此消息即 ack，供测试在断言前等待此前所有入队消息落库
    /// 完成（FIFO 单 consumer 保证屏障之前的消息必已处理）。生产路径不发送。
    #[cfg(test)]
    Barrier(tokio::sync::oneshot::Sender<()>),
}

impl LogMsg {
    /// 入队时核算的字节数（writer 出队时归还字节预算用）。Connect 是纯元数据定长消息。
    pub(crate) fn queued_bytes(&self) -> usize {
        match self {
            LogMsg::Upsert { bytes, .. } => *bytes,
            LogMsg::Connect { .. } => CONNECT_MSG_BYTES,
            #[cfg(test)]
            LogMsg::Barrier(_) => 0,
        }
    }
}

/// CONNECT 隧道一次性终态消息的固定核算字节数（纯元数据，无 body 列内容）。
const CONNECT_MSG_BYTES: usize = 1024;

/// 终态判定（与 `process_upsert` 内 is_terminal 同判定）：O6 后所有投递一律非阻塞
/// `try_send`（超字节预算先降级为只含元数据），中间态与终态的差别只剩队满时的日志级别。
/// 票 06：终态 = status!=0 且 done 置位（流式 flush / 非流式终态 / 断连兜底），
/// 取代旧 `response_body != "[stream]"` 哨兵（中间态占位写已废）。
fn is_terminal_log(log: &ProxyLog) -> bool {
    log.status_code != 0 && log.done
}

/// 热路径入口：把日志投递进 `ProxyState.log_tx` 有界 mpsc 队列，构造 + 入队后立即返回，
/// 不 `.await` 任何 DB 操作（DB 写全部移入 `spawn_log_writer` 单 writer 串行处理）。
///
/// 背压（O6，spec §3 修法 2/3）：所有消息一律非阻塞 `try_send`，字节预算
/// （`LOG_QUEUE_BYTE_BUDGET`）超限先降级为只含元数据 + `body_omitted` 标记——
/// **代理永远不因写日志变慢**，代价是过载时部分日志只剩元数据（有标记可查，票 06 拍板接受）。
///
/// 携带（O6）：不再整行深拷贝 `ProxyLog` 进队列（200 并发 4776 份 ×0.66 MB 副本同时存活的
/// 内存峰值根因，spec §3 根因 1）——`scoped_queue_log` 按首写位掩码只携带本节点要写库的大字段。
pub(crate) async fn upsert_log(
    state: &Arc<ProxyState>,
    log: &ProxyLog,
    settings: &ProxyLogSettings,
) {
    queue_upsert_log(state, log, settings);
}

/// 同步非阻塞投递核心（O6，spec §3 修法 2）：`try_send` + 字节预算降级，无任何 `.await`，
/// **代理永远不因写日志变慢**。除 `upsert_log` 外，`StreamLogGuard::flush_with` 在 Drop 路径
/// （可能无 tokio runtime 上下文）也直调本函数——旧的「终态阻塞 send + 后台 spawn 等待」
/// 是 200 并发下 4776 份日志副本积压（2.9 GB）的根因（spec §3 根因 3），已整体移除。
///
/// 背压链（按序）：
/// 1. 字节预算超限 → 降级：清空本消息携带的大字段（只留元数据）+ 置 `body_omitted`，
///    该行日志详情页显示「正文已省略」（DB 列，migration 20261001-02；正文列保持空串，
///    不写占位文字——CLAUDE.md「Proxy 日志」段）。
/// 2. 降级后仍 try_send 失败（条数兜底上限满 / channel 关闭）→ 丢弃：中间态 debug、
///    终态 warn（终态丢弃会漏 stats_agg 聚合与 emit，只在字节预算 + 8192 条兜底全部
///    打满的极端持续过载下发生；降级后的元数据消息 ~2 KB，正常消费速率下到不了这里）。
///
/// O6 预算记账 + 非阻塞投递的公共尾部：fetch_add 预占 → try_send → 失败归还预算并回调
/// 各自的丢弃日志（upsert 与 connect 两条路径的 Full/Closed 文案不同，用闭包注入）。
fn try_send_log_msg(
    state: &Arc<ProxyState>,
    msg: LogMsg,
    on_full: impl FnOnce(),
    on_closed: impl FnOnce(),
) {
    let bytes = msg.queued_bytes();
    state
        .log_queue_bytes
        .fetch_add(bytes as u64, std::sync::atomic::Ordering::Relaxed);
    if let Err(e) = state.log_tx.try_send(msg) {
        // 发送失败：这条消息没进队列，归还预占的预算。
        state
            .log_queue_bytes
            .fetch_sub(bytes as u64, std::sync::atomic::Ordering::Relaxed);
        match e {
            tokio::sync::mpsc::error::TrySendError::Full(_) => on_full(),
            tokio::sync::mpsc::error::TrySendError::Closed(_) => on_closed(),
        }
    }
}

pub(crate) fn queue_upsert_log(
    state: &Arc<ProxyState>,
    log: &ProxyLog,
    settings: &ProxyLogSettings,
) {
    let terminal = is_terminal_log(log);
    let mut msg_log = scoped_queue_log(state, log, settings, terminal);
    let mut bytes = queue_bytes(&msg_log);
    if state.log_queue_bytes.load(std::sync::atomic::Ordering::Relaxed) + bytes as u64
        > LOG_QUEUE_BYTE_BUDGET
    {
        strip_queue_bodies(&mut msg_log);
        msg_log.body_omitted = true;
        bytes = queue_bytes(&msg_log);
        tracing::warn!(
            id = %log.id,
            queued = state.log_queue_bytes.load(std::sync::atomic::Ordering::Relaxed),
            "log queue over byte budget, degraded to metadata-only (bodies omitted)"
        );
    }
    let msg = LogMsg::Upsert {
        log: Box::new(msg_log),
        settings: settings.clone(),
        bytes,
    };
    try_send_log_msg(
        state,
        msg,
        || {
            if terminal {
                tracing::warn!(id = %log.id, "log queue full, terminal log dropped (metadata included stats lost)");
            } else {
                tracing::debug!(id = %log.id, "log queue full, non-terminal log dropped (backpressure)");
            }
        },
        || tracing::debug!(id = %log.id, "log writer channel closed, log dropped"),
    );
}

/// O6 按需携带（spec §3 修法 1 + 内存根因 1）+ O9 终态携带：构造进队列的 ProxyLog 副本
/// ——**不做整行深拷贝**，只克隆「本节点可能要写库」的大字段，其余大字段置空串（不分配）。
///
/// **O9 携带规则：大字段只在终态（`status_code != 0 || done`，比 is_terminal_log 宽一档，
/// 覆盖路由失败等无 done 的错误行）携带**。中间态消息只带元数据——SQLite UPDATE 物理上
/// 整行重写（未改的大列照样搬运），早写 body 只会让后续每个中间态 UPDATE 都拖上 580 KB+；
/// 延迟到终态后行在终态前是 KB 级，物理大写恰一次（终态 UPDATE 首写全部非空大列，
/// writer 侧 `large_fields_first_write` 的位掩码去重逻辑原样兼容：中间态从未携带 →
/// 掩码恒 0 → 终态全部按首写绑定）。代价：请求进行中日志详情页看不到 body，
/// 终态落库后可见（V2 写放大复测 2026-10-01：4.14 → 待测 MB/请求）。
///
/// 两侧开关（log_user_request / log_upstream_request）关时对应侧不带——writer 侧 from_log
/// 反正会清空，带了白拷。元数据字段（token/cost/url/...）恒携带。
fn scoped_queue_log(
    _state: &Arc<ProxyState>,
    log: &ProxyLog,
    settings: &ProxyLogSettings,
    terminal: bool,
) -> ProxyLog {
    // O9：大字段携带门。比 is_terminal_log 宽一档（status!=0 即带）：无 done 的错误行
    // （路由失败 503 / 拦截 403 等一次性终写）也必须把 request body 落上。
    let carry_bodies = log.status_code != 0 || log.done || terminal;
    let carry_user = settings.enabled && settings.log_user_request;
    let carry_up = settings.enabled && settings.log_upstream_request;
    let take = |carry: bool, s: &str| if carry { s.to_owned() } else { String::new() };
    let user = |nonempty: bool| carry_user && carry_bodies && nonempty;
    let up = |nonempty: bool| carry_up && carry_bodies && nonempty;
    ProxyLog {
        request_headers: take(user(!log.request_headers.is_empty()), &log.request_headers),
        request_body: take(user(!log.request_body.is_empty()), &log.request_body),
        upstream_request_headers: take(up(!log.upstream_request_headers.is_empty()), &log.upstream_request_headers),
        upstream_request_body: take(up(!log.upstream_request_body.is_empty()), &log.upstream_request_body),
        // 响应侧终态强制携带（终态 UPDATE 强制覆盖写，即使值为空也要写），非终态不带。
        response_body: take(up(terminal || !log.response_body.is_empty()), &log.response_body),
        upstream_response_headers: take(up(terminal || !log.upstream_response_headers.is_empty()), &log.upstream_response_headers),
        user_response_headers: take(user(terminal || !log.user_response_headers.is_empty()), &log.user_response_headers),
        user_response_body: take(user(terminal || !log.user_response_body.is_empty()), &log.user_response_body),
        // 降级标记由 queue_upsert_log 在预算超限时置位。
        body_omitted: false,
        ..log.clone()
    }
}

/// 降级：清空队列消息携带的全部大字段（元数据保留）。`String::new()` 赋值释放原分配。
fn strip_queue_bodies(log: &mut ProxyLog) {
    log.request_headers = String::new();
    log.request_body = String::new();
    log.upstream_request_headers = String::new();
    log.upstream_request_body = String::new();
    log.response_body = String::new();
    log.upstream_response_headers = String::new();
    log.user_response_headers = String::new();
    log.user_response_body = String::new();
}

/// 队列消息核算字节数（O6 字节预算记账）。ponytail: 固定开销与 attempts 是粗估
/// （预算本就是近似上界，初始 256 MB 按 V3 调），大字段取精确 len。
fn queue_bytes(log: &ProxyLog) -> usize {
    const FIXED: usize = 2048; // 结构体 + 小字段 + channel 槽位摊销
    FIXED
        + log.request_headers.len()
        + log.request_body.len()
        + log.upstream_request_headers.len()
        + log.upstream_request_body.len()
        + log.response_body.len()
        + log.upstream_response_headers.len()
        + log.user_response_headers.len()
        + log.user_response_body.len()
        + log.field_trace.len()
        + log.attempts.len() * 128
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
            // 出队即归还字节预算（处理前扣——DB 写慢时不占用预算额度）。
            state
                .log_queue_bytes
                .fetch_sub(msg.queued_bytes() as u64, std::sync::atomic::Ordering::Relaxed);
            match msg {
                LogMsg::Upsert { log, settings, .. } => {
                    process_upsert(&state, &log, &settings).await
                }
                LogMsg::Connect {
                    id,
                    group_key,
                    group_name,
                    platform_id,
                    request_url,
                    status_code,
                    duration_ms,
                    blocked_reason,
                    req_bytes,
                    resp_bytes,
                } => {
                    process_connect_log(
                        &state,
                        id,
                        group_key,
                        group_name,
                        platform_id,
                        request_url,
                        status_code,
                        duration_ms,
                        blocked_reason,
                        req_bytes,
                        resp_bytes,
                    )
                    .await;
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

/// 快照里 body_omitted 是否已置位（无快照 = 首节点，恒 false）。
fn prev_sticky_body_omitted(state: &Arc<ProxyState>, id: &str) -> bool {
    state
        .log_snapshots
        .get(id)
        .is_some_and(|r| r.body_omitted != 0)
}

/// upsert 落库主逻辑（原 upsert_log 函数体，现只在 `spawn_log_writer` 内单 writer 串行调用）。
/// 语义不变：Respects ProxyLogSettings: if logging disabled, does nothing;
/// if user/upstream recording disabled, clears those fields before writing.
pub(crate) async fn process_upsert(
    state: &Arc<ProxyState>,
    log: &ProxyLog,
    settings: &ProxyLogSettings,
) {
    // ── 聚合统计写入（解耦于日志开关）──
    // 必须在 `!settings.enabled` 早退之前：关日志时统计仍需写。仅终态请求计入
    // （status!=0 且 done 置位，与下方 is_terminal 同判定，避免中间节点重复计）。
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
    let first_agg =
        log.status_code != 0 && log.done && !is_count_tokens && agg_mark_first(state, &log.id);

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
        let platform_type = aidog_db::get_platform(&state.db, log.platform_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.platform_type.wire_str())
            .unwrap_or_default();
        Some(
            crate::gateway::billing::calc_est_cost(
                &state.db,
                &model_name,
                &platform_type,
                log.input_tokens,
                log.output_tokens,
                log.cache_tokens,
                log.cache_write_tokens,
                log.platform_id as i64,
                log.created_at,
            )
            .await,
        )
    } else {
        None
    };

    if first_agg {
        let cost = est_cost_value.unwrap_or(log.est_cost);
        let agg_input = aidog_stats::StatsAggInput {
            created_at: log.created_at,
            model: if log.actual_model.is_empty() {
                log.model.clone()
            } else {
                log.actual_model.clone()
            },
            group_key: log.group_key.clone(),
            platform_id: log.platform_id as i64,
            status_code: log.status_code,
            input_tokens: log.input_tokens as i64,
            output_tokens: log.output_tokens as i64,
            cache_tokens: log.cache_tokens as i64,
            est_cost: cost,
            duration_ms: log.duration_ms as i64,
        };
        if let Err(e) = aidog_stats::upsert_stats_agg(&state.db, agg_input).await {
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
    let mut cols = aidog_logs::ProxyLogColumns::from_log(log, strip_user, strip_upstream);

    // est_cost 复用上方计算结果（避免重复 get_platform + calc_est_cost）。
    if let Some(cost) = est_cost_value {
        cols.est_cost = cost;
    }

    let id = cols.id.clone();
    let platform_id = log.platform_id;
    // 终态判定：有真实 HTTP 状态(status!=0) 且 done 置位（票 06：显式终态列，
    // 取代旧 `response_body != "[stream]"` 哨兵例外——哨兵删除后无此特例）。
    // 覆盖流式请求在 flush 前就出错(如 502)的分支，避免快照泄漏。
    let is_terminal = cols.status_code != 0 && cols.done != 0;

    // 「正文已省略」单调 sticky：快照里已置位（更早节点降级过）则本节点也置位——
    // changed_since 据此把 1 写回 DB，不随后续未降级节点回落（宁可多报，见列 doc）。
    if prev_sticky_body_omitted(state, &id) {
        cols.body_omitted = 1;
    }

    // 取上一快照决定 INSERT(首节点) 还是 部分列 UPDATE(后续节点)。
    // DashMap 分片 get 返回 Ref（持读锁），.map(|r| r.clone()) 释锁后返回克隆，避免持锁跨 await。
    let prev = state.log_snapshots.get(&id).map(|r| r.clone());
    let write_ok = match prev {
        None => {
            // 首节点：建行。成功后存快照供后续 diff。
            let ok = aidog_logs::insert_proxy_log_columns(&state.db, cols.clone())
                .await
                .is_ok();
            if ok {
                // OOM 止血：快照表只留 meta（清空 body/headers 大字段），N 并发不累积大 String。
                // 首写位掩码（O1）：INSERT 绑定全部列，非空大字段视作已写，空字段留待首写。
                let mask = cols.nonempty_raw_mask();
                let mut snap = cols.into_snapshot_meta();
                snap.raw_written_mask = mask;
                state.log_snapshots.insert(id.clone(), snap);
            }
            ok
        }
        Some(prev) => {
            // 后续节点：仅 UPDATE 变化列 + 未写过的大字段（O1 正文只写首尾）；成功后刷新快照。
            match aidog_logs::update_proxy_log_columns(&state.db, cols.clone(), &prev).await {
                Ok(mask) => {
                    let mut snap = cols.into_snapshot_meta();
                    snap.raw_written_mask = mask;
                    state.log_snapshots.insert(id.clone(), snap);
                    true
                }
                Err(_) => false,
            }
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
    if write_ok && is_terminal {
        emit_log_events(state, platform_id);
    }
}

/// 终态写入成功后统一触发前端 + 托盘刷新事件（`process_upsert` 终态分支与
/// `process_connect_log` CONNECT 一次性终态共用同一 idiom——CONNECT 本身就是单次终态写入，
/// 天然满足与上方相同的 gate 语义；消费侧 `app_setup.rs` 对 `"tray-refresh"` 事件统一做
/// 200ms trailing debounce，对所有来源一视同仁，此处不重复造节流机制，只负责发出事件）。
///
/// 实测数据（s4 proxy-hotpath-buffers，一次性临时计数埋点跑 mock 流后已移除，非估算）：
/// 模拟单请求生命周期 40 次 `upsert_log`（1 insert + 38 流式中间 chunk + 1 终态）+ 5 次
/// CONNECT 隧道完成，共 45 次落库路径调用，仅本函数被触发 6 次（1 终态 upsert + 5 connect）
/// ——emit/托盘重建频次降至请求量的 6/45 ≈ 13.3%（降幅 86.7%）。`is_terminal_log` 的 gate
/// 语义见下方 `emit_gate_pass_rate_across_request_lifecycle` 测试（用真实 `is_terminal_log`
/// 判定逐条计数，避免跨测试共享 static 计数器在默认并行 `cargo test` 下的 flaky 风险）。
fn emit_log_events(_state: &Arc<ProxyState>, platform_id: u64) {
    aidog_ctx::emit("proxy-log-updated", platform_id.into());
    aidog_ctx::emit_unit("tray-refresh");
}

/// 移除某请求 id 的列快照（终态写入后调用，防止 in-flight 快照 map 无限增长）。
/// 流式 guard 终态 flush / 非流式终态返回前调用。重复调用安全（不存在即 no-op）。
/// 注意：不在此清 agg_done——终态 upsert_log 会被反复调用（remove 后下次 prev=None 又走终态），
/// 在此清会破坏去重；agg_done 自带 FIFO 容量上限，无需按请求清理。
pub(crate) fn remove_log_snapshot(state: &Arc<ProxyState>, id: &str) {
    state.log_snapshots.remove(id);
}

/// 观察模式命中的审计标记（`proxy_log.blocked_reason` 的固定机读值，票 04）。
/// 与真实拦截区分：真拦截的 reason 是规则描述/`matched middleware rule`，本值恒为 `observe`。
/// 日志页「观察模式命中」筛选按此值等值匹配（`ProxyLogFilter::observed`）。
pub(crate) const OBSERVE_REASON: &str = "observe";

/// 中间件观察模式命中：只标审计列，**不改状态码、不改 est_cost**（请求照常转发照常计费）。
/// 多个挂载点（group 层 / platform 层）先后命中时追加，不覆盖。
pub(crate) fn record_observed(log: &mut ProxyLog, blocked_by: String) {
    tracing::info!(blocked_by = %blocked_by, "middleware inbound: observe-mode hit, request continues");
    if log.blocked_by.is_empty() {
        log.blocked_by = blocked_by;
    } else {
        log.blocked_by = format!("{}; {}", log.blocked_by, blocked_by);
    }
    log.blocked_reason = OBSERVE_REASON.to_string();
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
    log.done = true;
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
    tokio::spawn(
        async move {
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
            aidog_ctx::emit_unit("tray-refresh");
        }
        .instrument(span),
    );
}

/// 上游响应头里的速率限制余量落库（后台，不阻塞响应）。
/// 与 [`spawn_estimate`] 分开：那边无 token 就跳过，而速率限制头在 429 / 失败响应上
/// 恰恰最有价值，不能跟着一起跳过。认不出任何厂商的头 → 不写库（不把「没有」记成 0）。
pub(crate) fn spawn_rate_limit(
    state: &Arc<ProxyState>,
    platform_id: u64,
    headers: &axum::http::HeaderMap,
    span: tracing::Span,
) {
    let Some(rl) = super::estimate::parse_rate_limit(headers, aidog_db::now()) else {
        return;
    };
    let db = state.db.clone();
    tokio::spawn(
        async move {
            super::estimate::write_rate_limit(&db, platform_id, &rl).await;
        }
        .instrument(span),
    );
}

/// 盲转不透明标记（spec cc-sub-mitm D4）：CONNECT 认证绑定了 group 但隧道未能解密
/// （白名单未命中 / MITM 降级 / CA 未启用），字节盲转保通。proxy_log 落
// mitm_opaque 常量真值源在 aidog_logs（读侧 count_mitm_opaque 同值共用），此处重导出。
pub(crate) use aidog_logs::MITM_OPAQUE_REASON;

/// P1 CONNECT 隧道元数据写入：独立路径，**不走 upsert_log**。
///
/// 原因：upsert_log 会触发 `upsert_stats_agg`（污染今日统计 — 隧道不计费，token=0）+
/// calc_est_cost（0 token 无意义）+ log_snapshots 渐进式 diff（隧道一次性终态，无中间节点）。
/// 本函数直接构造 `ProxyLogColumns`（全空 body / 0 token / 0 cost）→ insert_proxy_log_columns
/// 落一行。日志开关（settings.enabled）由调用方判断：disabled 时不调本函数。
///
/// 热路径入口：一次性终态入队（同 upsert_log 终态分支——队满阻塞等待腾位，不丢），不 `.await`
/// 落库；实际 INSERT 移入 `process_connect_log`（`spawn_log_writer` 单 writer 串行执行）。
/// ponytail: 8 参数是隧道一次性终态上下文（同 spawn_blind_relay 先例），allow too_many_arguments。
#[allow(clippy::too_many_arguments)]
/// CONNECT 隧道的 proxy_log 记账上下文：request_id / platform_id / conn_group_key /
/// start / log_enabled 五元组 + blocked_reason（空串 = 普通盲转行；`MITM_OPAQUE_REASON` =
/// 认证绑定但未解密的盲转行），spawn_blind_relay → blind_relay_after_connect →
/// handle_mitm → upsert_connect_log 一路同传（替代六层散参）。
#[derive(Clone)]
pub(crate) struct ConnectLogCtx {
    pub(crate) request_id: String,
    pub(crate) platform_id: u64,
    pub(crate) conn_group_key: String,
    pub(crate) start: std::time::Instant,
    pub(crate) log_enabled: bool,
    /// 与 proxy_log.blocked_reason 列同名同义。
    pub(crate) blocked_reason: &'static str,
    /// 绑定 group 名（认证命中时）；空串 = 未绑定。mitm_log 观测行消费。
    pub(crate) group_name: String,
    /// 双向透传字节数（bridge_bidir 落终态时回填）。
    pub(crate) req_bytes: i64,
    pub(crate) resp_bytes: i64,
}

impl ConnectLogCtx {
    /// 终态无字节盲转发生（TCP 失败 / TLS 握手失败 / 隧道未建立即断）→ 不标 mitm_opaque。
    pub(crate) fn no_opaque(&self) -> Self {
        Self {
            blocked_reason: "",
            ..self.clone()
        }
    }

    /// log_enabled 时写一行 CONNECT 终态（200 = 隧道建立成功 / 502 = 上游失败 / 499 = upgrade 断）。
    pub(crate) async fn log_terminal(
        &self,
        state: &Arc<ProxyState>,
        request_url: String,
        status_code: i32,
    ) {
        if !self.log_enabled {
            return;
        }
        upsert_connect_log(
            state,
            self,
            request_url,
            status_code,
            self.start.elapsed().as_millis() as i32,
        )
        .await;
    }
}

pub(crate) async fn upsert_connect_log(
    state: &Arc<ProxyState>,
    ctx: &ConnectLogCtx,
    request_url: String,
    status_code: i32,
    duration_ms: i32,
) {
    let msg = LogMsg::Connect {
        id: ctx.request_id.clone(),
        group_key: ctx.conn_group_key.clone(),
        group_name: ctx.group_name.clone(),
        platform_id: ctx.platform_id,
        request_url,
        status_code,
        duration_ms,
        blocked_reason: ctx.blocked_reason.to_string(),
        req_bytes: ctx.req_bytes,
        resp_bytes: ctx.resp_bytes,
    };
    // O6 非阻塞投递：Connect 消息本就只含元数据（无 body 列内容，无可降级项），队满即丢 + warn
    //（同 upsert_log 背压链第 2 档；正常消费速率下到不了这里）。
    try_send_log_msg(
        state,
        msg,
        || tracing::warn!(id = %ctx.request_id, "log queue full, connect log dropped"),
        || tracing::warn!(id = %ctx.request_id, "log writer channel closed, connect log dropped"),
    );
}

/// connect log 落库主逻辑（原 upsert_connect_log 函数体，现只在 writer 内单 writer 串行调用）。
///
/// 字段语义（PRD 锁）:
/// - `source_protocol`/`target_protocol` = `"http-connect"`（Logs 页区分隧道请求）
/// - `platform_id` = host 命中平台 else 0
/// - `request_url` = CONNECT target（`host:port`）
/// - `status_code` = 200（隧道建立成功）/ 502（上游连不上）/ 499（客户端断）
/// - tokens/cost = 0（P1 不解析 body）
/// - `blocked_reason` = `MITM_OPAQUE_REASON`（认证绑定 + 盲转未解密）或空串（普通盲转）
#[allow(clippy::too_many_arguments)]
async fn process_connect_log(
    state: &Arc<ProxyState>,
    id: String,
    group_key: String,
    group_name: String,
    platform_id: u64,
    request_url: String,
    status_code: i32,
    duration_ms: i32,
    blocked_reason: String,
    req_bytes: i64,
    resp_bytes: i64,
) {
    let now = aidog_db::now();
    // 是否认证绑定的盲转（mitm_opaque）——cols move 掉 blocked_reason 前先取。
    let is_opaque = blocked_reason == aidog_logs::MITM_OPAQUE_REASON;
    // ponytail: host 从 request_url（"host:port"）剥端口，IPv6 字面量不处理——CONNECT 目标
    // 几乎恒为域名/IPv4，真出现时 host 记原样可用。先取（下面 cols move 掉 request_url）。
    let host = if is_opaque {
        request_url
            .rsplit_once(':')
            .map(|(h, _)| h)
            .unwrap_or(&request_url)
            .to_string()
    } else {
        String::new()
    };
    let cols = aidog_logs::ProxyLogColumns {
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
        cache_write_tokens: 0,
        est_cost: 0.0,
        is_stream: 0,
        attempts: String::new(),
        retry_count: 0,
        blocked_by: String::new(),
        blocked_reason,
        created_at: now,
        updated_at: now,
        deleted_at: 0,
        done: 1,
        // CONNECT 隧道日志不经出站 body 构造 seam，无字段留痕（票 10）。
        field_trace: String::new(),
        // 一次性终态 INSERT，无后续节点，位掩码无消费方（O1）。
        raw_written_mask: 0,
        // CONNECT 隧道不携带 body 列内容，无降级一说（O6）。
        body_omitted: 0,
    };
    if let Err(e) = aidog_logs::insert_proxy_log_columns(&state.db, cols).await {
        tracing::warn!(error = %e, "connect log insert failed (non-fatal)");
        return;
    }
    // 认证绑定的盲转隧道（mitm_opaque）补一行 mitm_log 观测行（decrypted=false / body 恒空），
    // MITM 观测页才能看到全部走代理的请求；其余（普通盲转 / 502/499 早断）不产生行。
    if is_opaque {
        let row = aidog_logs::MitmLogInsert {
            group_name,
            host,
            path: String::new(),
            status_code,
            req_bytes,
            resp_bytes,
            decrypted: false,
            request_body: String::new(),
            response_body: String::new(),
            created_at: now,
        };
        if let Err(e) = aidog_logs::insert_mitm_log(&state.db, row).await {
            tracing::warn!(error = %e, "mitm opaque mitm_log insert failed (non-fatal)");
        }
    }
    // 通知前端 Platforms/Stats 刷新（platform_id 可能为 0，前端按需处理）。CONNECT 一次性终态
    // 写入，复用与 `process_upsert` 终态分支相同的 emit idiom（s4 proxy-hotpath-buffers）。
    emit_log_events(state, platform_id);
}

#[cfg(test)]
#[path = "test_log.rs"]
mod test_log;
