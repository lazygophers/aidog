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
        .body(Body::from(
            r#"{"model":"glm-test","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
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

/// 平台 `glm`（单端点 `ep`，base_url = 本地假上游）收一条入站请求，返回 (客户端状态码, 客户端响应体, 上游记录)。
async fn glm_member_call(
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
        Protocol::Glm,
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
