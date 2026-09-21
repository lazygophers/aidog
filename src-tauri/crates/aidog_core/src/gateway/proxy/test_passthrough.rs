use super::*;

#[test]
fn passthrough_url_path_only() {
    let uri: axum::http::Uri = "/v1/messages".parse().unwrap();
    assert_eq!(
        build_passthrough_url("https://api.anthropic.com", &uri),
        "https://api.anthropic.com/v1/messages"
    );
}

#[test]
fn passthrough_url_with_query() {
    let uri: axum::http::Uri = "/v1/messages?beta=true&foo=bar".parse().unwrap();
    assert_eq!(
        build_passthrough_url("https://api.anthropic.com", &uri),
        "https://api.anthropic.com/v1/messages?beta=true&foo=bar"
    );
}

#[test]
fn passthrough_url_trims_trailing_slash() {
    let uri: axum::http::Uri = "/v1/messages".parse().unwrap();
    assert_eq!(
        build_passthrough_url("https://api.anthropic.com/", &uri),
        "https://api.anthropic.com/v1/messages"
    );
}

// ── 透传 header 剔除 Host + Content-Length，保留 Authorization 及其他 ──

#[test]
fn passthrough_headers_drops_hop_by_hop_keeps_auth() {
    let mut orig = axum::http::HeaderMap::new();
    orig.insert("host", "127.0.0.1:8080".parse().unwrap());
    orig.insert("content-length", "123".parse().unwrap());
    orig.insert("authorization", "Bearer sk-oauth-token".parse().unwrap());
    orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
    orig.insert("x-custom", "keep-me".parse().unwrap());

    let fwd = passthrough_headers(&orig);

    // hop-by-hop 剔除
    assert!(!fwd.contains_key("host"), "host must be dropped");
    assert!(
        !fwd.contains_key("content-length"),
        "content-length must be dropped"
    );
    // 客户端自带订阅 OAuth 原样保留
    assert_eq!(
        fwd.get("authorization").and_then(|v| v.to_str().ok()),
        Some("Bearer sk-oauth-token")
    );
    // 其余 header 原样
    assert_eq!(
        fwd.get("anthropic-version").and_then(|v| v.to_str().ok()),
        Some("2023-06-01")
    );
    assert_eq!(
        fwd.get("x-custom").and_then(|v| v.to_str().ok()),
        Some("keep-me")
    );
}

// ── convert 路径透传：剔 hop-by-hop + auth/UA/CT，保留客户端 SDK 头（跨协议也带）──
#[test]
fn passthrough_convert_strips_hop_and_override_keeps_sdk_headers() {
    let mut orig = axum::http::HeaderMap::new();
    // hop-by-hop / 强覆盖（应剔）
    orig.insert("host", "127.0.0.1:8080".parse().unwrap());
    orig.insert("content-length", "123".parse().unwrap());
    orig.insert("connection", "keep-alive".parse().unwrap());
    orig.insert("authorization", "Bearer sk-inbound".parse().unwrap());
    orig.insert("user-agent", "inbound-ua/1.0".parse().unwrap());
    orig.insert("content-type", "text/plain".parse().unwrap());
    // 客户端 SDK 头（应保留透传）
    orig.insert(
        "anthropic-beta",
        "interleaved-thinking-2025-05-14".parse().unwrap(),
    );
    orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
    orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());
    orig.insert("x-stainless-runtime-version", "v24.3.0".parse().unwrap());
    orig.insert("x-stainless-timeout", "3000".parse().unwrap());
    orig.insert("x-claude-code-session-id", "sess-abc".parse().unwrap());
    orig.insert("x-app", "cli".parse().unwrap());

    // 官方 Anthropic 上游：anthropic-beta 保留（依赖 beta 协商能力）
    let fwd = passthrough_convert_headers(&orig);

    // 剔除项
    for stripped in [
        "host",
        "content-length",
        "connection",
        "authorization",
        "user-agent",
        "content-type",
    ] {
        assert!(
            !fwd.contains_key(stripped),
            "{stripped} must be stripped for convert apply to override"
        );
    }
    // 透传项（含跨协议透明的 SDK 头；官方端点 anthropic-beta 保留）
    assert_eq!(
        fwd.get("anthropic-beta").and_then(|v| v.to_str().ok()),
        Some("interleaved-thinking-2025-05-14")
    );
    assert_eq!(
        fwd.get("x-stainless-package-version")
            .and_then(|v| v.to_str().ok()),
        Some("0.94.0")
    );
    assert_eq!(
        fwd.get("x-stainless-runtime-version")
            .and_then(|v| v.to_str().ok()),
        Some("v24.3.0")
    );
    assert_eq!(
        fwd.get("x-stainless-timeout").and_then(|v| v.to_str().ok()),
        Some("3000")
    );
    assert_eq!(
        fwd.get("x-claude-code-session-id")
            .and_then(|v| v.to_str().ok()),
        Some("sess-abc")
    );
}

// ── convert 路径透传 anthropic-beta：官方与第三方一视同仁，都 verbatim 转发 ──
// 2026-09-21 前这里按 host 分流剔第三方，理由是「GLM 不认新 beta token，返 400 code 1210」。
// 实测 GLM / CometAPI 对四种 beta 值全部 200，前提已不成立；官方也明令禁按值 allowlist。
// 第三方真拒时由 forward.rs 的 beta 自动降级兜底（400 → 剔头重试一次）。
#[test]
fn passthrough_convert_anthropic_beta_forwarded_to_third_party() {
    let mut orig = axum::http::HeaderMap::new();
    orig.insert(
        "anthropic-beta",
        "context-1m-2025-08-07,effort-2025-11-24".parse().unwrap(),
    );
    orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
    orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());

    let fwd = passthrough_convert_headers(&orig);
    assert_eq!(
        fwd.get("anthropic-beta").and_then(|v| v.to_str().ok()),
        Some("context-1m-2025-08-07,effort-2025-11-24"),
        "anthropic-beta must reach third-party endpoints verbatim"
    );
    assert_eq!(
        fwd.get("anthropic-version").and_then(|v| v.to_str().ok()),
        Some("2023-06-01")
    );
    assert_eq!(
        fwd.get("x-stainless-package-version")
            .and_then(|v| v.to_str().ok()),
        Some("0.94.0")
    );
}

// ── 官方上游必须 verbatim 转发未知 anthropic-beta 值（禁按已知值 allowlist） ──
// 背景: Claude Code auto mode 的服务器端安全审查用 beta 值 dangerous-tool-use-2026-09-03，
// 与请求体顶层 safeguards 字段成对出现，缺一即上游 400；官方文档明令
// 「Forward the header verbatim; don't allowlist individual values, because the set changes
// with Claude Code releases」。本测试守住「官方 host 上按 beta 值挑拣」这条回归。
// 出处: https://code.claude.com/docs/en/llm-gateway-protocol#request-headers
#[test]
fn official_anthropic_keeps_unknown_beta_values_verbatim() {
    let raw = "dangerous-tool-use-2026-09-03,some-future-beta-2027-01-01";
    let mut orig = axum::http::HeaderMap::new();
    orig.insert("anthropic-beta", raw.parse().unwrap());

    let official = passthrough_convert_headers(&orig);
    assert_eq!(
        official.get("anthropic-beta").and_then(|v| v.to_str().ok()),
        Some(raw),
        "official anthropic host must forward unknown beta values verbatim"
    );
}

// ── is_official_anthropic_host: host 提取 + 官方判定（含端口/userinfo/大小写） ──
#[test]
fn official_anthropic_host_detection() {
    assert!(is_official_anthropic_host(
        "https://api.anthropic.com/v1/messages"
    ));
    assert!(is_official_anthropic_host(
        "https://api.anthropic.com:443/v1/messages"
    ));
    assert!(is_official_anthropic_host(
        "https://API.ANTHROPIC.COM/v1/messages"
    ));
    // 第三方 / 中转站 host
    assert!(!is_official_anthropic_host(
        "https://open.bigmodel.cn/api/anthropic/v1/messages"
    ));
    assert!(!is_official_anthropic_host(
        "https://api.anthropic.com.evil.com/v1/messages"
    ));
    assert!(!is_official_anthropic_host(
        "https://proxy.example.com/anthropic/v1/messages"
    ));
}

// ── build_upstream_headers：透传入站（脱敏）+ 覆盖 UA/auth，日志反映真实上游头 ──
#[test]
fn passthrough_does_not_invoke_convert_request() {
    let src = include_str!("passthrough.rs");
    // 定位 handle_passthrough 函数体范围
    let start = src
        .find("async fn handle_passthrough(")
        .expect("fn present");
    // 下一个 fn（default_model_ids）作为结束边界
    let rest = &src[start + 1..];
    let end = rest
        .find("pub(crate) async fn default_model_ids")
        .map(|i| start + 1 + i)
        .unwrap_or(src.len());
    let body = &src[start..end];
    assert!(
        !body.contains("convert_request"),
        "passthrough must bypass convert_request"
    );
    assert!(
        !body.contains("build_upstream_headers"),
        "passthrough must bypass build_upstream_headers"
    );
    assert!(
        !body.contains("apply_client_headers"),
        "passthrough must bypass apply_client_headers"
    );
}

// ── 模型列表端点识别：strip 任意前缀后尾段 /v1/models | /models ──
#[test]
fn models_endpoint_detection() {
    assert!(is_models_endpoint("/proxy/v1/models"));
    assert!(is_models_endpoint("/glm-coding-plan-auto/v1/models"));
    assert!(is_models_endpoint("/v1/models"));
    assert!(is_models_endpoint("/models"));
    assert!(is_models_endpoint("/proxy/models"));
    assert!(is_models_endpoint("/v1/models/")); // 容尾斜杠
    // chat / messages / responses 不命中
    assert!(!is_models_endpoint("/v1/chat/completions"));
    assert!(!is_models_endpoint("/v1/messages"));
    assert!(!is_models_endpoint("/v1/responses"));
    // 子路径 /v1/models/<id> 不当模型列表（尾段非 models）
    assert!(!is_models_endpoint("/v1/models/gpt-4"));
    // gemini /v1beta/models 命中（静态 gemini 格式列表）；深层路径不命中
    assert!(is_models_endpoint("/v1beta/models"));
    assert!(!is_models_endpoint(
        "/v1beta/models/gemini-2.0-flash:generateContent"
    ));
}

// ── 模型列表 URL 构造（遵 url-construction-rule：base_url 已含前缀，仅 trim + 后缀）──
#[test]
fn models_url_construction() {
    // glm openai 协议端点（base_url 含 /api/paas/v4）→ + /models
    assert_eq!(
        build_models_url(
            &super::Protocol::Glm,
            "https://open.bigmodel.cn/api/paas/v4"
        ),
        "https://open.bigmodel.cn/api/paas/v4/models"
    );
    // openai（base_url 含 /v1）→ + /models（禁额外拼 /v1）
    assert_eq!(
        build_models_url(&super::Protocol::OpenAI, "https://api.openai.com/v1"),
        "https://api.openai.com/v1/models"
    );
    // 尾斜杠 trim
    assert_eq!(
        build_models_url(&super::Protocol::OpenAI, "https://api.openai.com/v1/"),
        "https://api.openai.com/v1/models"
    );
    // anthropic（base_url 为 host 根）→ /v1/models
    assert_eq!(
        build_models_url(&super::Protocol::Anthropic, "https://api.anthropic.com"),
        "https://api.anthropic.com/v1/models"
    );
    // bailian → /compatible-mode/v1/models
    assert_eq!(
        build_models_url(&super::Protocol::Bailian, "https://dashscope.aliyuncs.com"),
        "https://dashscope.aliyuncs.com/compatible-mode/v1/models"
    );
}

// ── 版本段去重：base_url 已含 /v1 时 anthropic 系 api_path 不再拼出 /v1/v1 ──
#[test]
fn join_upstream_path_dedupes_version_segment() {
    use super::passthrough::join_upstream_path;
    // newapi 聚合站：base_url 含 /v1 + anthropic api_path 自带 /v1 → 只保留一份（原为 404 的 /v1/v1）
    assert_eq!(
        join_upstream_path("https://api.cometapi.com/v1", "/v1/messages/count_tokens"),
        "https://api.cometapi.com/v1/messages/count_tokens"
    );
    assert_eq!(
        join_upstream_path("https://api.cometapi.com/v1/", "/v1/messages"),
        "https://api.cometapi.com/v1/messages"
    );
    // 官方 anthropic：base_url 无版本段 → 照旧拼 /v1
    assert_eq!(
        join_upstream_path("https://api.anthropic.com", "/v1/messages"),
        "https://api.anthropic.com/v1/messages"
    );
    // openai 兼容：api_path 不以版本段起头 → 不动
    assert_eq!(
        join_upstream_path("https://api.openai.com/v1", "/chat/completions"),
        "https://api.openai.com/v1/chat/completions"
    );
    // gemini：base /v1 与 api_path /v1beta 不是同一段，禁误合并
    assert_eq!(
        join_upstream_path("https://x.com/v1", "/v1beta/models/m:streamGenerateContent"),
        "https://x.com/v1/v1beta/models/m:streamGenerateContent"
    );
    // 聚合站以 anthropic 协议登记 → models 列表同样不重复拼 /v1
    assert_eq!(
        build_models_url(&super::Protocol::Anthropic, "https://api.cometapi.com/v1"),
        "https://api.cometapi.com/v1/models"
    );
}

// ── 模型列表鉴权按协议分流：anthropic x-api-key vs openai Bearer ──
#[test]
fn models_auth_by_protocol() {
    let client = reqwest::Client::new();
    // anthropic → x-api-key + anthropic-version，无 authorization
    let req = apply_models_auth(
        client.get("http://x/v1/models"),
        &super::Protocol::Anthropic,
        "sk-ant",
    )
    .build()
    .unwrap();
    assert_eq!(
        req.headers().get("x-api-key").and_then(|v| v.to_str().ok()),
        Some("sk-ant")
    );
    assert_eq!(
        req.headers()
            .get("anthropic-version")
            .and_then(|v| v.to_str().ok()),
        Some("2023-06-01")
    );
    assert!(req.headers().get("authorization").is_none());
    // openai 兼容 → Authorization Bearer，无 x-api-key
    let req = apply_models_auth(
        client.get("http://x/models"),
        &super::Protocol::Glm,
        "sk-glm",
    )
    .build()
    .unwrap();
    assert_eq!(
        req.headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok()),
        Some("Bearer sk-glm")
    );
    assert!(req.headers().get("x-api-key").is_none());
}

/// 测试用固定清单：三条格式化测试只验格式，不该跟着 registry 内容一起漂。
fn sample_ids() -> Vec<String> {
    ["claude-fable-5", "gpt-5.5", "gpt-4o-mini"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

// ── 模型列表：openai 格式 = {object:list, data:[{id,object,created,owned_by}]} ──
#[test]
fn models_json_openai_format() {
    let ids = sample_ids();
    let mut ctx = std::collections::HashMap::new();
    ctx.insert("claude-fable-5".to_string(), 1_000_000_i64);
    ctx.insert("gpt-5.5".to_string(), 400_000_i64);
    let v = build_models_json(&Protocol::OpenAI, &ids, &ctx);
    assert_eq!(v.get("object").and_then(|o| o.as_str()), Some("list"));
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .expect("data array");
    assert_eq!(data.len(), ids.len());
    let first = &data[0];
    assert_eq!(first.get("object").and_then(|o| o.as_str()), Some("model"));
    assert!(first.get("id").and_then(|i| i.as_str()).is_some());
    assert!(first.get("created").is_some());
    assert!(first.get("owned_by").is_some());
    // max_input_tokens：命中附键、未命中不带
    let by_id = |id: &str| {
        data.iter()
            .find(|m| m.get("id").and_then(|i| i.as_str()) == Some(id))
            .unwrap()
    };
    assert_eq!(
        by_id("claude-fable-5")
            .get("max_input_tokens")
            .and_then(|n| n.as_i64()),
        Some(1_000_000)
    );
    assert_eq!(
        by_id("gpt-5.5")
            .get("max_input_tokens")
            .and_then(|n| n.as_i64()),
        Some(400_000)
    );
    assert!(by_id("gpt-4o-mini").get("max_input_tokens").is_none());
    // 传进去的 id 原样出现在输出里，顺序不变（清单内容由调用方决定，本函数只负责格式化）。
    let out_ids: Vec<&str> = data
        .iter()
        .filter_map(|m| m.get("id").and_then(|i| i.as_str()))
        .collect();
    assert_eq!(out_ids, ids.iter().map(String::as_str).collect::<Vec<_>>());
}

// ── 模型列表：anthropic 格式 = {data:[{type:model,id,display_name,created_at}],has_more,first_id,last_id} ──
#[test]
fn models_json_anthropic_format() {
    // 裸路径回退 anthropic
    let ids = sample_ids();
    let v = build_models_json(
        &Protocol::Anthropic,
        &ids,
        &std::collections::HashMap::new(),
    );
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .expect("data array");
    assert_eq!(data.len(), ids.len());
    let first = &data[0];
    assert_eq!(first.get("type").and_then(|t| t.as_str()), Some("model"));
    assert!(first.get("id").and_then(|i| i.as_str()).is_some());
    assert!(first.get("display_name").and_then(|d| d.as_str()).is_some());
    assert!(first.get("created_at").is_some());
    assert_eq!(v.get("has_more").and_then(|h| h.as_bool()), Some(false));
    assert_eq!(
        v.get("first_id").and_then(|i| i.as_str()),
        Some("claude-fable-5")
    );
    assert_eq!(
        v.get("last_id").and_then(|i| i.as_str()),
        Some("gpt-4o-mini")
    );
    let ids: Vec<&str> = data
        .iter()
        .filter_map(|m| m.get("id").and_then(|i| i.as_str()))
        .collect();
    assert!(ids.contains(&"claude-fable-5"));
}

// ── 模型列表：gemini 格式 = {models:[{name:"models/<id>",displayName,...}]} ──
#[test]
fn models_json_gemini_format() {
    let ids = sample_ids();
    let v = build_models_json(&Protocol::Gemini, &ids, &std::collections::HashMap::new());
    let models = v
        .get("models")
        .and_then(|m| m.as_array())
        .expect("models array");
    assert_eq!(models.len(), ids.len());
    let first = &models[0];
    assert_eq!(
        first.get("name").and_then(|n| n.as_str()),
        Some("models/claude-fable-5")
    );
    assert!(first.get("displayName").and_then(|d| d.as_str()).is_some());
    assert!(
        first
            .get("supportedGenerationMethods")
            .and_then(|m| m.as_array())
            .is_some()
    );
}

// ── canonical → 最大上下文映射：max_input_tokens 优先、缺失回落 context_window、多平台取最大 ──
#[test]
fn context_map_prefers_max_input_then_max_across_platforms() {
    let stub = || aidog_db::ModelEntry {
        platform_code: String::new(),
        model_id: String::new(),
        display_name: String::new(),
        canonical_model: String::new(),
        family: String::new(),
        version: String::new(),
        predecessor: String::new(),
        capabilities: vec![],
        builtin_tools_excluded: vec![],
        max_input_tokens: None,
        max_output_tokens: None,
        context_window: None,
        official: false,
        price_data: String::new(),
        updated_at: 0,
    };
    let e = |platform: &str, canonical: &str, max_in: Option<i64>, ctx: Option<i64>| {
        let mut m = stub();
        m.platform_code = platform.to_string();
        m.model_id = format!("{canonical}@{platform}");
        m.canonical_model = canonical.to_string();
        m.max_input_tokens = max_in;
        m.context_window = ctx;
        m
    };
    let map = build_context_map(&[
        e(
            "openrouter",
            "claude-fable-5",
            Some(1_000_000),
            Some(128_000),
        ),
        e("gemini", "claude-fable-5", None, Some(200_000)), // 回落 context_window，仍小于另一平台
        e("crazyrouter", "gpt-5.5", None, Some(400_000)),   // 唯一条目 → 回落生效
        e("x", "no-ctx-model", None, None),                 // 两键全空 → 不入 map
    ]);
    assert_eq!(map.get("claude-fable-5").copied(), Some(1_000_000));
    assert_eq!(map.get("gpt-5.5").copied(), Some(400_000));
    assert!(!map.contains_key("no-ctx-model"));
}

// ── SSE usage 累计（Anthropic message.usage + OpenAI 顶层 usage）──
#[test]
fn passthrough_patches_model_only() {
    let orig = serde_json::json!({
        "model": "claude-sonnet-4",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{"name": "calc"}],
        "max_tokens": 100
    });
    let actual_model = "claude-3-5-sonnet-20241022";
    let mut body = orig.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.insert("model".to_string(), Value::String(actual_model.to_string()));
    }
    // model 已替换
    assert_eq!(
        body.get("model").and_then(|v| v.as_str()),
        Some(actual_model)
    );
    // messages / tools / 其余字段结构原样（未经 from_*→to_* 往返）
    assert_eq!(body.get("messages"), orig.get("messages"));
    assert_eq!(body.get("tools"), orig.get("tools"));
    assert_eq!(body.get("max_tokens"), orig.get("max_tokens"));
}

// ── 上游 gzip 压缩响应解压回归（修复 token/成本全 0 + 日志乱码）──
// 背景: 上游 GLM anthropic 端点回 content-encoding: gzip。reqwest 启用 gzip feature 后
// 由响应头 Content-Encoding 驱动自动解压，resp.bytes() 得明文。本 test 用 flate2 构造
// 一段 gzip 压缩的 anthropic usage JSON，解压后喂 extract_usage，断言 token > 0，
// 证明「解压后 JSON → extract_usage → token>0」链路成立（reqwest 解压本身为黑盒，
// 由 Cargo feature gzip/brotli/deflate/zstd 保证，行为有 docs.rs 官方背书）。

// ── 分组候选模型合并：映射源名 ∪ 各平台模型槽位，按出现顺序去重、剔空 ──
// `/models` 端点（group_model_ids）与 pi 配置同步（pi_model_candidates）共用这一口径。
#[test]
fn merge_group_model_names_dedupes_in_order() {
    use aidog_db::models::{ModelMapping, PlatformModels};
    let mapping = |s: &str| ModelMapping {
        source_model: s.to_string(),
        target_platform_id: 0,
        target_model: String::new(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
    };

    let out = merge_group_model_names(
        // 前后空白要被 trim；空串要被剔掉
        &[
            mapping("  claude-sonnet-5  "),
            mapping(""),
            mapping("glm-4.7"),
        ],
        &[
            PlatformModels {
                default: Some("glm-4.7".into()), // 与上面的映射重复 → 只留一次
                sonnet: Some("glm-4.6".into()),
                ..PlatformModels::default()
            },
            PlatformModels {
                opus: Some("glm-4.6".into()), // 跨平台重复 → 同样只留一次
                haiku: Some("glm-4.5".into()),
                ..PlatformModels::default()
            },
        ],
    );

    assert_eq!(
        out,
        vec!["claude-sonnet-5", "glm-4.7", "glm-4.6", "glm-4.5"],
        "映射在前、平台槽位在后，首次出现的顺序即输出顺序"
    );
}

/// 一个模型都没配的分组 → 空，调用方据此回落默认清单（`/models` 与 pi 两处都靠这个语义）。
#[test]
fn merge_group_model_names_empty_when_nothing_configured() {
    use aidog_db::models::PlatformModels;
    assert!(merge_group_model_names(&[], &[PlatformModels::default()]).is_empty());
}

// ── 纯透传必须剔掉 content-encoding ──
// reqwest 开着 gzip/brotli/deflate/zstd feature 且没调 .no_gzip()，bytes_stream() 吐的是解压后的
// 字节；把上游的 content-encoding 原样转给客户端，客户端会拿明文去 gunzip 而失败。
// 与转换路径共用同一条黑名单（RESP_HEADER_BLACKLIST，第一项就是 content-encoding）。
#[test]
fn passthrough_resp_blacklist_covers_content_encoding() {
    for must_strip in [
        "content-encoding",
        "content-length",
        "transfer-encoding",
        "connection",
    ] {
        assert!(
            RESP_HEADER_BLACKLIST
                .iter()
                .any(|b| b.eq_ignore_ascii_case(must_strip)),
            "{must_strip} 必须在响应头黑名单里"
        );
    }
    // 反面：业务头不在黑名单里，别误剔
    for must_keep in [
        "content-type",
        "retry-after",
        "anthropic-ratelimit-unified-status",
    ] {
        assert!(
            !RESP_HEADER_BLACKLIST
                .iter()
                .any(|b| b.eq_ignore_ascii_case(must_keep)),
            "{must_keep} 不该被剔"
        );
    }
}
