# Research: xiaomi_mimo（小米 MiMo）

- **Query**: 核对 xiaomi_mimo 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | anthropic: `https://api.xiaomimimo.com/anthropic`（claude_code）<br>openai: `https://api.xiaomimimo.com/v1`（codex_tui） |
| models.default | default: mimo-v2.5-pro |
| model_list | mimo-v2.5-pro, mimo-v2.5 |

## 官方文档列出值

### Source
- 平台文档：https://platform.xiaomimimo.com/docs
- 模型摘要：https://platform.xiaomimimo.com/docs/en-US/quick-start/summary/model

### 官方模型（model 摘要页提取）
**mimo-v2.5-pro**（主线 Pro）, **mimo-v2.5**（标准）, mimo-v2.5-asr, mimo-v2.5-tts, mimo-v2.5-tts-voiceclone, mimo-v2.5-tts-voicedesign, **mimo-v2-flash**（轻量 flash）, **mimo-v2-omni**（全模态）, mimo-v2-tts, mimo-v2-pro, mimo-v2-static(?)

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url | `api.xiaomimimo.com/anthropic` + `/v1` | ✅ 正确 | 维持 |
| models.default.default | mimo-v2.5-pro | ✅ 主线 | 维持 |
| model_list 缺 mimo-v2-flash | 无 | 官方在列（经济 flash） | **补 `mimo-v2-flash`**（haiku slot 对标的轻量档） |
| model_list 缺 mimo-v2-omni | 无 | 官方在列（全模态） | 可选补（若 coding 场景不用全模态可缓） |
| model_list 仅 text 模型 | 2 个 | 官方有 asr/tts/voiceclone 等语音系 | 不补（coding 场景） |
| 缺 mimo-v2-pro（旧 v2 线） | 无 | 官方有 mimo-v2-pro | 可选补（v2.5 是新一代，v2-pro 是前代） |

## 补齐建议

1. **补 `mimo-v2-flash`** 到 model_list（经济档，常见 coding 辅助）。
2. 可选补 `mimo-v2-omni`（全模态，若小米开放给 coding 工具）。
3. base_url / models slot 维持。

## Caveats

- 小米 MiMo 文档 SSR 较好，模型清单可静态提取。
- 缺失度低：仅缺 mimo-v2-flash 是明显遗漏（经济档常用），优先级中。
