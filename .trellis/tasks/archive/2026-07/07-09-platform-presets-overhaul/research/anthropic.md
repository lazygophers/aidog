# Research: anthropic（Anthropic 官方）

- **Query**: 核对 anthropic 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | anthropic: `https://api.anthropic.com` |
| models.default | default/opus: claude-opus-4-8, fable: claude-fable-5, sonnet: claude-sonnet-5, haiku: claude-haiku-4-5 |
| model_list | claude-fable-5, claude-opus-4-8, claude-sonnet-5, claude-haiku-4-5, claude-opus-4-7, claude-opus-4-6, claude-sonnet-4-6, claude-opus-4-5-20251101, claude-sonnet-4-5-20250929 |

## 官方文档列出值

### Source
- Models 总览：https://docs.anthropic.com/en/docs/about-claude/models
- Pricing：https://www.anthropic.com/pricing
- API overview：https://docs.anthropic.com/en/api/overview

### 官方模型清单（docs 页面提取，去重）
claude-opus-4-8（current Opus）, claude-opus-4-7, claude-opus-4-6, claude-opus-4-5-20251101, claude-opus-4-1-20250805, claude-sonnet-5（current Sonnet）, claude-sonnet-4-6, claude-sonnet-4-5-20250929, claude-haiku-4-5-20251001, **claude-fable-5（新）**, **claude-mythos-5 / claude-mythos-preview（新）**

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url | `https://api.anthropic.com` | ✅ 正确 | 维持 |
| model_list 缺 claude-mythos-5 | 无 | 官方新增（与 Fable 5 同期发布，shares Fable 5 capability） | **补 `claude-mythos-5`**（若面向 coding 场景非必需可缓） |
| model_list 缺 claude-opus-4-1-20250805 | 无 | 历史模型 | 优先级低（已有 4-5/4-6/4-7/4-8） |
| haiku 缺日期版本号 `-20251001` | `claude-haiku-4-5`（无日期） | 官方 API id 含 `-20251001`（也接受无日期 alias） | 维持 alias 形式（项目其他协议也用 alias） |

## 补齐建议

1. model_list 补 `claude-mythos-5`（高优先级，官方明确新模型）。
2. 其余维持。

## Caveats

- claude-mythos-preview 为 preview 变体，可不入 model_list（preview 模型通常不进稳定 preset）。
- fable/mythos 是否纳入 coding 场景的 model_list 取决于产品定位；JSON 已含 fable-5，对应 mythos-5 应同步。
