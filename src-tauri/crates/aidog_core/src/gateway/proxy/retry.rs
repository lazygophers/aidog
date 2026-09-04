use super::*;

/// 上游响应头透传黑名单（必剔 + RFC 7230 §6.1 hop-by-hop）。
/// 全小写常量；HeaderName 本身即小写存储，用 as_str() 比对即可。
pub(crate) const RESP_HEADER_BLACKLIST: &[&str] = &[
    // §4.1 必剔（解压/长度/传输编码失真）
    "content-encoding",
    "content-length",
    "transfer-encoding",
    // §4.2 应剔（hop-by-hop, RFC 7230 §6.1）
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "upgrade",
];

/// 流式（SSE）额外剔除集：这三个头归 SSE 自管，禁用上游值覆盖 SSE 语义。
pub(crate) const SSE_EXTRA_BLACKLIST: &[&str] = &["content-type", "cache-control", "connection"];

/// 上游响应头 → 透传给客户端的头（黑名单剔除 + 非法 value 跳过 + 多值逐个保留）。
///
/// - `is_stream=false`：仅按 RESP_HEADER_BLACKLIST 剔除（非流式 2xx 路径）。
/// - `is_stream=true`：在 RESP_HEADER_BLACKLIST 基础上额外剔除 SSE_EXTRA_BLACKLIST，
///   叠加于调用方设置的 SSE 三自管头之上。
///
/// 返回 `Vec<(HeaderName, HeaderValue)>`，调用方用 `extend` 注入 axum Response。
/// 多值头（如多个 set-cookie）逐项保留（Vec append 语义，不覆盖）。
/// 无法转为 axum header 类型的非法名/值跳过（不 panic）。
pub(crate) fn filter_upstream_resp_headers(
    src: &reqwest::header::HeaderMap,
    is_stream: bool,
) -> Vec<(axum::http::HeaderName, axum::http::HeaderValue)> {
    let mut out = Vec::with_capacity(src.len());
    for (k, v) in src.iter() {
        let name = k.as_str(); // HeaderName 已小写
        if RESP_HEADER_BLACKLIST
            .iter()
            .any(|b| name.eq_ignore_ascii_case(b))
        {
            continue;
        }
        if is_stream
            && SSE_EXTRA_BLACKLIST
                .iter()
                .any(|b| name.eq_ignore_ascii_case(b))
        {
            continue;
        }
        // reqwest header 类型 → axum(http) header 类型；非法则跳过不 panic
        if let (Ok(hn), Ok(hv)) = (
            axum::http::HeaderName::from_bytes(name.as_bytes()),
            axum::http::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out.push((hn, hv));
        }
    }
    out
}

/// 把实发头集合（HeaderName, HeaderValue）序列化为日志 JSON 字符串，
/// 与 upstream_response_headers 同格式 `{name: value}`；多值同名头保留首值（与既有格式约定一致）。
pub(crate) fn resp_headers_to_log_json(
    headers: &[(axum::http::HeaderName, axum::http::HeaderValue)],
) -> String {
    let mut h = serde_json::Map::new();
    for (k, v) in headers {
        if let Ok(s) = v.to_str() {
            h.entry(k.as_str().to_string())
                .or_insert_with(|| Value::String(s.to_string()));
        }
    }
    Value::Object(h).to_string()
}

/// 区分 429：配额耗尽（true）vs 限流 transient（false）。仅用于熔断分类（C3）：
/// 配额耗尽不计熔断（record_ignored），限流计熔断（record_failure）。
/// 429 不再触发 auto_disable（无论配额还是限流），统一走 failover 换下个候选。
/// 只看 message 文本，禁按 error.type（MiniMax 配额耗尽 type 也是 rate_limit_error）。
/// 无 marker 命中默认 false（保守按限流，避免误判配额）。
pub(crate) fn classify_429(message: &str) -> bool {
    const QUOTA_MARKERS: [&str; 8] = [
        "quota exhausted",
        "用量上限",
        "使用上限",  // GLM 1308「已达到 5 小时的使用上限」
        "使用限制",  // 同族措辞（周/月限额）
        "token plan",
        "insufficient",
        "余额",
        "积分",
    ];
    let lower = message.to_lowercase();
    QUOTA_MARKERS.iter().any(|m| lower.contains(m))
}

/// 配额冷却上限（24h）：超出一律丢弃，防上游给出离谱值或时区解析偏差把平台长期锁死。
const MAX_QUOTA_COOLDOWN_MS: i64 = 24 * 60 * 60 * 1000;

/// 从 429 响应解析「配额恢复时间」→ 绝对毫秒时间戳，供 `set_platform_quota_cooldown` 使用。
///
/// 两个来源，先 header 后 body：
/// 1. `Retry-After`：HTTP 标准（RFC 9110），整数秒。
/// 2. body 里第一个 `YYYY-MM-DD HH:MM:SS`（GLM 1308「您的限额将在 2026-09-04 21:37:35 重置」），
///    按**本机时区**解释 —— 上游返回的是请求方看到的钟面时间。
///
/// 结果必须落在 `(now, now + 24h]`，否则返回 None（调用方退回原有 failover 语义，不冷却）。
pub(crate) fn parse_quota_reset_at(
    retry_after: Option<&str>,
    body: &str,
    now_ms: i64,
) -> Option<i64> {
    let from_header = retry_after
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|secs| now_ms + secs * 1000);
    from_header
        .or_else(|| parse_body_datetime_ms(body))
        .filter(|&t| t > now_ms && t - now_ms <= MAX_QUOTA_COOLDOWN_MS)
}

/// 抓 body 里第一个 `YYYY-MM-DD HH:MM:SS`（允许 `T` 分隔），按本机时区转毫秒时间戳。
fn parse_body_datetime_ms(body: &str) -> Option<i64> {
    use chrono::TimeZone;
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2}:\d{2})").expect("static regex")
    });
    let caps = re.captures(body)?;
    let naive = chrono::NaiveDateTime::parse_from_str(
        &format!("{} {}", &caps[1], &caps[2]),
        "%Y-%m-%d %H:%M:%S",
    )
    .ok()?;
    chrono::Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.timestamp_millis())
}

/// 截断 attempt error 字段（上游错误体可能很大，attempts JSON 列只存摘要）
pub(crate) fn truncate_attempt_error(body: &str) -> String {
    const MAX: usize = 500;
    if body.len() <= MAX {
        body.to_string()
    } else {
        let mut s: String = body.chars().take(MAX).collect();
        s.push('…');
        s
    }
}

/// 截断 200-but-empty 取证文本（peek_text / resp_str）落 `proxy_log.response_body`。
///
/// 上游「200 + 空/无效流（或 body）」时，把上游真实首块原文截断落库用于后续取证（GLM 间歇空流根因）。
/// 截断格式：≤4KB 原样；超出尾部追加 `…[truncated N bytes]`（N=被截字节数，按 char 边界不切多字节字符）。
/// 空文本返回空串 → 调用点保留占位兜底文案。
pub(crate) fn truncate_peek_text(text: &str) -> String {
    const MAX: usize = 4 * 1024;
    if text.is_empty() {
        return String::new();
    }
    if text.len() <= MAX {
        return text.to_string();
    }
    // 按字符截断避免切断多字节 UTF-8（SSE 中文事件），累计字节 ≤ MAX。
    let mut end = 0usize;
    for (i, _) in text.char_indices().take_while(|(i, _)| *i <= MAX) {
        end = i;
    }
    let head = text[..end].to_string();
    format!("{}…[truncated {} bytes]", head, text.len() - end)
}

/// 决策 A：非 2xx 上游状态码是否应 failover 重试下一候选平台。
///
/// - **不重试（硬错，换平台也没用）**：400 / 422 —— 请求体本身非法（协议转换产物上游拒收），
///   遍历其他平台同样会被拒，直接返客户端避免无谓遍历。
/// - **重试**：401 / 403（鉴权，配合 auto_disabled）、404 / 405（死端点，配合 strike）、
///   429（限流/配额，换平台可能成功）、所有 5xx（上游故障）、其余未知非 2xx（保守重试）。
///
/// 连接错误 / 超时不经此函数（在 send() Err 分支已按可重试处理）。
/// 注意：中间件 error_rule 的 non-retryable 分类是显式覆盖机制，独立于本函数（见调用点）。
pub(crate) fn is_status_retryable(code: u16) -> bool {
    !matches!(code, 400 | 422)
}

/// 同平台瞬时重试次数上限（transport 错误专用，见 `is_transport_retryable`）。
/// 换平台 failover 只在多候选组内有效；单平台组遇上游瞬断直接 502。此常量让同一平台原地
/// 重试若干次，覆盖「上游中途掐连接」这类几百毫秒后即可恢复的抖动。
pub(crate) const TRANSPORT_RETRY_MAX: u32 = 2;

/// 第 n 次同平台重试前的退避时长（指数：200ms / 400ms）。
pub(crate) fn transport_retry_backoff(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_millis(200u64 << attempt.min(3))
}

/// 「快速失败」阈值：本次尝试耗时低于此值才认为是瞬时抖动，值得原地重试。
/// 2026-08-21 用户决策从 5s 提到 15s：实测 cometapi 断连多在 10-12s 落地，
/// 5s 门槛把这批全部挡在重试外；接受最坏情况单请求耗时翻倍换取断连自动恢复。
pub(crate) const TRANSPORT_FAST_FAIL: std::time::Duration = std::time::Duration::from_secs(15);

/// transport 层错误是否值得同平台原地重试。`elapsed` = 本次尝试已耗时。
///
/// - **连接建立失败**（`is_connect`）：恒重试。耗时被 connect_timeout 夹住（毫秒到数秒级），
///   重试代价小，且是最典型的瞬时抖动。
/// - **请求级读超时**（`is_timeout` 且非 connect）：恒不重试。已经等满 timeout_secs，
///   再来一轮只是把用户等待翻倍。
/// - **其余 transport 错误**（连接被上游中途掐断 / body 中断）：仅当本次尝试
///   `< TRANSPORT_FAST_FAIL` 才重试。历史实证（5s 时代）：cometapi 的
///   `SendRequest: connection closed before message completed` 也有挂 15~50 秒才掐的形态，
///   超过 15s 的慢失败重试收益低，维持不重试。
pub(crate) fn is_transport_retryable(e: &reqwest::Error, elapsed: std::time::Duration) -> bool {
    if e.is_connect() {
        return true;
    }
    if e.is_timeout() {
        return false;
    }
    elapsed < TRANSPORT_FAST_FAIL
}

/// 把错误的 `source()` 链展开成 `顶层: 次层: 末层`。
///
/// reqwest 的 Display 只打顶层，恒为 `error sending request for url (...)`，真实原因
/// （`connection closed before message completed` / `operation timed out` / TLS 握手失败）
/// 全在 source 链里。不展开则 proxy_log 里成千上万条 502 是同一句无诊断价值的文本。
pub(crate) fn err_chain(e: &dyn std::error::Error) -> String {
    let mut out = e.to_string();
    let mut src = e.source();
    while let Some(s) = src {
        out.push_str(": ");
        out.push_str(&s.to_string());
        src = s.source();
    }
    out
}

/// 决策 B：流式 200 首块缓冲判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamPeek {
    /// 已确认上游在产出有效内容（anthropic 真实事件 / openai choices delta / 通用 data 事件）→ 提交转发。
    Meaningful,
    /// 200 但空 / 无效（立即 [DONE] 无内容 / 立即 error 事件 / 流秒断无内容 / 空 body）→ 当作失败重试。
    EmptyOrError,
    /// 累积的字节尚不足以判定（仅注释/keepalive/不完整 SSE 帧）→ 继续缓冲下一块。
    NeedMore,
}

/// 决策 B：扫描已缓冲的上游 SSE 原文，判定首个「有效内容」是否到达。
///
/// 在**上游原始 wire 格式**上判定（转换前），覆盖 anthropic / openai / 同协议透传三类：
/// - **EmptyOrError**（重试）：首个有效事件是 `error`（`event: error` 或 JSON `{"type":"error"}` / 顶层 `error` 字段）；
///   或在任何内容事件前先出现 `[DONE]`。
/// - **Meaningful**（提交）：出现真实内容事件 —— anthropic `message_start`/`content_block_*`/`message_delta`；
///   openai `choices`（含 delta/role/content/tool_calls/finish_reason）；或任何非 error/非 [DONE] 的 `data:` JSON 事件。
/// - **NeedMore**：目前只见 SSE 注释行（`:` 开头 keepalive）/ 空行 / `event:` 名行但对应 `data:` 帧尚未到齐。
///
/// `stream_ended=true`（上游流已结束）时强制收敛：仍无内容事件 → EmptyOrError（流秒断无内容 / 空 body）。
pub(crate) fn classify_stream_first(text: &str, stream_ended: bool) -> StreamPeek {
    let mut saw_any_data = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(':') {
            // SSE 注释 / keepalive / 空分隔行 → 不构成判定依据
            continue;
        }
        // `event: error` 行：下一帧 data 即错误；提前判 error（无需等 data）
        if let Some(ev) = line.strip_prefix("event:") {
            if ev.trim().eq_ignore_ascii_case("error") {
                return StreamPeek::EmptyOrError;
            }
            // 其他 event 名行（message_start/content_block_delta...）单独不足以判定，等 data 帧
            continue;
        }
        let Some(data) = line.strip_prefix("data:") else {
            // 非 SSE 字段行（不完整帧的中段）→ 等更多
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            // 任何内容前先 [DONE] → 空响应；内容后的 [DONE] 不会进入本函数（已 Meaningful 提前返回）
            return StreamPeek::EmptyOrError;
        }
        saw_any_data = true;
        let Ok(json) = serde_json::from_str::<Value>(data) else {
            // data 帧 JSON 尚不完整（跨 chunk 截断）→ 等更多
            continue;
        };
        // 顶层 error 结构（openai `{"error":{...}}` / anthropic `{"type":"error",...}`）→ 失败
        let ty = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty == "error" || json.get("error").is_some() {
            return StreamPeek::EmptyOrError;
        }
        // 到此即确认上游产出了一个真实（非 error / 非 [DONE]）内容事件 → 提交
        return StreamPeek::Meaningful;
    }
    if stream_ended {
        // 流已结束仍无任何有效内容事件 → 空响应（哪怕收到过无法解析的残帧也判空）
        let _ = saw_any_data;
        StreamPeek::EmptyOrError
    } else {
        StreamPeek::NeedMore
    }
}

/// 决策 B：非流式 200 响应体是否「非空有效」。返回 false → 当作失败重试下一平台。
///
/// 空 body / 不含有效 choices/content / 是 error 结构 → false。
/// 在**上游原始 JSON**上判定（转换前 / 透传同理）。
pub(crate) fn is_nonstream_body_valid(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
        // 非 JSON 200 body：保守视为有效（避免把上游非标准但实质有内容的响应误判为空）
        return true;
    };
    // error 结构（顶层 error 字段 / type==error）→ 无效
    if json.get("error").is_some() || json.get("type").and_then(|v| v.as_str()) == Some("error") {
        return false;
    }
    // openai 风格：choices 非空且含实质内容（message/content/text/delta/tool_calls）
    if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
        return choices.iter().any(|c| {
            c.get("message").is_some() || c.get("text").is_some() || c.get("delta").is_some()
        });
    }
    // anthropic 风格：content 数组非空
    if let Some(content) = json.get("content").and_then(|v| v.as_array()) {
        return !content.is_empty();
    }
    // 其他形态（如 openai responses `output` 等）：非 error 且 JSON 有内容 → 视为有效
    json.as_object().map(|o| !o.is_empty()).unwrap_or(false)
}

#[cfg(test)]
#[path = "test_retry.rs"]
mod test_retry;

pub(crate) use aidog_db::extract_error_message;
