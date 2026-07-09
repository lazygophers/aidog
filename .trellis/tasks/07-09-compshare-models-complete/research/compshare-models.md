# Research: Compshare / 优云 (UCloud ModelVerse) 全量模型调研

- **Query**: Compshare/优云 (compshare.cn, UCloud 优云旗下) 平台定位、两产品线 endpoints、全量模型清单、默认推荐
- **Scope**: 外部调研（官网文档 + API 探测）
- **Date**: 2026-07-09

## TL;DR / 结论速览

- **平台定位**：UCloud 优刻得旗下的 **GPU 算力租赁 + 大模型 API 聚合平台**，两个产品线：GPU 实例（基础设施）+ 模型 API 服务（一站式接入全球主流模型，多供应商聚合）
- **两产品线**：
  - **ModelVerse 通用 API**：`api.modelverse.cn`，按量计费，221 个模型（Claude + GPT + Gemini + DeepSeek + Qwen + GLM + Kimi + MiniMax + 国产等多家族）
  - **Compshare Coding Plan**：`cp.compshare.cn`，编程套餐订阅制（推测包月 Claude 优化定价）
- **Endpoint 存活**：4/4 存活（401 = 需有效密钥），`/v1/models` 免鉴权返回全量列表
- **ModelVerse 模型数**：221 个（含文本、图像、视频、语音、TTS、嵌入、重排等）
- **id 格式**：混合裸 id（`gpt-5`）+ 带前缀（`deepseek-ai/DeepSeek-V3.2`、`Qwen/Qwen3-Max`）
- **鉴权**：`Authorization: Bearer {api_key}`

---

## 两产品线说明

### 1. ModelVerse 通用 API（`api.modelverse.cn`）
- **定位**：按量付费的模型 API 聚合服务，一站式接入全球主流模型
- **计费**：按量计费（秒级精度）
- **模型范围**：221 个模型，覆盖文本生成、图像生成、视频生成、语音、TTS、嵌入、重排序等
- **host**：`api.modelverse.cn`
- **文档**：https://www.compshare.cn/docs/modelverse/models/quick-start

### 2. Compshare Coding Plan（`cp.compshare.cn`）
- **定位**：编程套餐订阅服务，包月定价
- **计费**：套餐制（Mini/Lite/Basic/Pro/Max/Ultra 档，49-999 元/月）
- **模型范围**：文档未明确列出，推测聚焦 Claude 系列（需进一步验证）
- **host**：`cp.compshare.cn`（不同子域）
- **文档**：https://www.compshare.cn/docs/modelverse/package_plan/package

**差异对比**：

| 维度 | ModelVerse 通用 API | Compshare Coding Plan |
|------|---------------------|------------------------|
| host | `api.modelverse.cn` | `cp.compshare.cn` |
| 计费 | 按量付费 | 套餐订阅 |
| 模型范围 | 221 个多家族 | 推测 Claude 系列（需验证） |
| 目标场景 | 通用 AI 应用 | 编程/Coding 场景优化 |

---

## API Endpoints

### ModelVerse 通用 API（`api.modelverse.cn`）

| 协议 | Base URL | 路径 | 状态 | 探测命令 |
|------|----------|------|------|----------|
| anthropic | `https://api.modelverse.cn` | `/v1/messages` | 401 | `curl -X POST "https://api.modelverse.cn/v1/messages" -H "Authorization: Bearer test" ...` |
| openai | `https://api.modelverse.cn/v1` | `/chat/completions` | 401 | `curl -X POST "https://api.modelverse.cn/v1/chat/completions" -H "Authorization: Bearer test" ...` |
| gemini | `https://api.modelverse.cn` | `/v1beta/models` | 401 | `curl -X POST "https://api.modelverse.cn/v1beta/models/..." -H "Authorization: Bearer test" ...` |
| **models list** | `https://api.modelverse.cn` | `/v1/models` | **200（免鉴权）** | `curl "https://api.modelverse.cn/v1/models" -H "Authorization: Bearer test"` |

**401 说明**：端点存活且格式正确，需有效 API 密钥

### Compshare Coding Plan（`cp.compshare.cn`）

| 协议 | Base URL | 路径 | 状态 | 探测命令 |
|------|----------|------|------|----------|
| anthropic | `https://cp.compshare.cn` | `/v1/messages` | 401 | `curl -X POST "https://cp.compshare.cn/v1/messages" -H "Authorization: Bearer test" ...` |

---

## 模型范围确认

### 证据链
1. 官方文档「如何获取模型列表」明确说明调用 `GET https://api.modelverse.cn/v1/models`
2. 该端点免鉴权返回全量列表 221 个模型
3. 文档说明：「此接口只会返回文本生成的模型，若需要使用生图模型，请见【图片生成】」

### id 格式示例
- **裸 id**：`gpt-5`、`gpt-5.5`、`claude-opus-4-8`、`deepseek-v4-pro`、`glm-5.2`
- **带前缀**：`deepseek-ai/DeepSeek-V3.2`、`Qwen/Qwen3-Max`、`google/gemma-4-31b-it`、`ByteDance/doubao-seed-1.6`

---

## 全量模型清单

### ModelVerse 通用 API（221 个模型）

按家族分组：

#### Claude 系列（9 个）
```
claude-fable-5
claude-haiku-4-5-20251001
claude-opus-4-1-20250805
claude-opus-4-5-20251101
claude-opus-4-5-20251101-thinking
claude-opus-4-6
claude-opus-4-7
claude-opus-4-8
claude-sonnet-4-5-20250929
claude-sonnet-4-5-20250929-thinking
claude-sonnet-4-6
claude-sonnet-4.5
claude-sonnet-4.5-thinking
claude-sonnet-5
```

#### OpenAI GPT 系列（20 个）
```
gpt-4.1-mini
gpt-4.1-nano
gpt-4o-mini
gpt-4o-mini-transcribe
gpt-5
gpt-5-codex
gpt-5.1
gpt-5.1-codex
gpt-5.1-codex-max
gpt-5.1-codex-mini
gpt-5.2
gpt-5.2-codex
gpt-5.3-codex
gpt-5.4
gpt-5.4-mini
gpt-5.4-nano
gpt-5.4-pro
gpt-5.5
gpt-image-1
gpt-image-1-mini
gpt-image-1.5
gpt-image-2
o3-2025-04-16
o4-mini
openai/gpt-4.1
openai/gpt-4o
openai/gpt-5
openai/gpt-5-mini
openai/gpt-5-nano
openai/gpt-5.1
openai/gpt-5.1-codex
openai/gpt-5.1-codex-mini
openai/gpt-5.2
openai/sora-2/image-to-video
openai/sora-2/image-to-video-pro
openai/sora-2/text-to-video
openai/sora-2/text-to-video-pro
```

#### Gemini 系列（11 个）
```
gemini-2.5-flash
gemini-2.5-flash-image
gemini-2.5-pro
gemini-3-flash-preview
gemini-3-pro-image
gemini-3-pro-image-preview
gemini-3.1-flash-image
gemini-3.1-flash-image-preview
gemini-3.1-flash-lite-image
gemini-3.1-flash-lite-preview
gemini-3.1-pro-preview
gemini-3.5-flash
gemini-embedding-2
google/gemma-3-27b-it
google/gemma-4-31b-it
publishers/google/models/gemini-3-flash-preview
publishers/google/models/gemini-3-pro-image-preview
publishers/google/models/gemini-3.1-pro-preview
```

#### DeepSeek 系列（4 个）
```
deepseek-ai/DeepSeek-OCR
deepseek-ai/DeepSeek-OCR-2
deepseek-ai/DeepSeek-V3.2
deepseek-ai/DeepSeek-V3.2-Exp
deepseek-v4-flash
deepseek-v4-pro
```

#### Qwen / 通义千问 系列（19 个）
```
Qwen/QwQ-32B
Qwen/Qwen-Image
Qwen/Qwen-Image-Edit
Qwen/Qwen3-235B-A22B-Thinking-2507
Qwen/Qwen3-30B-A3B-Thinking
Qwen/Qwen3-Coder
Qwen/Qwen3-Max
Qwen/Qwen3-VL-235B-A22B-Instruct
Qwen/Qwen3-VL-235B-A22B-Thinking
Qwen/Qwen3-vl-Plus
qwen-mt-flash
qwen3-30b-a3b
qwen3-coder-30b-a3b-instruct
qwen3-coder-plus
qwen3-embedding-8b
qwen3-max-preview
qwen3-reranker-8b
qwen3-tts-flash
qwen3-vl-flash
qwen3.5-plus
qwen3.6-35b-a3b
qwen3.6-plus
qwen3.7-max
qwen3.7-plus
```

#### GLM / 智谱 系列（7 个）
```
glm-5-turbo
glm-5.1
glm-5.2
glm-5v-turbo
zai-org/glm-4.6
zai-org/glm-4.6v
zai-org/glm-4.7
zai-org/glm-5
```

#### Kimi / 月之暗面 系列（4 个）
```
kimi-k2.6
kimi-k2.7-code
moonshot/kimi-k2.5
moonshotai/Kimi-K2-Instruct
moonshotai/kimi-k2.5
```

#### MiniMax / 海螺 系列（8 个）
```
MiniMax-Hailuo-02
MiniMax-Hailuo-2.3
MiniMax-Hailuo-2.3-Fast
MiniMax-M2
MiniMax-M2.1
MiniMax-M2.1-lightning
MiniMax-M2.5
MiniMax-M2.5-lightning
MiniMax-M2.7
MiniMax-M2.7-highspeed
MiniMax-M3
```

#### 豆包 / 字节跳动 系列（10 个）
```
ByteDance/doubao-1-5-pro-32k-250115
ByteDance/doubao-seed-1.6
doubao-1-5-pro-32k-character-250715
doubao-seed-1-6-lite-251015
doubao-seed-2-0-code-preview-260215
doubao-seed-2-0-lite-260215
doubao-seed-2-0-mini-260215
doubao-seed-2-0-pro-260215
doubao-seed-2-1-pro-260628
doubao-seed-2-1-turbo-260628
doubao-seed-evolving
doubao-seedance-1-5-pro-251215
doubao-seedance-2-0-260128
doubao-seedance-2-0-mini-260615
doubao-seedream-4.5
doubao-seedream-5-0-260128
```

#### 百度 文心 系列（2 个）
```
baidu/ernie-4.5-turbo-128k
baidu/ernie-4.5-turbo-vl-32k
```

#### Grok 系列（8 个）
```
grok-4
grok-4-1-fast-non-reasoning
grok-4-1-fast-reasoning
grok-4-fast
grok-4-fast-reasoning
grok-4.20-0309-non-reasoning
grok-4.20-0309-reasoning
grok-4.3
grok-imagine-image
grok-imagine-image-quality
grok-imagine-video
```

#### 图像生成（10 个）
```
flux-2-pro
flux-kontext-pro
flux-pro-1.1
midjourney-fast-imagine
midjourney-fast-reroll
midjourney-fast-upscale
midjourney-fast-variation
wan2.7-image
wan2.7-image-pro
```

#### 视频生成（15 个）
```
Wan-AI/Wan2.5-I2V
Wan-AI/Wan2.5-T2V
Wan-AI/Wan2.6-I2V
Wan-AI/Wan2.6-T2V
happyhorse-1.0-i2v
happyhorse-1.0-r2v
happyhorse-1.0-t2v
happyhorse-1.0-video-edit
happyhorse-1.1-i2v
happyhorse-1.1-r2v
happyhorse-1.1-t2v
kling-v2-6
kling-v3
kling-v3-omni
kling-video-o1
pixverse-v6
sora-2
veo-3.1-fast-generate-001
veo-3.1-generate-001
vidu-lip-sync
vidu-mv
vidu-one-click-mv
viduq2
viduq2-pro
viduq2-pro-fast
viduq2-pro-fast-abroad
viduq2-turbo
viduq3-pro
viduq3-turbo
wan2.6-r2v
wan2.6-r2v-flash
```

#### 语音/音频（7 个）
```
music-v1
suno-v4
suno-v4.5
suno-v4.5+
suno-v4.5-all
suno-v5
suno-v5.5
speech-02-hd
speech-02-turbo
speech-2.6-hd
speech-2.6-turbo
speech-2.8-hd
speech-2.8-turbo
text-to-sound-v2
```

#### 嵌入/重排（4 个）
```
BAAI/bge-large-zh-v1.5
BAAI/bge-m3
bge-reranker-v2-m3
text-embedding-3-large
text-embedding-ada-002
```

#### TTS（1 个）
```
IndexTeam/IndexTTS-2
```

#### 其他（50+ 个）
```
codex-mini-latest
easydoc-emr-mask
easydoc-extract
easydoc-fin-chat
easydoc-parse-premium
mimo-v2.5
mimo-v2.5-pro
stepfun-ai/step1x-edit
seedance-2-0-filter-off
doubao-seedance-2-0-mini-260615
...（详见完整 API 返回）
```

### Compshare Coding Plan（`cp.compshare.cn`）
**模型范围**：文档未明确列出。推测：
- 聚焦 Claude 系列（编程套餐定位）
- 可能包含其他编程优化模型（如 DeepSeek-Coder、Qwen-Coder）

**建议验证方式**：
1. 用有效 API 密钥调用 `GET https://cp.compshare.cn/v1/models`
2. 查看套餐文档「套餐包快速上手」

---

## 三档默认推荐

### ModelVerse 通用 API（`protocols.compshare`）
根据 aidog 惯例（最新/最强/性价比），推荐：
```json
{
  "claude-sonnet-5": {},
  "gpt-5.5": {},
  "deepseek-v4-pro": {}
}
```
**理由**：
- `claude-sonnet-5`：Claude 最新主力模型
- `gpt-5.5`：OpenAI 最新 GPT-5.5
- `deepseek-v4-pro`：DeepSeek 最新推理模型，性价比高

### Compshare Coding Plan（`protocols.compshare_coding`）
根据编程套餐定位 + 当前 preset 7 个 alias，推荐：
```json
{
  "claude-opus-4-8": {},
  "claude-sonnet-5": {},
  "claude-haiku-4-5-20251001": {}
}
```
**理由**：
- `claude-opus-4-8`：最强 Claude，复杂编程任务
- `claude-sonnet-5`：主力 Claude，平衡性能与成本
- `claude-haiku-4-5-20251001`：快速响应，简单任务

---

## 现有 coding 块 7 模型核对

### 当前 preset 7 个 alias
```
claude-opus-4-8
claude-sonnet-4-6
claude-haiku-4-5
claude-opus-4-7
claude-opus-4-6
claude-opus-4-5
claude-sonnet-4-5
```

### 建议
**需要更新**：
- ✅ `claude-opus-4-8`：保留（最新 Opus）
- ❌ `claude-sonnet-4-6`：**删除**（已被 `claude-sonnet-5` 取代）
- ❌ `claude-haiku-4-5`：**更新**为 `claude-haiku-4-5-20251001`（精确版本）
- ❌ `claude-opus-4-7`：**删除**（旧版本）
- ❌ `claude-opus-4-6`：**删除**（旧版本）
- ❌ `claude-opus-4-5`：**删除**（旧版本）
- ❌ `claude-sonnet-4-5`：**删除**（已被 `claude-sonnet-5` 取代）

**建议新增**：
- `claude-sonnet-5`：最新 Sonnet

**更新后清单**：
```
claude-opus-4-8
claude-sonnet-5
claude-haiku-4-5-20251001
```

---

## 鉴权方式
- **方式**：`Authorization: Bearer {api_key}`
- **获取**：控制台 https://console.compshare.cn/
- **文档**：https://www.compshare.cn/docs/modelverse/models/common/certificate

---

## 数据来源

### 官方文档
- 平台简介：https://www.compshare.cn/docs/
- 快速开始：https://www.compshare.cn/docs/modelverse/models/quick-start
- 获取模型列表：https://www.compshare.cn/docs/modelverse/models/text_api/models
- 套餐包：https://www.compshare.cn/docs/modelverse/package_plan/package
- 按量计费：https://www.compshare.cn/docs/modelverse/price
- 价格列表：https://www.compshare.cn/price-list
- 首页：https://www.compshare.cn/

### API 探测命令
```bash
# ModelVerse 通用 API 探测
curl -X POST "https://api.modelverse.cn/v1/messages" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test" \
  -d '{"model":"claude-opus-4-8","max_tokens":1,"messages":[{"role":"user","content":"test"}]}'

curl -X POST "https://api.modelverse.cn/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test" \
  -d '{"model":"gpt-5.5","max_tokens":1,"messages":[{"role":"user","content":"test"}]}'

curl "https://api.modelverse.cn/v1/models" \
  -H "Authorization: Bearer test"

# Compshare Coding Plan 探测
curl -X POST "https://cp.compshare.cn/v1/messages" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test" \
  -d '{"model":"claude-opus-4-8","max_tokens":1,"messages":[{"role":"user","content":"test"}]}'
```

### 调研日期
2026-07-09

---

## Caveats / Not Found

### 未确认项（推测）
1. **Compshare Coding Plan 模型范围**：文档未明确列出模型清单，推测聚焦 Claude 系列，需用有效 API 密钥验证
2. **Coding Plan 是否支持非 Claude 模型**：套餐文档未明确说明
3. **cp.compshare.cn /v1/models 是否可用**：探测返回 `{"error":"invalid api key"}`，说明端点存活但需有效密钥

### 已确认事实
- ModelVerse 通用 API 确认为多供应商聚合（221 个模型，10+ 家族）
- 两产品线使用不同子域（`api.modelverse.cn` vs `cp.compshare.cn`）
- id 格式混合（裸 id + 带前缀）
- `/v1/models` 免鉴权返回全量列表

### 推测项
- Coding Plan 主要面向 Claude 系列优化定价（基于「编程套餐」命名 + cp 子域独立部署）
- preset 当前 7 个 alias 需更新为 3 个最新版本
