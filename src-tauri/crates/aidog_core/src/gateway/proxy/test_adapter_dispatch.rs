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
const RESPONSES_OK: &str = r#"{"id":"resp_1","object":"response","model":"gpt-x","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi","annotations":[]}]}],"usage":{"input_tokens":3,"output_tokens":2}}"#;

/// Doubao 适配器经本地假上游覆盖三种内置端点，断言实际出站路径与请求体。
#[tokio::test]
async fn doubao_disable_thinking_reaches_local_upstream_for_each_wire() {
    let cases = [
        (
            Protocol::OpenAI,
            "/chat/completions",
            OPENAI_OK,
            "thinking",
            "disabled",
        ),
        (
            Protocol::OpenAIResponses,
            "/responses",
            RESPONSES_OK,
            "reasoning",
            "none",
        ),
        (
            Protocol::Anthropic,
            "/v1/messages",
            ANTHROPIC_OK,
            "thinking",
            "disabled",
        ),
    ];

    for (i, (wire, expected_path, response, field, value)) in cases.into_iter().enumerate() {
        let (upstream, hits) = spawn_recording_upstream(response).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gkdwp{i}");
        setup_user_endpoint_group(&state, &gk, wire.clone(), &upstream).await;
        let platform = aidog_db::list_platforms(&state.db)
            .await
            .unwrap()
            .into_iter()
            .find(|p| p.name == "stub")
            .unwrap();
        let protocol = serde_json::to_string(&Protocol::Doubao).unwrap();
        state
            .db
            .call_platform_traced(None, std::panic::Location::caller(), move |conn| {
                conn.execute(
                    "UPDATE platform SET platform_type=?1 WHERE id=?2",
                    rusqlite::params![protocol, platform.id as i64],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        let request = HttpRequest::builder()
            .method("POST")
            .uri("/v1/messages")
            .header("authorization", format!("Bearer {gk}"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"model":"doubao-seed-2.1","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#,
            ))
            .unwrap();
        let resp = handle_proxy(AxumState(state.clone()), request).await;
        let resp_status = resp.status();
        if resp_status != StatusCode::OK {
            let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
            panic!(
                "wire={wire:?} status={resp_status} body={}",
                String::from_utf8_lossy(&body)
            );
        }
        flush_log_queue(&state).await;

        let hits = hits.lock().unwrap();
        assert_eq!(hits.len(), 1, "wire={wire:?}, hits={hits:?}");
        let (path, body) = &hits[0];
        assert_eq!(path, expected_path, "wire={wire:?}");
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        if field == "thinking" {
            assert_eq!(body[field]["type"], value, "wire={wire:?}: {body}");
        } else {
            assert_eq!(body[field]["effort"], value, "wire={wire:?}: {body}");
        }
        assert!(
            body.get("disable_thinking").is_none(),
            "wire={wire:?}: {body}"
        );
    }
}

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

const MINIMAX_FAMILY: [(Protocol, &str); 3] = [
    (Protocol::MiniMax, "gkminimax"),
    (Protocol::MiniMaxEn, "gkminimaxen"),
    (Protocol::MinimaxCoding, "gkminimaxcp"),
];

/// issue 77: all MiniMax family members use the official OpenAI-wire thinking parameter.
#[tokio::test]
async fn minimax_family_openai_endpoint_disables_thinking_with_official_param() {
    for (pt, gk) in MINIMAX_FAMILY {
        let body = outbound_body_for_model(pt.clone(), Protocol::OpenAI, gk, "MiniMax-M3").await;
        assert_eq!(
            body.get("thinking"),
            Some(&serde_json::json!({"type": "disabled"})),
            "{pt:?} OpenAI endpoint must use thinking.type=disabled: {body}"
        );
        assert!(body.get("reasoning_effort").is_none(), "{pt:?} must not use reasoning_effort=none: {body}");
        assert!(body.get("disable_thinking").is_none(), "{pt:?} private field must be removed: {body}");
    }
}

/// The MiniMax M3.1-Flash registry entry is forced-thinking; disable params must be stripped.
#[tokio::test]
async fn minimax_forced_thinking_model_strips_disable_params() {
    let body = outbound_body_for_model(
        Protocol::MiniMax,
        Protocol::OpenAI,
        "gkminimaxforced",
        "MiniMax-M3.1-Flash",
    )
    .await;
    assert!(body.get("thinking").is_none(), "forced model rejects thinking disable: {body}");
    assert!(body.get("reasoning_effort").is_none(), "forced model rejects reasoning_effort=none: {body}");
    assert!(body.get("disable_thinking").is_none(), "private field must be removed: {body}");
}

/// Anthropic endpoint already uses the official Messages thinking shape and remains unchanged.
#[tokio::test]
async fn minimax_family_anthropic_endpoint_disables_thinking() {
    for (pt, gk) in MINIMAX_FAMILY {
        let body = outbound_body_for_model(pt.clone(), Protocol::Anthropic, gk, "MiniMax-M3").await;
        assert_eq!(body["thinking"], serde_json::json!({"type": "disabled"}), "{pt:?}: {body}");
        assert!(body.get("disable_thinking").is_none(), "{pt:?}: {body}");
    }
}

/// Every family member accepts both configured wire endpoints through the local fake upstream.
#[tokio::test]
async fn minimax_family_converts_requests_for_both_endpoint_protocols() {
    for (pt, gk) in MINIMAX_FAMILY {
        for wire in [Protocol::OpenAI, Protocol::Anthropic] {
            let body = outbound_body_for_model(pt.clone(), wire.clone(), gk, "MiniMax-M3").await;
            assert_eq!(body["model"], "MiniMax-M3", "{pt:?}/{wire:?}: {body}");
            assert!(body.get("messages").is_some(), "{pt:?}/{wire:?}: {body}");
            if wire == Protocol::Anthropic {
                assert_eq!(body["max_tokens"], 32, "{pt:?}/{wire:?}: {body}");
            }
        }
    }
}

const MINIMAX_ANTHROPIC_RESP: &str = r#"{"id":"msg_minimax","type":"message","role":"assistant","model":"MiniMax-M3","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":2,"cache_creation_input_tokens":1}}"#;
const MINIMAX_ANTHROPIC_SSE: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_minimax\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"MiniMax-M3\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
const MINIMAX_ANTHROPIC_ERROR: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad request"}}"#;

/// Exercise Anthropic responses, including cached token usage, through the proxy for each family member.
#[tokio::test]
async fn minimax_family_anthropic_endpoint_proxy_response_usage_stream_and_error() {
    for (pt, gk) in MINIMAX_FAMILY {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            gk,
            "/chat/completions",
            r#"{"model":"MiniMax-M3","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
            (200, "application/json", MINIMAX_ANTHROPIC_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}: {hits:?}");
        assert!(resp.contains("\"prompt_tokens\":6"), "{pt:?}: {resp}");
        assert!(resp.contains("\"completion_tokens\":3"), "{pt:?}: {resp}");
        assert!(resp.contains("\"cached_tokens\":2"), "{pt:?}: {resp}");

        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            gk,
            "/chat/completions",
            r#"{"model":"MiniMax-M3","max_tokens":32,"stream":true,"stream_options":{"include_usage":true},"messages":[{"role":"user","content":"hi"}]}"#,
            (200, "text/event-stream", MINIMAX_ANTHROPIC_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}: {hits:?}");
        assert!(resp.contains("\"content\":\"hi\""), "{pt:?}: {resp}");
        assert!(resp.contains("\"cached_tokens\":2"), "{pt:?}: {resp}");
        assert!(resp.contains("[DONE]"), "{pt:?}: {resp}");

        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            gk,
            "/chat/completions",
            r#"{"model":"MiniMax-M3","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
            (400, "application/json", MINIMAX_ANTHROPIC_ERROR),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
        assert!(!hits.is_empty());
        assert!(resp.contains("invalid_request_error"), "{resp}");
    }
}

/// MiniMax docs checked 2026-11-18:
/// - https://platform.minimaxi.com/docs/api-reference/text-chat-openai.md (stream chunks, finish_reason, usage)
/// - https://platform.minimaxi.com/docs/api-reference/text-chat-anthropic.md (message_stop terminal event)
/// - https://platform.minimaxi.com/docs/api-reference/errorcode.md (base_resp application codes)
///
/// The docs do not map code 2022 to HTTP; this synthetic fixture supplies 400 to test existing behavior only.
const MINIMAX_OPENAI_RESP: &str = r#"{"id":"chatcmpl_minimax","model":"MiniMax-M3","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":6,"completion_tokens":3,"total_tokens":9,"prompt_tokens_details":{"cached_tokens":2}}}"#;
const MINIMAX_OPENAI_SSE: &str = "data: {\"id\":\"chatcmpl_minimax\",\"object\":\"chat.completion.chunk\",\"model\":\"MiniMax-M3\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n\
data: {\"id\":\"chatcmpl_minimax\",\"object\":\"chat.completion.chunk\",\"model\":\"MiniMax-M3\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":6,\"completion_tokens\":3,\"total_tokens\":9,\"prompt_tokens_details\":{\"cached_tokens\":2}}}\n\n\
data: [DONE]\n\n";
const MINIMAX_SYNTHETIC_BASE_RESP: &str =
    r#"{"base_resp":{"status_code":2022,"status_msg":"synthetic model error"}}"#;

/// OpenAI endpoint non-streaming response converts usage and request through the local upstream for each MiniMax member.
#[tokio::test]
async fn minimax_family_openai_endpoint_proxy_non_stream_response_usage() {
    for (pt, gk) in MINIMAX_FAMILY {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            gk,
            "/v1/messages",
            r#"{"model":"MiniMax-M3","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
            (200, "application/json", MINIMAX_OPENAI_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits.len(), 1, "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}: {hits:?}");
        let upstream: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(upstream["model"], "MiniMax-M3", "{pt:?}: {upstream}");
        assert_eq!(upstream["messages"][0]["content"], "hi", "{pt:?}: {upstream}");
        assert_eq!(upstream["max_tokens"], 32, "{pt:?}: {upstream}");
        let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(response["content"][0]["text"], "hi", "{pt:?}: {response}");
        assert_eq!(response["usage"]["input_tokens"], 6, "{pt:?}: {response}");
        assert_eq!(response["usage"]["output_tokens"], 3, "{pt:?}: {response}");
        assert_eq!(response["usage"]["cache_read_input_tokens"], 2, "{pt:?}: {response}");
    }
}

/// OpenAI endpoint streaming and synthetic error traverse the real proxy path for each MiniMax member.
#[tokio::test]
async fn minimax_family_openai_endpoint_proxy_stream_and_synthetic_error() {
    use aidog_adapter::platform::error::UpstreamErrorClass;

    for (pt, gk) in MINIMAX_FAMILY {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            gk,
            "/v1/messages",
            r#"{"model":"MiniMax-M3","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
            (200, "text/event-stream", MINIMAX_OPENAI_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}: {hits:?}");
        assert!(resp.contains("content_block_delta"), "{pt:?}: {resp}");
        assert!(resp.contains("\"text\":\"hi\""), "{pt:?}: {resp}");
        assert!(resp.contains("message_stop"), "{pt:?}: {resp}");
        assert!(resp.contains("\"cache_read_input_tokens\":2"), "{pt:?}: {resp}");

        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            gk,
            "/v1/messages",
            r#"{"model":"MiniMax-M3","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
            (400, "application/json", MINIMAX_SYNTHETIC_BASE_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{pt:?}: {resp}");
        assert!(!hits.is_empty(), "{pt:?}: fake upstream must receive request");
        assert!(resp.contains("2022"), "{pt:?}: {resp}");
        assert_eq!(
            aidog_adapter::platform::adapter_for("minimax")
                .classify_error(400, MINIMAX_SYNTHETIC_BASE_RESP),
            UpstreamErrorClass::Unknown,
            "{pt:?}: synthetic status code must retain base classification"
        );
    }
}

// ══ issue 92：AtlasCloud Anthropic / OpenAI endpoints ══
// Official docs checked 2026-11-24:
// https://www.atlascloud.ai/docs/llm-protocols
// https://www.atlascloud.ai/docs/createChatCompletion
// https://www.atlascloud.ai/docs/errors
const ATLASCLOUD_ANTHROPIC_RESP: &str = r#"{"id":"msg_atlas","type":"message","role":"assistant","model":"atlas-test","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":2,"cache_creation_input_tokens":1}}"#;
const ATLASCLOUD_ANTHROPIC_SSE: &str = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_atlas","type":"message","role":"assistant","model":"atlas-test","content":[],"usage":{"input_tokens":6,"output_tokens":0,"cache_read_input_tokens":2,"cache_creation_input_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3}}

event: message_stop
data: {"type":"message_stop"}

"#;
const ATLASCLOUD_ANTHROPIC_ERROR: &str =
    r#"{"code":422,"msg":"model not found","request_id":"req-atlas-anthropic","data":null}"#;
const ATLASCLOUD_OPENAI_RESP: &str = r#"{"id":"chatcmpl_atlas","object":"chat.completion","model":"atlas-test","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"prompt_tokens_details":{"cached_tokens":4}}}"#;
const ATLASCLOUD_OPENAI_SSE: &str = r#"data: {"id":"chatcmpl_atlas","object":"chat.completion.chunk","model":"atlas-test","choices":[{"index":0,"delta":{"content":"hi"}}]}

data: {"id":"chatcmpl_atlas","object":"chat.completion.chunk","model":"atlas-test","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"prompt_tokens_details":{"cached_tokens":4}}}

data: [DONE]

"#;
const ATLASCLOUD_OPENAI_ERROR: &str =
    r#"{"code":422,"msg":"model not found","request_id":"req-atlas-openai","data":null}"#;

#[tokio::test]
async fn atlascloud_anthropic_endpoint_proxy_conversion_response_stream_error_and_usage() {
    use aidog_adapter::platform::adapter_for;

    let (status, resp, hits) = glm_member_call(
        Protocol::AtlasCloud,
        Protocol::Anthropic,
        "gk92an",
        "/chat/completions",
        r#"{"model":"atlas-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", ATLASCLOUD_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    let request: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["max_tokens"], 32, "{request}");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(
        response["choices"][0]["message"]["content"], "hi",
        "{response}"
    );
    assert!(response.get("usage").is_some(), "{response}");
    let (input, output, cache_read, cache_write, cache_write_1h, _) =
        adapter_for("atlascloud").extract_usage(ATLASCLOUD_ANTHROPIC_RESP);
    assert_eq!(
        (input, output, cache_read, cache_write, cache_write_1h),
        (6, 3, 2, 1, 0)
    );

    let (status, resp, hits) = glm_member_call(
        Protocol::AtlasCloud,
        Protocol::Anthropic,
        "gk92ans",
        "/chat/completions",
        r#"{"model":"atlas-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "text/event-stream", ATLASCLOUD_ANTHROPIC_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert!(resp.contains("\"content\":\"hi\""), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");

    let (status, resp, hits) = glm_member_call(
        Protocol::AtlasCloud,
        Protocol::Anthropic,
        "gk92ane",
        "/chat/completions",
        r#"{"model":"atlas-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (422, "application/json", ATLASCLOUD_ANTHROPIC_ERROR),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert!(!hits.is_empty(), "fake upstream must receive request");
    assert!(resp.contains("model not found"), "{resp}");
}

#[tokio::test]
async fn atlascloud_openai_endpoint_proxy_conversion_response_stream_error_and_usage() {
    use aidog_adapter::platform::adapter_for;

    let (status, resp, hits) = glm_member_call(
        Protocol::AtlasCloud,
        Protocol::OpenAI,
        "gk92oa",
        "/v1/messages",
        r#"{"model":"atlas-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", ATLASCLOUD_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let request: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["model"], "atlas-test", "{request}");
    assert_eq!(request["messages"][0]["content"], "hi", "{request}");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(response["content"][0]["text"], "hi", "{response}");
    assert!(response.get("usage").is_some(), "{response}");
    let (input, output, cache_read, cache_write, cache_write_1h, _) =
        adapter_for("atlascloud").extract_usage(ATLASCLOUD_OPENAI_RESP);
    assert_eq!(
        (input, output, cache_read, cache_write, cache_write_1h),
        (6, 3, 4, 0, 0)
    );

    let (status, resp, hits) = glm_member_call(
        Protocol::AtlasCloud,
        Protocol::OpenAI,
        "gk92oas",
        "/v1/messages",
        r#"{"model":"atlas-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "text/event-stream", ATLASCLOUD_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    assert!(resp.contains("text_delta") && resp.contains("hi"), "{resp}");
    assert!(resp.contains("message_stop"), "{resp}");
    assert!(
        !resp.contains("[DONE]"),
        "OpenAI upstream terminator must not leak into Anthropic output: {resp}"
    );

    let (status, resp, hits) = glm_member_call(
        Protocol::AtlasCloud,
        Protocol::OpenAI,
        "gk92oae",
        "/v1/messages",
        r#"{"model":"atlas-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (422, "application/json", ATLASCLOUD_OPENAI_ERROR),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert!(!hits.is_empty(), "fake upstream must receive request");
    assert!(resp.contains("model not found"), "{resp}");
}

// ══ issue 95：DeepSeek（deepseek）逐端点协议护栏（请求转换 / 非流式 / 流式 / 错误 / usage）══
// 官方出处（核对日期 2026-11-24）：
// - Chat Completions https://api-docs.deepseek.com/api/create-chat-completion
//   （`/chat/completions`、usage required 含 prompt_cache_hit_tokens / prompt_cache_miss_tokens、
//   `data: [DONE]` 结束、末 chunk 搭载 usage、思考增量 delta.reasoning_content）
// - Anthropic API https://api-docs.deepseek.com/guides/anthropic_api
//   （base_url `https://api.deepseek.com/anthropic`、标准事件链不发 [DONE]、
//   usage.server_tool_use {web_search_requests, tool_search}、错误事件
//   `{"type":"error","error":{"type","message"}}`）
const DEEPSEEK_OPENAI_RESP: &str = r#"{"id":"ds-1","object":"chat.completion","model":"deepseek-flash","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"prompt_cache_hit_tokens":30,"prompt_cache_miss_tokens":70,"prompt_tokens_details":{"cached_tokens":30}}}"#;
const DEEPSEEK_OPENAI_SSE: &str = r#"data: {"id":"ds-1","object":"chat.completion.chunk","model":"deepseek-flash","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}],"usage":null}

data: {"id":"ds-1","object":"chat.completion.chunk","model":"deepseek-flash","choices":[{"index":0,"delta":{"reasoning_content":"think"},"finish_reason":null}],"usage":null}

data: {"id":"ds-1","object":"chat.completion.chunk","model":"deepseek-flash","choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}],"usage":null}

data: {"id":"ds-1","object":"chat.completion.chunk","model":"deepseek-flash","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"prompt_cache_hit_tokens":30,"prompt_cache_miss_tokens":70}}

data: [DONE]
"#;
const DEEPSEEK_OPENAI_ERROR: &str =
    r#"{"error":{"message":"Invalid Parameters: deepseek-x is not a valid model name","type":"invalid_request_error"}}"#;
const DEEPSEEK_ANTHROPIC_RESP: &str = r#"{"id":"msg_ds1","type":"message","role":"assistant","model":"deepseek-flash","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"cache_read_input_tokens":2,"cache_creation_input_tokens":0,"output_tokens":3}}"#;
const DEEPSEEK_ANTHROPIC_SSE: &str = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_ds1","type":"message","role":"assistant","model":"deepseek-flash","content":[],"usage":{"input_tokens":5,"output_tokens":0}}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3,"server_tool_use":{"web_search_requests":1,"tool_search":1}}}

event: message_stop
data: {"type":"message_stop"}

"#;
const DEEPSEEK_ANTHROPIC_ERROR: &str =
    r#"{"type":"error","error":{"type":"invalid_request_error","message":"model not found deepseek-x"}}"#;

#[tokio::test]
async fn deepseek_openai_endpoint_proxy_conversion_response_stream_error_and_usage() {
    use aidog_adapter::platform::adapter_for;

    let (status, resp, hits) = glm_member_call(
        Protocol::DeepSeek,
        Protocol::OpenAI,
        "gk95oa",
        "/v1/messages",
        r#"{"model":"deepseek-flash","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", DEEPSEEK_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let request: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["model"], "deepseek-flash", "{request}");
    assert_eq!(request["messages"][0]["content"], "hi", "{request}");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(response["content"][0]["text"], "hi", "{response}");
    assert!(response.get("usage").is_some(), "{response}");
    let (input, output, cache_read, cache_write, cache_write_1h, _) =
        adapter_for("deepseek").extract_usage(DEEPSEEK_OPENAI_RESP);
    assert_eq!(
        (input, output, cache_read, cache_write, cache_write_1h),
        (70, 20, 30, 0, 0)
    );

    let (status, resp, hits) = glm_member_call(
        Protocol::DeepSeek,
        Protocol::OpenAI,
        "gk95oas",
        "/v1/messages",
        r#"{"model":"deepseek-flash","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "text/event-stream", DEEPSEEK_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    assert!(
        resp.contains("text_delta") && resp.contains("hi"),
        "{resp}"
    );
    assert!(resp.contains("message_stop"), "{resp}");
    assert!(
        !resp.contains("[DONE]"),
        "OpenAI upstream terminator must not leak into Anthropic output: {resp}"
    );

    let (status, resp, hits) = glm_member_call(
        Protocol::DeepSeek,
        Protocol::OpenAI,
        "gk95oae",
        "/v1/messages",
        r#"{"model":"deepseek-flash","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (422, "application/json", DEEPSEEK_OPENAI_ERROR),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert!(!hits.is_empty(), "fake upstream must receive request");
    assert!(resp.contains("not a valid model name"), "{resp}");
}

#[tokio::test]
async fn deepseek_anthropic_endpoint_proxy_conversion_response_stream_error_and_usage() {
    use aidog_adapter::platform::adapter_for;

    let (status, resp, hits) = glm_member_call(
        Protocol::DeepSeek,
        Protocol::Anthropic,
        "gk95an",
        "/chat/completions",
        r#"{"model":"deepseek-flash","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", DEEPSEEK_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    let request: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["max_tokens"], 32, "{request}");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(
        response["choices"][0]["message"]["content"], "hi",
        "{response}"
    );
    assert!(response.get("usage").is_some(), "{response}");
    let (input, output, cache_read, cache_write, cache_write_1h, tools) =
        adapter_for("deepseek").extract_usage(DEEPSEEK_ANTHROPIC_RESP);
    assert_eq!(
        (input, output, cache_read, cache_write, cache_write_1h),
        (5, 3, 2, 0, 0)
    );
    assert!(tools.is_empty(), "non-stream fixture has no server_tool_use");

    let (status, resp, hits) = glm_member_call(
        Protocol::DeepSeek,
        Protocol::Anthropic,
        "gk95ans",
        "/chat/completions",
        r#"{"model":"deepseek-flash","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "text/event-stream", DEEPSEEK_ANTHROPIC_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert!(resp.contains("\"content\":\"hi\""), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");

    let (status, resp, hits) = glm_member_call(
        Protocol::DeepSeek,
        Protocol::Anthropic,
        "gk95ane",
        "/chat/completions",
        r#"{"model":"deepseek-flash","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (400, "application/json", DEEPSEEK_ANTHROPIC_ERROR),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(!hits.is_empty(), "fake upstream must receive request");
    assert!(resp.contains("model not found deepseek-x"), "{resp}");
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

#[tokio::test]
async fn gemini_local_upstream_request_and_response() {
    let response = r#"{"candidates":[{"content":{"role":"model","parts":[{"text":"hi"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":3,"cachedContentTokenCount":4}}"#;
    let (status, body, hits) = glm_member_call(
        Protocol::Gemini,
        Protocol::Gemini,
        "gk96gem",
        "/v1/messages",
        r#"{"model":"gemini-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", response),
    ).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(hits[0].0.contains(":generateContent"), "{:?}", hits[0].0);
    let request: Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["contents"][0]["parts"][0]["text"], "hi");
    let response: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(response["content"][0]["text"], "hi", "{response}");
    let usage = aidog_adapter::platform::adapter_for("gemini").extract_usage(
        r#"{"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":3,"cachedContentTokenCount":4}}"#,
    );
    assert_eq!((usage.0, usage.1, usage.2), (6, 3, 4));

    let (status, body, hits) = glm_member_call(
        Protocol::Gemini, Protocol::Gemini, "gk96stream", "/v1/messages",
        r#"{"model":"gemini-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "text/event-stream", "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"hi\"}]},\"finishReason\":\"STOP\"}]}\n\n"),
    ).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(hits[0].0.contains(":streamGenerateContent"), "{:?}", hits[0].0);
    assert!(body.contains("hi"), "{body}");

    let (status, body, hits) = glm_member_call(
        Protocol::Gemini, Protocol::Gemini, "gk96error", "/v1/messages",
        r#"{"model":"gemini-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (400, "application/json", r#"{"error":{"code":400,"message":"invalid request","status":"INVALID_ARGUMENT"}}"#),
    ).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(!hits.is_empty());
    assert!(body.contains("invalid request"), "{body}");
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
    pa.inject_coding_plan(&mut body, &Protocol::GlmCoding, &Protocol::OpenAI);
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
    pa.inject_coding_plan(&mut body, &Protocol::GlmCodingEn, &Protocol::OpenAI);
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
/// openai 端点禁思考写官方 `thinking` 的成员（kimi_coding 写 `reasoning_effort`，见成员 issue 119）。
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

/// issue 119（先红；review R5）：kimi_coding 的 openai 端点禁思考写官方认收的
/// `reasoning_effort: "none"`，不写基座第三方端点用的 vLLM 式 `chat_template_kwargs`
/// （Kimi Code 文档无此参数），也不写 `thinking`；temperature 无官方固定值约束，原样透传。
/// K3.1 Pro 系列（k3.1-pro / k3.1-pro-highspeed）不支持关闭思考，禁思考请求必须原样携带
/// `reasoning_effort: "none"` 让上游按官方返回 400，不在适配器静默剥除后按默认思考处理
/// （review R4 blocking：剥除会把禁思考请求静默转换成默认思考请求）。
/// 出处（核对日期 2026-11-17，本地留档 .tmp/issue-119/docs/）：
/// https://www.kimi.com/code/docs/kimi-code/models.html「off / none / disabled … K3.1 Pro 系列：
/// 关闭思考的请求会被拒绝（HTTP 400）；K3 系列、K2.8 Preview 与 K2.7 Code Highspeed：关闭
/// thinking 后，请求均由 K2.8 Preview（无思考）处理」「其他未知取值返回 HTTP 400」；
/// https://www.kimi.com/code/docs/kimi-code/error-reference.html「error, status code: 400,
/// message: K3.1 Pro does not support disabled thinking mode. Please enable thinking and try
/// again, or choose another model.」；https://platform.kimi.com/docs/api/chat.md「Kimi K3 系列
/// 始终开启思考与 Preserved Thinking，使用顶层 `reasoning_effort`（支持 "low"/"high"/"max"，
/// 默认 "max"），不使用 `thinking` 参数」。
#[tokio::test]
async fn kimi_coding_openai_disable_thinking_writes_reasoning_effort_none() {
    let pt = Protocol::KimiCoding;
    for model in ["kimi-test", "k3.1-pro", "k3.1-pro-highspeed"] {
        let req = serde_json::json!({"model": model, "max_tokens": 32, "temperature": 1.0,
            "disable_thinking": true, "messages": [{"role": "user", "content": "hi"}]});
        let body = outbound_body_with(pt.clone(), Protocol::OpenAI, &kimi_gk(&pt, "cbb"), req).await;
        assert_eq!(
            body.get("reasoning_effort"),
            Some(&serde_json::json!("none")),
            "{model} 禁思考应写 reasoning_effort=none: {body}"
        );
        assert!(body.get("chat_template_kwargs").is_none(), "kimi_coding 不应写 chat_template_kwargs: {body}");
        assert!(body.get("thinking").is_none(), "kimi_coding 不应写 thinking: {body}");
        assert_eq!(body.get("temperature"), Some(&serde_json::json!(1.0)), "kimi_coding 应透传 temperature: {body}");
        assert!(body.get("disable_thinking").is_none(), "私有字段应剥除: {body}");
    }
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

/// Issue 119 follow-up: ambiguous model-or-permission wording must not be a shared model-only marker.
#[test]
fn ambiguous_model_or_permission_error_remains_unknown() {
    use aidog_adapter::platform::error::UpstreamErrorClass::Unknown;
    let body = r#"{"error":{"message":"Not found the model kimi-for-coding or Permission denied"}}"#;
    for code in ["kimi_coding", "anthropic"] {
        assert_eq!(aidog_adapter::platform::adapter_for(code).classify_error(404, body), Unknown);
    }
}

/// Issue 119 follow-up: same-protocol OpenAI passthrough preserves reasoning history and derives repeatable session keys.
#[tokio::test]
async fn kimi_coding_openai_passthrough_preserves_history_and_session_key() {
    const IN: &str = r#"{"model":"kimi-for-coding","metadata":{"session_id":"session-42"},"messages":[{"role":"assistant","reasoning_content":"prior thought","tool_calls":[{"id":"call-1","type":"function","function":{"name":"lookup","arguments":"{}"}}]},{"role":"tool","tool_call_id":"call-1","content":"result"},{"role":"user","content":"continue"}]}"#;
    let (status, _, first_hits) = glm_member_call(
        Protocol::KimiCoding,
        Protocol::OpenAI,
        "gk119pass1",
        "/v1/chat/completions",
        IN,
        (200, "application/json", KIMI_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, second_hits) = glm_member_call(
        Protocol::KimiCoding,
        Protocol::OpenAI,
        "gk119pass2",
        "/v1/chat/completions",
        IN,
        (200, "application/json", KIMI_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let first: serde_json::Value = serde_json::from_str(&first_hits[0].1).unwrap();
    let second: serde_json::Value = serde_json::from_str(&second_hits[0].1).unwrap();
    assert_eq!(first["messages"][0]["reasoning_content"], "prior thought");
    assert!(first["prompt_cache_key"].as_str().unwrap().ends_with("session-42"));
    assert_eq!(first["prompt_cache_key"], second["prompt_cache_key"]);
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
/// `"required"`），不降级；anthropic wire 不在范围；kimi_coding 无已证实约束不降级（成员 issue 119）。
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
    // kimi_coding 无已证实约束，不降级（成员 issue 119）。
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
    kimi_member_outbound_pt(Protocol::Kimi, gk, coding_plan).await
}

/// 同 [`kimi_member_outbound`]，平台协议按参指定（issue 118 成员 kimi_en 复用）。
async fn kimi_member_outbound_pt(pt: Protocol, gk: &str, coding_plan: bool) -> serde_json::Value {
    kimi_member_outbound_raw(pt, Protocol::OpenAI, gk, coding_plan, GLM_ANTHROPIC_IN).await
}

/// 同 [`kimi_member_outbound_pt`]，端点 wire 协议与入站 body 可指定
/// （review R2：anthropic coding_plan 端点与多轮工具历史用例复用）。
async fn kimi_member_outbound_raw(
    pt: Protocol,
    ep: Protocol,
    gk: &str,
    coding_plan: bool,
    in_body: &str,
) -> serde_json::Value {
    let resp_ok = if ep == Protocol::Anthropic { ANTHROPIC_OK } else { KIMI_OPENAI_RESP };
    let (base, hits) = spawn_glm_upstream(200, "application/json", resp_ok).await;
    let state = make_state(test_db().await).await;
    let pid = aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        pt,
        &base,
        std::slice::from_ref(&ep),
    )
    .await;
    // fixture 恒 coding_plan:false，按用例直写 DB 覆盖端点
    let eps = aidog_db::serialize_endpoints(&[PlatformEndpoint {
        protocol: ep,
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
        .body(Body::from(in_body.to_string()))
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
    let key = body.get("prompt_cache_key").and_then(|v| v.as_str()).unwrap();
    assert!(key.starts_with("aidog-glm-test-"), "coding_plan 端点应注入 prompt_cache_key: {body}");
    assert_eq!(key.rsplit('-').next().unwrap().len(), 6, "标准 kimi 保留旧六位五分钟键: {body}");

    let first = kimi_member_outbound_pt(Protocol::KimiCoding, "gk119a", true).await;
    let second = kimi_member_outbound_pt(Protocol::KimiCoding, "gk119b", true).await;
    let first_key = first.get("prompt_cache_key").and_then(|v| v.as_str()).unwrap();
    let second_key = second.get("prompt_cache_key").and_then(|v| v.as_str()).unwrap();
    assert_ne!(first_key, second_key, "无会话身份的请求不得共享缓存键: {first_key}");

    let body = kimi_member_outbound("gk117cn", false).await;
    assert!(
        body.get("prompt_cache_key").is_none(),
        "非 coding_plan 端点不应注入 prompt_cache_key: {body}"
    );
}

// ══ issue 118：Kimi 国际版（月之暗面）成员 `kimi_en` 护栏 ══
// 官方出处（核对日期 2026-11-17，国际站 platform.kimi.ai 原文，本地留档 .tmp/issue-118/docs/）：
// - Chat Completions https://platform.kimi.ai/docs/api/chat.md
//   （curl 示例 `https://api.moonshot.ai/v1/chat/completions`；prompt_cache_key「For Kimi Code Plan,
//   this field is required」）
// - Messages https://platform.kimi.ai/docs/api/messages.md（「simply point the base URL at
//   `https://api.moonshot.ai/anthropic`」）
// - models-overview https://platform.kimi.ai/docs/api/models-overview.md（kimi-k2.6「Thinking is on by
//   default」/「Passing `"required"` returns an error」；K3 `thinking`「Not supported」）
// - 错误码 https://platform.kimi.ai/docs/api/errors.md（404 `resource_not_found_error`）
// 思考 / tool_choice / temperature / usage / 流式 / 错误体逐项核对结论同家族基座（出处见
// kimi_family_* 各测试头注与 registry.rs 成员节，kimi_en 均在家族循环内），本成员无代码差异，
// 登记 KimiFamilyAdapter；本节补端到端护栏：kimi_en 走用户配置端点、家族 openai 臂覆写生效、
// 不注入 prompt_cache_key（非编码套餐，基座按平台协议分派对 KimiEn 为 no-op）。

/// issue 118（护栏）：kimi_en 端到端——anthropic 入站经用户配置端点出站为 openai chat/completions
/// 形状（家族基座转换）、不注入 prompt_cache_key、禁思考写官方 `thinking.type=disabled` 且剥
/// temperature（家族 openai 臂覆写对成员生效）。
#[tokio::test]
async fn kimi_en_member_outbound_matches_family_base_without_prompt_cache_key() {
    let body = kimi_member_outbound_pt(Protocol::KimiEn, "gk118en", false).await;
    assert!(
        body.get("prompt_cache_key").is_none(),
        "kimi_en 不应注入 prompt_cache_key: {body}"
    );
    assert!(
        body.get("messages").is_some(),
        "应经家族基座转换为 openai 形状: {body}"
    );
    let mut req = serde_json::json!({"model": "kimi-k2.6", "max_tokens": 32, "temperature": 1.0,
        "disable_thinking": true, "messages": [{"role": "user", "content": "hi"}]});
    let body = outbound_body_with(
        Protocol::KimiEn,
        Protocol::OpenAI,
        "gk118ent",
        req.clone(),
    )
    .await;
    assert_eq!(body.get("thinking"), Some(&serde_json::json!({"type": "disabled"})), "{body}");
    assert!(body.get("temperature").is_none(), "禁思考应剥 temperature: {body}");
    req.as_object_mut().unwrap().remove("disable_thinking");
    let body = outbound_body_with(Protocol::KimiEn, Protocol::OpenAI, "gk118enk", req).await;
    assert_eq!(body.get("temperature"), Some(&serde_json::json!(1.0)), "未禁思考应透传: {body}");
}

// ══ issue 119：Kimi 编程套餐（月之暗面会员）成员 `kimi_coding` 护栏 ══
// 官方出处（核对日期 2026-11-17，本地留档 .tmp/issue-119/docs/）：
// - Kimi Code 模型配置 https://www.kimi.com/code/docs/kimi-code/models.html
//   （编码端点 Base URL 国内 `https://api.kimi.com/coding/v1`（openai）/ `https://api.kimi.com/coding/`
//   （anthropic）；模型 k3 / k3-256k / kimi-for-coding / kimi-for-coding-highspeed；effort 映射表
//   「off / none / disabled … 关闭 thinking 后，请求均由 K2.8 Preview（无思考）处理」「K3.1 Pro 系列：
//   关闭思考的请求会被拒绝（HTTP 400）」「其他未知取值 返回 HTTP 400」）
// - 错误参考 https://www.kimi.com/code/docs/kimi-code/error-reference.html
//   （错误条目均为 `error, status code: <code>, message: <text>` 文案，JSON 错误体形状未证实）
// - Chat Completions https://platform.kimi.com/docs/api/chat.md
//   （prompt_cache_key「对于 Kimi Code Plan，此字段为必填以提高缓存命中率」；K3「使用顶层
//   `reasoning_effort` … 不使用 `thinking` 参数」）

/// issue 119（先红）：kimi_coding 的 coding_plan 端点出站注入 `prompt_cache_key`
/// （chat.md「对于 Kimi Code Plan，此字段为必填以提高缓存命中率」，aidog- 前缀会话键）；
/// 非 coding_plan 端点不注入（家族按平台协议分派，注入条件同源：端点 coding_plan flag）。
#[tokio::test]
async fn kimi_coding_member_coding_plan_endpoint_injects_prompt_cache_key() {
    let body = kimi_member_outbound_pt(Protocol::KimiCoding, "gk119cd", true).await;
    assert!(
        body.get("prompt_cache_key")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.starts_with("aidog-")),
        "kimi_coding coding_plan 端点应注入 prompt_cache_key: {body}"
    );
    let body = kimi_member_outbound_pt(Protocol::KimiCoding, "gk119cn", false).await;
    assert!(
        body.get("prompt_cache_key").is_none(),
        "非 coding_plan 端点不应注入 prompt_cache_key: {body}"
    );
}

/// review R2-2（issue 119）：anthropic wire 的 coding_plan 端点不注入 prompt_cache_key。
/// messages.md（Anthropic Messages，https://platform.kimi.com/docs/api/messages.md）全篇
/// 未列 prompt_cache_key（缓存走 `cache_control`）；chat.md 的「必填」仅限 Chat Completions。
/// kimi_coding 的 anthropic 端点同为 coding_plan，注入会把未文档化参数送进 Messages 体。
#[tokio::test]
async fn kimi_coding_anthropic_coding_plan_endpoint_no_prompt_cache_key() {
    let body = kimi_member_outbound_raw(
        Protocol::KimiCoding,
        Protocol::Anthropic,
        "gk119ca",
        true,
        GLM_ANTHROPIC_IN,
    )
    .await;
    assert!(
        body.get("prompt_cache_key").is_none(),
        "anthropic wire coding_plan 端点不应注入 prompt_cache_key: {body}"
    );
}

/// review R2-1（issue 119）：kimi_coding openai 端点多轮工具历史携带 reasoning_content。
/// 官方错误参考 400「thinking is enabled but reasoning_content is missing in assistant
/// tool call message」；anthropic 客户端回灌的 thinking 块在 openai 转换中被跳过，
/// assistant tool-call 消息缺 reasoning_content → 上游 400。
/// https://www.kimi.com/code/docs/kimi-code/error-reference.html
#[tokio::test]
async fn kimi_coding_openai_tool_history_carries_reasoning_content() {
    let history = r#"{"model":"glm-test","max_tokens":64,"messages":[
        {"role":"user","content":"北京天气?"},
        {"role":"assistant","content":[
            {"type":"thinking","thinking":"需要调用天气工具","signature":"sig"},
            {"type":"tool_use","id":"tu1","name":"get_weather","input":{"city":"北京"}}]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"晴"}]}]}"#;
    let body = kimi_member_outbound_raw(
        Protocol::KimiCoding,
        Protocol::OpenAI,
        "gk119mr",
        true,
        history,
    )
    .await;
    let asst = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant" && m.get("tool_calls").is_some_and(|v| !v.is_null()))
        .expect("出站应含 assistant tool-call 消息");
    assert_eq!(
        asst["reasoning_content"],
        "需要调用天气工具",
        "assistant tool-call 历史应携带 reasoning_content: {asst}"
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

/// 百度千帆家族（issue 80）：适配器经本地假上游覆盖家族涉及的全部内置端点协议，
/// 断言实际出站路径与请求体（qianfan 内置 anthropic 单端点；qianfan_coding 内置
/// openai + anthropic 双端点）。openai wire 禁思考写官方顶层 `thinking.type=disabled`
/// （文本生成 API 默认值，https://cloud.baidu.com/doc/qianfan-api/s/3m7of64lb ）。
#[tokio::test]
async fn qianfan_disable_thinking_reaches_local_upstream_for_each_wire() {
    let cases = [
        ("qianfan", Protocol::QianFan, Protocol::Anthropic, "/v1/messages", ANTHROPIC_OK),
        ("qianfan_coding", Protocol::QianfanCoding, Protocol::OpenAI, "/chat/completions", OPENAI_OK),
        (
            "qianfan_coding",
            Protocol::QianfanCoding,
            Protocol::Anthropic,
            "/v1/messages",
            ANTHROPIC_OK,
        ),
    ];

    for (i, (code, platform_type, wire, expected_path, response)) in cases.into_iter().enumerate() {
        let (upstream, hits) = spawn_recording_upstream(response).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gkqf{i}");
        setup_user_endpoint_group(&state, &gk, wire.clone(), &upstream).await;
        let platform = aidog_db::list_platforms(&state.db)
            .await
            .unwrap()
            .into_iter()
            .find(|p| p.name == "stub")
            .unwrap();
        let protocol = serde_json::to_string(&platform_type).unwrap();
        state
            .db
            .call_platform_traced(None, std::panic::Location::caller(), move |conn| {
                conn.execute(
                    "UPDATE platform SET platform_type=?1 WHERE id=?2",
                    rusqlite::params![protocol, platform.id as i64],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        let request = HttpRequest::builder()
            .method("POST")
            .uri("/v1/messages")
            .header("authorization", format!("Bearer {gk}"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"model":"deepseek-v4-pro","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#,
            ))
            .unwrap();
        let resp = handle_proxy(AxumState(state.clone()), request).await;
        let resp_status = resp.status();
        if resp_status != StatusCode::OK {
            let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
            panic!(
                "code={code} wire={wire:?} status={resp_status} body={}",
                String::from_utf8_lossy(&body)
            );
        }
        flush_log_queue(&state).await;

        let hits = hits.lock().unwrap();
        assert_eq!(hits.len(), 1, "code={code} wire={wire:?}, hits={hits:?}");
        let (path, body) = &hits[0];
        assert_eq!(path, expected_path, "code={code} wire={wire:?}");
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["thinking"]["type"], "disabled", "code={code} wire={wire:?}: {body}");
        assert!(
            body.get("disable_thinking").is_none(),
            "code={code} wire={wire:?}: {body}"
        );
    }
}

// ══ issue 83：StepFun 家族（stepfun / stepfun_en）端点协议护栏（本地假上游）══
// 官方出处（核对日期 2026-11-23）：
// - Chat Completions https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create.md
//   （`reasoning_format` 默认 general → 思考在 message/delta.reasoning；
//   usage.prompt_tokens_details.cached_tokens；流式 `data: [DONE]` 结束）
// - Messages API https://platform.stepfun.com/docs/zh/api-reference/chat/messages-create.md
//   （官方参数表无 `thinking` 对象；思考深度 = `output_config.effort` 三档）
// - 错误码 https://platform.stepfun.com/docs/zh/api-reference/error-codes.md
//   （400 案由「模型不存在或无权限」；错误体 JSON 形状未在错误码页给出——按 OpenAI 兼容
//   惯例构造，字段未证实）

/// 官方响应形状（reasoning_format 默认 general：思考在 message.reasoning；含 cached_tokens）。
const STEPFUN_OPENAI_RESP: &str = r#"{"id":"chatcmpl-step1","object":"chat.completion","model":"step-5-preview","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"hi","reasoning":"think"}}],"usage":{"prompt_tokens":90,"completion_tokens":24,"total_tokens":114,"prompt_tokens_details":{"cached_tokens":40}}}"#;
/// 官方流式形状（general 格式 delta.reasoning → delta.content → finish + usage → `data: [DONE]`）。
const STEPFUN_OPENAI_SSE: &str = "data: {\"id\":\"chatcmpl-step1\",\"object\":\"chat.completion.chunk\",\"model\":\"step-5-preview\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning\":\"think\"}}]}\n\n\
data: {\"id\":\"chatcmpl-step1\",\"object\":\"chat.completion.chunk\",\"model\":\"step-5-preview\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n\
data: {\"id\":\"chatcmpl-step1\",\"object\":\"chat.completion.chunk\",\"model\":\"step-5-preview\",\"choices\":[{\"index\":0,\"finish_reason\":\"stop\",\"delta\":{}}],\"usage\":{\"prompt_tokens\":90,\"completion_tokens\":24,\"total_tokens\":114,\"prompt_tokens_details\":{\"cached_tokens\":40}}}\n\n\
data: [DONE]\n\n";
const STEPFUN_ANTHROPIC_RESP: &str = r#"{"id":"msg_step1","type":"message","role":"assistant","model":"step-5-preview","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":90,"output_tokens":24}}"#;
const STEPFUN_ANTHROPIC_SSE: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_step1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"step-5-preview\",\"content\":[],\"usage\":{\"input_tokens\":90,\"output_tokens\":0}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":24}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
const STEPFUN_ERR_400: &str = r#"{"error":{"message":"模型不存在或无权限","type":"request_params_invalid"}}"#;
const STEPFUN_ANTHROPIC_IN: &str =
    r#"{"model":"step-5-preview","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#;
const STEPFUN_ANTHROPIC_IN_STREAM: &str =
    r#"{"model":"step-5-preview","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#;

/// issue 83：`stepfun` openai 端点非流式——入站 anthropic 转 openai 打 `/chat/completions`；
/// general 格式 message.reasoning → thinking 块，cached_tokens → cache_read_input_tokens。
#[tokio::test]
async fn stepfun_openai_endpoint_non_stream_request_response_usage() {
    let (status, resp, hits) = glm_member_call(
        Protocol::StepFun,
        Protocol::OpenAI,
        "gk83on",
        "/v1/messages",
        STEPFUN_ANTHROPIC_IN,
        (200, "application/json", STEPFUN_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/chat/completions");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["model"], "step-5-preview", "{up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    let content = v["content"].as_array().unwrap();
    assert!(
        content.iter().any(|b| b["type"] == "thinking" && b["thinking"] == "think"),
        "{v}"
    );
    assert!(content.iter().any(|b| b["type"] == "text" && b["text"] == "hi"), "{v}");
    assert_eq!(v["usage"]["cache_read_input_tokens"], 40, "{v}");
    assert_eq!(v["usage"]["output_tokens"], 24, "{v}");
}

/// issue 83：`stepfun` openai 端点流式——general 格式 delta.reasoning → thinking_delta，
/// delta.content → text_delta，`[DONE]` 收尾 message_stop。
#[tokio::test]
async fn stepfun_openai_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::StepFun,
        Protocol::OpenAI,
        "gk83os",
        "/v1/messages",
        STEPFUN_ANTHROPIC_IN_STREAM,
        (200, "text/event-stream", STEPFUN_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    assert!(resp.contains("thinking_delta") && resp.contains("think"), "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("message_stop"), "{resp}");
}

/// issue 83：禁思考经 openai 端点出站——官方无关闭形态，出站不注入任何禁思考参数
/// （基座非官方 host 的 `chat_template_kwargs.enable_thinking=false` 不出现）。
#[tokio::test]
async fn stepfun_openai_endpoint_disable_thinking_injects_nothing() {
    let inbound = r#"{"model":"step-5-preview","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
    let (status, resp, hits) = glm_member_call(
        Protocol::StepFun,
        Protocol::OpenAI,
        "gk83od",
        "/v1/messages",
        inbound,
        (200, "application/json", STEPFUN_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert!(up.get("disable_thinking").is_none(), "{up}");
    assert!(up.get("enable_thinking").is_none(), "{up}");
    assert!(up.get("chat_template_kwargs").is_none(), "{up}");
    assert!(up.get("reasoning_effort").is_none(), "{up}");
    assert!(up.get("thinking").is_none(), "{up}");
}

/// issue 83：禁思考经 openai 端点 + openai 客户端透传流式——StepFun 默认
/// `reasoning_format=general` 的 `delta.reasoning` 思考帧被剥，正文与 `[DONE]` 保留
/// （覆盖 SseThinkingStripper 透传分支，与 anthropic 客户端转换分支的
/// `stepfun_normalize_reasoning` 互补）。
#[tokio::test]
async fn stepfun_openai_passthrough_stream_disable_thinking_strips_reasoning() {
    let inbound = r#"{"model":"step-5-preview","max_tokens":32,"stream":true,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
    let (status, resp, hits) = glm_member_call(
        Protocol::StepFun,
        Protocol::OpenAI,
        "gk83op",
        "/v1/chat/completions",
        inbound,
        (200, "text/event-stream", STEPFUN_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    assert!(!resp.contains("reasoning"), "思考帧被剥: {resp}");
    assert!(!resp.contains("think"), "思考内容不下发: {resp}");
    assert!(resp.contains("\"content\":\"hi\""), "正文保留: {resp}");
    assert!(resp.contains("[DONE]"), "终止哨兵保留: {resp}");
}

/// issue 83：禁思考经 anthropic 端点出站——`thinking` 认受性未证实保持框架默认
/// （`thinking.type=disabled`），官方 `output_config.effort` 思考档位剥除。
#[tokio::test]
async fn stepfun_anthropic_endpoint_disable_thinking_uses_default_shape() {
    let inbound = r#"{"model":"step-5-preview","max_tokens":32,"disable_thinking":true,"output_config":{"effort":"high"},"messages":[{"role":"user","content":"hi"}]}"#;
    let (status, resp, hits) = glm_member_call(
        Protocol::StepFun,
        Protocol::Anthropic,
        "gk83ad",
        "/v1/messages",
        inbound,
        (200, "application/json", STEPFUN_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["thinking"]["type"], "disabled", "{up}");
    assert!(up.get("disable_thinking").is_none(), "{up}");
}

/// issue 83：`stepfun` anthropic 端点非流式——入站 openai 转 anthropic 打 `/v1/messages`；
/// 响应回 openai 形状。
#[tokio::test]
async fn stepfun_anthropic_endpoint_non_stream_request_response() {
    let (status, resp, hits) = glm_member_call(
        Protocol::StepFun,
        Protocol::Anthropic,
        "gk83an",
        "/chat/completions",
        GLM_OPENAI_IN,
        (200, "application/json", STEPFUN_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(up["max_tokens"], 32, "{up}");
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "hi", "{v}");
    assert_eq!(v["usage"]["completion_tokens"], 24, "{v}");
}

/// issue 83：`stepfun` anthropic 端点流式——anthropic 事件转 openai chunk，结尾 `[DONE]`。
#[tokio::test]
async fn stepfun_anthropic_endpoint_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::StepFun,
        Protocol::Anthropic,
        "gk83as",
        "/chat/completions",
        GLM_OPENAI_IN_STREAM,
        (200, "text/event-stream", STEPFUN_ANTHROPIC_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert!(resp.contains("\"hi\""), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");
}

/// issue 83：`stepfun` 错误——400「模型不存在或无权限」归模型类，客户端拿到上游原状态码。
#[tokio::test]
async fn stepfun_openai_endpoint_error_body() {
    let pa = aidog_adapter::platform::adapter_for("stepfun");
    assert_eq!(
        pa.classify_error(400, STEPFUN_ERR_400),
        aidog_adapter::platform::error::UpstreamErrorClass::Model
    );
    let (status, resp, hits) = glm_member_call(
        Protocol::StepFun,
        Protocol::OpenAI,
        "gk83oe",
        "/v1/messages",
        STEPFUN_ANTHROPIC_IN,
        (400, "application/json", STEPFUN_ERR_400),
    )
    .await;
    assert!(!hits.is_empty());
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("模型不存在"), "{resp}");
}

// ── issue 84：腾讯云家族（tencent_coding / tencent_tokenhub / tencent_tokenhub_intl）──
// 家族取证（官方文档核对日期 2026-11-23）见 aidog_adapter platform::registry
// `TencentFamilyAdapter` 模块注释。本节全部经真实 gateway handle_proxy → 分组路由 →
// 用户端点 (protocol, base_url=本地假上游) → 协议转换 / 透传 → 上游。

/// TokenHub openai 非流式：reasoning_content 思考 + prompt_tokens_details.cached_tokens
/// 缓存（135872 usage 表；顶层 cache_read_tokens 与 prompt_tokens 包含关系未证实，不在断言内）。
const TC_OPENAI_RESP: &str = r#"{"id":"tc_1","created":1,"model":"tc-code-latest","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"hi","reasoning_content":"think"}}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"prompt_tokens_details":{"cached_tokens":4}}}"#;
/// TokenHub openai 流式：delta.reasoning_content → delta.content → 末帧 finish + usage → [DONE]。
const TC_OPENAI_SSE: &str = "data: {\"id\":\"tc_1\",\"created\":1,\"model\":\"tc-code-latest\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"think\"}}]}\n\n\
data: {\"id\":\"tc_1\",\"created\":1,\"model\":\"tc-code-latest\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n\
data: {\"id\":\"tc_1\",\"created\":1,\"model\":\"tc-code-latest\",\"choices\":[{\"index\":0,\"finish_reason\":\"stop\",\"delta\":{}}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":4}}}\n\n\
data: [DONE]\n\n";
/// TokenHub anthropic 非流式（135874：标准 Messages 形状 + cache 两件）。
const TC_ANTHROPIC_RESP: &str = r#"{"id":"msg_tc","type":"message","role":"assistant","model":"tc-code-latest","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":4,"cache_creation_input_tokens":2}}"#;
/// TokenHub anthropic 流式：标准事件链 + message_start/message_delta 部分帧 usage。
const TC_ANTHROPIC_SSE: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_tc\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"tc-code-latest\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0,\"cache_read_input_tokens\":4}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
/// TokenHub 错误体：6 位业务码、前三位对应 HTTP 状态；早期拦截错误 type=gateway_error
/// （130079 错误处理节 / 135872 错误响应节）。模型不存在按基座特征归模型类。
const TC_ERR_400: &str = r#"{"error":{"message":"模型不存在或无权限","message_zh":"模型不存在或无权限","code":"400001","type":"invalid_request_error","request_id":"req_1"}}"#;

/// issue 84：三成员 openai 端点禁思考逐成员作用域（经真实 gateway）——TokenHub 两成员写
/// 平台标准 `thinking.type=disabled`（135872）；Coding Plan 官方未列思考参数（130092），
/// 保持框架默认 `chat_template_kwargs.enable_thinking=false`。
#[tokio::test]
async fn tencent_family_openai_endpoint_disable_thinking_per_member_scope() {
    let inbound = r#"{"model":"tc-code-latest","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
    for (i, (pt, official_thinking)) in [
        (Protocol::TencentCoding, false),
        (Protocol::TencentTokenhub, true),
        (Protocol::TencentTokenhubIntl, true),
    ]
    .into_iter()
    .enumerate()
    {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk84do{i}"),
            "/v1/messages",
            inbound,
            (200, "application/json", TC_OPENAI_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits.len(), 1, "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert!(up.get("disable_thinking").is_none(), "{pt:?}: {up}");
        if official_thinking {
            assert_eq!(up["thinking"]["type"], "disabled", "{pt:?}: {up}");
            assert!(up.get("chat_template_kwargs").is_none(), "{pt:?}: {up}");
        } else {
            assert!(up.get("thinking").is_none(), "{pt:?}: {up}");
            assert_eq!(
                up["chat_template_kwargs"]["enable_thinking"],
                false,
                "{pt:?}: {up}"
            );
        }
    }
}

/// issue 84：三成员 anthropic 端点禁思考——基座 anthropic 臂标准 `thinking.type=disabled`
/// （135874 thinking 对象为平台标准参数），家族一致。
#[tokio::test]
async fn tencent_family_anthropic_endpoint_disable_thinking_default_shape() {
    let inbound = r#"{"model":"tc-code-latest","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
    for (i, pt) in [
        Protocol::TencentCoding,
        Protocol::TencentTokenhub,
        Protocol::TencentTokenhubIntl,
    ]
    .into_iter()
    .enumerate()
    {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &format!("gk84da{i}"),
            "/v1/messages",
            inbound,
            (200, "application/json", TC_ANTHROPIC_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["thinking"]["type"], "disabled", "{pt:?}: {up}");
        assert!(up.get("disable_thinking").is_none(), "{pt:?}: {up}");
    }
}

/// issue 84：三成员 openai 端点非流式——入站 anthropic 转 openai 打 `/chat/completions`；
/// 响应回 anthropic：reasoning_content → thinking 块、cached_tokens → cache_read_input_tokens。
#[tokio::test]
async fn tencent_family_openai_endpoint_non_stream_request_response_usage() {
    for (i, pt) in [
        Protocol::TencentCoding,
        Protocol::TencentTokenhub,
        Protocol::TencentTokenhubIntl,
    ]
    .into_iter()
    .enumerate()
    {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk84on{i}"),
            "/v1/messages",
            GLM_ANTHROPIC_IN,
            (200, "application/json", TC_OPENAI_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits.len(), 1, "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["model"], "glm-test", "{pt:?}: {up}");
        assert_eq!(up["max_tokens"], 32, "{pt:?}: {up}");
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        let content = v["content"].as_array().unwrap();
        assert!(
            content.iter().any(|b| b["type"] == "thinking" && b["thinking"] == "think"),
            "{pt:?}: {v}"
        );
        assert!(content.iter().any(|b| b["type"] == "text" && b["text"] == "hi"), "{pt:?}: {v}");
        assert_eq!(v["usage"]["cache_read_input_tokens"], 4, "{pt:?}: {v}");
        assert_eq!(v["usage"]["output_tokens"], 3, "{pt:?}: {v}");
    }
}

/// issue 84：三成员 anthropic 端点非流式——入站 openai 转 anthropic 打 `/v1/messages`；
/// 响应回 openai 形状（usage.completion_tokens 沿 completion_tokens）。
#[tokio::test]
async fn tencent_family_anthropic_endpoint_non_stream_request_response_usage() {
    for (i, pt) in [
        Protocol::TencentCoding,
        Protocol::TencentTokenhub,
        Protocol::TencentTokenhubIntl,
    ]
    .into_iter()
    .enumerate()
    {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &format!("gk84an{i}"),
            "/chat/completions",
            GLM_OPENAI_IN,
            (200, "application/json", TC_ANTHROPIC_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits.len(), 1, "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["max_tokens"], 32, "{pt:?}: {up}");
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["choices"][0]["message"]["content"], "hi", "{pt:?}: {v}");
        assert_eq!(v["usage"]["completion_tokens"], 3, "{pt:?}: {v}");
    }
}

/// issue 84：三成员 openai 端点流式——delta.reasoning_content → thinking_delta、
/// delta.content → text_delta、`[DONE]` 收尾 message_stop。
#[tokio::test]
async fn tencent_family_openai_endpoint_stream() {
    for (i, pt) in [
        Protocol::TencentCoding,
        Protocol::TencentTokenhub,
        Protocol::TencentTokenhubIntl,
    ]
    .into_iter()
    .enumerate()
    {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk84os{i}"),
            "/v1/messages",
            GLM_ANTHROPIC_IN_STREAM,
            (200, "text/event-stream", TC_OPENAI_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        assert!(resp.contains("thinking_delta") && resp.contains("think"), "{pt:?}: {resp}");
        assert!(resp.contains("text_delta") && resp.contains("\"hi\""), "{pt:?}: {resp}");
        assert!(resp.contains("message_stop"), "{pt:?}: {resp}");
    }
}

/// issue 84：三成员 anthropic 端点流式——标准事件链转 openai chunk，结尾 `[DONE]`。
#[tokio::test]
async fn tencent_family_anthropic_endpoint_stream() {
    for (i, pt) in [
        Protocol::TencentCoding,
        Protocol::TencentTokenhub,
        Protocol::TencentTokenhubIntl,
    ]
    .into_iter()
    .enumerate()
    {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &format!("gk84as{i}"),
            "/chat/completions",
            GLM_OPENAI_IN_STREAM,
            (200, "text/event-stream", TC_ANTHROPIC_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}");
        assert!(resp.contains("\"hi\""), "{pt:?}: {resp}");
        assert!(resp.contains("[DONE]"), "{pt:?}: {resp}");
    }
}

/// issue 84：OpenAI Chat 错误——三成员都将 HTTP 400 + 腾讯云 6 位业务码原样送到客户端，
/// 模型不存在文案按基座特征归类。错误格式证据仅取自 OpenAI Chat 官方页（135872）；
/// Anthropic 专属错误形状未证实，保持框架默认，不在此伪造契约。
#[tokio::test]
async fn tencent_family_openai_endpoint_error_body() {
    let pa = aidog_adapter::platform::adapter_for("tencent_tokenhub");
    assert_eq!(
        pa.classify_error(400, TC_ERR_400),
        aidog_adapter::platform::error::UpstreamErrorClass::Model
    );
    for (i, pt) in [
        Protocol::TencentCoding,
        Protocol::TencentTokenhub,
        Protocol::TencentTokenhubIntl,
    ]
    .into_iter()
    .enumerate()
    {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk84er{i}"),
            "/v1/messages",
            GLM_ANTHROPIC_IN,
            (400, "application/json", TC_ERR_400),
        )
        .await;
        assert!(!hits.is_empty(), "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        assert_eq!(status, StatusCode::BAD_REQUEST, "{pt:?}: {resp}");
        assert!(resp.contains("模型不存在"), "{pt:?}: {resp}");
    }
}

// ── issue 85：小米 MiMo 家族（xiaomi_mimo / xiaomi_mimo_coding / xiaomi_mimo_coding_en）──
// 家族取证（官方文档核对日期 2026-11-23）见 aidog_adapter platform::registry
// `MimoFamilyAdapter` 模块注释。本节全部经真实 gateway handle_proxy → 分组路由 →
// 用户端点 (protocol, base_url=本地假上游) → 协议转换 / 透传 → 上游，走本地假上游禁真实外呼。

/// MiMo openai 非流式：reasoning_content 思考 + prompt_tokens_details.cached_tokens 缓存 +
/// completion_tokens_details.reasoning_tokens（官方响应示例形状）。
const MM_OPENAI_RESP: &str = r#"{"id":"mm_1","created":1,"model":"mimo-v2.6-pro","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"hi","reasoning_content":"think"}}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"completion_tokens_details":{"reasoning_tokens":2},"prompt_tokens_details":{"cached_tokens":4}}}"#;
/// MiMo openai 流式：delta.reasoning_content → delta.content → 末帧 choices 空 + usage → [DONE]。
const MM_OPENAI_SSE: &str = "data: {\"id\":\"mm_1\",\"created\":1,\"model\":\"mimo-v2.6-pro\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"think\"}}]}\n\n\
data: {\"id\":\"mm_1\",\"created\":1,\"model\":\"mimo-v2.6-pro\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n\
data: {\"id\":\"mm_1\",\"created\":1,\"model\":\"mimo-v2.6-pro\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":4}}}\n\n\
data: [DONE]\n\n";
/// MiMo anthropic 非流式：标准 Messages 形状 + cache_read（官方无 cache_creation 字段）。
const MM_ANTHROPIC_RESP: &str = r#"{"id":"msg_mm","type":"message","role":"assistant","model":"mimo-v2.6-pro","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":4}}"#;
/// MiMo anthropic 流式：标准事件链（含 thinking_delta）。
const MM_ANTHROPIC_SSE: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_mm\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"mimo-v2.6-pro\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0,\"cache_read_input_tokens\":4}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"plan\"}}\n\n\
event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

const MIMO_FAMILY: [(Protocol, &str); 3] = [
    (Protocol::XiaomiMimo, "gk85m"),
    (Protocol::XiaomiMimoCoding, "gk85mc"),
    (Protocol::XiaomiMimoCodingEn, "gk85mce"),
];

/// issue 85：三成员 openai 端点禁思考逐成员作用域（经真实 gateway）——`xiaomi_mimo` 写
/// 官方 `thinking.type=disabled`（deep-thinking.md），不剥 temperature（官方静默覆盖非报错）；
/// 套餐两成员官方未列 API 参数，保持框架默认 `chat_template_kwargs.enable_thinking=false`。
#[tokio::test]
async fn mimo_family_openai_endpoint_disable_thinking_per_member_scope() {
    let inbound = r#"{"model":"mimo-v2.6-pro","max_tokens":32,"disable_thinking":true,"temperature":0.7,"messages":[{"role":"user","content":"hi"}]}"#;
    for (i, (pt, official)) in MIMO_FAMILY.iter().enumerate().map(|(i, (pt, _))| (i, (pt.clone(), *pt == Protocol::XiaomiMimo))) {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk85do{i}"),
            "/v1/messages",
            inbound,
            (200, "application/json", MM_OPENAI_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits.len(), 1, "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert!(up.get("disable_thinking").is_none(), "{pt:?}: {up}");
        if official {
            assert_eq!(up["thinking"]["type"], "disabled", "{pt:?}: {up}");
            assert!(up.get("chat_template_kwargs").is_none(), "{pt:?}: {up}");
            assert!(
                up.get("temperature").is_some(),
                "官方静默覆盖非报错，不剥: {pt:?}: {up}"
            );
        } else {
            assert!(up.get("thinking").is_none(), "{pt:?}: {up}");
            assert_eq!(
                up["chat_template_kwargs"]["enable_thinking"],
                false,
                "{pt:?}: {up}"
            );
        }
    }
}

/// issue 85：三成员 anthropic 端点禁思考——基座 anthropic 臂标准 `thinking.type=disabled`
/// （官方参数表 enabled/disabled），家族一致。
#[tokio::test]
async fn mimo_family_anthropic_endpoint_disable_thinking_default_shape() {
    let inbound = r#"{"model":"mimo-v2.6-pro","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
    for (i, (pt, _)) in MIMO_FAMILY.iter().enumerate() {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &format!("gk85da{i}"),
            "/v1/messages",
            inbound,
            (200, "application/json", MM_ANTHROPIC_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["thinking"]["type"], "disabled", "{pt:?}: {up}");
        assert!(up.get("disable_thinking").is_none(), "{pt:?}: {up}");
    }
}

/// issue 85：三成员 openai 端点非流式——入站 anthropic 转 openai 打 `/chat/completions`；
/// 响应回 anthropic：reasoning_content → thinking 块、cached_tokens → cache_read_input_tokens。
#[tokio::test]
async fn mimo_family_openai_endpoint_non_stream_request_response_usage() {
    for (i, (pt, _)) in MIMO_FAMILY.iter().enumerate() {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk85on{i}"),
            "/v1/messages",
            GLM_ANTHROPIC_IN,
            (200, "application/json", MM_OPENAI_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits.len(), 1, "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["model"], "glm-test", "{pt:?}: {up}");
        assert_eq!(up["max_tokens"], 32, "{pt:?}: {up}");
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        let content = v["content"].as_array().unwrap();
        assert!(
            content.iter().any(|b| b["type"] == "thinking" && b["thinking"] == "think"),
            "{pt:?}: {v}"
        );
        assert!(content.iter().any(|b| b["type"] == "text" && b["text"] == "hi"), "{pt:?}: {v}");
        assert_eq!(v["usage"]["cache_read_input_tokens"], 4, "{pt:?}: {v}");
        assert_eq!(v["usage"]["output_tokens"], 3, "{pt:?}: {v}");
    }
}

/// issue 85：三成员 anthropic 端点非流式——入站 openai 转 anthropic 打 `/v1/messages`；
/// 响应回 openai 形状（官方无 cache_creation 字段，usage 沿 input/output）。
#[tokio::test]
async fn mimo_family_anthropic_endpoint_non_stream_request_response_usage() {
    for (i, (pt, _)) in MIMO_FAMILY.iter().enumerate() {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &format!("gk85an{i}"),
            "/chat/completions",
            GLM_OPENAI_IN,
            (200, "application/json", MM_ANTHROPIC_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits.len(), 1, "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}");
        let up: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
        assert_eq!(up["max_tokens"], 32, "{pt:?}: {up}");
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["choices"][0]["message"]["content"], "hi", "{pt:?}: {v}");
        assert_eq!(v["usage"]["completion_tokens"], 3, "{pt:?}: {v}");
    }
}

/// issue 85：三成员 openai 端点流式——delta.reasoning_content → thinking_delta、
/// delta.content → text_delta、末帧 usage、`[DONE]` 收尾 message_stop。
#[tokio::test]
async fn mimo_family_openai_endpoint_stream() {
    for (i, (pt, _)) in MIMO_FAMILY.iter().enumerate() {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk85os{i}"),
            "/v1/messages",
            GLM_ANTHROPIC_IN_STREAM,
            (200, "text/event-stream", MM_OPENAI_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        assert!(resp.contains("thinking_delta") && resp.contains("think"), "{pt:?}: {resp}");
        assert!(resp.contains("text_delta") && resp.contains("\"hi\""), "{pt:?}: {resp}");
        assert!(resp.contains("message_stop"), "{pt:?}: {resp}");
    }
}

/// issue 85：三成员 anthropic 端点流式——标准事件链（含 thinking_delta）转 openai chunk，
/// 结尾 `[DONE]`。
#[tokio::test]
async fn mimo_family_anthropic_endpoint_stream() {
    for (i, (pt, _)) in MIMO_FAMILY.iter().enumerate() {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &format!("gk85as{i}"),
            "/chat/completions",
            GLM_OPENAI_IN_STREAM,
            (200, "text/event-stream", MM_ANTHROPIC_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits[0].0, "/v1/messages", "{pt:?}");
        assert!(resp.contains("\"hi\""), "{pt:?}: {resp}");
        assert!(resp.contains("[DONE]"), "{pt:?}: {resp}");
    }
}

/// issue 85：错误体——官方错误码页仅状态码表格、无 JSON 错误体形状示例（未证实，不伪造
/// 模型类契约）。断言经真实 gateway 的 401 错误原样透传 + 状态码保持；归类按状态码
/// 401/429 → Service（换平台）。
#[tokio::test]
async fn mimo_family_error_body_passthrough_and_service_classification() {
    let pa = aidog_adapter::platform::adapter_for("xiaomi_mimo");
    // 状态码归类（官方未给「模型不存在」确定性文案，不测 Model 类特征）。
    assert_eq!(
        pa.classify_error(401, r#"{"error":{"message":"invalid api key"}}"#),
        aidog_adapter::platform::error::UpstreamErrorClass::Service
    );
    assert_eq!(
        pa.classify_error(429, r#"{"error":{"message":"rate limit exceeded"}}"#),
        aidog_adapter::platform::error::UpstreamErrorClass::Service
    );
    // 真实 gateway 往返：两 wire × 三成员，上游 401 + 中性错误体 → 客户端 401 原样透传。
    for (i, (pt, _)) in MIMO_FAMILY.iter().enumerate() {
        for (j, (wire, inbound_path, inbound_body, expected_path)) in [
            (
                Protocol::OpenAI,
                "/v1/messages",
                GLM_ANTHROPIC_IN,
                "/chat/completions",
            ),
            (Protocol::Anthropic, "/chat/completions", GLM_OPENAI_IN, "/v1/messages"),
        ]
        .into_iter()
        .enumerate()
        {
            let (status, resp, hits) = glm_member_call(
                pt.clone(),
                wire.clone(),
                &format!("gk85er{i}{j}"),
                inbound_path,
                inbound_body,
                (
                    401,
                    "application/json",
                    r#"{"error":{"message":"invalid api key"}}"#,
                ),
            )
            .await;
            assert!(!hits.is_empty(), "{pt:?}: {hits:?}");
            assert_eq!(hits[0].0, expected_path, "{pt:?}/{wire:?}");
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{pt:?}/{wire:?}: {resp}");
            assert!(resp.contains("invalid api key"), "{pt:?}/{wire:?}: {resp}");
        }
    }
}

/// issue 85（护栏）：`xiaomi_mimo` openai 端点多轮工具历史携带 reasoning_content
/// （官方 400 校验）；套餐成员官方文档未列该行为，不外推、不补写。
/// 取证：deep-thinking.md「must completely pass back the reasoning_content field,
/// otherwise the API will return a 400 error」。
#[tokio::test]
async fn mimo_family_openai_tool_history_carries_reasoning_content() {
    let history = r#"{"model":"mimo-v2.6-pro","max_tokens":64,"messages":[
        {"role":"user","content":"北京天气?"},
        {"role":"assistant","content":[
            {"type":"thinking","thinking":"需要调用天气工具","signature":"sig"},
            {"type":"tool_use","id":"tu1","name":"get_weather","input":{"city":"北京"}}]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"晴"}]}]}"#;
    let body = kimi_member_outbound_raw(
        Protocol::XiaomiMimo,
        Protocol::OpenAI,
        "gk85mr",
        false,
        history,
    )
    .await;
    let asst = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant" && m.get("tool_calls").is_some_and(|v| !v.is_null()))
        .expect("出站应含 assistant tool-call 消息");
    assert_eq!(
        asst["reasoning_content"],
        "需要调用天气工具",
        "assistant tool-call 历史应携带 reasoning_content: {asst}"
    );
    let body = kimi_member_outbound_raw(
        Protocol::XiaomiMimoCoding,
        Protocol::OpenAI,
        "gk85mrc",
        true,
        history,
    )
    .await;
    let asst = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant" && m.get("tool_calls").is_some_and(|v| !v.is_null()))
        .expect("出站应含 assistant tool-call 消息");
    assert!(
        asst.get("reasoning_content").is_none(),
        "套餐成员不外推 carry 行为: {asst}"
    );
}

/// issue 85：禁思考响应侧清洗覆盖 MiMo 官方 reasoning_content（非流式 message 与流式 delta）。
/// 即使上游返回思考内容，客户端请求 disable_thinking 时不得泄露；其它文本仍保留。
#[tokio::test]
async fn mimo_disable_thinking_strips_reasoning_from_non_stream_and_stream_responses() {
    for (i, (pt, _)) in MIMO_FAMILY.iter().enumerate() {
        let inbound = r#"{"model":"mimo-v2.6-pro","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
        let (status, resp, _) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk85stripn{i}"),
            "/v1/messages",
            inbound,
            (200, "application/json", MM_OPENAI_RESP),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert!(!resp.contains("think"), "reasoning_content must be stripped: {pt:?}: {resp}");
        assert!(resp.contains("hi"), "text must remain: {pt:?}: {resp}");

        let inbound_stream = r#"{"model":"mimo-v2.6-pro","max_tokens":32,"stream":true,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
        let (status, resp, _) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk85strips{i}"),
            "/v1/messages",
            inbound_stream,
            (200, "text/event-stream", MM_OPENAI_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert!(!resp.contains("think"), "stream reasoning delta must be stripped: {pt:?}: {resp}");
        assert!(resp.contains("hi"), "stream text must remain: {pt:?}: {resp}");
    }
}

/// issue 85：Anthropic wire 禁思考响应侧不泄露 thinking block（含 signature）或 thinking_delta，
/// 非流式与流式都保留普通文本。MiMo 官方 Anthropic schema 定义 thinking.signature；该测试也
/// 锁定通用清洗路径，避免未来家族覆写遗漏签名思考块。
#[tokio::test]
async fn mimo_anthropic_disable_thinking_strips_signed_blocks_and_stream_deltas() {
    let thinking_response = r#"{"id":"msg_mm","type":"message","role":"assistant","model":"mimo-v2.6-pro","content":[{"type":"thinking","thinking":"private reasoning","signature":"sig"},{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3}}"#;
    for (i, (pt, _)) in MIMO_FAMILY.iter().enumerate() {
        let inbound = r#"{"model":"mimo-v2.6-pro","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
        let (status, resp, _) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &format!("gk85antn{i}"),
            "/chat/completions",
            inbound,
            (200, "application/json", thinking_response),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert!(!resp.contains("private reasoning") && !resp.contains("signature"), "{pt:?}: {resp}");
        assert!(resp.contains("hi"), "text must remain: {pt:?}: {resp}");

        let inbound_stream = r#"{"model":"mimo-v2.6-pro","max_tokens":32,"stream":true,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#;
        let (status, resp, _) = glm_member_call(
            pt.clone(),
            Protocol::Anthropic,
            &format!("gk85ants{i}"),
            "/chat/completions",
            inbound_stream,
            (200, "text/event-stream", MM_ANTHROPIC_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert!(!resp.contains("plan"), "thinking delta must be stripped: {pt:?}: {resp}");
        assert!(resp.contains("hi"), "stream text must remain: {pt:?}: {resp}");
    }
}

// ── issue 86：Cline 家族（cline / cline_pass）──
// 官方取证见 aidog_adapter platform::registry::ClineFamilyAdapter。两成员均只提供 OpenAI
// Chat Completions 端点；下列 gateway 测试把用户 base_url 指向本地假上游，不访问外网。
const CLINE_FAMILY: [(Protocol, &str); 2] = [
    (Protocol::Cline, "gk86cline"),
    (Protocol::ClinePass, "gk86pass"),
];
const CLINE_OPENAI_RESP: &str = r#"{"id":"cline_1","model":"anthropic/claude-sonnet-4-6","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"hi"}}],"usage":{"prompt_tokens":10,"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":3},"cost":0.00012}}"#;
const CLINE_OPENAI_SSE: &str = "data: {\"id\":\"cline_1\",\"model\":\"cline-pass/qwen3.7-max\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"hi\"}}]}\n\ndata: {\"id\":\"cline_1\",\"model\":\"cline-pass/qwen3.7-max\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"prompt_tokens_details\":{\"cached_tokens\":3},\"cost\":0.00012}}\n\ndata: [DONE]\n\n";

/// 非流式请求必须发出官方格式头，并覆盖客户端提供的冲突值。
#[tokio::test]
async fn cline_family_gateway_sends_openai_response_header() {
    let requests = [
        (
            "/chat/completions",
            r#"{"model":"test","stream":false,"messages":[{"role":"user","content":"hi"}]}"#,
        ),
        (
            "/v1/messages",
            r#"{"model":"test","max_tokens":64,"messages":[{"role":"user","content":"hi"}]}"#,
        ),
    ];
    for (i, (pt, _)) in CLINE_FAMILY.iter().enumerate() {
        for (j, (uri, body)) in requests.iter().enumerate() {
            let seen = Arc::new(Mutex::new(Vec::new()));
            let sink = seen.clone();
            let app = axum::Router::new().fallback(move |headers: axum::http::HeaderMap| async move {
                sink.lock().unwrap().push(headers);
                ([ ("content-type", "application/json") ], CLINE_OPENAI_RESP)
            });
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let state = make_state(test_db().await).await;
            let pid = aidog_db::test_support::create_platform_at_fake_upstream(
                &state.db, pt.clone(), &base, &[Protocol::OpenAI],
            ).await;
            let key = format!("gk86header{i}-{j}");
            let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(&key, vec![])).await.unwrap();
            aidog_db::set_group_platforms(&state.db, group.id, &[GroupPlatformInput {
                platform_id: pid, priority: Some(0), weight: Some(1), level_priority: Some(0),
            }]).await.unwrap();
            let req = HttpRequest::builder().method("POST").uri(*uri)
                .header("authorization", format!("Bearer {key}"))
                .header("content-type", "application/json")
                .header("x-response-format", "wrapped")
                .body(Body::from(*body)).unwrap();
            let response = handle_proxy(AxumState(state.clone()), req).await;
            assert_eq!(response.status(), StatusCode::OK, "{pt:?} {uri}");
            if *uri == "/v1/messages" {
                let response = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap();
                let json: serde_json::Value = serde_json::from_slice(&response).unwrap();
                assert_eq!(json["type"], "message", "Anthropic response shape: {json}");
            }
            let headers = seen.lock().unwrap();
            assert_eq!(headers.len(), 1, "{pt:?} {uri}");
            assert_eq!(headers[0].get("x-response-format").unwrap(), "openai", "{pt:?} {uri}");
        }
    }
}

/// issue 86：Cline 的非流式 OpenAI 端点走真实 gateway，经本地假上游返回 usage 与缓存 token。
#[tokio::test]
async fn cline_family_openai_endpoint_non_stream() {
    let (status, resp, hits) = glm_member_call(
        Protocol::Cline,
        Protocol::OpenAI,
        "gk86nonstream",
        "/chat/completions",
        r#"{"model":"anthropic/claude-sonnet-4-6","messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", CLINE_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, "/chat/completions");
    let request: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["model"], "anthropic/claude-sonnet-4-6");
    assert_eq!(request["messages"][0]["role"], "user");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(response["choices"][0]["message"]["content"], "hi");
    assert_eq!(response["usage"]["prompt_tokens_details"]["cached_tokens"], 3);
    assert_eq!(response["usage"]["cost"], 0.00012);
}

// issue 94：CrazyRouter 三协议都由用户端点配置驱动，经本地假上游覆盖转换、响应、流、错误与 usage。
const CRAZYROUTER_OPENAI_RESP: &str = r#"{"id":"chatcmpl_cr","object":"chat.completion","model":"cr-test","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"prompt_tokens_details":{"cached_tokens":4}}}"#;
const CRAZYROUTER_OPENAI_SSE: &str = concat!(
    "data: {\"id\":\"chatcmpl_cr\",\"object\":\"chat.completion.chunk\",\"model\":\"cr-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
    "data: {\"id\":\"chatcmpl_cr\",\"object\":\"chat.completion.chunk\",\"model\":\"cr-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":4}}}\n\n",
    "data: [DONE]\n\n",
);
const CRAZYROUTER_ANTHROPIC_RESP: &str = r#"{"id":"msg_cr","type":"message","role":"assistant","model":"cr-test","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":2,"cache_creation_input_tokens":1}}"#;
const CRAZYROUTER_ANTHROPIC_SSE: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_cr\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"cr-test\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1}}}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
);
const CRAZYROUTER_GEMINI_RESP: &str = r#"{"candidates":[{"index":0,"finishReason":"STOP","content":{"role":"model","parts":[{"text":"hi"}]}}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":3,"totalTokenCount":13,"cachedContentTokenCount":4}}"#;
const CRAZYROUTER_GEMINI_SSE: &str = concat!(
    "data: {\"candidates\":[{\"index\":0,\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"hi\"}]}}]}\n\n",
    "data: {\"candidates\":[{\"index\":0,\"finishReason\":\"STOP\",\"content\":{\"role\":\"model\",\"parts\":[]}}],\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":3,\"totalTokenCount\":13,\"cachedContentTokenCount\":4}}\n\n",
);
const CRAZYROUTER_ERROR: &str = r#"{"error":{"message":"model not found","type":"invalid_request_error","code":"model_not_found"}}"#;

#[tokio::test]
async fn crazyrouter_openai_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::CrazyRouter, Protocol::OpenAI, "gk94oa", "/v1/messages", r#"{"model":"cr-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", CRAZYROUTER_OPENAI_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let req: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(req["model"], "cr-test");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["content"][0]["text"], "hi");
    assert_eq!(adapter_for("crazyrouter").extract_usage(CRAZYROUTER_OPENAI_RESP).0, 6);
    let (status, resp, _) = glm_member_call(Protocol::CrazyRouter, Protocol::OpenAI, "gk94oas", "/v1/messages", r#"{"model":"cr-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", CRAZYROUTER_OPENAI_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("hi") && resp.contains("message_stop"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::CrazyRouter, Protocol::OpenAI, "gk94oae", "/v1/messages", r#"{"model":"cr-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (400, "application/json", CRAZYROUTER_ERROR)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("model not found"), "{resp}");
}

#[tokio::test]
async fn crazyrouter_anthropic_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::CrazyRouter, Protocol::Anthropic, "gk94an", "/chat/completions", r#"{"model":"cr-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", CRAZYROUTER_ANTHROPIC_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["choices"][0]["message"]["content"], "hi");
    assert_eq!(adapter_for("crazyrouter").extract_usage(CRAZYROUTER_ANTHROPIC_RESP).2, 2);
    let (status, resp, _) = glm_member_call(Protocol::CrazyRouter, Protocol::Anthropic, "gk94ans", "/chat/completions", r#"{"model":"cr-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", CRAZYROUTER_ANTHROPIC_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("hi") && resp.contains("[DONE]"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::CrazyRouter, Protocol::Anthropic, "gk94ane", "/chat/completions", r#"{"model":"cr-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (400, "application/json", CRAZYROUTER_ERROR)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("model not found"), "{resp}");
}

#[tokio::test]
async fn crazyrouter_gemini_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::CrazyRouter, Protocol::Gemini, "gk94ge", "/v1/messages", r#"{"model":"cr-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", CRAZYROUTER_GEMINI_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1beta/models/cr-test:generateContent");
    let req: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(req["contents"][0]["parts"][0]["text"], "hi");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["content"][0]["text"], "hi");
    assert_eq!(adapter_for("crazyrouter").extract_usage(CRAZYROUTER_GEMINI_RESP).0, 6);
    let (status, resp, _) = glm_member_call(Protocol::CrazyRouter, Protocol::Gemini, "gk94ges", "/v1/messages", r#"{"model":"cr-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", CRAZYROUTER_GEMINI_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("hi"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::CrazyRouter, Protocol::Gemini, "gk94gee", "/v1/messages", r#"{"model":"cr-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (400, "application/json", CRAZYROUTER_ERROR)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("model not found"), "{resp}");
}

/// issue 86：两成员的 OpenAI SSE 端点都透传标准 chunk、末帧 usage 与 [DONE]。
#[tokio::test]
async fn cline_family_openai_endpoint_stream() {
    for (i, (pt, _)) in CLINE_FAMILY.iter().enumerate() {
        let (status, resp, hits) = glm_member_call(
            pt.clone(),
            Protocol::OpenAI,
            &format!("gk86stream{i}"),
            "/chat/completions",
            r#"{"model":"cline-pass/qwen3.7-max","stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
            (200, "text/event-stream", CLINE_OPENAI_SSE),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{pt:?}: {resp}");
        assert_eq!(hits.len(), 1, "{pt:?}: {hits:?}");
        assert_eq!(hits[0].0, "/chat/completions", "{pt:?}");
        assert!(resp.contains("data: [DONE]"), "{pt:?}: {resp}");
        assert!(resp.contains("hi"), "{pt:?}: {resp}");
    }
}

// ══ issue 93：Command Code（commandcode）Provider API 四端点协议 ══
// Official docs checked 2026-11-24: https://commandcode.ai/docs/provider
// 三 chat 协议「request and response bodies follow the」官方 schema（基座构件即官方形状）；
// 「Every endpoint emits token usage at the end of every stream … No opt-in required」；
// 错误体为标准 OpenAI / Anthropic envelope（400 unsupported_model 仅 OpenAI envelope 有 code）。
const CC_OPENAI_RESP: &str = r#"{"id":"cc_1","object":"chat.completion","model":"cc-test","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":6,"completion_tokens":3,"total_tokens":9,"prompt_tokens_details":{"cached_tokens":2}}}"#;
const CC_OPENAI_SSE: &str = "data: {\"id\":\"cc_1\",\"object\":\"chat.completion.chunk\",\"model\":\"cc-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n\
data: {\"id\":\"cc_1\",\"object\":\"chat.completion.chunk\",\"model\":\"cc-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":6,\"completion_tokens\":3,\"total_tokens\":9}}\n\n\
data: [DONE]\n\n";
const CC_OPENAI_ERROR: &str = r#"{"error":{"message":"Model isn't in the catalog","type":"invalid_request_error","code":"unsupported_model","param":null}}"#;
const CC_ANTHROPIC_RESP: &str = r#"{"id":"msg_cc","type":"message","role":"assistant","model":"cc-test","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":2,"cache_creation_input_tokens":1}}"#;
const CC_ANTHROPIC_SSE: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_cc\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"cc-test\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0,\"cache_read_input_tokens\":2}}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
const CC_ANTHROPIC_ERROR: &str =
    r#"{"type":"error","error":{"type":"invalid_request_error","message":"Wrong endpoint for the model"}}"#;
const CC_RESPONSES_RESP: &str = r#"{"id":"resp_cc","object":"response","model":"cc-test","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi","annotations":[]}]}],"usage":{"input_tokens":6,"output_tokens":2,"input_tokens_details":{"cached_tokens":4}}}"#;
const CC_RESPONSES_SSE: &str = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hi\"}\n\n\
data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_cc\",\"status\":\"completed\",\"usage\":{\"input_tokens\":6,\"output_tokens\":2}}}\n\n";
const CC_DECISION_RESP: &str = r#"{"model":"cc-jev-1","answers":[{"type":"noul","noul":0.96}],"usage":{"input_tokens":84,"output_tokens":3}}"#;

/// openai 端点：入站 anthropic → 出站 openai 打 `/chat/completions`；非流式回 anthropic、
/// 流式末帧带 usage 后 `[DONE]`；400 unsupported_model（官方 code）原样回客户端。
#[tokio::test]
async fn commandcode_openai_endpoint_request_response_stream_error_usage() {
    use aidog_adapter::platform::adapter_for;

    let (status, resp, hits) = glm_member_call(
        Protocol::CommandCode,
        Protocol::OpenAI,
        "gk93oa",
        "/v1/messages",
        r#"{"model":"cc-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", CC_OPENAI_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let request: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["model"], "cc-test", "{request}");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    // 入站 anthropic（/v1/messages）→ 出站 openai 转换、响应按客户端协议回 anthropic 形状。
    assert_eq!(response["content"][0]["text"], "hi", "{response}");
    assert!(response.get("usage").is_some(), "{response}");
    let (input, output, cache_read, cache_write, cache_write_1h, _) =
        adapter_for("commandcode").extract_usage(CC_OPENAI_RESP);
    assert_eq!(
        (input, output, cache_read, cache_write, cache_write_1h),
        (4, 3, 2, 0, 0)
    );

    let (status, resp, hits) = glm_member_call(
        Protocol::CommandCode,
        Protocol::OpenAI,
        "gk93oas",
        "/v1/messages",
        r#"{"model":"cc-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "text/event-stream", CC_OPENAI_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    // 客户端协议 anthropic：流按 anthropic SSE 输出，message_stop 收官、无 [DONE]。
    assert!(resp.contains("hi"), "{resp}");
    assert!(resp.contains("message_stop"), "{resp}");
    assert!(
        !resp.contains("[DONE]"),
        "openai upstream terminator must not leak into Anthropic output: {resp}"
    );

    let (status, resp, hits) = glm_member_call(
        Protocol::CommandCode,
        Protocol::OpenAI,
        "gk93oae",
        "/v1/messages",
        r#"{"model":"cc-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (400, "application/json", CC_OPENAI_ERROR),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(!hits.is_empty(), "fake upstream must receive request");
    assert!(resp.contains("unsupported_model"), "{resp}");
}

/// anthropic 端点：入站 openai → 出站 anthropic 打 `/v1/messages`；流式 message_delta
/// 带 usage；anthropic envelope 错误原样回客户端。
#[tokio::test]
async fn commandcode_anthropic_endpoint_request_response_stream_error() {
    let (status, resp, hits) = glm_member_call(
        Protocol::CommandCode,
        Protocol::Anthropic,
        "gk93an",
        "/chat/completions",
        r#"{"model":"cc-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", CC_ANTHROPIC_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    let request: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["max_tokens"], 32, "{request}");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(response["choices"][0]["message"]["content"], "hi", "{response}");
    assert!(response.get("usage").is_some(), "{response}");

    let (status, resp, hits) = glm_member_call(
        Protocol::CommandCode,
        Protocol::Anthropic,
        "gk93ans",
        "/chat/completions",
        r#"{"model":"cc-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "text/event-stream", CC_ANTHROPIC_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert!(resp.contains("hi"), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");

    let (status, resp, hits) = glm_member_call(
        Protocol::CommandCode,
        Protocol::Anthropic,
        "gk93ane",
        "/chat/completions",
        r#"{"model":"cc-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (400, "application/json", CC_ANTHROPIC_ERROR),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(!hits.is_empty(), "fake upstream must receive request");
    assert!(resp.contains("Wrong endpoint for the model"), "{resp}");
}

/// openai_responses 端点：出站打 `/responses`；流式 `response.completed` 收官；
/// usage 走 responses 缓存臂（cached_tokens 从 input 扣减）。
#[tokio::test]
async fn commandcode_openai_responses_endpoint_request_response_stream_usage() {
    use aidog_adapter::platform::adapter_for;

    let (status, resp, hits) = glm_member_call(
        Protocol::CommandCode,
        Protocol::OpenAIResponses,
        "gk93or",
        "/chat/completions",
        r#"{"model":"cc-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "application/json", CC_RESPONSES_RESP),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/responses");
    let request: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(request["model"], "cc-test", "{request}");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(response["choices"][0]["message"]["content"], "hi", "{response}");
    assert!(response.get("usage").is_some(), "{response}");
    let (input, output, cache_read, cache_write, cache_write_1h, _) =
        adapter_for("commandcode").extract_usage(CC_RESPONSES_RESP);
    // responses 官方公式：ordinary input = input_tokens − cached。
    assert_eq!(
        (input, output, cache_read, cache_write, cache_write_1h),
        (2, 2, 4, 0, 0)
    );

    let (status, resp, hits) = glm_member_call(
        Protocol::CommandCode,
        Protocol::OpenAIResponses,
        "gk93ors",
        "/chat/completions",
        r#"{"model":"cc-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
        (200, "text/event-stream", CC_RESPONSES_SSE),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/responses");
    assert!(resp.contains("hi"), "{resp}");
    assert!(resp.contains("[DONE]"), "{resp}");
}

/// typesafe 端点：决策请求 `/v1/systemone` 打用户 base_url + `/systemone`（R10），
/// model 改写为 jev 槽位（R8），usage.input_tokens 落 proxy_log。
#[tokio::test]
async fn commandcode_typesafe_endpoint_decision_dispatch_usage() {
    let (base, hits) = spawn_glm_upstream(200, "application/json", CC_DECISION_RESP).await;
    let state = make_state(test_db().await).await;
    let pid = aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        Protocol::CommandCode,
        &base,
        std::slice::from_ref(&Protocol::TypeSafe),
    )
    .await;
    aidog_db::update_platform(
        &state.db,
        aidog_db::models::UpdatePlatform {
            id: pid,
            models: Some(crate::gateway::models::PlatformModels {
                jev: vec!["cc-test".into()],
                ..Default::default()
            }),
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            endpoints: None,
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
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group("gk93ts", vec![]))
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
        .uri("/v1/systemone")
        .header("authorization", "Bearer gk93ts")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"whatever","usage":{},"answers":[]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["model"], "cc-jev-1", "决策响应透传: {v}");

    let hits_snapshot = hits.lock().unwrap().clone();
    assert_eq!(hits_snapshot.len(), 1, "{hits_snapshot:?}");
    assert_eq!(
        hits_snapshot[0].0,
        "/systemone",
        "URL = 用户 base_url + /systemone (R10)"
    );
    let sent: serde_json::Value = serde_json::from_str(&hits_snapshot[0].1).unwrap();
    assert_eq!(sent["model"], "cc-test", "model 应改写为平台模型链首: {sent}");
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let log = logs.iter().find(|l| l.group_key == "gk93ts").expect("log row missing");
    assert_eq!(log.input_tokens, 84, "typesafe usage.input_tokens 应落库");
}

// issue 98：LiteLLM Gateway 单 openai 端点协议，经本地假上游覆盖转换、响应、流、错误与 usage。
// 官方统一接口镜像 OpenAI ChatCompletions 形状（https://docs.litellm.ai/docs/proxy/quick_start）。
const LITELLM_OPENAI_RESP: &str = r#"{"id":"lt_1","object":"chat.completion","model":"lt-test","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"prompt_tokens_details":{"cached_tokens":4}}}"#;
const LITELLM_OPENAI_SSE: &str = concat!(
    "data: {\"id\":\"lt_1\",\"object\":\"chat.completion.chunk\",\"model\":\"lt-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
    "data: {\"id\":\"lt_1\",\"object\":\"chat.completion.chunk\",\"model\":\"lt-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":4}}}\n\n",
    "data: [DONE]\n\n",
);
const LITELLM_ERROR: &str = r#"{"error":{"message":"Invalid Virtual Key","type":null,"param":null,"code":"401"}}"#;

/// openai 端点：出站打 `/chat/completions`；非流式回 anthropic；流式末帧 usage 后 `[DONE]`；
/// 401 归服务类；usage 缓存扣减（prompt 10 − cached 4 = 6）。
#[tokio::test]
async fn litellm_openai_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::LiteLLM, Protocol::OpenAI, "gk98oa", "/v1/messages", r#"{"model":"lt-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", LITELLM_OPENAI_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let req: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(req["model"], "lt-test");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["content"][0]["text"], "hi");
    assert_eq!(adapter_for("litellm").extract_usage(LITELLM_OPENAI_RESP).0, 6);
    let (status, resp, _) = glm_member_call(Protocol::LiteLLM, Protocol::OpenAI, "gk98oas", "/v1/messages", r#"{"model":"lt-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", LITELLM_OPENAI_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("hi") && resp.contains("message_stop"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::LiteLLM, Protocol::OpenAI, "gk98oae", "/v1/messages", r#"{"model":"lt-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (401, "application/json", LITELLM_ERROR)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{resp}");
    assert!(resp.contains("Invalid Virtual Key"), "{resp}");
}

const LITELLM_ANTHROPIC_RESP: &str = r#"{"id":"msg_lt","type":"message","role":"assistant","model":"lt-test","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":2,"cache_creation_input_tokens":1}}"#;
const LITELLM_ANTHROPIC_SSE: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_lt\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"lt-test\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1}}}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
);
// anthropic 路由预算错误：error.type 持 Anthropic 状态类型，网关类型在 error.litellm_error_type
// （官方错误参考「Errors on the Anthropic Messages routes」节，核对日期 2026-11-25）。
const LITELLM_ANTHROPIC_BUDGET_ERROR: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Budget has been exceeded! Key=zero-budget Current cost: 0.0, Max budget: 0.0","litellm_error_type":"budget_exceeded"}}"#;

/// anthropic 端点（用户可配，官方 /v1/messages 路由）：出站打 `/v1/messages`；
/// 流式透传标准事件；预算错误归服务类、末次尝试原样透传。
#[tokio::test]
async fn litellm_anthropic_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::LiteLLM, Protocol::Anthropic, "gk98an", "/chat/completions", r#"{"model":"lt-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", LITELLM_ANTHROPIC_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["choices"][0]["message"]["content"], "hi");
    assert_eq!(adapter_for("litellm").extract_usage(LITELLM_ANTHROPIC_RESP).2, 2);
    let (status, resp, _) = glm_member_call(Protocol::LiteLLM, Protocol::Anthropic, "gk98ans", "/chat/completions", r#"{"model":"lt-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", LITELLM_ANTHROPIC_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("hi") && resp.contains("[DONE]"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::LiteLLM, Protocol::Anthropic, "gk98ane", "/chat/completions", r#"{"model":"lt-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (422, "application/json", LITELLM_ANTHROPIC_BUDGET_ERROR)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert!(resp.contains("Budget has been exceeded"), "{resp}");
}

const LITELLM_GEMINI_RESP: &str = r#"{"candidates":[{"index":0,"finishReason":"STOP","content":{"role":"model","parts":[{"text":"hi"}]}}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":3,"totalTokenCount":13,"cachedContentTokenCount":4}}"#;
const LITELLM_GEMINI_SSE: &str = concat!(
    "data: {\"candidates\":[{\"index\":0,\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"hi\"}]}}]}\n\n",
    "data: {\"candidates\":[{\"index\":0,\"finishReason\":\"STOP\",\"content\":{\"role\":\"model\",\"parts\":[]}}],\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":3,\"totalTokenCount\":13,\"cachedContentTokenCount\":4}}\n\n",
);

/// gemini 端点（用户可配）：出站打 `/v1beta/models/<model>:generateContent`；
/// 非流式与流式照 Gemini wire 转换。
#[tokio::test]
async fn litellm_gemini_endpoint_dispatch() {
    let (status, resp, hits) = glm_member_call(Protocol::LiteLLM, Protocol::Gemini, "gk98ge", "/v1/messages", r#"{"model":"lt-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", LITELLM_GEMINI_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1beta/models/lt-test:generateContent");
    let req: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(req["contents"][0]["parts"][0]["text"], "hi");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["content"][0]["text"], "hi");
    let (status, resp, _) = glm_member_call(Protocol::LiteLLM, Protocol::Gemini, "gk98ges", "/v1/messages", r#"{"model":"lt-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", LITELLM_GEMINI_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("hi"), "{resp}");
}

// issue 97：书生·端砚三协议（openai / openai_responses / anthropic）都由用户端点配置驱动，
// 经本地假上游覆盖转换、响应、流、错误与 usage。官方无线级文档，形状按协议标准（第三方多源佐证）。
const INK_OPENAI_RESP: &str = r#"{"id":"ink_1","object":"chat.completion","model":"ink-test","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"prompt_tokens_details":{"cached_tokens":4}}}"#;
const INK_OPENAI_SSE: &str = concat!(
    "data: {\"id\":\"ink_1\",\"object\":\"chat.completion.chunk\",\"model\":\"ink-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
    "data: {\"id\":\"ink_1\",\"object\":\"chat.completion.chunk\",\"model\":\"ink-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":4}}}\n\n",
    "data: [DONE]\n\n",
);
const INK_RESPONSES_RESP: &str = r#"{"id":"resp_ink","object":"response","model":"ink-test","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi","annotations":[]}]}],"usage":{"input_tokens":6,"output_tokens":2,"input_tokens_details":{"cached_tokens":4}}}"#;
const INK_RESPONSES_SSE: &str = concat!(
    "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hi\"}\n\n",
    "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_ink\",\"status\":\"completed\",\"usage\":{\"input_tokens\":6,\"output_tokens\":2}}}\n\n",
);
const INK_ANTHROPIC_RESP: &str = r#"{"id":"msg_ink","type":"message","role":"assistant","model":"ink-test","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3,"cache_read_input_tokens":2,"cache_creation_input_tokens":1}}"#;
const INK_ANTHROPIC_SSE: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_ink\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"ink-test\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1}}}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
);
const INK_ERROR: &str = r#"{"error":{"message":"invalid api key","type":"authentication_error"}}"#;

/// openai 端点：出站打 `/chat/completions`；非流式回 anthropic；流式末帧 usage 后 `[DONE]`；
/// 401 归服务类；usage 缓存扣减（prompt 10 − cached 4 = 6）。
#[tokio::test]
async fn intern_inkstone_openai_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::InternInkstone, Protocol::OpenAI, "gk97oa", "/v1/messages", r#"{"model":"ink-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", INK_OPENAI_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let req: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(req["model"], "ink-test");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["content"][0]["text"], "hi");
    assert_eq!(adapter_for("intern_inkstone").extract_usage(INK_OPENAI_RESP).0, 6);
    let (status, resp, _) = glm_member_call(Protocol::InternInkstone, Protocol::OpenAI, "gk97oas", "/v1/messages", r#"{"model":"ink-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", INK_OPENAI_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("hi") && resp.contains("message_stop"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::InternInkstone, Protocol::OpenAI, "gk97oae", "/v1/messages", r#"{"model":"ink-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (401, "application/json", INK_ERROR)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{resp}");
    assert!(resp.contains("invalid api key"), "{resp}");
}

/// openai_responses 端点：出站打 `/responses`；流式 `response.completed` 收官；
/// usage 走 responses 缓存臂（ordinary input = input − cached）。
#[tokio::test]
async fn intern_inkstone_openai_responses_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::InternInkstone, Protocol::OpenAIResponses, "gk97or", "/chat/completions", r#"{"model":"ink-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", INK_RESPONSES_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/responses");
    let req: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(req["model"], "ink-test");
    let response: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(response["choices"][0]["message"]["content"], "hi");
    let (input, output, cache_read, cache_write, cache_write_1h, _) =
        adapter_for("intern_inkstone").extract_usage(INK_RESPONSES_RESP);
    assert_eq!((input, output, cache_read, cache_write, cache_write_1h), (2, 2, 4, 0, 0));
    let (status, resp, _) = glm_member_call(Protocol::InternInkstone, Protocol::OpenAIResponses, "gk97ors", "/chat/completions", r#"{"model":"ink-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", INK_RESPONSES_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/responses");
    assert!(resp.contains("hi") && resp.contains("[DONE]"), "{resp}");
}

/// anthropic 端点：出站打 `/v1/messages`；非流式回 openai 形状；流式透传官方事件集；
/// usage 缓存读臂。
#[tokio::test]
async fn intern_inkstone_anthropic_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::InternInkstone, Protocol::Anthropic, "gk97an", "/chat/completions", r#"{"model":"ink-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", INK_ANTHROPIC_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["choices"][0]["message"]["content"], "hi");
    assert_eq!(adapter_for("intern_inkstone").extract_usage(INK_ANTHROPIC_RESP).2, 2);
    let (status, resp, _) = glm_member_call(Protocol::InternInkstone, Protocol::Anthropic, "gk97ans", "/chat/completions", r#"{"model":"ink-test","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", INK_ANTHROPIC_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("hi") && resp.contains("[DONE]"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::InternInkstone, Protocol::Anthropic, "gk97ane", "/chat/completions", r#"{"model":"ink-test","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (401, "application/json", INK_ERROR)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{resp}");
    assert!(resp.contains("invalid api key"), "{resp}");
}

// issue 99：LongCat 双协议（registry 内置默认 anthropic / openai）都由用户端点配置驱动，
// 经本地假上游覆盖转换、响应、流、错误与 usage。
const LONGCAT_OPENAI_RESP: &str = r#"{"id":"chatcmpl_lc","object":"chat.completion","model":"LongCat-2.5-Preview","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13,"completion_tokens_details":{"reasoning_tokens":2}}}"#;
const LONGCAT_OPENAI_SSE: &str = concat!(
    "data: {\"id\":\"chatcmpl_lc\",\"object\":\"chat.completion.chunk\",\"model\":\"LongCat-2.5-Preview\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
    "data: {\"id\":\"chatcmpl_lc\",\"object\":\"chat.completion.chunk\",\"model\":\"LongCat-2.5-Preview\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13}}\n\n",
    "data: [DONE]\n\n",
);
const LONGCAT_ANTHROPIC_RESP: &str = r#"{"id":"msg_lc","type":"message","role":"assistant","model":"LongCat-2.5-Preview","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":6,"output_tokens":3}}"#;
const LONGCAT_ANTHROPIC_SSE: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_lc\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"LongCat-2.5-Preview\",\"content\":[],\"usage\":{\"input_tokens\":6,\"output_tokens\":0}}}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":3}}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
);
const LONGCAT_ERROR: &str = r#"{"error":{"code":"rate_limit_exceeded","message":"Request rate limit exceeded, please try again later","type":"rate_limit_error","retry_after":60}}"#;

/// openai 端点：出站打 `/chat/completions`；非流式回 anthropic；流式末帧 usage 后 `[DONE]`；
/// 429 限频归服务类、末次尝试原样透传；usage 无缓存字段（prompt 10 全额计 input）。
#[tokio::test]
async fn longcat_openai_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::Longcat, Protocol::OpenAI, "gk99oa", "/v1/messages", r#"{"model":"LongCat-2.5-Preview","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", LONGCAT_OPENAI_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/chat/completions");
    let req: serde_json::Value = serde_json::from_str(&hits[0].1).unwrap();
    assert_eq!(req["model"], "LongCat-2.5-Preview");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["content"][0]["text"], "hi");
    assert_eq!(adapter_for("longcat").extract_usage(LONGCAT_OPENAI_RESP).0, 10);
    let (status, resp, _) = glm_member_call(Protocol::Longcat, Protocol::OpenAI, "gk99oas", "/v1/messages", r#"{"model":"LongCat-2.5-Preview","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", LONGCAT_OPENAI_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("text_delta") && resp.contains("hi") && resp.contains("message_stop"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::Longcat, Protocol::OpenAI, "gk99oae", "/v1/messages", r#"{"model":"LongCat-2.5-Preview","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (429, "application/json", LONGCAT_ERROR)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{resp}");
    assert!(resp.contains("rate limit exceeded"), "{resp}");
}

/// anthropic 端点：出站打 `/v1/messages`；非流式回 openai 形状；流式透传标准事件链；
/// usage 走基座 anthropic 臂（input 6 − cache 0）。
#[tokio::test]
async fn longcat_anthropic_endpoint_dispatch() {
    use aidog_adapter::platform::adapter_for;
    let (status, resp, hits) = glm_member_call(Protocol::Longcat, Protocol::Anthropic, "gk99an", "/chat/completions", r#"{"model":"LongCat-2.5-Preview","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (200, "application/json", LONGCAT_ANTHROPIC_RESP)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(hits[0].0, "/v1/messages");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&resp).unwrap()["choices"][0]["message"]["content"], "hi");
    assert_eq!(adapter_for("longcat").extract_usage(LONGCAT_ANTHROPIC_RESP).0, 6);
    let (status, resp, _) = glm_member_call(Protocol::Longcat, Protocol::Anthropic, "gk99ans", "/chat/completions", r#"{"model":"LongCat-2.5-Preview","max_tokens":32,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#, (200, "text/event-stream", LONGCAT_ANTHROPIC_SSE)).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert!(resp.contains("hi") && resp.contains("[DONE]"), "{resp}");
    let (status, resp, _) = glm_member_call(Protocol::Longcat, Protocol::Anthropic, "gk99ane", "/chat/completions", r#"{"model":"LongCat-2.5-Preview","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#, (400, "application/json", r#"{"type":"error","error":{"type":"invalid_request_error","message":"/top_p: 2 is not less or equal to 1"}}"#)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert!(resp.contains("top_p"), "{resp}");
}
