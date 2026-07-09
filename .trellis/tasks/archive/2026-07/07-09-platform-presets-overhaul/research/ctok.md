# Research: ctok（CTok.ai 聚合转发站）⭐ 用户报过缺失

- **Query**: 核对 ctok 协议 endpoints/models/model_list（用户反馈过缺失问题）
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | anthropic: `https://api.ctok.ai`（claude_code）<br>openai: `https://api.ctok.ai/v1`（codex_tui）<br>gemini: `https://api.ctok.ai`（default） |
| models.default | opus:claude-opus-4-8, sonnet:claude-sonnet-4-6, haiku:claude-haiku-4-5（**无 default slot**） |
| model_list | claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5, claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5 |

## 官方文档列出值

### Source
- 主站：https://ctok.ai/
- API 入口：https://api.ctok.ai/（**SPA，curl 抓取仅得 HTML 壳 + APP_CONFIG 元数据**，无静态模型列表）

### 官方模型
`需要: CTok.ai 模型清单官方文档链接`。CTok 是聚合转发站，模型随上游（Anthropic/OpenAI/Gemini 等）变化，最佳effort 维护。

## Diff

| 项 | 现状 | 推测 / 官方 | 建议 |
|---|---|---|---|
| base_url（3 协议同根） | `api.ctok.ai` + `/v1` | ✅ 转发站常见模式 | 维持 |
| models.default **无 default slot** | 仅 opus/sonnet/haiku | 应补 default | **补 `default: claude-opus-4-8`**（与 opus 同）—— 这是「缺失」核心，default slot 缺会让 caller 取不到默认模型 |
| model_list 缺 claude-sonnet-5 / claude-fable-5 | 无 | 上游 Anthropic 已发 sonnet-5 / fable-5 | **建议补 `claude-sonnet-5`**（与 anthropic 协议对齐），可选补 fable-5 |
| model_list 缺 claude-mythos-5 | 无 | 上游新发 | 可选补 |
| model_list 含 opus-4-5（无日期）/ opus-4-6 / opus-4-7 | 有 | 推测：聚合站提供多版本 | 维持 |
| 无 openai / gemini 模型 in model_list | 仅 Claude 系 | endpoints 含 openai/gemini 协议但 model_list 无对应模型 | `需要: CTok 是否转发 OpenAI/Gemini 模型`（推测：是，应补 gpt-5.5 / gemini-3 等） |

## 补齐建议（用户报缺失 → 高优先级）

1. **补 `models.default.default = "claude-opus-4-8"`**（与 opus slot 一致）—— 关键修复。
2. **补 model_list**：
   - `claude-sonnet-5`（Anthropic 当前主线，缺失是 bug）
   - 可选 `claude-fable-5`
3. 核对 OpenAI / Gemini 模型是否经 ctok 转发（若转发，补 `gpt-5.5` / `gemini-3-flash` 等头部模型）。
4. base_url 维持。

## Caveats

- CTok 是转发聚合站，模型清单非自有，**随上游变化**，preset 应标「最佳effort，以 `/v1/models` 实时查询为准」。
- **用户报过 ctok 缺失问题**：最可能是 `models.default` 无 default slot + 缺 claude-sonnet-5。ST7 应优先处理。
- 完整模型清单需登录 ctok.ai 控制台或调 `/v1/models`。`需要: CTok 官方模型清单页 URL 或 /v1/models 返回样例`。
