# Research: codex（OpenAI Codex TUI — 走 openai_responses 协议）

- **Query**: 核对 codex 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| client_type | default |
| endpoints.default | openai_responses: `https://api.openai.com/v1`（client_type: codex_tui） |
| models.default | gpt: gpt-5.5, **fast: gpt-5.4-mini**（非标 slot） |
| model_list | gpt-5.5, gpt-5.4, gpt-5.4-mini, gpt-5.3-codex-spark |

## 官方文档列出值

### Source
- Codex 指南：https://platform.openai.com/docs/guides/codex
- Models：https://platform.openai.com/docs/models
- Pricing：https://openai.com/api/pricing/

### 官方 Codex CLI 默认模型
Codex CLI（codex_tui）默认走 `gpt-5.5`（与 `openai` 协议一致），`gpt-5.4-mini` 用于快速任务。`gpt-5.3-codex` 系列：landing 未明示 `-codex` 与 `-codex-spark` 区别。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url | `https://api.openai.com/v1`（openai_responses） | ✅ 正确（Codex 走 Responses API） | 维持 |
| models.default.fast slot | `fast: gpt-5.4-mini` | 非标 slot | **D3 删 fast slot**（非白名单），经济模型 mini 可并入 `default` 或弃（default 已是 gpt-5.5，fast 含义由 caller 选择不同 model） |
| model_list 含 gpt-5.3-codex-spark | 有 | landing 未展示 `-spark` 变体 | `需要: gpt-5.3-codex-spark 官方说明`（推测：codex 系列轻量变体，专给 CLI） |
| model_list 缺 gpt-5.5-pro / gpt-5.4-pro | 无（openai 协议有） | 见 openai 协议 caveat | 可选补 pro 档 |

## 补齐建议

1. **删 `models.default.fast` slot**（D3），保留 `gpt: gpt-5.5`。fast 的语义（用 mini 快速应答）由用户在 model_list 自选，不进 slot 体系。
2. base_url / endpoint 维持。
3. model_list 可选补 `gpt-5.5-pro`（高阶档），但优先级低。

## Caveats

- `gpt-5.3-codex` 与 `gpt-5.3-codex-spark` 的官方区分未在抓取页面找到，`需要: Codex CLI 模型清单官方文档`。
- codex 协议本质是 `openai_responses`，与 openai 协议（`openai` chat completion）路由不同；preset 分两个协议合理。
