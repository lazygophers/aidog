# Research: CherryIN 全量模型与 Endpoints 研究

- **Query**: 查清 CherryIN (open.cherryin.net) 全量模型清单 + API endpoint 形态，为 aidog `platform-presets.json` cherryin 协议补全提供权威数据源
- **Scope**: external（CherryIN 官方网关 API）
- **Date**: 2026-07-09

## 核心结论（TL;DR）

- CherryIN 是基于开源 **new-api** 的聚合路由网关，当前全量 **155 个模型**，跨 14 家供应商（Anthropic / OpenAI / Google / DeepSeek / Moonshot / 智谱 GLM / xAI Grok / MiniMax / 阿里 Qwen / 字节 / 腾讯 / BAAI / 快手 / 阶跃）。
- 现有 preset 13 个模型里 **`grok-4`（无前缀）官方不存在**，官方真名是 `x-ai/grok-4` → 必须修正为带前缀形式。
- 其余 12 个全部在官方清单内可核验。
- 端点契约：base = `https://open.cherryin.net`（不含 `/v1`），由 path 自带版本前缀。

---

## 数据来源

| 来源 | URL | 说明 |
|---|---|---|
| 官方定价/模型 API（公开免鉴权） | `https://open.cherryin.net/api/pricing` | new-api 标准公开端点，返回全量模型 + 端点 + 计费规则。本次返回 `pricing_version=5a90f2b8...`，155 条数据。访问日期 2026-07-09 |
| 官方网关根 | `https://open.cherryin.net/` | SPA，标题 "New API"，`<meta generator="new-api">`，确证是 new-api 框架 |
| 官方网关 `/v1/models` | `https://open.cherryin.net/v1/models` | 需 token，未鉴权返回 `Invalid token`。仅 `api/pricing` 公开。 |
| 项目主页 | `https://cherryin.net` | 自述：统一 AI 模型聚合与分发网关，跨格式转换为 OpenAI / Claude / Gemini 兼容接口 |

## API Endpoints

### 端点类型与路径（官方 `supported_endpoint` 字段）

来源：`/api/pricing` 响应顶层 `supported_endpoint` 对象。

| 协议（endpoint type） | HTTP 方法 | 路径 | 备注 |
|---|---|---|---|
| `anthropic` | POST | `/v1/messages` | Claude 协议；鉴权 `x-api-key: <token>` |
| `openai` | POST | `/v1/chat/completions` | OpenAI Chat Completions；鉴权 `Authorization: Bearer <token>` |
| `openai-response` | POST | `/v1/responses` | OpenAI Responses API（GPT-5 系/codex 系原生走此） |
| `gemini` | POST | `/v1beta/models/{model}:generateContent` | Gemini 协议；`{model}` 为 model id（需 URL 编码，含 `/`） |
| `embeddings` | POST | `/v1/embeddings` | 向量模型（bge-m3 / qwen3-embedding-*） |
| `image-generation` | POST | `/v1/images/generations` | 文生图（gpt-image-* / qwen-image*） |
| `jina-rerank` | POST | `/v1/rerank` | 重排（qwen3-reranker-* / bge-reranker） |

### Base URL 与 aidog preset 约定

- 官方路径**自带版本前缀**（`/v1/...`、`/v1beta/...`），因此 base_url **不含** `/v1`：`https://open.cherryin.net`
- 与 aidog CLAUDE.md「base_url 含版本前缀」约束的关系（推断，`推测:`）：
  - **anthropic / gemini 协议**：preset `base_url` 写裸 host `https://open.cherryin.net`（现有 preset 即如此，正确）。aidog anthropic 适配器内部拼 `/v1/messages`，gemini 适配器拼 `/v1beta/models/.../generateContent`。
  - **openai 协议**：aidog 约束要求 `base_url` 含 `/v1`，写 `https://open.cherryin.net/v1`（路径仅拼 `/chat/completions`）。Codex TUI / 普通 OpenAI client 走此。
- 单 host 三协议同源：所有端点共用同一域名 `open.cherryin.net`，仅 path 区分；token 在所有协议间通用（new-api 统一鉴权层）。

### Model id 格式

- **统一为 `<provider>/<model>` 形态**（与新 API vendor 路由对齐），例：
  - `anthropic/claude-opus-4.8`、`openai/gpt-5.5`、`google/gemini-3-pro-preview`、`deepseek/deepseek-v4-pro`、`moonshotai/kimi-k2.7-code`、`z-ai/glm-5.2`、`x-ai/grok-4`、`minimax/minimax-m3`、`qwen/qwen3.7-max`、`agent/<...>`（agent 是平台聚合的代理通道前缀，见下方说明）。
- **裸 id `grok-4` 官方不存在**（现有 preset 写法错误），官方真名 `x-ai/grok-4`。

### `agent/*` 前缀说明

`agent/` 是 CherryIN 内部「聚合通道」前缀（与具名供应商 `deepseek/`、`moonshotai/`、`z-ai/`、`qwen/`、`minimax/` 并列）。同一底层模型常有两条 entry：
- `deepseek/deepseek-v4-pro`（走 DeepSeek 官方上游）
- `agent/deepseek-v4-pro`（走平台聚合通道，价同或不同）

`agent/` 系支持 anthropic+openai 双协议，与新 API 的「agent 网关」路由对齐。**aidog preset 推荐用 `agent/` 形态**（与现有 13 个的 `agent/glm-5.2` 风格一致，双协议适配更广）。

---

## 全量模型清单（155 条，按供应商分组）

字段说明：`ep=` supported_endpoint_types；`ratio=` input model_ratio（USD / 1K token 的倍率基准）；`cr=` completion_ratio（输出相对输入倍数）。计费 `USD = (in_tokens × ratio + out_tokens × ratio × cr) × group_ratio(=1) × 0.002`（new-api 标准，`推测:` 0.002 = 1 美元 = 500 quota 单位）。`(free)` 后缀为 0 倍率免费模型。

### Anthropic 系（vendor 2，9 条，全部支持 anthropic 协议）

| model id | ep | ratio | cr |
|---|---|---|---|
| `anthropic/claude-haiku-4.5` | anthropic, openai | 0.5 | 5 |
| `anthropic/claude-opus-4.5` | openai, anthropic | 2.5 | 5 |
| `anthropic/claude-opus-4.6` | anthropic, openai | 2.5 | 5 |
| `anthropic/claude-opus-4.7` | openai, anthropic | 2.5 | 5 |
| `anthropic/claude-opus-4.8` | openai, anthropic | 2.5 | 5 |
| `anthropic/claude-sonnet-4` | openai, openai-response, anthropic | 3 | 3.75 |
| `anthropic/claude-sonnet-4.5` | anthropic, openai | 3 | 3.75 |
| `anthropic/claude-sonnet-4.6` | anthropic, openai | 1.5 | 5 |
| `anthropic/claude-sonnet-5` | anthropic, openai | 1 | 5 |

### OpenAI 系（vendor 11，30 条）

| model id | ep | ratio | cr |
|---|---|---|---|
| `openai/gpt-4.1` | openai, anthropic | 1 | 4 |
| `openai/gpt-4.1-mini` | openai, anthropic | 0.2 | 4 |
| `openai/gpt-4.1-nano` | openai, anthropic | 0.05 | 4 |
| `openai/gpt-4o` | openai, anthropic | 1.25 | 4 |
| `openai/gpt-4o-mini` | openai, anthropic | 0.075 | 4 |
| `openai/gpt-5` | openai-response, anthropic | 0.625 | 8 |
| `openai/gpt-5-chat` | openai-response, anthropic | 0.625 | 8 |
| `openai/gpt-5-codex` | openai-response | 0.625 | 8 |
| `openai/gpt-5-mini` | openai-response, anthropic | 0.125 | 8 |
| `openai/gpt-5-nano` | openai-response, anthropic | 0.025 | 8 |
| `openai/gpt-5-pro` | openai-response | 7.5 | 8 |
| `openai/gpt-5.1` | openai-response, anthropic | 0.625 | 8 |
| `openai/gpt-5.1-chat` | openai-response, anthropic | 0.625 | 8 |
| `openai/gpt-5.1-codex` | openai-response | 0.625 | 8 |
| `openai/gpt-5.2` | openai-response, anthropic | 0.875 | 8 |
| `openai/gpt-5.2-chat` | openai-response, anthropic | 0.875 | 8 |
| `openai/gpt-5.2-codex` | openai-response | 0.875 | 8 |
| `openai/gpt-5.3-chat` | openai-response, anthropic | 0.875 | 8 |
| `openai/gpt-5.3-codex` | openai-response | 0.875 | 8 |
| `openai/gpt-5.4` | openai-response, anthropic | 1.25 | 6 |
| `openai/gpt-5.4-mini` | openai-response, anthropic | 0.375 | 6 |
| `openai/gpt-5.4-nano` | openai-response, anthropic | 0.1 | 6.25 |
| `openai/gpt-5.4-pro` | openai-response | 30 | 4.5 |
| `openai/gpt-5.5` | openai-response, anthropic | 2.5 | 6 |
| `openai/gpt-image-1` | image-generation | 5 | 4 |
| `openai/gpt-image-2` | image-generation | 4 | 3.75 |
| `openai/o1` | openai, anthropic | 7.5 | 4 |
| `openai/o1-mini` | openai, anthropic | 0.55 | 4 |
| `openai/o3` | openai, anthropic | 1 | 4 |
| `openai/o4-mini` | openai, anthropic | 0.55 | 4 |

注：`gpt-5*` 系原生走 **openai-response**（`/v1/responses`），需 codex_tui / OpenAI Responses 客户端；普通 openai `/v1/chat/completions` 不支持（除 4.1/4o 系），aidog preset 应配 `codex_tui` client_type。

### Google 系（vendor 3，11 条，全部支持 gemini + openai + anthropic）

| model id | ratio | cr |
|---|---|---|
| `google/gemini-2.5-flash` | 0.15 | 8.33 |
| `google/gemini-2.5-flash-image` | 0.15 | 100 |
| `google/gemini-2.5-flash-lite` | 0.05 | 4 |
| `google/gemini-2.5-pro` | 1.25 | 6 |
| `google/gemini-3-flash-preview` | 0.25 | 6 |
| `google/gemini-3-pro-image-preview` | 1 | 60 |
| `google/gemini-3-pro-preview` | 2 | 4.5 |
| `google/gemini-3.1-flash-image-preview` | 0.25 | 120 |
| `google/gemini-3.1-flash-lite-preview` | 0.125 | 6 |
| `google/gemini-3.1-pro-preview` | 2 | 4.5 |
| `google/gemini-3.5-flash` | 0.75 | 6 |

### DeepSeek 系（vendor 6，11 条，含 agent 通道）

| model id | ep | ratio | cr |
|---|---|---|---|
| `agent/deepseek-v3.2` | openai, anthropic | 0.143 | 1.5 |
| `agent/deepseek-v3.2(free)` | openai, anthropic | 0 | 1 |
| `agent/deepseek-v4-flash` | anthropic, openai | 0.075 | 2 |
| `agent/deepseek-v4-pro` | anthropic, openai | 0.225 | 2 |
| `deepseek/deepseek-ocr(free)` | openai | 0 | 1 |
| `deepseek/deepseek-v3.1-terminus` | openai | 0.135 | 3.70 |
| `deepseek/deepseek-v3.2` | openai, anthropic | 0.143 | 1.5 |
| `deepseek/deepseek-v3.2(free)` | openai, anthropic | 0 | 1 |
| `deepseek/deepseek-v4-flash` | anthropic, openai | 0.075 | 2 |
| `deepseek/deepseek-v4-flash(free)` | openai, anthropic | 0 | 1 |
| `deepseek/deepseek-v4-pro` | anthropic, openai | 0.225 | 2 |

### Moonshot / Kimi 系（vendor 7，12 条，含 agent 通道）

| model id | ep | ratio | cr |
|---|---|---|---|
| `agent/kimi-k2-0905` | openai, anthropic | 0.27 | 4 |
| `agent/kimi-k2-thinking` | openai, anthropic | 0.27 | 4 |
| `agent/kimi-k2.5` | anthropic | 0.3 | 5 |
| `agent/kimi-k2.6` | openai, anthropic | 0.475 | 4.16 |
| `agent/kimi-k2.7-code` | openai, anthropic | 0.48 | 4.15 |
| `moonshotai/kimi-k2-0905` | openai | 0.3 | 4.17 |
| `moonshotai/kimi-k2-instruct` | openai, anthropic | 0.5 | 3 |
| `moonshotai/kimi-k2-thinking` | openai, anthropic | 0.27 | 4 |
| `moonshotai/kimi-k2-thinking-turbo` | openai, anthropic | 0.55 | 7.25 |
| `moonshotai/kimi-k2.5` | anthropic, openai | 0.3 | 5 |
| `moonshotai/kimi-k2.6` | openai, anthropic | 0.475 | 4.16 |
| `moonshotai/kimi-k2.7-code` | openai, anthropic | 0.48 | 4.15 |

### 智谱 GLM 系（vendor 9，10 条，含 agent 通道）

| model id | ep | ratio | cr |
|---|---|---|---|
| `agent/glm-4.6` | openai | 0.27 | 4 |
| `agent/glm-4.7` | openai | 0.3 | 3.3 |
| `agent/glm-5` | openai, anthropic | 0.43 | 3.67 |
| `agent/glm-5.1` | openai, anthropic | 0.589 | 3.50 |
| `agent/glm-5.2` | openai, anthropic | 0.59 | 3.5 |
| `z-ai/glm-4.6` | openai, anthropic | 0.3 | 3.3 |
| `z-ai/glm-4.7` | openai | 0.3 | 3.3 |
| `z-ai/glm-5` | openai, anthropic | 0.43 | 3.67 |
| `z-ai/glm-5.1` | openai, anthropic | 0.589 | 3.50 |
| `z-ai/glm-5.2` | openai, anthropic | 0.59 | 3.5 |

### xAI Grok 系（vendor 10，9 条，全部仅 openai/openai-response，不支持 anthropic）

| model id | ep | ratio | cr |
|---|---|---|---|
| `x-ai/grok-3` | openai, openai-response | 1.5 | 5 |
| `x-ai/grok-3-mini` | openai, openai-response | 0.15 | 1.67 |
| `x-ai/grok-4` | openai, openai-response | 1.5 | 5 |
| `x-ai/grok-4-1-fast-non-reasoning` | openai, openai-response | 0.1 | 2.5 |
| `x-ai/grok-4-1-fast-reasoning` | openai, openai-response | 0.1 | 2.5 |
| `x-ai/grok-4-fast-non-reasoning` | openai, openai-response | 0.1 | 2.5 |
| `x-ai/grok-4-fast-reasoning` | openai, openai-response | 0.1 | 2.5 |
| `x-ai/grok-4.3` | openai, openai-response | 0.625 | 2 |
| `x-ai/grok-code-fast-1` | openai, openai-response | 0.1 | 7.5 |

### MiniMax 系（vendor 30，11 条，含 agent 通道）

| model id | ep | ratio | cr |
|---|---|---|---|
| `agent/minimax-m2.5` | anthropic, openai | 0.15 | 4 |
| `agent/minimax-m2.5-highspeed` | openai, anthropic | 0.3 | 4 |
| `agent/minimax-m2.7` | openai, anthropic | 0.15 | 4 |
| `agent/minimax-m2.7-highspeed` | openai, anthropic | 0.3 | 4 |
| `minimax/minimax-m2.1` | openai, anthropic | 0.15 | 4 |
| `minimax/minimax-m2.1--lightning` | openai, anthropic | 0.15 | 4 |
| `minimax/minimax-m2.5` | openai | 0.15 | 4 |
| `minimax/minimax-m2.5-highspeed` | openai, anthropic | 0.3 | 4 |
| `minimax/minimax-m2.7` | openai, anthropic | 0.15 | 4 |
| `minimax/minimax-m2.7-highspeed` | openai, anthropic | 0.3 | 4 |
| `minimax/minimax-m3` | openai, anthropic | 0.31 | 4 |

### 阿里 Qwen 系（vendor 8，44 条，含 agent 通道）

文本/对话（openai, anthropic）:

| model id | ratio | cr |
|---|---|---|
| `agent/qwen3.5-flash` | 0.086 | 10 |
| `agent/qwen3.5-plus` | 0.286 | 6 |
| `agent/qwen3.6-plus` | 0.143 | 6 |
| `agent/qwen3.7-max` | 0.885 | 2.99 |
| `qwen/qwen3-235b-a22b-instruct-2507` | 0.045 | 6.67 |
| `qwen/qwen3-235b-a22b-thinking-2507` | 0.039 | 4 |
| `qwen/qwen3-30b-a3b-instruct-2507` | 0.1 | 4 |
| `qwen/qwen3-30b-a3b-instruct-2507(free)` | 0 | 1 |
| `qwen/qwen3-30b-a3b-thinking-2507` | 0.045 | 6.67 |
| `qwen/qwen3-coder-30b-a3b-instruct(free)` | 0 | 1 |
| `qwen/qwen3-coder-480b-a35b-instruct` | 0.11 | 4.32 |
| `qwen/qwen3-coder-flash` | 0.35 | 5 |
| `qwen/qwen3-coder-plus` | 1.4 | 10 |
| `qwen/qwen3-max` | 0.68 | 4 |
| `qwen/qwen3-next-80b-a3b-instruct` | 0.07 | 10 |
| `qwen/qwen3-vl-235b-a22b-instruct` | 0.15 | 5 |
| `qwen/qwen3-vl-235b-a22b-thinking` | 0.15（仅 openai） | 5 |
| `qwen/qwen3-vl-30b-a3b-instruct(free)` | 0 | 1 |
| `qwen/qwen3-vl-30b-a3b-thinking(free)` | 0 | 1 |
| `qwen/qwen3-vl-flash` | 0.04 | 10 |
| `qwen/qwen3-vl-plus` | 0.21 | 10 |
| `qwen/qwen3.5-122b-a10b`（仅 openai） | 0.057 | 8 |
| `qwen/qwen3.5-27b`（仅 openai） | 0.043 | 8 |
| `qwen/qwen3.5-35b-a3b`（仅 openai） | 0.029 | 8 |
| `qwen/qwen3.5-35b-a3b(free)` | 0 | 1 |
| `qwen/qwen3.5-397b-a17b`（仅 openai） | 0.086 | 6 |
| `qwen/qwen3.5-4b(free)` | 0 | 1 |
| `qwen/qwen3.5-9b(free)` | 0 | 1 |
| `qwen/qwen3.5-flash` | 0.086 | 10 |
| `qwen/qwen3.5-plus` | 0.286 | 6 |
| `qwen/qwen3.6-plus` | 0.143 | 6 |
| `qwen/qwen3.7-max` | 0.885 | 2.99 |

向量/重排/绘图:

| model id | ep | ratio |
|---|---|---|
| `qwen/qwen3-embedding-0.6b` | embeddings | 0.0048 |
| `qwen/qwen3-embedding-0.6b(free)` | openai | 0 |
| `qwen/qwen3-embedding-4b` | openai | 0.005 |
| `qwen/qwen3-embedding-8b` | openai | 0.28 |
| `qwen/qwen3-reranker-0.6b` | jina-rerank | 0.0048 |
| `qwen/qwen3-reranker-0.6b(free)` | openai | 0 |
| `qwen/qwen3-reranker-4b` | openai | 0.0096 |
| `qwen/qwen3-reranker-8b` | openai | 0.28 |
| `qwen/qwen-image(free)` | image-generation | 0 |
| `qwen/qwen-image-edit(free)` | image-generation | 0 |
| `qwen/qwen-image-edit-2509(free)` | image-generation | 0 |

### 其他

| model id | 供应商 | ep | ratio |
|---|---|---|---|
| `bytedance/seed-oss-36b-instruct(free)` | 字节跳动 | openai, anthropic | 0 |
| `tencent/hunyuan-mt-7b(free)` | 腾讯 | openai, anthropic | 0 |
| `kwai-kolors/kolors(free)` | 快手 | openai | 0 |
| `stepfun-ai/step-3.5-flash(free)` | 阶跃星辰 | openai, anthropic | 0 |
| `BAAI/bge-reranker-v2-m3` | BAAI | openai | 0 |
| `BAAI/bge-reranker-v2-m3(free)` | BAAI | openai | 0 |
| `baai/bge-m3` | BAAI | embeddings | 0.01 |
| `baai/bge-m3(free)` | BAAI | openai | 0 |

---

## 三档默认推荐（供 `models.default`）

按 aidog preset 风格（`agent/` 聚合通道优先，双协议适配；旗舰代号最新）:

| 档位 | 推荐 model id | 说明 |
|---|---|---|
| **Claude 默认** | `anthropic/claude-opus-4.8` | 最新 opus 旗舰，anthropic 协议原生 |
| **OpenAI 默认** | `openai/gpt-5.5` | 最新 gpt 旗舰；注意原生走 openai-response，需配 codex_tui / Responses client；若用 chat completions 走 anthropic 兼容分支 |
| **国产默认** | `agent/glm-5.2` | 智谱最新旗舰，双协议，与现有 13 模型风格一致（现有即用 agent/glm-5.2）；备选 `deepseek/deepseek-v4-pro`（DeepSeek 系旗舰，性价比 0.225/2 倍） |

> 推荐 `models.default` 同时落 3 个：`["anthropic/claude-opus-4.8", "openai/gpt-5.5", "agent/glm-5.2"]`，覆盖三档客户端默认。

---

## 现有 13 模型核对

| 现有 model id | 官方核对 | 备注 |
|---|---|---|
| `anthropic/claude-opus-4.8` | ✅ | 最新 opus |
| `anthropic/claude-sonnet-4.6` | ✅ | |
| `anthropic/claude-opus-4.5` | ✅ | 旧旗舰，可保留 |
| `openai/gpt-5.5` | ✅ | |
| `openai/gpt-5.3-codex` | ✅ | codex 专款 |
| `google/gemini-3.5-flash` | ✅ | |
| `google/gemini-3-pro-preview` | ✅ | |
| `deepseek/deepseek-v4-pro` | ✅ | |
| `deepseek/deepseek-v4-flash` | ✅ | |
| `deepseek/deepseek-v3.2` | ✅ | |
| `agent/glm-5.2` | ✅ | 与 `z-ai/glm-5.2` 同价同协议 |
| `moonshotai/kimi-k2.7-code` | ✅ | |
| `grok-4` | ⚠️ **待修正** | 官方清单无裸 `grok-4`，官方真名 `x-ai/grok-4`（且 grok 系**不支持 anthropic 协议**，仅 openai/openai-response，现有 anthropic 单端点 preset 下走不通） |

---

## 关键 caveat / 待确认

1. **`grok-4` 必须改 `x-ai/grok-4`**：官方清单里 grok 系全部带 `x-ai/` 前缀，裸 id 不存在；且 `x-ai/*` 系**全部不支持 anthropic endpoint**，现有 cherryin preset 只有 anthropic 单端点 → grok-4 在当前 preset 下不可用。若补全需新增 openai 端点（`base_url: https://open.cherryin.net/v1`，`client_type: codex_tui` 或 default）。

2. **base_url 写法分歧**：CLAUDE.md 约束「base_url 含版本前缀」是 aidog preset 风格指南，但现有 cherryin preset 的 anthropic 端点 base_url 是 `https://open.cherryin.net`（无 `/v1`）。`推测:` aidog 的 anthropic / gemini 适配器内部会自动拼版本前缀，因此裸 host 写法对这两协议正确；但**新增 openai 端点时必须写 `https://open.cherryin.net/v1`**（因 openai 适配器 `provider_api_path()` 仅返回 `/chat/completions`）。

3. **`agent/*` vs 具名前缀取舍**：同一底层模型有 `agent/X` 与 `<vendor>/X` 双 entry。agent 通道为平台聚合路由（可能多上游容灾），具名前缀为单上游。**价同**的情况下推荐 `agent/`（与现有 13 风格一致，但需确认单上游的 `<vendor>/` 是否更稳定）。

4. **OpenAI gpt-5 系协议特殊性**：`gpt-5*` / `gpt-5.1*` / `gpt-5.2*` / `gpt-5.3*` / `gpt-5.4*` / `gpt-5.5` 全部原生走 **openai-response**（`/v1/responses`），普通 `/v1/chat/completions` **不支持**（除非走 anthropic 协议分支——aidog 转换器是否支持 openai-response 协议待 codex_tui 适配层确认，`推测:` codex_tui client_type 应原生支持）。

5. **未知上下文窗口**：pricing API 不返回 context window 字段，需查各供应商官方；aidog preset `models.default` 为空 object 时不影响（context 仅用于 UI 展示，可后续手补）。

6. **`(free)` 0 倍率模型**：标 `(free)` 的模型不消耗 quota，但仍受 RPM 限制（new-api 标准行为）；preset `model_list` 是否纳入由实现方决定。

7. **真实性保证**：所有 model id 字符串、ratio、endpoint 类型均为 2026-07-09 实拉 `/api/pricing` 原始数据（pricing_version `5a90f2b8...`），非营销名、非推测。

## Related Specs

- `src-tauri/defaults/platform-presets.json:2003-2060` — 现有 cherryin 协议 preset（待补全）
- `CLAUDE.md` §「平台默认配置」— preset 真值源约定、`base_url` 版本前缀约束、`models.default`/`model_list` 结构
- `.wiki/modules/pricing.md`（`推测:` 存在）— `est_cost` 估算与 preset model_ratio 的换算关系
