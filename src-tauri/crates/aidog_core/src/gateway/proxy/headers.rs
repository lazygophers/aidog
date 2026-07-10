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
    let id = crate::logging::current_trace_id()
        .unwrap_or_else(crate::logging::new_trace_id);
    if let Ok(hv) = axum::http::HeaderValue::from_str(&id) {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static("x-aidog-trace"),
            hv,
        );
    }
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
// ── client-types.json simulation 配置驱动 ─────────────────────────────────────
//
// 真值源 = `src-tauri/defaults/client-types.json`（每 entry `simulation` 字段全自包含
// UA + auth 矩阵 + 占位符）。Rust 执行引擎**禁写任何 client_type 特定代码依赖**
// （用户逐字约束）—— 所有 client_type 差异由 JSON 配置表达，本文件仅提供通用：
//   1. 配置加载（OnceLock 启动加载，禁每请求读盘/IPC）
//   2. protocol key 查表（serde rename → auth headers 数组，`default` 兜底）
//   3. 占位符引擎（`{api_key}` / `{uuid}` → 运行时值，非 client_type 特定）
//
// 远端同步链见 `client_types_sync.rs`（7 件套），schema gate 已扩 simulation 校验。
// 数据流硬规：simulation 由 Rust 内部消费，前端不感知（仅消费 label/group/name 展示层）。

/// 编译期编入的本地真值（与 `commands/defaults.rs::CLIENT_TYPES_BUNDLED` 同源，各自 include_str!）。
const BUNDLED_CLIENT_TYPES: &str = include_str!("../../../../../defaults/client-types.json");

/// 单条 simulation header 定义（name + value 模板，value 含占位符由引擎替换）。
#[derive(Debug, Clone, serde::Deserialize)]
struct SimulationHeader {
    name: String,
    value: String,
}

/// per-protocol auth 矩阵：serde rename key（`anthropic`/`openai`/`gemini`/...）→ headers 数组。
/// 含保留 key `default` 兜底未知 protocol。
type AuthMatrix = std::collections::HashMap<String, Vec<SimulationHeader>>;

/// 单 entry 的 simulation 配置（全自包含，禁 family 继承）。
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct Simulation {
    /// 缺省 = 不注入 UA（如 `default` entry）。
    #[serde(default)]
    user_agent: Option<String>,
    #[serde(default)]
    auth: AuthMatrix,
}

/// client-types.json 顶层文档（仅消费 simulation 相关字段，label/desc 忽略）。
#[derive(Debug, Clone, serde::Deserialize)]
struct ClientTypesDoc {
    client_types: Vec<ClientTypeEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ClientTypeEntry {
    value: String,
    #[serde(default)]
    simulation: Simulation,
}

/// 启动加载缓存：`client_type.as_str()` → Simulation（缺省 entry 不在 map = 等价 default 行为）。
static SIMULATION_CACHE: std::sync::OnceLock<std::collections::HashMap<String, Simulation>> =
    std::sync::OnceLock::new();

/// 读 client-types.json body（app data 优先 → bundled fallback；同 `get_client_types_json` reader）。
/// 启动一次（OnceLock get_or_init），禁每请求读盘。
fn load_client_types_body() -> String {
    let dir = match crate::shared::aidog_data_dir() {
        Ok(d) => d,
        Err(_) => return BUNDLED_CLIENT_TYPES.to_string(),
    };
    let path = dir.join("client-types.json");
    if !path.exists() {
        return BUNDLED_CLIENT_TYPES.to_string();
    }
    match std::fs::read_to_string(&path) {
        Ok(content)
            if !content.trim().is_empty()
                && serde_json::from_str::<serde_json::Value>(&content).is_ok() =>
        {
            content
        }
        _ => {
            tracing::warn!(
                path = %path.display(),
                "client-types.json empty/corrupt in sim load, fallback to bundled"
            );
            BUNDLED_CLIENT_TYPES.to_string()
        }
    }
}

/// 首次访问解析 simulation map；解析失败兜底空 map（apply 路径再兜底 Bearer）。
fn simulation_map() -> &'static std::collections::HashMap<String, Simulation> {
    SIMULATION_CACHE.get_or_init(|| {
        let body = load_client_types_body();
        match serde_json::from_str::<ClientTypesDoc>(&body) {
            Ok(doc) => doc
                .client_types
                .into_iter()
                .map(|e| (e.value, e.simulation))
                .collect(),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "client-types.json simulation parse failed (should never happen), using empty map"
                );
                std::collections::HashMap::new()
            }
        }
    })
}

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
fn resolve_auth_headers<'a>(
    sim: &'a Simulation,
    protocol_key: &str,
) -> &'a [SimulationHeader] {
    sim.auth
        .get(protocol_key)
        .or_else(|| sim.auth.get("default"))
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

/// 未知 client_type（JSON 无 entry）→ 回落 `default` entry 的 simulation（PRD R2：
/// 「等价 default」语义）—— default entry 的 auth 矩阵覆盖 anthropic/openai/gemini/default，
/// 与重构前 `match _ => apply_default_headers` 行为对齐。`default` 也缺 → Bearer-only 终极兜底。
fn resolve_simulation<'a>(
    map: &'a std::collections::HashMap<String, Simulation>,
    client_type: &str,
) -> Option<&'a Simulation> {
    map.get(client_type)
        .or_else(|| map.get("default"))
}

pub fn apply_client_headers(
    req_builder: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    let map = simulation_map();
    // 未知 client_type → 回落 `default` entry（等价旧 apply_default_headers，保留 client_type 审计字符串）。
    // default entry 的 user_agent 缺省 → 不注入 UA（与旧 default 行为一致）。
    let sim = resolve_simulation(map, client_type.as_str());
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
    format!("{:08x}-{:04}-4{:03}-{:04}-{:012x}",
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
/// upstream_url 用于 anthropic-beta host 判定（须与 passthrough_convert_headers 同参，日志与实发一致）。
/// api_key 经 redact_key 脱敏后再填占位符（日志安全）。
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
    // ② 覆盖：Content-Type + simulation 配置驱动的 UA/auth/extra headers（占位符用 redact_key 脱敏）。
    h.push(("Content-Type".into(), "application/json".into()));
    let map = simulation_map();
    // 未知 client_type → 回落 `default` entry（与 apply_client_headers 对称，保日志镜像与实发一致）。
    let sim = resolve_simulation(map, client_type.as_str());
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
