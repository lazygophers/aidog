use super::*;

// ─── /api/notify（N1 — 系统通知端点）────────────────────────

/// 通知端点请求体：`{event?, type?, content?, vars?}`。
/// - `event`（N2）：CC hook 事件名（通用脚本 aidog-notify.py 发；后端按 per_event 解析 type+模板）。
/// - `type`（兼容旧路径 / Codex complete 脚本）：通知类型字面量，未知 → TaskComplete。
///   event 命中 per_event 时优先于 type。两者都缺省 → type 空串 → 兜底 TaskComplete。
#[derive(serde::Deserialize)]
struct NotifyReq {
    #[serde(default)]
    event: Option<String>,
    #[serde(rename = "type", default)]
    notif_type: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    vars: std::collections::HashMap<String, String>,
}

/// 通知端点 — localhost-only，鉴权 `Authorization: Bearer <group_key>`（仿 /api/group-info）。
/// hook 脚本调用此端点触发通知。body `{type, content?, vars?}`。
/// 鉴权用的 group_key 校验存在性，并作为 `{group}` 变量回填（脚本未显式带 group 时）。
pub(crate) async fn handle_notify(
    state: AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    let span = tracing::info_span!("notify", trace_id = %crate::logging::new_trace_id());
    handle_notify_inner(state, headers, body).instrument(span).await
}

async fn handle_notify_inner(
    AxumState(state): AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    // Bearer group_key 鉴权
    let group_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let group_key = match group_key {
        Some(n) if !n.is_empty() => n,
        _ => {
            let mut r = StatusCode::UNAUTHORIZED.into_response();
            inject_trace_header(&mut r);
            return r;
        }
    };
    // 校验分组存在（防任意 token 触发；不存在则拒绝）；同时取显示名供脚本 {group} 渲染。
    let group_name = match super::db::list_groups(&state.db).await {
        Ok(groups) => match groups.iter().find(|g| g.group_key == group_key) {
            Some(g) => g.name.clone(),
            None => {
                tracing::debug!(group = %group_key, "notify: group not found, reject");
                let mut r = StatusCode::UNAUTHORIZED.into_response();
                inject_trace_header(&mut r);
                return r;
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "notify: list_groups failed");
            let mut r = StatusCode::INTERNAL_SERVER_ERROR.into_response();
            inject_trace_header(&mut r);
            return r;
        }
    };

    let req: NotifyReq = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "notify: invalid body");
            let mut r = (StatusCode::BAD_REQUEST, format!("invalid body: {e}")).into_response();
            inject_trace_header(&mut r);
            return r;
        }
    };

    // 注入内置变量：{group} 默认取鉴权分组的显示名（name，非 token group_key）；{time} 默认当前本地时间（脚本可覆盖）。
    let mut vars = req.vars;
    vars.entry("group".to_string()).or_insert_with(|| group_name.clone());
    vars.entry("time".to_string()).or_insert_with(|| {
        chrono::Local::now().format("%H:%M:%S").to_string()
    });

    let result = super::notification::dispatch(
        &state.db,
        state.app.as_ref(),
        req.event.as_deref(),
        &req.notif_type,
        req.content.as_deref(),
        &vars,
    )
    .await;

    tracing::debug!(
        event = ?req.event,
        notif_type = %req.notif_type,
        dispatched = result.dispatched,
        inbox = result.inbox,
        popup = result.popup,
        tts = result.tts,
        "notify dispatched"
    );

    let mut r = (StatusCode::OK, Json(result)).into_response();
    inject_trace_header(&mut r);
    r
}
