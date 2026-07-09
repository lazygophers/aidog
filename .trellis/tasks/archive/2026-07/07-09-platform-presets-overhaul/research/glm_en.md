# Research: glm_en（智谱 GLM 国际版 z.ai）

- **Query**: 核对 glm_en 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | openai: `https://api.z.ai/api/paas/v4`（codex_tui）<br>anthropic: `https://api.z.ai/api/anthropic`（claude_code） |
| models.default | default: glm-5.2, **fast: glm-4.7-flashx（非标）** |
| model_list | 同 glm 协议（10 个 glm 模型） |
| peak_hours | **无**（与 glm 协议不同 —— glm 协议有 peak_hours） |

## 官方文档列出值

### Source
- 国际版文档：https://docs.z.ai/guide/start/quick-start
- 国际版定价：https://z.ai/pricing（**curl 抓取需复核**，国际版站点 SSR 待验）

### 官方模型
国际版 z.ai 与国内 open.bigmodel.cn 同源（智谱海外品牌），模型同 glm-5.2 / 5-turbo / 4.7 系列。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url `api.z.ai/api/paas/v4` + `/api/anthropic` | 与国内 open.bigmodel.cn 路径结构一致 | ✅ 推测正确 | 维持 |
| **peak_hours 缺失** | 无 | 国际版是否同样有「GLM-5.2/5-Turbo 高峰 3 倍」规则？ | `需要: z.ai 国际版 peak 规则官方说明`（推测：海外版计费规则可能不同，高峰时段定义也可能不同——若以美西时区定义则与国内 UTC+8 不同） |
| models.default.fast slot | 非标 | D3 删 | **删 fast**（同 glm 协议处理） |
| model_list 同 glm | 10 个 | 同源模型 | 维持 |
| 国际版是否有 Coding Plan 套餐 | JSON 未体现 | `需要: z.ai 是否有 coding plan 对应套餐`（推测：有，但路径是否同 `/api/coding/paas/v4` 待证） |

## 补齐建议

1. **D3 删 `fast` slot**。
2. 核对国际版 peak_hours：若 z.ai 同样适用高峰 3 倍规则，补 peak_hours（时区可能不同）。
3. 若 z.ai 也有 coding plan，可考虑补 `glm_en-coding` 或在 glm_en 用 endpoint flag。
4. base_url 维持。

## Caveats

- z.ai 是智谱海外品牌，与国内 open.bigmodel.cn 同源。模型清单同。
- 国际版 peak 规则 / coding plan 套餐是否存在：未在抓取页面明示。`需要: z.ai 国际版 coding plan 与 peak 规则官方文档`。
- 与 glm 协议配对处理（D2 决策下，glm 删 peak_hours；glm_en 是否也删/补，取决于国际版规则）。
