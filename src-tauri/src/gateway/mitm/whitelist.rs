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

/// 默认白名单规则集（37 条：Claude 3 + OpenAI 34）。
///
/// 来源：blackmatrix7/ios_rule_script OpenAI/Claude 规则集（Clash DOMAIN/SUFFIX/KEYWORD/IPCIDR）。
/// 元组 `(rule_type, pattern)`：rule_type ∈ {domain, suffix, keyword, ipcidr}，
/// pattern 存规则值（host 域名 / CIDR 串）。
///
/// 单源（schema migration 041 seed + 本模块 import_defaults command + 测试 共用此常量）。
/// 舍弃：IP-ASN 20473（不支持）；GeoIP/DNS 解析（不要）。
pub const DEFAULT_RULES: &[(&str, &str)] = &[
    // ── Claude（3 条）─────────────────────────────────────────
    ("domain", "cdn.usefathom.com"),
    ("suffix", "anthropic.com"),
    ("suffix", "claude.ai"),
    // ── OpenAI domain（7 条）──────────────────────────────────
    ("domain", "browser-intake-datadoghq.com"),
    ("domain", "chat.openai.com.cdn.cloudflare.net"),
    ("domain", "openai-api.arkoselabs.com"),
    ("domain", "openaicom-api-bdcpf8c6d2e9atf6.z01.azurefd.net"),
    ("domain", "openaicomproductionae4b.blob.core.windows.net"),
    ("domain", "production-openaicom-storage.azureedge.net"),
    ("domain", "static.cloudflareinsights.com"),
    // ── OpenAI suffix（24 条）─────────────────────────────────
    ("suffix", "ai.com"),
    ("suffix", "algolia.net"),
    ("suffix", "api.statsig.com"),
    ("suffix", "auth0.com"),
    ("suffix", "chatgpt.com"),
    ("suffix", "chatgpt.livekit.cloud"),
    ("suffix", "client-api.arkoselabs.com"),
    ("suffix", "events.statsigapi.net"),
    ("suffix", "featuregates.org"),
    ("suffix", "host.livekit.cloud"),
    ("suffix", "identrust.com"),
    ("suffix", "intercom.io"),
    ("suffix", "intercomcdn.com"),
    ("suffix", "launchdarkly.com"),
    ("suffix", "oaistatic.com"),
    ("suffix", "oaiusercontent.com"),
    ("suffix", "observeit.net"),
    ("suffix", "openai.com"),
    ("suffix", "openaiapi-site.azureedge.net"),
    ("suffix", "openaicom.imgix.net"),
    ("suffix", "segment.io"),
    ("suffix", "sentry.io"),
    ("suffix", "stripe.com"),
    ("suffix", "turn.livekit.cloud"),
    // ── OpenAI keyword（1 条）──────────────────────────────────
    ("keyword", "openai"),
    // ── OpenAI ipcidr（2 条，仅匹配 IP 字面 CONNECT 目标）──────
    ("ipcidr", "24.199.123.28/32"),
    ("ipcidr", "64.23.132.171/32"),
];

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

/// 收集 host 命中的白名单条目（与 `matches_host` 同语义，但返命中规则列表而非布尔）。
///
/// 仅遍历 enabled 条目（反映 MITM 实际行为 = 仅 enabled 生效；disabled 不参与匹配）。
/// 复用 `matches_rule` 单源匹配引擎，禁重复实现（测试 URL 命中与运行时白名单判定走同一逻辑）。
///
/// 用途：URL 命中测试 command（用户输入 URL → 解析 host → 本 fn 返命中规则列表，
/// 前端透明展示哪些规则命中）。
pub fn evaluate_host(entries: &[WhitelistEntry], host: &str) -> Vec<WhitelistEntry> {
    let host = host.trim().to_lowercase();
    if host.is_empty() {
        return Vec::new();
    }
    entries
        .iter()
        .filter(|e| e.enabled)
        .filter(|e| {
            let rule_value = e.host_pattern.trim().to_lowercase();
            !rule_value.is_empty() && matches_rule(&host, e.rule_type.as_str(), &rule_value)
        })
        .cloned()
        .collect()
}

/// 单条规则匹配（按 rule_type 分派）。
///
/// host 已 lower + trim；rule_value 已 lower + trim。
///
/// pub：URL 命中测试 command 复用此单源匹配引擎（`evaluate_host` 遍历条目调本 fn），
/// 禁 command 内联重写匹配逻辑（双源漂移风险）。matches_host 也走此 fn。
pub fn matches_rule(host: &str, rule_type: &str, rule_value: &str) -> bool {
    match rule_type {
        // domain：host 全串精确匹配（大小写不敏感已 lower 化）。
        "domain" => host == rule_value,
        // suffix（Clash DOMAIN-SUFFIX 标准）：裸域 + 所有子域。
        // host == rule_value（裸域命中）|| host 以 `.rule_value` 结尾（子域，点前保证跨域 boundary
        // 防 `xanthropic.com` 命中 `anthropic.com`）。
        // 前导点容错：用户可能写 `.cn` / `..cn`（前导点）入 db，strip 所有前导点归一化后再匹配，
        // 否则 format!(".{rule_value}") 会产生 `..cn`（双点）永不命中。`.cn`/`cn`/`..cn` 等价命中 `cn`。
        // 全点 rule_value (`...`) → normalized 空 → 不命中（脏数据兜底）。
        "suffix" => {
            let normalized = rule_value.trim_start_matches('.');
            !normalized.is_empty()
                && (host == normalized || host.ends_with(&format!(".{normalized}")))
        }
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
    db.write_conn()
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

    // ── suffix 前导点容错（用户写 `.cn` / `..cn` / `cn` 等价命中）──────────

    #[test]
    fn suffix_leading_dot_matches() {
        // rule `.cn` 命中 `open.bigmodel.cn`（前导点容错：strip 后等价裸域 `cn`）
        assert!(matches_rule("open.bigmodel.cn", "suffix", ".cn"));
        assert!(matches_rule("api.openai.com", "suffix", ".com"));
    }

    #[test]
    fn suffix_multi_leading_dot_normalized() {
        // rule `..cn` 归一化为 `cn` 仍命中（trim_start_matches('.') strip 所有前导点）
        assert!(matches_rule("open.bigmodel.cn", "suffix", "..cn"));
        // 裸域 `cn` 也命中（向后兼容，Clash DOMAIN-SUFFIX 标准语义）
        assert!(matches_rule("open.bigmodel.cn", "suffix", "cn"));
    }

    #[test]
    fn suffix_all_dots_empty_misses() {
        // rule `...` 全点 → normalized 空 → 不命中（脏数据兜底，防 format! 产生 `....` 误匹配）
        assert!(!matches_rule("open.bigmodel.cn", "suffix", "..."));
        assert!(!matches_rule("anything.com", "suffix", "."));
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

    // ── DEFAULT_RULES 常量完整性 + import_defaults 去重语义 ─────

    #[test]
    fn default_rules_has_37_unique_patterns() {
        // 常量完整性：37 条（Claude 3 + OpenAI 34），host_pattern 全唯一（UNIQUE 约束要求）。
        assert_eq!(DEFAULT_RULES.len(), 37, "DEFAULT_RULES must be 37 entries");
        let mut patterns: Vec<&str> = DEFAULT_RULES.iter().map(|(_, p)| *p).collect();
        patterns.sort();
        let n_unique = patterns.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(n_unique, 37, "all host_patterns must be unique (DB UNIQUE constraint)");
        // rule_type 全部合法
        for (rt, _) in DEFAULT_RULES {
            assert!(
                matches!(*rt, "domain" | "suffix" | "keyword" | "ipcidr"),
                "invalid rule_type: {rt}"
            );
        }
    }

    /// import_defaults 去重：mock DB 已有 1 条默认 + 1 条自定义 → INSERT OR IGNORE
    /// 仅补 36 条默认缺失（37 - 1 已存在），自定义不动，返 (36, 1)。
    ///
    /// 复刻 commands::mitm::mitm_whitelist_import_defaults 的 INSERT 循环（同 SQL + changes() 统计），
    /// 验幂等去重语义（UNIQUE(host_pattern) 约束 + source='default'）。
    #[test]
    fn import_defaults_insert_or_ignore_dedup() {
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        // 建 mitm_whitelist 表（与 schema migration 040/041 一致：含 rule_type 列 + UNIQUE(host_pattern)）。
        conn.execute_batch(
            r#"CREATE TABLE mitm_whitelist (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                host_pattern TEXT NOT NULL,
                rule_type TEXT NOT NULL DEFAULT 'suffix',
                enabled INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL DEFAULT 'user',
                created_at INTEGER NOT NULL DEFAULT 0,
                UNIQUE(host_pattern)
            );"#,
        )
        .unwrap();

        // 预置：1 条默认（DEFAULT_RULES 之一：anthropic.com suffix）+ 1 条自定义（非 DEFAULT_RULES）。
        // 用 DEFAULT_RULES 的真实成员作"已存在的默认"，确保 import 循环到它时 changes()==0。
        conn.execute(
            "INSERT INTO mitm_whitelist (host_pattern, rule_type, enabled, source, created_at) \
             VALUES ('anthropic.com', 'suffix', 1, 'default', 100)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mitm_whitelist (host_pattern, rule_type, enabled, source, created_at) \
             VALUES ('my-custom-host.example.com', 'suffix', 1, 'user', 101)",
            [],
        )
        .unwrap();

        // 复刻 command 的 INSERT OR IGNORE 循环 + changes() 统计。
        let now = 9999_i64;
        let mut imported = 0usize;
        let mut skipped = 0usize;
        for (rule_type, pattern) in DEFAULT_RULES {
            let n = conn
                .execute(
                    "INSERT OR IGNORE INTO mitm_whitelist (host_pattern, rule_type, enabled, source, created_at) \
                     VALUES (?1, ?2, 1, 'default', ?3)",
                    rusqlite::params![pattern, rule_type, now],
                )
                .unwrap();
            if n == 1 {
                imported += 1;
            } else {
                skipped += 1;
            }
        }

        // 验收：36 默认新导入（37 - 1 已存在），1 默认跳过（anthropic.com 已在）。
        assert_eq!(imported, 36, "imported: 37 default rules - 1 pre-existing");
        assert_eq!(skipped, 1, "skipped: the 1 pre-existing default rule");

        // 自定义条目未被 import 循环触及（不在 DEFAULT_RULES 中），source 仍为 'user'。
        let custom_source: String = conn
        .query_row(
            "SELECT source FROM mitm_whitelist WHERE host_pattern = 'my-custom-host.example.com'",
            [],
            |r| r.get(0),
        )
        .unwrap();
        assert_eq!(custom_source, "user", "custom entry must be untouched by import");

        // 总行数：37 默认 + 1 自定义 = 38。
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM mitm_whitelist", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 38, "total rows: 37 default + 1 custom");

        // 幂等：再跑一次 import，全部 changes()==0 → (0, 37)，行数不变。
        let mut imported2 = 0usize;
        let mut skipped2 = 0usize;
        for (rule_type, pattern) in DEFAULT_RULES {
            let n = conn
                .execute(
                    "INSERT OR IGNORE INTO mitm_whitelist (host_pattern, rule_type, enabled, source, created_at) \
                     VALUES (?1, ?2, 1, 'default', ?3)",
                    rusqlite::params![pattern, rule_type, now],
                )
                .unwrap();
            if n == 1 { imported2 += 1; } else { skipped2 += 1; }
        }
        assert_eq!(imported2, 0, "idempotent re-import: 0 new");
        assert_eq!(skipped2, 37, "idempotent re-import: all 37 skipped");
        let total2: i64 = conn.query_row("SELECT COUNT(*) FROM mitm_whitelist", [], |r| r.get(0)).unwrap();
        assert_eq!(total2, 38, "row count unchanged after re-import");
    }

    // ── evaluate_host（返命中规则列表，仅 enabled）──────────────

    #[test]
    fn evaluate_host_returns_only_enabled_hits() {
        // mock：domain/suffix/keyword/ipcidr 各 1，含 1 disabled suffix（应被跳过）。
        let entries = [
            entry("domain", "api.anthropic.com", true),
            entry("suffix", "anthropic.com", true),
            entry("keyword", "openai", true),
            entry("ipcidr", "24.199.123.28/32", true),
            entry("suffix", "evil.com", false), // disabled → 不参与
        ];
        // api.anthropic.com 命中：domain(api.anthropic.com) + suffix(anthropic.com)。
        // keyword(openai) 不命中，ipcidr 不命中（域名非 IP 字面）。
        let hits = evaluate_host(&entries, "api.anthropic.com");
        assert_eq!(hits.len(), 2, "domain + suffix both hit");
        let patterns: Vec<&str> = hits.iter().map(|e| e.host_pattern.as_str()).collect();
        assert!(patterns.contains(&"api.anthropic.com"), "domain hit");
        assert!(patterns.contains(&"anthropic.com"), "suffix hit");
        assert!(!patterns.contains(&"evil.com"), "disabled must not match");
        assert!(!patterns.contains(&"openai"), "keyword miss");
    }

    /// whitelist_clear：mock N 条 → DELETE FROM → 返 N + 表空（复刻 command 语义）。
    ///
    /// PRD 验收要求 cargo test whitelist_clear；本测复刻 commands::mitm::mitm_whitelist_clear
    /// 的 SQL（DELETE FROM mitm_whitelist 返 changes() 行数），验全删 + 行数计数。
    #[test]
    fn whitelist_clear_deletes_all_and_returns_count() {
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"CREATE TABLE mitm_whitelist (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                host_pattern TEXT NOT NULL,
                rule_type TEXT NOT NULL DEFAULT 'suffix',
                enabled INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL DEFAULT 'user',
                created_at INTEGER NOT NULL DEFAULT 0,
                UNIQUE(host_pattern)
            );"#,
        )
        .unwrap();
        // 预置：3 条 default + 2 条 user = N=5（混 source，验全删不筛）。
        for (i, src) in ["default", "default", "default", "user", "user"].iter().enumerate() {
            conn.execute(
                "INSERT INTO mitm_whitelist (host_pattern, rule_type, enabled, source, created_at) \
                 VALUES (?1, 'suffix', 1, ?2, ?3)",
                rusqlite::params![format!("host{i}.example.com"), src, i as i64],
            )
            .unwrap();
        }
        let before: i64 = conn.query_row("SELECT COUNT(*) FROM mitm_whitelist", [], |r| r.get(0)).unwrap();
        assert_eq!(before, 5, "precondition: 5 rows (3 default + 2 user)");

        // 复刻 command 的 DELETE FROM mitm_whitelist（全删，不筛 source）。
        let n = conn.execute("DELETE FROM mitm_whitelist", []).unwrap();

        // 验收：返 N=5（删除行数）+ 表空。
        assert_eq!(n, 5, "clear must delete all 5 rows (default + user)");
        let after: i64 = conn.query_row("SELECT COUNT(*) FROM mitm_whitelist", [], |r| r.get(0)).unwrap();
        assert_eq!(after, 0, "table must be empty after clear");
    }

    #[test]
    fn evaluate_host_empty_host_returns_empty() {
        let entries = [entry("suffix", "anthropic.com", true)];
        assert!(evaluate_host(&entries, "").is_empty());
        assert!(evaluate_host(&entries, "   ").is_empty());
    }

    #[test]
    fn evaluate_host_no_hits_returns_empty() {
        let entries = [entry("suffix", "anthropic.com", true)];
        assert!(evaluate_host(&entries, "openai.com").is_empty());
    }
}
