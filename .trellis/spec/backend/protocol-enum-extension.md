---
updated: 2026-07-10
rewrite-version: 1
authored-by: trellisx-spec
mode: sediment
---

# Protocol 枚举变体扩展范式

何时被读: 新增 `Protocol` 枚举变体时（新协议 / 新 cp 变体 / 新平台子类型）
谁读: trellis-implement sub-agent / main
不遵守的代价: 过度实现 —— 预设「全链复制专属分支」改 router/converter/quota/estimate/price_sync，实际这些层走 `_` 兜底无需动，引入无意义 diff + 回归风险

---

## 新增变体 MUST 先 grep 同构变体命中点 (MUST)

新增 `Protocol` 变体前，**MUST** grep 现有同构变体全链命中点，据实际命中分类决定改动面，禁预设「全链复制专属分支」。

- **cp 类变体**（如 KimiCoding/QianfanCoding/XiaomiMimoCoding）：grep 模板 `GlmCoding` / `glm_coding`
- **普通协议变体**：grep 任一现有普通变体（如 `OpenAI` / `openai`）

## 命中点 3 类分类（据实判定改动面）

grep 同构变体命中点，按下列 3 类分类，**仅第 1 类必须改**：

1. **enum 定义 + serde rename（必改）**：`models/protocol.rs` Protocol enum 加变体，`#[serde(rename = "<key>")]` 与 `platform-presets.json` key 一致
2. **字符串 JSON 查询（按需改）**：如 `coding_plan.rs::default_is_coding_plan` 字符串查询（非枚举 match）—— 若新变体属 cp 类，JSON 查询需覆盖新 key；若非 cp 类，跳过
3. **历史 migration 测试名（禁改）**：如 `schema_late.rs` `migrations_late_*_backfill_*` 测试名含旧变体字面量，是历史记录，禁改

## 零专属 match 臂 → 加枚举即覆盖 (MUST)

**反直觉发现**：`router.rs` / `adapter/converter.rs` / `quota.rs` / `estimate.rs` / `price_sync.rs` **零** `match Protocol::<变体>` 专属分支 —— 全走 `_` 兜底通用 OpenAI 兼容路径。

- 若 grep 同构变体在这些层**零专属 match 臂** → 新变体加枚举即自动覆盖全链，**禁** 预设「全链复制专属分支」改这些文件
- 若 grep 发现有专属 match 臂 → 新变体同模式加分支（按命中点实际覆盖，禁臆造）

## serde round-trip + JSON key 对齐 (MUST)

- `#[serde(rename = "<key>")]` 与 `platform-presets.json` protocols 表 key **一一对应**
- 变体 research 证伪独立协议（如 minimax/minimax_en Token Plan 共用 endpoint）→ **禁** 加变体（JSON 不落 key，Rust 枚举不跟）
- round-trip 测试覆盖：JSON key → 枚举 → JSON key 无损

## 验收断言（可复用）

```bash
# 新变体字面量全链命中点清单（据分类决定改动面）
grep -rn '<NewVariant>\|<new_key>' src-tauri/src

# 同构变体（cp 类用 glm_coding）专属 match 臂数 —— 验零专属臂则加枚举即覆盖
grep -rn 'match Protocol::GlmCoding\|Protocol::GlmCoding =>' src-tauri/src/{router.rs,adapter,quota.rs,estimate.rs,price_sync.rs}  # 0 = 加枚举即覆盖

# serde round-trip 测试
cargo test --lib protocol_coding_variants  # PASS

# baseline 不回归
cargo test --lib | grep passed  # >= baseline
```

## 实例

task 07-10-protocols-rust-enum：+3 cp 变体（KimiCoding/QianfanCoding/XiaomiMimoCoding）。grep GlmCoding 命中点 3 类（enum 定义 + coding_plan 字符串查询 + schema_late migration 测试名），router/converter/quota/estimate/price_sync 零专属 match 臂 → 加枚举即覆盖，无需改这些层。minimax/minimax_en research 证伪（Token Plan 共用 endpoint 无 `/coding/`）禁加变体。

## Cross-reference

- research 结论：`.trellis/tasks/archive/2026-07/07-10-protocols-json-schema/prd.md`（3 YES / 2 NO）
- JSON 真值源：`src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆盖，见 project CLAUDE.md）
