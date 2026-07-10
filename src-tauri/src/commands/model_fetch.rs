use aidog_core::gateway::{self, db::{self, Db}};
#[allow(unused_imports)]
use aidog_core::logging;
#[allow(unused_imports)]
use gateway::models::*;
#[allow(unused_imports)]
use tauri::State;
#[allow(unused_imports)]
use serde_json::Value;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use tauri::Manager;
use serde::Serialize;

/// fetch-models 失败的结构化错误。前端按 `kind` 分流：
/// - `Auth`(401/403) → 鉴权问题，立即 break 回退链 + 鉴权专用文案
/// - `NotFound`(404) → 端点不存在，continue 试下一协议
/// - `Other`(其余) → 网络错 / 5xx 等，continue 试下一协议
///
/// tag="kind" 内部表示：`{"kind":"Auth","code":401,"message":"..."}`。
/// Tauri command Err 走 serde_json 序列化，enum 需 derive(Serialize)。
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum FetchModelsError {
    Auth { code: u16, message: String },
    NotFound { code: u16, message: String },
    Other { code: u16, message: String },
}

impl FetchModelsError {
    /// 按 HTTP status code 映射变体。0 = 无 status（网络错 / 读 body 失败等）。
    fn from_status(code: u16, message: impl Into<String>) -> Self {
        let message = message.into();
        match code {
            401 | 403 => FetchModelsError::Auth { code, message },
            404 => FetchModelsError::NotFound { code, message },
            _ => FetchModelsError::Other { code, message },
        }
    }
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_fetch_models(
    protocol: Protocol,
    base_url: String,
    api_key: String,
    db: State<'_, Db>,
) -> Result<Vec<String>, FetchModelsError> {
    tracing::debug!(command = "platform_fetch_models", protocol = ?protocol, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let db_arc = Arc::new(db.inner().clone());
    let client = gateway::http_client::build_http_client_system(&db_arc, 30, 10).await;

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = gateway::db::now();
    let target_protocol = format!("{:?}", protocol).to_lowercase();

    // fetch-models 日志构造器（复用 model_test 标记模式：source_protocol 约定串 + platform_id=0）
    let make_log = |upstream_status: i32, user_status: i32, body: &str, log_url: &str| -> gateway::models::ProxyLog {
        gateway::models::ProxyLog {
            id: request_id.clone(),
            group_key: "[fetch-models]".into(),
            model: String::new(),
            actual_model: String::new(),
            source_protocol: "fetch-models".into(),
            target_protocol: target_protocol.clone(),
            platform_id: 0,
            request_headers: r#"{"source":"fetch-models"}"#.into(),
            request_body: String::new(),
            upstream_request_headers: String::new(),
            upstream_request_body: String::new(),
            response_body: body.into(),
            request_url: "/fetch-models".into(),
            upstream_request_url: log_url.to_string(),
            upstream_response_headers: String::new(),
            upstream_status_code: upstream_status,
            user_response_headers: r#"{"content-type":"application/json"}"#.to_string(),
            user_response_body: body.into(),
            status_code: user_status,
            duration_ms: start.elapsed().as_millis() as i32,
            input_tokens: 0,
            output_tokens: 0,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: false,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at,
            updated_at: created_at,
            deleted_at: 0,
        }
    };

    // Mock / Claude Code 透传平台无真实上游模型列表，不拉取模型
    if matches!(protocol, Protocol::Mock | Protocol::ClaudeCode) {
        return Ok(Vec::new());
    }

    // URL + 鉴权与 proxy.rs models 端点 relay 单一事实源（build_models_url / apply_models_auth）。
    // OpenCode Zen：api_key 留空时注入 $opencode（与 proxy 路径一致；/v1/models 无 auth 亦可）。
    let is_zen = matches!(protocol, Protocol::OpenCodeZen)
        || base_url.to_lowercase().contains("opencode.ai/zen");
    let api_key = gateway::proxy::opencode_zen_fallback(&api_key, is_zen);
    let url = gateway::proxy::build_models_url(&protocol, &base_url);
    let rb = gateway::proxy::apply_models_auth(client.get(&url), &protocol, &api_key);
    tracing::info!(method = "GET", url = %url, "fetch models request");
    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("fetch models request failed: {e}");
            if let Err(le) = db::upsert_proxy_log(&db, make_log(0, 502, &format!("upstream error: {e}"), &url)).await {
                tracing::warn!(command = "platform_fetch_models", error = %le, "persist fetch-models log failed");
            }
            return Err(FetchModelsError::from_status(0, format!("fetch models: {e}")));
        }
    };
    let status = resp.status();
    let body = resp.text().await.map_err(|e| {
        tracing::error!(url = %url, "read body failed: {e}");
        FetchModelsError::from_status(0, format!("read body: {e}"))
    })?;
    tracing::info!(url = %url, %status, "fetch models response status");
    tracing::debug!(url = %url, body = %gateway::log_util::log_body_preview(&body), "fetch models response body");
    // 记录 fetch-models 请求到 proxy_log（成功响应，保留原文便于排查）
    let upstream_status = status.as_u16() as i32;
    if let Err(le) = db::upsert_proxy_log(&db, make_log(upstream_status, upstream_status, &body, &url)).await {
        tracing::warn!(command = "platform_fetch_models", error = %le, "persist fetch-models log failed");
    }
    // 🔴 status code 参与控制流：非 2xx 按 code 映射错误变体（401/403 → Auth, 404 → NotFound, 其余 → Other）。
    // 之前 401 错误体 {"error":{...}} 是合法 JSON，解析「成功」但无 data → unwrap_or_default() 返空 Vec，
    // 与 404 表现完全相同，用户无法区分鉴权失败与端点不存在。这里把 status 透传给前端做分流。
    if !status.is_success() {
        let code = status.as_u16();
        tracing::warn!(url = %url, %code, "fetch models non-success status");
        return Err(FetchModelsError::from_status(code, body));
    }
    let resp: Value = serde_json::from_str::<Value>(&body)
        .map_err(|e| {
            tracing::error!("parse response failed: {e}, body={}", &body[..body.len().min(500)]);
            FetchModelsError::from_status(status.as_u16(), format!("parse response: {e}"))
        })?;

    // 解析 {"data": [{"id": "..."}, ...]} 格式
    let models = resp
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            let mut ids: Vec<String> = arr
                .iter()
                .filter_map(|item| item.get("id").and_then(|v| v.as_str()).map(String::from))
                .collect();
            ids.sort();
            ids
        })
        .unwrap_or_default();

    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_variant_for_401_403() {
        let e = FetchModelsError::from_status(401, "bad key");
        match e {
            FetchModelsError::Auth { code, message } => {
                assert_eq!(code, 401);
                assert_eq!(message, "bad key");
            }
            other => panic!("expected Auth, got {other:?}"),
        }
        assert!(matches!(
            FetchModelsError::from_status(403, "forbidden"),
            FetchModelsError::Auth { .. }
        ));
    }

    #[test]
    fn notfound_variant_for_404() {
        let e = FetchModelsError::from_status(404, "no route");
        match e {
            FetchModelsError::NotFound { code, message } => {
                assert_eq!(code, 404);
                assert_eq!(message, "no route");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn other_variant_for_rest_including_zero() {
        assert!(matches!(
            FetchModelsError::from_status(500, "boom"),
            FetchModelsError::Other { .. }
        ));
        assert!(matches!(
            FetchModelsError::from_status(0, "network"),
            FetchModelsError::Other { .. }
        ));
    }

    #[test]
    fn serialize_tag_kind_for_frontend_dispatch() {
        let e = FetchModelsError::Auth { code: 401, message: "invalid".into() };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "Auth");
        assert_eq!(v["code"], 401);
        assert_eq!(v["message"], "invalid");
    }
}
