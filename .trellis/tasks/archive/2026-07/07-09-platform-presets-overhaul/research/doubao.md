# Research: doubao（火山引擎方舟 ModelArk 国内）

- **Query**: 核对 doubao 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | anthropic: `https://ark.cn-beijing.volces.com/api/plan`（claude_code）<br>openai: `https://ark.cn-beijing.volces.com/api/plan/v3`<br>openai_responses: 同 |
| models.default | default:doubao-seed-2-0-code, **fast:doubao-seed-2-0-mini（非标）**, **thinking:doubao-seed-evolving（非标）** |
| model_list | doubao-seed-evolving, doubao-seed-2-1-pro-260628, doubao-seed-2-1-turbo-260628, doubao-seed-2-0-code/pro/lite/mini, doubao-seed-code, doubao-seed-character, doubao-seed-1.8/1.6, + 跨厂商（minimax-m2.7/m3, glm-5.2, deepseek-v4-flash/pro, kimi-k2.6/k2.7-code） |

## 官方文档列出值

### Source
- 方舟文档主页：https://www.volcengine.com/docs/82379
- 定价：https://www.volcengine.com/docs/82379/1544106
- BytePlus 国际版对照：https://docs.byteplus.com/en/docs/ModelArk/1544106

### 官方模型（BytePlus pricing 页提取，doubao 系同源）
**seed 系**：seed-2-0-pro, seed-2-0-code-preview, seed-2-0-lite, seed-2-0-mini, seed-1-8, seed-1-6, seed-1-6-flash, seed-translation
**日期变体**：seed-2-0-pro-260328, seed-2-0-mini-260215/260428, seed-2-0-lite-260228/260428, seed-2-0-code-preview-260328, seed-1-8-251228, seed-1-6-250915, seed-1-6-flash-250715
**跨厂商**：glm-5-2-260617, glm-4-7-251222, deepseek-v4-pro-260425, deepseek-v4-flash-260425, deepseek-v3.1/v3.2/v3-2-251201, deepseek-r1

注意：**火山方舟用 dash 命名（seed-2-0-pro, glm-5-2），doubao JSON 也用 dash（doubao-seed-2-0-pro）**；但 doubao 自营又用 dot（`doubao-seed-2.1`）—— 见 JSON model_list 有 `doubao-seed-2-1-pro-260628`（dash）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url `/api/plan/v3` | 用 plan 路径 | 方舟标准是 `/api/v3`，`plan` 路径推测是套餐计划专用 | `需要: 方舟 plan 路径与标准 v3 路径区别官方说明` |
| models.default.fast / thinking slot | 非标 | D3 删 | **删 fast + thinking**；seed-evolving（thinking 对应）放 `sonnet` 或 `default` |
| model_list 命名 dot vs dash 混用 | `doubao-seed-2-1-pro-260628`（dash 2-1） | 官方 dash 风格 `seed-2-0-pro` / dot 风格 `seed-2.1`？抓取页全用 dash（seed-2-0-pro-260328） | **核对 doubao 厂商自身 dot 风格**（JSON 老格式 `doubao-seed-2.1` 与新 dash `doubao-seed-2-1-pro-260628` 并存） |
| model_list 跨厂商模型 | 含 minimax/glm/deepseek/kimi | ✅ 方舟是多模型聚合平台 | 维持 |
| model_list 缺 seed-1-6-flash / seed-translation | 无 | 官方在列 | 可选补 |

## 补齐建议

1. **D3 删 `fast` / `thinking` slot**。建议：
   ```json
   {"default":"doubao-seed-2-0-code", "sonnet":"doubao-seed-evolving", "haiku":"doubao-seed-2-0-mini", "opus":"doubao-seed-2-1-pro-260628"}
   ```
2. 命名风格统一（dash vs dot）：核对 doubao 自营模型最新官方 id 形式，统一一种。`需要: doubao 自营模型官方 id 命名规范（dot 2.1 vs dash 2-1）`。
3. base_url `/api/plan/v3` 维持（推测是套餐专用，与 statusline 方案一致）。

## Caveats

- 火山方舟文档 SPA，curl 抓取的 `/docs/82379/1544106` 静态内容少，跨厂商模型清单从 BytePlus 镜像页提取（同源数据）。
- doubao 协议本质是方舟多模型网关，model_list 含跨厂商模型合理（用户经方舟统一接入）。
- seed-evolving 是「持续迭代」模型（无固定版本），适合 thinking slot 语义但 slot 名要改。
