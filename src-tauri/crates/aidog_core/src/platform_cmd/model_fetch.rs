use crate::gateway;
use gateway::models::*;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

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

fn models_request_headers_log(protocol: &Protocol) -> String {
    let headers = match protocol {
        Protocol::Anthropic => serde_json::json!({
            "x-api-key": "[REDACTED]",
            "anthropic-version": "2023-06-01",
        }),
        _ => serde_json::json!({
            "authorization": "[REDACTED]",
            "api-key": "[REDACTED]",
        }),
    };
    headers.to_string()
}

fn models_response_headers_log(headers: &reqwest::header::HeaderMap) -> String {
    let values = headers
        .iter()
        .filter_map(|(name, value)| {
            value.to_str().ok().map(|value| {
                (
                    name.as_str().to_string(),
                    serde_json::Value::String(if matches!(
                        name.as_str().to_ascii_lowercase().as_str(),
                        "authorization"
                            | "api-key"
                            | "x-api-key"
                            | "x-goog-api-key"
                            | "cookie"
                            | "set-cookie"
                    ) {
                        "[REDACTED]".to_string()
                    } else {
                        value.to_string()
                    }),
                )
            })
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::Value::Object(values).to_string()
}

fn redact_models_url(url: &str) -> String {
    let Ok(mut parsed) = reqwest::Url::parse(url) else {
        return url
            .split_once('?')
            .map_or_else(|| url.to_string(), |(base, _)| format!("{base}?[REDACTED]"));
    };
    if parsed.query().is_none() {
        return parsed.to_string();
    }
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(name, value)| {
            let lower = name.to_ascii_lowercase();
            let sensitive = lower.contains("key")
                || lower.contains("token")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("signature");
            (
                name.into_owned(),
                if sensitive {
                    "[REDACTED]".into()
                } else {
                    value.into_owned()
                },
            )
        })
        .collect();
    parsed.query_pairs_mut().clear().extend_pairs(pairs);
    parsed.to_string()
}

crate::tauri_command! {
pub async fn platform_fetch_models(
    protocol: Protocol,
    base_url: String,
    api_key: String) -> Result<Vec<String>, FetchModelsError> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "platform_fetch_models", protocol = ?protocol, base_url = %base_url, api_key = "[REDACTED]", "command invoked");
    let db_arc = Arc::new(db.clone());
    let client = gateway::http_client::build_http_client_system(&db_arc, 30, 10).await;

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = aidog_db::now();
    let target_protocol = format!("{:?}", protocol).to_lowercase();

    // fetch-models 日志构造器（复用 model_test 标记模式：source_protocol 约定串 + platform_id=0）
    let make_log = |upstream_status: i32,
                    user_status: i32,
                    response_headers: &str,
                    body: &str,
                    log_url: &str|
     -> gateway::models::ProxyLog {
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
            upstream_request_headers: models_request_headers_log(&protocol),
            upstream_request_body: String::new(),
            response_body: body.into(),
            request_url: "/fetch-models".into(),
            upstream_request_url: redact_models_url(log_url),
            upstream_response_headers: response_headers.into(),
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
            done: true,
            // 不经代理出站 body 构造 seam，无字段留痕（票 10）。
            field_trace: String::new(),
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
            if let Err(le) = aidog_logs::upsert_proxy_log(
                db,
                make_log(0, 502, "{}", &format!("upstream error: {e}"), &url),
            )
            .await
            {
                tracing::warn!(command = "platform_fetch_models", error = %le, "persist fetch-models log failed");
            }
            return Err(FetchModelsError::from_status(
                0,
                format!("fetch models: {e}"),
            ));
        }
    };
    let status = resp.status();
    let response_headers = models_response_headers_log(resp.headers());
    let body = match resp.text().await {
        Ok(body) => body,
        Err(e) => {
            tracing::error!(url = %url, "read body failed: {e}");
            if let Err(le) = aidog_logs::upsert_proxy_log(
                db,
                make_log(
                    status.as_u16() as i32,
                    502,
                    &response_headers,
                    &format!("read body: {e}"),
                    &url,
                ),
            )
            .await
            {
                tracing::warn!(command = "platform_fetch_models", error = %le, "persist fetch-models log failed");
            }
            return Err(FetchModelsError::from_status(0, format!("read body: {e}")));
        }
    };
    tracing::info!(url = %url, %status, "fetch models response status");
    tracing::debug!(url = %url, body = %gateway::log_util::log_body_preview(&body), "fetch models response body");
    // 记录 fetch-models 请求到 proxy_log（成功响应，保留原文便于排查）
    let upstream_status = status.as_u16() as i32;
    if let Err(le) =
        aidog_logs::upsert_proxy_log(db, make_log(upstream_status, upstream_status, &response_headers, &body, &url))
            .await
    {
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
    let resp: Value = serde_json::from_str::<Value>(&body).map_err(|e| {
        tracing::error!(
            "parse response failed: {e}, body={}",
            &body[..body.len().min(500)]
        );
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_request_log_headers_match_protocol_without_credentials() {
        let anthropic = models_request_headers_log(&Protocol::Anthropic);
        assert!(anthropic.contains("x-api-key"));
        assert!(anthropic.contains("anthropic-version"));
        assert!(anthropic.contains("[REDACTED]"));

        let openai = models_request_headers_log(&Protocol::OpenAI);
        assert!(openai.contains("authorization"));
        assert!(openai.contains("api-key"));
        assert!(openai.contains("[REDACTED]"));

        let mut response = reqwest::header::HeaderMap::new();
        response.insert("content-type", "application/json".parse().unwrap());
        response.insert("set-cookie", "credential".parse().unwrap());
        let response = models_response_headers_log(&response);
        assert!(response.contains("content-type"));
        assert!(response.contains("[REDACTED]"));
        assert!(!response.contains("credential"));

        let url = redact_models_url("https://example.invalid/models?api_key=credential&region=eu");
        assert!(url.contains("region=eu"));
        assert!(!url.contains("credential"));
    }

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
        let e = FetchModelsError::Auth {
            code: 401,
            message: "invalid".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "Auth");
        assert_eq!(v["code"], 401);
        assert_eq!(v["message"], "invalid");
    }
}
