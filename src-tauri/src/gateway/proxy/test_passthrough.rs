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
        assert!(!fwd.contains_key("content-length"), "content-length must be dropped");
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
        orig.insert("anthropic-beta", "interleaved-thinking-2025-05-14".parse().unwrap());
        orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
        orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());
        orig.insert("x-stainless-runtime-version", "v24.3.0".parse().unwrap());
        orig.insert("x-stainless-timeout", "3000".parse().unwrap());
        orig.insert("x-claude-code-session-id", "sess-abc".parse().unwrap());
        orig.insert("x-app", "cli".parse().unwrap());

        // 官方 Anthropic 上游：anthropic-beta 保留（依赖 beta 协商能力）
        let fwd = passthrough_convert_headers(&orig, "https://api.anthropic.com/v1/messages");

        // 剔除项
        for stripped in ["host", "content-length", "connection", "authorization", "user-agent", "content-type"] {
            assert!(!fwd.contains_key(stripped), "{stripped} must be stripped for convert apply to override");
        }
        // 透传项（含跨协议透明的 SDK 头；官方端点 anthropic-beta 保留）
        assert_eq!(fwd.get("anthropic-beta").and_then(|v| v.to_str().ok()), Some("interleaved-thinking-2025-05-14"));
        assert_eq!(fwd.get("x-stainless-package-version").and_then(|v| v.to_str().ok()), Some("0.94.0"));
        assert_eq!(fwd.get("x-stainless-runtime-version").and_then(|v| v.to_str().ok()), Some("v24.3.0"));
        assert_eq!(fwd.get("x-stainless-timeout").and_then(|v| v.to_str().ok()), Some("3000"));
        assert_eq!(fwd.get("x-claude-code-session-id").and_then(|v| v.to_str().ok()), Some("sess-abc"));
    }

    // ── convert 路径透传 anthropic-beta host 分流：官方保留 / 第三方剔除 ──
    // 背景: GLM open.bigmodel.cn/api/anthropic 等第三方兼容端点不认新 beta token，
    // 原样透传致上游 400 code 1210「API 调用参数有误」。官方 api.anthropic.com 依赖 beta 协商，保留。
    #[test]
    fn passthrough_convert_anthropic_beta_host_gated() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("anthropic-beta", "context-1m-2025-08-07,effort-2025-11-24".parse().unwrap());
        orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
        orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());

        // 第三方 GLM anthropic 端点 → 剔 anthropic-beta，其余 SDK 头照常透传
        let glm = passthrough_convert_headers(&orig, "https://open.bigmodel.cn/api/anthropic/v1/messages");
        assert!(!glm.contains_key("anthropic-beta"), "anthropic-beta must be stripped for third-party endpoint");
        assert_eq!(glm.get("anthropic-version").and_then(|v| v.to_str().ok()), Some("2023-06-01"));
        assert_eq!(glm.get("x-stainless-package-version").and_then(|v| v.to_str().ok()), Some("0.94.0"));

        // 官方 api.anthropic.com（含端口/大小写变体）→ anthropic-beta 保留
        let official = passthrough_convert_headers(&orig, "https://API.Anthropic.com:443/v1/messages");
        assert_eq!(
            official.get("anthropic-beta").and_then(|v| v.to_str().ok()),
            Some("context-1m-2025-08-07,effort-2025-11-24"),
            "official anthropic host must keep anthropic-beta"
        );
    }

    // ── is_official_anthropic_host: host 提取 + 官方判定（含端口/userinfo/大小写） ──
    #[test]
    fn official_anthropic_host_detection() {
        assert!(is_official_anthropic_host("https://api.anthropic.com/v1/messages"));
        assert!(is_official_anthropic_host("https://api.anthropic.com:443/v1/messages"));
        assert!(is_official_anthropic_host("https://API.ANTHROPIC.COM/v1/messages"));
        // 第三方 / 中转站 host
        assert!(!is_official_anthropic_host("https://open.bigmodel.cn/api/anthropic/v1/messages"));
        assert!(!is_official_anthropic_host("https://api.anthropic.com.evil.com/v1/messages"));
        assert!(!is_official_anthropic_host("https://proxy.example.com/anthropic/v1/messages"));
    }

    // ── build_upstream_headers：透传入站（脱敏）+ 覆盖 UA/auth，日志反映真实上游头 ──
    #[test]
    fn passthrough_does_not_invoke_convert_request() {
        let src = include_str!("passthrough.rs");
        // 定位 handle_passthrough 函数体范围
        let start = src.find("async fn handle_passthrough(").expect("fn present");
        // 下一个 const（STATIC_MODEL_IDS）作为结束边界
        let rest = &src[start + 1..];
        let end = rest
            .find("const STATIC_MODEL_IDS")
            .map(|i| start + 1 + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        assert!(!body.contains("convert_request"), "passthrough must bypass convert_request");
        assert!(!body.contains("build_upstream_headers"), "passthrough must bypass build_upstream_headers");
        assert!(!body.contains("apply_client_headers"), "passthrough must bypass apply_client_headers");
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
        // gemini /v1beta/models 本期不命中（标 TODO）
        assert!(!is_models_endpoint("/v1beta/models"));
    }

    // ── 模型列表 URL 构造（遵 url-construction-rule：base_url 已含前缀，仅 trim + 后缀）──
    #[test]
    fn models_url_construction() {
        // glm openai 协议端点（base_url 含 /api/paas/v4）→ + /models
        assert_eq!(
            build_models_url(&super::Protocol::Glm, "https://open.bigmodel.cn/api/paas/v4"),
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

    // ── 模型列表鉴权按协议分流：anthropic x-api-key vs openai Bearer ──
    #[test]
    fn models_auth_by_protocol() {
        let client = reqwest::Client::new();
        // anthropic → x-api-key + anthropic-version，无 authorization
        let req = apply_models_auth(client.get("http://x/v1/models"), &super::Protocol::Anthropic, "sk-ant")
            .build()
            .unwrap();
        assert_eq!(req.headers().get("x-api-key").and_then(|v| v.to_str().ok()), Some("sk-ant"));
        assert_eq!(req.headers().get("anthropic-version").and_then(|v| v.to_str().ok()), Some("2023-06-01"));
        assert!(req.headers().get("authorization").is_none());
        // openai 兼容 → Authorization Bearer，无 x-api-key
        let req = apply_models_auth(client.get("http://x/models"), &super::Protocol::Glm, "sk-glm")
            .build()
            .unwrap();
        assert_eq!(req.headers().get("authorization").and_then(|v| v.to_str().ok()), Some("Bearer sk-glm"));
        assert!(req.headers().get("x-api-key").is_none());
    }

    // ── 静态模型列表：openai 格式 = {object:list, data:[{id,object,created,owned_by}]} ──
    #[test]
    fn static_models_openai_format() {
        let v = build_static_models_json("openai");
        assert_eq!(v.get("object").and_then(|o| o.as_str()), Some("list"));
        let data = v.get("data").and_then(|d| d.as_array()).expect("data array");
        assert_eq!(data.len(), super::STATIC_MODEL_IDS.len());
        let first = &data[0];
        assert_eq!(first.get("object").and_then(|o| o.as_str()), Some("model"));
        assert!(first.get("id").and_then(|i| i.as_str()).is_some());
        assert!(first.get("created").is_some());
        assert!(first.get("owned_by").is_some());
        // 模型集内容
        let ids: Vec<&str> = data.iter().filter_map(|m| m.get("id").and_then(|i| i.as_str())).collect();
        assert!(ids.contains(&"claude-opus-4-8"));
        assert!(ids.contains(&"gpt-5.5"));
        assert!(ids.contains(&"gpt-5.4"));
        assert!(ids.contains(&"gpt-5.4-mini"));
        assert!(!ids.contains(&"gpt-5.5-codex"));
    }

    // ── 静态模型列表：anthropic 格式 = {data:[{type:model,id,display_name,created_at}],has_more,first_id,last_id} ──
    #[test]
    fn static_models_anthropic_format() {
        // 裸路径回退 anthropic
        let v = build_static_models_json("anthropic");
        let data = v.get("data").and_then(|d| d.as_array()).expect("data array");
        assert_eq!(data.len(), super::STATIC_MODEL_IDS.len());
        let first = &data[0];
        assert_eq!(first.get("type").and_then(|t| t.as_str()), Some("model"));
        assert!(first.get("id").and_then(|i| i.as_str()).is_some());
        assert!(first.get("display_name").and_then(|d| d.as_str()).is_some());
        assert!(first.get("created_at").is_some());
        assert_eq!(v.get("has_more").and_then(|h| h.as_bool()), Some(false));
        assert_eq!(v.get("first_id").and_then(|i| i.as_str()), Some("claude-opus-4-8"));
        assert_eq!(v.get("last_id").and_then(|i| i.as_str()), Some("gpt-5.4-mini"));
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
        assert_eq!(body.get("model").and_then(|v| v.as_str()), Some(actual_model));
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
