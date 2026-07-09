# Research: CrazyRouter 全量模型清单

- **Query**: 调研 CrazyRouter (crazyrouter.com) 全量官方信息：平台定位、endpoints 核验、model_list 全量清单、models.default 三档推荐
- **Scope**: 外部调研（官网 + API 探测 + 定价 API）
- **Date**: 2026-07-09

## TL;DR 结论速览

| 项目 | 结论 |
|------|------|
| **平台定位** | 聚合路由（Aggregator）—— 接入 20+ 供应商、627+ 模型（官网宣称） |
| **endpoint 存活** | 三端点全部存活（401 返回 "未提供令牌"） |
| **多 host 布局** | `api.crazyrouter.com`（国际）/ `cn.crazyrouter.com`（东亚优先） |
| **模型总数** | 定价 API 返回 **165 个** 公开模型 |
| **id 格式** | 裸 id（如 `claude-opus-4-8`），无 `provider/` 前缀 |
| **现有 7 模型** | 缺 `claude-sonnet-5` / `claude-fable-5`，需补全 |

---

## API Endpoints

### 多 Host 布局

| Host | 用途 | 状态 |
|------|------|------|
| `api.crazyrouter.com` | 国际 API 入口（默认） | ✓ 存活 |
| `cn.crazyrouter.com` | 东亚优先入口 | ✓ 存活 |
| `crazyrouter.com` | 网站/控制台/定价页 | ✓ 存活 |
| `docs.crazyrouter.com` | 文档（Mintlify） | ✓ 存活 |

> 来源：llms.txt + API 端点文档
> `api.crazyrouter.com` 是 API 专用入口，根域返回 `api_only_endpoint` 错误提示用 `/v1` 路径。

### 三端点存活测试

| 协议 | 国际端点 | 东亚端点 | 状态 | curl 探测 |
|------|----------|----------|------|----------|
| Anthropic | `https://api.crazyrouter.com/v1/messages` | `https://cn.crazyrouter.com/v1/messages` | ✓ 401 | `{"error":{"message":"未提供令牌"}}` |
| OpenAI | `https://api.crazyrouter.com/v1/chat/completions` | `https://cn.crazyrouter.com/v1/chat/completions` | ✓ 401 | `{"error":{"message":"未提供令牌"}}` |
| Gemini | `https://api.crazyrouter.com/v1beta/models/{model}:generateContent` | `https://cn.crazyrouter.com/v1beta/models/{model}:generateContent` | ✓ 401 | `{"error":{"message":"未提供令牌"}}` |

> 测试时间：2026-07-09
> 401 错误表明端点存活且鉴权正常，缺 token 时返回中文 "未提供令牌"（`new_api_error` 类型）。

### 端点路径对照

| 用途 | 国际 | 东亚（优先） |
|------|------|--------------|
| OpenAI SDK | `https://api.crazyrouter.com/v1` | `https://cn.crazyrouter.com/v1` |
| Claude Code / Anthropic 原生 | `https://api.crazyrouter.com`（根域） | `https://cn.crazyrouter.com`（根域） |
| Gemini 原生 | `https://api.crazyrouter.com/v1beta/models/{model}:generateContent` | `https://cn.crazyrouter.com/v1beta/models/{model}:generateContent` |

> 警告：Anthropic/Claude Code 客户端会自动追加 `/v1/messages`，故 base_url 必须是根域（不含 `/v1`），否则会出现 `/v1/v1/messages` 双重路径错误。

---

## 模型范围确认

### 平台定位证据链

1. **homepage meta 描述**：
   > "Crazyrouter 汇聚 OpenAI、Claude、Gemini、Sora2、Kling、Suno 等 300+ 全球顶尖AI模型，统一API接口，一键调用。"
   
2. **llms.txt 首段**：
   > "Crazyrouter is a unified AI model API gateway for OpenAI-compatible, Claude, Gemini, image, video, audio, embeddings, rerank, and AI tool integrations."

3. **schema.org FAQ**：
   > "Crazyrouter is an AI API gateway that provides one API key to access 627+ AI models from 20+ providers including OpenAI, Anthropic, Google, and DeepSeek."

### 定价 API 作为真值源

llms.txt 明确：
> Use https://crazyrouter.com/pricing as the source of truth for public models, prices, and billing modes.

定价页是 SPA（React），数据端点：
```
GET https://crazyrouter.com/api/pricing
```

该端点无需登录即返回完整模型列表（JSON 格式，`data` 数组含 165 个模型对象）。

---

## 全量模型清单（按家族分组）

> 总计 **165 个** 模型（来源：`https://crazyrouter.com/api/pricing` 2026-07-09）

### Claude 系列（9 个）

```
claude-fable-5
claude-haiku-4-5
claude-opus-4-5
claude-opus-4-6
claude-opus-4-7
claude-opus-4-8
claude-sonnet-4-5
claude-sonnet-4-6
claude-sonnet-5
```

> 现有 preset 缺：`claude-fable-5`, `claude-sonnet-5`

### GPT 系列（17 个）

```
gpt-4.1
gpt-4.1-mini
gpt-4.1-nano
gpt-4o
gpt-4o-mini
gpt-5
gpt-5-codex
gpt-5-mini
gpt-5-nano
gpt-5.1
gpt-5.1-codex
gpt-5.1-codex-max
gpt-5.1-codex-mini
gpt-5.2-pro
gpt-5.4
gpt-5.4-pro
gpt-5.5
gpt-5.5-pro
gpt-image-2
```

### Gemini 系列（7 个）

```
gemini-2.5-flash
gemini-2.5-flash-lite
gemini-2.5-pro
gemini-3-flash
gemini-3.1-flash-lite
gemini-3.1-pro
gemini-3.5-flash
```

### DeepSeek（2 个）

```
deepseek-v4-flash
deepseek-v4-pro
```

### Qwen 系列（19 个）

```
qwen-max
qwen-max-longcontext
qwen-plus
qwen-turbo
qwen-vl-max-latest
qwen-image-2.0
qwen-image-max
qwen-image-plus
qwen3-235b-a22b
qwen3-coder-plus
qwen3-max
qwen3-vl-flash
qwen3-vl-plus
qwen3.5-flash
qwen3.5-plus
qwen3.6-flash
qwen3.6-plus
qwen3.7-max
qwen3.7-plus
```

### GLM 系列（6 个）

```
glm-3-turbo
glm-5
glm-5-turbo
glm-5.1
glm-5.2
glm-5v-turbo
glm-ocr
```

### MiniMax（3 个）

```
MiniMax-M2.5
MiniMax-M2.7
MiniMax-M3
```

### Kimi（3 个）

```
kimi-k2-0905-preview
kimi-k2.5
kimi-k2.6
```

### Grok（7 个）

```
grok-4
grok-4-0709
grok-4-0709
grok-4-1-fast-non-reasoning
grok-4-1-fast-reasoning
grok-4-fast-non-reasoning
grok-4-fast-reasoning
grok-4.1
grok-4.2
```

### O 系列（7 个）

```
o1
o1-mini
o3
o3-mini
o3-pro
o4-mini
```

### Llama（7 个）

```
llama-3.1-405b
llama-3.1-70b
llama-3.1-8b
llama-3.2-11b-vision-instruct
llama-3.2-1b-instruct
llama-3.2-3b-instruct
llama-3.2-90b-vision-instruct
```

### Doubao / 字节系（25 个）

```
doubao-seed-1-6-251015
doubao-seed-1-6-flash-250615
doubao-seed-1-6-flash-250828
doubao-seed-1-6-vision-250815
doubao-seed-code-preview-251028
doubao-seedance-1-0-lite-i2v
doubao-seedance-1-0-lite-t2v
doubao-seedance-1-0-pro
doubao-seedance-1-0-pro-fast
doubao-seedance-1-5-pro
doubao-seedance-2-0
doubao-seedance-2-0-fast
doubao-seedream-4-0
doubao-seedream-4-5
doubao-seedream-5-0
```

### Mimo（3 个）

```
mimo-v2-flash
mimo-v2-omni
mimo-v2-pro
```

### 视频（22 个）

```
aigc-video-gv-3.1
aigc-video-gv-3.1-fast
aigc-video-gv-3.1-lite
aigc-video-kling-1.6
aigc-video-kling-2.0
aigc-video-kling-2.1
aigc-video-kling-2.5-turbo
aigc-video-kling-2.6
aigc-video-kling-2.6-motion-control
aigc-video-kling-3.0
aigc-video-kling-3.0-motion-control
aigc-video-kling-3.0-turbo
aigc-video-kling-avatar
aigc-video-kling-identifyface
aigc-video-kling-o1
kling-v2-5-turbo
kling-v2-6
kling-v3
veo-3.1
veo-3.1-fast
wan2.2-i2v-flash
wan2.2-i2v-plus
wan2.2-kf2v-flash
wan2.2-s2v
wan2.2-t2v-plus
wan2.5-t2v-preview
youtu-vita
```

### 图像（12 个）

```
aigc-image-kling-3.0
cogview-3-flash
cogview-4
dall-e-3
gpt-image-2
mj_imagine
mj_upscale
mj_variation
nano-banana
nano-banana-2
nano-banana-pro
qwen-image-2.0
qwen-image-max
qwen-image-plus
```

### 音频 / 音乐（5 个）

```
suno_music
tts-1
tts-1-1106
tts-1-hd-1106
whisper-1
```

### Embedding / Rerank（4 个）

```
gte-rerank-v2
text-embedding-3-large
text-embedding-3-small
text-embedding-v1
```

### Legacy / 特殊（10 个）

```
babbage-002
chat-seededit
davinci-002
search
text-ada-001
text-babbage-001
text-curie-001
text-davinci-edit-001
text-moderation-latest
text-moderation-stable
```

---

## ID 格式判定

**结论：裸 id，无 `provider/` 前缀**

证据：
1. 定价 API `model_name` 字段全部为裸 id（如 `claude-opus-4-8`, `gpt-5.5`, `deepseek-v4-pro`）
2. 官网示例（llms.txt）：
   - `"model": "gpt-5.5"`（非 `openai/gpt-5.5`）
   - `"model": "claude-opus-4-8"`（非 `anthropic/claude-opus-4-8`）
3. 文档示例（latest-model-examples.md）全部使用裸 id

与其他聚合站（如 OpenRouter 用 `anthropic/claude-sonnet-4-6` 格式）不同，CrazyRouter 直接用官方 id。

---

## 现有 7 模型核对

### 当前 preset（src-tauri/defaults/platform-presets.json）

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

### 需增删

| 操作 | 模型 | 原因 |
|------|------|------|
| **+ 增** | `claude-fable-5` | 定价 API 有此模型，官方 Claude Fable 5 |
| **+ 增** | `claude-sonnet-5` | 定价 API 有此模型，官方 Claude Sonnet 5 |
| **- 删** | 无 | 现有 7 个全部有效 |

### 作为聚合站的模型覆盖问题

当前 preset 仅列 9 个 Claude 模型，作为聚合站严重缺失：
- 缺 GPT 全系列（17 个）
- 缺 Gemini 全系列（7 个）
- 缺 DeepSeek（2 个）
- 缺国产系（Qwen 19 / GLM 6 / MiniMax 3 / Kimi 3）
- 缺 Grok / O 系列 / Llama 等

**建议**：model_list.default 应覆盖主流聊天模型（GPT + Claude + Gemini + DeepSeek + 国产主力），或保持精简但文档注明 "仅部分示例，全量见定价页"。

---

## 三档默认推荐（models.default）

当前 preset `models.default` 为空 `{}`。

建议三档（按平台主力 + aidog 用户场景）：

```json
"models": {
  "default": {
    "claude-opus-4-8": {},
    "gpt-5.5": {},
    "deepseek-v4-flash": {}
  }
}
```

| 档位 | 模型 | 理由 |
|------|------|------|
| Claude 主力 | `claude-opus-4-8` | 官方旗舰，文档推荐示例默认 |
| GPT 旗舰 | `gpt-5.5` | 文档推荐 "Default chat and general reasoning" |
| 国产性价比 | `deepseek-v4-flash` | DeepSeek V4 Flash，低成本推理 |

> 格式：aidog 约定 model id 直接作 key（空 obj `{}`），禁用档位名作 key。

替代方案（按家族三选）：
- Claude: `claude-sonnet-4-6`（均衡）
- Gemini: `gemini-2.5-flash`（低成本）
- 国产: `glm-5.2` 或 `qwen3.7-max`

---

## Caveats / Not Found

### 627 vs 165 模型数量差异

- 官网首页宣称 "627+ AI models from 20+ providers"
- 定价 API `https://crazyrouter.com/api/pricing` 返回 **165 个** 公开模型
- **推测**: 627 可能包含：
  1. 不同端点类型的同一模型（如 `openai` / `anthropic` / `gemini` 协议分别计数）
  2. 私有 / 企业专属模型（不在公开定价页）
  3. 视频/图像任务变体（如 Kling 不同参数组合）

### 无 openrouter 风格前缀

与其他聚合站不同，CrazyRouter 不使用 `provider/model` 前缀格式：
- ❌ `anthropic/claude-sonnet-4-6`
- ✓ `claude-sonnet-4-6`

### cn. 子域首页 404

- `https://cn.crazyrouter.com` 根域返回 404（nginx 默认页）
- 但 API 端点（`/v1/messages`, `/v1/chat/completions`）**存活**
- 推测：`cn.crazyrouter.com` 仅用于 API，无 web 首页

### 鉴权方式

未在文档找到明确 key 格式说明：
- 401 错误返回中文 "未提供令牌"（非标准英文）
- 推测：`Authorization: Bearer sk-xxxxx` 格式（标准）
- 未验证是否支持 `x-api-key` 头

### 模型可用性

定价 API `enable_groups` 字段显示大部分模型在 `"default"` 组，但：
- 实际可用性依赖 token 权限（allowlist）
- 部分模型可能有地区限制
- 建议：生产前用 `GET https://api.crazyrouter.com/v1/models` 验证 token 可见模型

---

## 数据来源

| 项目 | URL / 命令 | 备注 |
|------|------------|------|
| 平台定位 | `https://crazyrouter.com` (homepage meta) | 2026-07-09 |
| llms.txt | `https://docs.crazyrouter.com/llms.txt` | 权威入口 |
| API 端点文档 | `https://docs.crazyrouter.com/en/api-endpoint.md` | 2026-06-16 更新 |
| 多 host 布局 | llms.txt + API 端点文档 | `api.crazyrouter.com` / `cn.crazyrouter.com` |
| 端点存活测试 | `curl -X POST https://cn.crazyrouter.com/v1/messages ...` | 401 = 存活 |
| 全量模型清单 | `https://crazyrouter.com/api/pricing` | JSON `data[].model_name` |
| 模型总数 | `curl -s "https://crazyrouter.com/api/pricing" \| jq -r '.data[].model_name' \| wc -l` | 165 个 |
| ID 格式 | 定价 API + 文档示例 | 裸 id |
| 推荐模型 | `https://docs.crazyrouter.com/en/latest-model-examples.md` | 2026-06-07 |
| 调研日期 | 2026-07-09 | - |
