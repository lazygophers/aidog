# Research: Google Gemini 官方模型清单（generativelanguage.googleapis.com）

- **Query**: 查 Gemini 全部官方模型清单 + 核实 `gemini-3.5-flash` 是否存在 + 现 preset 遗漏对比
- **Scope**: external（WebFetch 官方文档）+ internal（preset JSON 核对）
- **Date**: 2026-07-09
- **Primary Source**: https://ai.google.dev/gemini-api/docs/models （页面页脚 `Last updated 2026-06-30 UTC`，HTML 抓取于 2026-07-09）

## 核心结论（TL;DR）

1. **`gemini-3.5-flash` 是官方真实模型，非笔误。** 官方文档把它作为 "Stable"（稳定版）模型串的**标准示例**直接引用：「For example: `gemini-3.5-flash`.」显示名 "Gemini 3.5 Flash"，定位 "Most intelligent model for sustained frontier performance on agentic and coding tasks."。
2. preset 现 `model_list.default` 4 个模型**全部官方存在且合法**：`gemini-2.5-pro` / `gemini-2.5-flash` / `gemini-2.5-flash-lite` / `gemini-3.5-flash`。
3. **遗漏项**（同为当前官方在列、文本可用、preset 未收）：
   - `gemini-3.1-flash-lite` — **Stable**，「Frontier-class performance rivaling larger models at a fraction of the cost.」
   - `gemini-3.1-pro`（亦作 `gemini-3.1-pro-preview`）— **Preview**，「Advanced intelligence, complex problem-solving skills, and powerful agentic and vibe coding capabilities.」
   - `gemini-3-flash`（亦作 `gemini-3-flash-preview`）— **Preview**，「Frontier-class performance rivaling larger models at a fraction of the cost.」

## 全文本模型清单表（API model id + 状态 + 定位）

> 仅收录**文本生成 / 对话 / 代理**类（excludes: Imagen / Veo / Nano Banana / Lyria / TTS / Live / Embeddings / Robotics 等图像视频音频嵌入机器人专用模型，按任务要求排除）。状态原词沿用官方文档（Stable / Preview / Shut down / Deprecated）。

| API model id | 显示名 | 状态 | 用途（官方原文） |
|---|---|---|---|
| `gemini-3.5-flash` | Gemini 3.5 Flash | **Stable** | Most intelligent model for sustained frontier performance on agentic and coding tasks. |
| `gemini-3.1-flash-lite` | Gemini 3.1 Flash-Lite | **Stable** | Frontier-class performance rivaling larger models at a fraction of the cost. |
| `gemini-2.5-flash` | Gemini 2.5 Flash | Stable（在 Gemini 2.5 系列正文出现，无 "Shut down/Deprecated" 标记） | Our best price-performance model for low-latency, high-volume tasks that require reasoning. |
| `gemini-2.5-flash-lite` | Gemini 2.5 Flash-Lite | Stable | The fastest and most budget-friendly multimodal model in the 2.5 family. |
| `gemini-2.5-pro` | Gemini 2.5 Pro | Stable | Our most advanced model for complex tasks, featuring deep reasoning and coding capabilities. |
| `gemini-3.1-pro`（亦 `gemini-3.1-pro-preview`） | Gemini 3.1 Pro | **Preview** | Advanced intelligence, complex problem-solving skills, and powerful agentic and vibe coding capabilities. |
| `gemini-3-flash`（亦 `gemini-3-flash-preview`） | Gemini 3 Flash | **Preview** | Frontier-class performance rivaling larger models at a fraction of the cost. |
| `gemini-2.0-flash` | Gemini 2.0 Flash | **Shut down** | Our second generation workhorse model, with next-gen features and improved capabilities, including superior speed, native tool use, and a 1M token context window. |
| `gemini-2.0-flash-lite` | Gemini 2.0 Flash-Lite | **Shut down** | Our fastest second generation model, optimized for cost efficiency and low latency. |
| `gemini-3-pro-preview` | Gemini 3 Pro Preview | **Shut down** | Our state-of-the-art reasoning model, with advanced multimodal understanding. |
| `gemini-3.1-flash-lite-preview` | Gemini 3.1 Flash-Lite Preview | **Shut down** | Our most cost-efficient multimodal model, offering the fastest performance for high-frequency, lightweight tasks. |

补充（专用任务文本-相关，按需可考虑，但默认建议不进普通对话列表）：
- `gemini-2.5-computer-use-preview-10-2025` — Computer Use（Preview，UI 自动化专用）
- `gemini-deep-research-preview` / `gemini-deep-research-max-preview` — Deep Research（Preview，研究代理）

### 版本命名约定（官方原文，2025-09 起生效）

文档原文（`Model version name patterns` 章节）：
- **Stable** — 「Points to a specific stable model. Stable models usually don't change. Most production apps should use a specific stable model. **For example: `gemini-3.5-flash`.**」
- **Preview** — 「Points to a preview model which may be used for production. … **For example: `gemini-2.5-flash-preview-09-2025`.**」
- **Latest** — 「Points to the latest release for a specific model variation. … **For example: `gemini-flash-latest`.**」
- **Experimental** — 实验模型，不建议生产使用。

→ 官方推荐**生产用具体 stable 串**（如 `gemini-3.5-flash`），不用 `-latest` 热更别名（避免版本被静默替换）。

## `gemini-3.5-flash` 核实（重点可疑项）

- **判定：官方存在，合法 stable model id，preset 用法正确。**
- 证据：`https://ai.google.dev/gemini-api/docs/models` 页：
  1. 正文表格「Gemini 3」系列首列：显示名 **"Gemini 3.5 Flash"**，状态 **"Stable"**，描述 "Most intelligent model for sustained frontier performance on agentic and coding tasks."
  2. 「Stable」命名约定段直接用 `gemini-3.5-flash` 作标准示例串。
  3. HTML 内同页同时出现 `gemini-3.5`（系列根别名）与 `gemini-3.5-flash`（具体变体）。
- 推断（非笔误，非未来模型）：Gemini 3 系列已正式发布，且 3.5 Flash 为当前 **Stable** 主力；3.1 Flash-Lite 同为 Stable；3 / 3.1 Pro 仍在 Preview。
- 历史：`gemini-3-pro-preview`（Gemini 3 Pro Preview）已 Shut down，被 3.5 Flash / 3.1 Pro 接替。

## 现 preset 对比（`src-tauri/defaults/platform-presets.json` `protocols.gemini`）

### preset 当前 `model_list.default`（line 178-183）

```json
"default": [
  "gemini-2.5-pro",
  "gemini-2.5-flash",
  "gemini-2.5-flash-lite",
  "gemini-3.5-flash"
]
```

### 逐项核实

| preset model id | 官方状态 | 结论 |
|---|---|---|
| `gemini-2.5-pro` | Stable | OK |
| `gemini-2.5-flash` | Stable | OK |
| `gemini-2.5-flash-lite` | Stable | OK |
| `gemini-3.5-flash` | **Stable** | OK（非笔误） |

### 遗漏（同为官方当前在列、文本可用）

| 建议补入 model id | 官方状态 | 理由 |
|---|---|---|
| `gemini-3.1-flash-lite` | **Stable** | 与 3.5 Flash 同为当前 stable 主力；官方定位 "frontier-class at fraction of cost"，性价比档。 |
| `gemini-3.1-pro` | Preview | Gemini 3 系列的 Pro 档（对应旧的 `gemini-3-pro-preview`，已被 3.1 Pro 接替），代理/编码用户可能需要。 |
| `gemini-3-flash` | Preview | 价格友好档的 preview 版本（用户若显式想试 preview 可用）。 |

### 不建议补入（已 shut down 或专用任务）

- `gemini-2.0-flash` / `gemini-2.0-flash-lite` — 官方 **Shut down**，调用会失败。
- `gemini-3-pro-preview` / `gemini-3.1-flash-lite-preview` — 官方 **Shut down**。
- `*-tts-preview` / `*-live-preview` — 语音/实时流，非普通对话。
- `gemini-2.5-computer-use-preview-10-2025` — UI 自动化专用，默认不放通用列表。
- `*-image` / Nano Banana / Veo / Lyria / Imagen — 图像/视频/音乐。
- `gemini-embedding-001` / `gemini-embedding-2` — 嵌入向量，非生成。

## 推荐 `model_list.default` 最终清单

```json
"default": [
  "gemini-3.5-flash",
  "gemini-3.1-pro",
  "gemini-3.1-flash-lite",
  "gemini-2.5-pro",
  "gemini-2.5-flash",
  "gemini-2.5-flash-lite"
]
```

排序逻辑：**最新 Stable 优先**（3.5 Flash / 3.1 Flash-Lite）→ Pro 档（3.1 Pro Preview / 2.5 Pro Stable）→ 2.5 性价比档（Flash / Flash-Lite）。是否纳入 preview 模型由产品策略决定（preview 计费已启用但 rate limit 更严，至少 2 周前通知弃用）；保守起见可只加 Stable：

```json
"default": [
  "gemini-3.5-flash",
  "gemini-3.1-flash-lite",
  "gemini-2.5-pro",
  "gemini-2.5-flash",
  "gemini-2.5-flash-lite"
]
```

`models.default.default`（默认模型）维持 `gemini-2.5-pro` 合理（官方定位 "most advanced for complex tasks"）；若想推最新主力可改 `gemini-3.5-flash`（官方 "most intelligent … agentic and coding"）。

## 其他协议 preset 中的 Gemini 引用（旁证）

preset 其他协议也引用了 Gemini 模型（非 gemini 协议，多为聚合型平台）：
- line 1190, 1519, 1733：`google/gemini-3.5-flash`、`google/gemini-3.1-pro-preview` — 前缀 `google/` 为聚合商命名空间，与官方 `gemini-3.5-flash` 等价，均合法。
- line 1734：`google/gemini-3-pro-preview` — 该模型官方已 **Shut down**，建议相关聚合协议 preset 同步移除/替换为 `google/gemini-3.1-pro-preview`。

## 引用

- **主源（模型表 + 命名约定）**: https://ai.google.dev/gemini-api/docs/models （HTML 页脚 `Last updated 2026-06-30 UTC`；抓取 2026-07-09；正文 "Gemini 3" 系列、"Model version name patterns" 段）
  - Stable 示例原文：`For example: gemini-3.5-flash.`
  - Gemini 3.5 Flash 描述：`Most intelligent model for sustained frontier performance on agentic and coding tasks.`
  - Gemini 3.1 Flash-Lite 描述：`Frontier-class performance rivaling larger models at a fraction of the cost.`
  - Gemini 3 Pro Preview / Gemini 2.0 Flash-Lite 状态：`Shut down`（位于 "Previous models — These models are deprecated and will be shut down soon" 段）
  - Imagen 4 状态：`Deprecated`
- **模型弃用清单**: https://ai.google.dev/gemini-api/docs/deprecations （文档内链 "Gemini deprecations page"）
- **本仓库 preset 真值源**: `src-tauri/defaults/platform-presets.json:161-184`（`protocols.gemini`）

## Caveats / 未决项

- **ListModels API 直查未成功**：`GET https://generativelanguage.googleapis.com/v1beta/models` 需有效 API key，本环境无可用 key（返 `API_KEY_INVALID`）。若 main 持有有效 key，可补一次 `curl '...?key=KEY'` 拿机器可读 model id 列表二次校验（推荐做，可发现 page 未列的子变体如 `gemini-3.5-flash-<MM-YYYY>`）。
- **上下文窗口数值未单独抓取**：page 仅 Gemini 2.0 Flash 段提及 `1M token context window`；3.5 / 3.1 / 2.5 的具体 token 数未在抓取文本中显式出现（多为 JS 渲染）。`需要:` 若要精确窗口值，建议查 https://ai.google.dev/gemini-api/docs/models/gemini-v3 等子页或 ListModels API 返回的 `inputTokenLimit` / `outputTokenLimit` 字段。
- **`gemini-3.1-pro` vs `gemini-3.1-pro-preview` 精确串**：页面 HTML 同时出现 `gemini-3.1-pro` 与 `gemini-3.1-pro-preview` 两种 token，前者疑为 stable alias、后者为 preview 指针。生产建议优先试 `gemini-3.1-pro-preview`（与 preset 其他协议现有用法 line 1344/1409 一致），稳定后再切 `gemini-3.1-pro`。`需要:` ListModels 实测确认。
- **`gemini-3-flash` 同理**：HTML 同时含 `gemini-3-flash` 与 `gemini-3-flash-preview`，建议优先用 `-preview` 后缀。
