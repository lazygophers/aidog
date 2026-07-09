# Research: minimax / minimax_en（MiniMax 国内 / 国际）

- **Query**: 核对 minimax 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | minimax（国内） | minimax_en（国际） |
|---|---|---|
| openai base_url | `https://api.minimaxi.com/v1`（coding_plan:false） | `https://api.minimax.io/v1`（coding_plan:false） |
| anthropic base_url | `https://api.minimaxi.com/anthropic` | `https://api.minimax.io/anthropic` |
| models.default | default:MiniMax-M3, sonnet:MiniMax-M2.7, **coder:MiniMax-M2.5（非标）**, fast:MiniMax-M2.7-highspeed（非标） | 同左 |
| model_list | M3, M2.7, M2.7-highspeed, M2.5, M2.5-highspeed, M2.1, M2.1-highspeed, M2 | 同左 |

## 官方文档列出值

### Source
- 国内文档：https://platform.minimaxi.com/document/Announcement
- 国际文档：https://platform.minimax.io/document/Announcement
- 国内定价：https://platform.minimaxi.com/document/Price
- 国际定价：https://platform.minimax.io/document/Price
- Models 文档：https://platform.minimaxi.com/document/Models

### 官方模型清单（Models 文档提取）
MiniMax-M1, **MiniMax-M2**, **MiniMax-M2.1**, **MiniMax-M2.5**, **MiniMax-M2.7**, **MiniMax-M3**（text 主线，全部在列）, MiniMax-Text-01, MiniMax-VL-01, MiniMax-Hailuo, MiniMax-Hailuo-2.3, MiniMax-Speech-2.6。

`-highspeed` 变体：Models 列表页未显式（但 pricing 页常见，是经济快速档变体）。

## Diff

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| base_url（国内 / 国际） | minimaxi.com / minimax.io | ✅ 官方双站 | 维持 |
| models.default.coder slot | `coder: MiniMax-M2.5`（非标） | D3 删，M2.5 是 text 主线 | **D3 删 coder slot**；若需保留 code 倾向，并入 `gpt` 或 `default`（M3 是 default，M2.5 可放 `gpt` slot 作次选） |
| models.default.fast slot | `fast: MiniMax-M2.7-highspeed`（非标） | D3 删 | **删 fast**，highspeed 模型用户从 model_list 自选 |
| model_list 缺 M1 | 无 | 官方在列（前代） | 可选补（优先级低，M2 已是 baseline） |
| model_list 缺 MiniMax-Text-01 / VL-01 | 无 | 官方在列（长文本 / 视觉） | 可选补（多模态/长文本场景） |
| highspeed 变体（M2.7/M2.5/M2.1-highspeed） | 有 | Models 列表未显式，pricing 页常见 | `需要: highspeed 变体官方模型 id 确认`（推测：经济快速档，pricing 页计费独立） |

## 补齐建议

1. **D3 删 `coder` / `fast` slot**。建议改 models.default 为：
   ```json
   {"default":"MiniMax-M3", "sonnet":"MiniMax-M2.7", "gpt":"MiniMax-M2.5", "haiku":"MiniMax-M2.7-highspeed"}
   ```
   （haiku 对标经济快速档，highspeed 归 haiku slot）
2. model_list 补 `MiniMax-M1`（前代完整线）、可选补 `MiniMax-Text-01`。
3. endpoints base_url 维持。

## Caveats

- minimax 与 minimax_en 仅域名差异（minimaxi.com 国内 / minimax.io 国际），模型同。
- highspeed 变体是否官方正式 id：Models 文档页未列，但 pricing 计费行常见。`需要: MiniMax-highspeed 官方 Models 页确认链接`。
- 国内 / 国际文档 SPA，curl 仅得部分模型；完整 list 需翻 Models 子页。
