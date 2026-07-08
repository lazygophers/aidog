# Research: OpenAI 官方模型清单 vs aidog preset 遗漏分析

- **Query**: 查 OpenAI 全部官方模型清单（从官方文档），判现 aidog preset 遗漏了哪些
- **Scope**: external（OpenAI 官方文档站 developers.openai.com / platform.openai.com）
- **Date**: 2026-07-08
- **API 协议**: OpenAI Chat Completions（`base_url=https://api.openai.com/v1` + `/chat/completions`）

---

## 数据源（官方文档 URL）

| 资源 | URL | 用途 |
|---|---|---|
| All models 总览（含 Available/Deprecated 标签） | https://developers.openai.com/api/docs/models/all/ | 主真值源，按系列分类，标「Deprecated」 |
| 单模型详情页 | https://developers.openai.com/api/docs/models/{slug}/ | 上下文窗口 / max output / 知识截止 / snapshot id / 支持的 endpoints |
| Deprecations 时间线 | https://developers.openai.com/api/docs/deprecations | sunset 日期 + 推荐替代 |
| Models 入口（旧） | https://platform.openai.com/docs/models | 301 跳 developers.openai.com，JS 渲染无内容 |

> 注：developers.openai.com 是 Astro SSR 站，单模型页含全部结构化字段；总览页（`/all/`）同时列出 available 与 deprecated（带「Deprecated」后缀）。

---

## 现 aidog preset 状态（核对源）

`src-tauri/defaults/platform-presets.json:60-93`（protocols.openai）：

```jsonc
"models":     { "default": { "gpt": "gpt-5.5" } },
"model_list": { "default": ["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano"] }
```

`src-tauri/src/gateway/proxy/passthrough.rs:238-247` STATIC_MODEL_IDS 同步上述 4 个 GPT-5.x id（注释「最近核对: 2026-07-08」）。

---

## OpenAI 当前可用模型全表（仅 Chat Completions text/code 相关）

> 范围：从 https://developers.openai.com/api/docs/models/all/ 的「Frontier models」+「More models」筛选 **未带 Deprecated 标签** 且支持 `/v1/chat/completions` 的模型。Image / Realtime & audio / Embedding / Moderation / ChatGPT-only 单独列出，**不进 model_list**。

### GPT 主线（Chat Completions 文本/代码）

| Model ID (alias) | 日期 snapshot | 系列 | 上下文窗口 | Max Output | 知识截止 | 用途（官方原文摘） | 详情页 |
|---|---|---|---|---|---|---|---|
| `gpt-5.5` | `gpt-5.5-2026-04-23` | Frontier 旗舰 | 1,050,000 | 128,000 | Dec 01, 2025 | "A new class of intelligence for coding and professional work" | /models/gpt-5.5/ |
| `gpt-5.5-pro` | `gpt-5.5-pro-2026-04-23` | Frontier Pro | 1,050,000 | 128,000 | Dec 01, 2025 | "Version of GPT-5.5 that produces smarter and more precise responses" | /models/gpt-5.5-pro/ |
| `gpt-5.4` | `gpt-5.4-2026-03-05` | Frontier | 1,050,000 | 128,000 | Aug 31, 2025 | "A more affordable model for coding and professional work" | /models/gpt-5.4/ |
| `gpt-5.4-pro` | `gpt-5.4-pro-2026-03-05` | Frontier Pro | 1,050,000 | 128,000 | Aug 31, 2025 | "Version of GPT-5.4 that produces smarter and more precise responses" | /models/gpt-5.4-pro/ |
| `gpt-5.4-mini` | `gpt-5.4-mini-2026-03-17` | Frontier mini | 400,000 | 128,000 | Aug 31, 2025 | "Strongest mini model yet for coding, computer use, and subagents" | /models/gpt-5.4-mini/ |
| `gpt-5.4-nano` | `gpt-5.4-nano-2026-03-17` | Frontier nano | 400,000 | 128,000 | Aug 31, 2025 | "Cheapest GPT-5.4-class model for simple high-volume tasks" | /models/gpt-5.4-nano/ |
| `gpt-5.3-codex` | （无日期 alias，仅自身） | Codex | 400,000 | 128,000 | Aug 31, 2025 | "The most capable agentic coding model to date" | /models/gpt-5.3-codex/ |
| `gpt-5.2` | `gpt-5.2-2025-12-11` | Previous Frontier | 400,000 | 128,000 | Aug 31, 2025 | "Previous frontier model for professional work with configurable reasoning effort" | /models/gpt-5.2/ |
| `gpt-5.2-pro` | `gpt-5.2-pro-2025-12-11` | Previous Pro | 400,000 | 128,000 | Aug 31, 2025 | "Previous pro model ... smarter and more precise responses" | /models/gpt-5.2-pro/ |
| `gpt-5.1` | `gpt-5.1-2025-11-13` | Previous | 400,000 | 128,000 | Sep 30, 2024 | "Best model for coding and agentic tasks with configurable reasoning effort" | /models/gpt-5.1/ |
| `gpt-5` | `gpt-5-2025-08-07` | Previous | 400,000 | 128,000 | Sep 30, 2024 | "Previous intelligent reasoning model for coding and agentic tasks" | /models/gpt-5/ |
| `gpt-5-pro` | `gpt-5-pro-2025-10-06` | Previous Pro | 400,000 | 272,000 | Sep 30, 2024 | "Version of GPT-5 that produces smarter and more precise responses" | /models/gpt-5-pro/ |
| `gpt-5-mini` | `gpt-5-mini-2025-08-07` | Mini | 400,000 | 128,000 | May 31, 2024 | "Near-frontier intelligence for cost sensitive, low latency, high volume workloads" | /models/gpt-5-mini/ |
| `gpt-5-nano` | `gpt-5-nano-2025-08-07` | Nano | 400,000 | 128,000 | May 31, 2024 | "Fastest, most cost-efficient version of GPT-5" | /models/gpt-5-nano/ |

### o 系列推理模型（Chat Completions）

| Model ID | 日期 snapshot | 上下文窗口 | Max Output | 知识截止 | 用途 | 详情页 |
|---|---|---|---|---|---|---|
| `o3` | `o3-2025-04-16` | 200,000 | 100,000 | Jun 01, 2024 | "Reasoning model for complex tasks, succeeded by GPT-5" | /models/o3/ |
| `o3-pro` | `o3-pro-2025-06-10` | 200,000 | 100,000 | Jun 01, 2024 | "Version of o3 with more compute for better responses" | /models/o3-pro/ |

### GPT-4.1 / GPT-4o mini（仍可用）

| Model ID | 日期 snapshot | 上下文窗口 | Max Output | 知识截止 | 状态 | 详情页 |
|---|---|---|---|---|---|---|
| `gpt-4.1` | `gpt-4.1-2025-04-14` | 1,047,576 | 32,768 | Jun 01, 2024 | 可用（"Smartest non-reasoning model"） | /models/gpt-4.1/ |
| `gpt-4.1-mini` | `gpt-4.1-mini-2025-04-14` | 1,047,576 | 32,768 | Jun 01, 2024 | 可用 | /models/gpt-4.1-mini/ |
| `gpt-4o-mini` | `gpt-4o-mini-2024-07-18` | 128,000 | 16,384 | Oct 01, 2023 | 可用（注意：`gpt-4o` 已 Deprecated，但 mini 未弃用） | /models/gpt-4o-mini/ |

### 开放权重（Apache 2.0，OpenAI API 亦托管）

| Model ID | 说明 |
|---|---|
| `gpt-oss-120b` | "Most powerful open-weight model, fits into an H100 GPU" |
| `gpt-oss-20b` | "Medium-sized open-weight model for low latency" |

> 数据源：https://developers.openai.com/api/docs/models/all/ 「Open-weight models」段。

---

## 不进 model_list 的类别（排除理由）

| 类别 | 代表模型 | 排除理由 |
|---|---|---|
| Image generation | `gpt-image-2`、`gpt-image-1.5`(Deprecated)、`gpt-image-1`、`dall-e-3` | 走 `/v1/images/generations`，非 `/chat/completions` 文本任务 |
| Realtime / Audio | `gpt-realtime-2.1`、`gpt-realtime-2.1-mini`、`gpt-realtime-2`、`gpt-realtime-translate`、`gpt-realtime-whisper`、`gpt-realtime-1.5`、`gpt-realtime`、`gpt-realtime-mini`、`gpt-audio-1.5`、`gpt-audio`、`gpt-4o-transcribe`、`gpt-4o-mini-transcribe`、`gpt-4o-transcribe-diarize`、`tts-1`、`tts-1-hd`、`whisper` | 走 `/v1/realtime/*` 或 `/v1/audio/*`，非 chat 文本任务 |
| Embedding | `text-embedding-3-large`、`text-embedding-3-small`、`text-embedding-ada-002` | 走 `/v1/embeddings` |
| Moderation | `omni-moderation` | 走 `/v1/moderations` |
| ChatGPT-only | `chatgpt-latest`（"Chat Latest"，列在 "ChatGPT models"） | 官方标注「Models used in ChatGPT, not recommended for API use」 |
| 已弃用（已 sunset 或标 Deprecated） | `gpt-4o`、`gpt-4-turbo`、`gpt-4`、`gpt-4.5-preview`、`gpt-3.5-turbo`、`o1`、`o1-mini`、`o1-preview`、`o1-pro`、`o3-mini`、`o4-mini`、`o3-deep-research`、`o4-mini-deep-research`、`gpt-4.1-nano`、`computer-use-preview`、`gpt-4o-search-preview`、`gpt-4o-mini-search-preview`、`gpt-5.3-chat`、`gpt-5.2-chat`、`gpt-5.2-codex`、`gpt-5.1`-chat/-codex/-codex-max/-codex-mini、`gpt-5-chat`、`gpt-5-codex`、`codex-mini-latest`、`sora-2`、`sora-2-pro`、`babbage-002`、`davinci-002`、`chatgpt-4o`、`text-moderation*`、`gpt-image-1*`、`dall-e-2/3`、`gpt-4o-audio*`、`gpt-4o-realtime*`、`gpt-audio-mini` 等 | 见 https://developers.openai.com/api/docs/models/all/ 「Deprecated」标签 + https://developers.openai.com/api/docs/deprecations sunset 时间线 |

> 重要不对称：`gpt-4o` 已 Deprecated，但 `gpt-4o-mini` 仍可用（官方 All models 页面对比）；`o3` / `o3-pro` 未弃用，但 `o1` 全家、`o3-mini`、`o4-mini` 已 Deprecated。

---

## 现 preset 已有 vs 遗漏对比

### ✅ 已在 preset（4 个，全部最新一代 GPT-5.4/5.5）

`gpt-5.5`、`gpt-5.4`、`gpt-5.4-mini`、`gpt-5.4-nano`

### ❌ 遗漏（当前可用、走 Chat Completions、适合文本/代码任务的官方模型）

| 遗漏 Model ID | 类别 | 是否值得补 |
|---|---|---|
| `gpt-5.5-pro` | Frontier Pro 旗舰 | 强烈推荐 |
| `gpt-5.4-pro` | Frontier Pro | 强烈推荐 |
| `gpt-5.3-codex` | 最新 Codex 代理编程 | 推荐（编程场景） |
| `gpt-5.2` | 上一代 Frontier | 推荐 |
| `gpt-5.2-pro` | 上一代 Pro | 可选 |
| `gpt-5.1` | 编程/agentic 旧代 | 可选 |
| `gpt-5` | 旧代推理 | 可选 |
| `gpt-5-pro` | 旧代 Pro（272K output） | 可选 |
| `gpt-5-mini` | 上一代 mini | 可选 |
| `gpt-5-nano` | 上一代 nano | 可选 |
| `o3` | 推理模型（被 GPT-5 接继但仍可用） | 推荐 |
| `o3-pro` | 推理 Pro | 推荐 |
| `gpt-4.1` | 最强非推理 | 可选 |
| `gpt-4.1-mini` | 非推理 mini | 可选 |
| `gpt-4o-mini` | 快速廉价（gpt-4o 已弃用但 mini 未弃用） | 可选 |
| `gpt-oss-120b` | 开放权重旗舰 | 可选 |
| `gpt-oss-20b` | 开放权重低延迟 | 可选 |

---

## 推荐 `model_list.default` 最终清单

> 设计原则：`openai` 协议 `client_type=codex_tui`，面向 Codex CLI / 编程 / 通用 chat。优先收录**最新一代全档 + 上一代 Pro + 仍可用的推理 + 仍可用的非推理主力**。日期 snapshot **不进 list**（alias 自动指向最新，preset 约定）。

### Tier A：核心主力（强推荐加入，与现有 4 个并列）

```jsonc
"gpt-5.5",          // 已有
"gpt-5.5-pro",      // 新增 — Frontier Pro 旗舰
"gpt-5.4",          // 已有
"gpt-5.4-pro",      // 新增 — Frontier Pro
"gpt-5.4-mini",     // 已有
"gpt-5.4-nano",     // 已有
"gpt-5.3-codex",    // 新增 — 官方「最强 agentic 编程模型」，编程场景首选
"o3",               // 新增 — 当前可用的旗舰推理模型
"o3-pro"            // 新增 — 推理 Pro
```

### Tier B：兼容历史/廉价（可选加入）

```jsonc
"gpt-5.2",          // 上一代 Frontier，仍有用户
"gpt-5.2-pro",      // 上一代 Pro
"gpt-5.1",          // 编程/agentic 旧代
"gpt-5",            // 旧代推理
"gpt-5-pro",        // 旧代 Pro（max output 272K，长输出场景独有）
"gpt-5-mini",       // 上一代 mini，低延迟
"gpt-5-nano",       // 上一代 nano，最便宜
"gpt-4.1",          // 最强非推理（1M 上下文）
"gpt-4.1-mini",     // 非推理 mini
"gpt-4o-mini"       // 廉价小型（注意 gpt-4o 已弃用，仅 mini 仍可用）
```

### Tier C：开放权重（可选，看 OpenAI API 是否托管；若仅自托管则不进此 preset）

```jsonc
"gpt-oss-120b",
"gpt-oss-20b"
```

### 最终建议清单（Tier A，10 个）

```jsonc
"model_list": {
  "default": [
    "gpt-5.5",
    "gpt-5.5-pro",
    "gpt-5.4",
    "gpt-5.4-pro",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
    "gpt-5.3-codex",
    "o3",
    "o3-pro",
    // 若需向下兼容再追加 Tier B 选择项
  ]
}
```

> 注：`STATIC_MODEL_IDS`（passthrough.rs:238）若同步扩充需同样手工对齐；但其用途仅限 tokenless 探测，可保持精简（保留 4 个最新 GPT-5.x 即可，注释里写明）。`models.default.gpt=gpt-5.5` 维持不变（已是当前 alias）。

---

## Caveats / 不确定

1. **`gpt-5.4-pro` 在 All models 总览的「Frontier models」段未标 Deprecated**（与 `gpt-5.5-pro` 同列），且独立详情页 `/models/gpt-5.4-pro/` 返回 200 + 完整字段，确认当前可用。若官方实际仅通过 Pro access tier 开放，需账户 tier 满足才能调（不影响 preset 收录）。
2. **`gpt-4o` 与 `gpt-4o-mini` 不对称**：All models 页 `gpt-4o` 标 Deprecated，`gpt-4o-mini` 未标（"Fast, affordable small model for focused tasks"）。仅 `gpt-4o-mini` 进 Tier B。
3. **`gpt-5.3-codex` 详情页无日期 snapshot**（仅 `gpt-5.3-codex` 自身 alias，无 `-YYYY-MM-DD` 形式），可能尚未发布 snapshot 锁定版本；alias 调用照常工作。
4. **`gpt-oss-*` 是否经 OpenAI API 托管**：All models 页列在「Open-weight models」段，但未明说 OpenAI 平台是否提供 hosted 推理。若仅开源权重自部署，则不应进此 preset（用户需自备端点）。建议保守起见**先不收录**，待确认 OpenAI API 是否支持 `model="gpt-oss-120b"` 调用。
5. OpenAI 官方文档站为 Astro SSR，模型数据不在 HTML 内联 JSON 中（已尝试 `__NEXT_DATA__` / 直接 JSON 端点均失败）；唯一可靠结构化来源是单模型详情页 `/api/docs/models/{slug}/` 的 SSR 正文。本次研究通过批量抓取 19 个详情页 + 总览页 + deprecations 页交叉核对。
6. STATIC_MODEL_IDS 注释「月级腐化需手工核对，最近核对 2026-07-08」——本次研究恰好同期，结果与该注释一致。
