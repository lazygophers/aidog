//! 上游非 2xx 错误体分类（issue 68 迁入：错误体处理接口的默认实现）。
//! 原 `aidog_core::gateway::proxy::retry.rs` 同名项原样搬家（行为不变）。

use serde_json::Value;
/// 上游是否明确因审核机制拒绝请求。只识别结构化 `error.type`，避免误把普通 400 当审核拒绝。
pub fn is_censorship_blocked(body: &str) -> bool {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error")?
                .get("type")?
                .as_str()
                .map(|s| s == "censorship_blocked")
        })
        .unwrap_or(false)
}

/// 上游非 2xx 的错误类别（同平台换模型 vs 换平台，spec D5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamErrorClass {
    /// 模型类：换同平台下一个模型。
    Model,
    /// 上游服务类：不换模型，直接换平台。
    Service,
    /// 无法判定：调用方按 `Model` 处理（宁可多试一次）。
    Unknown,
}

impl UpstreamErrorClass {
    /// `ProxyAttempt.error_class` 的落库字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Service => "service",
            Self::Unknown => "unknown",
        }
    }
}

/// 扫描 body 的字节上限：错误体特征都在头部，超长 body 不全量 lowercase。
const CLASSIFY_SCAN_MAX: usize = 8 * 1024;

/// 上游非 2xx 分类（用户 2026-11-02 拍板）：429/402/401/403 直接 `Service`（不读 body）；
/// 结构化 `censorship_blocked` 同为 `Service`；其余状态码读 error.message/code/type
/// （非 JSON 读原文），命中模型类特征 → `Model`（含 5xx 例外：new-api 503「无可用渠道」
/// 实为模型不可用），未命中：5xx → `Service`，其余 `Unknown`（调用方按模型类处理）。
/// 出处：Anthropic errors 页（402 billing_error / 403 permission_error / 404 not_found_error /
/// 529 overloaded_error）、one-api `middleware/distributor.go`「无可用渠道」、
/// new-api `i18n/locales/en.yaml`「No available channel for model」、
/// litellm `exception_mapping_utils.py`（model_not_found）。
pub fn classify_upstream_error(status: u16, body: &str) -> UpstreamErrorClass {
    if matches!(status, 401 | 402 | 403 | 429) || is_censorship_blocked(body) {
        return UpstreamErrorClass::Service;
    }
    let mut end = body.len().min(CLASSIFY_SCAN_MAX);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    let window = &body[..end];
    // JSON 的 message/code/type 比原文干净；解析失败（含被截断）回落原文窗口。
    let text = serde_json::from_str::<Value>(window)
        .ok()
        .map(|v| {
            let e = v.get("error").unwrap_or(&v);
            if let Some(s) = e.as_str() {
                return s.to_string();
            }
            ["message", "code", "type"]
                .iter()
                .filter_map(|k| e.get(k).and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| window.to_string())
        .to_lowercase();
    const MODEL_MARKERS: [&str; 8] = [
        "not_found_error",
        "model_not_found",
        "model not found",
        "not found the model",
        "no available channel",
        "无可用渠道",
        "invalid model",
        "unknown model",
    ];
    let has_model = text.contains("model") || text.contains("模型");
    let hit = MODEL_MARKERS.iter().any(|m| text.contains(m))
        || (has_model
            && ["not supported", "does not exist", "不支持", "不存在"]
                .iter()
                .any(|m| text.contains(m)));
    if hit {
        UpstreamErrorClass::Model
    } else if status >= 500 {
        UpstreamErrorClass::Service
    } else {
        UpstreamErrorClass::Unknown
    }
}

