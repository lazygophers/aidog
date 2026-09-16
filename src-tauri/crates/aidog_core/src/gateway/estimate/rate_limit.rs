//! 上游响应头里的**速率限制**余量（每分钟能发多少），与 `EstTier` 承载的**套餐额度**
//! （周期内还剩多少）是两个维度，故独立成型不复用后者。
//!
//! 三家厂商的头名互不相同，但语义一一对应，取第一个匹配到的家族（一次响应不会同时带两家）：
//!
//! | 厂商 | 请求数余量 | token 余量 | 重置 | 官方文档 |
//! |---|---|---|---|---|
//! | Anthropic | `anthropic-ratelimit-requests-remaining` | `-tokens-remaining` | `-requests-reset`（RFC3339） | <https://docs.claude.com/en/api/rate-limits> |
//! | OpenAI | `x-ratelimit-remaining-requests` | `-remaining-tokens` | `x-ratelimit-reset-requests`（`1s` / `6m0s` 时长串） | <https://platform.openai.com/docs/guides/rate-limits> |
//! | OpenRouter | `x-ratelimit-remaining` | 无 | `x-ratelimit-reset`（unix 毫秒） | <https://openrouter.ai/docs/api-reference/limits> |
//!
//! 一个头都不认识 → `None`，调用方不写库（不把「没有」记成「0」）。

use serde::{Deserialize, Serialize};

/// 持久化于 `platform.rate_limit` 的速率限制快照。字段全为 Option：
/// 厂商只给了请求数没给 token 时，缺的那边保持 None 而不是 0。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RateLimit {
    /// 厂商标识：`anthropic` / `openai` / `openrouter`，用于前端选展示口径
    pub vendor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_remaining: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_remaining: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_limit: Option<i64>,
    /// 窗口重置时刻（unix 毫秒）。三家给的格式各不相同，统一归一化到毫秒。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
    /// 本条快照的观测时刻（unix 毫秒），用于前端判断是否已过期
    pub observed_at: i64,
}

impl RateLimit {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| String::new())
    }
    pub fn from_json(s: &str) -> Option<Self> {
        if s.trim().is_empty() {
            return None;
        }
        serde_json::from_str(s).ok()
    }
}

/// OpenAI 的重置字段是时长串（`1s` / `6m0s` / `1h30m0s` / `88ms`），不是时刻。
/// 解析成毫秒；识别不了的返回 None（不猜 0，0 会被当成「立刻重置」）。
fn parse_go_duration_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut total_ms = 0f64;
    let mut num = String::new();
    let mut chars = s.chars().peekable();
    let mut matched_any = false;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            continue;
        }
        // 单位：ms 要先于 m 判定，否则 "88ms" 会被当成 88 分钟
        let unit_ms = match c {
            'm' if chars.peek() == Some(&'s') => {
                chars.next();
                1.0
            }
            'm' => 60_000.0,
            's' => 1_000.0,
            'h' => 3_600_000.0,
            _ => return None,
        };
        let v: f64 = num.parse().ok()?;
        num.clear();
        total_ms += v * unit_ms;
        matched_any = true;
    }
    // 尾部还有没跟单位的数字 = 格式不认识
    if !num.is_empty() || !matched_any {
        return None;
    }
    Some(total_ms.round() as i64)
}

/// RFC3339 时刻串 → unix 毫秒
fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|dt| dt.timestamp_millis())
}

/// 从上游响应头提取速率限制快照。`now_ms` 由调用方传入（纯函数，便于单测）。
pub fn parse_rate_limit(headers: &axum::http::HeaderMap, now_ms: i64) -> Option<RateLimit> {
    let get = |name: &str| -> Option<&str> { headers.get(name)?.to_str().ok() };
    let num = |name: &str| -> Option<i64> { get(name)?.trim().parse::<i64>().ok() };

    // Anthropic：字段最全，先认它
    if headers.contains_key("anthropic-ratelimit-requests-remaining")
        || headers.contains_key("anthropic-ratelimit-tokens-remaining")
    {
        return Some(RateLimit {
            vendor: "anthropic".to_string(),
            requests_remaining: num("anthropic-ratelimit-requests-remaining"),
            requests_limit: num("anthropic-ratelimit-requests-limit"),
            tokens_remaining: num("anthropic-ratelimit-tokens-remaining"),
            tokens_limit: num("anthropic-ratelimit-tokens-limit"),
            resets_at: get("anthropic-ratelimit-requests-reset")
                .or_else(|| get("anthropic-ratelimit-tokens-reset"))
                .and_then(parse_rfc3339_ms),
            observed_at: now_ms,
        });
    }

    // OpenAI：remaining-requests / remaining-tokens 带方位词，与 OpenRouter 的裸 remaining 区分得开
    if headers.contains_key("x-ratelimit-remaining-requests")
        || headers.contains_key("x-ratelimit-remaining-tokens")
    {
        return Some(RateLimit {
            vendor: "openai".to_string(),
            requests_remaining: num("x-ratelimit-remaining-requests"),
            requests_limit: num("x-ratelimit-limit-requests"),
            tokens_remaining: num("x-ratelimit-remaining-tokens"),
            tokens_limit: num("x-ratelimit-limit-tokens"),
            resets_at: get("x-ratelimit-reset-requests")
                .or_else(|| get("x-ratelimit-reset-tokens"))
                .and_then(parse_go_duration_ms)
                .map(|d| now_ms + d),
            observed_at: now_ms,
        });
    }

    // OpenRouter：只有裸 remaining，且 reset 是 unix 毫秒时刻
    if headers.contains_key("x-ratelimit-remaining") {
        return Some(RateLimit {
            vendor: "openrouter".to_string(),
            requests_remaining: num("x-ratelimit-remaining"),
            requests_limit: num("x-ratelimit-limit"),
            tokens_remaining: None,
            tokens_limit: None,
            resets_at: num("x-ratelimit-reset"),
            observed_at: now_ms,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn hm(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn anthropic_family_parsed_with_rfc3339_reset() {
        let h = hm(&[
            ("anthropic-ratelimit-requests-remaining", "42"),
            ("anthropic-ratelimit-requests-limit", "50"),
            ("anthropic-ratelimit-tokens-remaining", "18000"),
            ("anthropic-ratelimit-tokens-limit", "20000"),
            ("anthropic-ratelimit-requests-reset", "2026-09-16T10:00:00Z"),
        ]);
        let r = parse_rate_limit(&h, 1_700_000_000_000).expect("anthropic 头应被识别");
        assert_eq!(r.vendor, "anthropic");
        assert_eq!(r.requests_remaining, Some(42));
        assert_eq!(r.requests_limit, Some(50));
        assert_eq!(r.tokens_remaining, Some(18000));
        assert_eq!(r.resets_at, Some(1_789_552_800_000), "2026-09-16T10:00:00Z");
    }

    #[test]
    fn openai_family_reset_is_duration_added_to_now() {
        let now = 1_700_000_000_000;
        let h = hm(&[
            ("x-ratelimit-remaining-requests", "99"),
            ("x-ratelimit-limit-requests", "100"),
            ("x-ratelimit-remaining-tokens", "9000"),
            ("x-ratelimit-reset-requests", "6m0s"),
        ]);
        let r = parse_rate_limit(&h, now).expect("openai 头应被识别");
        assert_eq!(r.vendor, "openai");
        assert_eq!(r.requests_remaining, Some(99));
        assert_eq!(r.resets_at, Some(now + 360_000));
    }

    #[test]
    fn openrouter_family_reset_is_unix_ms() {
        let h = hm(&[
            ("x-ratelimit-remaining", "7"),
            ("x-ratelimit-limit", "10"),
            ("x-ratelimit-reset", "1789646400000"),
        ]);
        let r = parse_rate_limit(&h, 1_700_000_000_000).expect("openrouter 头应被识别");
        assert_eq!(r.vendor, "openrouter");
        assert_eq!(r.requests_remaining, Some(7));
        assert_eq!(r.tokens_remaining, None, "OpenRouter 不给 token 余量");
        assert_eq!(r.resets_at, Some(1_789_646_400_000));
    }

    #[test]
    fn unknown_headers_yield_none() {
        let h = hm(&[("content-type", "application/json")]);
        assert!(parse_rate_limit(&h, 0).is_none(), "不认识的头不得编造快照");
    }

    #[test]
    fn missing_fields_stay_none_not_zero() {
        let h = hm(&[("anthropic-ratelimit-requests-remaining", "5")]);
        let r = parse_rate_limit(&h, 0).unwrap();
        assert_eq!(r.tokens_remaining, None, "缺失字段必须是 None 而不是 0");
        assert_eq!(r.resets_at, None);
    }

    #[test]
    fn go_duration_units() {
        assert_eq!(parse_go_duration_ms("1s"), Some(1_000));
        assert_eq!(parse_go_duration_ms("6m0s"), Some(360_000));
        assert_eq!(parse_go_duration_ms("1h30m0s"), Some(5_400_000));
        assert_eq!(parse_go_duration_ms("88ms"), Some(88), "ms 不得被当成分钟");
        assert_eq!(parse_go_duration_ms("1.5s"), Some(1_500));
        assert_eq!(parse_go_duration_ms("abc"), None);
        assert_eq!(parse_go_duration_ms("12"), None, "无单位的裸数字不认");
        assert_eq!(parse_go_duration_ms(""), None);
    }

    #[test]
    fn json_roundtrip_and_empty_is_none() {
        let r = RateLimit {
            vendor: "anthropic".into(),
            requests_remaining: Some(1),
            observed_at: 123,
            ..Default::default()
        };
        assert_eq!(RateLimit::from_json(&r.to_json()), Some(r));
        assert_eq!(RateLimit::from_json(""), None);
        assert_eq!(RateLimit::from_json("   "), None);
    }
}
