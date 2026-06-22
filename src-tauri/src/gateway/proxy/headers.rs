use super::*;

/// 构建透传转发 header：原样保留客户端全部 header（含 Authorization OAuth），
/// 仅剔除 hop-by-hop（Host / Content-Length，由 reqwest 按目标 URL + body 重设）。
pub(crate) fn passthrough_headers(orig: &axum::http::HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (k, v) in orig {
        let name = k.as_str();
        if name.eq_ignore_ascii_case("host") || name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out.append(hn, hv);
        }
    }
    out
}

/// hop-by-hop + 强覆盖头名（convert 路径透传时剔除）。
/// host / content-length / 标准 hop-by-hop（RFC 7230 §6.1）交给 reqwest 按目标重设；
/// auth 三件套 / user-agent / content-type 由 apply_client_headers 用平台配置覆盖，
/// 故透传底座剔除，避免同名 append 造成多值。
const STRIPPED_ON_CONVERT_PASSTHROUGH: &[&str] = &[
    "host",
    "content-length",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "authorization",
    "x-api-key",
    "x-goog-api-key",
    "user-agent",
    "content-type",
];

/// 判定上游 URL 是否指向 Anthropic 官方接口（host == api.anthropic.com，忽略大小写 + 端口）。
/// 仅官方接口依赖 `anthropic-beta` 头协商能力（1m-context / interleaved-thinking 等）；
/// 第三方 anthropic 兼容端点（GLM open.bigmodel.cn / 各中转站）不认新 beta token，
/// 原样透传会触发上游参数校验失败（如 GLM 400 code 1210）。
/// 故 convert/forward 透传路径仅对官方接口保留 anthropic-beta，对第三方端点剔除（见 strip_anthropic_beta_for_third_party）。
pub(crate) fn is_official_anthropic_host(upstream_url: &str) -> bool {
    // 提取 host：scheme://host[:port]/path → host
    let after_scheme = upstream_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(upstream_url);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    // 去 userinfo（user:pass@host）+ 端口
    let host = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority)
        .split(':')
        .next()
        .unwrap_or("");
    host.eq_ignore_ascii_case("api.anthropic.com")
}

/// 是否应在透传路径剔除入站 `anthropic-beta` 头。
/// 上游非 Anthropic 官方接口 → true（第三方兼容端点不依赖 beta 协商，原样透传会被参数校验拒）。
fn strip_anthropic_beta_for_third_party(name: &str, upstream_url: &str) -> bool {
    name.eq_ignore_ascii_case("anthropic-beta") && !is_official_anthropic_host(upstream_url)
}

/// 鉴权凭证头名（proxy_log 脱敏判定，不区分大小写）。
/// `api-key` 系小米 token-plan openai 端点要求的鉴权头（与 Authorization 同发），属凭证须 redact。
const SENSITIVE_AUTH_HEADERS: &[&str] = &[
    "authorization",
    "api-key",
    "x-api-key",
    "x-goog-api-key",
];

/// 判定 header 是否为需脱敏的鉴权凭证头（不区分大小写）。
pub(crate) fn is_sensitive_auth_header(name: &str) -> bool {
    SENSITIVE_AUTH_HEADERS.iter().any(|h| name.eq_ignore_ascii_case(h))
}

/// convert 路径透传入站头底座：全量入站头，剔 hop-by-hop + auth/UA/CT（由 apply 覆盖）。
/// 其余（anthropic-* / x-stainless-* / x-app / session-id / originator / version / 未知自定义头）
/// 原样透传 —— 跨协议（如 CC 入站转 OpenAI）也带，上游忽略未知头不报错，保留利于诊断。
/// 例外：`anthropic-beta` 仅发给 Anthropic 官方接口；第三方 anthropic 兼容端点剔除（不认新 beta token，
/// 原样透传致上游参数校验失败，如 GLM 400 code 1210）。upstream_url 用于 host 判定。
pub(crate) fn passthrough_convert_headers(
    orig: &axum::http::HeaderMap,
    upstream_url: &str,
) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (k, v) in orig {
        let name = k.as_str();
        if STRIPPED_ON_CONVERT_PASSTHROUGH.iter().any(|s| name.eq_ignore_ascii_case(s)) {
            continue;
        }
        if strip_anthropic_beta_for_third_party(name, upstream_url) {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out.append(hn, hv);
        }
    }
    out
}
pub fn apply_client_headers(
    req_builder: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    match client_type {
        ClientType::Default => apply_default_headers(req_builder, protocol, api_key),
        // Claude Code family — 共享 Stainless SDK headers，仅 UA 不同
        ClientType::ClaudeCode
        | ClientType::ClaudeCodeVscode
        | ClientType::ClaudeCodeSdkTs
        | ClientType::ClaudeCodeSdkPy
        | ClientType::ClaudeCodeGhAction => {
            apply_claude_code_family_headers(req_builder, client_type, protocol, api_key)
        }
        // Codex family — 共享 Codex 基础 headers，仅 UA 不同
        ClientType::CodexCli
        | ClientType::CodexTui
        | ClientType::CodexDesktop
        | ClientType::CodexVscode => {
            apply_codex_family_headers(req_builder, client_type, protocol, api_key)
        }
        ClientType::Cursor => apply_cursor_headers(req_builder, protocol, api_key),
        ClientType::Windsurf => apply_windsurf_headers(req_builder, protocol, api_key),
    }
}

/// 根据 ClientType 子变体返回 Claude Code 家族的 User-Agent 字符串。
/// 格式: claude-cli/<version> (external, <entrypoint>[, agent-sdk/<sdk_ver>])
fn claude_code_ua(client_type: &ClientType) -> &'static str {
    match client_type {
        ClientType::ClaudeCode => "claude-cli/1.0.117 (external, cli)",
        ClientType::ClaudeCodeVscode => "claude-cli/1.0.117 (external, claude-vscode, agent-sdk/0.1.30)",
        ClientType::ClaudeCodeSdkTs => "claude-cli/1.0.117 (external, sdk-ts)",
        ClientType::ClaudeCodeSdkPy => "claude-cli/1.0.117 (external, sdk-py)",
        ClientType::ClaudeCodeGhAction => "claude-cli/1.0.117 (external, claude-code-github-action)",
        _ => "claude-cli/1.0.117 (external, cli)",
    }
}

/// 根据 ClientType 子变体返回 Codex 家族的 User-Agent 字符串
fn codex_ua(client_type: &ClientType) -> &'static str {
    match client_type {
        ClientType::CodexCli => "codex_cli_rs/0.38.0 (MacOS; arm64) Terminal",
        ClientType::CodexTui => "Codex/0.38.0",
        ClientType::CodexDesktop => "codex desktop/0.38.0",
        ClientType::CodexVscode => "codex-vscode/0.38.0",
        _ => "codex_cli_rs/0.38.0 (MacOS; arm64) Terminal",
    }
}

fn apply_default_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    // 仅设 auth（UA/Content-Type 由别处，其余入站头透传）。anthropic-version 走入站透传。
    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            // openai/兼容：Bearer 之外叠加 api-key 头（小米 token-plan openai 端点要求）。
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("api-key", api_key);
        }
    }
    rb
}

/// Claude Code 家族：仅设 User-Agent + auth（覆盖）。
/// Stainless SDK 头（x-stainless-* / anthropic-version / anthropic-beta /
/// anthropic-dangerous-direct-browser-access / x-app / x-claude-code-session-id）
/// 由 convert 路径从入站透传（passthrough_convert_headers），不再硬编码静态默认 ——
/// 上游可见客户端真实 SDK 版本/会话，跨协议（CC→OpenAI）也带（透明自定义头）。
/// 来源: @anthropic-ai/claude-code/cli.js — buildHeaders() + fV()
/// 参考: claude-code-hub client-detector.ts — confirmClaudeCodeSignals()
fn apply_claude_code_family_headers(
    mut rb: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb.header("User-Agent", claude_code_ua(client_type));

    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
        }
        super::models::Protocol::OpenAI => {
            // openai：Bearer 之外叠加 api-key 头（小米 token-plan openai 端点要求）。
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("api-key", api_key);
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("api-key", api_key);
        }
    }
    rb
}

/// Codex 家族：仅设 UA + auth + OpenAI 协议必需（OpenAI-Beta / session_id / conversation_id）。
/// originator/version/Accept 等由入站透传。session_id/conversation_id 入站无则生成。
/// 来源: codex-rs/core/src/default_client.rs + model_provider_info.rs + client.rs
/// 参考: claude-code-hub client-detector.ts — CODEX_FAMILY_RULES
fn apply_codex_family_headers(
    mut rb: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb.header("User-Agent", codex_ua(client_type));

    match protocol {
        super::models::Protocol::OpenAI => {
            // openai：Bearer 之外叠加 api-key 头（小米 token-plan openai 端点要求）。
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("api-key", api_key)
                .header("OpenAI-Beta", "responses=experimental")
                .header("conversation_id", uuid_sim())
                .header("session_id", uuid_sim());
        }
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
    }
    rb
}

/// 模拟 Cursor IDE：仅 UA + auth。x-app / anthropic-version 由入站透传。
/// 来源: GitHub 逆向 — 使用 Anthropic SDK 但有特定 header 组合
fn apply_cursor_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb.header("User-Agent", "Cursor/0.50.7");

    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
        }
        super::models::Protocol::OpenAI => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
    }
    rb
}

/// 模拟 Windsurf IDE：仅 UA + auth。x-app / anthropic-version 由入站透传。
/// 来源: GitHub 逆向 — 类似 Cursor，使用 Anthropic SDK
fn apply_windsurf_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb.header("User-Agent", "Windsurf/1.5.0");

    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
        }
        super::models::Protocol::OpenAI => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
    }
    rb
}

/// 生成简易 UUID v4 格式的随机字符串
fn uuid_sim() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:08x}-{:04}-4{:03}-{:04}-{:012x}",
        (ts as u32).wrapping_mul(0x45d9f3b),
        (ts >> 16) as u16,
        ((ts >> 32) as u16) & 0x0fff,
        ((ts >> 48) as u16) | 0x8000,
        ((ts >> 60) as u64) & 0xffffffffffff,
    )
}

/// 构建上游请求头 KV 表（用于日志记录，反映实际发送：入站透传 + apply 覆盖）。
/// 透传头从 orig 取并脱敏（auth/cookie），覆盖头（UA/auth/CT + codex 协议必需）按 apply 逻辑。
/// upstream_url 用于 anthropic-beta host 判定（须与 passthrough_convert_headers 同参，日志与实发一致）。
pub fn build_upstream_headers(
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
    orig: &axum::http::HeaderMap,
    upstream_url: &str,
) -> Vec<(String, String)> {
    let mut h: Vec<(String, String)> = Vec::new();
    // ① 透传入站头（剔 stripped：hop-by-hop + auth/UA/CT；非官方 anthropic 端点剔 anthropic-beta）。脱敏敏感值。
    for (k, v) in orig {
        let name = k.as_str();
        if STRIPPED_ON_CONVERT_PASSTHROUGH.iter().any(|s| name.eq_ignore_ascii_case(s)) {
            continue;
        }
        if strip_anthropic_beta_for_third_party(name, upstream_url) {
            continue;
        }
        let val = v.to_str().unwrap_or("");
        let val = if name.eq_ignore_ascii_case("cookie") || name.eq_ignore_ascii_case("set-cookie") {
            "[REDACTED]".to_string()
        } else {
            val.to_string()
        };
        h.push((name.to_string(), val));
    }
    // ② 覆盖：Content-Type + auth（redact_key 日志安全）+ UA + codex 协议必需。
    h.push(("Content-Type".into(), "application/json".into()));
    match protocol {
        super::models::Protocol::Anthropic => {
            h.push(("x-api-key".into(), redact_key(api_key)));
        }
        super::models::Protocol::Gemini => {
            h.push(("x-goog-api-key".into(), redact_key(api_key)));
        }
        _ => {
            h.push(("Authorization".into(), format!("Bearer {}", redact_key(api_key))));
        }
    }
    match client_type {
        ClientType::Default => {}
        ClientType::ClaudeCode
        | ClientType::ClaudeCodeVscode
        | ClientType::ClaudeCodeSdkTs
        | ClientType::ClaudeCodeSdkPy
        | ClientType::ClaudeCodeGhAction => {
            h.push(("User-Agent".into(), claude_code_ua(client_type).into()));
        }
        ClientType::CodexCli
        | ClientType::CodexTui
        | ClientType::CodexDesktop
        | ClientType::CodexVscode => {
            h.push(("User-Agent".into(), codex_ua(client_type).into()));
            if matches!(protocol, super::models::Protocol::OpenAI) {
                h.push(("OpenAI-Beta".into(), "responses=experimental".into()));
                h.push(("conversation_id".into(), uuid_sim()));
                h.push(("session_id".into(), uuid_sim()));
            }
        }
        ClientType::Cursor => {
            h.push(("User-Agent".into(), "Cursor/0.50.7".into()));
        }
        ClientType::Windsurf => {
            h.push(("User-Agent".into(), "Windsurf/1.5.0".into()));
        }
    }
    h
}

/// Redact API key: show first 4 and last 4 chars, mask the rest
pub fn redact_key(key: &str) -> String {
    if key.len() <= 12 {
        "[REDACTED]".into()
    } else {
        format!("{}****{}", &key[..4], &key[key.len()-4..])
    }
}

/// 为 Coding Plan 端点注入平台特有字段
/// - Kimi Code Plan: 注入 prompt_cache_key（必填，用 group + model hash 作会话标识）
pub fn inject_coding_plan_fields(body: &mut Value, protocol: &super::models::Protocol) {
    match protocol {
        super::models::Protocol::Kimi => {
            // Kimi Code Plan 要求 prompt_cache_key 以提升缓存命中率
            // 用模型名 + 短随机串生成会话级 cache key
            if let Some(obj) = body.as_object_mut() {
                let model = obj.get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let session_id = format!("aidog-{}-{:06x}",
                    model,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() / 300  // 5-minute window
                );
                obj.insert(
                    "prompt_cache_key".to_string(),
                    Value::String(session_id),
                );
            }
        }
        _ => {
            // GLM / MiniMax / 百炼 等 coding plan 暂无额外字段
        }
    }
}

/// Coding Plan 的 API 路径覆盖（当前各平台 base_url 已区分 coding/normal，api_path 无需变更）
pub fn override_coding_plan_path(_api_path: &mut String, _protocol: &super::models::Protocol) {
    // 预留：后续若有平台需 coding plan 专用 api_path 可在此扩展
}

/// Pretty-print JSON string; return original if parsing fails
pub(crate) fn format_pretty_json(s: &str) -> String {
    serde_json::from_str::<Value>(s)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
#[path = "test_headers.rs"]
mod test_headers;
