use super::*;

/// Debug build (`make run` = yarn tauri dev = debug) 下，给 AirDog 构造的响应注入
/// `X-AiDog-Trace: <id>`，便于客户端报错（如 h2 CANCEL）时关联 AirDog 侧 proxy_log / span。
///
/// - **id 取值链**：`current_trace_id()`（读线程活跃 span 链最内层 trace_id/request_id）→ 否则 `new_trace_id()`（6 [0-9a-z] 兜底，与日志行 traceid 字段格式一致以保 grep 直达）。
/// - **gate**：`cfg!(debug_assertions)` 编译期判定；release build 调用点空操作（无 header 插入 + 无运行时开销）。
/// - **覆盖范围**：凡 AirDog 直构的 proxy 响应（健康端点 / models / group-info / count_tokens / forward / passthrough / responses / mock / non_success / CONNECT 200）。
/// - **豁免**：`blind_relay_after_connect`（TCP 字节透传，AirDog 看不见/改不了 HTTP 层；CONNECT 200 响应本身已注入）。
pub(crate) fn inject_trace_header(response: &mut axum::response::Response) {
    if !cfg!(debug_assertions) {
        return;
    }
    let id = crate::logging::current_trace_id().unwrap_or_else(crate::logging::new_trace_id);
    if let Ok(hv) = axum::http::HeaderValue::from_str(&id) {
        response
            .headers_mut()
            .insert(axum::http::HeaderName::from_static("x-aidog-trace"), hv);
    }
}

/// 非回环来源的本地端点请求直接 403（bind_lan=true 监听 0.0.0.0 时同样只放行本机）。
/// `who` 仅用于 warn 文案前缀（如 "group-info" / "notify"）。
pub(crate) fn reject_non_loopback(addr: &std::net::SocketAddr, who: &str) -> Response {
    tracing::warn!(peer = %addr, "{who}: non-loopback request rejected");
    let mut r = StatusCode::FORBIDDEN.into_response();
    inject_trace_header(&mut r);
    r
}

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
    "api-key",
    "x-goog-api-key",
    "user-agent",
    "content-type",
];

/// 判定上游 URL 是否指向 Anthropic 官方接口（host == api.anthropic.com，忽略大小写 + 端口）。
///
/// 2026-09-21 起**不再**据此剔除第三方的 `anthropic-beta`：官方明令该头要 verbatim 转发、
/// 禁按值 allowlist（<https://code.claude.com/docs/en/llm-gateway-protocol#request-headers>），
/// 且实测 GLM 与 CometAPI 对四种 beta 值全部 200，旧注释里「第三方不认新 beta token 会
/// 400 code 1210」的前提已不成立。本函数现在只用于 `forward.rs` 的 beta 自动降级闸门
/// ——官方上游依赖 beta 协商，永不降级。
pub(crate) fn is_official_anthropic_host(upstream_url: &str) -> bool {
    // 提取 host：scheme://host[:port]/path → host
    let after_scheme = upstream_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(upstream_url);
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
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

/// 鉴权凭证头名（不区分大小写）。
/// `api-key` 系小米 token-plan openai 端点要求的鉴权头（与 Authorization 同发），属凭证。
const SENSITIVE_AUTH_HEADERS: &[&str] =
    &["authorization", "api-key", "x-api-key", "x-goog-api-key"];

/// 判定 header 是否为鉴权凭证头（不区分大小写）。
/// 职责是 inject denylist（见 `sanitize_header_injects`）；日志脱敏判定走 `is_sensitive_log_header`。
pub(crate) fn is_sensitive_auth_header(name: &str) -> bool {
    SENSITIVE_AUTH_HEADERS
        .iter()
        .any(|h| name.eq_ignore_ascii_case(h))
}

/// 判定 header 是否为落库前须脱敏的敏感头（不区分大小写；值经 `redact_log_header_value`，认证 scheme 保留）。
/// 全仓库唯一一份日志脱敏判定（用户侧与上游侧共用，db-log-redact-hardening 票 01）：
/// 鉴权四头（派生自 `SENSITIVE_AUTH_HEADERS`，不另抄一份）+ cookie/set-cookie（浏览器型客户端的 session 凭据）。
/// 所有落库 headers 的路径（用户请求捕获、上游请求、上游/用户响应）共用本判定。
pub(crate) fn is_sensitive_log_header(name: &str) -> bool {
    is_sensitive_auth_header(name)
        || name.eq_ignore_ascii_case("cookie")
        || name.eq_ignore_ascii_case("set-cookie")
}

/// 中间件 `inject`/`header_set` **不得覆盖**的头名（不区分大小写）。
///
/// 两类，都是「代理自己拥有」的头，被规则覆盖只会坏事而不是配置能力：
///   1. 认证凭证（`SENSITIVE_AUTH_HEADERS`）——覆盖 = 拿用户规则替换平台 api_key，
///      等于给了中间件一个绕过平台凭证的口子；安全上一律拒绝。
///   2. 传输/形态头（host / content-length / content-type / user-agent 及 hop-by-hop）——
///      由 reqwest 按目标 URL+body 重设，或由 `apply_client_headers` 按 client_type 模拟；
///      注入后会被覆盖或产生同名多值，不如当场拒绝并告诉用户。
///
/// 其余头（含 cookie、各类自定义头）允许注入。
const HEADER_INJECT_DENYLIST: &[&str] = &[
    "host",
    "content-length",
    "content-type",
    "user-agent",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "te",
    "trailer",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// 校验中间件收集到的 header 注入，产出可直接写进上游请求的 (名, 值) 对。
///
/// 跳过并 warn（不阻断请求）的三种：认证头/代理自有头（见 [`HEADER_INJECT_DENYLIST`]）、
/// 头名非法（空名、含控制字符或分隔符）、头值非法（含控制字符）。
/// 同名多条时后者覆盖前者（与调用方 `HeaderMap::insert` 的替换语义一致）。
pub(crate) fn sanitize_header_injects(
    injects: &[(String, String)],
) -> Vec<(reqwest::header::HeaderName, reqwest::header::HeaderValue)> {
    let mut out: Vec<(reqwest::header::HeaderName, reqwest::header::HeaderValue)> = Vec::new();
    for (name, value) in injects {
        if is_sensitive_auth_header(name)
            || HEADER_INJECT_DENYLIST
                .iter()
                .any(|d| name.eq_ignore_ascii_case(d))
        {
            tracing::warn!(
                header = %name,
                "middleware inject header_set: refusing to override proxy-owned header, skipped"
            );
            continue;
        }
        let Ok(hn) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) else {
            tracing::warn!(header = %name, "middleware inject header_set: invalid header name, skipped");
            continue;
        };
        let Ok(hv) = reqwest::header::HeaderValue::from_str(value) else {
            tracing::warn!(header = %name, "middleware inject header_set: invalid header value, skipped");
            continue;
        };
        out.retain(|(k, _)| *k != hn);
        out.push((hn, hv));
    }
    out
}

/// convert 路径透传入站头底座：全量入站头，剔 hop-by-hop + auth/UA/CT（由 apply 覆盖）。
/// 其余（anthropic-* / x-stainless-* / x-app / session-id / originator / version / 未知自定义头）
/// 原样透传 —— 跨协议（如 CC 入站转 OpenAI）也带，上游忽略未知头不报错，保留利于诊断。
/// `anthropic-beta` 也在原样透传之列（2026-09-21 起不再按 host 剔除，见 is_official_anthropic_host）。
pub(crate) fn passthrough_convert_headers(
    orig: &axum::http::HeaderMap,
) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (k, v) in orig {
        let name = k.as_str();
        if STRIPPED_ON_CONVERT_PASSTHROUGH
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
        {
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
// ── client-types.json simulation 配置驱动 ─────────────────────────────────────
//
// 配置加载 / 查表已下沉 aidog_db::client_types_const（2026-10-04，quota 脚本默认 UA 也要
// 读它，core↔adapter 不能倒挂）；本文件仅保留通用引擎：
//   1. protocol key 查表（serde rename → auth headers 数组，`default` 兜底）
//   2. 占位符引擎（`{api_key}` / `{uuid}` → 运行时值，非 client_type 特定）
// 数据流硬规不变：simulation 由 Rust 内部消费，前端不感知。

use crate::gateway::client_types_const::{Simulation, SimulationHeader, simulation_for};

/// 取 protocol 的 serde rename 字符串（如 `Protocol::Anthropic` → `"anthropic"`）。
/// 用于查 simulation.auth 矩阵的 protocol key。
fn protocol_serde_name(protocol: &super::models::Protocol) -> String {
    serde_json::to_value(protocol)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "default".into())
}

/// 占位符引擎：通用 `{key}` 替换，非 client_type 特定。
///   `{api_key}` → 调用方提供值（apply 路径 = 平台 api_key；日志路径 = redact_key(api_key)）
///   `{uuid}`    → uuid_sim() 运行时生成（每次调用新 uuid）
fn fill_placeholder(template: &str, api_key_value: &str) -> String {
    template
        .replace("{api_key}", api_key_value)
        .replace("{uuid}", &uuid_sim())
}

/// 查 simulation entry 的 protocol-specific headers 数组（缺失 protocol key → `default` 兜底；
/// `default` 也缺 → 空切片，apply 路径再兜底 Bearer）。
fn resolve_auth_headers<'a>(sim: &'a Simulation, protocol_key: &str) -> &'a [SimulationHeader] {
    sim.auth
        .get(protocol_key)
        .or_else(|| sim.auth.get("default"))
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

pub fn apply_client_headers(
    req_builder: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    // 未知 client_type → 回落 `default` entry（simulation_for，等价旧 apply_default_headers，
    // 保留 client_type 审计字符串）。default entry 的 user_agent 缺省 → 不注入 UA。
    let sim = simulation_for(client_type.as_str());
    let mut rb = req_builder;

    // ① UA（若 simulation.user_agent 存在；default entry 无 UA）
    if let Some(ua) = sim.and_then(|s| s.user_agent.as_deref()) {
        rb = rb.header("User-Agent", ua);
    }

    // ② auth headers（protocol key → default protocol 兜底；全缺 → Bearer only 终极兜底）
    let protocol_key = protocol_serde_name(protocol);
    let headers = sim
        .map(|s| resolve_auth_headers(s, &protocol_key))
        .unwrap_or(&[]);
    if headers.is_empty() {
        // simulation/auth 完全缺（含 default entry 被远端裁剪的极端情况）：保守 Bearer only。
        rb = rb.header("Authorization", format!("Bearer {api_key}"));
    } else {
        for h in headers {
            rb = rb.header(&h.name, fill_placeholder(&h.value, api_key));
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
    format!(
        "{:08x}-{:04}-4{:03}-{:04}-{:012x}",
        (ts as u32).wrapping_mul(0x45d9f3b),
        (ts >> 16) as u16,
        ((ts >> 32) as u16) & 0x0fff,
        ((ts >> 48) as u16) | 0x8000,
        ((ts >> 60) as u64) & 0xffffffffffff,
    )
}

/// 构建上游请求头 KV 表（用于日志记录，反映实际发送：入站透传 + apply 覆盖）。
/// 透传头从 orig 取并脱敏（auth/cookie），覆盖头（UA/auth/CT + extra headers）复用
/// 同一 simulation 配置（与 apply_client_headers 同源，日志镜像实发）。
/// api_key 经 redact_key 脱敏后再填占位符（日志安全）。
pub fn build_upstream_headers(
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
    orig: &axum::http::HeaderMap,
) -> Vec<(String, String)> {
    let mut h: Vec<(String, String)> = Vec::new();
    // ① 透传入站头（剔 stripped：hop-by-hop + auth/UA/CT）。脱敏敏感值。
    for (k, v) in orig {
        let name = k.as_str();
        if STRIPPED_ON_CONVERT_PASSTHROUGH
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
        {
            continue;
        }
        let val = v.to_str().unwrap_or("");
        let val = if is_sensitive_log_header(name) {
            redact_log_header_value(val)
        } else {
            val.to_string()
        };
        h.push((name.to_string(), val));
    }
    // ② 覆盖：Content-Type + simulation 配置驱动的 UA/auth/extra headers（占位符用 redact_key 脱敏）。
    h.push(("Content-Type".into(), "application/json".into()));
    // 未知 client_type → 回落 `default` entry（simulation_for，与 apply_client_headers 对称，保日志镜像与实发一致）。
    let sim = simulation_for(client_type.as_str());
    let redacted = redact_key(api_key);
    if let Some(ua) = sim.and_then(|s| s.user_agent.as_deref()) {
        h.push(("User-Agent".into(), ua.to_string()));
    }
    let protocol_key = protocol_serde_name(protocol);
    let headers = sim
        .map(|s| resolve_auth_headers(s, &protocol_key))
        .unwrap_or(&[]);
    if headers.is_empty() {
        // simulation/auth 完全缺（含 default entry 被远端裁剪）：保守 Bearer only（与 apply_client_headers 对称）。
        h.push(("Authorization".into(), format!("Bearer {redacted}")));
    } else {
        for hdr in headers {
            h.push((hdr.name.clone(), fill_placeholder(&hdr.value, &redacted)));
        }
    }
    h
}

/// Redact API key to a fixed mask（票 02）：首尾 8 字符跨行一致可当关联指纹，对短 key
/// 信息量更不可忽略，故整值替换、不留任何残段。定位「哪条日志是哪个平台」用 platform_name 字段。
pub fn redact_key(_key: &str) -> String {
    "[REDACTED]".into()
}

/// 敏感头**值**落库脱敏（db-log-redact-hardening 追加 2026-10-20）：保留凭证前的认证
/// scheme（`Bearer` / `Basic` / 自定义模板的 `API-KEY` 等首 token），只把 key 部分换成
/// [REDACTED]——日志读者能确认「配的就是 Bearer {key}」这类认证形态，凭证零残留。
/// 无 scheme 结构（api-key 直值 / cookie `k=v` 串）整值替换。
/// 判据：首个空白前的 token 视为 scheme，含 `=` 或 `;`（cookie/set-cookie 属性形态）不算。
pub(crate) fn redact_log_header_value(value: &str) -> String {
    let v = value.trim();
    if let Some(idx) = v.find(char::is_whitespace) {
        let (scheme, rest) = v.split_at(idx);
        if !scheme.contains('=') && !scheme.contains(';') && !rest.trim().is_empty() {
            return format!("{scheme} [REDACTED]");
        }
    }
    "[REDACTED]".to_string()
}

/// URL query 里的密钥通道参数（`api_key` / `key`，ADR 0009）值是凭证，落库/回显前
/// 替换为 [REDACTED]。只动参数名前为 `?` 或 `&` 的真参数（防 `my_api_key=` 误伤）。
pub(crate) fn redact_api_key_query(url: &str) -> String {
    super::endpoint::KEY_QUERY_PARAMS
        .iter()
        .fold(url.to_string(), |u, name| redact_query_param(&u, name))
}

fn redact_query_param(url: &str, name: &str) -> String {
    let needle = format!("{name}=");
    if !url.contains(&needle) {
        return url.to_string();
    }
    let mut out = String::with_capacity(url.len());
    let mut rest = url;
    while let Some(pos) = rest.find(&needle) {
        let is_param = pos == 0 || matches!(rest.as_bytes()[pos - 1], b'?' | b'&');
        let cut = pos + needle.len();
        if is_param {
            let tail = &rest[cut..];
            let val_end = tail.find('&').unwrap_or(tail.len());
            out.push_str(&rest[..cut]);
            out.push_str("[REDACTED]");
            rest = &tail[val_end..];
        } else {
            out.push_str(&rest[..cut]);
            rest = &rest[cut..];
        }
    }
    out.push_str(rest);
    out
}

/// 入站请求头落库形态（JSON 对象串）：敏感头 + 自定义密钥通道头（ADR 0009）值脱敏。
pub(crate) fn capture_request_headers(
    headers: &axum::http::HeaderMap,
    custom_key_headers: &[String],
) -> String {
    let mut h = serde_json::Map::new();
    for (k, v) in headers {
        if let Ok(s) = v.to_str() {
            let name = k.as_str();
            let sensitive = is_sensitive_log_header(name)
                || custom_key_headers.iter().any(|n| name.eq_ignore_ascii_case(n));
            let val = if sensitive { redact_log_header_value(s) } else { s.to_string() };
            h.insert(name.to_string(), Value::String(val));
        }
    }
    Value::Object(h).to_string()
}

/// 为 Coding Plan 端点注入平台特有字段
/// - Kimi Code Plan: 注入 prompt_cache_key（必填，用 group + model hash 作会话标识）
pub fn inject_coding_plan_fields(body: &mut Value, protocol: &super::models::Protocol) {
    match protocol {
        super::models::Protocol::Kimi => {
            // Kimi Code Plan 要求 prompt_cache_key 以提升缓存命中率
            // 用模型名 + 短随机串生成会话级 cache key
            if let Some(obj) = body.as_object_mut() {
                let model = obj
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let session_id = format!(
                    "aidog-{}-{:06x}",
                    model,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        / 300 // 5-minute window
                );
                obj.insert("prompt_cache_key".to_string(), Value::String(session_id));
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

#[cfg(test)]
#[path = "test_headers.rs"]
mod test_headers;
