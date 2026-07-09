# Research: StepFun（阶跃星辰）模型清单

- **Query**: 研究阶跃星辰 StepFun 全部官方模型清单、端点、认证方式（覆盖 stepfun 国内 + stepfun_en 国际两协议）
- **Scope**: external（外部文档搜索）
- **Date**: 2026-07-09

## 官方文档源

| URL | 用途 |
|------|------|
| https://platform.stepfun.com/docs | 国内站文档首页 |
| https://platform.stepfun.com/docs/zh/guides/models/overview | 模型能力总览 |
| https://platform.stepfun.com/docs/zh/guides/pricing/details | 国内站定价页 |
| https://platform.stepfun.com/docs/zh/guides/model-migration | 模型迁移/下线公告 |
| https://platform.stepfun.com/docs/zh/step-plan/overview | Step Plan 订阅制 |
| https://platform.stepfun.ai/docs | 国际站文档首页 |
| https://platform.stepfun.ai/docs/en/guides/pricing/details | 国际站定价页 |
| https://platform.stepfun.com/docs/zh/api-reference/models/list | Models List API |

## model_list 最终清单

### 主线文本推理模型（应并入 preset）

| Model ID | 状态 | Context | 类型 | 出处 |
|----------|------|---------|------|------|
| `step-3.7-flash` | **Stable（推荐）** | 256K | 多模态推理旗舰 | [pricing](https://platform.stepfun.com/docs/zh/guides/pricing/details), [overview](https://platform.stepfun.com/docs/zh/guides/models/step-3.7-flash) |
| `step-3.5-flash` | **Stable** | 256K | 推理旗舰 | [pricing](https://platform.stepfun.com/docs/zh/guides/pricing/details), [overview](https://platform.stepfun.com/docs/zh/guides/models/step-3.5-flash) |
| `step-3.5-flash-2603` | Stable | 256K | Agent 优化版 | [pricing](https://platform.stepfun.ai/docs/en/guides/pricing/details), [Step Plan](https://platform.stepfun.com/docs/zh/step-plan/overview) |
| `step-1o-turbo-vision` | **Stable（推荐）** | 32K | 视觉理解 | [pricing](https://platform.stepfun.com/docs/zh/guides/pricing/details), [vision](https://platform.stepfun.com/docs/zh/guides/models/vision) |

### 国内站 vs 国际站差异

- **模型差异**：无，双方文档列出的推理模型完全一致
- **定价差异**：国内站（人民币）vs 国际站（美元），汇率约 1:7，计费结构相同
  - step-3.7-flash 国内：输入 1.35元/M（缓存未命中）/ 0.27元/M（命中），输出 8.1元/M
  - step-3.7-flash 国际：输入 $0.20/M（缓存未命中）/ $0.04/M（命中），输出 $1.15/M

## models.default.default 推荐

**当前推荐**：`step-3.7-flash`

**理由**：
- 官方标记为「推荐使用」([vision](https://platform.stepfun.com/docs/zh/guides/models/vision))
- 阶跃星辰旗舰多模态推理模型，原生支持图片/视频理解
- 支持 256K 上下文，三档推理强度（low/medium/high）
- Step Plan 订阅制默认支持此模型
- 是已下线 `step-3` 的官方替代([migration](https://platform.stepfun.com/docs/zh/guides/model-migration))

## endpoints

### stepfun 国内（api.stepfun.com）

| Protocol | Base URL | Client Type | 用途 |
|----------|----------|-------------|------|
| `openai` | `https://api.stepfun.com/v1` | codex_tui | OpenAI 兼容 Chat Completions |
| `anthropic` | `https://api.stepfun.com/step_plan` | claude_code | **Anthropic 兼容 / Claude Code 套餐专用**（注意：路径为 `/step_plan`，非 `/v1`） |

**step_plan 路径说明**：
- `/step_plan` 是 Step Plan 订阅制服务的 Anthropic 兼容端点
- Step Plan 是订阅制服务，通过专用 API Key 接入，按 Credit 统一计费
- 适用于 Claude Code、OpenClaw、Cursor 等主流编码工具
- Step Plan 普通端点：`https://api.stepfun.com/step_plan/v1`（OpenAI 兼容）

### stepfun_en 国际（api.stepfun.ai）

| Protocol | Base URL | Client Type | 用途 |
|----------|----------|-------------|------|
| `openai` | `https://api.stepfun.ai/v1` | codex_tui | OpenAI 兼容 Chat Completions |
| `anthropic` | `https://api.stepfun.ai/step_plan` | claude_code | **Anthropic 兼容 / Claude Code 套餐专用** |

**确认**：国际站路径结构完全一致，仅域名差异。

## 认证方式

- **类型**：API Key
- **Header 格式**：`Authorization: Bearer $STEP_API_KEY`
- **出处**：[API 文档](https://platform.stepfun.com/docs/zh/api-reference/models/list)

## 非主线模型区（不并入 model_list）

### 语音模型（Audio）

| Model ID | 类型 | 用途 |
|----------|------|------|
| `stepaudio-2.5-realtime` | 端到端实时语音 | 实时语音对话，活人感 + 副语言感知 |
| `stepaudio-2.5-chat` | 端到端语音对话 | 文本返回 + 副语言感知 |
| `stepaudio-2.5-tts` | Contextual TTS | 语境感知语音合成 |
| `step-tts-2` | TTS | 新一代文本转语音 |
| `step-tts-mini` | TTS | 基础文本转语音 |
| `stepaudio-2.5-asr` | ASR | 流式语音识别 |
| `stepaudio-2.5-asr-stream` | ASR | 流式语音识别 |
| `stepaudio-2-asr-pro` | ASR Pro | 32B 参数 ASR |
| `step-asr` | ASR | 基础语音识别 |
| `step-asr-1.1` | ASR | 语音识别 |
| `step-asr-1.1-stream` | ASR | 流式语音识别 |
| `step-1o-audio` | 端到端语音 | 端到端语音模型 |
| `step-audio-2` | 端到端语音 | 端到端语音模型 |
| `step-audio-r1.5` | 端到端语音 | 端到端语音模型 |

### 图像模型（Image）

| Model ID | 类型 | 用途 |
|----------|------|------|
| `step-2x-large` | 文生图 | 图像生成 |
| `step-image-edit-2` | 文生图 + 编辑 | 图像生成与编辑一体化 |
| `step-1x-edit` | 图像编辑 | 图像编辑、人像美化（已下线？） |

### 智能路由

| Model ID | 用途 |
|----------|------|
| `step-router-v1` | 智能路由，按任务复杂度自动调度 |

## 排除项与原因

### 已 Deprecated（2026-07-08 下线）

以下模型已于 2026 年 07 月 08 日正式下线，停止推理服务：

| 下线模型 | 推荐替代 | 原因 |
|----------|----------|------|
| `step-1-8k` | `step-1o-turbo-vision` | 模型迁移([migration](https://platform.stepfun.com/docs/zh/guides/model-migration)) |
| `step-1-32k` | `step-1o-turbo-vision` | 同上 |
| `step-1v-8k` | `step-1o-turbo-vision` | 同上 |
| `step-1v-32k` | `step-1o-turbo-vision` | 同上 |
| `step-2-mini` | `step-1o-turbo-vision` | 同上 |
| `step-1o-vision-32k` | `step-1o-turbo-vision` | 同上 |
| `step-2-16k` | `step-1o-turbo-vision` | 同上 |
| `step-3` | `step-3.7-flash` | 同上 |
| `step-1x-medium` | `step-2x-large` | 同上 |

### 视觉/语音/图像模型（非主线）

- **视觉模型**：`step-1o-turbo-vision` 是视觉理解模型，但已被纳入主线（因是 step-1/step-2 系列的统一替代）
- **语音模型**：全部 TTS/ASR/端到端语音模型，因非文本推理场景，不并入 model_list
- **图像模型**：文生图/图像编辑模型，不并入 model_list

## caveats / 需要 main 关注

### 1. 模型列表完整性

当前 preset 中 `model_list` 只有 `step-3.7-flash` 和 `step-3.5-flash`，**遗漏**：
- `step-3.5-flash-2603`（Agent 优化版，Step Plan 专供）
- `step-1o-turbo-vision`（视觉理解，step-1/step-2 系列的统一替代）

### 2. 已下线模型清理

preset 中可能存在历史遗留的已下线模型（如 `step-3`），需根据迁移指南清理。

### 3. step_plan 路径特殊性

- Anthropic 兼容端点使用 `/step_plan` 路径（非 `/v1`）
- Step Plan 是订阅制专用端点，与按量付费的 `/v1` 并存
- 国际站路径结构一致，仅域名差异

### 4. 定价差异

- 国内站：人民币计费
- 国际站：美元计费（约为国内站 1/7，汇率关系）
- Step Plan 订阅制：Credit 统一计费，1M Credit = ¥1

### 5. 需验证项

- [ ] step-3.5-flash-2603 是否应并入 model_list（Step Plan 专供，但 API 可调用）
- [ ] step-1o-turbo-vision 在 preset 中的当前状态（是否已存在）
- [ ] 已下线模型是否仍在 preset 中，需清理

## 推测: 未在官方文档明确列出的模型

以下模型未在当前官方文档中找到明确说明，推测状态：
- `step-1-flash`：未在文档中出现，可能已下线或并入其他模型
- `step-1-200k`：未在文档中出现
- `step-1-256k`：未在文档中出现
- `step-coder`：未在文档中出现，step-3.7-flash / step-3.5-flash 已覆盖 coding 场景

以上推测需通过实际调用 `GET /v1/models` API 验证。

## 参考输出（Models List API）

```json
{
  "object": "list",
  "data": [
    {
      "id": "step-3.7-flash",
      "object": "model",
      "created": 1713196800,
      "owned_by": "stepai"
    },
    {
      "id": "step-3.5-flash",
      "object": "model",
      "created": 1713974400,
      "owned_by": "stepai"
    },
    {
      "id": "step-3.5-flash-2603",
      "object": "model",
      "created": 1711015200,
      "owned_by": "stepai"
    },
    {
      "id": "step-1o-turbo-vision",
      "object": "model",
      "created": 1711015200,
      "owned_by": "stepai"
    }
  ]
}
```

出处：[API 文档](https://platform.stepfun.com/docs/zh/api-reference/models/list)
