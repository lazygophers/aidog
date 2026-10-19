---
name: price-source-update
description: |
  更新 aidog 模型定价信源（registry 人工维护 + app 远端拉取）。固化 aidog_db::resolve_price 回退链（ADR 0006，禁自查表硬编码默认价）、context_tiers 数据驱动选档、price_sync::sync_registry 拉取链路（jsDelivr 主源 + raw 兜底 + index 驱动）、registry 数据三件套（platform.json + models/*.json + index.json）、bump-registry-last-updated 盖戳与 check-registry.mjs 门禁。触发词：更新价格、定价信源、加模型定价、context_tiers、阶梯计费、price_sync、模型价格月级腐化、新平台定价、registry 同步、max_input_tokens、max_output_tokens、context_window。
when_to_use: registry 模型价/参数过期需刷新；加新平台/新模型需接定价；改 context_tiers 阶梯计费；price_sync 拉取链路出问题；resolve_price 回退不到价
---

# aidog 定价信源更新

更新 aidog 模型定价 + max_tokens/context_window + capabilities + family/version 等必补字段。本 skill 覆盖**信源维护 / 阶梯计费 / 拉取链路 / resolve_price 回退**全流程，把架构铁律（resolve_price 不改、信源单一）前置。

> 行号漂移，定位以**文件名 / 符号名**为准。

---

## 0. 架构铁律（动手前必读）

1. **est_cost 必走 `aidog_db::resolve_price` 回退链，禁自查表绕过默认价。** 所有成本估算经 `resolve_price`（位于 `src-tauri/crates/aidog_db/src/registry.rs`，ADR 0006），回退顺序固定不可换：① 命中 preset `peak` 窗口且条目 `price.peak` 存在 → 用模型 **peak 绝对价**（`PriceResolution::peak_applied=true`，调用方**不得再乘平台倍率**）→ ② 否则条目默认价 × 平台 `peak` 倍率 → ③ 条目缺失 → `PriceSyncSettings` 的 fallback 单价（默认 3.0 $/M，不返 0）。加价/改价走 registry 同步进 `model_entry` 表，**别在估算代码里自查表硬编码默认价**。

2. **信源 = `src-tauri/defaults/registry/`（人工维护，push GitHub master）+ app 远端拉取。** 真值源是 registry 目录三件套：`index.json`（平台清单 + 全局 `last_updated`）/ `platforms/<code>/platform.json`（端点 + 模型清单 + peak + key_prefixes）/ `platforms/<code>/models/<model>.json`（per-platform 模型条目，含 `price` 子树与 `max_input_tokens`/`max_output_tokens`/`context_window`）。开发者手编 push master，app 运行期 `gateway/price_sync.rs::sync_registry` 定时从 jsDelivr 主源 + raw.githubusercontent 兜底，**index 驱动 + best-effort 逐文件**（单文件两源全败 → 记进 `failures` 清单并**保留 DB 旧行**）。

3. **模型名月级腐化必靠 fetchModels 兜底，改默认价须 WebSearch 核官方。** 平台默认模型 `getDefaultModels`（前端 `Platforms.tsx`）是前端预设，模型名月级腐化，运行期靠 `fetchModels` 兜底。改某模型默认价/型号前 **WebSearch 核官方当前页**，禁凭记忆。

---

## 1. 任务分类

### 任务 A：刷新过期价（最常见）

模型价格月级腐化 → 手编 registry JSON 后 push master，app 下次同步拉取，不手改 DB 单值。

1. 定位目标文件：`src-tauri/defaults/registry/platforms/<code>/models/<model>.json`。
2. 编辑 `price` 子树（数值字段 `$/token`，禁字符串表达式）：
   - `input` 必填 + `output` 必填 + `cache_read` / `cache_write` 可选 + `peak` 绝对价可选 + `free` 显式免费标记（仅官方来源才标 `true`）+ `context_tiers` / `time_tiers` 可选。
3. 同步 `max_input_tokens` / `max_output_tokens` / `context_window`（同源官方页）。
4. **不重写其它平台同名条目的价**（同一 `canonical_model` 跨平台聚合，每平台独立定价）。
5. push master（jsDelivr / raw CDN 会同步）。
6. 跑盖戳脚本：`node scripts/bump-registry-last-updated.mjs`（无参=只盖 git 变更文件）。
7. 跑门禁：`node scripts/check-registry.mjs`（schema 校验 + 覆盖率统计 + 数值漂移）。
8. 验证：app 内「立即同步」触发 `sync_registry`；DB `model_entry` 表最新 `last_updated` 已更新。

🔴 **CHECKPOINT**：未跑 `check-registry.mjs` 绿之前不 commit。

### 任务 B：加新平台定价

走 skill `aidog-add-platform` 完整流程（含 Protocol 枚举 Rust↔TS 双写、registry JSON 三件套、可选 quota_scripts）。本 skill 不重复该流程。

判定走 aidog-add-platform：需要新增 platforms/<code>/ 目录、注册 Protocol 变体、改 index.json 平台清单。

### 任务 C：加新模型到已存在平台

1. 落 `src-tauri/defaults/registry/platforms/<code>/models/<new_model>.json`，字段对齐 schema/model.schema.json 必填：`last_updated` / `model_id` / `canonical_model` / `capabilities` / `official` / `supported_protocols` / `price`。
2. **同步更新 `platform.json` 的 `models.default` 数组**（加入新 model_id）与 `model_list.default`（下拉冷启动兜底列全候选），**不变量**：`model_list.default` 必须是 `models` 各分支（default/peak）值集的超集。
3. 同步更新 `index.json`（如新模型触达 `pricing_only` 豁免，已于 2026-08-31 起为空，本任务场景不涉及）。
4. 盖戳 + 门禁 + commit（同 A 步骤 6-8）。

🔴 **CHECKPOINT**：漏更新 `platform.json` 的 `models` 数组会触发 check-registry.mjs 报「platform 引用了无文件的 model id」warning（AIDOG_REGISTRY_STRICT=1 时升级为 failure）。

### 任务 D：阶梯计费（context_tiers）

`context_tiers`：上下文阶梯计费 = `context_tiers` 档位数据 + `resolve_price` 按 `input_tokens` 选档。改阶梯：

1. 在 `models/<model>.json` 加 `price.context_tiers`（档位 token 阈值 + 倍率；档内可嵌 `time_tiers`）。
2. **`resolve_price` 选档逻辑不动**（已实现，改 tiers 数据即可）。
3. 验证：cargo test 跑 `aidog_db::resolve_price` 相关测试（长上下文请求命中正确档位价）。

🛑 **STOP**：改 `aidog_db::resolve_price` 函数体 = 触碰铁律 1，本 skill 流程不覆盖。走正常代码改动走 general 工作流。

### 任务 E：补全必补字段（family / version / capabilities / context_window / display_name 等）

`check-registry.mjs` 覆盖率门禁：AIDOG_REGISTRY_COVERAGE_MIN=90 表示四必补字段（family / version / capabilities / context_window）覆盖率全部 ≥90% 才算过。补数流程：

1. 跑 `node scripts/check-registry.mjs` 看「平台缺失排名（必补字段缺失格子数，降序）」输出。
2. 选平台（按缺失数降序）。
3. 一手源补值：上游厂商 model 详情页（OpenAI platform.openai.com/docs/models、Anthropic docs.anthropic.com、Vertex AI 等）。**webReader 摘要 = 假设不是事实**，落库前必须有 raw 一手（curl 原文）或双源对照。
4. 落 `models/<model>.json` 的对应字段：
   - `family` (string) / `version` (string) / `capabilities` (array) / `context_window` (number)
   - bonus：`display_name` / `thinking_supported` / `thinking_toggleable` / `predecessor`
5. agents 取证 + 报告，**主会话落库**（shared-worktree r102 教训：agent 写数据须主会话采信）。
6. 盖戳 + 门禁 + commit（每平台独立：`git commit -- platforms/<code>/`）。

🔴 **CHECKPOINT**：补数前用 `node -e` 跑预检确认目标键不存在/不重复（registry-data-edit.md 编辑脚本先预检条）。

---

## 2. 拉取链路（price_sync::sync_registry）

- `src-tauri/src/gateway/price_sync.rs::sync_registry`（command 名仍是 `model_price_sync`，理由见该文件顶部注释）：app 定时拉取信源更新 `model_entry` 表。
- **jsDelivr 主源 + raw.githubusercontent 兜底**，**index 驱动**（先拉 `index.json`，失败即整轮放弃）+ **best-effort 逐文件**（单文件两源全败 → 记进 `failures` 清单并**保留 DB 旧行**，不清空不部分覆盖），16 路并发。
- 拉取出站 HTTP **落 proxy_log** 排除项：price_sync 是唯一不出站落 proxy_log 的对平台 HTTP（memory `platform-egress-http-logging`）。
- 拉取失败 → resolve_price 回退到库里上次成功值，不炸。

调试拉取链路：看 price_sync 日志 + `model_entry` 表最新 `updated_at`。

🛑 **STOP**：手动改 DB `model_entry` 单行价格 = 绕开拉取链路，下次同步会被覆盖。改价走 registry JSON 路径。

---

## 3. 验证门禁

```bash
# 1. registry schema + 覆盖率 + 数值漂移门禁
node scripts/check-registry.mjs
# 期望: "registry schema 校验通过：N 个文件"
# AIDOG_REGISTRY_COVERAGE_MIN=90 时必补字段覆盖率全部 ≥90%

# 2. 盖戳脚本（无参 = 只盖 git 变更文件；--all = 全量）
node scripts/bump-registry-last-updated.mjs
# 期望: "N 个文件已盖戳"

# 3. Rust 编译（改了 registry.rs / price_sync.rs / schema 字段定义）
cd src-tauri && cargo build && cargo clippy   # warning 必须清

# 4. resolve_price 回退测试（含 context_tiers 选档）
cd src-tauri && cargo test --package aidog_db
# 关键测试: test_registry.rs::every_registry_platform_code_parses_as_protocol
# + resolve_price 选档用例

# 5. 抽样验证（手测）：对目标模型跑请求看 est_cost 落库值
# log 表 est_cost 字段、proxy_log 的 est_cost 单列
```

收尾自检：
- [ ] 没在估算代码里硬编码默认价（走 `aidog_db::resolve_price`）。
- [ ] 模型名经 WebSearch 核官方，非凭记忆。
- [ ] context_tiers 改的是数据，没动 `resolve_price` 选档逻辑。
- [ ] registry JSON push master 后 `sync_registry` 成功（看 `model_entry.updated_at`）。
- [ ] `check-registry.mjs` 绿且覆盖率 ≥ 门禁。
- [ ] commit 走 `git commit -- <明确路径>`（不 `-a`，共享 checkout 防误扫，shared-worktree.md）。

---

## 反例黑名单（不要做）

1. ❌ 在 `estimate.rs` / `gateway/db.rs` 自查表硬编码默认价 —— 走 `aidog_db::resolve_price` 回退链。
2. ❌ 凭记忆改模型名/默认价 —— 月级腐化，WebSearch 核官方，webReader 摘要是假设不是事实。
3. ❌ 改 `aidog_db::resolve_price` 函数体选档逻辑 —— 只改 `price.context_tiers` 数据。
4. ❌ 把 `price_sync` 出站落 `proxy_log` —— 它是唯一排除项。
5. ❌ 改价后不跑 `check-registry.mjs` 与 `cargo test` —— 门禁可能断。
6. ❌ 手动改 DB `model_entry` 单行价 —— 会被下次 `sync_registry` 覆盖。
7. ❌ 跨平台合并改价 —— 同一 `canonical_model` 跨平台聚合，每平台独立定价。
8. ❌ `git commit -a` / 不带路径 commit —— 共享 checkout 误扫他人改动（shared-worktree.md 硬规则 1）。
9. ❌ 重写已有字段值（即使觉得错也只标「存疑：当前 X，建议 Y」，主会话复核）—— 取证型 agent 不写数据（shared-worktree r102 教训）。
10. ❌ 编辑 registry JSON 时用 round-trip JSON.stringify —— 科学计数法漂移（check-registry.mjs 数值指纹检测会判失败）。文本级插入/替换。

---

## 相关

- 架构 ADR：`docs/adr/0005-registry-as-db-mirror.md`、`docs/adr/0006-resolve-price-fallback.md`
- 单源约定：memory `pricing-github-single-source`、`pricing-resolve-single-source`
- 出站日志例外：memory `platform-egress-http-logging`
- 默认模型腐化：memory `platform-default-model`
- 持久化：memory `est-cost-persistence`
- 阶梯计费：memory `openai-pricing-scraper-tiered`
- 拉取链路：`src-tauri/src/gateway/price_sync.rs`（command `model_price_sync`）
- 回退链：`src-tauri/crates/aidog_db/src/registry.rs`（`resolve_price`）
- 平台新增：skill `aidog-add-platform`（含 Protocol 枚举 Rust↔TS 双写）
- 门禁与盖戳：`scripts/check-registry.mjs`、`scripts/bump-registry-last-updated.mjs`
- 关键纪律：`.claude/rules/registry-data-edit.md`（预检 + 数值漂移 + 模型清单同步）、`.claude/rules/shared-worktree.md`（commit 不带 `-a` + 共享 checkout 提交纪律）
- 主要涉及文件：
  - `src-tauri/defaults/registry/index.json`
  - `src-tauri/defaults/registry/platforms/<code>/platform.json`
  - `src-tauri/defaults/registry/platforms/<code>/models/<model>.json`
  - `src-tauri/defaults/registry/schema/{model,platform,index}.schema.json`
