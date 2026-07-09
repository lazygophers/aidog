# 补全 gemini model_list 全部官方模型

## Goal

Google Gemini 官方 API（`generativelanguage.googleapis.com`，gemini 协议）。现 preset `model_list.default` 含 4 模型，research 核实全部合法（均为 Stable），遗漏 3 个当前官方在列模型。

- **补全**：新增 `gemini-3.1-flash-lite`（Stable）+ `gemini-3.1-pro-preview`（Preview，Gemini 3 Pro 档）+ 可选 `gemini-3-flash-preview`（Preview，性价比档）
- **核实**：`gemini-3.5-flash` 非 UI 笔误，官方 Stable 模型（research line 10-11 页脚 `Last updated 2026-06-30 UTC` 证实）
- **不动**：endpoints / models.default.default 保持

## Research References

- [`research/gemini-models.md`](research/gemini-models.md) — research 全文（模型表 line 21-33 + 推荐清单 line 98-121 + caveats line 142-148）

## Requirements

### 1. endpoints（1 端点，保留不动）

```json
"default": [
  {"protocol": "gemini", "base_url": "https://generativelanguage.googleapis.com", "client_type": "default"}
]
```

gemini 协议仅 host（无 `/v1`），符合全局 URL 约定。

### 2. model_list.default（7 模型，`gemini-<ver>-<variant>` 格式）

按 research line 98-109 推荐清单（含 Preview 档）。新增 3 项标 **(新)**：

```json
"default": [
  "gemini-3.5-flash",
  "gemini-3.1-pro-preview",
  "gemini-3.1-flash-lite",
  "gemini-3-flash-preview",
  "gemini-2.5-pro",
  "gemini-2.5-flash",
  "gemini-2.5-flash-lite"
]
```

排序逻辑（research line 111）：最新 Stable 优先（3.5 Flash）→ Pro 档（3.1 Pro Preview / 2.5 Pro Stable）→ Flash 性价比档。

- **新增**：`gemini-3.1-flash-lite`（Stable，frontier-class at fraction of cost）、`gemini-3.1-pro-preview`（Preview，Gemini 3 Pro 接替已下线的 `gemini-3-pro-preview`）、`gemini-3-flash-preview`（Preview，性价比档）
- **保留**：现有 4 模型全部保留（gemini-2.5-pro/2.5-flash/2.5-flash-lite/3.5-flash）
- **Preview 后缀策略**：research caveat（line 146-147）建议 Preview 模型优先用 `-preview` 后缀串（与 preset 其他协议现有用法一致），稳定后再切无后缀 alias

**保守方案（如产品策略排除 Preview）**：仅加 `gemini-3.1-flash-lite`，共 5 模型。由 implement 阶段按产品策略决定，默认推荐含 Preview 的 7 模型版（model_list 是可选清单，含 Preview 不影响默认模型）。

### 3. models.default（档位名 key → model id string）

```json
"models": {
  "default": {
    "default": "gemini-2.5-pro",
    "fast": "gemini-2.5-flash",
    "thinking": "gemini-3.5-flash"
  }
}
```

- `default`：保持 `gemini-2.5-pro`（research line 123 「维持合理」，官方定位 "most advanced for complex tasks"，当前 preset 值）
- `fast`：`gemini-2.5-flash`（flash → fast 档，低延迟高吞吐）
- `thinking`：`gemini-3.5-flash`（research line 10 官方定位 "Most intelligent model for sustained frontier performance on agentic and coding tasks"，最适合深度推理）

对齐 `Partial<Record<ModelSlot,string>>`。

### 4. desc（保留 8 语言不动）

### 5. source_urls（保留）

```json
"source_urls": {
  "docs": "https://ai.google.dev/gemini-api/docs",
  "pricing": "https://ai.google.dev/gemini-api/docs/pricing"
}
```

与 research 主源 `https://ai.google.dev/gemini-api/docs/models` 同域，保留。

## Acceptance Criteria

- [ ] model_list.default 含 7 模型（或保守版 5 模型，JSON 合法 + 无重复）
- [ ] models.default 含 default/fast/thinking 三档
- [ ] `gemini-3.5-flash` 保留（非笔误，已 research 核实）
- [ ] 不含已 Shut down 模型（`gemini-2.0-flash` / `gemini-3-pro-preview` / `gemini-3.1-flash-lite-preview`）
- [ ] endpoints/desc/source_urls 保留不动
- [ ] `cd src-tauri && cargo build/clippy/test` clean
- [ ] `yarn build` clean（前端无改动可跳）
- [ ] 不动 name/homepage/logo_url/client_type

## Out of Scope

- 上下文窗口数值字段（research caveat line 145 标 `需要:`）
- STATIC_MODEL_IDS（passthrough.rs，独立 task）
- peak_hours / coding_plan 分支
- 其他协议块中引用 Gemini 模型的聚合商（line 1190/1519/1733 等 `google/gemini-*`）
- 已 Shut down 模型（research line 30-33）
- pricing 字段（cost 估算，独立 task）
- 专用任务模型（Computer Use / Deep Research / TTS / Live / Embeddings / Imagen / Veo / Lyria）

## Technical Notes

- 真值源：`src-tauri/defaults/platform-presets.json` 的 `protocols.gemini`
- 数据来源：research 基于 `https://ai.google.dev/gemini-api/docs/models` HTML 抓取（页脚 `Last updated 2026-06-30 UTC`，抓取 2026-07-09）
- ID 格式：`gemini-<major.minor>-<variant>`（如 `gemini-3.5-flash`），Preview 用 `-preview` 后缀（如 `gemini-3.1-pro-preview`）
- 命名约定（research line 39-47）：Stable = 具体稳定串（如 `gemini-3.5-flash`）；Preview = 可能变更（如 `gemini-3.1-pro-preview`）；Latest = 热更别名（如 `gemini-flash-latest`，preset 不用）
- ListModels API 直查未成功（caveat line 144 无 API key），如 main 持有 key 可补查 `GET .../v1beta/models?key=KEY` 二次校验
