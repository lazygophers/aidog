# mitm-whitelist-clash-rules — MITM 白名单 Clash 规则集多类型支持 + 默认扩充

- **Status**: planning
- **Source**: session:claude_96a0dd46-757b-40db-a9f7-4555767078d3
- **Spec**: `.trellis/spec/backend/proxy-connect-relay.md`（MITM 白名单段）

---

## 背景

用户要求 MITM 解密白名单 default 至少包含完整 Clash 规则集（Claude 3 + OpenAI 34，来源 blackmatrix7/ios_rule_script），覆盖 DOMAIN / DOMAIN-SUFFIX / DOMAIN-KEYWORD / IP-CIDR 四类。

## 用户决策（已答）

- **落点**：MITM 解密白名单 default（schema_late.rs `DEFAULT_AI_HOSTS` 扩充）
- **规则类型**：domain / suffix / keyword / **ipcidr**（仅匹配 IP 字面 CONNECT 目标）
- **IP-ASN**：**不支持**（舍弃，不植）
- **GeoIP/DNS**：**不要**（IP-CIDR 不解析 host→IP，仅匹配 CONNECT 目标里已是 IP 字面的情况）

## 现状（实证）

- `schema_late.rs:452` `DEFAULT_AI_HOSTS = ["*.anthropic.com", "*.openai.com"]` — 仅 2 条 suffix 通配
- `whitelist.rs` 匹配引擎只支持 host pattern（suffix `*.domain` + 精确 + 大小写不敏感），无 keyword / ipcidr
- MITM 白名单在 CONNECT 隧道建立**前**匹配（`matches_db`），此时拿到 CONNECT 目标的 host 段（可能是域名 `api.openai.com` 或 IP 字面 `24.199.123.28`）
- `mitm_whitelist` 表字段：host_pattern / enabled / source / created_at（无 rule_type 列）

## 目标规则集（37 条）

### Claude（3 条）
| 类型 | 值 |
|---|---|
| domain | cdn.usefathom.com |
| suffix | anthropic.com |
| suffix | claude.ai |

### OpenAI（34 条）
| 类型 | 数量 | 值 |
|---|---|---|
| domain | 7 | browser-intake-datadoghq.com / chat.openai.com.cdn.cloudflare.net / openai-api.arkoselabs.com / openaicom-api-bdcpf8c6d2e9atf6.z01.azurefd.net / openaicomproductionae4b.blob.core.windows.net / production-openaicom-storage.azureedge.net / static.cloudflareinsights.com |
| suffix | 24 | ai.com / algolia.net / api.statsig.com / auth0.com / chatgpt.com / chatgpt.livekit.cloud / client-api.arkoselabs.com / events.statsigapi.net / featuregates.org / host.livekit.cloud / identrust.com / intercom.io / intercomcdn.com / launchdarkly.com / oaistatic.com / oaiusercontent.com / observeit.net / openai.com / openaiapi-site.azureedge.net / openaicom.imgix.net / segment.io / sentry.io / stripe.com / turn.livekit.cloud |
| keyword | 1 | openai |
| ipcidr | 2 | 24.199.123.28/32 / 64.23.132.171/32 |

**舍弃**：IP-ASN 20473（不支持）。

## 方案

### 1. rule_type 模型（schema Migration）
`mitm_whitelist` 加 `rule_type` 列（domain/suffix/keyword/ipcidr），复用 host_pattern 存规则值（语义泛化，不改列名避迁移成本）。

```sql
-- Migration 0NN
ALTER TABLE mitm_whitelist ADD COLUMN rule_type TEXT NOT NULL DEFAULT 'suffix';
-- 回填存量：*.x → suffix（去 *．前缀的值已是 suffix 语义），无 *．前缀 → domain
UPDATE mitm_whitelist SET rule_type='suffix' WHERE host_pattern LIKE '*.%';
UPDATE mitm_whitelist SET rule_type='domain' WHERE host_pattern NOT LIKE '*.%';
```

存量 default（`*.anthropic.com` / `*.openai.com`）回填为 suffix。

### 2. 匹配引擎扩展（whitelist.rs）
`matches_host` 内部按 rule_type 分派：

```rust
fn matches_rule(host: &str, rule_type: &str, rule_value: &str) -> bool {
    match rule_type {
        "domain" => host.eq_ignore_ascii_case(rule_value),
        "suffix" => matches_suffix(host, rule_value),  // Clash DOMAIN-SUFFIX 标准：裸域 + 所有子域（语义扩展，含裸域；现状 *.domain 仅子域）
        "keyword" => host.to_lowercase().contains(rule_value),  // 大小写不敏感子串
        "ipcidr" => {
            // 仅当 CONNECT 目标 host 段本身是 IP 字面才匹配（不解析域名→IP）
            let ip: std::net::IpAddr = match host.parse() { Ok(ip) => ip, Err(_) => return false };
            let net: ipnet::IpNet = match rule_value.parse() { Ok(n) => n, Err(_) => return false };
            net.contains(&ip)
        }
        _ => false,
    }
}
```

`matches_host` 遍历 entries 时按各自 rule_type 调 matches_rule。host 是 IP 字面 → domain/suffix/keyword 自动 miss（IpAddr 不含点号子串语义），ipcidr 命中；host 是域名 → ipcidr 自动 miss，host 类规则命中。

### 3. seed_default_whitelist 扩充（schema_late.rs）
`DEFAULT_AI_HOSTS` 升级为结构化规则集常量：

```rust
const DEFAULT_RULES: &[(&str, &str)] = &[
    // Claude
    ("domain", "cdn.usefathom.com"),
    ("suffix", "anthropic.com"),
    ("suffix", "claude.ai"),
    // OpenAI domain ×7
    ("domain", "browser-intake-datadoghq.com"),
    // ... 7 条
    // OpenAI suffix ×24
    ("suffix", "ai.com"),
    // ... 24 条
    // OpenAI keyword
    ("keyword", "openai"),
    // OpenAI ipcidr ×2
    ("ipcidr", "24.199.123.28/32"),
    ("ipcidr", "64.23.132.171/32"),
];
```

seed 植入时 source='default' + rule_type 字段。

## 改动文件

| 文件 | 改动 |
|---|---|
| `src-tauri/src/gateway/mitm/whitelist.rs` | WhitelistEntry 加 rule_type 字段；matches_host 按 rule_type 分派 4 类型；现有 `*.domain` 测试不回归 |
| `src-tauri/src/gateway/db/schema_late.rs` | mitm_whitelist 加 rule_type 列 + Migration 回填 + DEFAULT_RULES 扩 37 条 + seed_default_whitelist_if_empty 改植结构化规则 |
| `src-tauri/Cargo.toml` | 加 `ipnet` 依赖（轻量纯 Rust，IPv4+IPv6 CIDR） |
| `src-tauri/src/commands/mitm.rs` | WhitelistEntryDto 加 rule_type 字段序列化 |
| `src/components/settings/MitmConfig.tsx` | 白名单 UI 展示 rule_type 标签（domain/suffix/keyword/ipcidr） |
| `src/services/api/mitm.ts`（主区 `api.ts`） | WhitelistEntry TS 类型加 rule_type |
| `src/locales/*.json` ×8 | rule_type 标签文案（domain→域名/suffix→后缀/keyword→关键字/ipcidr→IP 段） |

## 验证

```bash
cd src-tauri
cargo test whitelist -- --nocapture  # 4 类型匹配引擎单测 + 现有 *.domain 不回归
cargo test seed_default_whitelist -- --nocapture  # 37 条 seed 植入 + rule_type 字段
cargo test connect -- --nocapture  # CONNECT host=IP 字面命中 ipcidr；host=域名命中 suffix
cargo clippy --all-targets --no-deps
cargo build
yarn build && node scripts/check-i18n.mjs
# 人工：MITM 启用 → 白名单展示 37 条 default（含 rule_type 标签）→ CONNECT api.openai.com 命中 suffix / CONNECT 24.199.123.28:443 命中 ipcidr
```

## 不做

- IP-ASN（用户明确不支持）
- GeoIP / DNS 解析（用户明确不要；ipcidr 仅匹配 CONNECT 目标 IP 字面）
- 用户自定义规则的 rule_type UI 选择器（默认规则集为主，用户增删仍走现有 host 输入，默认 rule_type=suffix）

## 调度

单 task，串行（schema Migration → 匹配引擎 → seed → DTO/UI → locale 链式）。与 `07-03-mitm-ca-elevated-install` 文件集不相交，可并行（各 worktree）。
