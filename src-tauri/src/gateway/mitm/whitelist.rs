//! MITM 白名单 host 匹配（Clash 规则集 4 类型：domain/suffix/keyword/ipcidr）。
//!
//! `rule_type` + `host_pattern`（语义泛化，复用列存规则值，不改列名避迁移成本）：
//!  - `domain`  → host 全串精确匹配（大小写不敏感）
//!  - `suffix`  → Clash DOMAIN-SUFFIX 标准：裸域 + 所有子域（`anthropic.com` 命中 `anthropic.com`
//!    与 `api.anthropic.com`，不命中 `xanthropic.com`）
//!  - `keyword` → host 子串（大小写不敏感）
//!  - `ipcidr`  → 仅匹配 IP 字面 CONNECT 目标；host 解析成 IpAddr 后判 CIDR contains
//!    （不解析域名→IP，GeoIP/DNS 用户明确不要）
//!
//! 设计：CONNECT 是全局能力（无 group_key 概念），白名单全局；命中走 MITM，未命中走 P1 盲转。
//! 匹配在内存做（每条 CONNECT 请求 O(n)，n = 白名单行数，通常 < 50）；DB 读缓存于 ProxyState
//! 后续 ST4 接入时加（ST2 阶段只验证匹配逻辑 + DB 表可读）。
//!
//! 设计依据：design.md §2、spec `.trellis/spec/backend/proxy-connect-relay.md`。

use crate::gateway::db::Db;

/// 白名单行（DB mitm_whitelist 表映射）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitelistEntry {
    pub host_pattern: String, // 规则值（domain/suffix 域名 / keyword 子串 / ipcidr CIDR 串）
    pub rule_type: String,    // "domain" | "suffix" | "keyword" | "ipcidr"
    pub enabled: bool,
    pub source: String,       // "default" | "user"
}

/// host 是否命中给定白名单条目集合（enabled=true 的才参与匹配）。
///
/// 大小写不敏感（host 在 DNS 层不区分大小写；CONNECT target host 经 endpoint_host 已小写化，
/// 但白名单条目可能用户手填大小写不一，这里统一 lower 后比较）。
///
/// 按 rule_type 分派到 matches_rule：
///  - host 是 IP 字面 → domain/suffix/keyword 自动 miss（IpAddr 不含点号子串语义），ipcidr 命中
///  - host 是域名 → ipcidr 自动 miss（不解析域名→IP），host 类规则命中
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
        let rule_value = e.host_pattern.trim().to_lowercase();
        if rule_value.is_empty() {
            continue;
        }
        if matches_rule(&host, e.rule_type.as_str(), &rule_value) {
            return true;
        }
    }
    false
}

/// 单条规则匹配（按 rule_type 分派）。
///
/// host 已 lower + trim；rule_value 已 lower + trim。
fn matches_rule(host: &str, rule_type: &str, rule_value: &str) -> bool {
    match rule_type {
        // domain：host 全串精确匹配（大小写不敏感已 lower 化）。
        "domain" => host == rule_value,
        // suffix（Clash DOMAIN-SUFFIX 标准）：裸域 + 所有子域。
        // host == rule_value（裸域命中）|| host 以 `.rule_value` 结尾（子域，点前保证跨域 boundary
        // 防 `xanthropic.com` 命中 `anthropic.com`）。
        "suffix" => host == rule_value || host.ends_with(&format!(".{rule_value}")),
        // keyword：host 子串（大小写不敏感已 lower 化）。
        "keyword" => host.contains(rule_value),
        // ipcidr：仅匹配 IP 字面 CONNECT 目标。host 解析成 IpAddr 失败 → 域名 → miss。
        // 不解析域名→IP（GeoIP/DNS 用户明确不要，仅匹配 CONNECT target host 段已是 IP 字面情况）。
        "ipcidr" => {
            let Ok(ip) = host.parse::<std::net::IpAddr>() else {
                return false;
            };
            match rule_value.parse::<ipnet::IpNet>() {
                Ok(net) => net.contains(&ip),
                Err(_) => false,
            }
        }
        _ => false,
    }
}

/// 从 DB 读全部白名单条目（含 enabled=0，调用方按需过滤；matches_host 已按 enabled 跳过）。
///
/// 排序：created_at 升序（默认 host 在前，用户加的在后，行为可预测）。
pub async fn list_whitelist(db: &Db) -> Result<Vec<WhitelistEntry>, String> {
    db.0
        .call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT host_pattern, rule_type, enabled, source FROM mitm_whitelist ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(WhitelistEntry {
                    host_pattern: r.get::<_, String>(0)?,
                    rule_type: r.get::<_, String>(1)?,
                    enabled: r.get::<_, i64>(2)? != 0,
                    source: r.get::<_, String>(3)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
}

/// host 是否命中 DB 白名单（便捷封装：先读全表再匹配）。
///
/// ponytail: 每 CONNECT 调用读一次 DB，O(n) 扫描；n 小（< 50）可接受。
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

    fn entry(rule_type: &str, pattern: &str, enabled: bool) -> WhitelistEntry {
        WhitelistEntry {
            host_pattern: pattern.to_string(),
            rule_type: rule_type.to_string(),
            enabled,
            source: "default".to_string(),
        }
    }

    // ── domain（精确匹配）──────────────────────────────────────

    #[test]
    fn whitelist_domain_exact_hit_case_insensitive() {
        let entries = [entry("domain", "api.openai.com", true)];
        assert!(matches_host(entries.clone(), "api.openai.com"));
        assert!(matches_host(entries.clone(), "API.OpenAI.com")); // 大小写不敏感
        assert!(!matches_host(entries.clone(), "v1.api.openai.com")); // 子域不命中
        assert!(!matches_host(entries, "xapi.openai.com")); // 不含点号边界
    }

    // ── suffix（Clash DOMAIN-SUFFIX：裸域 + 子域）──────────────

    #[test]
    fn whitelist_suffix_hits_bare_and_subdomain() {
        let entries = [entry("suffix", "anthropic.com", true)];
        // Clash DOMAIN-SUFFIX 标准：裸域 + 所有子域（语义扩展，比旧 *.domain 更宽）
        assert!(matches_host(entries.clone(), "anthropic.com")); // 裸域命中
        assert!(matches_host(entries.clone(), "api.anthropic.com")); // 子域命中
        assert!(matches_host(entries.clone(), "API.Anthropic.com")); // 大小写不敏感
        assert!(matches_host(entries, "x.api.anthropic.com")); // 多层子域
    }

    #[test]
    fn whitelist_suffix_misses_cross_domain_attack() {
        // 关键安全测试：跨域 boundary 必须有点号保护
        let entries = [entry("suffix", "anthropic.com", true)];
        assert!(!matches_host(entries.clone(), "xanthropic.com")); // 无点号 boundary
        assert!(!matches_host(entries.clone(), "anthropic.com.evil.com")); // suffix 后接 evil
        assert!(!matches_host(entries, "x.anthropic.com.evil.com"));
    }

    // ── keyword（子串，大小写不敏感）────────────────────────────

    #[test]
    fn whitelist_keyword_substring_case_insensitive() {
        let entries = [entry("keyword", "openai", true)];
        assert!(matches_host(entries.clone(), "openai.com"));
        assert!(matches_host(entries.clone(), "chat.openai.com"));
        assert!(matches_host(entries.clone(), "OPENAI-API.example")); // 大小写不敏感
        assert!(matches_host(entries.clone(), "myopenai.tool")); // 子串命中
        assert!(!matches_host(entries, "anthropic.com"));
    }

    // ── ipcidr（仅匹配 IP 字面）─────────────────────────────────

    #[test]
    fn whitelist_ipcidr_matches_ip_literal_target() {
        let entries = [entry("ipcidr", "24.199.123.28/32", true)];
        assert!(matches_host(entries.clone(), "24.199.123.28")); // /32 精确 IP 命中
        assert!(!matches_host(entries.clone(), "24.199.123.29")); // 同段不同 IP 不命中
    }

    #[test]
    fn whitelist_ipcidr_matches_cidr_range() {
        let entries = [entry("ipcidr", "10.0.0.0/8", true)];
        assert!(matches_host(entries.clone(), "10.1.2.3"));
        assert!(matches_host(entries.clone(), "10.255.255.255"));
        assert!(!matches_host(entries.clone(), "11.0.0.1"));
    }

    #[test]
    fn whitelist_ipcidr_misses_domain_target() {
        // 关键反例：host 是域名 → IpAddr parse 失败 → ipcidr 自动 miss（不解析域名→IP）
        let entries = [entry("ipcidr", "24.199.123.28/32", true)];
        assert!(!matches_host(entries.clone(), "api.openai.com"));
        assert!(!matches_host(entries, "openai.com"));
    }

    // ── 跨类型互斥（host 类型自动 miss 错类型规则）──────────────

    #[test]
    fn whitelist_host_class_when_host_is_ip_literal() {
        // host 是 IP 字面时的跨类型行为：
        //  - domain 精确匹配仍命中（host == rule_value 全串相等，与 host 是否 IP 无关）
        //  - suffix/keyword 仍按字符串语义判（IP 含点号 / 子串，可命中相应规则）
        //  - 真正的跨类型互斥只在「host 是域名 + 规则是 ipcidr」时发生（见下一个测试）
        let entries = [
            entry("domain", "24.199.123.28", true),
            entry("suffix", "123.28", true),
            entry("keyword", "199", true),
        ];
        assert!(matches_host(entries.clone(), "24.199.123.28")); // domain 精确命中
        // suffix/keyword 在 IP 字面 host 上仍按字符串语义判（点号子串 / 字符子串）
        let suffix_only = [entry("suffix", "123.28", true)];
        assert!(matches_host(suffix_only, "24.199.123.28")); // ends_with(".123.28")
        let keyword_only = [entry("keyword", "199", true)];
        assert!(matches_host(keyword_only, "24.199.123.28")); // contains("199")
    }

    #[test]
    fn whitelist_ipcidr_misses_when_host_is_domain() {
        // host 是域名 → ipcidr 自动 miss（IpAddr parse 失败）
        let entries = [entry("ipcidr", "1.2.3.0/24", true)];
        assert!(!matches_host(entries, "api.openai.com"));
    }

    // ── enabled / 空 host / 综合集成 ─────────────────────────────

    #[test]
    fn whitelist_match_disabled_entry_ignored() {
        let entries = [
            entry("suffix", "anthropic.com", false), // 用户禁用
            entry("domain", "api.openai.com", true),
        ];
        assert!(!matches_host(entries.clone(), "api.anthropic.com"));
        assert!(matches_host(entries, "api.openai.com"));
    }

    #[test]
    fn whitelist_match_empty_host_denied() {
        let entries = [entry("suffix", "anthropic.com", true)];
        assert!(!matches_host(entries.clone(), ""));
        assert!(!matches_host(entries, "   "));
    }

    #[test]
    fn whitelist_unknown_rule_type_denied() {
        // 未知 rule_type（含空）必 miss（防脏数据 / 旧版残留）
        let entries = [entry("ip-asn", "20473", true)];
        assert!(!matches_host(entries.clone(), "1.2.3.4"));
        assert!(!matches_host(entries, "api.openai.com"));
    }

    #[test]
    fn whitelist_default_clash_ruleset_integration() {
        // 综合：模拟 37 条 default seed 的核心覆盖（不逐条列），验各类型同表共存
        let entries = [
            entry("domain", "chat.openai.com.cdn.cloudflare.net", true),
            entry("suffix", "anthropic.com", true),
            entry("suffix", "openai.com", true),
            entry("keyword", "openai", true),
            entry("ipcidr", "24.199.123.28/32", true),
        ];
        // 各类型各自命中
        assert!(matches_host(entries.clone(), "chat.openai.com.cdn.cloudflare.net")); // domain
        assert!(matches_host(entries.clone(), "api.anthropic.com")); // suffix
        assert!(matches_host(entries.clone(), "openai.com")); // suffix 裸域
        assert!(matches_host(entries.clone(), "myopenai.tool")); // keyword 子串
        assert!(matches_host(entries.clone(), "24.199.123.28")); // ipcidr IP 字面
        // 跨类型互斥
        assert!(!matches_host(entries.clone(), "evil.com"));
        assert!(!matches_host(entries, "1.2.3.4"));
    }
}
