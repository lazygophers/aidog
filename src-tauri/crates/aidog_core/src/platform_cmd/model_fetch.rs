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

/// 镜像实发头（与 apply_client_headers 同一 simulation 配置，api_key 全脱敏）。
/// GET 无 body，剔 build_upstream_headers 固定塞的 Content-Type。UA 一并入日志。
fn models_request_headers_log(client_type: &ClientType, protocol: &Protocol) -> String {
    let empty = axum::http::HeaderMap::new();
    let map: serde_json::Map<String, serde_json::Value> = gateway::proxy::build_upstream_headers(
        client_type,
        protocol,
        "",
        &empty,
    )
    .into_iter()
    .filter(|(k, _)| !k.eq_ignore_ascii_case("content-type"))
    .map(|(k, v)| (k, serde_json::Value::String(v)))
    .collect();
    serde_json::Value::Object(map).to_string()
}

fn models_response_headers_log(headers: &reqwest::header::HeaderMap) -> String {
    let values = headers
        .iter()
        .filter_map(|(name, value)| {
            value.to_str().ok().map(|value| {
                (
                    name.as_str().to_string(),
                    serde_json::Value::String(
                        if matches!(
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
                        },
                    ),
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

/// 多行 api_key（创建态批量输入 `k1\nk2`）→ 取首行并 trim。
/// 整串塞 header 会被 reqwest 以 illegal header value 拒掉（「builder error」）。
fn first_api_key(api_key: &str) -> &str {
    api_key.lines().next().unwrap_or("").trim()
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
    // 客户端模拟（2026-10-04）：client_type 按协议派生（anthropic→claude_code、openai 系→
    // codex_tui，registry::derive_client_type 与 proxy 端点缺省派生同源），UA + auth 全套
    // 走 client-types.json simulation —— 用 openai base_url 拉模型列表就模拟 openai 客户端。
    // protocol 是平台别名（newapi/deepseek/...），走平台级派生而非协议级。
    let client_type = aidog_db::registry::derive_client_type_for_platform(&protocol.wire_str());

    // fetch-models 日志构造器（复用 model_test 标记模式：source_protocol 约定串 + platform_id=0）
    let make_log = |upstream_status: i32,
                    user_status: i32,
                    response_headers: &str,
                    body: &str,
                    log_url: &str|
     -> gateway::models::ProxyLog {
        gateway::models::ProxyLog {
            id: request_id.clone(),
            target_protocol: target_protocol.clone(),
            request_headers: r#"{"source":"fetch-models"}"#.into(),
            upstream_request_headers: models_request_headers_log(&client_type, &protocol),
            response_body: body.into(),
            request_url: "/fetch-models".into(),
            upstream_request_url: redact_models_url(log_url),
            upstream_response_headers: response_headers.into(),
            upstream_status_code: upstream_status,
            user_response_body: body.into(),
            status_code: user_status,
            duration_ms: start.elapsed().as_millis() as i32,
            created_at,
            updated_at: created_at,
            ..Default::default()
        }
        .out_of_band("[fetch-models]", "fetch-models")
        // 不经代理出站 body 构造 seam，无字段留痕（票 10）。
    };

    // Mock / Claude Code 透传平台无真实上游模型列表，不拉取模型
    if matches!(protocol, Protocol::Mock | Protocol::ClaudeCode) {
        return Ok(Vec::new());
    }

    // URL + 鉴权与 proxy.rs models 端点 relay 单一事实源（build_models_url / apply_models_auth）。
    // 创建态批量 key 时 api_key 是多行文本（`k1\nk2`）——整串塞 header 会被 reqwest
    // 以 illegal header value 拒掉（症状「fetch models: builder error」）。取首行 +
    // trim：拉模型列表用第一个 key 探测即可（单 key 行为不变）。
    let api_key = first_api_key(&api_key).to_string();

    // OpenCode Zen：api_key 留空时注入 $opencode（与 proxy 路径一致；/v1/models 无 auth 亦可）。
    let is_zen = gateway::proxy::is_opencode_zen(&protocol, &base_url, None);
    let api_key = gateway::proxy::opencode_zen_fallback(&api_key, is_zen);
    let url = gateway::proxy::build_models_url(&protocol, &base_url);
    let rb = gateway::proxy::apply_client_headers(client.get(&url), &client_type, &protocol, &api_key);
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
        let ct_anthropic = aidog_db::registry::derive_client_type_for_platform("anthropic");
        let anthropic = models_request_headers_log(&ct_anthropic, &Protocol::Anthropic);
        assert!(anthropic.contains("x-api-key"));
        assert!(anthropic.contains("anthropic-version"));
        assert!(anthropic.contains("claude-cli/"));
        assert!(anthropic.contains("[REDACTED]"));

        let ct_openai = aidog_db::registry::derive_client_type_for_platform("openai");
        let openai = models_request_headers_log(&ct_openai, &Protocol::OpenAI);
        assert!(openai.to_lowercase().contains("authorization"));
        assert!(openai.contains("api-key"));
        assert!(openai.contains("Codex/"));
        assert!(openai.contains("[REDACTED]"));
        // GET 无 body，不该带 Content-Type
        assert!(!openai.to_lowercase().contains("content-type"));

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
    fn first_api_key_takes_first_line_and_trims() {
        assert_eq!(first_api_key("k1\nk2"), "k1");
        assert_eq!(first_api_key("  k1  \n k2"), "k1");
        assert_eq!(first_api_key("solo"), "solo");
        assert_eq!(first_api_key("  "), "");
        assert_eq!(first_api_key(""), "");
    }

    #[test]
    fn single_line_key_produces_legal_header_multiline_does_not() {
        // 症状复现：多行值是非法 header value（reqwest builder error 的来源）。
        let req = reqwest::Client::new()
            .get("https://example.invalid/v1/models")
            .header("Authorization", format!("Bearer {}", first_api_key("k1\nk2")));
        assert!(req.build().is_ok(), "first line must build a legal header");
        let bad = reqwest::Client::new()
            .get("https://example.invalid/v1/models")
            .header("Authorization", "Bearer k1\nk2");
        assert!(bad.build().is_err(), "raw multiline key is the reported bug");
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
