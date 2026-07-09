# Research: openai（OpenAI 官方）

- **Query**: 核对 openai 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| client_type | codex_tui |
| endpoints.default | openai: `https://api.openai.com/v1` |
| models.default | gpt: gpt-5.5 |
| model_list | gpt-5.5, gpt-5.5-pro, gpt-5.4, gpt-5.4-pro, gpt-5.4-mini, gpt-5.4-nano, gpt-5.3-codex, gpt-5.2, gpt-5.2-pro, gpt-5.1, gpt-5, gpt-5-pro, gpt-5-mini, gpt-5-nano, o3, o3-pro, gpt-4.1, gpt-4.1-mini, gpt-4o-mini |

## 官方文档列出值

### Source
- Models landing：https://platform.openai.com/docs/models
- API reference：https://platform.openai.com/docs/api-reference
- Pricing：https://openai.com/api/pricing/

### 官方 models 页面提取
**当前主推（页面卡片）**：gpt-5.5, gpt-5.4, gpt-5.4-mini, gpt-5.4-nano。
其他散见（页面静态文本）：gpt-5-6-sol（推测图像/特殊，未确认）, gpt-realtime-*, gpt-image-2, gpt-oss。
旧代（gpt-5.2 / 5.1 / 5 / o3 / gpt-4.1 / gpt-4o-mini）：未在 landing 卡片显式展示，但 API 仍可调（legacy）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url | `https://api.openai.com/v1` | ✅ 正确 | 维持 |
| models.default.gpt | gpt-5.5 | ✅ 最新主推 | 维持 |
| model_list 含 gpt-5.5-pro | 有 | landing 未明示 `pro` 变体（但 OpenAI 历来有 pro/large 变体） | `需要: gpt-5.5-pro 官方模型卡片 URL 确认`（landing 抓取未覆盖 pro 档） |
| model_list 含 gpt-5.3-codex / gpt-5.2 等 legacy | 有 | landing 不展示但 API 仍可调 | 维持（用户向后兼容） |
| o3 / o3-pro | 有 | landing 未展示（可能 legacy 或 reasoning 线） | 维持 |

## 补齐建议

1. base_url / models / model_list 基本正确，无需大改。
2. 建议在 source_urls.pricing 补充直接 models 链接（已有 docs API reference）。
3. **优先级低**：gpt-5.5-pro / gpt-5.4-pro 需官方确认（landing 未显式列出 pro 档），但不阻塞 —— 实际 API 调用失败时再删。

## Caveats

- OpenAI models 页面是 SPA，curl 只拿到 landing 卡片（5.5 / 5.4 / 5.4-mini / 5.4-nano），完整 model id 列表（含 pro / legacy）需登录或调 `/v1/models`。**现状 JSON 较齐全**，未发现明显缺漏。
- gpt-5-6-sol 在页面出现一次，context 不明，`需要: gpt-5-6-sol 模型说明`（推测：图像/特殊用途，不入 model_list）。
