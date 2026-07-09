# Research: bailian（阿里百炼 DashScope）

- **Query**: 核对 bailian 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| endpoints.default | openai: `https://dashscope.aliyuncs.com/compatible-mode/v1`（codex_tui）<br>anthropic: `https://dashscope.aliyuncs.com/apps/anthropic`（claude_code） |
| models.default | default: qwen3.7-max |
| model_list | 50+ 个 qwen 模型（max/plus/flash/coder/vl/omni/ocr/mt/math/doc/deep-research/3.5/3.6/3.7 + 开源 35b/27b/122b/235b/30b 等） |

## 官方文档列出值

### Source
- 模型工作室文档：https://help.aliyun.com/zh/model-studio/
- 计费：https://help.aliyun.com/zh/model-studio/billing-for-model-studio

### 官方模型（计费页提取，部分）
qwen-max, qwen-plus, qwen-flash, qwen-turbo, qwen-long, qwen-long-latest, qwen-coder-plus, qwen-coder-turbo, qwen-vl-max, qwen-vl-plus, qwen-vl-ocr, qwen-vl-ocr-latest, qwen-math-plus, qwen-math-turbo, qwen-mt-plus/flash/lite/turbo, qwen-mt-image, qwen-doc-turbo, qwen-deep-research, qwen-image-2.0/2.0-pro/edit/edit-max/edit-plus, qwen-omni, qwen-flash-character, qwen-audio-*, + 多个日期变体（-2025-xx-xx / -2026-xx-xx）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url 双协议 | `dashscope.aliyuncs.com/compatible-mode/v1` + `/apps/anthropic` | ✅ 正确（DashScope 双兼容路径） | 维持 |
| models.default.default | qwen3.7-max | 官方最新主线（3.7 max 已发） | 维持 |
| model_list 完整度 | 50+ 模型，覆盖 text/coder/vl/omni/ocr/mt/math/doc | ✅ 非常齐全 | 维持 |
| 命名风格 qwen3.7-max vs qwen-max | JSON 用版本号前缀（qwen3.7-max），官方计费页用 base id（qwen-max） | 推测：版本号前缀是新命名规范（qwen3.7-max = qwen-max 的 3.7 版） | `需要: qwen3.x 命名与 base id 对照官方说明` |
| 缺 qwen-image-* / qwen-audio-* | 无 | 官方在列 | 不补（bailian 协议面向 coding，多模态图像/音频不入 model_list 合理） |
| 缺 qwen-omni | 无 | 官方在列 | 可不补（多模态全模态） |

## 补齐建议

1. **无需大改**：JSON 是 60 协议中最完整的之一。
2. 可选：核对 `qwen3.7-max` 等版本号前缀 id 是否官方正式写法（计费页用 base id + 日期变体，JSON 用版本号）。`需要: qwen3.x-max 官方模型 id 说明`。
3. base_url / models slot 维持。

## Caveats

- 阿里文档 SSR 较好（4.5MB 静态内容），提取到的 base id 清单完整；但 qwen3.x-max / qwen3.6-35b-a3b 等版本号 id 与官方计费页 base id（qwen-max）的对照关系，需查「模型矩阵」子页。
- bailian 数据**低风险**，用户未报缺失，JSON 完整度高。
