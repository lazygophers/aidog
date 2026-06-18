---
name: aidog-price-source-update
description: |
  更新 aidog 模型定价信源（first-party scraper / LiteLLM / OpenRouter 三层 + 模型 max_tokens）。固化三层信源优先级架构（REGISTRY 顺序 + _ensure_platform_pricing 首次非空，不改 resolve_price）、first-party scraper 覆盖清单（deepseek/openai/anthropic/gemini/glm/kimi/minimax/xiaomi_mimo）、新平台注册 REGISTRY（xiaomi_mimo 需手注册）、上下文阶梯定价（context_tiers + resolve_price 按 input_tokens 选档）、出站 cap、app 定时拉取（price_sync.rs）。触发词：更新价格、定价信源、加模型定价、max_tokens、first-party scraper、LiteLLM、OpenRouter、context_tiers、阶梯计费、price_sync、模型价格月级腐化、新平台定价。
when_to_use: 模型价格过期需刷新（月级腐化）；加新平台/新模型需接定价；要写 first-party scraper 抓一手价；要改 context_tiers 阶梯计费；price_sync 拉取链路出问题；resolve_price 回退不到价
disable-model-invocation: true
paths:
  - scripts/pricing/**
  - src-tauri/src/gateway/price_sync.rs
  - src-tauri/src/gateway/estimate.rs
  - src-tauri/src/gateway/db.rs
---

# aidog 定价信源更新

更新 aidog 模型定价 + max_tokens。本 skill 给**三层信源架构 / 新平台接入 / scraper 编写 / 阶梯计费 / 拉取链路**全流程，把架构铁律（resolve_price 不改、REGISTRY 顺序定优先级、first-party 覆盖清单）前置。

> 行号漂移，定位以**文件名 / 符号名**为准。

---

## 0. 架构铁律（动手前必读）

1. **est_cost 必走 resolve_price 回退链，禁自查表绕过默认价。** 所有成本估算经 `resolve_price`（`src-tauri/src/gateway/db.rs`），回退链：`pricing[platform_type]` → 顶层 → `default_platform` → fallback。加价/改价走信源同步进 `model_price` 表，**别在估算代码里自查表硬编码默认价**（memory `pricing-resolve-single-source` / `est-cost-persistence`）。

2. **三层信源优先级靠 REGISTRY 顺序，不改 resolve_price。** first-party scraper > LiteLLM > OpenRouter。优先级在 REGISTRY 数组顺序 + `_ensure_platform_pricing` 取首个非空实现，**不在 resolve_price 里写 if 优先级**（memory `pricing-tiered-source-architecture`）。

3. **信源 = GitHub JSON（LiteLLM）+ Python 聚合 + Makefile 单入口 + app 定时拉取 + 出站 cap。** memory `pricing-github-single-source`：定价 + max_tokens 信源是 LiteLLM→GitHub JSON，Python 脚本聚合，Makefile 单入口，app 定时拉取，对平台出站 HTTP 有 cap。

4. **first-party scraper 已覆盖清单**（memory `first-party-scraper-coverage`）：deepseek / openai / anthropic / gemini / glm / kimi / minimax / xiaomi_mimo。**xiaomi_mimo 需手动注册 REGISTRY**（OR PREFIX_MAP 无 mimo 映射，不注册抓不到）。

5. **模型名月级腐化必靠 fetchModels 兜底，改默认价须 WebSearch 核官方。** memory `platform-default-model`：平台默认模型 `getDefaultModels`（前端 `Platforms.tsx`）是前端预设，模型名月级腐化，运行期靠 `fetchModels` 兜底。改某模型默认价/型号前 **WebSearch 核官方当前页**，禁凭记忆。

---

## 1. 三层信源架构

| 层 | 信源 | 形态 | 何时用 |
|---|---|---|---|
| L1 first-party | 平台官方定价页 scraper（`scripts/pricing/`） | Astro JSON / HTML / API island live fetch | 一手价最准，优先 |
| L2 LiteLLM | GitHub JSON（litellm model_prices） | Python 聚合脚本拉 | L1 无覆盖时兜底 |
| L3 OpenRouter | OpenRouter API | prefix→platform_type 兜底 | L1/L2 都无 |

### 优先级机制（memory `pricing-tiered-source-architecture`）

- `REGISTRY` 数组按 [first-party, litellm, openrouter] 顺序排列。
- `_ensure_platform_pricing` 依次试，**首个非空胜出**。
- LiteLLM 按 `prefix → platform_type` 映射兜底。
- 🔴 改优先级 = 调 REGISTRY 顺序，**不改 resolve_price**。

---

## 2. 任务分类

### 任务 A：刷新过期价（最常见）

模型价格月级腐化 → 跑信源聚合重新拉，不手改单值。

```bash
# Makefile 单入口跑定价聚合
make pricing           # 或 scripts/pricing/ 下对应脚本，见实际 Makefile target
```

产出更新 `model_price` 表的 price_data。验证：对几个模型 `resolve_price` 看回退是否命中新价。

### 任务 B：加新平台定价

1. **有官方定价页** → 写 first-party scraper（见 §3），注册进 REGISTRY。
2. **无官方页** → 靠 LiteLLM（在 prefix→platform_type 映射加该平台）或 OpenRouter 兜底。
3. 🔴 **xiaomi_mimo 类**：即便写了 scraper 也需手动注册 REGISTRY（PREFIX_MAP 无映射）。新平台检查 REGISTRY 注册状态。

### 任务 C：加新模型定价

- 模型名进某平台的 price_data.pricing。
- 模型名不确定 → WebSearch 核官方（§0-5），禁凭记忆。
- max_tokens 同源更新（LiteLLM JSON 含 max_tokens）。

### 任务 D：阶梯计费（context_tiers）

memory `openai-pricing-scraper-tiered`：上下文阶梯计费 = `context_tiers`（长档倍率推导）+ `resolve_price` 按 `input_tokens` 选档。改阶梯：

1. 在 price_data 加 `context_tiers`（档位 token 阈值 + 倍率）。
2. resolve_price 按 input_tokens 选档（已实现，改 tiers 数据即可，**不动 resolve_price 选档逻辑**）。
3. 验证：长上下文请求命中正确档位价。

---

## 3. first-party scraper 编写（L1）

参考已覆盖清单的 scraper（`scripts/pricing/`）。要点：

- 优先抓**官方 live JSON island**（如 OpenAI developers.openai.com 的 Astro JSON）> HTML 解析 > API。
- 输出统一格式进 price_data.pricing（input/output/cache 价 + max_tokens + 可选 context_tiers）。
- 出站 HTTP 受 app cap（memory `pricing-github-single-source`）—— scraper 跑在构建/聚合期，不在请求热路径。
- 注册：新 scraper 进 REGISTRY 数组（first-party 段）。xiaomi_mimo 类需额外注册（§2 步骤 3）。

---

## 4. 拉取链路（price_sync.rs）

- `src-tauri/src/gateway/price_sync.rs`：app 定时拉取信源更新 `model_price` 表。
- 拉取出站 HTTP **落 proxy_log**（memory `platform-egress-http-logging`：所有对平台出站 HTTP 落 proxy_log，**price_sync 排除**——这是唯一例外）。
- 拉取失败 → resolve_price 回退到库里上次成功值，不炸。

调试拉取链路：看 price_sync 日志 + model_price 表最新 updated_at。

---

## 5. 验证门禁

```bash
# 1. 跑定价聚合
make pricing                          # 或 scripts/pricing/ 对应脚本

# 2. Rust 编译（改了 estimate.rs / db.rs / price_sync.rs）
cd src-tauri && cargo build && cargo clippy   # warning 必须清

# 3. resolve_price 回退测试
cd src-tauri && cargo test                      # estimate/db 有 resolve_price 相关 #[test]

# 4. 抽样验证（手测）
# 对目标模型跑请求看 est_cost 落库值是否合理（log est_cost 字段）
```

收尾自检：
- [ ] 没在估算代码里硬编码默认价（走 resolve_price）。
- [ ] 改优先级调的是 REGISTRY 顺序，没改 resolve_price。
- [ ] 新平台已注册 REGISTRY（xiaomi_mimo 类特检）。
- [ ] 模型名经 WebSearch 核官方，非凭记忆。
- [ ] context_tiers 改的是数据，没动 resolve_price 选档逻辑。
- [ ] first-party scraper 出站不在请求热路径（聚合期跑）。

---

## 反例黑名单（不要做）

1. ❌ 在 estimate.rs 自查表硬编码默认价 —— 走 resolve_price 回退链。
2. ❌ 改 resolve_price 写 if 优先级 —— 优先级在 REGISTRY 顺序。
3. ❌ 写了 first-party scraper 不注册 REGISTRY —— 抓到也不用（xiaomo_mimo 坑）。
4. ❌ 凭记忆改模型名/默认价 —— 月级腐化，WebSearch 核官方。
5. ❌ 改 context_tiers 选档逻辑 —— 只改 tiers 数据。
6. ❌ first-party scraper 跑在请求热路径 —— 聚合期跑 + 出站 cap。
7. ❌ 把 price_sync 出站落 proxy_log —— 它是唯一排除项。
8. ❌ 改价后不跑 resolve_price 相关 cargo test —— 回退链可能断。

## 相关

- 三层架构：memory `pricing-tiered-source-architecture`
- 一手价 scraper 覆盖：memory `first-party-scraper-coverage`、`openai-pricing-scraper-tiered`
- 单源约定：memory `pricing-github-single-source`、`pricing-resolve-single-source`
- 出站日志例外：memory `platform-egress-http-logging`
- 默认模型腐化：memory `platform-default-model`
- 持久化：memory `est-cost-persistence`
- 脚本：`scripts/pricing/`、`src-tauri/src/gateway/price_sync.rs`、`estimate.rs`、`db.rs`（resolve_price）
