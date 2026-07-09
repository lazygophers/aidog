# Research: deepseek（DeepSeek 官方）

- **Query**: 核对 deepseek 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | openai: `https://api.deepseek.com/v1`（codex_tui）<br>anthropic: `https://api.deepseek.com/anthropic` |
| models.default | default: deepseek-v4-flash, **thinking: deepseek-v4-pro**（非标） |
| model_list | deepseek-v4-flash, deepseek-v4-pro |

## 官方文档列出值

### Source
- API 文档：https://api-docs.deepseek.com/
- 定价：https://api-docs.deepseek.com/quick_start/pricing

### 官方模型（pricing 页提取）
**deepseek-v4-flash**（快速通用）、**deepseek-v4-pro**（思维链/reasoning，对标 Pro）。
legacy：deepseek-chat / deepseek-reasoner（前代 alias，pricing 页提及）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url | `api.deepseek.com/v1` + `/anthropic` | ✅ 正确（DeepSeek 双协议） | 维持 |
| model_list 完整 | 仅 v4-flash / v4-pro | 官方现役主力就这两个 | ✅ 正确，无需补 |
| models.default.thinking slot | `thinking: deepseek-v4-pro`（非标） | D3 删 | **删 thinking**，v4-pro 是 reasoning 主力，应放 `default` 或 `sonnet` slot |
| models.default.default | deepseek-v4-flash | flash 是经济档，Pro 是高阶 | 考虑 default 改 `deepseek-v4-pro`（高阶对标），flash 放 `haiku` |

## 补齐建议

1. **D3 删 thinking slot**。建议 models.default 改：
   ```json
   {"default":"deepseek-v4-pro", "sonnet":"deepseek-v4-pro", "haiku":"deepseek-v4-flash"}
   ```
2. base_url / model_list 维持。

## Caveats

- DeepSeek 文档页面是 SPA，pricing 页静态内容仅这两个 v4 模型明示计费。
- DeepSeek 是否支持 anthropic 协议 `/anthropic` 路径：JSON 这么配，`需要: DeepSeek 官方 anthropic-compat 端点说明`（推测：已开放，对标 GLM/MiniMax 双协议模式）。
- 整体 deepseek JSON 数据较准确，仅 slot 清理（D3）需动。
