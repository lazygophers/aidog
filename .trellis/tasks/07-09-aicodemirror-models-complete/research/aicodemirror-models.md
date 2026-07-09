# Research: AICodeMirror 全量研究

- **Query**: AICodeMirror (aicodemirror.com) 全量模型清单 + endpoints 形态，为 aidog `platform-presets.json` 补全提供权威数据源
- **Scope**: external（官网/API 探测）+ internal（preset 对齐）
- **Date**: 2026-07-09

---

## 结论速览（TL;DR）

1. **AICodeMirror 是纯 Claude 代理共享平台**（"Claude Code 官方共享平台"，前身为 Claude Mirror），**仅支持 Claude 系模型**，不是多供应商聚合。
2. **3 个 endpoint 全部确认存活**（HTTP 401 鉴权拦截 = 路由有效），路径为平台自定义前缀，**base_url 是前缀**，客户端需追加协议后缀：
   - anthropic: `https://api.aicodemirror.com/api/claudecode` + `/v1/messages`
   - openai(codex): `https://api.aicodemirror.com/api/codex/backend-api/codex`（codex TUI 专用，**base 即完整请求 URL**）
   - gemini: `https://api.aicodemirror.com/api/gemini` + `/v1beta/models/...`
3. **关键证据**：gemini/codex endpoint 的鉴权报错 `Invalid API Key format. Expected format: sk-ant-api03-xxx` —— 三个 endpoint **都用 Anthropic API key 鉴权**，证明三者都是 Claude 协议翻译网关，底层统一 Claude。
4. **model_list 已正确**：现有 7 个 id 与 aidog 内 18+ 个 Claude 代理 preset（packycode/cubence/aigocode/claudeapi/claudecn/runapi…）完全一致，是项目级 alias 约定。无需扩展。

---

## API Endpoints

### 路由验证（curl 探测，2026-07-09）

| 协议 | preset base_url | 追加后缀 | 探测路径 | 响应 |
|---|---|---|---|---|
| anthropic | `https://api.aicodemirror.com/api/claudecode` | `/v1/messages` | `POST /api/claudecode/v1/messages` | **401** `{"error":"Unauthorized - No authentication provided"}` → `Invalid API Key`（路由存在） |
| openai (codex_tui) | `https://api.aicodemirror.com/api/codex/backend-api/codex` | 无（base 即完整请求 URL） | `POST /api/codex/backend-api/codex` | **401** `{"error":"Unauthorized - No authorization header provided"}` → `Invalid API Key format. Expected format: sk-ant-api03-xxx` |
| gemini | `https://api.aicodemirror.com/api/gemini` | `/v1beta/models/{model}:generateContent` | `GET /api/gemini/v1beta/models` | **401** `{"error":"Unauthorized - No API key provided. Please provide either Authorization header or x-goog-api-key header"}` → `Invalid API Key format. Expected format: sk-ant-api03-xxx` |

> `api.aicodemirror.com` 子域为代理网关（Next.js，`<title>Claude Mirror Proxy Service</title>`）；`www.aicodemirror.com` 为 dashboard/官网。两者分离。

### 路径含义解释

- **`/api/claudecode`**：Claude Code CLI 专用网关前缀。Anthropic 协议路径直接挂在该前缀下 → 完整请求 URL = `base_url + /v1/messages`。这与 aidog `URL 构造` 约定（`base_url + provider_api_path`，anthropic 的 `provider_api_path = /v1/messages`）天然吻合。
- **`/api/codex/backend-api/codex`**：OpenAI Codex CLI（codex TUI）专用网关。Codex CLI 不用标准 `/v1/chat/completions`，而是直接 POST 到 `/backend-api/codex`（复刻 OpenAI 官方 Codex backend 路径）。**base_url 本身即完整请求 URL**，不再追加。aidog 的 `client_type: codex_tui` 走专用 adapter，不会误拼 `/chat/completions`。
- **`/api/gemini`**：Gemini CLI 专用网关前缀，下挂 Google `v1beta` REST 形态（`/v1beta/models/...`）。完整请求 URL = `base_url + /v1beta/models/{model}:generateContent`。

### 鉴权方式

- **三协议统一用 Anthropic API key 格式**（`sk-ant-api03-xxx`）—— gemini/codex endpoint 的 401 报错直接揭示这一点。
- anthropic 路由：`Authorization: Bearer <key>` 或 `x-api-key: <key>`（两者都被接受，无 token 时报 "No authentication provided"，有 token 但无效时报 "Invalid API Key"）。
- gemini 路由：`Authorization: Bearer <key>` 或 `x-goog-api-key: <key>`。
- codex 路由：`Authorization: Bearer <key>`。
- key 由用户在 dashboard 注册/订阅后获取（订阅 FREE/PRO/MAX/ULTRA 4 档，按 credit 额度区分；`/api/pricing` 返 plans）。

---

## 模型范围确认

**仅 Claude 系**。证据链：

1. 官网自我定位："**Claude Code 官方共享平台**"（前身为 Claude Mirror，域名 aicodemirror.com 保留以维持连续性，见 `/about-claude-code` 页）。
2. footer "Claude 模型" 栏仅列 3 个营销名：**Claude Opus 4 / Claude Sonnet 4.5 / Claude Haiku 3.5**。
3. 首页 hero 强调 "企业级 **Claude Sonnet 5**"。
4. pricing 页 FAQ 明示："所有订阅/按量付费模式，均可使用 **Claude Code、Codex 和 Gemini CLI**。我们完全使用官方服务，因此总是第一时间支持最新模型。" —— 即 3 种客户端协议都连官方 Claude 后端。
5. 三 endpoint 鉴权全部要求 `sk-ant-api03-xxx`（Anthropic key 格式），不存在 OpenAI / Google 原生 key 通道。
6. `/api/pricing` 公开 API 仅返订阅套餐（FREE/PRO/MAX/ULTRA + credit 额度），**无 per-model 价格表、无模型清单字段**（与 new-api/one-api 系聚合站不同，AICodeMirror 不暴露模型列表 API）。

**不支持**：OpenAI(gpt/o3)、Google(gemini 原生)、DeepSeek、Qwen、GLM、Kimi、MiniMax、Grok 等。`gemini` / `codex` endpoint 名只是**客户端协议入口**，不是独立模型供应商。

---

## 全量模型清单

### Claude 系（唯一支持的家族）

AICodeMirror 官方未公开"API 调用级 model id 字符串"清单（dashboard 登录后可能展示，公开页面只给营销名）。公开可确认的营销名 → aidog alias 映射：

| 官网营销名 | 出现位置 | aidog preset alias（推测对应） |
|---|---|---|
| Claude Opus 4 | footer / 首页 | `claude-opus-4-8`（最新档别名，aidog 约定） |
| Claude Sonnet 4.5 | footer | `claude-sonnet-4-5` / `claude-sonnet-4-6` |
| Claude Haiku 3.5 | footer | `claude-haiku-4-5` |
| Claude Sonnet 5 | 首页 hero | （aidog 主线 `anthropic` preset 用 `claude-sonnet-5`，AICodeMirror 暂未列） |

### OpenAI 系 / Google 系 / 国产系 / 其他

**均无**（见上节"模型范围确认"）。`openai`/`gemini` endpoint 仅作协议翻译入口，底层仍 Claude。

### 关于 model id 字符串精确性

- AICodeMirror 作为代理，对客户端发送的 `model` 字段做透传/映射到官方 Claude；平台未公开"接受哪些 id 字符串"白名单。
- aidog 项目对 Claude 代理平台使用一套**内部 alias 约定**（`claude-opus-4-8` / `claude-sonnet-4-6` / `claude-haiku-4-5` / `claude-opus-4-7` / `claude-opus-4-6` / `claude-opus-4-5` / `claude-sonnet-4-5`），**不是**真实 Anthropic id（如 `claude-opus-4-5-20251101`），而是项目内统一短别名。
- 该 alias 集**已在 18+ 个同类 Claude 代理 preset 中复用**（packycode/cubence/aigocode/rightcode/ccsub/apikeyfun/apinebula/sudocode/claudeapi/claudecn/runapi/relaxycode/crazyrouter/sssaicode/compshare_coding/pateway/aicodemirror…），保持一致性比追平台未公开的真名更重要。

---

## 三档默认推荐（供 `models.default`）

平台 Claude-only，三档只能在 Claude 系内分档：

| 档位 | 推荐 alias | 对应营销名 | 用途 |
|---|---|---|---|
| Sonnet 档（默认主力） | `claude-sonnet-4-6` | Claude Sonnet 4.5 | 性价比 / 高吞吐 |
| Opus 档（重型） | `claude-opus-4-8` | Claude Opus 4 | 复杂分析 / 长任务 |
| Haiku 档（轻量） | `claude-haiku-4-5` | Claude Haiku 3.5 | 快速轻量动作 |

> OpenAI 档 / 国产档：**不适用**（平台不支持）。

`models.default` 当前为空 `{}`，与 aidog 其他 Claude 代理 preset 一致（model_list.default 已覆盖），保持现状即可。

---

## 现有 7 模型核对

`src-tauri/defaults/platform-presets.json` `protocols.aicodemirror.model_list.default`（line 2329-2337）：

```
claude-opus-4-8       ✅ aidog 标准 alias（最新 Opus 档）
claude-sonnet-4-6     ✅ aidog 标准 alias（最新 Sonnet 档）
claude-haiku-4-5      ✅ 对应官方 Claude Haiku 3.5
claude-opus-4-7       ✅ aidog 标准 alias（上一代 Opus）
claude-opus-4-6       ✅ aidog 标准 alias
claude-opus-4-5       ✅ aidog 标准 alias
claude-sonnet-4-5     ✅ aidog 标准 alias
```

**结论**：7 个 id 与 18 个兄弟 Claude 代理 preset 完全同构，覆盖平台全部公开 Claude 营销名（Opus 4 / Sonnet 4.5 / Haiku 3.5）的 aidog alias 投影。**无需增删**。

可选微调（非必须）：
- 若要对齐首页 hero 宣传的 "Claude Sonnet 5"，可补 `claude-sonnet-5`（aidog 主 `anthropic` preset 已用），但官方 footer 未列 Sonnet 5，平台是否真开通待用户验证 → 建议暂不补。
- 首页 hero "Claude Sonnet 5" 可能是营销前瞻，footer 实际仍以 Opus 4 / Sonnet 4.5 / Haiku 3.5 为准。

---

## Caveats / Not Found

1. **官方 model id 白名单不可得**：dashboard 需登录，公开 `/api/pricing` 不含模型字段，无 new-api 风格 `/api/models` 免鉴权端点（`/api/models`、`/v1/models` 均返 Next.js 404）。本研究的 id 精确性建立在"aidog 项目级 alias 约定 + 兄弟 preset 一致性"之上，而非平台官方文档直陈。
2. **Sonnet 5 疑虑**：首页 hero 出现 "Claude Sonnet 5"，但 footer 模型栏未列，平台是否实际开通 sonnet-5 路由未验证（需有效订阅 key 实测）。
3. **codex endpoint 是否真无后缀**：探测 401 证明 `/api/codex/backend-api/codex` 路由存在，但未用有效 key 实测请求体格式；推测: 该路径复刻 OpenAI Codex 官方 backend-api，codex CLI 直接 POST 此路径无额外 `/chat/completions`，aidog `client_type: codex_tui` adapter 已处理。
4. **OpenClaw**：首页提到 "同时支持 Claude Code Codex Gemini CLI **OpenClaw**"，第四个客户端协议，aidog preset 未列其 endpoint（推测: OpenClaw 是另一 CLI 工具，可能复用 anthropic 或 openai endpoint，未深查）。
5. 未查 status 页（status.aicodemirror.com）的模型可用性实时信息。

---

## 数据来源（URL + 2026-07-09）

- 官网首页：https://www.aicodemirror.com/ （footer 模型栏 + hero "Claude Sonnet 5"）
- 使用教程：https://www.aicodemirror.com/docs （仅教程合集，无 API 细节）
- 定价：https://www.aicodemirror.com/pricing （FAQ "均可使用 Claude Code、Codex 和 Gemini CLI"）
- 关于：https://www.aicodemirror.com/about-claude-code （"Claude Code 官方共享平台"，原 Claude Mirror）
- 定价 API（免鉴权）：`GET https://www.aicodemirror.com/api/pricing` → plans(FREE/PRO/MAX/ULTRA)，**无模型字段**
- 网关探测（curl）：
  - `POST https://api.aicodemirror.com/api/claudecode/v1/messages` → 401 Unauthorized
  - `POST https://api.aicodemirror.com/api/codex/backend-api/codex` → 401 "Expected format: sk-ant-api03-xxx"
  - `GET https://api.aicodemirror.com/api/gemini/v1beta/models` → 401 "Expected format: sk-ant-api03-xxx"
- 内部对齐：`src-tauri/defaults/platform-presets.json` `protocols.aicodemirror`（line 2304-2365）+ 18 个兄弟 Claude 代理 preset 的 alias 集
