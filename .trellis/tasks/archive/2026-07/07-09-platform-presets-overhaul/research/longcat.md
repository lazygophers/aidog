# Research: longcat（美差 LongCat）

- **Query**: 核对 longcat 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | anthropic: `https://api.longcat.chat/anthropic`（claude_code）<br>openai: `https://api.longcat.chat/openai/v1`（codex_tui） |
| models.default | default: LongCat-2.0 |
| model_list | LongCat-2.0 |

## 官方文档列出值

### Source
- 主站：https://longcat.chat/
- 定价：https://longcat.chat/pricing（**curl 抓取仅 10KB，疑似 SPA 壳**）

### 官方模型
`需要: LongCat 官方模型清单文档链接`（pricing 页 SSR 不友好，未提取到模型 id）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url `/openai/v1`（多 /openai 段） | 与众不同（多数是 `/v1`） | 需核实 | `需要: LongCat OpenAI 兼容 base_url 官方说明`（推测：longcat 在路径中加 `/openai` 区分协议，与 doubao 的 `/api/plan/v3` 同模式） |
| model_list 仅 LongCat-2.0 | 1 个 | 需核实是否有 LongCat-1.x / flash / pro 变体 | `需要: LongCat 全模型清单` |
| models.default.default | LongCat-2.0 | 推测正确（2.0 是当前主线） | 维持 |

## 补齐建议

1. 核对 base_url `/openai/v1` 是否官方正式路径。
2. 核对是否仅 LongCat-2.0 一个模型（还是有更细粒度变体）。
3. 当前数据自洽，无明确缺漏证据。

## Caveats

- LongCat 是较小众厂商，文档 SSR 不友好，curl 抓不到模型清单。
- **优先级低**：JSON 自洽，无用户报缺失。
- `需要: LongCat 模型与定价页有效 URL`。
