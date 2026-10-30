//! 入站客户端识别：按 User-Agent 归类为固定代码集，写 `proxy_log.client` 与 `stats_agg_hourly.client`。
//! 只存代码、不存原始 UA（否则聚合表维度基数失控）。规则表是唯一归类真值源，前端只认代码。
//!
//! 代码集：`claude_code | codex | pi | cursor | windsurf | gemini_cli | other`；空串（未知）仅存量行 /
//! 带外日志，由调用方保持默认值，本函数不产出。

use axum::http::{HeaderMap, header::USER_AGENT};

#[derive(Clone, Copy)]
enum Rule {
    /// UA（小写）包含该子串。
    Contains(&'static str),
    /// UA（小写）以该前缀开头（短 token 用，防 `api` 之类误命中）。
    Prefix(&'static str),
}

/// 有序规则表，首个命中即返回。UA 取证：
/// - claude_code / codex：与 `endpoint.rs::infer_passthrough_protocol_from_ua` 同子串（既有先例）。
/// - pi：`gateway/pi.rs` 的 `pi_user_agent()` 写进 provider headers 的 `pi (<platform>; <arch>)`，
///   pi 自带形态 `pi/<version> (...)`（pi 源 `pi-user-agent.ts`，见 pi.rs 注释），故按前缀 `pi ` / `pi/` 匹配。
/// - cursor / windsurf / gemini_cli：推测: 子串取自产品名，未实抓入站 UA；抓到真实 UA 后在此校正。
const RULES: &[(Rule, &str)] = &[
    (Rule::Contains("claude-cli"), "claude_code"),
    (Rule::Contains("codex"), "codex"),
    (Rule::Prefix("pi "), "pi"),
    (Rule::Prefix("pi/"), "pi"),
    (Rule::Contains("cursor"), "cursor"),
    (Rule::Contains("windsurf"), "windsurf"),
    (Rule::Contains("geminicli"), "gemini_cli"),
    (Rule::Contains("gemini-cli"), "gemini_cli"),
];

fn classify(ua: &str) -> &'static str {
    let ua = ua.to_lowercase();
    RULES
        .iter()
        .find(|(r, _)| match r {
            Rule::Contains(s) => ua.contains(s),
            Rule::Prefix(s) => ua.starts_with(s),
        })
        .map_or("other", |(_, code)| code)
}

/// 无 UA 头 / 非 UTF-8 / 不匹配任何规则 → `other`。
pub(crate) fn detect_client(headers: &HeaderMap) -> &'static str {
    classify(headers.get(USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_driven() {
        let cases = [
            ("claude-cli/2.0.1 (external, cli)", "claude_code"),
            ("CLAUDE-CLI/2.0.1", "claude_code"),
            ("codex_cli_rs/0.1 (Mac OS)", "codex"),
            ("Codex-TUI/1.0", "codex"),
            ("pi (darwin; arm64)", "pi"),
            ("pi/0.5.0 (darwin; node; arm64)", "pi"),
            ("PI (linux; x64)", "pi"),
            ("Cursor/0.42", "cursor"),
            ("Windsurf/1.0", "windsurf"),
            ("GeminiCLI/0.1.0 (darwin; arm64)", "gemini_cli"),
            ("gemini-cli/1", "gemini_cli"),
            ("Mozilla/5.0 api (x)", "other"),
            ("curl/8.0", "other"),
            ("\u{0}garbage;;;", "other"),
            ("", "other"),
            // 表序：同时含 claude-cli 与 codex → 先到先得 claude_code
            ("claude-cli codex", "claude_code"),
        ];
        for (ua, want) in cases {
            assert_eq!(classify(ua), want, "ua={ua:?}");
        }
    }

    #[test]
    fn missing_or_non_utf8_header_is_other() {
        assert_eq!(detect_client(&HeaderMap::new()), "other");
        let mut h = HeaderMap::new();
        h.insert(USER_AGENT, axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap());
        assert_eq!(detect_client(&h), "other");
        let mut h = HeaderMap::new();
        h.insert(USER_AGENT, "claude-cli/x".parse().unwrap());
        assert_eq!(detect_client(&h), "claude_code");
    }
}
