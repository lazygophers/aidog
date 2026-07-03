//! MITM 白名单 host suffix 匹配（P3 ST2，D6 全局）。
//!
//! `host_pattern` 形态：
//!  - `*.anthropic.com` → suffix 匹配任意 anthropic.com 子域（不含 `anthropic.com` 本身）
//!  - `api.anthropic.com` → 精确匹配
//!  - `anthropic.com` → 精确匹配 `anthropic.com`（不隐式匹配子域）
//!
//! 设计：CONNECT 是全局能力（无 group_key 概念），白名单全局；命中走 MITM，未命中走 P1 盲转。
//! 匹配在内存做（每条 CONNECT 请求 O(n)，n = 白名单行数，通常 < 20）；DB 读缓存于 ProxyState
//! 后续 ST4 接入时加（ST2 阶段只验证匹配逻辑 + DB 表可读）。
//!
//! 设计依据：design.md §2、spec `.trellis/spec/backend/proxy-connect-relay.md`。

use crate::gateway::db::Db;

/// 白名单行（DB mitm_whitelist 表映射）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitelistEntry {
    pub host_pattern: String,
    pub enabled: bool,
    pub source: String, // "default" | "user"
}

/// host 是否命中给定白名单条目集合（enabled=true 的才参与匹配）。
///
/// 大小写不敏感（host 在 DNS 层不区分大小写；CONNECT target host 经 endpoint_host 已小写化，
/// 但白名单条目可能用户手填大小写不一，这里统一 lower 后比较）。
///
/// `*.anthropic.com` 仅匹配子域（`api.anthropic.com` ✓，`anthropic.com` ✗，`xapi.anthropic.com.evil.com` ✗）——
/// 防 suffix 误匹配跨域攻击（`evil.com` 末段含 `anthropic.com` 不算命中）。
pub fn matches_host<I>(entries: I, host: &str) -> bool
where
    I: IntoIterator<Item = WhitelistEntry>,
{
    let host = host.trim().to_lowercase();
    if host.is_empty() {
        return false;
    }
    for e in entries {
        if !e.enabled {
            continue;
        }
        let pat = e.host_pattern.trim().to_lowercase();
        if pat.is_empty() {
            continue;
        }
        if let Some(suffix) = pat.strip_prefix("*.") {
            // wildcard：host 必须是 `*.domain` 的真子域：host == <something>.domain 且
            // <something> 非空、不含额外点（防 `anthropic.com.evil.com` 命中 `*.anthropic.com`）。
            // 正确做法：host 以 `.domain` 结尾（点前保证跨域 boundary）。
            let dot_domain = format!(".{suffix}");
            if host.ends_with(&dot_domain) && host.len() > dot_domain.len() {
                return true;
            }
            continue;
        }
        // 精确匹配（不做隐式子域扩展）。
        if host == pat {
            return true;
        }
    }
    false
}

/// 从 DB 读全部白名单条目（含 enabled=0，调用方按需过滤；matches_host 已按 enabled 跳过）。
///
/// 排序：created_at 升序（默认 host 在前，用户加的在后，行为可预测）。
pub async fn list_whitelist(db: &Db) -> Result<Vec<WhitelistEntry>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT host_pattern, enabled, source FROM mitm_whitelist ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(WhitelistEntry {
                    host_pattern: r.get::<_, String>(0)?,
                    enabled: r.get::<_, i64>(1)? != 0,
                    source: r.get::<_, String>(2)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// host 是否命中 DB 白名单（便捷封装：先读全表再匹配）。
///
/// ponytail: 每 CONNECT 调用读一次 DB，O(n) 扫描；n 小（< 20）可接受。
/// ST4 接入热路径时再加 ProxyState 内存缓存（HashMap<String, Vec<WhitelistEntry>> + 写时失效）。
pub async fn matches_db(db: &Db, host: &str) -> bool {
    match list_whitelist(db).await {
        Ok(entries) => matches_host(entries, host),
        Err(e) => {
            tracing::warn!(error = %e, host, "mitm whitelist: list failed, deny MITM (fallback to blind relay)");
            false // 读失败保守拒绝 MITM（走 P1 盲转，更安全）
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(pattern: &str, enabled: bool) -> WhitelistEntry {
        WhitelistEntry {
            host_pattern: pattern.to_string(),
            enabled,
            source: "default".to_string(),
        }
    }

    #[test]
    fn whitelist_match_wildcard_hits_subdomain() {
        let entries = [entry("*.anthropic.com", true)];
        assert!(matches_host(entries.clone(), "api.anthropic.com"));
        assert!(matches_host(entries, "API.Anthropic.com")); // 大小写不敏感
    }

    #[test]
    fn whitelist_match_wildcard_misses_bare_domain() {
        let entries = [entry("*.anthropic.com", true)];
        assert!(!matches_host(entries, "anthropic.com")); // 裸域不命中 *.domain
    }

    #[test]
    fn whitelist_match_wildcard_misses_cross_domain_attack() {
        // 关键安全测试：`evil.com` 末段含 `anthropic.com` 不能命中 `*.anthropic.com`
        let entries = [entry("*.anthropic.com", true)];
        assert!(!matches_host(entries.clone(), "anthropic.com.evil.com"));
        assert!(!matches_host(entries, "x.anthropic.com.evil.com"));
    }

    #[test]
    fn whitelist_match_exact_host() {
        let entries = [entry("api.openai.com", true)];
        assert!(matches_host(entries.clone(), "api.openai.com"));
        assert!(!matches_host(entries, "v1.api.openai.com")); // 精确不隐式扩展子域
    }

    #[test]
    fn whitelist_match_disabled_entry_ignored() {
        let entries = [
            entry("*.anthropic.com", false), // 用户禁用
            entry("api.openai.com", true),
        ];
        assert!(!matches_host(entries.clone(), "api.anthropic.com"));
        assert!(matches_host(entries, "api.openai.com"));
    }

    #[test]
    fn whitelist_match_empty_host_denied() {
        let entries = [entry("*.anthropic.com", true)];
        assert!(!matches_host(entries.clone(), ""));
        assert!(!matches_host(entries, "   "));
    }

    #[test]
    fn whitelist_match_evil_com_not_in_whitelist() {
        // design 验收断言：`evil.com` 不命中
        let entries = [
            entry("*.anthropic.com", true),
            entry("*.openai.com", true),
            entry("api.anthropic.com", true),
        ];
        assert!(!matches_host(entries.clone(), "evil.com"));
        assert!(!matches_host(entries, "bank.evil.com"));
    }

    #[test]
    fn whitelist_match_multiple_patterns_any_hit() {
        let entries = [
            entry("*.anthropic.com", true),
            entry("*.openai.com", true),
            entry("api.deepseek.com", true),
        ];
        assert!(matches_host(entries.clone(), "api.anthropic.com"));
        assert!(matches_host(entries.clone(), "chatgpt.openai.com"));
        assert!(matches_host(entries.clone(), "api.deepseek.com"));
        assert!(!matches_host(entries, "api.unknown.com"));
    }
}
