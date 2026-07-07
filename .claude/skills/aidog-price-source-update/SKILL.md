---
name: aidog-price-source-update
description: |
  更新 aidog 模型定价信源（models.json 人工维护 + app 远端拉取 + 模型 max_tokens）。固化 resolve_price 回退链（不改估算代码里默认价）、上下文阶梯定价（context_tiers + resolve_price 按 input_tokens 选档）、出站 cap、app 定时拉取（price_sync.rs）。触发词：更新价格、定价信源、加模型定价、max_tokens、context_tiers、阶梯计费、price_sync、模型价格月级腐化、新平台定价。
when_to_use: 模型价格过期需刷新（月级腐化）；加新平台/新模型需接定价；要改 context_tiers 阶梯计费；price_sync 拉取链路出问题；resolve_price 回退不到价
disable-model-invocation: true
paths:
  - src-tauri/defaults/models.json
  - src-tauri/src/gateway/price_sync.rs
  - src-tauri/src/gateway/estimate.rs
  - src-tauri/src/gateway/db.rs
---

# aidog 定价信源更新

更新 aidog 模型定价 + max_tokens。本 skill 覆盖**信源维护 / 阶梯计费 / 拉取链路 / resolve_price 回退**全流程，把架构铁律（resolve_price 不改、信源单一）前置。

> 行号漂移，定位以**文件名 / 符号名**为准。

---

## 0. 架构铁律（动手前必读）

1. **est_cost 必走 resolve_price 回退链，禁自查表绕过默认价。** 所有成本估算经 `resolve_price`（`src-tauri/src/gateway/db.rs`），回退链：`pricing[platform_type]` → 顶层 → `default_platform` → fallback。加价/改价走信源同步进 `model_price` 表，**别在估算代码里自查表硬编码默认价**（memory `pricing-resolve-single-source` / `est-cost-persistence`）。

2. **信源 = models.json（人工维护，push GitHub master）+ app 远端拉取 + 出站 cap。** memory `pricing-github-single-source`：定价 + max_tokens 唯一信源是 `src-tauri/defaults/models.json`，开发者手编 push master，app 运行期 price_sync.rs 定时从 jsDelivr / raw 拉取，对平台出站 HTTP 有 cap。

3. **模型名月级腐化必靠 fetchModels 兜底，改默认价须 WebSearch 核官方。** memory `platform-default-model`：平台默认模型 `getDefaultModels`（前端 `Platforms.tsx`）是前端预设，模型名月级腐化，运行期靠 `fetchModels` 兜底。改某模型默认价/型号前 **WebSearch 核官方当前页**，禁凭记忆。

---

## 1. 任务分类

### 任务 A：刷新过期价（最常见）

模型价格月级腐化 → 手编 models.json 后 push master，app 下次同步拉取，不手改 DB 单值。

1. 编辑 `src-tauri/defaults/models.json`（顶层 models 对象，每项含 input/output/cache_read pricing + max_tokens/context_window + 可选 context_tiers）。
2. push master（jsDelivr / raw CDN 会同步）。
3. 验证：对几个模型 `resolve_price` 看回退是否命中新价；app 内「立即同步」触发 price_sync.rs。

### 任务 B：加新平台定价

- 模型名进 models.json 顶层 models 对象的对应条目（input/output/cache 价 + max_tokens）。
- 模型名不确定 → WebSearch 核官方（§0-3），禁凭记忆。

### 任务 C：加新模型定价

- 模型名进 models.json 的对应条目 pricing。
- max_tokens / context_window 同源更新。

### 任务 D：阶梯计费（context_tiers）

memory `openai-pricing-scraper-tiered`：上下文阶梯计费 = `context_tiers`（长档倍率推导）+ `resolve_price` 按 `input_tokens` 选档。改阶梯：

1. 在 models.json 对应条目加 `context_tiers`（档位 token 阈值 + 倍率）。
2. resolve_price 按 input_tokens 选档（已实现，改 tiers 数据即可，**不动 resolve_price 选档逻辑**）。
3. 验证：长上下文请求命中正确档位价。

---

## 2. 拉取链路（price_sync.rs）

- `src-tauri/src/gateway/price_sync.rs`：app 定时拉取信源更新 `model_price` 表。
- 拉取出站 HTTP **落 proxy_log**（memory `platform-egress-http-logging`：所有对平台出站 HTTP 落 proxy_log，**price_sync 排除**——这是唯一例外）。
- 拉取失败 → resolve_price 回退到库里上次成功值，不炸。

调试拉取链路：看 price_sync 日志 + model_price 表最新 updated_at。

---

## 3. 验证门禁

```bash
# 1. Rust 编译（改了 estimate.rs / db.rs / price_sync.rs / models.json schema）
cd src-tauri && cargo build && cargo clippy   # warning 必须清

# 2. resolve_price 回退测试
cd src-tauri && cargo test                      # estimate/db 有 resolve_price 相关 #[test]

# 3. 抽样验证（手测）
# 对目标模型跑请求看 est_cost 落库值是否合理（log est_cost 字段）
```

收尾自检：
- [ ] 没在估算代码里硬编码默认价（走 resolve_price）。
- [ ] 模型名经 WebSearch 核官方，非凭记忆。
- [ ] context_tiers 改的是数据，没动 resolve_price 选档逻辑。
- [ ] models.json push master 后 app 同步成功（看 model_price updated_at）。

---

## 反例黑名单（不要做）

1. ❌ 在 estimate.rs 自查表硬编码默认价 —— 走 resolve_price 回退链。
2. ❌ 凭记忆改模型名/默认价 —— 月级腐化，WebSearch 核官方。
3. ❌ 改 context_tiers 选档逻辑 —— 只改 tiers 数据。
4. ❌ 把 price_sync 出站落 proxy_log —— 它是唯一排除项。
5. ❌ 改价后不跑 resolve_price 相关 cargo test —— 回退链可能断。

## 相关

- 单源约定：memory `pricing-github-single-source`、`pricing-resolve-single-source`
- 出站日志例外：memory `platform-egress-http-logging`
- 默认模型腐化：memory `platform-default-model`
- 持久化：memory `est-cost-persistence`
- 阶梯计费：memory `openai-pricing-scraper-tiered`
- 脚本：`src-tauri/src/gateway/price_sync.rs`、`estimate.rs`、`db.rs`（resolve_price）
