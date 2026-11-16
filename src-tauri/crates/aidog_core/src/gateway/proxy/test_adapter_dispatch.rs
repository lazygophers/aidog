//! 端点来源回归护栏（issue 68 验收⑤）：出站永远按**用户平台配置**的端点
//! (protocol, base_url) 出站，registry 内置端点不参与运行时。
//! 端点读取是既有行为（运行时只读 DB platform.endpoints），本模块是回归护栏——
//! 旧码已绿，不构成新行为先红证据（测试纪律按 parallel-implement.md §22 分类）。

use super::*;
use aidog_middleware::MiddlewareEngine;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, PlatformEndpoint, Protocol};
use aidog_db::test_support::test_db;
use axum::body::Body;
use axum::http::Request as HttpRequest;
use aidog_db::models::UpdatePlatform;
use std::sync::{Arc, Mutex};

/// 起一个记录 (path, body) 的本地假上游，返回 (base_url, 记录句柄)。
async fn spawn_recording_upstream(resp_body: &'static str) -> (String, Arc<Mutex<Vec<(String, String)>>>) {
    let hits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = hits.clone();
    let resp_body = resp_body.to_string();
    let app = axum::Router::new().fallback(|uri: axum::http::Uri, body: String| async move {
        sink.lock().unwrap().push((uri.path().to_string(), body));
        (
            axum::http::StatusCode::OK,
            [("content-type", "application/json")],
            resp_body,
        )
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (format!("http://{addr}"), hits)
}

/// 建一个带用户端点的平台并挂进分组。
async fn setup_user_endpoint_group(
    state: &Arc<ProxyState>,
    gk: &str,
    ep_protocol: Protocol,
    base_url: &str,
) {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "stub".into(),
            platform_type: Protocol::OpenAI,
            base_url: base_url.to_string(),
            api_key: "sk-up".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: Some(vec![PlatformEndpoint {
                protocol: ep_protocol,
                base_url: base_url.to_string(),
                client_type: "default".to_string(),
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
}

const OPENAI_OK: &str =
    r#"{"id":"chatcmpl_1","object":"chat.completion","model":"gpt-x","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}"#;
const ANTHROPIC_OK: &str = r#"{"id":"msg_1","type":"message","role":"assistant","model":"claude-3","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":3}}"#;

/// 用户端点 = openai wire：入站 anthropic /v1/messages → 出站按用户配置的
/// openai 端点协议转 openai 形状、打用户 base_url 的 /chat/completions。
#[tokio::test]
async fn outbound_follows_user_endpoint_protocol_and_base_url() {
    let (upstream, hits) = spawn_recording_upstream(OPENAI_OK).await;
    let state = make_state(test_db().await).await;
    setup_user_endpoint_group(&state, "gkuep", Protocol::OpenAI, &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer gkuep")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-3","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;

    let hits = hits.lock().unwrap();
    assert_eq!(hits.len(), 1, "上游应恰好收到一次请求: {hits:?}");
    let (path, body) = &hits[0];
    assert_eq!(path, "/chat/completions", "出站路径应按用户端点协议派生");
    assert!(body.contains("\"messages\""), "应转换成 openai 请求体: {body}");
    assert!(
        !body.contains("\"stop_reason\""),
        "不应是 anthropic 原体透传: {body}"
    );
}

/// 用户改端点协议（openai → anthropic）：同一入站 openai 请求出站按新协议转
/// anthropic 形状、打 /v1/messages——端点协议是运行时唯一真值。
#[tokio::test]
async fn outbound_follows_endpoint_protocol_after_user_change() {
    let (upstream, hits) = spawn_recording_upstream(ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_user_endpoint_group(&state, "gkchg", Protocol::OpenAI, &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("authorization", "Bearer gkchg")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"gpt-x","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;
    assert_eq!(hits.lock().unwrap().len(), 1);

    // 用户把端点协议改成 anthropic（update_platform 全量同步 endpoints）
    let plats = aidog_db::list_platforms(&state.db).await.unwrap();
    let pid = plats.iter().find(|p| p.name == "stub").unwrap().id;
    aidog_db::update_platform(
        &state.db,
        UpdatePlatform {
            id: pid,
            endpoints: Some(vec![PlatformEndpoint {
                protocol: Protocol::Anthropic,
                base_url: upstream.clone(),
                client_type: "default".to_string(),
                coding_plan: false,
            }]),
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("authorization", "Bearer gkchg")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"gpt-x","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;

    let hits = hits.lock().unwrap();
    assert_eq!(hits.len(), 2, "两次请求都应打到假上游: {hits:?}");
    let (path, body) = &hits[1];
    assert_eq!(path, "/v1/messages", "改端点协议后出站应按新协议派生路径");
    assert!(
        body.contains("\"system\"") || body.contains("\"stop_reason\"") || body.contains("\"max_tokens\""),
        "出站体应按 anthropic wire 构造: {body}"
    );
}

/// GLM 家族四个成员 × 端点协议，入站带 aidog 扩展 `disable_thinking: true`，返回上游收到的 body。
async fn glm_outbound_body(platform_type: Protocol, ep: Protocol, gk: &str) -> serde_json::Value {
    outbound_body_for_model(platform_type, ep, gk, "glm-test").await
}

/// 同 `glm_outbound_body`，请求模型名可指定（用于命中 registry 条目的 `thinking_toggleable`）。
async fn outbound_body_for_model(platform_type: Protocol, ep: Protocol, gk: &str, model: &str) -> serde_json::Value {
    let req = serde_json::json!({"model": model, "max_tokens": 32, "disable_thinking": true,
        "messages": [{"role": "user", "content": "hi"}]});
    outbound_body_with(platform_type, ep, gk, req).await
}

/// 同 `outbound_body_for_model`，入站请求体整体可指定。
async fn outbound_body_with(platform_type: Protocol, ep: Protocol, gk: &str, req_body: serde_json::Value) -> serde_json::Value {
    let resp_ok = if ep == Protocol::Anthropic { ANTHROPIC_OK } else { OPENAI_OK };
    let (upstream, hits) = spawn_recording_upstream(resp_ok).await;
    let state = make_state(test_db().await).await;
    // 厂商直连协议建库端点被锁成 preset 域名，用 fixture 直写 DB 指向本地假上游
    let pid = aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        platform_type.clone(),
        &upstream,
        std::slice::from_ref(&ep),
    )
    .await;
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: pid,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(req_body.to_string()))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    let status = resp.status();
    let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{platform_type:?}/{ep:?}: {}", String::from_utf8_lossy(&resp_body));
    flush_log_queue(&state).await;
    let hits = hits.lock().unwrap();
    assert_eq!(hits.len(), 1, "上游应恰好收到一次请求: {hits:?}");
    serde_json::from_str(&hits[0].1).unwrap()
}

const GLM_FAMILY: [(Protocol, &str); 4] = [
    (Protocol::Glm, "gkglm"),
    (Protocol::GlmEn, "gkglmen"),
    (Protocol::GlmCoding, "gkglmcp"),
    (Protocol::GlmCodingEn, "gkglmcpen"),
];

/// issue 75：GLM 家族 openai 端点禁思考写官方参数 `thinking.type=disabled`
/// （docs.bigmodel.cn / docs.z.ai thinking-mode），不写 vLLM 式 `chat_template_kwargs`。
#[tokio::test]
async fn glm_family_openai_endpoint_disables_thinking_with_official_param() {
    for (pt, gk) in GLM_FAMILY {
        let body = glm_outbound_body(pt.clone(), Protocol::OpenAI, gk).await;
        assert_eq!(
            body.get("thinking"),
            Some(&serde_json::json!({"type": "disabled"})),
            "{pt:?} openai 端点应写 thinking.type=disabled: {body}"
        );
        assert!(
            body.get("chat_template_kwargs").is_none(),
            "{pt:?} openai 端点不应写 chat_template_kwargs: {body}"
        );
        assert!(body.get("disable_thinking").is_none(), "{pt:?} 私有字段应剥除: {body}");
    }
}

/// issue 75：GLM 家族 anthropic 端点禁思考与基座一致（`thinking.type=disabled`）。
#[tokio::test]
async fn glm_family_anthropic_endpoint_disables_thinking() {
    for (pt, gk) in GLM_FAMILY {
        let body = glm_outbound_body(pt.clone(), Protocol::Anthropic, gk).await;
        assert_eq!(
            body.get("thinking"),
            Some(&serde_json::json!({"type": "disabled"})),
            "{pt:?} anthropic 端点应写 thinking.type=disabled: {body}"
        );
        assert!(body.get("disable_thinking").is_none(), "{pt:?} 私有字段应剥除: {body}");
    }
}

// ══ issue 113：GLM（智谱）成员 `glm` 逐端点协议护栏（请求转换 / 非流式 / 流式 / 错误 / usage）══
// 官方出处（核对日期 2026-11-16）：
// - 对话补全 https://docs.bigmodel.cn/api-reference/%E6%A8%A1%E5%9E%8B-api/%E5%AF%B9%E8%AF%9D%E8%A1%A5%E5%85%A8
//   （usage.prompt_tokens_details.cached_tokens、delta.reasoning_content、流结束 `data: [DONE]`）
// - 错误码 https://docs.bigmodel.cn/cn/api/api-code
//   （错误体 `{"error":{"code":"1001","message":"..."}}`；1211 / HTTP 400「模型不存在，请检查模型代码」）
// - Claude API 兼容 https://docs.bigmodel.cn/cn/guide/develop/claude/introduction

/// 起一个按固定 (status, content-type, body) 应答的本地假上游，记录 (path, body)。
async fn spawn_glm_upstream(
    status: u16,
    content_type: &'static str,
    resp_body: &'static str,
) -> (String, Arc<Mutex<Vec<(String, String)>>>) {
    let hits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = hits.clone();
    let app = axum::Router::new().fallback(move |uri: axum::http::Uri, body: String| async move {
        sink.lock().unwrap().push((uri.path().to_string(), body));
        (
            axum::http::StatusCode::from_u16(status).unwrap(),
            [("content-type", content_type)],
            resp_body,
        )
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (format!("http://{addr}"), hits)
}

/// GLM 家族成员平台 `pt`（单端点 `ep`，base_url = 本地假上游）收一条入站请求，返回 (客户端状态码, 客户端响应体, 上游记录)。
async fn glm_member_call(
    pt: Protocol,
    ep: Protocol,
    gk: &str,
    inbound_path: &str,
    inbound_body: &'static str,
    upstream: (u16, &'static str, &'static str),
) -> (StatusCode, String, Vec<(String, String)>) {
    let (base, hits) = spawn_glm_upstream(upstream.0, upstream.1, upstream.2).await;
    let state = make_state(test_db().await).await;
    let pid = aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        pt,
        &base,
        std::slice::from_ref(&ep),
    )
    .await;
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: pid,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
    let req = HttpRequest::builder()
        .method("POST")
        .uri(inbound_path)
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(inbound_body))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    flush_log_queue(&state).await;
    let hits = hits.lock().unwrap().clone();
    (status, String::from_utf8_lossy(&bytes).into_owned(), hits)
}

const GLM_ANTHROPIC_IN: &str =
    r#"{"model":"glm-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#;
const GLM_ANTHROPIC_IN_STREAM: &str =
    r#"{"model":"glm-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#;
const GLM_OPENAI_IN: &str =
    r#"{"model":"glm-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#;
const GLM_OPENAI_IN_STREAM: &str =
    r#"{"model":"glm-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#;

/// 官方对话补全非流式响应形状（含 reasoning_content 与 cached_tokens）。
const GLM_OPENAI_RESP: &str = r#"{"id":"glm_1","created":1,"model":"glm-test","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"hi","reasoning_content":"think"}}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"prompt_tokens_details":{"cached_tokens":4}}}"#;
/// 官方流式形状：delta.reasoning_content → delta.content → 末帧 finish_reason + usage → `data: [DONE]`。
const GLM_OPENAI_SSE: &str = "data: {\"id\":\"glm_1\",\"created\":1,\"model\":\"glm-test\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"think\"}}]}\n\n\
data: {\"id\":\"glm_1\",\"created\":1,\"model\":\"glm-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n\
data: {\"id\":\"glm_1\",\"created\":1,\"model\":\"glm-test\",\"choices\":[{\"index\":0,\"finish_reason\":\"stop\",\"delta\":{}}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":4}}}\n\n\
data: [DONE]\n\n";
/// 官方错误码页：错误体 `{"error":{"code","message"}}`，1211 / HTTP 400 模型不存在。
const GLM_ERR_1211: &str = r#"{"error":{"code":"1211","message":"模型不存在，请检查模型代码"}}"#;
const GLM_ANTHROPIC_RESP: &str = r#"{"id":"msg_glm","type":"message","role":"assistant","model":"glm-test","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":4}}"#;
const GLM_ANTHROPIC_SSE: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_glm\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"glm-test\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0,\"cache_read_input_tokens\":4}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

/// issue 113：`glm` openai 端点非流式——入站 anthropic 转 openai 打用户 base_url `/chat/completions`；
/// 响应回 anthropic：reasoning_content → thinking 块，cached_tokens → cache_read_input_tokens。
#[tokio::test]
async fn glm_openai_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::Glm,
        Protocol::OpenAI,
        "gk113on",
        "/v1/messages",
        GLM_ANTHROPIC_IN,
        (200, "application/json", GLM_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["model"], "glm-test");
    assert_eq!(up["messages"][0]["role"], "user", "{up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["type"], "message", "{v}");
    assert_eq!(v["stop_reason"], "end_turn", "{v}");
    let content = v["content"].as_array().unwrap();
    assert!(content.iter().any(|b| b["type"] == "thinking" && b["thinking"] == "think"), "{v}");
    assert!(content.iter().any(|b| b["type"] == "text" && b["text"] == "hi"), "{v}");
    assert_eq!(v["usage"]["cache_read_input_tokens"], 4, "{v}");
    assert_eq!(v["usage"]["output_tokens"], 3, "{v}");
}

/// issue 113：`glm` openai 端点流式——reasoning_content / content 增量转 anthropic 事件，`[DONE]` 收尾为 message_stop。
#[tokio::test]
async fn glm_openai_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::Glm,
        Protocol::OpenAI,
        "gk113os",
        "/v1/messages",
        GLM_ANTHROPIC_IN_STREAM,
        (200, "text/event-stream", GLM_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["stream"], true, "{up}");
    assert!(resp.contains("thinking_delta") && resp.contains("think"), "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("message_stop"), "{resp}");
}

/// issue 113：`glm` 官方错误体（1211 模型不存在）——适配器分类为模型类，客户端拿到上游原状态码与错误码。
#[tokio::test]
async fn glm_openai_endpoint_error_body() {
    let pa = aidog_adapter::platform::adapter_for("glm");
    assert_eq!(
        pa.classify_error(400, GLM_ERR_1211),
        aidog_adapter::platform::error::UpstreamErrorClass::Model
    );
    let (status, resp, hits) = glm_member_call(
        Protocol::Glm,
        Protocol::OpenAI,
        "gk113oe",
        "/v1/messages",
        GLM_ANTHROPIC_IN,
        (400, "application/json", GLM_ERR_1211),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("1211"), "{resp}");
}

/// issue 113：`glm` 非流式 usage 抽取——openai 臂 input 扣除 cached_tokens（避免双重计价），cache_read = cached_tokens。
#[tokio::test]
async fn glm_usage_extraction_openai_and_anthropic() {
    let pa = aidog_adapter::platform::adapter_for("glm");
    let (i, o, c, cw, _, _) = pa.extract_usage(GLM_OPENAI_RESP);
    assert_eq!((i, o, c, cw), (6, 3, 4, 0));
    let (i, o, c, cw, _, _) = pa.extract_usage(GLM_ANTHROPIC_RESP);
    assert_eq!((i, o, c, cw), (6, 3, 4, 0));
}

/// issue 113：`glm` anthropic 端点非流式——入站 openai 转 anthropic 打用户 base_url `/v1/messages`；
/// 响应回 openai 形状，usage 透出 prompt / completion。
#[tokio::test]
async fn glm_anthropic_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::Glm,
        Protocol::Anthropic,
        "gk113an",
        "/chat/completions",
        GLM_OPENAI_IN,
        (200, "application/json", GLM_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/v1/messages");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["max_tokens"], 32, "{up}");
    assert_eq!(up["messages"][0]["role"], "user", "{up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "hi", "{v}");
    assert_eq!(v["choices"][0]["finish_reason"], "stop", "{v}");
    assert_eq!(v["usage"]["completion_tokens"], 3, "{v}");
}

/// issue 113：`glm` anthropic 端点流式——anthropic 事件转 openai chunk，结尾 `[DONE]`。
#[tokio::test]
async fn glm_anthropic_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::Glm,
        Protocol::Anthropic,
        "gk113as",
        "/chat/completions",
        GLM_OPENAI_IN_STREAM,
        (200, "text/event-stream", GLM_ANTHROPIC_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert!(resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");
}

/// issue 113：`glm` anthropic 端点错误——官方错误体原样回客户端（状态码 400 + 业务码 1211）。
#[tokio::test]
async fn glm_anthropic_endpoint_error_body() {
    let (status, resp, hits) = glm_member_call(
        Protocol::Glm,
        Protocol::Anthropic,
        "gk113ae",
        "/chat/completions",
        GLM_OPENAI_IN,
        (400, "application/json", GLM_ERR_1211),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("1211"), "{resp}");
}

// ══ issue 114：GLM 国际版（z.ai）成员 `glm_en` 逐端点协议护栏 + 1212 英文错误分类 ══
// 官方出处（核对日期 2026-11-16）：
// - 对话补全 https://docs.z.ai/api-reference/llm/chat-completion
//   （usage.prompt_tokens_details.cached_tokens、流结束 `data: [DONE]`）
// - 流式 https://docs.z.ai/guides/capabilities/streaming（`choices[0].delta.reasoning_content`）
// - 错误码 https://docs.z.ai/api-reference/api-code
//   （错误体 `{"error":{"code","message"}}`；1211 / HTTP 400「Unknown Model, please check the model code.」；
//   1212 / HTTP 400「Current model does not support `${method}` call method」）

/// 官方英文错误体：1211 模型不存在。
const GLM_EN_ERR_1211: &str = r#"{"error":{"code":"1211","message":"Unknown Model, please check the model code."}}"#;
/// 官方英文错误体：1212 当前模型不支持该调用方式（`${method}` 占位按官方原文保留）。
const GLM_EN_ERR_1212: &str =
    r#"{"error":{"code":"1212","message":"Current model does not support `${method}` call method"}}"#;
/// bigmodel 中文同码错误体（https://docs.bigmodel.cn/cn/api/api-code「当前模型不支持 `${method}` 调用方式」）。
const GLM_ERR_1212_ZH: &str = r#"{"error":{"code":"1212","message":"当前模型不支持 `${method}` 调用方式"}}"#;

/// issue 114：1212 是模型类错误（当前模型不支持该调用方式）。中文文案经基座特征命中模型类，
/// 英文文案「does not support」不在基座特征内会落「未知」类；家族适配器按业务码 1212 归模型类，
/// 与中文成员一致（模型类且无同平台备选时换平台、不记平台 last_error）。
#[tokio::test]
async fn glm_en_error_1212_classified_as_model() {
    use aidog_adapter::platform::error::{classify_upstream_error, UpstreamErrorClass};
    assert_eq!(classify_upstream_error(400, GLM_ERR_1212_ZH), UpstreamErrorClass::Model);
    for code in ["glm_en", "glm_coding_en", "glm"] {
        let pa = aidog_adapter::platform::adapter_for(code);
        assert_eq!(pa.classify_error(400, GLM_EN_ERR_1212), UpstreamErrorClass::Model, "{code}");
        assert_eq!(pa.classify_error(400, GLM_ERR_1212_ZH), UpstreamErrorClass::Model, "{code}");
    }
    // 其余业务码不受影响：仍走基座分类（1214 参数非法 = 未知，429 = 服务类）。
    let pa = aidog_adapter::platform::adapter_for("glm_en");
    let e1214 = r#"{"error":{"code":"1214","message":"Parameter `x` is invalid. Please check the documentation."}}"#;
    assert_eq!(pa.classify_error(400, e1214), UpstreamErrorClass::Unknown);
    // 鉴权 / 余额 / 限频状态码优先于业务码：1212 体在这些状态下仍归服务类（PR #180 审查跟进）。
    for status in [401, 402, 403, 429] {
        for body in [GLM_EN_ERR_1212, GLM_ERR_1212_ZH, GLM_EN_ERR_1212_INT] {
            assert_eq!(pa.classify_error(status, body), UpstreamErrorClass::Service, "{status}: {body}");
        }
    }
}

/// 官方 OpenAPI 中 `Error.code` 为 `type: integer`（https://docs.z.ai/api-reference/llm/chat-completion），
/// 错误码页示例为字符串；整数形态的 1212 英文错误体。
const GLM_EN_ERR_1212_INT: &str =
    r#"{"error":{"code":1212,"message":"Current model does not support `${method}` call method"}}"#;

/// issue 115（PR #180 审查跟进）：业务码 1212 以 JSON 整数下发时同样归模型类（家族四成员）。
#[tokio::test]
async fn glm_family_error_1212_integer_code_classified_as_model() {
    use aidog_adapter::platform::error::UpstreamErrorClass;
    for code in ["glm", "glm_en", "glm_coding", "glm_coding_en"] {
        let pa = aidog_adapter::platform::adapter_for(code);
        assert_eq!(pa.classify_error(400, GLM_EN_ERR_1212_INT), UpstreamErrorClass::Model, "{code}");
    }
}

/// issue 114：`glm_en` openai 端点非流式——入站 anthropic 转 openai 打用户 base_url `/chat/completions`；
/// 响应回 anthropic：reasoning_content → thinking 块，cached_tokens → cache_read_input_tokens。
#[tokio::test]
async fn glm_en_openai_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmEn,
        Protocol::OpenAI,
        "gk114on",
        "/v1/messages",
        GLM_ANTHROPIC_IN,
        (200, "application/json", GLM_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["model"], "glm-test");
    assert_eq!(up["messages"][0]["role"], "user", "{up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["type"], "message", "{v}");
    assert_eq!(v["stop_reason"], "end_turn", "{v}");
    let content = v["content"].as_array().unwrap();
    assert!(content.iter().any(|b| b["type"] == "thinking" && b["thinking"] == "think"), "{v}");
    assert!(content.iter().any(|b| b["type"] == "text" && b["text"] == "hi"), "{v}");
    assert_eq!(v["usage"]["cache_read_input_tokens"], 4, "{v}");
    assert_eq!(v["usage"]["output_tokens"], 3, "{v}");
}

/// issue 114：`glm_en` openai 端点流式——reasoning_content / content 增量转 anthropic 事件，`[DONE]` 收尾为 message_stop。
#[tokio::test]
async fn glm_en_openai_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmEn,
        Protocol::OpenAI,
        "gk114os",
        "/v1/messages",
        GLM_ANTHROPIC_IN_STREAM,
        (200, "text/event-stream", GLM_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["stream"], true, "{up}");
    assert!(resp.contains("thinking_delta") && resp.contains("think"), "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("message_stop"), "{resp}");
}

/// issue 114：`glm_en` 官方英文错误体（1211 Unknown Model）——模型类，客户端拿到上游原状态码与错误码。
#[tokio::test]
async fn glm_en_openai_endpoint_error_body() {
    let pa = aidog_adapter::platform::adapter_for("glm_en");
    assert_eq!(
        pa.classify_error(400, GLM_EN_ERR_1211),
        aidog_adapter::platform::error::UpstreamErrorClass::Model
    );
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmEn,
        Protocol::OpenAI,
        "gk114oe",
        "/v1/messages",
        GLM_ANTHROPIC_IN,
        (400, "application/json", GLM_EN_ERR_1211),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("1211"), "{resp}");
}

/// issue 114：`glm_en` 非流式 usage 抽取——openai 臂 input 扣除 cached_tokens，cache_read = cached_tokens。
#[tokio::test]
async fn glm_en_usage_extraction_openai_and_anthropic() {
    let pa = aidog_adapter::platform::adapter_for("glm_en");
    let (i, o, c, cw, _, _) = pa.extract_usage(GLM_OPENAI_RESP);
    assert_eq!((i, o, c, cw), (6, 3, 4, 0));
    let (i, o, c, cw, _, _) = pa.extract_usage(GLM_ANTHROPIC_RESP);
    assert_eq!((i, o, c, cw), (6, 3, 4, 0));
}

/// issue 114：`glm_en` anthropic 端点非流式——入站 openai 转 anthropic 打用户 base_url `/v1/messages`；
/// 响应回 openai 形状，usage 透出 completion。
#[tokio::test]
async fn glm_en_anthropic_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmEn,
        Protocol::Anthropic,
        "gk114an",
        "/chat/completions",
        GLM_OPENAI_IN,
        (200, "application/json", GLM_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/v1/messages");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["max_tokens"], 32, "{up}");
    assert_eq!(up["messages"][0]["role"], "user", "{up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "hi", "{v}");
    assert_eq!(v["choices"][0]["finish_reason"], "stop", "{v}");
    assert_eq!(v["usage"]["completion_tokens"], 3, "{v}");
}

/// issue 114：`glm_en` anthropic 端点流式——anthropic 事件转 openai chunk，结尾 `[DONE]`。
#[tokio::test]
async fn glm_en_anthropic_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmEn,
        Protocol::Anthropic,
        "gk114as",
        "/chat/completions",
        GLM_OPENAI_IN_STREAM,
        (200, "text/event-stream", GLM_ANTHROPIC_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert!(resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");
}

/// issue 114：`glm_en` anthropic 端点错误——官方英文错误体原样回客户端（状态码 400 + 业务码 1211）。
#[tokio::test]
async fn glm_en_anthropic_endpoint_error_body() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmEn,
        Protocol::Anthropic,
        "gk114ae",
        "/chat/completions",
        GLM_OPENAI_IN,
        (400, "application/json", GLM_EN_ERR_1211),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("1211"), "{resp}");
}

// ══ issue 115：GLM 编码套餐（智谱）成员 `glm_coding` 逐端点协议护栏 ══
// 官方出处（核对日期 2026-11-16）：
// - 接入端点 https://docs.bigmodel.cn/cn/coding-plan/quick-start（Anthropic `/api/anthropic`、OpenAI `/api/coding/paas/v4`）
// - 保留式思考 https://docs.bigmodel.cn/cn/guide/capabilities/thinking-mode（Coding Plan 端点默认开启，aidog 不写 clear_thinking）
// - 错误码 https://docs.bigmodel.cn/cn/api/api-code（1309 / HTTP 429「您的 GLM Coding Plan 套餐已到期」）
// 响应形状与 `glm` 共用对话补全构件（GLM_OPENAI_RESP / GLM_OPENAI_SSE / GLM_ANTHROPIC_*）。

/// issue 115：`glm_coding` openai 端点非流式——入站 anthropic 转 openai 打 `/chat/completions`，不写 `clear_thinking`；
/// 响应回 anthropic：reasoning_content → thinking 块，cached_tokens → cache_read_input_tokens。
#[tokio::test]
async fn glm_coding_openai_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCoding,
        Protocol::OpenAI,
        "gk115on",
        "/v1/messages",
        GLM_ANTHROPIC_IN,
        (200, "application/json", GLM_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["model"], "glm-test");
    assert_eq!(up["messages"][0]["role"], "user", "{up}");
    assert!(up.get("clear_thinking").is_none(), "保留式思考按官方端点默认，不写 clear_thinking: {up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["stop_reason"], "end_turn", "{v}");
    let content = v["content"].as_array().unwrap();
    assert!(content.iter().any(|b| b["type"] == "thinking" && b["thinking"] == "think"), "{v}");
    assert!(content.iter().any(|b| b["type"] == "text" && b["text"] == "hi"), "{v}");
    assert_eq!(v["usage"]["cache_read_input_tokens"], 4, "{v}");
    assert_eq!(v["usage"]["output_tokens"], 3, "{v}");
}

/// issue 115：`glm_coding` openai 端点流式——reasoning_content / content 增量转 anthropic 事件，`[DONE]` 收尾为 message_stop。
#[tokio::test]
async fn glm_coding_openai_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCoding,
        Protocol::OpenAI,
        "gk115os",
        "/v1/messages",
        GLM_ANTHROPIC_IN_STREAM,
        (200, "text/event-stream", GLM_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["stream"], true, "{up}");
    assert!(resp.contains("thinking_delta") && resp.contains("think"), "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("message_stop"), "{resp}");
}

/// issue 115：`glm_coding` 错误——1211 原样回客户端；套餐到期 1309（HTTP 429）归服务类。
#[tokio::test]
async fn glm_coding_openai_endpoint_error_body() {
    use aidog_adapter::platform::error::UpstreamErrorClass;
    let pa = aidog_adapter::platform::adapter_for("glm_coding");
    assert_eq!(pa.classify_error(400, GLM_ERR_1211), UpstreamErrorClass::Model);
    let e1309 = r#"{"error":{"code":"1309","message":"您的 GLM Coding Plan 套餐已到期，暂无法使用，前往官方续订后即可恢复"}}"#;
    // 官方 1309 恒为 HTTP 429，状态码先于业务码归服务类；断言 429 无法失败，改验非 429 状态下
    // 业务码覆写只认 1212，1309 不被当作模型类（issue 116，PR #181 审查跟进）。
    assert_eq!(pa.classify_error(400, e1309), UpstreamErrorClass::Unknown);
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCoding,
        Protocol::OpenAI,
        "gk115oe",
        "/v1/messages",
        GLM_ANTHROPIC_IN,
        (400, "application/json", GLM_ERR_1211),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("1211"), "{resp}");
}

/// issue 115：`glm_coding` usage 抽取——openai 臂 input 扣除 cached_tokens，cache_read = cached_tokens。
#[tokio::test]
async fn glm_coding_usage_extraction_openai_and_anthropic() {
    let pa = aidog_adapter::platform::adapter_for("glm_coding");
    let (i, o, c, cw, _, _) = pa.extract_usage(GLM_OPENAI_RESP);
    assert_eq!((i, o, c, cw), (6, 3, 4, 0));
    let (i, o, c, cw, _, _) = pa.extract_usage(GLM_ANTHROPIC_RESP);
    assert_eq!((i, o, c, cw), (6, 3, 4, 0));
}

/// issue 115：`glm_coding` 的 coding_plan 端点注入为 no-op（迁移前转发层同为 no-op，行为不变）。
#[test]
fn glm_coding_coding_plan_injection_is_noop() {
    let pa = aidog_adapter::platform::adapter_for("glm_coding");
    let mut body = serde_json::from_str::<serde_json::Value>(GLM_OPENAI_IN).unwrap();
    let before = body.clone();
    pa.inject_coding_plan(&mut body, &Protocol::GlmCoding);
    assert_eq!(body, before);
}

/// issue 115：`glm_coding` anthropic 端点非流式——入站 openai 转 anthropic 打 `/v1/messages`，响应回 openai 形状。
#[tokio::test]
async fn glm_coding_anthropic_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCoding,
        Protocol::Anthropic,
        "gk115an",
        "/chat/completions",
        GLM_OPENAI_IN,
        (200, "application/json", GLM_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/v1/messages");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["max_tokens"], 32, "{up}");
    assert_eq!(up["messages"][0]["role"], "user", "{up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "hi", "{v}");
    assert_eq!(v["choices"][0]["finish_reason"], "stop", "{v}");
    assert_eq!(v["usage"]["completion_tokens"], 3, "{v}");
}

/// issue 115：`glm_coding` anthropic 端点流式——anthropic 事件转 openai chunk，结尾 `[DONE]`。
#[tokio::test]
async fn glm_coding_anthropic_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCoding,
        Protocol::Anthropic,
        "gk115as",
        "/chat/completions",
        GLM_OPENAI_IN_STREAM,
        (200, "text/event-stream", GLM_ANTHROPIC_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert!(resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");
}

/// issue 115：`glm_coding` anthropic 端点错误——官方错误体原样回客户端（400 + 1211）。
#[tokio::test]
async fn glm_coding_anthropic_endpoint_error_body() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCoding,
        Protocol::Anthropic,
        "gk115ae",
        "/chat/completions",
        GLM_OPENAI_IN,
        (400, "application/json", GLM_ERR_1211),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("1211"), "{resp}");
}

// ══ issue 116：GLM 编码套餐国际版（z.ai）成员 `glm_coding_en` 逐端点协议护栏 ══
// 官方出处（核对日期 2026-11-16）：
// - 接入端点 https://docs.z.ai/devpack/quick-start（Anthropic `/api/anthropic`、OpenAI `/api/coding/paas/v4`）
// - 保留式思考 https://docs.z.ai/guides/capabilities/thinking-mode（Coding Plan 端点默认开启，aidog 不写 clear_thinking）
// - 错误码 https://docs.z.ai/api-reference/api-code（英文文案；1211 / 1212 / 套餐 13xx 均同 bigmodel 码表）
// - OpenAPI https://docs.z.ai/api-reference/llm/chat-completion（失败响应 `Error` 为顶层 `{code: integer, message}`）

/// OpenAPI `Error` 形态：顶层 `code` / `message`，无 `error` 包裹。
const GLM_EN_ERR_1212_FLAT: &str =
    r#"{"code":1212,"message":"Current model does not support `${method}` call method"}"#;

/// issue 116：错误码页示例为 `error` 包裹，OpenAPI 失败响应为顶层 `{code, message}`；
/// 两种形态的 1212 都归模型类（家族四成员，与基座 `error` 缺省回落顶层的读法一致）。
#[test]
fn glm_family_error_1212_flat_shape_classified_as_model() {
    use aidog_adapter::platform::error::UpstreamErrorClass;
    for code in ["glm", "glm_en", "glm_coding", "glm_coding_en"] {
        let pa = aidog_adapter::platform::adapter_for(code);
        assert_eq!(pa.classify_error(400, GLM_EN_ERR_1212_FLAT), UpstreamErrorClass::Model, "{code}");
        assert_eq!(pa.classify_error(429, GLM_EN_ERR_1212_FLAT), UpstreamErrorClass::Service, "{code}");
    }
}

/// issue 116：`glm_coding_en` openai 端点非流式——入站 anthropic 转 openai 打 `/chat/completions`，不写 `clear_thinking`；
/// 响应回 anthropic：reasoning_content → thinking 块，cached_tokens → cache_read_input_tokens。
#[tokio::test]
async fn glm_coding_en_openai_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCodingEn,
        Protocol::OpenAI,
        "gk116on",
        "/v1/messages",
        GLM_ANTHROPIC_IN,
        (200, "application/json", GLM_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["model"], "glm-test");
    assert_eq!(up["messages"][0]["role"], "user", "{up}");
    assert!(up.get("clear_thinking").is_none(), "保留式思考按官方端点默认，不写 clear_thinking: {up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["stop_reason"], "end_turn", "{v}");
    let content = v["content"].as_array().unwrap();
    assert!(content.iter().any(|b| b["type"] == "thinking" && b["thinking"] == "think"), "{v}");
    assert!(content.iter().any(|b| b["type"] == "text" && b["text"] == "hi"), "{v}");
    assert_eq!(v["usage"]["cache_read_input_tokens"], 4, "{v}");
    assert_eq!(v["usage"]["output_tokens"], 3, "{v}");
}

/// issue 116：`glm_coding_en` openai 端点流式——reasoning_content / content 增量转 anthropic 事件，`[DONE]` 收尾为 message_stop。
#[tokio::test]
async fn glm_coding_en_openai_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCodingEn,
        Protocol::OpenAI,
        "gk116os",
        "/v1/messages",
        GLM_ANTHROPIC_IN_STREAM,
        (200, "text/event-stream", GLM_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["stream"], true, "{up}");
    assert!(resp.contains("thinking_delta") && resp.contains("think"), "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("message_stop"), "{resp}");
}

/// issue 116：`glm_coding_en` 错误——官方英文 1211 归模型类并原样回客户端（400 + 1211）。
#[tokio::test]
async fn glm_coding_en_openai_endpoint_error_body() {
    let pa = aidog_adapter::platform::adapter_for("glm_coding_en");
    assert_eq!(
        pa.classify_error(400, GLM_EN_ERR_1211),
        aidog_adapter::platform::error::UpstreamErrorClass::Model
    );
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCodingEn,
        Protocol::OpenAI,
        "gk116oe",
        "/v1/messages",
        GLM_ANTHROPIC_IN,
        (400, "application/json", GLM_EN_ERR_1211),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("1211"), "{resp}");
}

/// issue 116：`glm_coding_en` usage 抽取——openai 臂 input 扣除 cached_tokens，cache_read = cached_tokens。
#[tokio::test]
async fn glm_coding_en_usage_extraction_openai_and_anthropic() {
    let pa = aidog_adapter::platform::adapter_for("glm_coding_en");
    let (i, o, c, cw, _, _) = pa.extract_usage(GLM_OPENAI_RESP);
    assert_eq!((i, o, c, cw), (6, 3, 4, 0));
    let (i, o, c, cw, _, _) = pa.extract_usage(GLM_ANTHROPIC_RESP);
    assert_eq!((i, o, c, cw), (6, 3, 4, 0));
}

/// issue 116：`glm_coding_en` 的 coding_plan 端点注入为 no-op（迁移前转发层同为 no-op，行为不变）。
#[test]
fn glm_coding_en_coding_plan_injection_is_noop() {
    let pa = aidog_adapter::platform::adapter_for("glm_coding_en");
    let mut body = serde_json::from_str::<serde_json::Value>(GLM_OPENAI_IN).unwrap();
    let before = body.clone();
    pa.inject_coding_plan(&mut body, &Protocol::GlmCodingEn);
    assert_eq!(body, before);
}

/// issue 116：`glm_coding_en` anthropic 端点非流式——入站 openai 转 anthropic 打 `/v1/messages`，响应回 openai 形状。
#[tokio::test]
async fn glm_coding_en_anthropic_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCodingEn,
        Protocol::Anthropic,
        "gk116an",
        "/chat/completions",
        GLM_OPENAI_IN,
        (200, "application/json", GLM_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/v1/messages");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["max_tokens"], 32, "{up}");
    assert_eq!(up["messages"][0]["role"], "user", "{up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "hi", "{v}");
    assert_eq!(v["choices"][0]["finish_reason"], "stop", "{v}");
    assert_eq!(v["usage"]["completion_tokens"], 3, "{v}");
}

/// issue 116：`glm_coding_en` anthropic 端点流式——anthropic 事件转 openai chunk，结尾 `[DONE]`。
#[tokio::test]
async fn glm_coding_en_anthropic_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCodingEn,
        Protocol::Anthropic,
        "gk116as",
        "/chat/completions",
        GLM_OPENAI_IN_STREAM,
        (200, "text/event-stream", GLM_ANTHROPIC_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert!(resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");
}

/// issue 116：`glm_coding_en` anthropic 端点错误——官方英文错误体原样回客户端（400 + 1211）。
#[tokio::test]
async fn glm_coding_en_anthropic_endpoint_error_body() {
    let (status, resp, hits) = glm_member_call(
        Protocol::GlmCodingEn,
        Protocol::Anthropic,
        "gk116ae",
        "/chat/completions",
        GLM_OPENAI_IN,
        (400, "application/json", GLM_EN_ERR_1211),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("1211"), "{resp}");
}

/// issue 115（PR #180 审查跟进）：端到端换平台——两平台 GLM 家族分组，A 回 400 + 英文 1212，B 回 200。
/// 1212 归模型类：客户端拿到 B 的 200，A 只被打一次，A 的 last_error 与熔断计数不动。
#[tokio::test]
async fn glm_family_1212_fails_over_without_touching_platform_health() {
    let (base_a, hits_a) = spawn_glm_upstream(400, "application/json", GLM_EN_ERR_1212).await;
    let (base_b, hits_b) = spawn_glm_upstream(200, "application/json", GLM_OPENAI_RESP).await;
    let state = make_state(test_db().await).await;
    let a = aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        Protocol::GlmCodingEn,
        &base_a,
        &[Protocol::OpenAI],
    )
    .await;
    let b = aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        Protocol::GlmCoding,
        &base_b,
        &[Protocol::OpenAI],
    )
    .await;
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group("gk115fo", vec![]))
        .await
        .unwrap();
    let inputs: Vec<_> = [a, b]
        .iter()
        .enumerate()
        .map(|(i, id)| GroupPlatformInput {
            platform_id: *id,
            priority: Some(i as i32),
            weight: Some(1),
            level_priority: Some(1),
        })
        .collect();
    aidog_db::set_group_platforms(&state.db, group.id, &inputs).await.unwrap();
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer gk115fo")
        .header("content-type", "application/json")
        .body(Body::from(GLM_ANTHROPIC_IN))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    flush_log_queue(&state).await;
    // 先断言 A 的尝试裁决（issue 116，PR #181 审查跟进）：400 从不碰平台健康，下方 last_error /
    // 熔断断言单独无法区分分类；1212 若落未知类或服务类，此处 error_class / exclusion 即红。
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let id = logs.iter().find(|l| l.group_key == "gk115fo").expect("log row missing").id.clone();
    let log = aidog_logs::get_proxy_log(&state.db, &id).await.unwrap().unwrap();
    assert_eq!(log.attempts[0].error_class.as_deref(), Some("model"), "{:?}", log.attempts);
    assert_eq!(serde_json::to_value(log.attempts[0].exclusion).unwrap(), "pair", "{:?}", log.attempts);
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    assert_eq!(hits_a.lock().unwrap().len(), 1, "A 应恰好被打一次");
    assert_eq!(hits_b.lock().unwrap().len(), 1, "B 应接住请求");
    let pa = aidog_db::get_platform(&state.db, a).await.unwrap().unwrap();
    assert!(pa.last_error.is_empty(), "模型类失败不写平台 last_error: {:?}", pa.last_error);
    assert_eq!(state.scheduler.breaker_fails(a), 0, "模型类失败不计熔断");
}

// ══ issue 76：月之暗面 Kimi 家族基座（kimi / kimi_en / kimi_coding）逐端点协议护栏 ══
// 官方出处（核对日期 2026-11-16）：
// - Chat Completions https://platform.kimi.com/docs/api/chat.md 、https://platform.kimi.ai/docs/api/chat.md
//   （kimi-k2.6「`thinking.type` 默认 "enabled"，传 "disabled" 可关闭思考」；usage
//   `prompt_tokens_details.cached_tokens`；流式先 `delta.reasoning_content` 后 `delta.content`，`data: [DONE]` 结束）
// - Messages https://platform.kimi.com/docs/api/messages.md
//   （usage `cache_read_input_tokens`；错误体 `{"type":"error","error":{"type","message"}}`）
// - 错误码 https://platform.kimi.com/docs/api/errors.md 、https://platform.kimi.ai/docs/api/errors.md
//   （404 `resource_not_found_error`「Model not found, or this account does not have permission to access the model」）

const KIMI_FAMILY: [Protocol; 3] = [Protocol::Kimi, Protocol::KimiEn, Protocol::KimiCoding];
/// openai 端点禁思考写官方 `thinking` 的成员（kimi_coding 保持基座行为，见成员 issue 119）。
const KIMI_OFFICIAL_THINKING: [Protocol; 2] = [Protocol::Kimi, Protocol::KimiEn];

const KIMI_OPENAI_RESP: &str = r#"{"id":"chatcmpl-1","object":"chat.completion","created":1,"model":"kimi-test","choices":[{"index":0,"message":{"role":"assistant","content":"hi","reasoning_content":"think"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"prompt_tokens_details":{"cached_tokens":4}}}"#;
const KIMI_OPENAI_SSE: &str = "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"kimi-test\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\n\n\
data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"kimi-test\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"think\"},\"finish_reason\":null}]}\n\n\
data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"kimi-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"},\"finish_reason\":\"stop\"}]}\n\n\
data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"kimi-test\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":4}}}\n\n\
data: [DONE]\n\n";
const KIMI_ERR_404: &str = r#"{"error":{"type":"resource_not_found_error","message":"Model not found, or this account does not have permission to access the model"}}"#;
const KIMI_ANTHROPIC_ERR_404: &str = r#"{"type":"error","error":{"type":"resource_not_found_error","message":"Model not found, or this account does not have permission to access the model"}}"#;
const KIMI_ANTHROPIC_RESP: &str = r#"{"id":"msg_k","type":"message","role":"assistant","model":"kimi-test","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":4}}"#;

fn kimi_gk(pt: &Protocol, tag: &str) -> String {
    format!("gk76{}{tag}", pt.wire_str().replace('_', ""))
}

/// issue 76（先红）：Kimi 家族 openai 端点禁思考写官方 `thinking.type=disabled`，
/// 不写 vLLM 式 `chat_template_kwargs`（Kimi Chat Completions 文档无此参数）。
#[tokio::test]
async fn kimi_family_openai_endpoint_disables_thinking_with_official_param() {
    for pt in KIMI_OFFICIAL_THINKING {
        let body = glm_outbound_body(pt.clone(), Protocol::OpenAI, &kimi_gk(&pt, "dto")).await;
        assert_eq!(
            body.get("thinking"),
            Some(&serde_json::json!({"type": "disabled"})),
            "{pt:?} openai 端点应写 thinking.type=disabled: {body}"
        );
        assert!(body.get("chat_template_kwargs").is_none(), "{pt:?} 不应写 chat_template_kwargs: {body}");
        assert!(body.get("disable_thinking").is_none(), "{pt:?} 私有字段应剥除: {body}");
    }
}

/// issue 76（先红）：kimi-k2.6 非思考模式 temperature 固定 0.6、传其他值报错
/// （models-overview.md「思考模式固定 `1.0`，非思考模式固定 `0.6`」「传入其他值会报错，建议不要显式传入」），
/// openai 端点禁思考时剥除客户端 temperature；未禁思考时原样透传。
#[tokio::test]
async fn kimi_family_openai_disable_thinking_strips_temperature() {
    for pt in KIMI_OFFICIAL_THINKING {
        let mut req = serde_json::json!({"model": "kimi-test", "max_tokens": 32, "temperature": 1.0,
            "disable_thinking": true, "messages": [{"role": "user", "content": "hi"}]});
        let body = outbound_body_with(pt.clone(), Protocol::OpenAI, &kimi_gk(&pt, "tmd"), req.clone()).await;
        assert_eq!(body.get("thinking"), Some(&serde_json::json!({"type": "disabled"})), "{pt:?}: {body}");
        assert!(body.get("temperature").is_none(), "{pt:?} 禁思考时不应透传 temperature: {body}");
        req.as_object_mut().unwrap().remove("disable_thinking");
        let body = outbound_body_with(pt.clone(), Protocol::OpenAI, &kimi_gk(&pt, "tmk"), req).await;
        assert_eq!(body.get("temperature"), Some(&serde_json::json!(1.0)), "{pt:?} 未禁思考应透传: {body}");
    }
}

/// issue 76（先红）：kimi_coding 的 openai 端点禁思考保持 PR 前基座行为——写
/// `chat_template_kwargs.enable_thinking=false`、不写 `thinking`、客户端 temperature 原样透传。
/// 本 PR 的取证（platform.kimi.com chat.md / models-overview.md）只覆盖开放平台模型；Kimi Code 模型页
/// 「off / none / disabled … 关闭 thinking 后，请求均由 K2.8 Preview（无思考）处理」「K3.1 Pro 系列：
/// 关闭思考的请求会被拒绝（HTTP 400）」，语义归成员 issue 119 拍板。
#[tokio::test]
async fn kimi_coding_openai_disable_thinking_keeps_base_behaviour() {
    let pt = Protocol::KimiCoding;
    let req = serde_json::json!({"model": "kimi-test", "max_tokens": 32, "temperature": 1.0,
        "disable_thinking": true, "messages": [{"role": "user", "content": "hi"}]});
    let body = outbound_body_with(pt.clone(), Protocol::OpenAI, &kimi_gk(&pt, "cbb"), req).await;
    assert_eq!(
        body.pointer("/chat_template_kwargs/enable_thinking"),
        Some(&serde_json::json!(false)),
        "kimi_coding 应保持基座 chat_template_kwargs: {body}"
    );
    assert!(body.get("thinking").is_none(), "kimi_coding 不应写 thinking: {body}");
    assert_eq!(body.get("temperature"), Some(&serde_json::json!(1.0)), "kimi_coding 应透传 temperature: {body}");
    assert!(body.get("disable_thinking").is_none(), "私有字段应剥除: {body}");
}

/// issue 76（护栏）：不可关闭思考的真实 registry 模型（chat.md「kimi-k2.7-code 和
/// kimi-k2.7-code-highspeed … 仅支持 "enabled"，传 "disabled" 会报错」；K3「不使用 `thinking` 参数」），
/// openai 端点禁思考时不写任何 `thinking` 键。
#[tokio::test]
async fn kimi_forced_thinking_models_send_no_thinking_on_openai_endpoint() {
    for pt in [Protocol::Kimi, Protocol::KimiEn] {
        for model in ["kimi-k2.7-code", "kimi-k2.7-code-highspeed", "kimi-k3"] {
            let tag = format!("ft{}", model.replace(['.', '-'], ""));
            let body = outbound_body_for_model(pt.clone(), Protocol::OpenAI, &kimi_gk(&pt, &tag), model).await;
            assert!(body.get("thinking").is_none(), "{pt:?}/{model} 不应写 thinking: {body}");
            assert!(body.get("chat_template_kwargs").is_none(), "{pt:?}/{model}: {body}");
            assert!(body.get("disable_thinking").is_none(), "{pt:?}/{model}: {body}");
        }
    }
}

/// issue 76（护栏）：anthropic 端点禁思考同基座。
#[tokio::test]
async fn kimi_family_anthropic_endpoint_disables_thinking() {
    for pt in KIMI_FAMILY {
        let body = glm_outbound_body(pt.clone(), Protocol::Anthropic, &kimi_gk(&pt, "dta")).await;
        assert_eq!(body.get("thinking"), Some(&serde_json::json!({"type": "disabled"})), "{pt:?}: {body}");
        assert!(body.get("disable_thinking").is_none(), "{pt:?}: {body}");
    }
}

/// issue 76（护栏）：openai 端点非流式——请求转换、reasoning_content → thinking 块、cached_tokens → cache_read。
#[tokio::test]
async fn kimi_family_openai_endpoint_non_stream() {
    for pt in KIMI_FAMILY {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &kimi_gk(&pt, "on"),
            "/v1/messages",
            GLM_ANTHROPIC_IN,
            (200, "application/json", KIMI_OPENAI_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["messages"][0]["role"], "user", "{pt:?}: {up}");
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["stop_reason"], "end_turn", "{pt:?}: {v}");
        let content = v["content"].as_array().unwrap();
        assert!(content.iter().any(|b| b["type"] == "thinking" && b["thinking"] == "think"), "{pt:?}: {v}");
        assert!(content.iter().any(|b| b["type"] == "text" && b["text"] == "hi"), "{pt:?}: {v}");
        assert_eq!(v["usage"]["cache_read_input_tokens"], 4, "{pt:?}: {v}");
    }
}

/// issue 76（护栏）：openai 端点流式——reasoning_content / content 增量转 anthropic 事件，`[DONE]` 收尾。
#[tokio::test]
async fn kimi_family_openai_endpoint_stream() {
    for pt in KIMI_FAMILY {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &kimi_gk(&pt, "os"),
            "/v1/messages",
            GLM_ANTHROPIC_IN_STREAM,
            (200, "text/event-stream", KIMI_OPENAI_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["stream"], true, "{pt:?}: {up}");
        assert!(resp.contains("thinking_delta") && resp.contains("think"), "{pt:?}: {resp}");
        assert!(resp.contains("text_delta") && resp.contains("\"hi\""), "{pt:?}: {resp}");
        assert!(resp.contains("message_stop"), "{pt:?}: {resp}");
    }
}

/// issue 76（护栏）：404 `resource_not_found_error` 归模型类；两种端点的官方错误体原样回客户端。
#[tokio::test]
async fn kimi_family_error_body() {
    use aidog_adapter::platform::error::UpstreamErrorClass;
    for pt in KIMI_FAMILY {
        let pa = aidog_adapter::platform::adapter_for(&pt.wire_str());
        assert_eq!(pa.classify_error(404, KIMI_ERR_404), UpstreamErrorClass::Model, "{pt:?}");
        assert_eq!(pa.classify_error(404, KIMI_ANTHROPIC_ERR_404), UpstreamErrorClass::Model, "{pt:?}");
        for (ep, inbound_path, inbound, err) in [
            (Protocol::OpenAI, "/v1/messages", GLM_ANTHROPIC_IN, KIMI_ERR_404),
            (Protocol::Anthropic, "/chat/completions", GLM_OPENAI_IN, KIMI_ANTHROPIC_ERR_404),
        ] {
            let (status, resp, hits) = glm_member_call(
                pt.clone(),
                ep.clone(),
                &kimi_gk(&pt, &format!("e{}", ep.wire_str())),
                inbound_path,
                inbound,
                (404, "application/json", err),
            )
            .await;
            assert!(!hits.is_empty(), "{pt:?}/{ep:?}");
            assert_eq!(status, StatusCode::NOT_FOUND, "{pt:?}/{ep:?}: {resp}");
            assert!(resp.contains("resource_not_found_error"), "{pt:?}/{ep:?}: {resp}");
        }
    }
}

/// issue 76（护栏）：非流式 usage 抽取——openai 臂 input 扣除 cached_tokens，anthropic 臂读 cache_read_input_tokens。
#[test]
fn kimi_family_usage_extraction() {
    for pt in KIMI_FAMILY {
        let pa = aidog_adapter::platform::adapter_for(&pt.wire_str());
        let (i, o, c, cw, _, _) = pa.extract_usage(KIMI_OPENAI_RESP);
        assert_eq!((i, o, c, cw), (6, 3, 4, 0), "{pt:?}");
        let (i, o, c, cw, _, _) = pa.extract_usage(KIMI_ANTHROPIC_RESP);
        assert_eq!((i, o, c, cw), (6, 3, 4, 0), "{pt:?}");
    }
}

/// issue 76（护栏）：anthropic 端点非流式 + 流式——入站 openai 转 anthropic 打 `/v1/messages`，响应回 openai 形状。
#[tokio::test]
async fn kimi_family_anthropic_endpoint_non_stream_and_stream() {
    for pt in KIMI_FAMILY {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &kimi_gk(&pt, "an"),
            "/chat/completions",
            GLM_OPENAI_IN,
            (200, "application/json", KIMI_ANTHROPIC_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["max_tokens"], 32, "{pt:?}: {up}");
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["choices"][0]["message"]["content"], "hi", "{pt:?}: {v}");
        assert_eq!(v["usage"]["completion_tokens"], 3, "{pt:?}: {v}");

        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &kimi_gk(&pt, "as"),
            "/chat/completions",
            GLM_OPENAI_IN_STREAM,
            (200, "text/event-stream", GLM_ANTHROPIC_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}");
        assert!(resp.contains("\"hi\"") && resp.contains("[DONE]"), "{pt:?}: {resp}");
    }
}

/// issue 76 跟进（先红）：openai 客户端直发 `tool_choice:"required"`（同协议透传），
/// kimi-k2.6 / kimi-k2.7-code 出站降级为 `"auto"`——models-overview.md 对这两列
/// 「传 `"required"` 会报错」。anthropic 入站的 `tool_choice` 对象当前在入站解析即 400
/// （`ToolChoice` untagged 不认 `{"type":"any"}`，types.rs 既有缺口），到不了出站，
/// 故本组测试走透传（真实可达路径）。
#[tokio::test]
async fn kimi_family_openai_tool_choice_required_downgraded_for_affected_models() {
    for pt in KIMI_OFFICIAL_THINKING {
        for model in ["kimi-k2.6", "kimi-k2.7-code"] {
            let body = format!(
                r#"{{"model":"{model}","max_tokens":32,"tool_choice":"required","tools":[{{"type":"function","function":{{"name":"get_weather","description":"w","parameters":{{"type":"object","properties":{{}}}}}}}}],"messages":[{{"role":"user","content":"hi"}}]}}"#
            );
            let body: &'static str = Box::leak(body.into_boxed_str());
            let (status, resp, hits) = glm_member_call(
                pt.clone(),
                Protocol::OpenAI,
                &kimi_gk(&pt, "tcany"),
                "/chat/completions",
                body,
                (200, "application/json", KIMI_OPENAI_RESP),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{pt:?}/{model}: {resp}");
            let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
            assert_eq!(
                up.get("tool_choice"),
                Some(&serde_json::json!("auto")),
                "{pt:?}/{model} 应降级为 auto: {up}"
            );
        }
    }
}

/// issue 76 跟进（反证）：K3 系列接受 `"required"`（models-overview.md K3 列 `"auto"` / `"none"` /
/// `"required"`），不降级；anthropic wire 不在范围；kimi_coding 保持现状（成员 issue 119）。
#[tokio::test]
async fn kimi_family_tool_choice_required_not_downgraded_outside_affected_scope() {
    const K3_IN: &str = r#"{"model":"kimi-k3","max_tokens":32,"tool_choice":"required","tools":[{"type":"function","function":{"name":"get_weather","description":"w","parameters":{"type":"object","properties":{}}}}],"messages":[{"role":"user","content":"hi"}]}"#;
    for pt in KIMI_OFFICIAL_THINKING {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &kimi_gk(&pt, "tck3"),
            "/chat/completions",
            K3_IN,
            (200, "application/json", KIMI_OPENAI_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(
            up.get("tool_choice"),
            Some(&serde_json::json!("required")),
            "{pt:?}/kimi-k3 应保持 required: {up}"
        );
    }
    // kimi_coding 保持现状（成员 issue 119）。
    const CD_IN: &str = r#"{"model":"kimi-k2.6","max_tokens":32,"tool_choice":"required","tools":[{"type":"function","function":{"name":"get_weather","description":"w","parameters":{"type":"object","properties":{}}}}],"messages":[{"role":"user","content":"hi"}]}"#;
    let pt = Protocol::KimiCoding;
    let (status, resp, hits) = glm_member_call(
        pt.clone(),
        Protocol::OpenAI,
        &kimi_gk(&pt, "tccd"),
        "/chat/completions",
        CD_IN,
        (200, "application/json", KIMI_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(
        up.get("tool_choice"),
        Some(&serde_json::json!("required")),
        "{pt:?} 应保持 required: {up}"
    );
}

// ══ issue 117：Kimi（月之暗面）成员 `kimi` 护栏 ══
// 官方出处（核对日期 2026-11-16）：
// - Chat Completions https://platform.kimi.com/docs/api/chat.md
//   （prompt_cache_key「对于 Kimi Code Plan，此字段为必填以提高缓存命中率。对于其他多轮对话
//   Agent，也建议使用此字段」——标准平台可选）
// 思考 / tool_choice / temperature / usage / 流式 / 错误体逐项核对结论同家族基座（出处见
// kimi_family_* 各测试头注与 registry.rs 成员节），本成员无代码差异，登记 KimiFamilyAdapter；
// 本节只补家族循环未覆盖的 coding_plan 端点 prompt_cache_key 注入端到端护栏。

/// 建 Kimi 平台（单 openai 端点指向假上游，coding_plan 按参指定）并挂进分组，收一条入站请求，
/// 返回上游收到的出站体。
async fn kimi_member_outbound(gk: &str, coding_plan: bool) -> serde_json::Value {
    let (base, hits) = spawn_glm_upstream(200, "application/json", KIMI_OPENAI_RESP).await;
    let state = make_state(test_db().await).await;
    let pid = aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        Protocol::Kimi,
        &base,
        &[Protocol::OpenAI],
    )
    .await;
    // fixture 恒 coding_plan:false，按用例直写 DB 覆盖端点
    let eps = aidog_db::serialize_endpoints(&[PlatformEndpoint {
        protocol: Protocol::OpenAI,
        base_url: base.clone(),
        client_type: "default".to_string(),
        coding_plan,
    }]);
    state
        .db
        .call_platform_traced(None, std::panic::Location::caller(), move |conn| {
            conn.execute("UPDATE platform SET endpoints=?1 WHERE id=?2", rusqlite::params![eps, pid as i64])?;
            Ok(())
        })
        .await
        .expect("set coding_plan endpoint");
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: pid,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(GLM_ANTHROPIC_IN))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    flush_log_queue(&state).await;
    let hits = hits.lock().unwrap();
    assert_eq!(hits.len(), 1, "上游应恰好收到一次请求: {hits:?}");
    serde_json::from_str(&hits[0].1).unwrap()
}

/// issue 117（护栏）：coding_plan 端点出站注入 prompt_cache_key（迁移自转发层的既有行为，
/// 家族按平台协议分派仅 `Protocol::Kimi` 注入，aidog- 前缀会话键）；非 coding_plan 端点不注入。
#[tokio::test]
async fn kimi_member_coding_plan_endpoint_injects_prompt_cache_key() {
    let body = kimi_member_outbound("gk117cp", true).await;
    assert!(
        body.get("prompt_cache_key")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.starts_with("aidog-")),
        "coding_plan 端点应注入 prompt_cache_key: {body}"
    );
    let body = kimi_member_outbound("gk117cn", false).await;
    assert!(
        body.get("prompt_cache_key").is_none(),
        "非 coding_plan 端点不应注入 prompt_cache_key: {body}"
    );
}

async fn make_state(db: aidog_db::Db) -> Arc<ProxyState> {
    let (log_tx, log_rx) = tokio::sync::mpsc::channel(1024);
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: dashmap::DashMap::new(),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
        settings_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
        log_tx,
        log_queue_bytes: std::sync::atomic::AtomicU64::new(0),
    });
    spawn_log_writer(state.clone(), log_rx);
    state
}
