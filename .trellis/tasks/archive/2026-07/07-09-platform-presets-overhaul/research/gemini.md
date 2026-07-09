# Research: gemini（Google Gemini API）

- **Query**: 核对 gemini 协议 endpoints/models/model_list 与官方文档差异
- **Scope**: external
- **Date**: 2026-07-09

## 现有 JSON

| 字段 | 值 |
|---|---|
| client_type | default |
| endpoints.default | gemini: `https://generativelanguage.googleapis.com` |
| models.default | default: gemini-2.5-pro, **fast: gemini-2.5-flash**（非标）, **thinking: gemini-3.5-flash**（非标） |
| model_list | gemini-3.5-flash, gemini-3.1-pro-preview, gemini-3.1-flash-lite, gemini-3-flash-preview, gemini-2.5-pro, gemini-2.5-flash, gemini-2.5-flash-lite |

## 官方文档列出值

### Source
- Models：https://ai.google.dev/gemini-api/docs/models
- Pricing：https://ai.google.dev/gemini-api/docs/pricing

### 官方模型清单（docs 提取，去重）
**Gemini 3 系**：gemini-3, gemini-3-preview, gemini-3-flash, gemini-3-flash-preview, gemini-3-pro-image, gemini-3-1-flash-live-preview-2, gemini-3-1-flash-lite-preview-deprecated
**Gemini 2.5 系**：gemini-2.5-pro, gemini-2.5-flash, gemini-2.5-flash-lite, gemini-2.5-flash-image, gemini-2.5-flash-preview-09-2025, gemini-2.5-flash-lite-preview-09-2025, gemini-2.5-pro-preview-tts, gemini-2.5-computer-use-preview-10-2025, gemini-2.5-flash-native-audio-preview-12-2025
**Gemini 2.0 系（含 deprecated）**：gemini-2.0-flash, gemini-2.0-flash-lite, gemini-2-0-flash-deprecated, gemini-2-0-flash-lite-deprecated

## Diff（**多处问题，优先级高**）

| 项 | 现状 | 官方 | 建议 |
|---|---|---|---|
| `gemini-3.5-flash` | JSON 有 | **官方无 `gemini-3.5`，只有 `gemini-3` / `gemini-3-flash`** | **疑似臆造 id，删或改 `gemini-3-flash`** |
| `gemini-3.1-pro-preview` | JSON 有 | 官方只有 `gemini-3-1-flash-lite-preview-deprecated` / `gemini-3-1-flash-live-preview-2`，**无 `3.1-pro`** | **疑似臆造，删** |
| `gemini-3.1-flash-lite`（无 -preview 后缀） | JSON 有 | 官方写法是 `gemini-3-1-flash-lite-preview-deprecated`（dash 分隔 + preview + deprecated） | 改对 id 或删 |
| `gemini-3-flash-preview` | JSON 有 | 官方有 `gemini-3-flash-preview` ✅ | 维持 |
| models.default.thinking = `gemini-3.5-flash` | 非标 slot + 不存在 id | 改 `gemini-3-flash` | **D3 删 thinking slot + 改 id** |
| models.default.fast = `gemini-2.5-flash` | 非标 slot | 删 fast slot | **D3 删** |
| models.default.default = `gemini-2.5-pro` | OK 但官方最新是 `gemini-3-pro` 系（gemini-3-pro-image 已发，gemini-3-pro 通用版推测存在） | `gemini-3-pro` 未在抓取列表明确，但 gemini-3 系已发 | 维持 2.5-pro 作 default 稳妥（3-pro 通用版 id 待官方明示） |
| 命名分隔符：点 vs dash | JSON 用点（2.5 / 3.1 / 3.5） | 官方最新（3 系）用 dash（`gemini-3-flash`），2.5 系用点（`gemini-2.5-pro`） | **2.5 系维持点，3 系改 dash** |

## 补齐建议

1. **D3 删 `fast` / `thinking` slot**，保留 `default: gemini-2.5-pro`。
2. **model_list 重构**：
   - 删 `gemini-3.5-flash`（臆造）
   - 删 `gemini-3.1-pro-preview`（臆造）
   - `gemini-3.1-flash-lite` → `gemini-3-flash-lite` 或保留官方 deprecated 全名
   - 补 `gemini-3` / `gemini-3-flash`（官方最新主推）
   - 2.5 系维持
3. 建议新 model_list：
   ```
   ["gemini-3", "gemini-3-flash", "gemini-3-flash-preview",
    "gemini-2.5-pro", "gemini-2.5-flash", "gemini-2.5-flash-lite",
    "gemini-2.0-flash", "gemini-2.0-flash-lite"]
   ```

## Caveats

- `gemini-3-pro`（通用版，对标 Pro）在抓取页面只见到 `gemini-3-pro-image`（图像变体），通用 gemini-3-pro id 是否存在 `需要: 官方 gemini-3 通用 Pro 模型 id 说明`。
- Google 模型 id 历史用点（1.5/2.0/2.5），3 系转 dash 风格 —— 已在 docs 确认（gemini-3-flash 而非 gemini-3.0-flash）。
- **优先级高**：现有 JSON 多个 3.x id 疑似臆造，ST7 必须核实修正。
