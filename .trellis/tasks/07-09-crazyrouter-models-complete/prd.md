# 补全 crazyrouter model_list+endpoints 全部官方信息

## Goal
CrazyRouter 是接入 20+ 供应商的聚合路由（官网宣称 627+ 模型）。定价 API `https://crazyrouter.com/api/pricing` 无需登录返回完整 165 个公开模型。当前 preset 仅含 7 个 Claude alias，作为聚合站严重缺失。本次改动：endpoints 3 端点全保留（已正确）、model_list.default 扩为全量 165 模型（裸 id）、models.default 补三档、desc/source_urls 保留（已准确）。数据强度：强（免鉴权定价 API + 文档示例双证）。

## Research References
- [`research/crazyrouter-models.md`](research/crazyrouter-models.md) — 定价 API 返回 165 模型；id 格式裸 id；三端点（anthropic/openai/gemini）全部 401 存活；cn. 子域为东亚优先入口

## Requirements

### 1. endpoints（default 分支，3 端点，全部保留）
现有三端点已正确指向 `cn.crazyrouter.com`（东亚优先），research 2026-07-09 curl 验证全部 401 存活，保留不改：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://cn.crazyrouter.com", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://cn.crazyrouter.com/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://cn.crazyrouter.com", "client_type": "default"}
  ]
}
```

> Anthropic base_url 必须根域（客户端自动追加 `/v1/messages`）；OpenAI 带 `/v1`；Gemini 根域。

### 2. model_list.default（165 模型，裸 id，按家族分组）
来源：`GET https://crazyrouter.com/api/pricing` 的 `data[].model_name` 字段（2026-07-09）。去重后 165 个：

**Claude（9）**
```
claude-fable-5, claude-haiku-4-5, claude-opus-4-5, claude-opus-4-6, claude-opus-4-7, claude-opus-4-8, claude-sonnet-4-5, claude-sonnet-4-6, claude-sonnet-5
```

**GPT 系（19，含 gpt-image-2）**
```
gpt-4.1, gpt-4.1-mini, gpt-4.1-nano, gpt-4o, gpt-4o-mini, gpt-5, gpt-5-codex, gpt-5-mini, gpt-5-nano, gpt-5.1, gpt-5.1-codex, gpt-5.1-codex-max, gpt-5.1-codex-mini, gpt-5.2-pro, gpt-5.4, gpt-5.4-pro, gpt-5.5, gpt-5.5-pro, gpt-image-2
```

**Gemini（7）**
```
gemini-2.5-flash, gemini-2.5-flash-lite, gemini-2.5-pro, gemini-3-flash, gemini-3.1-flash-lite, gemini-3.1-pro, gemini-3.5-flash
```

**DeepSeek（2）**
```
deepseek-v4-flash, deepseek-v4-pro
```

**Qwen（19，含 qwen-image-*）**
```
qwen-max, qwen-max-longcontext, qwen-plus, qwen-turbo, qwen-vl-max-latest, qwen-image-2.0, qwen-image-max, qwen-image-plus, qwen3-235b-a22b, qwen3-coder-plus, qwen3-max, qwen3-vl-flash, qwen3-vl-plus, qwen3.5-flash, qwen3.5-plus, qwen3.6-flash, qwen3.6-plus, qwen3.7-max, qwen3.7-plus
```

**GLM（7）**
```
glm-3-turbo, glm-5, glm-5-turbo, glm-5.1, glm-5.2, glm-5v-turbo, glm-ocr
```

**MiniMax（3）**
```
MiniMax-M2.5, MiniMax-M2.7, MiniMax-M3
```

**Kimi（3）**
```
kimi-k2-0905-preview, kimi-k2.5, kimi-k2.6
```

**Grok（8，research 原文 grok-4-0709 重复已去重）**
```
grok-4, grok-4-0709, grok-4-1-fast-non-reasoning, grok-4-1-fast-reasoning, grok-4-fast-non-reasoning, grok-4-fast-reasoning, grok-4.1, grok-4.2
```

**O 系列（6）**
```
o1, o1-mini, o3, o3-mini, o3-pro, o4-mini
```

**Llama（7）**
```
llama-3.1-405b, llama-3.1-70b, llama-3.1-8b, llama-3.2-11b-vision-instruct, llama-3.2-1b-instruct, llama-3.2-3b-instruct, llama-3.2-90b-vision-instruct
```

**Doubao / 字节系（15）**
```
doubao-seed-1-6-251015, doubao-seed-1-6-flash-250615, doubao-seed-1-6-flash-250828, doubao-seed-1-6-vision-250815, doubao-seed-code-preview-251028, doubao-seedance-1-0-lite-i2v, doubao-seedance-1-0-lite-t2v, doubao-seedance-1-0-pro, doubao-seedance-1-0-pro-fast, doubao-seedance-1-5-pro, doubao-seedance-2-0, doubao-seedance-2-0-fast, doubao-seedream-4-0, doubao-seedream-4-5, doubao-seedream-5-0
```

**Mimo（3）**
```
mimo-v2-flash, mimo-v2-omni, mimo-v2-pro
```

**视频（27）**
```
aigc-video-gv-3.1, aigc-video-gv-3.1-fast, aigc-video-gv-3.1-lite, aigc-video-kling-1.6, aigc-video-kling-2.0, aigc-video-kling-2.1, aigc-video-kling-2.5-turbo, aigc-video-kling-2.6, aigc-video-kling-2.6-motion-control, aigc-video-kling-3.0, aigc-video-kling-3.0-motion-control, aigc-video-kling-3.0-turbo, aigc-video-kling-avatar, aigc-video-kling-identifyface, aigc-video-kling-o1, kling-v2-5-turbo, kling-v2-6, kling-v3, veo-3.1, veo-3.1-fast, wan2.2-i2v-flash, wan2.2-i2v-plus, wan2.2-kf2v-flash, wan2.2-s2v, wan2.2-t2v-plus, wan2.5-t2v-preview, youtu-vita
```

**图像（10，跨家族重复的 gpt-image-2 / qwen-image-* 已归入原家族，此处仅列独有）**
```
aigc-image-kling-3.0, cogview-3-flash, cogview-4, dall-e-3, mj_imagine, mj_upscale, mj_variation, nano-banana, nano-banana-2, nano-banana-pro
```

**音频 / 音乐（5）**
```
suno_music, tts-1, tts-1-1106, tts-1-hd-1106, whisper-1
```

**Embedding / Rerank（4）**
```
gte-rerank-v2, text-embedding-3-large, text-embedding-3-small, text-embedding-v1
```

**Legacy / 特殊（10）**
```
babbage-002, chat-seededit, davinci-002, search, text-ada-001, text-babbage-001, text-curie-001, text-davinci-edit-001, text-moderation-latest, text-moderation-stable
```

> 去重后家族求和 = 165（9+19+7+2+19+7+3+3+8+6+7+15+3+27+10+5+4+10）。

### 3. models.default（三档，档位名 key → model id string）
aidog 真值格式 = `Partial<Record<ModelSlot, string>>`，key 是档位名（default/gpt/fast 等），value 是 model id 字符串。

```json
"models": {
  "default": {
    "default": "claude-opus-4-8",
    "gpt": "gpt-5.5",
    "fast": "deepseek-v4-flash"
  }
}
```

| 档位（key） | 模型（value） | 理由 |
|------|------|------|
| `default` | `claude-opus-4-8` | 官方旗舰，文档推荐示例默认，主力兜底 |
| `gpt` | `gpt-5.5` | GPT 非 mini 旗舰，文档推荐 "Default chat and general reasoning" |
| `fast` | `deepseek-v4-flash` | DeepSeek V4 Flash，轻量快速档 |

### 4. desc（保留，8 语言不动）
现有 "聚合路由, 接入多家模型供应商" 准确反映定位，保留。

### 5. source_urls（保留）
- docs: `https://docs.crazyrouter.com/`
- pricing: `https://crazyrouter.com/pricing`

## Acceptance Criteria
- [ ] endpoints.default 三端点不变（anthropic/openai/gemini 全部 cn.crazyrouter.com）
- [ ] model_list.default 含 165 个裸 id（按家族去重后）
- [ ] models.default 三档：default=claude-opus-4-8 / gpt=gpt-5.5 / fast=deepseek-v4-flash（档位名 key → string）
- [ ] desc 8 语言保留不动
- [ ] source_urls 保留
- [ ] JSON 合法
- [ ] 验证命令输出：`165 {'default': 'claude-opus-4-8', 'gpt': 'gpt-5.5', 'fast': 'deepseek-v4-flash'} 3`

## Out of Scope
- 上下文窗口 / STATIC_MODEL_IDS / peak_hours / coding_plan / 其他协议块 / id 日期后缀 / 鉴权 key 格式验证

## Technical Notes
- 真值源：`protocols.crazyrouter`
- 数据来源：定价 API（免鉴权 JSON）+ llms.txt 文档示例 + curl 端点存活测试；627 vs 165 差异推测为协议变体计数
- id 格式：裸 id（非 `provider/model`）
- research 原文 Grok 块 grok-4-0709 重复列两次，已去重
