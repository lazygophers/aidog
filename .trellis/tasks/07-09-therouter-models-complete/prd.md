# 补全 therouter model_list+endpoints 全部官方信息

## Goal

TheRouter (therouter.ai) 是 **三协议聚合网关**（anthropic + openai chat/responses + gemini 原生），**291 模型 / 28 provider**（2026-07-09 实测探测 + 官方清单）。当前 preset 仅 1 个 anthropic endpoint + 空 model_list。base_url `https://api.therouter.ai` 正确可用（实测 `/v1/messages` 路由存在返 401）。需补全 endpoints（加 openai + gemini）+ 全量主力 model_list + 三档默认。

## Research References

- [`research/therouter-models.md`](research/therouter-models.md) — 291 模型 28 provider 逐项 + 三协议 endpoint 实测 + canonical id 格式 `provider/model-id`

## Requirements

### 1. endpoints（default 分支，补全 3 协议）

现有 1 anthropic endpoint base_url 正确。**新增 openai + gemini**：

```json
"endpoints": {
  "default": [
    {"protocol": "anthropic", "base_url": "https://api.therouter.ai", "client_type": "claude_code"},
    {"protocol": "openai", "base_url": "https://api.therouter.ai/v1", "client_type": "codex_tui"},
    {"protocol": "gemini", "base_url": "https://api.therouter.ai", "client_type": "default"}
  ]
}
```

base_url 规则：openai 必带 `/v1`；anthropic/gemini 仅 host（框架自拼 `/v1/messages` 与 `/v1beta/models/{model}:generateContent`）。

### 2. model_list.default（全量对话/coding/推理主力，`provider/model-id` 单斜杠格式）

从 research 291 清单提取**全部文本对话 / coding / 推理 / 多模态文本模型**，排除：纯 embedding / image 生成 / audio(TTS/whisper/transcribe) / video(sora) / rerank / safety(guard) / reward / OCR / search-pro。

**Anthropic（14 全）**：claude-fable-5 / claude-haiku-4.5 / claude-opus-4 / claude-opus-4.1 / claude-opus-4.5 / claude-opus-4.5-20251101 / claude-opus-4.6 / claude-opus-4.6-thinking / claude-opus-4.7 / claude-opus-4.8 / claude-sonnet-4 / claude-sonnet-4.5 / claude-sonnet-4.6 / claude-sonnet-5

**OpenAI（对话+coding+推理，约 22）**：gpt-4.1 / gpt-4.1-mini / gpt-4.1-nano / gpt-4o / gpt-4o-mini / gpt-5 / gpt-5-chat / gpt-5-codex / gpt-5-mini / gpt-5-nano / gpt-5-pro / gpt-5.1 / gpt-5.1-chat / gpt-5.1-codex / gpt-5.1-codex-max / gpt-5.1-codex-mini / gpt-5.2 / gpt-5.2-chat / gpt-5.2-codex / gpt-5.2-pro / gpt-5.3-chat-latest / gpt-5.3-codex / gpt-5.4 / gpt-5.4-mini / gpt-5.4-nano / gpt-5.4-pro / gpt-5.5 / gpt-5.5-pro / o1 / o3 / o3-deep-research / o3-pro / o4-mini / o4-mini-deep-research / gpt-oss-120b / gpt-oss-20b
（排除 chatgpt-image/gpt-image/gpt-audio/sora/tts/whisper/transcribe/embedding/safeguard）

**Google（对话+多模态文本，10）**：gemini-2.5-flash / gemini-2.5-flash-lite / gemini-2.5-pro / gemini-3.1-flash-lite / gemini-3.5-flash / gemma-2-9b-it / gemma-3-12b / gemma-3-27b / gemma-3-27b-it / gemma-3-4b / gemma-4-26b-a4b-it / gemma-4-31b-it
（排除 gemini-3-pro-image / gemini-3.1-flash-image 纯图像）

**DeepSeek（11 全）**：deepseek-r1 / deepseek-r1-0528 / deepseek-v3-0324 / deepseek-v3.1 / deepseek-v3.1-terminus / deepseek-v3.1-think / deepseek-v3.2 / deepseek-v3.2-exp / deepseek-v3.2-exp-think / deepseek-v4-flash / deepseek-v4-pro

**Qwen（对话+coder+VL，约 30）**：qwen-flash / qwen-long / qwen-max / qwen-plus / qwen-plus-thinking / qwen2.5-vl-72b-instruct / qwen3-14b / qwen3-235b / qwen3-235b-a22b-thinking-2507 / qwen3-235b-thinking / qwen3-30b / qwen3-30b-a3b-thinking-2507 / qwen3-30b-thinking / qwen3-32b / qwen3-32b-thinking / qwen3-8b / qwen3-coder-30b / qwen3-coder-480b / qwen3-coder-plus / qwen3-max / qwen3-next-80b / qwen3-vl-235b / qwen3-vl-235b-a22b-thinking / qwen3-vl-32b / qwen3-vl-8b / qwen3-vl-flash / qwen3-vl-plus / qwen3.5-flash / qwen3.5-plus / qwen3.6-27b / qwen3.6-flash / qwen3.6-max-preview / qwen3.6-plus / qwen3.7-max / qwen3.7-plus / qwq-32b
（排除 qwen-image-edit / text-embedding）

**Moonshot（8 全）**：kimi-k2 / kimi-k2-instruct / kimi-k2-instruct-0905 / kimi-k2-thinking / kimi-k2.5 / kimi-k2.6 / kimi-k2.7-code / kimi-k2.7-code-highspeed

**MiniMax（13 全）**：m2 / m2.1 / m2.1-highspeed / m2.5 / m2.5-highspeed / m2.7 / m2.7-highspeed / minimax-m2 / minimax-m2.1 / minimax-m2.1-lightning / minimax-m2.7 / minimax-m2.7-highspeed / minimax-m3

**ZAI（7 全）**：glm-4.5 / glm-4.5v / glm-4.6 / glm-4.6v / glm-4.7 / glm-5 / glm-5.2

**Zhipu（GLM 对话+VL，排除 cogvideo/cogview/embedding/ocr/search）**：glm-4.1v-thinking-flash / glm-4.1v-thinking-flashx / glm-4.5-air / glm-4.5v / glm-4.6 / glm-4.6v / glm-4.6v-flash / glm-4.6v-flashx / glm-4.7 / glm-4.7-flash / glm-4.7-flashx / glm-5 / glm-5-turbo / glm-5.1 / glm-5v-turbo

**其余供应商主力**（按 research 第 3 节后续 provider，从 line 289+ 读完整列表补全 baidu/bytedance/doubao/mistral/meta/xai/perplexity/cohere/amazon/nvidia/tencent/xiaomi/stepfun/inclusionai/streamlake/alibaba/fishaudio/indexteam/wan 各家的对话/coding 模型，排除 embedding/image/audio/video/safety）。

> **执行规则**（交 implement agent）：逐 provider 走 research 清单，保留 modalities 含 `→ text` 且非 image/audio/video/embedding/safety/reward/ocr/search 专用的全部模型。最终 model_list 约 180-220 个。

### 3. models.default（三档默认）

```json
"models": {
  "default": {
    "anthropic/claude-sonnet-4.5": {},
    "openai/gpt-5.2-codex": {},
    "deepseek/deepseek-v3.2": {}
  }
}
```

三档：Claude 主力（sonnet-4.5，cc 默认档）/ OpenAI 编程旗舰（gpt-5.2-codex）/ 国产通用（deepseek-v3.2 主线含 Sparse Attention）。

### 4. desc 改写（8 语言）

现有无 desc 字段则新增；失实时修正。TheRouter 是多供应商聚合：
- en-US: "TheRouter multi-vendor AI gateway (Anthropic/OpenAI/Google/DeepSeek/Qwen/GLM/Kimi/MiniMax etc., 28 providers)"
- zh-Hans: "TheRouter 多供应商 AI 聚合网关（Anthropic/OpenAI/Google/DeepSeek/Qwen/GLM/Kimi/MiniMax 等，28 家）"
- 其余 6 语言同步翻译

## Acceptance Criteria

- [ ] endpoints 3 协议（anthropic + openai/v1 + gemini）
- [ ] model_list.default 约 180-220（全量对话/coding/推理，排除纯功能模型）
- [ ] model id 全用 `provider/model-id` 单斜杠格式
- [ ] models.default 三档
- [ ] desc 8 语言
- [ ] JSON 合法
- [ ] 仅改 therouter 协议块

## Out of Scope

- 上下文窗口字段（value 留空 object）
- 价格字段（实时价走 price_sync.rs）
- STATIC_MODEL_IDS
- 其他协议块

## Technical Notes

- 真值源：`protocols.therouter`
- canonical id 格式：`provider/model-id` 单斜杠（URL slug 用 `--` 双横，API body 用单斜杠）
- base_url 规则：openai 带 `/v1`，anthropic/gemini 仅 host
- 同源参考：cherryin（聚合平台 provider/model 前缀格式）
