# Rust Protocol +3 cp 变体全链（统一 glm_coding 逻辑）

## Goal
Rust `Protocol` 枚举加 **3 变体**（KimiCoding/QianfanCoding/XiaomiMimoCoding），serde key 与 JSON（child1 加的 kimi_coding/qianfan_coding/xiaomi_mimo_coding）对齐。

**5→3 修订**（2026-07-10，implement 后据真值修）：原 prd 写 5 变体基于「5 key 已落 master」误判。实际 C1 task `07-10-protocols-json-schema` research 证伪 minimax/minimax_en 独立 cp（Token Plan 共用 endpoint，无 `/coding/` 路径，纯计费层 → ❌ NO），仅落 3 cp key（kimi/qianfan/xiaomi_mimo ✅ YES）。故 Rust 枚举同步只 +3 变体，删 MiniMaxCoding/MiniMaxEnCoding（research 证伪，独立协议不成立）。

## 实现发现（implement agent 报告，2026-07-10）
**GlmCoding 全链无专属 match 臂**：grep `GlmCoding`/`glm_coding` 仅 3 处命中 —— ① models Protocol enum 定义 ② coding_plan.rs 字符串 JSON 查询（非枚举 match）③ schema_late.rs 历史 migration 测试名。router/converter/quota/estimate/price_sync **零** `match Protocol::GlmCoding` 专属分支，全走 `_` 兜底通用 OpenAI 兼容路径。故 3 新变体加枚举即全链覆盖，无需改 router/converter/quota/estimate/price_sync。原 R2.2-R2.6「全链复制 glm_coding 分支」假设过重，实际 Rust 侧 glm_coding 本就无特殊处理。

## Requirements
### R1 枚举 + serde
- R1.1 models.rs Protocol 枚举 **+3 变体**，serde rename = `kimi_coding`/`qianfan_coding`/`xiaomi_mimo_coding`（与 JSON key 一致）。
- R1.2 ~~grep GlmCoding 全引用点加分支~~ **修订**：实际 GlmCoding 无专属 match 臂，新变体走 `_` 兜底即覆盖。仅 coding_plan.rs 字符串查询点需确认 3 新 key 覆盖（is_coding_plan JSON 查询）。

### R2 全链（修订：实际无需改）
- ~~R2.2-R2.6~~（implement 证伪：glm_coding 在 router/converter/quota/estimate/price_sync 无专属分支，新变体走 `_` 兜底通用路径即覆盖）

### R3 门禁
- R3.1 cargo build + cargo test（db/proxy/converter/router/usage_color）全过，1348 baseline 不回归。
- R3.2 cargo clippy 无新 warning。
- R3.3 3 新变体 serde round-trip 测试（JSON key ↔ 枚举）。

## Acceptance
- [ ] Protocol +3 变体（删 MiniMaxCoding/MiniMaxEnCoding），serde key 对齐 JSON
- [ ] cargo test/clippy 全绿，1351+ passed（1348 baseline + 3 serde round-trip）
- [ ] 主仓零改动

## Dependencies
depends_on: 07-10-protocols-json-schema（serde key 需 JSON 先定，已 done，3 cp key 落 master）。
