# 补全 Compshare model_list + endpoints 全部官方信息

## Goal

Compshare（优云 / UCloud 旗下）有两块产品线，aidog preset 拆为两个 protocol 块：`compshare`（ModelVerse 通用 API，host `api.modelverse.cn`，按量计费，221 个模型多供应商聚合）+ `compshare_coding`（Coding Plan 编程套餐，host `cp.compshare.cn`，订阅制）。

**第一段：ModelVerse 通用 API（`protocols.compshare`）** — 当前 model_list 为空 `[]`，3 个端点（anthropic/openai/gemini）均验证存活（401）。本任务补 model_list 为全量文本对话模型（排除图像/视频/音频/TTS/嵌入/重排等非对话模型），补 models.default 三档，desc 由 "Claude 兼容" 改写为 "多供应商聚合（Claude/GPT/Gemini/DeepSeek/Qwen/GLM 等）"。

**第二段：Coding Plan（`protocols.compshare_coding`）** — 当前 7 个 Claude alias + 1 anthropic 端点（401 存活）。**数据局限**：`cp.compshare.cn/v1/models` 需有效密钥（invalid api key），文档未列出套餐具体模型清单，research 仅能「推测聚焦 Claude 系列」。**保守策略**：保留现有 7 个 aidog alias 不臆造，仅补 models.default 三档、desc 微调突出套餐定位。

## Research References
- [`research/compshare-models.md`](research/compshare-models.md) — 平台为 UCloud 优云旗下 GPU 算力 + 模型 API 聚合；两产品线 host 不同（`api.modelverse.cn` vs `cp.compshare.cn`）；ModelVerse `/v1/models` 端点免鉴权返回 221 个模型（2026-07-09 实测 200）；4 端点探测全 401（存活）；id 格式混合（裸 id + `provider/` 前缀）；Coding Plan 模型范围文档未明确（推测聚焦 Claude 系列）。

## Requirements

### 第一段：protocols.compshare（ModelVerse 通用 API）

#### 1a. endpoints（default 分支，3 端点，全保留）

| # | 操作 | protocol | base_url | client_type | 依据 |
|---|------|----------|----------|-------------|------|
| 1 | 保留 | anthropic | `https://api.modelverse.cn` | claude_code | `/v1/messages` 实测 401 |
| 2 | 保留 | openai | `https://api.modelverse.cn/v1` | codex_tui | `/v1/chat/completions` 实测 401 |
| 3 | 保留 | gemini | `https://api.modelverse.cn` | default | `/v1beta/models` 实测 401 |

#### 2a. model_list.default（混合 id 格式：裸 id + `provider/` 前缀，按平台实际格式）

来源：`https://api.modelverse.cn/v1/models` 免鉴权返回的 221 个模型，**剔除非文本对话**（图像生成 / 视频生成 / 语音 / TTS / 嵌入 / 重排 / 转写）。保留各家族主线对话 / 推理模型（含 `-thinking` / `-codex` 变体）。

```json
"model_list": {
  "default": [
    "claude-fable-5",
    "claude-haiku-4-5-20251001",
    "claude-opus-4-1-20250805",
    "claude-opus-4-5-20251101",
    "claude-opus-4-5-20251101-thinking",
    "claude-opus-4-6",
    "claude-opus-4-7",
    "claude-opus-4-8",
    "claude-sonnet-4-5-20250929",
    "claude-sonnet-4-5-20250929-thinking",
    "claude-sonnet-4-6",
    "claude-sonnet-4.5",
    "claude-sonnet-4.5-thinking",
    "claude-sonnet-5",

    "gpt-4.1-mini",
    "gpt-4.1-nano",
    "gpt-4o-mini",
    "gpt-5",
    "gpt-5-codex",
    "gpt-5.1",
    "gpt-5.1-codex",
    "gpt-5.1-codex-max",
    "gpt-5.1-codex-mini",
    "gpt-5.2",
    "gpt-5.2-codex",
    "gpt-5.3-codex",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
    "gpt-5.4-pro",
    "gpt-5.5",
    "openai/gpt-4.1",
    "openai/gpt-4o",
    "openai/gpt-5",
    "openai/gpt-5-mini",
    "openai/gpt-5-nano",
    "openai/gpt-5.1",
    "openai/gpt-5.1-codex",
    "openai/gpt-5.1-codex-mini",
    "openai/gpt-5.2",

    "gemini-2.5-flash",
    "gemini-2.5-pro",
    "gemini-3-flash-preview",
    "gemini-3.5-flash",
    "gemini-3.1-pro-preview",
    "google/gemma-3-27b-it",
    "google/gemma-4-31b-it",
    "publishers/google/models/gemini-3-flash-preview",
    "publishers/google/models/gemini-3-pro-image-preview",
    "publishers/google/models/gemini-3.1-pro-preview",

    "deepseek-v4-flash",
    "deepseek-v4-pro",
    "deepseek-ai/DeepSeek-V3.2",
    "deepseek-ai/DeepSeek-V3.2-Exp",

    "Qwen/QwQ-32B",
    "Qwen/Qwen3-235B-A22B-Thinking-2507",
    "Qwen/Qwen3-30B-A3B-Thinking",
    "Qwen/Qwen3-Coder",
    "Qwen/Qwen3-Max",
    "Qwen/Qwen3-VL-235B-A22B-Instruct",
    "Qwen/Qwen3-VL-235B-A22B-Thinking",
    "Qwen/Qwen3-vl-Plus",
    "qwen3-30b-a3b",
    "qwen3-coder-30b-a3b-instruct",
    "qwen3-coder-plus",
    "qwen3-max-preview",
    "qwen3-vl-flash",
    "qwen3.5-plus",
    "qwen3.6-35b-a3b",
    "qwen3.6-plus",
    "qwen3.7-max",
    "qwen3.7-plus",

    "glm-5-turbo",
    "glm-5.1",
    "glm-5.2",
    "glm-5v-turbo",
    "zai-org/glm-4.6",
    "zai-org/glm-4.6v",
    "zai-org/glm-4.7",
    "zai-org/glm-5",

    "kimi-k2.6",
    "kimi-k2.7-code",
    "moonshot/kimi-k2.5",
    "moonshotai/Kimi-K2-Instruct",
    "moonshotai/kimi-k2.5",

    "MiniMax-Hailuo-02",
    "MiniMax-Hailuo-2.3",
    "MiniMax-Hailuo-2.3-Fast",
    "MiniMax-M2",
    "MiniMax-M2.1",
    "MiniMax-M2.1-lightning",
    "MiniMax-M2.5",
    "MiniMax-M2.5-lightning",
    "MiniMax-M2.7",
    "MiniMax-M2.7-highspeed",
    "MiniMax-M3",

    "ByteDance/doubao-1-5-pro-32k-250115",
    "ByteDance/doubao-seed-1.6",
    "doubao-1-5-pro-32k-character-250715",
    "doubao-seed-1-6-lite-251015",
    "doubao-seed-2-0-code-preview-260215",
    "doubao-seed-2-0-lite-260215",
    "doubao-seed-2-0-mini-260215",
    "doubao-seed-2-0-pro-260215",
    "doubao-seed-2-1-pro-260628",
    "doubao-seed-2-1-turbo-260628",
    "doubao-seed-evolving",

    "baidu/ernie-4.5-turbo-128k",
    "baidu/ernie-4.5-turbo-vl-32k",

    "grok-4",
    "grok-4-1-fast-non-reasoning",
    "grok-4-1-fast-reasoning",
    "grok-4-fast",
    "grok-4-fast-reasoning",
    "grok-4.20-0309-non-reasoning",
    "grok-4.20-0309-reasoning",
    "grok-4.3",

    "codex-mini-latest",
    "mimo-v2.5",
    "mimo-v2.5-pro"
  ]
}
```

> **排除项**（非文本对话，本任务不纳入）：`gpt-image-1` / `gpt-image-1-mini` / `gpt-image-1.5` / `gpt-image-2` / `openai/sora-2/*`（图像/视频生成）；`gemini-2.5-flash-image` / `gemini-3-pro-image*` / `gemini-3.1-flash-*-image*` / `gemini-embedding-2`（图像/嵌入）；`deepseek-ai/DeepSeek-OCR*`（OCR）；`Qwen/Qwen-Image*` / `qwen-mt-flash` / `qwen3-embedding-8b` / `qwen3-reranker-8b` / `qwen3-tts-flash`（图像/翻译/嵌入/重排/TTS）；`doubao-seedance-*` / `doubao-seedream-*`（视频/图像）；`grok-imagine-*`（图像/视频）；`flux-*` / `midjourney-*` / `wan2.7-image*`（图像）；`Wan-AI/*` / `happyhorse-*` / `kling-*` / `pixverse-*` / `sora-2` / `veo-*` / `vidu-*` / `viduq*` / `wan2.6-r2v*`（视频）；`music-*` / `suno-*` / `speech-*` / `text-to-sound-v2`（音频）；`BAAI/*` / `bge-*` / `text-embedding-*`（嵌入/重排）；`IndexTeam/IndexTTS-2`（TTS）；`easydoc-*` / `stepfun-ai/step1x-edit` / `qwen-mt-flash`（文档/编辑工具）。

#### 3a. models.default（三档，档位名 key → model id string，对齐 `Partial<Record<ModelSlot, string>>`）

主力 = `claude-sonnet-5`（Claude 最新主力）；重型 = `claude-opus-4-8`（Claude 最强）；轻量 = `deepseek-v4-pro`（DeepSeek 最新推理，性价比高，覆盖非 Claude 需求场景）。

```json
"models": {
  "default": {
    "sonnet": "claude-sonnet-5",
    "opus": "claude-opus-4-8",
    "default": "deepseek-v4-pro"
  }
}
```

#### 4a. desc（改写，8 语言）

```json
"desc": {
  "en-US": "Compshare ModelVerse multi-vendor AI API (Claude / GPT / Gemini / DeepSeek / Qwen / GLM)",
  "zh-Hans": "Compshare ModelVerse 多供应商 AI API（Claude / GPT / Gemini / DeepSeek / Qwen / GLM）",
  "ar-SA": "Compshare ModelVerse واجهة AI متعددة الموردين (Claude / GPT / Gemini / DeepSeek / Qwen / GLM)",
  "fr-FR": "API IA multi-fournisseurs Compshare ModelVerse (Claude / GPT / Gemini / DeepSeek / Qwen / GLM)",
  "de-DE": "Compshare ModelVerse Multi-Vendor KI-API (Claude / GPT / Gemini / DeepSeek / Qwen / GLM)",
  "ru-RU": "Мультивендорный AI API Compshare ModelVerse (Claude / GPT / Gemini / DeepSeek / Qwen / GLM)",
  "ja-JP": "Compshare ModelVerse マルチベンダー AI API（Claude / GPT / Gemini / DeepSeek / Qwen / GLM）",
  "es-ES": "API IA multi-proveedor Compshare ModelVerse (Claude / GPT / Gemini / DeepSeek / Qwen / GLM)"
}
```

#### 5a. source_urls（保留）

`docs=https://www.compshare.cn/docs/modelverse/models/quick-start` + `pricing=https://www.compshare.cn/price-list`，均实测 200。

---

### 第二段：protocols.compshare_coding（Coding Plan 编程套餐）

#### 1b. endpoints（default 分支，1 端点，保留）

| # | 操作 | protocol | base_url | client_type | 依据 |
|---|------|----------|----------|-------------|------|
| 1 | 保留 | anthropic | `https://cp.compshare.cn` | claude_code | `/v1/messages` 实测 401（端点存活） |

#### 2b. model_list.default（7 模型，裸 id，全保留）

**数据局限**：套餐文档未列出具体模型清单，`/v1/models` 需有效密钥。**保守保留现有 7 个 aidog alias**，不臆造：

```json
"model_list": {
  "default": [
    "claude-opus-4-8",
    "claude-sonnet-4-6",
    "claude-haiku-4-5",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-opus-4-5",
    "claude-sonnet-4-5"
  ]
}
```

#### 3b. models.default（三档，档位名 key → model id string，对齐 `Partial<Record<ModelSlot, string>>`）

主力 = `claude-sonnet-4-6`；重型 = `claude-opus-4-8`；轻量 = `claude-haiku-4-5`。

```json
"models": {
  "default": {
    "sonnet": "claude-sonnet-4-6",
    "opus": "claude-opus-4-8",
    "haiku": "claude-haiku-4-5"
  }
}
```

#### 4b. desc（保留）

现有 desc 准确反映「编程套餐端点」定位，保留 8 语言不变。

#### 5b. source_urls（保留）

`docs=https://www.compshare.cn/docs/modelverse/models/quick-start` + `pricing=https://www.compshare.cn/price-list`，均实测 200。保留。

## Acceptance Criteria

### compshare（ModelVerse）
- [ ] endpoints.default 保留 3 端点不变
- [ ] model_list.default 含文本对话模型全量（约 110+ 项，剔除图像/视频/音频/TTS/嵌入/重排）
- [ ] model_list.default id 格式保留平台混合原貌（裸 id + `provider/` 前缀）
- [ ] models.default 恰 3 档位名 key（sonnet / opus / default），value 为对应 model id string
- [ ] desc 8 语言全改写，反映多供应商聚合
- [ ] source_urls 保留
- [ ] JSON 合法

### compshare_coding（Coding Plan）
- [ ] endpoints.default 保留 1 端点不变
- [ ] model_list.default 保留原 7 个 aidog alias 不变（数据局限，不臆造）
- [ ] models.default 恰 3 档位名 key（sonnet / opus / haiku），value 为对应 model id string
- [ ] desc 保留
- [ ] source_urls 保留
- [ ] JSON 合法

### 全局
- [ ] 未动 STATIC_MODEL_IDS / 其他协议块 / version / last_updated

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan 分支机制 / 其他协议块 / id 日期后缀（保留平台原貌）
- Coding Plan 新增模型 id（数据局限，需有效套餐密钥验证后再补）
- ModelVerse 非文本对话模型（图像/视频/音频/TTS/嵌入/重排，aidog 不路由）

## Technical Notes
- 真值源：`protocols.compshare` + `protocols.compshare_coding`（src-tauri/defaults/platform-presets.json）
- 数据来源：`https://api.modelverse.cn/v1/models` 免鉴权 GET（2026-07-09 实测 200 返 221 模型）+ 官方文档 compshare.cn/docs/modelverse/*
- id 格式：ModelVerse 混合格式（裸 id + `provider/` 前缀，保留平台原貌）；Coding Plan 裸 id
- 数据强度：**compshare = 强**（公开 API 实测）；**compshare_coding = 弱**（套餐文档未列模型，research 仅推测）— prd 已标注「数据局限」，采用保守策略
