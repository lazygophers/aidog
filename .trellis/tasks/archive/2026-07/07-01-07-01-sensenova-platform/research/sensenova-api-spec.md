# Research: 商汤 SenseNova（日日新）平台 API 接入规格

- **Query**: 调研 SenseNova API 协议 / base_url / 鉴权 / 模型 / Token Plan 配额接口，为加平台 task 的 prd 提供数据
- **Scope**: external（docs 站 + endpoint 主动探测 + GitHub 旁证）+ internal（对照现有平台架构）
- **Date**: 2026-07-01
- **只读调研，未改码**

## 信源获取方式（关键）

`https://platform.sensenova.cn/docs` 是 Next.js 客户端渲染（curl 拿不到正文，RSC payload 仅 shell）。**docs 正文已内联到首页加载的 JS chunk**（`/_next/static/chunks/*.js`）—— 拉全部 19 个 chunk 拼成 1.4MB 文本，grep 抽出 markdown 源文档。所有下述引用来自该 JS 内联的 docs markdown 原文（含中英文 i18n 字符串），辅以 endpoint 主动探测（401 vs 404 区分真假端点）。

---

## Findings

### 1. API 协议 —— OpenAI + Anthropic 双兼容

**同时兼容 OpenAI Chat Completions、Anthropic Messages、OpenAI Images**（docs 原文："SenseNova API is compatible with both OpenAI and Anthropic protocols"）。

| 协议 | 路径 | 方法 | 备注 |
|---|---|---|---|
| OpenAI Chat | `/v1/chat/completions` | POST | 标准 OpenAI 请求体 |
| Anthropic Messages | `/v1/messages` | POST | 标准 Anthropic 请求体（system/max_tokens 等） |
| OpenAI Images | `/v1/images/generations` | POST | 仅 `sensenova-u1-fast` |
| 模型列表 | `/v1/models` | GET | 标准 OpenAI 列表 |

**流式 SSE**：支持。`stream: true` → SSE，`stream_options.include_usage=true` 控制 usage 在末块返回（与 OpenAI 一致）。流式 chunk 形如：
```json
data: {...,"object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":8,"total_tokens":20}}
data: [DONE]
```

**请求参数（OpenAI Chat 侧，docs 原文表格）**：

| 字段 | 类型 | 必填 | 默认 | 说明 |
|---|---|---|---|---|
| `model` | string | ✅ | — | 必须 `sensenova-6.7-flash-lite` 等 |
| `messages` | array | ✅ | — | role ∈ {system, user, assistant, tool} |
| `stream` | bool | — | false | SSE 流式 |
| `stream_options` | object | — | `{"include_usage":true}` | 仅 stream=true |
| `temperature` | float | — | 0.6 | [0,2] |
| `top_p` | float | — | 0.95 | (0,1] |
| `max_tokens` | int | — | 65535 | [1,65536] |
| `n` | int | — | 1 | 1–7 |
| `stop` | string\|array | — | — | 停止序列 |
| `frequency_penalty` / `presence_penalty` | float | — | 0 | [0,2] |
| **`reasoning_effort`** | string | — | `"medium"` | **low / medium / high / none**（推理力度） |
| `tools` / `tool_choice` / `parallel_tool_calls` | — | — | auto | 工具调用，parallel 默认 true |
| `seed` | int | — | — | [0,9999999) beta |

**错误响应**（所有端点统一信封）：
```json
{"error":{"type":"invalid_request_error","code":"3","message":"invalid temperature, should in [0,2]."}}
```
（注意：比标准 OpenAI 多 `code` 数值字符串字段，但 `type`/`message` 兼容）

**HTTP 状态码表**：

| status | type | 含义 |
|---|---|---|
| 400 | `invalid_request_error` | 参数非法 |
| 400 | `failed_precondition_error` | 前置失败（编码/引擎不可用） |
| 403 | `permission_denied_error` | 语言/策略拒绝 |
| 404 | `not_found_error` | 未知/已退役 model id |
| 408 | `canceled_error` | 客户端取消 |
| **429** | **`quota_exceeded_error`** | **配额/速率超限**（→ 命中本项目 auto-disable 429-quota 契约） |
| 500 | `internal_server_error` | 内部错误 |

### 2. base_url —— `https://token.sensenova.cn`

**生产唯一 base_url = `https://token.sensenova.cn`**（不是 `opapi.sensenova.cn`/`api.sensenova.cn`/`platform.sensenova.cn` —— 这三个都不对；`opapi.sensenova.cn` **DNS 不解析**）。

DNS 验证：`token.sensenova.cn → gtm-token.sensenova.cn → 180.184.249.196`（GTM+WAF，sensecore.cn 同套基建）。

端点探测结果（区分 401=真端点 vs 404=假端点）：

| 端点 | GET | POST | 结论 |
|---|---|---|---|
| `/v1/models` | **401** | — | 真端点 |
| `/v1/chat/completions` | 404 | **401** | 真端点 |
| `/v1/messages` | 404 | **401** | 真端点 |
| `/v1/images/generations` | 404 | **401** | 真端点 |
| 其他所有 `/v1/*` | 404 | 404 | 不存在 |

按本项目 url-construction-rule（base_url 含版本前缀 + adapter 拼 `/chat/completions`），两个端点配置：
- **OpenAI 侧**：`base_url = https://token.sensenova.cn/v1`（adapter 拼 `/chat/completions`）
- **Anthropic 侧**：`base_url = https://token.sensenova.cn`（adapter 拼 `/v1/messages`）—— 注意 anthropic 侧 base_url **不含 `/v1`**，docs 明确警告："`ANTHROPIC_BASE_URL` 必须不含 `/v1` 后缀：Claude Code SDK 自动追加 `/v1/messages`，加了会变成 `.../v1/v1/messages` 返回 404"。

### 3. 鉴权 —— `Authorization: Bearer $SENSENOVA_API_KEY`

- **请求头**：`Authorization: Bearer <api_key>`（OpenAI/Anthropic 端点用**同一把 key**，同 header，docs 原文："Uses the same API Key as the OpenAI-compatible endpoint, passed via `Authorization: Bearer` header"）。
- **API Key 申请页**：`https://platform.sensenova.cn/console/keys`（控制台 → API Keys）。
- **Key 前缀**：docs 示例统一写 `sk-xxx`（占位），用户在 console 申请后得到真实 key（推测:实际前缀待用户 key 落地确认，但 docs 用 `sk-` 范式 → 大概率走 KEY_PREFIXES 里 `sk-` 默认匹配）。
- **每账户最多 20 个 key**（docs："Up to 20 keys per account"）。
- **Key 不过期**（docs："They do not expire"）。
- Claude Code 接入用 `ANTHROPIC_AUTH_TOKEN`（同 key）；Cursor/Cline 用 `OPENAI_API_KEY`（同 key）。

### 4. 模型列表 —— 3 个，含 1 个推理模型

docs 原文 `## Models overview` 表格：

| Model Name | Model ID | Request Quota | Description |
|---|---|---|---|
| SenseNova 6.7 Flash-Lite | `sensenova-6.7-flash-lite` | **1500 次 / 5 小时** | 轻量多模态 agent，支持文本聊天 + 图像理解（OCR/图表），256K context（max input 252K, max output 64K） |
| SenseNova U1 Fast | `sensenova-u1-fast` | 1500 次 / 5 小时 | 信息图（infographic）生成加速版，**不能作 chat 模型**，走 `/v1/images/generations` 专用 |
| DeepSeek V4 Flash | `deepseek-v4-flash` | **500 次 / 5 小时** | **thinking mode + 1M context** ← 推理模型 |

**推理模型支持**：是。
- `deepseek-v4-flash` 原生支持 thinking mode（docs："High-performance chat model with thinking mode and 1M context"）。
- 请求参数 `reasoning_effort: low|medium|high|none`（默认 medium）—— 等价于 OpenAI 的 reasoning_effort 协议。
- docs Claude Code FAQ 提及：当前接口 `effort` 只支持 `low/medium/high/max`，`xhigh` 不支持 → 建议设 `high`。（注意：request-params 表写的是 `none` 也支持，FAQ 写的是 `max` —— 两个值需用户实测确认全集，但 `high` 是安全默认）。

### 5. Token Plan 配额查询接口 —— **无 API-Key 配额接口**（类比 [[xiaomi-mimo-token-plan-no-api]]）

**Token Plan 确实存在**（用户判断正确），机制：
- docs 原文（中文 i18n）："公测期间免费畅享 Token Plan，各模型 5 小时窗口独立计数，账户下所有 Key 共享配额"。
- 定价页 i18n：`"quotaLabel":"单模型配额"`,`"quotaValue":"1,500 次 / 5 小时"`,`"quotaFootnote":"每个模型独立计数 · 同账户所有 Key 共享"`，`"accountPlanValue":"Free · 公测"`。
- 控制台 `/token-plan` 页有 quota dashboard，i18n：`"quotaTitle":"当前窗口调用余量 · 按模型"`,`"chartTitle":"Token 用量趋势"`,`"legendRemaining":"余量 / 限额"`。

**但配额查询无 API-Key 接口**：
- `token.sensenova.cn` 网关**只暴露 4 个 LLM 端点**（`/v1/{models,chat/completions,messages,images/generations}`），全部 `/v1/{usage,quota,balance,user/*,token-plan/*,coding-plan/*,dashboard/*,api/*}` 路径 GET/POST 探测全 **404**（见 §2 表格）。
- 控制台 quota 面板的数据来自 **`nova-auth-sdk` → SenseCore 账号体系**（`signin.sensecore.tech` / `console.sensecore.tech`），是浏览器会话鉴权（OAuth2 session），**不是 API-Key Bearer**。第三方工具拿 API-Key 查不到。
- 结论：与 [[xiaomi-mimo-token-plan-no-api]] 同型 —— Token Plan 平台但**无 API-Key 配额查询接口**，quota 只能进控制台肉眼看。

对照现有 quota.rs 平台：

| 现有平台 | 配额接口路径 | sensenova 对照 |
|---|---|---|
| DeepSeek | `GET /user/balance` (Bearer) | ❌ 无对应 |
| StepFun | `GET /v1/accounts` (Bearer) | ❌ 无对应 |
| SiliconFlow | `GET /v1/user/info` (Bearer) | ❌ 无对应 |
| OpenRouter | `GET /api/v1/credits` (Bearer) | ❌ 无对应 |
| Novita | `GET /v3/user/balance` (Bearer) | ❌ 无对应 |
| Kimi | `GET /coding/v1/usages` (Bearer) | ❌ 无对应（虽同为 5h 窗口 coding-plan 形态） |
| GLM | `GET /api/monitor/usage/quota/limit` (raw key) | ❌ 无对应 |
| MiniMax | `GET /v1/api/openplatform/coding_plan/remains` (Bearer) | ❌ 无对应 |
| **Xiaomi MiMo** | 无 | ✅ **sensenova 同型** |

### 6. 本项目接入映射（5 维）

#### 6.1 Protocol 归属

**复用 `OpenAI` + `Anthropic` 通用端点协议**，**不需新增 Protocol 变体**。

理由（对照 [[protocol-same-proto-passthrough]]）：sensenova 是纯标准 OpenAI/Anthropic 兼容平台（不像 glm/kimi/minimax/bailian 有原生协议变体或 coding-plan 专属 host）。两条路：

- **方案 A（推荐，最省）**：用通用 `OpenAI` + `Anthropic` Protocol，平台 preset 不加新 Protocol 枚举。用户添加平台时选 openai/anthropic 端点，填 `https://token.sensenova.cn/v1`（openai）或 `https://token.sensenova.cn`（anthropic）。这是 stepfun 的现行做法（看 Platforms.tsx:40 stepfun preset 用通用 anthropic 协议，无独立 Protocol）。
- **方案 B（如要 preset 自动填充）**：参考 stepfun 模式加一个 preset 行 `{ value: "sensenova", label: "商汤日日新（SenseNova）", keywords: ["sensenova","商汤","日日新","sense nova"] }`，但**不进 Protocol 枚举**（platformPaste matchPlatform 走 keyword/host，Protocol 留通用）。adapter 完全复用 `adapter/openai_completions.rs` + `adapter/anthropic.rs`。

#### 6.2 Adapter 复用

**无需写新 adapter**。
- OpenAI 侧：`src-tauri/src/gateway/adapter/openai_completions.rs`（已有，3.1K）。
- Anthropic 侧：`src-tauri/src/gateway/adapter/anthropic.rs`（已有，5.4K）。
- 走 [[protocol-same-proto-passthrough]]：入站 anthropic + endpoint anthropic → 直发；入站 openai + endpoint openai → 直发；跨协议转换走现有 converter。

#### 6.3 Quota（quota.rs）

**不需新 case**。`src-tauri/src/gateway/quota/mod.rs` 的 `query_quota_inner` base_url 子串匹配列表**不要加 sensenova**，让其 fallthrough 到 `err_quota("Unsupported platform for quota query")`。

与 [[xiaomi-mimo-token-plan-no-api]] 一致处理：
- 平台卡片余额栏无值（与小米 MiMo 同）。
- 用户只能进控制台肉眼看 5h 窗口余量。
- 推测:可考虑在平台卡片显示"5h 窗口 1500/500 次"静态文案作为替代（但这是产品决策，非本次调研范围）。

#### 6.4 智能粘贴（platformPaste.ts + Platforms.tsx preset）

加 preset 行（Platforms.tsx PLATFORM_PRESETS），无需改 platformPaste.ts 逻辑：

```ts
{ value: "sensenova", label: "商汤日日新（SenseNova）", keywords: ["sensenova", "商汤", "日日新", "sense nova"] },
```

`getDefaultEndpoints`（Platforms.tsx）加：
```ts
sensenova: [
  { protocol: "openai", base_url: "https://token.sensenova.cn/v1", client_type: "codex_tui" },
  { protocol: "anthropic", base_url: "https://token.sensenova.cn", client_type: "claude_code" },
],
```

注意 hosts 子串派生（platformPaste matchPlatform 优先级 1）：`token.sensenova.cn` 是 sensenova 独有 host，无冲突，会被自动派生进 preset.hosts（参考 Platforms.tsx 现有 hosts 派生逻辑），无需手填。**codingPlan 不标**（无独立 coding host，与 minimax 同 host 模式不同；quota 也无接口，标 codingPlan 也不能查）。

`codingKeyPrefixes` 不填 —— key 是 `sk-` 前缀，无法与普通 key 区分（且本来就没有普通/coding 双形态）。

#### 6.5 定价（price_sync / model_price）

- **LiteLLM 无 sensenova 条目**（已确认 `model_prices_and_context_window.json` sensenova 命中 0）。
- 本项目 `data/models.json` 也无 `sensenova-6.7-flash-lite` / `sensenova-u1-fast`。
- **`deepseek-v4-flash` 在两个信源都有**：LiteLLM 裸名 `deepseek-v4-flash` → input $0.14 / output $0.28 per M tokens（$1.4e-7 / $2.8e-7 per token），本地 `data/models.json` 也有 `deepseek-v4-flash`。**模型名同名直接复用现有定价**。
- 公测期 Token Plan **免费**（"公测期间免费畅享"），所以 sensenova 自有 2 模型实际不计费 → 定价缺失**不影响 est_cost**（公测免费 → 0 成本）。可手工在 model_price 建占位条目或留空走 fallback。

### 7. 协议兼容性细节备忘（实现时注意）

1. **anthropic 端点 base_url 不含 `/v1`**（SDK 自动加 `/v1/messages`）—— 与 glm(`open.bigmodel.cn/api/anthropic`)/kimi(`api.moonshot.cn/anthropic`) 模式**不同**：glm/kimi 是 host+`/anthropic` path，sensenova 是**裸 host**。这是 preset 配置时最易踩的坑（配错会出现 `/v1/v1/messages` 404）。
2. **错误信封多了 `code` 字段**：`{"error":{"type":"...","code":"3","message":"..."}}`。本项目 proxy 解析上游错误若按 OpenAI 标准 `{"error":{"message":...}}` 取 message 不受影响（type/message 字段都在），但若按 `code` 字段做分支需感知 sensenova code 是数值字符串（"16"=forbidden, "5"=not_found）。
3. **流式 SSE 格式**：`data: {...}\n\ndata: [DONE]`，与 OpenAI 一致，现有 `parse_sse` 可直接用。
4. **reqwest 解压**：sensecore.cn 套 WAF + GTM，响应大概率 gzip —— 已由现有 reqwest 自动解压覆盖（[[reqwest-decompress-independent-of-request-header]]）。
5. **anthropic-beta header**：sensenova 是第三方 anthropic 端点，[[anthropic-beta-host-gated-strip]] + [[third-party-anthropic-thinking-strip]] 的 host-gated strip 逻辑需确认 sensenova host 是否进白名单（推测:需实测，可能也要 strip thinking 字段）。

---

## Caveats / Not Found

1. **`reasoning_effort` 取值全集不确定**：request-params 表写 `low/medium/high/none`，Claude Code FAQ 写 `low/medium/high/max`。`none` 与 `max` 哪个真存在需用户用真 key 实测。安全默认 `high`。
2. **API Key 实际前缀未确认**：docs 示例统一写 `sk-xxx`（占位），但占位与真 key 前缀不一定一致。需用户申请到真 key 后看实际前缀（推测:大概率 `sk-`，平台Paste KEY_PREFIXES 已含）。
3. **Quota 接口彻底无**：仅基于 endpoint 主动探测（全 404）+ docs 无配额 API 文档 + 控制台走 OAuth 会话。**不排除**控制台 quota 数据有一个隐藏的需 OAuth-token（非 API-Key）的内部接口，但对第三方工具无意义（第三方只有 API-Key）。结论与 [[xiaomi-mimo-token-plan-no-api]] 一致成立。
4. **docs 静态内联只覆盖 quickstart + Claude Code + Cursor/Cline 接入 + models overview + Token Plan dashboard i18n**；完整 docs（pricing/billing/限流细节/全部 model 详情页）是动态加载，本次未取全。但 5 维接入映射所需信息已齐。
5. **未实测真实请求**：本次为只读调研（无真 key），端点存在性靠 401（真）/404（假）区分；请求体/响应体字段全来自 docs 原文，未经真实往返验证。

## `需要:`

无阻塞项。以下为可选（用户有真 key 后可补）：
- `需要:`（可选）用真 key 实测 `reasoning_effort` 全集 + anthropic-beta header strip 行为 + 真 key 前缀，补到 spec。
