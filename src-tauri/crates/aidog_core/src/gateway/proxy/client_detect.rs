//! 入站客户端识别：按 User-Agent 查 `crate::client_vocab` 两级词表，写 `proxy_log.client` 与
//! `stats_agg_hourly.client`。统计维度只存词表码或限长 `ua:<产品名>`（基数受控）；原始 UA 另存 `proxy_log.user_agent`。
//!
//! 两种语义同一接口：[`identify_client`] 返回 `None` = 认不出（白名单判定视为未命中）；
//! [`detect_client`] 把 `None` 映射为统计码 `other`。存量空串（未知）仅旧行 / 带外日志，本模块不产出。

use axum::http::{HeaderMap, header::USER_AGENT};

use crate::client_vocab::{ClientEntry, identify};
use crate::gateway::router::ClientProbe;

/// 无 UA 头 / 非 UTF-8 / 不在词表 → `None`（认不出）；命中变体返回变体，只命中大类返回大类。
pub(crate) fn identify_client(headers: &HeaderMap) -> Option<&'static ClientEntry> {
    identify(headers.get(USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or(""))
}

/// 白名单闸门用探针：UA 原文 + 内置词表识别结果（无 UA / 非 UTF-8 → 空串、认不出）。
pub(crate) fn client_probe(headers: &HeaderMap) -> ClientProbe {
    ClientProbe::from_ua(headers.get(USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or(""))
}

/// 统计 / 日志用码：词表码；认不出 → `ua:<产品名>`；截不出 / 无 UA → `other`。
pub(crate) fn detect_client(headers: &HeaderMap) -> String {
    if let Some(e) = identify_client(headers) {
        return e.code.to_string();
    }
    let ua = headers.get(USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("");
    ua_product_name(ua).map_or_else(|| "other".to_string(), |n| format!("{}{n}", crate::client_vocab::UA_CODE_PREFIX))
}

/// UA 产品名：第一个空白或 `/` 之前，首字符须是 ASCII 字母数字（`(compatible)` 这类不算产品名），
/// 只留 ASCII 字母数字与 `. _ -`，最长 32 字符，保留大小写；否则 `None`。
/// 过滤并限长是为防乱码 UA 把统计 client 维度基数刷爆。
fn ua_product_name(ua: &str) -> Option<String> {
    let first = ua.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
    if !first.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return None;
    }
    let name: String = first
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .take(32)
        .collect();
    Some(name)
}

/// 原始 UA（截断到 `USER_AGENT_MAX_CHARS` 字符）；无头 / 非 UTF-8 → 空串。
pub(crate) fn raw_user_agent(headers: &HeaderMap) -> String {
    headers
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(aidog_db::models::USER_AGENT_MAX_CHARS).collect())
        .unwrap_or_default()
}

/// Claude Code 网关提示头（`x-claude-code-request-class` / `x-claude-code-agent-type`）的值上限。
const CLAUDE_HINT_MAX_CHARS: usize = 64;

/// Claude Code 网关提示头：`(request_class, agent_type)`。trim、截 64 字符；含非可打印 ASCII → 空串；
/// 未知取值原样保留（不校验词表）。无头 → 空串。
pub(crate) fn claude_hint_headers(headers: &HeaderMap) -> (String, String) {
    let get = |name: &str| -> String {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| s.chars().all(|c| c.is_ascii_graphic() || c == ' '))
            .map(|s| s.chars().take(CLAUDE_HINT_MAX_CHARS).collect())
            .unwrap_or_default()
    };
    (get("x-claude-code-request-class"), get("x-claude-code-agent-type"))
}

/// Claude Code 网关提示头里除 request-class / agent-type 之外的七个（见 `claude_hint_ext`）。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ClaudeHintsExt {
    pub prompt_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub parent_agent_id: String,
    pub compaction: String,
    pub context_compacted: String,
    pub prev_tool_durations: String,
}

impl ClaudeHintsExt {
    pub(crate) fn apply(self, log: &mut aidog_db::models::ProxyLog) {
        log.prompt_id = self.prompt_id;
        log.session_id = self.session_id;
        log.agent_id = self.agent_id;
        log.parent_agent_id = self.parent_agent_id;
        log.compaction = self.compaction;
        log.context_compacted = self.context_compacted;
        log.prev_tool_durations = self.prev_tool_durations;
    }
}

/// 单个提示头的净化值：trim、仅可打印 ASCII（否则空串）、截 `cap` 字符；无头 → 空串。
fn hint_value(headers: &HeaderMap, name: &str, cap: usize) -> String {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| s.chars().all(|c| c.is_ascii_graphic() || c == ' '))
        .map(|s| s.chars().take(cap).collect())
        .unwrap_or_default()
}

/// 百分号解码（`%XX`）；非法序列原样保留；解码后字节按 UTF-8 宽松还原。
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `x-claude-code-prev-tool-durations` 解码后的字节上限。
const PREV_TOOL_DURATIONS_MAX_BYTES: usize = 4096;

/// Claude Code 网关提示头（prompt-id / session-id / agent-id / parent-agent-id / compaction /
/// context-compacted / prev-tool-durations）。id 与标记同 `claude_hint_headers` 净化、截 64；
/// prev-tool-durations 先按 ASCII 净化再百分号解码，解码后截 4096 字节（不劈开字符）。
pub(crate) fn claude_hint_ext(headers: &HeaderMap) -> ClaudeHintsExt {
    let get = |name: &str| hint_value(headers, name, CLAUDE_HINT_MAX_CHARS);
    let mut durations = percent_decode(&hint_value(headers, "x-claude-code-prev-tool-durations", usize::MAX));
    if durations.len() > PREV_TOOL_DURATIONS_MAX_BYTES {
        let mut end = PREV_TOOL_DURATIONS_MAX_BYTES;
        while !durations.is_char_boundary(end) {
            end -= 1;
        }
        durations.truncate(end);
    }
    ClaudeHintsExt {
        prompt_id: get("x-claude-code-prompt-id"),
        session_id: get("x-claude-code-session-id"),
        agent_id: get("x-claude-code-agent-id"),
        parent_agent_id: get("x-claude-code-parent-agent-id"),
        compaction: get("x-claude-code-compaction"),
        context_compacted: get("x-claude-code-context-compacted"),
        prev_tool_durations: durations,
    }
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
    fn claude_hint_headers_sanitized() {
        let mut h = HeaderMap::new();
        assert_eq!(claude_hint_headers(&h), (String::new(), String::new()));
        h.insert("x-claude-code-request-class", " main ".parse().unwrap());
        h.insert("x-claude-code-agent-type", "general-purpose".parse().unwrap());
        assert_eq!(claude_hint_headers(&h), ("main".into(), "general-purpose".into()));
        // 非可打印 ASCII（含 UTF-8）→ 丢弃为空；超长 → 截 64
        h.insert("x-claude-code-request-class", axum::http::HeaderValue::from_bytes("子代理".as_bytes()).unwrap());
        h.insert("x-claude-code-agent-type", "a".repeat(100).parse().unwrap());
        let (rc, at) = claude_hint_headers(&h);
        assert_eq!(rc, "");
        assert_eq!(at.len(), 64);
    }

    #[test]
    fn claude_hint_ext_sanitized_and_decoded() {
        let mut h = HeaderMap::new();
        assert_eq!(claude_hint_ext(&h), ClaudeHintsExt::default());
        h.insert("x-claude-code-prompt-id", " 6f1c0c1e-aaaa-bbbb-cccc-000000000001 ".parse().unwrap());
        h.insert("x-claude-code-session-id", "sess-1".parse().unwrap());
        h.insert("x-claude-code-agent-id", "agent-1".parse().unwrap());
        h.insert("x-claude-code-parent-agent-id", "agent-0".parse().unwrap());
        h.insert("x-claude-code-compaction", "auto".parse().unwrap());
        h.insert("x-claude-code-context-compacted", "brand_new".parse().unwrap());
        h.insert("x-claude-code-prev-tool-durations", "Bash=742;mcp__a%3Bb%3Dc%20d=9;%E5%B7%A5%E5%85%B7=3".parse().unwrap());
        let x = claude_hint_ext(&h);
        assert_eq!(x.prompt_id, "6f1c0c1e-aaaa-bbbb-cccc-000000000001");
        assert_eq!((x.session_id.as_str(), x.agent_id.as_str(), x.parent_agent_id.as_str()), ("sess-1", "agent-1", "agent-0"));
        assert_eq!((x.compaction.as_str(), x.context_compacted.as_str()), ("auto", "brand_new"));
        assert_eq!(x.prev_tool_durations, "Bash=742;mcp__a;b=c d=9;工具=3");

        // 非可打印 ASCII → 空；id 超长 → 截 64
        h.insert("x-claude-code-prompt-id", axum::http::HeaderValue::from_bytes("提示".as_bytes()).unwrap());
        h.insert("x-claude-code-session-id", "s".repeat(100).parse().unwrap());
        // 坏百分号序列原样保留；解码后超 4096 字节截断（不劈开 UTF-8）
        h.insert("x-claude-code-prev-tool-durations", format!("a%ZZ=1;{}", "%E5%B7%A5".repeat(2000)).parse().unwrap());
        let x = claude_hint_ext(&h);
        assert_eq!(x.prompt_id, "");
        assert_eq!(x.session_id.len(), 64);
        assert!(x.prev_tool_durations.starts_with("a%ZZ=1;工"));
        assert!(x.prev_tool_durations.len() <= 4096 && x.prev_tool_durations.len() > 4000);
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
        assert!(identify_client(&with_ua("Mozilla/5.0 api (x)")).is_none());
        // 变体 → 所属大类（白名单勾大类用）
        assert_eq!(identify_client(&with_ua("codex-tui/1.0")).unwrap().category(), "codex");
    }

    #[test]
    fn unknown_ua_becomes_product_name_code() {
        let long = "a".repeat(40);
        let want_long = format!("ua:{}", "a".repeat(32));
        let cases: [(&str, &str); 9] = [
            ("OpenViking", "ua:OpenViking"),
            ("OpenViking/1.2", "ua:OpenViking"),
            ("OpenViking/0.5.2 (darwin)", "ua:OpenViking"),
            ("My_Tool-2.x agent", "ua:My_Tool-2.x"),
            ("(compatible)", "other"),
            ("!!!/1.0", "other"),
            ("a!b@c", "ua:abc"),
            ("", "other"),
            (long.as_str(), want_long.as_str()),
        ];
        for (ua, want) in cases {
            let h = if ua.is_empty() { HeaderMap::new() } else { with_ua(ua) };
            assert_eq!(detect_client(&h), want, "ua={ua:?}");
        }
        // 白名单语义不变：产品名码不算认出
        assert!(identify_client(&with_ua("OpenViking/1.2")).is_none());
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
