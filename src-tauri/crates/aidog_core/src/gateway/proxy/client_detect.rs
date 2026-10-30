//! 入站客户端识别：按 User-Agent 查 `crate::client_vocab` 两级词表，写 `proxy_log.client` 与
//! `stats_agg_hourly.client`。只存词表码、不存原始 UA（否则聚合表维度基数失控）。
//!
//! 两种语义同一接口：[`identify_client`] 返回 `None` = 认不出（白名单判定视为未命中）；
//! [`detect_client`] 把 `None` 映射为统计码 `other`。存量空串（未知）仅旧行 / 带外日志，本模块不产出。

use axum::http::{HeaderMap, header::USER_AGENT};

use crate::client_vocab::{ClientEntry, identify};

/// 无 UA 头 / 非 UTF-8 / 不在词表 → `None`（认不出）；命中变体返回变体，只命中大类返回大类。
pub(crate) fn identify_client(headers: &HeaderMap) -> Option<&'static ClientEntry> {
    identify(headers.get(USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or(""))
}

/// 统计 / 日志用码：词表码，认不出 → `other`。
pub(crate) fn detect_client(headers: &HeaderMap) -> &'static str {
    identify_client(headers).map_or("other", |e| e.code)
}

/// 原始 UA（截断到 `USER_AGENT_MAX_CHARS` 字符）；无头 / 非 UTF-8 → 空串。
pub(crate) fn raw_user_agent(headers: &HeaderMap) -> String {
    headers
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(aidog_db::models::USER_AGENT_MAX_CHARS).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_ua(s: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(USER_AGENT, s.parse().unwrap());
        h
    }

    #[test]
    fn raw_user_agent_kept_and_truncated() {
        let mut h = HeaderMap::new();
        assert_eq!(raw_user_agent(&h), "");
        h.insert(USER_AGENT, "claude-cli/2.0.1 (external, cli)".parse().unwrap());
        assert_eq!(raw_user_agent(&h), "claude-cli/2.0.1 (external, cli)");
        h.insert(USER_AGENT, "x".repeat(1000).parse().unwrap());
        assert_eq!(raw_user_agent(&h).chars().count(), aidog_db::models::USER_AGENT_MAX_CHARS);
    }

    #[test]
    fn stat_code_is_variant_category_or_other() {
        assert_eq!(detect_client(&with_ua("claude-cli/2.0 (external, claude-vscode)")), "claude_code_vscode");
        assert_eq!(detect_client(&with_ua("claude-cli/2.0 (external, cli)")), "claude_code");
        assert_eq!(detect_client(&with_ua("curl/8.0")), "curl");
        assert_eq!(detect_client(&with_ua("Mozilla/5.0 api (x)")), "other");
        assert!(identify_client(&with_ua("Mozilla/5.0 api (x)")).is_none());
        // 变体 → 所属大类（白名单勾大类用）
        assert_eq!(identify_client(&with_ua("codex-tui/1.0")).unwrap().category(), "codex");
    }

    #[test]
    fn missing_or_non_utf8_header_is_unrecognized() {
        assert!(identify_client(&HeaderMap::new()).is_none());
        assert_eq!(detect_client(&HeaderMap::new()), "other");
        let mut h = HeaderMap::new();
        h.insert(USER_AGENT, axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap());
        assert!(identify_client(&h).is_none());
        assert_eq!(detect_client(&h), "other");
    }
}
