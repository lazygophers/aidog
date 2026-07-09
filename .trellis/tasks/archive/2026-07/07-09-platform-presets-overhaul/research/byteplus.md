# Research: byteplus（火山引擎 ModelArk 国际版）

- **Query**: 核对 byteplus 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | anthropic: `https://ark.ap-southeast.bytepluses.com/api/coding`（claude_code）<br>openai: `https://ark.ap-southeast.bytepluses.com/api/plan/v3`<br>openai_responses: 同 |
| models.default | default:seed-2-0-pro, **fast:seed-2-0-mini（非标）** |
| model_list | seed-2-0-pro, seed-2-0-code-preview, seed-2-0-lite, seed-2-0-mini, seed-1-8, seed-1-6, glm-5-2, deepseek-v4-pro, deepseek-v4-flash |

## 官方文档列出值

### Source
- ModelArk 文档：https://docs.byteplus.com/en/docs/ModelArk
- 定价：https://docs.byteplus.com/en/docs/ModelArk/1544106

### 官方模型（pricing 页提取，去重）
**seed 系**：seed-2-0-pro, seed-2-0-pro-260328, seed-2-0-code-preview-260328, seed-2-0-lite-260228/260428, seed-2-0-mini-260215/260428, seed-1-8-251228, seed-1-6-250915, seed-1-6-flash-250715, seed-translation
**跨厂商**：glm-5-2-260617, glm-4-7-251222, deepseek-v4-pro-260425, deepseek-v4-flash-260425, deepseek-v3.1/v3.2/v3-2-251201, deepseek-r1

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url `ark.ap-southeast.bytepluses.com` | ✅ 国际版域名正确 | 维持 |
| anthropic path `/api/coding` | 与 doubao 国内 `/api/plan` 不同 | 推测：国际版 coding plan 路径 | `需要: BytePlus coding vs plan 路径官方说明` |
| openai path `/api/plan/v3` | 与 doubao 同 | ✅ 一致 | 维持 |
| models.default.fast slot | 非标 | D3 删 | **删 fast**，mini 放 `haiku` slot |
| model_list 缺日期变体 | 仅 base id | 官方提供日期变体（260328 等） | 可选补（point-in-time 快照，默认用 base id 即可） |
| model_list 缺 deepseek-v3.x / r1 | 无 | 官方在列 | 可选补（legacy reasoning 线） |
| model_list 缺 seed-1-6-flash / seed-translation | 无 | 官方在列 | 可选补 |
| 命名：JSON `glm-5-2` / `deepseek-v4-pro` | dash 风格 | ✅ 官方 dash 风格 | 维持（与 doubao 同步） |

## 补齐建议

1. **D3 删 fast slot**。建议 models.default：
   ```json
   {"default":"seed-2-0-pro", "sonnet":"seed-2-0-code-preview", "haiku":"seed-2-0-mini", "opus":"seed-2-0-pro"}
   ```
2. model_list 可补 `seed-1-6-flash`（经济 flash 档）、`seed-1-8`（已含）、跨厂商 `deepseek-v3.2`（若仍可调）。

## Caveats

- byteplus 是 doubao 国际版镜像，数据结构镜像，仅域名 + 部分路径差异。
- 官方定价页静态内容较完整（与国内 SPA 不同，BytePlus 文档更 SSR 友好）。
