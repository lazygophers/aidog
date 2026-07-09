# Research: TheRouter 全量研究

- **Query**: TheRouter (therouter.ai) 全量模型清单 + endpoints，为 aidog `platform-presets.json` 补全提供数据源
- **Scope**: external（官网 + API 探测）+ internal（当前 preset 现状）
- **Date**: 2026-07-09

---

## 核心结论（TL;DR）

1. **TheRouter 是三协议聚合网关**（实测探测确认）：
   - `https://api.therouter.ai/v1/messages` — **Anthropic 原生协议**（401 实测存在，非 404）
   - `https://api.therouter.ai/v1/chat/completions` — **OpenAI Chat Completions**
   - `https://api.therouter.ai/v1/responses` — **OpenAI Responses API**
   - `https://api.therouter.ai/v1beta/models/{model}:generateContent` 与 `:streamGenerateContent` — **Gemini 原生协议**
2. **鉴权统一为 Bearer**：`Authorization: Bearer sk-xxx`（sk- 前缀的 TheRouter API key）。Anthropic 端点也接受 `x-api-key`；Gemini 端点也接受 `x-goog-api-key`；但**任何协议都不接受原厂 Google API key**（仅认 TheRouter key）。Dashboard: https://dashboard.therouter.ai/api-keys
3. **canonical model id 格式 = `provider/model-id`**（单斜杠），例：`anthropic/claude-sonnet-4.5`。URL slug 用 `provider--model-id`（双横），但 API body 里传 `provider/model-id`。所有代码示例一致。
4. **全量清单 = 291 个模型 / 28 个 provider**（2026-07-09 抓取），下文逐 provider 列出。
5. **当前 aidog preset 现状**（`src-tauri/defaults/platform-presets.json:1959`）：单 anthropic endpoint `https://api.therouter.ai` + 空 model_list — **base_url 与 protocol 正确可用**（anthropic 协议会拼成 `https://api.therouter.ai/v1/messages`，命中真实路由），仅需补 model_list 与（可选）增加 openai / gemini endpoint。

---

## API Endpoints

实测探测结果（401 = 路由存在，鉴权失败；404 = 路由不存在）：

| 协议 | Method | Full URL | Auth header | 探测响应 |
|---|---|---|---|---|
| Anthropic Messages | POST | `https://api.therouter.ai/v1/messages` | `Authorization: Bearer sk-xxx` 或 `x-api-key: sk-xxx` | 401 `{"type":"error","error":{"type":"authentication_error",...}}` |
| OpenAI Chat | POST | `https://api.therouter.ai/v1/chat/completions` | `Authorization: Bearer sk-xxx` | 401 OpenAI 错误格式 |
| OpenAI Responses | POST | `https://api.therouter.ai/v1/responses` | `Authorization: Bearer sk-xxx` | 401 |
| OpenAI Models 列表 | GET | `https://api.therouter.ai/v1/models` | `Authorization: Bearer sk-xxx` | 401（需有效 key 才能枚举） |
| Gemini generateContent | POST | `https://api.therouter.ai/v1beta/models/{model}:generateContent` | `x-goog-api-key: sk-xxx` 或 `Authorization: Bearer sk-xxx` | 401（明示 "Google API keys are not accepted on the public gateway"） |
| Gemini stream | POST | `https://api.therouter.ai/v1beta/models/{model}:streamGenerateContent` | 同上 | 401 |
| 根 / 健康 | GET | `https://api.therouter.ai/` | — | 404 `Route GET:/ not found` |

**对 aidog preset 的映射**：

| aidog `protocol` | base_url 应填 | provider_api_path（自动派生） | 最终 URL |
|---|---|---|---|
| `anthropic` | `https://api.therouter.ai` | `/v1/messages` | ✅ 正确 |
| `openai` | `https://api.therouter.ai/v1` | `/chat/completions` | ✅ 正确 |
| `gemini` | `https://api.therouter.ai` | `/v1beta/models/{model}:generateContent` | 推测: 可用（需 `model_name_template`，未在 aidog presets 框架内验证过） |

base_url 含/不含 `/v1` 的规则：**OpenAI 协议 base_url 必须含 `/v1`**（`https://api.therouter.ai/v1`）；**Anthropic 协议 base_url 仅含 host**（`https://api.therouter.ai`，框架自己拼 `/v1/messages`）。代码示例：
- OpenAI SDK: `new OpenAI({ apiKey, baseURL: "https://api.therouter.ai/v1" })`
- curl: `curl https://api.therouter.ai/v1/chat/completions -H "Authorization: Bearer $THE_ROUTER_API_KEY"`

环境变量名：官方文档用 `THE_ROUTER_API_KEY` 与 `THEROUTER_API_KEY` 两种（同义）。

---

## 全量模型清单（291 个 / 28 provider）

字段说明：Context = 上下文窗口；MaxOut = 最大输出；In/Out = 每 1M token 美元定价（聚合网关价，已含 TheRouter 折扣）；Modalities = 输入 → 输出模态。`-` = 官网卡未显示（多见于非 chat 模型如 image/audio/video/embed）。

### Anthropic (14)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `anthropic/claude-fable-5` | 1M | 128K | $12.00 | $60.00 | text image → text | 2026-06 frontier 旗舰，agentic coding 最强 |
| `anthropic/claude-haiku-4.5` | 200K | 8K | $1.20 | $6.00 | text image pdf → text | 近-frontier 轻量档 |
| `anthropic/claude-opus-4` | 200K | 32K | $18.00 | $90.00 | text image pdf → text | Opus 初代 |
| `anthropic/claude-opus-4.1` | 200K | 32K | $18.00 | $90.00 | text image pdf → text | Opus 4.1 |
| `anthropic/claude-opus-4.5` | 200K | 64K | $6.00 | $30.00 | text image pdf → text | Opus 4.5 主线 |
| `anthropic/claude-opus-4.5-20251101` | 200K | 32K | $6.26 | $31.30 | text image → text | Opus 4.5 dated snapshot |
| `anthropic/claude-opus-4.6` | 1M | 128K | $6.00 | $30.00 | text image pdf → text | Opus 4.6 主线，coding 业界最强 |
| `anthropic/claude-opus-4.6-thinking` | 200K | 32K | $6.00 | $30.00 | text image → text | Opus 4.6 extended-thinking 预启用版 |
| `anthropic/claude-opus-4.7` | 1M | 128K | $6.00 | $30.00 | text image pdf → text | Opus 4.7，coding 较 4.6 +13% |
| `anthropic/claude-opus-4.8` | 200K | 32K | $6.00 | $30.00 | text image → text | 最新 Opus 旗舰（partner network） |
| `anthropic/claude-sonnet-4` | 1M | 64K | $3.60 | $18.00 | text image pdf → text | Sonnet 4 |
| `anthropic/claude-sonnet-4.5` | 1M | 64K | $3.60 | $18.00 | text image pdf → text | Sonnet 4.5（claude code 默认档） |
| `anthropic/claude-sonnet-4.6` | 1M | 64K | $3.60 | $18.00 | text image pdf → text | Sonnet 4.6 |
| `anthropic/claude-sonnet-5` | 1M | 128K | $2.40 | $12.00 | text image → text | 最新 Sonnet frontier |

### OpenAI (58)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `openai/chatgpt-image-latest` | - | - | $6.00 | - | text image → image | ChatGPT 内嵌图像生成 |
| `openai/gpt-4.1` | 1.0M | 33K | $2.40 | $9.60 | text image → text | 非推理旗舰 |
| `openai/gpt-4.1-mini` | 1.0M | 33K | $0.480 | $1.92 | text image → text | 4.1 小档 |
| `openai/gpt-4.1-nano` | 1.0M | 33K | $0.120 | $0.480 | text image → text | 4.1 最小档 |
| `openai/gpt-4o` | 128K | 16K | $3.00 | $12.00 | text image → text | 多模态旗舰 |
| `openai/gpt-4o-mini` | 128K | 16K | $0.180 | $0.720 | text image → text | 4o mini |
| `openai/gpt-4o-mini-transcribe` | - | - | $1.50 | - | audio → text | 语音转文本 |
| `openai/gpt-4o-mini-tts` | - | - | $0.720 | - | text → audio | TTS |
| `openai/gpt-4o-transcribe` | - | - | $3.00 | - | audio → text | 语音转文本 |
| `openai/gpt-4o-transcribe-diarize` | - | - | $3.00 | - | audio → text | 说话人分离转录 |
| `openai/gpt-5` | 400K | 128K | $1.50 | $12.00 | text image → text | GPT-5 |
| `openai/gpt-5-chat` | 400K | 128K | $1.50 | $12.00 | text → text | GPT-5 chat 优化 |
| `openai/gpt-5-codex` | 400K | 128K | $1.50 | $12.00 | text image → text | GPT-5 Codex 编程 |
| `openai/gpt-5-mini` | 400K | 128K | $0.300 | $2.40 | text image → text | GPT-5 mini |
| `openai/gpt-5-nano` | 400K | 128K | $0.060 | $0.480 | text image → text | GPT-5 nano |
| `openai/gpt-5-pro` | 400K | 128K | $22.50 | $180.00 | text image → text | GPT-5 Pro 高算力 |
| `openai/gpt-5.1` | 400K | 128K | $1.50 | $12.00 | text image → text | GPT-5.1 |
| `openai/gpt-5.1-chat` | 400K | 128K | $1.50 | $12.00 | text → text | GPT-5.1 chat |
| `openai/gpt-5.1-codex` | 400K | 128K | $1.50 | $12.00 | text image → text | GPT-5.1 Codex |
| `openai/gpt-5.1-codex-max` | 400K | 128K | $1.50 | $12.00 | text image → text | GPT-5.1 Codex Max 长任务 |
| `openai/gpt-5.1-codex-mini` | 400K | 128K | $0.300 | $2.40 | text image → text | GPT-5.1 Codex Mini |
| `openai/gpt-5.2` | 400K | 128K | $2.10 | $16.80 | text image → text | GPT-5.2 |
| `openai/gpt-5.2-chat` | 400K | 128K | $2.10 | $16.80 | text → text | GPT-5.2 chat |
| `openai/gpt-5.2-codex` | 400K | 128K | $2.10 | $16.80 | text image → text | GPT-5.2 Codex 编程旗舰 |
| `openai/gpt-5.2-pro` | 400K | 128K | $6.00 | $48.00 | text image → text | GPT-5.2 Pro |
| `openai/gpt-5.3-chat-latest` | 400K | 128K | $2.10 | $16.80 | text image → text | GPT-5.3 chat "latest" alias |
| `openai/gpt-5.3-codex` | 400K | 128K | $6.00 | $48.00 | text image → text | GPT-5.3 Codex |
| `openai/gpt-5.4` | 1M | 128K | $3.00 | $18.00 | text image → text | GPT-5.4 |
| `openai/gpt-5.4-mini` | 400K | 128K | $1.80 | $5.40 | text image → text | GPT-5.4 mini |
| `openai/gpt-5.4-nano` | 400K | 128K | $0.360 | $2.10 | text image → text | GPT-5.4 nano |
| `openai/gpt-5.4-pro` | 1M | 128K | $48.00 | $288.00 | text image → text | GPT-5.4 Pro |
| `openai/gpt-5.5` | 1M | 128K | $8.40 | $48.00 | text image → text | GPT-5.5 frontier |
| `openai/gpt-5.5-pro` | 1M | 128K | $48.00 | $288.00 | text image → text | GPT-5.5 Pro |
| `openai/gpt-audio` | - | - | $3.00 | $12.00 | text audio → text audio | Chat Completions audio I/O |
| `openai/gpt-audio-1.5` | - | - | $4.80 | $19.20 | text audio image → text audio | 语音旗舰 |
| `openai/gpt-audio-mini` | - | - | $0.180 | $0.720 | text audio → text audio | audio mini |
| `openai/gpt-image-1` | - | - | $6.00 | - | text image → image | 上一代图像 |
| `openai/gpt-image-1-mini` | - | - | $2.40 | - | text image → image | 图像 mini |
| `openai/gpt-image-1.5` | - | - | $6.00 | - | text image → image | 图像 1.5 |
| `openai/gpt-image-2` | - | - | $6.00 | - | text image → image | 最新图像旗舰 |
| `openai/gpt-oss-120b` | 128K | 16K | $0.240 | $0.960 | text → text | 开源 120B |
| `openai/gpt-oss-20b` | 128K | 16K | $0.120 | $0.480 | text → text | 开源 20B |
| `openai/gpt-oss-safeguard-120b` | 128K | 16K | $0.240 | $0.960 | text → text | 安全推理 120B |
| `openai/gpt-oss-safeguard-20b` | 128K | 16K | $0.120 | $0.240 | text → text | 安全分类 20B |
| `openai/gpt-realtime-whisper` | - | - | - | - | audio → text | Realtime Whisper ASR |
| `openai/o1` | 200K | 100K | $18.00 | $72.00 | text image → text | 推理初代 |
| `openai/o3` | 200K | 100K | $2.40 | $9.60 | text image → text | 推理主力 |
| `openai/o3-deep-research` | 200K | 100K | $12.00 | $48.00 | text image → text | Deep Research 旗舰 |
| `openai/o3-pro` | 200K | 100K | $24.00 | $96.00 | text image → text | o3 Pro 高算力 |
| `openai/o4-mini` | 200K | 100K | $1.32 | $5.28 | text image → text | o4 mini 推理 |
| `openai/o4-mini-deep-research` | 200K | 100K | $2.40 | $9.60 | text image → text | o4 mini Deep Research |
| `openai/sora-2` | - | - | - | - | text image → video | 视频生成 |
| `openai/sora-2-pro` | - | - | - | - | text image → video | Sora Pro |
| `openai/text-embedding-3-large` | 8K | - | $0.156 | - | text → embedding | 3072 dim 嵌入 |
| `openai/text-embedding-3-small` | 8K | - | $0.024 | - | text → embedding | 1536 dim 嵌入 |
| `openai/tts-1` | - | - | - | - | text → audio | TTS 速度档 |
| `openai/tts-1-hd` | - | - | - | - | text → audio | TTS 质量档 |
| `openai/whisper-1` | - | - | - | - | audio → text | 通用 ASR |

### Google (14)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `google/gemini-2.5-flash` | 1.0M | 66K | $0.360 | $3.00 | text image pdf → text | 主力 flash |
| `google/gemini-2.5-flash-lite` | 1.0M | 66K | $0.120 | $0.480 | text image pdf → text | 低成本 flash |
| `google/gemini-2.5-pro` | 1.0M | 66K | $1.50 | $12.00 | text image pdf → text | Pro 主力 |
| `google/gemini-3-pro-image` | 66K | 33K | $2.68 | $146.40 | text image → text image | Gemini 3 Pro 图像 GA |
| `google/gemini-3.1-flash-image` | 131K | 33K | $0.660 | $73.25 | text image → text image | Gemini 3.1 Flash 图像 GA |
| `google/gemini-3.1-flash-lite` | 1M | 8K | $0.300 | $1.80 | text → text | 3.1 flash lite |
| `google/gemini-3.5-flash` | 1M | 8K | $1.83 | $10.99 | text image → text | 3.5 flash |
| `google/gemma-2-9b-it` | 8K | 4K | $0.360 | $0.360 | text → text | Gemma 2 9B IT |
| `google/gemma-3-12b` | 128K | 8K | $0.144 | $0.456 | text image → text | Gemma 3 12B |
| `google/gemma-3-27b` | 128K | 8K | $0.360 | $0.600 | text image → text | Gemma 3 27B |
| `google/gemma-3-27b-it` | 128K | 32K | $0.0488 | $0.1831 | text → text | Gemma 3 27B IT |
| `google/gemma-3-4b` | 128K | 8K | $0.060 | $0.120 | text image → text | Gemma 3 4B |
| `google/gemma-4-26b-a4b-it` | 128K | 8K | $0.240 | $0.480 | text image → text | Gemma 4 26B MoE |
| `google/gemma-4-31b-it` | 128K | 8K | $0.360 | $0.600 | text image → text | Gemma 4 31B dense |

### DeepSeek (11)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `deepseek/deepseek-r1` | 128K | 33K | $2.10 | $8.40 | text → text | R1 推理 |
| `deepseek/deepseek-r1-0528` | 128K | 33K | $1.34 | $5.38 | text → text | R1 0528 snapshot |
| `deepseek/deepseek-v3-0324` | 128K | 8K | $0.672 | $2.69 | text → text | V3 0324 |
| `deepseek/deepseek-v3.1` | 128K | 33K | $0.900 | $2.64 | text → text | V3.1 混合思考 |
| `deepseek/deepseek-v3.1-terminus` | 131K | 33K | $0.480 | $1.74 | text → text | V3.1 工具调用优化 |
| `deepseek/deepseek-v3.1-think` | 128K | 33K | $1.34 | $4.03 | text → text | V3.1 思考态 |
| `deepseek/deepseek-v3.2` | 128K | 33K | $0.960 | $2.88 | text → text | V3.2 主线（含 Sparse Attention） |
| `deepseek/deepseek-v3.2-exp` | 131K | 16K | $0.480 | $0.720 | text → text | V3.2 实验 |
| `deepseek/deepseek-v3.2-exp-think` | 128K | 33K | $0.672 | $1.01 | text → text | V3.2 思考实验 |
| `deepseek/deepseek-v4-flash` | 1M | 384K | $0.240 | $0.480 | text → text | V4 Flash 1M ctx |
| `deepseek/deepseek-v4-pro` | 1M | 384K | $3.00 | $6.00 | text → text | V4 Pro 高能力 |

### Qwen (39)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `qwen/qwen-flash` | 1.0M | 33K | $0.084 | $0.600 | text → text | 超低价 flash 1M |
| `qwen/qwen-image-edit` | - | - | $0.048 | - | text image → image | 图像编辑 |
| `qwen/qwen-long` | 10.5M | 8K | $0.144 | $0.540 | text → text | 长上下文 10M |
| `qwen/qwen-max` | 131K | 33K | $2.40 | $9.60 | text → text | 上一代旗舰 |
| `qwen/qwen-plus` | 1.0M | 33K | $0.600 | $1.80 | text → text | 中档 |
| `qwen/qwen-plus-thinking` | 1.0M | 33K | $0.264 | $2.69 | text → text | Plus 思考态 |
| `qwen/qwen2.5-vl-72b-instruct` | 33K | 8K | $5.38 | $16.13 | text image → text | 2.5 VL 72B |
| `qwen/qwen3-14b` | 131K | 33K | $0.084 | $0.252 | text → text | Qwen3 14B |
| `qwen/qwen3-235b` | 131K | 33K | $0.360 | $1.38 | text → text | Qwen3 235B 旗舰 |
| `qwen/qwen3-235b-a22b-thinking-2507` | 256K | 33K | $0.672 | $6.72 | text → text | 235B 思考 2507 |
| `qwen/qwen3-235b-thinking` | 131K | 33K | $0.492 | $1.68 | text → text | 235B 思考 |
| `qwen/qwen3-30b` | 131K | 33K | $0.168 | $0.504 | text → text | 30B MoE |
| `qwen/qwen3-30b-a3b-thinking-2507` | 128K | 33K | $0.252 | $2.52 | text → text | 30B 思考 2507 |
| `qwen/qwen3-30b-thinking` | 131K | 33K | $0.168 | $0.504 | text → text | 30B 思考 |
| `qwen/qwen3-32b` | 131K | 33K | $0.240 | $0.960 | text → text | 32B dense |
| `qwen/qwen3-32b-thinking` | 128K | 33K | $0.672 | $6.72 | text → text | 32B 思考 |
| `qwen/qwen3-8b` | 131K | 33K | $0.036 | $0.072 | text → text | 8B 轻量 |
| `qwen/qwen3-coder-30b` | 262K | 66K | $0.240 | $0.960 | text → text | Coder 30B |
| `qwen/qwen3-coder-480b` | 262K | 66K | $0.720 | $2.82 | text → text | Coder 480B 旗舰 |
| `qwen/qwen3-coder-plus` | 262K | 33K | $1.44 | $7.20 | text → text | 商用 Coder |
| `qwen/qwen3-max` | 262K | 33K | $1.80 | $9.00 | text → text | 商用旗舰 |
| `qwen/qwen3-next-80b` | 262K | 16K | $0.240 | $0.960 | text → text | Transformer-Mamba 混合 |
| `qwen/qwen3-vl-235b` | 256K | 33K | $0.360 | $1.38 | text image → text | VL 235B |
| `qwen/qwen3-vl-235b-a22b-thinking` | 256K | 33K | $0.672 | $6.72 | text image → text | VL 235B 思考 |
| `qwen/qwen3-vl-32b` | 33K | 8K | $0.336 | $1.01 | text image → text | VL 32B |
| `qwen/qwen3-vl-8b` | 33K | 8K | $0.084 | $0.252 | text image → text | VL 8B |
| `qwen/qwen3-vl-flash` | 256K | 8K | $0.0514 | $0.504 | text image → text | VL flash |
| `qwen/qwen3-vl-plus` | 262K | 16K | $0.360 | $2.40 | text image → text | VL plus 商用 |
| `qwen/qwen3.5-flash` | 1M | 33K | $0.048 | $0.180 | text → text | 3.5 flash |
| `qwen/qwen3.5-plus` | 1M | 8K | $0.264 | $1.61 | text → text | 3.5 plus |
| `qwen/qwen3.6-27b` | 262K | 33K | $0.240 | $0.960 | text → text | 3.6 27B |
| `qwen/qwen3.6-flash` | 1M | 33K | $0.084 | $0.300 | text → text | 3.6 flash |
| `qwen/qwen3.6-max-preview` | 1M | 33K | $1.80 | $9.00 | text → text | 3.6 max preview |
| `qwen/qwen3.6-plus` | 1M | 33K | $0.600 | $1.80 | text → text | 3.6 plus |
| `qwen/qwen3.7-max` | 1M | 66K | $1.50 | $4.50 | text → text | 2026-05 推理旗舰 |
| `qwen/qwen3.7-plus` | 1M | 66K | $0.384 | $1.54 | text image video → text | 2026-05 多模态 |
| `qwen/qwq-32b` | 131K | 33K | $0.264 | $1.02 | text → text | QwQ 推理 32B |
| `qwen/text-embedding-v3` | 8K | - | $0.144 | - | text → embedding | 通用嵌入 v3 |
| `qwen/text-embedding-v4` | 8K | - | $0.144 | - | text → embedding | 最新嵌入 |

### Moonshot (8)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `moonshot/kimi-k2` | 131K | 33K | $0.972 | $3.84 | text → text | K2 fast 非思考 |
| `moonshot/kimi-k2-instruct` | 200K | 8K | $1.34 | $5.38 | text → text | K2 instruct 1T MoE |
| `moonshot/kimi-k2-instruct-0905` | 200K | 8K | $1.34 | $5.38 | text → text | K2 instruct 0905 |
| `moonshot/kimi-k2-thinking` | 256K | 64K | $0.960 | $3.90 | text → text | K2 推理 1T MoE |
| `moonshot/kimi-k2.5` | 256K | 64K | $0.960 | $4.68 | text image → text | K2.5 多模态旗舰 |
| `moonshot/kimi-k2.6` | 256K | 64K | $1.14 | $5.40 | text image → text | 2026-04 旗舰 |
| `moonshot/kimi-k2.7-code` | 256K | 66K | $1.14 | $4.80 | text → text | 2026-06 编程旗舰 |
| `moonshot/kimi-k2.7-code-highspeed` | 256K | 66K | $1.74 | $7.20 | text → text | K2.7 Code 高速版 |

### MiniMax (13)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `minimax/m2` | 205K | 66K | $0.480 | $1.92 | text → text | M2 主线 |
| `minimax/m2.1` | 205K | 66K | $0.480 | $1.92 | text → text | M2.1 |
| `minimax/m2.1-highspeed` | 205K | 66K | $0.960 | $3.84 | text → text | M2.1 ~100 tok/s |
| `minimax/m2.5` | 205K | 66K | $0.480 | $1.92 | text → text | M2.5 agent-native |
| `minimax/m2.5-highspeed` | 205K | 66K | $0.960 | $3.84 | text → text | M2.5 高速 |
| `minimax/m2.7` | 205K | 66K | $0.480 | $1.92 | text → text | M2.7 推理旗舰 |
| `minimax/m2.7-highspeed` | 205K | 66K | $0.960 | $3.84 | text → text | M2.7 高速 |
| `minimax/minimax-m2` | 200K | 8K | $0.708 | $2.82 | text → text | 上一代 M2 |
| `minimax/minimax-m2.1` | 200K | 8K | $0.708 | $2.82 | text → text | 上一代 M2.1 |
| `minimax/minimax-m2.1-lightning` | 200K | 8K | $0.708 | $5.64 | text → text | M2.1 Lightning |
| `minimax/minimax-m2.7` | 200K | 8K | $0.708 | $2.82 | text → text | 上一代 M2.7 |
| `minimax/minimax-m2.7-highspeed` | 200K | 8K | $1.42 | $5.64 | text → text | M2.7 高速 |
| `minimax/minimax-m3` | 1M | 66K | $0.360 | $1.44 | text image video → text | 2026-06 frontier，1M ctx |

### ZAI (7)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `zai/glm-4.5` | 128K | 8K | $0.720 | $2.64 | text → text | GLM-4.5 |
| `zai/glm-4.5v` | 128K | 8K | $0.720 | $2.16 | text image → text | GLM-4.5 VL |
| `zai/glm-4.6` | 128K | 8K | $0.720 | $2.64 | text → text | GLM-4.6 |
| `zai/glm-4.6v` | 128K | 8K | $0.360 | $1.08 | text image → text | GLM-4.6 VL |
| `zai/glm-4.7` | 200K | 8K | $0.720 | $2.64 | text → text | GLM-4.7 |
| `zai/glm-5` | 200K | 8K | $1.20 | $3.84 | text → text | GLM-5 |
| `zai/glm-5.2` | 1M | 8K | $1.85 | $5.81 | text → text | 2026-06 旗舰 1M ctx |

### Zhipu (33)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `zhipu/cogvideox-3` | - | - | - | - | text image → video | 视频生成 |
| `zhipu/cogvideox-flash` | - | - | - | - | text image → video | 视频生成免费档 |
| `zhipu/cogview-3-flash` | - | - | - | - | text → image | 文生图免费档 |
| `zhipu/cogview-4` | - | - | - | - | text → image | 文生图 4 |
| `zhipu/embedding-2` | 512 | - | $0.120 | - | text → embedding | 1024-dim |
| `zhipu/embedding-3` | 3K | - | $0.120 | - | text → embedding | 2048-dim |
| `zhipu/glm-4.1v-thinking-flash` | 66K | 16K | - | - | text image video → text | 4.1V thinking 免费 |
| `zhipu/glm-4.1v-thinking-flashx` | 66K | 16K | $0.480 | $0.480 | text image video → text | 4.1V thinking 付费 |
| `zhipu/glm-4.5-air` | 131K | 66K | $0.180 | $1.44 | text → text | 4.5 Air 轻量 |
| `zhipu/glm-4.5v` | 66K | 16K | $0.960 | $2.88 | text image video pdf → text | 4.5V |
| `zhipu/glm-4.6` | 131K | 66K | $0.336 | $1.68 | text → text | 4.6 |
| `zhipu/glm-4.6v` | 131K | 66K | $0.336 | $1.68 | text image video pdf → text | 4.6V |
| `zhipu/glm-4.6v-flash` | 131K | 16K | - | - | text image video pdf → text | 4.6V flash 免费 |
| `zhipu/glm-4.6v-flashx` | 131K | 16K | $0.060 | $0.360 | text image video pdf → text | 4.6V FlashX 9B |
| `zhipu/glm-4.7` | 203K | 131K | $0.960 | $3.48 | text → text | 4.7 358B MoE |
| `zhipu/glm-4.7-flash` | 203K | 131K | - | - | text → text | 4.7 flash 免费 |
| `zhipu/glm-4.7-flashx` | 203K | 131K | $0.144 | $0.720 | text → text | 4.7 FlashX 付费 |
| `zhipu/glm-5` | 203K | 131K | $1.56 | $5.04 | text → text | 旗舰 754B MoE |
| `zhipu/glm-5-turbo` | 203K | 131K | $1.80 | $6.00 | text → text | 5 Turbo |
| `zhipu/glm-5.1` | 203K | 131K | $0.588 | $4.70 | text → text | 最新旗舰 |
| `zhipu/glm-5v-turbo` | 203K | 131K | $0.588 | $4.70 | text image → text | 5V Turbo VL |
| `zhipu/glm-image` | - | - | - | - | text → image | GLM Image 2K |
| `zhipu/glm-ocr` | 66K | 16K | $0.0336 | $0.0336 | text image pdf → text ocr | OCR 0.9B |
| `zhipu/search-pro` | 8K | - | - | - | text → text | Web Search Pro |
| `zhipu/search-pro-quark` | 8K | - | - | - | text → text | Search Pro 夸克 |
| `zhipu/search-pro-sogou` | 8K | - | - | - | text → text | Search Pro 搜狗 |
| `zhipu/search-std` | 8K | - | - | - | text → text | Web Search Standard |
| `zhipu/vidu2-image` | - | - | - | - | image text → video | Vidu 2 图生视频 |
| `zhipu/vidu2-reference` | - | - | - | - | image text → video | Vidu 2 参考 |
| `zhipu/vidu2-start-end` | - | - | - | - | image text → video | Vidu 2 首尾帧 |
| `zhipu/viduq1-image` | - | - | - | - | image text → video | Vidu Q1 1080p |
| `zhipu/viduq1-start-end` | - | - | - | - | image text → video | Vidu Q1 首尾帧 |
| `zhipu/viduq1-text` | - | - | - | - | text → video | Vidu Q1 文生视频 |

### Baidu (4)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `baidu/ernie-4.5` | 131K | 33K | $0.468 | $1.85 | text → text | 旗舰 300B MoE |
| `baidu/ernie-4.5-turbo-128k` | 131K | 8K | $0.264 | $1.08 | text → text | Turbo 128K |
| `baidu/ernie-4.5-turbo-vl-32k` | 33K | 8K | $1.01 | $3.02 | text image → text | Turbo VL 32K |
| `baidu/ernie-x1-turbo-32k` | 33K | 8K | $0.336 | $1.34 | text → text | X1 Turbo |

### ByteDance (12)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `bytedance/doubao-1.5-pro-32k-250115` | 33K | 8K | $0.264 | $0.672 | text → text | 1.5 Pro 250115 |
| `bytedance/doubao-1.5-thinking-vision-pro` | 128K | 33K | $1.01 | $3.02 | text image → text | 1.5 thinking VL |
| `bytedance/doubao-seed-1.6` | 256K | 8K | $0.264 | $0.672 | text → text | Seed 1.6 |
| `bytedance/doubao-seed-1.6-thinking` | 256K | 33K | $0.264 | $2.69 | text → text | Seed 1.6 思考 |
| `bytedance/doubao-seed-2.0-code-preview-260215` | 262K | 8K | $1.08 | $5.38 | text → text | 2.0 Code Preview |
| `bytedance/doubao-seed-2.0-lite-260215` | 262K | 8K | $0.204 | $1.21 | text → text | 2.0 Lite |
| `bytedance/doubao-seed-2.0-mini-260215` | 262K | 8K | $0.072 | $0.672 | text → text | 2.0 Mini |
| `bytedance/doubao-seed-2.0-pro-260215` | 262K | 8K | $1.08 | $5.38 | text → text | 2.0 Pro 260215 |
| `bytedance/doubao-seed-2.1-pro` | 256K | 33K | $1.10 | $5.50 | text image → text | 2.1 Pro |
| `bytedance/doubao-seed-2.1-turbo` | 256K | 33K | $0.552 | $2.75 | text → text | 2.1 Turbo |
| `bytedance/doubao-seed-evolving` | 256K | 33K | $1.10 | $5.50 | text → text | Seed Evolving |
| `bytedance/seed-oss-36b` | 131K | 33K | $0.168 | $0.504 | text → text | Seed OSS 36B |

### Doubao (火山方舟命名空间, 17)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `doubao/doubao-1-5-lite-32k` | 33K | 16K | $0.050 | $0.100 | text → text | 1.5 Lite |
| `doubao/doubao-1-5-pro-32k` | 33K | 16K | $0.1333 | $0.3334 | text → text | 1.5 Pro |
| `doubao/doubao-1-5-vision-pro-32k` | 33K | 16K | $0.500 | $1.50 | text image → text | 1.5 VL Pro |
| `doubao/doubao-seed-1-6` | 131K | 33K | $0.1333 | $1.33 | text → text | Seed 1.6 |
| `doubao/doubao-seed-1-6-flash` | 131K | 33K | $0.025 | $0.250 | text → text | Seed 1.6 Flash |
| `doubao/doubao-seed-1-6-vision` | 131K | 33K | $0.1333 | $1.33 | text image → text | Seed 1.6 VL |
| `doubao/doubao-seed-1-8` | 131K | 33K | $0.1333 | $1.33 | text → text | Seed 1.8 |
| `doubao/doubao-seed-2-0-code` | 131K | 33K | $0.5333 | $2.67 | text → text | Seed 2.0 Code |
| `doubao/doubao-seed-2-0-lite` | 131K | 33K | $0.100 | $0.600 | text → text | Seed 2.0 Lite |
| `doubao/doubao-seed-2-0-mini` | 131K | 33K | $0.0334 | $0.3334 | text → text | Seed 2.0 Mini |
| `doubao/doubao-seed-2-0-pro` | 131K | 33K | $0.5333 | $2.67 | text → text | Seed 2.0 Pro |
| `doubao/doubao-seed-character` | 131K | 33K | $0.1333 | $0.3334 | text → text | 角色扮演 |
| `doubao/doubao-seed-code` | 131K | 33K | $0.200 | $1.33 | text → text | 编程 |
| `doubao/doubao-seedance-2-0` | - | - | - | $4.67 | text image → video | SeedDance 2.0 |
| `doubao/doubao-seedance-2-0-fast` | - | - | - | $3.67 | text image → video | SeedDance 2.0 Fast |
| `doubao/doubao-seedream-4-5` | - | - | - | - | text image → image | SeedDream 4.5 |
| `doubao/doubao-seedream-5-0` | - | - | - | - | text image → image | SeedDream 5.0 |

### Mistral (9)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `mistral/codestral` | 256K | 16K | $0.360 | $1.08 | text → text | 编程 |
| `mistral/devstral-2` | 128K | 16K | $0.660 | $3.12 | text → text | 编程 123B |
| `mistral/magistral-small` | 128K | 131K | $0.600 | $1.80 | text image → text | 推理 24B |
| `mistral/ministral-14b` | 128K | 16K | $0.312 | $0.312 | text image → text | 14B |
| `mistral/ministral-3b` | 128K | 16K | $0.120 | $0.120 | text image → text | 3B |
| `mistral/ministral-8b` | 128K | 16K | $0.180 | $0.180 | text image → text | 8B |
| `mistral/mistral-large-3` | 128K | 16K | $0.600 | $1.80 | text image → text | Large 3 675B |
| `mistral/mistral-medium-3.5` | 262K | 33K | $1.80 | $9.00 | text image → text | 2026-04 dense 128B |
| `mistral/mistral-small-4` | 131K | 16K | $0.240 | $0.720 | text image → text | 2026-03 119B MoE |

### Meta (11)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `meta/llama-3.1-405b` | 128K | 8K | $3.72 | $3.72 | text → text | 最大开源 |
| `meta/llama-3.1-70b` | 128K | 8K | $1.08 | $1.08 | text → text | 70B |
| `meta/llama-3.1-8b` | 128K | 8K | $0.336 | $0.336 | text → text | 8B |
| `meta/llama-3.2-11b` | 128K | 8K | $0.240 | $0.240 | text image → text | 11B VL |
| `meta/llama-3.2-1b` | 128K | 8K | $0.156 | $0.156 | text → text | 1B |
| `meta/llama-3.2-3b` | 128K | 8K | $0.240 | $0.240 | text → text | 3B |
| `meta/llama-3.2-90b` | 128K | 8K | $1.08 | $1.08 | text image → text | 90B VL |
| `meta/llama-3.3-70b` | 128K | 8K | $1.08 | $1.08 | text → text | 3.3 70B |
| `meta/llama-3.3-70b-versatile` | 128K | 33K | $0.948 | $1.20 | text → text | Groq LPU 调优 |
| `meta/llama-4-maverick` | 1M | 16K | $0.360 | $1.50 | text image → text | Maverick 400B MoE |
| `meta/llama-4-scout` | 10M | 16K | $0.264 | $1.02 | text image → text | Scout 10M ctx |

### xAI (10)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `xai/grok-3-mini` | 131K | 33K | $0.540 | $0.900 | text → text | mini 推理 |
| `xai/grok-4` | 256K | 66K | $5.40 | $27.00 | text image → text | Grok 4 旗舰 |
| `xai/grok-4-1-fast-non-reasoning` | 2M | 66K | $0.360 | $0.900 | text image → text | 4.1 fast 非推理 |
| `xai/grok-4-1-fast-reasoning` | 2M | 66K | $0.360 | $0.900 | text image → text | 4.1 fast 推理 |
| `xai/grok-4-fast` | 2M | 128K | $0.360 | $0.900 | text image → text | 4 fast 默认非推理 |
| `xai/grok-4-fast-non-reasoning` | 2M | 128K | $0.360 | $0.900 | text image → text | 4 fast 非推理 |
| `xai/grok-4-fast-reasoning` | 2M | 128K | $0.360 | $0.900 | text image → text | 4 fast 推理 |
| `xai/grok-4.20-non-reasoning` | 2M | 66K | $3.60 | $10.80 | text image → text | 2026-03 旗舰 |
| `xai/grok-4.20-reasoning` | 2M | 66K | $3.60 | $10.80 | text image → text | 2026-03 推理 |
| `xai/grok-4.3` | 1M | 66K | $1.87 | $3.76 | text image → text | 最新旗舰 |

### Perplexity (2)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `perplexity/sonar` | 128K | 8K | $1.20 | $1.20 | text → text | 搜索增强 |
| `perplexity/sonar-pro` | 200K | 8K | $3.60 | $18.00 | text → text | 高级搜索 |

### Cohere (3)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `cohere/command-a` | 256K | 16K | $3.00 | $12.00 | text → text | 企业旗舰 |
| `cohere/command-r-plus` | 128K | 4K | $3.00 | $12.00 | text → text | RAG |
| `cohere/embed-v4` | 8K | - | $0.120 | - | text image → embedding | 多模态嵌入 |

### Amazon (6)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `amazon/nova-2-lite` | 1M | 65K | $0.180 | $0.720 | text image → text | Nova 2 Lite |
| `amazon/nova-lite` | 300K | 5K | $0.072 | $0.288 | text image → text | Nova Lite |
| `amazon/nova-micro` | 128K | 5K | $0.042 | $0.168 | text → text | Nova Micro 文本 |
| `amazon/nova-premier` | 1M | 5K | $3.00 | $12.00 | text image → text | Nova Premier 旗舰 |
| `amazon/nova-pro` | 300K | 5K | $0.960 | $3.84 | text image → text | Nova Pro |
| `amazon/titan-embed-v2` | 8K | - | $0.024 | - | text → embedding | Titan Embed V2 |

### NVIDIA (2)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `nvidia/nemotron-nano-30b` | 1M | 262K | $0.072 | $0.288 | text → text | Nano 30B Mamba+Attention |
| `nvidia/nemotron-super-120b` | 1M | 262K | $0.240 | $1.02 | text → text | Super 120B LatentMoE |

### Tencent (1)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `tencent/hunyuan-a13b` | 131K | 33K | $0.240 | $0.960 | text → text | Hunyuan A13B MoE |

### Xiaomi (5)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `xiaomi/mimo-v2-flash` | 131K | 16K | $0.144 | $0.432 | text → text | V2 Flash |
| `xiaomi/mimo-v2.5` | 131K | 16K | $0.192 | $0.384 | text image audio video → text | V2.5 多模态 |
| `xiaomi/mimo-v2.5-asr` | - | - | - | - | audio → text | V2.5 ASR |
| `xiaomi/mimo-v2.5-pro` | 131K | 16K | $0.600 | $1.20 | text image audio video → text | V2.5 Pro 旗舰 |
| `xiaomi/mimo-v2.5-pro-ultraspeed` | 131K | 16K | $1.50 | $3.00 | text image audio video → text | V2.5 Pro UltraSpeed |

### StepFun (1)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `stepfun/step-3.5-flash` | 131K | 33K | $0.240 | $0.960 | text → text | Step 3.5 Flash |

### InclusionAI (2)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `inclusionai/ling-flash-2` | 131K | 33K | $0.084 | $0.252 | text → text | Ling Flash 2.0 |
| `inclusionai/ring-flash-2` | 131K | 33K | $0.084 | $0.252 | text → text | Ring Flash 2.0 推理 |

### StreamLake (3)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `streamlake/kat-coder-128k` | 131K | 8K | $2.18 | $8.74 | text → text | KAT-Coder 128K |
| `streamlake/kat-coder-256k` | 262K | 8K | $3.19 | $12.77 | text → text | KAT-Coder 256K |
| `streamlake/kat-coder-32k` | 33K | 8K | $1.85 | $7.39 | text → text | KAT-Coder 32K |

### Alibaba — 语音 (2)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `alibaba/cosyvoice2-0.5b` | - | - | - | - | text → audio | 多语种 TTS |
| `alibaba/sensevoice-small` | - | - | - | - | audio → text | 多语种 ASR |

### FishAudio (1)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `fishaudio/fish-speech-1.5` | - | - | - | - | text → audio | 中英 TTS |

### IndexTeam (1)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `indexteam/indextts-2` | - | - | - | - | text → audio | IndexTTS 2 |

### Wan (Alibaba 通义万相, 2)

| Model ID | Context | MaxOut | In $/1M | Out $/1M | Modalities | 说明 |
|---|---|---|---|---|---|---|
| `wan/wan2.2-t2i-flash` | - | - | - | - | text → image | 文生图 flash（异步 `/v1/jobs`） |
| `wan/wan2.2-t2i-plus` | - | - | - | - | text → image | 文生图 plus |

---

## 三档默认推荐（供 `models.default`）

依据：当前 aidog 各协议 default 推荐模型 + TheRouter 价目表能力档位。**model id 用 `provider/model-id` 格式**。

### Claude 系（anthropic 协议默认）
- **anthropic/claude-sonnet-4.5** — 主力默认，平衡档（1M ctx, $3.60/$18.00）
- 备选：`anthropic/claude-opus-4.6`（旗舰）、`anthropic/claude-haiku-4.5`（轻量）

### OpenAI 系（openai 协议默认）
- **openai/gpt-5.2-codex** — 编程主力（400K ctx, $2.10/$16.80）
- 备选：`openai/gpt-5.2`（通用旗舰）、`openai/o3`（推理）、`openai/gpt-5.4`（最新 1M ctx）

### 国产系（多协议可选）
- **deepseek/deepseek-v3.2** — 通用默认（128K ctx, $0.960/$2.88，DeepSeek Sparse Attention）
- 编程备选：`qwen/qwen3-coder-480b`（Coder 旗舰）、`moonshot/kimi-k2.7-code`（2026-06 编程旗舰）
- 推理备选：`deepseek/deepseek-r1`、`minimax/m2.7`、`zhipu/glm-5.1`

---

## 数据来源（访问日期 2026-07-09）

- 官网首页：https://therouter.ai/
- 全量模型清单（291 卡片）：https://therouter.ai/models/
- 单模型示例：https://therouter.ai/models/anthropic--claude-sonnet-4.5/（含 `Copy model ID` 徽标显示 canonical id 为 `anthropic/claude-sonnet-4.5`）
- API 参考：https://therouter.ai/models/anthropic--claude-sonnet-4.5/api/
- Quickstart：https://therouter.ai/docs/quickstart/
- SDK 概览：https://therouter.ai/docs/sdks/typescript/overview/
- Pricing：https://therouter.ai/pricing/
- 探测验证（curl）：`/v1/messages`、`/v1/chat/completions`、`/v1/responses`、`/v1beta/models/{m}:generateContent`、`:streamGenerateContent` 均 401（路由存在）；`/` 返 404（无关路由）；`/v1/models` 401
- 当前 aidog preset：`src-tauri/defaults/platform-presets.json:1959-2002`

## Caveats / 不确定项

1. **Canonical id 双重表示**：URL slug 与 sitemap 用 `provider--model-id`（双横），但 API body **必填 `provider/model-id`**（单斜杠）——所有官方代码示例（curl / OpenAI SDK / Python）一致。改 preset 时 model_list 字段应用单斜杠格式。
2. **aidog 现有 preset 的 `protocol: anthropic` 实际可用**（base_url 不变，框架拼出 `/v1/messages` 命中真实路由）。Anthropic 端点支持双鉴权头（`x-api-key` 或 `Authorization: Bearer`），aidog Claude Code client 默认用 `x-api-key` 应可工作（推测，未实测成功请求）。
3. **官方文档仅明示 OpenAI 协议**（首页反复强调 "OpenAI SDK compatible"），Anthropic/Gemini 协议虽实测存在但官方 docs 无独立文档页，可能为内部兼容层；用作 Claude Code 后端时建议保留 anthropic endpoint 但需做一次带有效 key 的实测验证。
4. **`/v1/responses`（OpenAI Responses API）路由存在（401）**，是否完整支持所有模型未在文档明示，推测: 至少 OpenAI 系模型支持。
5. **多模态/audio/video/image/embed 类模型**（约 53 个）不能作 chat 用，填 `model_list` 时建议排除或单列。可作 chat 用的约 238 个。
6. **`anthropic/claude-*` 命名提前到 5.x / 4.8 / fable-5**：与 Anthropic 官网当前对齐到 2026-06 的产品线；aidog 内 `STATIC_MODEL_IDS`（passthrough.rs）若以月级腐化检查，应同步包含这些 id。
7. **本文档不含 `last_updated` Unix 秒数**：preset JSON 内 therouter 条目无 `last_updated` 字段，与平台 default 约定一致（仅部分协议有）。补 model_list 时无需加该字段。
8. **价格已含 TheRouter 聚合折扣**（与各原厂 API 直连价略有差异），适合作 `est_cost` 估算输入；但 aidog 内 `resolve_price` 回退链仍以 `price_sync.rs` 同步的实时价为优先，preset 价仅兜底。
