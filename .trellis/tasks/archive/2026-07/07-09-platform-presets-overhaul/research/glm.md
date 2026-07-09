# Research: glm（智谱 GLM 普通版）

- **Query**: 核对 glm 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external（官方文档）+ internal（JSON 现状）
- **Date**: 2026-07-09

## 现有 JSON 摘要（src-tauri/defaults/platform-presets.json）

| 字段 | 值 |
|---|---|
| client_type | `default` |
| endpoints.default | openai: `https://open.bigmodel.cn/api/paas/v4`（coding_plan:false）<br>anthropic: `https://open.bigmodel.cn/api/anthropic`（coding_plan:false） |
| models.default | `default: glm-5.2`, `fast: glm-4.7-flashx` |
| model_list.default | glm-5.2, glm-5.1, glm-5, glm-5-turbo, glm-4.7, glm-4.7-flashx, glm-4.7-flash, glm-4.6, glm-4.5-air, glm-4.5-airx |
| peak_hours | `[{start_hour:6, end_hour:10, multiplier:3.0}]`（协议级，未限定 model） |

## 官方文档列出值

### Source
- 模型总览（最新模型页）：https://docs.bigmodel.cn/cn/coding-plan/latest-model
- 快速开始（endpoint 表）：https://docs.bigmodel.cn/cn/coding-plan/quick-start
- 编码套餐概览（peak 规则原文）：https://docs.bigmodel.cn/cn/coding-plan/overview
- 普通版快速开始：https://docs.bigmodel.cn/cn/guide/start/quick-start

### 官方 endpoint（编码套餐 GLM Coding Plan）
| 协议 | base_url | 备注 |
|---|---|---|
| Anthropic Message | `https://open.bigmodel.cn/api/anthropic` | 与普通版同 |
| OpenAI Chat Completion | `https://open.bigmodel.cn/api/coding/paas/v4` | **注意 `/coding/` 段，与普通版 `/api/paas/v4` 不同** |

### 官方最新模型清单（docs 文本提取）
glm-4, glm-4-flash-250414, glm-4.5, glm-4.6, glm-4.6v, glm-4.6v-flash, glm-4.7, glm-4.7-flash, glm-4.7-flashx（JSON 侧）, glm-5, glm-5-turbo, glm-5.1, glm-5.2, glm-5v-turbo, glm-4.1v-thinking, glm-4v-flash 等。

### 官方 GLM-5.2 / GLM-5-Turbo 高阶模型 peak 规则（原文）
> **GLM-5.2/GLM-5-Turbo** 作为高阶模型，对标 Claude Opus，调用时将按照 "高峰期 3 倍，非高峰期 2 倍" 系数消耗额度。
> **（作为限时福利，GLM-5.2/GLM-5-Turbo 将在非高峰期仅作为 1 倍抵扣，持续到 9 月底。）**
> 注："高峰期"为每日的 14:00～18:00（UTC+8）。

来源：https://docs.bigmodel.cn/cn/coding-plan/overview（FAQ 同款表述 https://docs.bigmodel.cn/cn/coding-plan/faq）

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| peak_hours 放协议级 | 全模型高峰 3 倍 | **仅 GLM-5.2 / GLM-5-Turbo 受影响** | **删除** glm 协议的 peak_hours（普通 glm 协议不该全模型 3 倍）。GLM 普通版（按 GLM-4.7 调用）无高阶倍率 |
| models.default.fast slot | `fast: glm-4.7-flashx` | 非白名单 slot | D3 清非标 slot：删 `fast`（`default` slot 已是 glm-5.2） |
| model_list 缺 glm-4.6v / glm-4.6v-flash / glm-4v-flash / glm-4.1v-thinking 等多模态/视觉变体 | 10 个文本模型 | 文档列多模态变体 | 可选补齐（若仅服务文本 coding 场景，可不补；视觉模型经专用 endpoint） |
| model_list 缺 glm-4-flash-250414 | 无 | 官方免费/低价 flash 在列 | 可选补 |
| endpoints 普通版 base_url | `/api/paas/v4` | 普通版（非 coding plan）确实是 `/api/paas/v4` | ✅ 正确 |

## 补齐建议（具体改什么）

1. **删 `peak_hours` 字段**（D2 决策：普通版无高阶倍率，peak_hours 迁移到 glm-coding 独立协议）。
2. **删 `models.default.fast` slot**（D3 非标 slot 清理），保留 `default: glm-5.2`。若需保留 flash 系列快速访问能力，并入 `default` 或弃（default 已够）。
3. model_list 可补 `glm-4-flash-250414`（低价 flash，常用于轻量任务）—— 优先级低。
4. endpoints 维持现状（普通版 `/api/paas/v4` + `/api/anthropic` 正确）。

## Caveats / Not Found

- GLM 普通版（非 coding plan）是否也支持 GLM-5.2 / GLM-5-Turbo 高阶模型并适用 3 倍规则？官方文档的 3 倍表述出现在「编码套餐」文档树（coding-plan/overview + faq），普通按量计费版是否同规则未在抓取的页面内明确。**推测**：普通版同样按高阶模型倍率（GLM-5.2 / 5-Turbo 标定为高阶是模型属性非套餐属性），但福利期「非高峰 1 倍」是 coding plan 专属。`需要: 普通版按量计费 GLM-5.2 是否同 peak 规则的官方说明链接`。
- 普通版 `/api/paas/v4` 与 coding plan `/api/coding/paas/v4` 是否互通：抓取页面未明确，`需要: 同一 API Key 是否两边可用`。
