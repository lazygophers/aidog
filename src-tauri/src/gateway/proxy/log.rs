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

/// Upsert a proxy log entry; silently ignore errors.
/// Respects ProxyLogSettings: if logging disabled, does nothing;
/// if user/upstream recording disabled, clears those fields before writing.
pub(crate) async fn upsert_log(state: &Arc<ProxyState>, log: &ProxyLog, settings: &ProxyLogSettings) {
    // ── 聚合统计写入（解耦于日志开关）──
    // 必须在 `!settings.enabled` 早退之前：关日志时统计仍需写。仅终态请求计入
    // （status!=0 且非流式占位 "[stream]"，与下方 is_terminal 同判定，避免占位/中间节点重复计）。
    // est_cost：log 已带则用；否则（关日志路径不会经下方计算）就地走 calc_est_cost 回退链。
    // 失败非致命：warn 不中断请求。eff_pid 回溯在 upsert_stats_agg 的 SQL 内做。
    //
    // 去重：upsert_log 在单个请求生命周期内被多次调用（insert + 多次 update + 流式 flush），
    // 终态后每次调用 gate 仍为真。HashSet::insert 返回 false 表示该 id 已聚合过 → 跳过，
    // 保证每请求只 +1 一次（id 在 remove_log_snapshot 清理，见下）。
    let first_agg = log.status_code != 0
        && log.response_body != "[stream]"
        && agg_mark_first(state, &log.id);
    if first_agg {
        let mut est_cost = log.est_cost;
        if est_cost == 0.0 && (log.input_tokens > 0 || log.output_tokens > 0) {
            let model_name = if log.actual_model.is_empty() { &log.model } else { &log.actual_model };
            let platform_type = super::db::get_platform(&state.db, log.platform_id)
                .await
                .ok()
                .flatten()
                .map(|p| serde_json::to_string(&p.platform_type).unwrap_or_default().trim_matches('"').to_string())
                .unwrap_or_default();
            est_cost = super::db::calc_est_cost(
                &state.db,
                model_name,
                &platform_type,
                log.input_tokens,
                log.output_tokens,
                log.cache_tokens,
            )
            .await;
        }
        let agg_input = super::db::StatsAggInput {
            created_at: log.created_at,
            model: if log.actual_model.is_empty() { log.model.clone() } else { log.actual_model.clone() },
            group_key: log.group_key.clone(),
            platform_id: log.platform_id as i64,
            status_code: log.status_code,
            input_tokens: log.input_tokens as i64,
            output_tokens: log.output_tokens as i64,
            cache_tokens: log.cache_tokens as i64,
            est_cost,
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
            est_cost = est_cost,
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

    // Calculate est_cost from model_price if tokens are present（语义同旧路径，作用于列快照）
    if cols.est_cost == 0.0 && (cols.input_tokens > 0 || cols.output_tokens > 0) {
        let model_name = if log.actual_model.is_empty() { &log.model } else { &log.actual_model };
        // best-effort 取平台主类型的 serde 裸名（如 "deepseek"）以启用 pricing[platform_type] override；
        // 拿不到则传 ""，calc_est_cost 的 fallback 回退链仍保证非 0。
        let platform_type = super::db::get_platform(&state.db, log.platform_id)
            .await
            .ok()
            .flatten()
            .map(|p| serde_json::to_string(&p.platform_type).unwrap_or_default().trim_matches('"').to_string())
            .unwrap_or_default();
        cols.est_cost = super::db::calc_est_cost(
            &state.db,
            model_name,
            &platform_type,
            cols.input_tokens,
            cols.output_tokens,
            cols.cache_tokens,
        )
        .await;
    }

    let id = cols.id.clone();
    let platform_id = log.platform_id;
    // 终态判定：有真实 HTTP 状态(status!=0)。唯一例外是流式占位写（response_body=="[stream]"，
    // 终态由 guard.flush 后显式 remove，不在此误删以免 guard 再 INSERT 撞主键）。
    // 覆盖流式请求在占位前就出错(如 502)的分支，避免快照泄漏。
    let is_terminal = cols.status_code != 0 && cols.response_body != "[stream]";

    // 取上一快照决定 INSERT(首节点) 还是 部分列 UPDATE(后续节点)。
    let prev = {
        let map = state.log_snapshots.lock().unwrap();
        map.get(&id).cloned()
    };
    let write_ok = match prev {
        None => {
            // 首节点：建行。成功后存快照供后续 diff。
            let ok = super::db::insert_proxy_log_columns(&state.db, cols.clone()).await.is_ok();
            if ok {
                state.log_snapshots.lock().unwrap().insert(id.clone(), cols);
            }
            ok
        }
        Some(prev) => {
            // 后续节点：仅 UPDATE 变化列；成功后刷新快照。
            let ok = super::db::update_proxy_log_columns(&state.db, cols.clone(), &prev).await.is_ok();
            if ok {
                state.log_snapshots.lock().unwrap().insert(id.clone(), cols);
            }
            ok
        }
    };

    // 终态写完移除快照，防 in-flight map 无限增长（流式占位写除外，由 guard 显式移除）。
    if is_terminal {
        remove_log_snapshot(state, &id);
    }

    if write_ok {
        // 日志写库成功后通知前端三页（Platforms/Groups/Stats）实时刷新统计。
        // 同时通知托盘刷新今日统计（请求数、Token、费用等）。
        // app handle 为 None（无 GUI 上下文）时安全跳过，不影响代理逻辑。
        if let Some(app) = &state.app {
            use tauri::Emitter;
            let _ = app.emit("proxy-log-updated", platform_id);
            let _ = app.emit("tray-refresh", ());
        }
    }
}

/// 移除某请求 id 的列快照（终态写入后调用，防止 in-flight 快照 map 无限增长）。
/// 流式 guard 终态 flush / 非流式终态返回前调用。重复调用安全（不存在即 no-op）。
/// 注意：不在此清 agg_done——终态 upsert_log 会被反复调用（remove 后下次 prev=None 又走终态），
/// 在此清会破坏去重；agg_done 自带 FIFO 容量上限，无需按请求清理。
pub(crate) fn remove_log_snapshot(state: &Arc<ProxyState>, id: &str) {
    state.log_snapshots.lock().unwrap().remove(id);
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
    (
        StatusCode::FORBIDDEN,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
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
    // serde rename 裸名（去掉 to_string 的引号），与 pricing JSON key 一致
    let ptype = serde_json::to_string(platform_type)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
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

#[cfg(test)]
#[path = "test_log.rs"]
mod test_log;
