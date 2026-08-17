# pi（earendil-works/pi）接入调研：endpoint / token / 协议 / 模型来源

调研目的：为 aidog 增加 pi 作为受支持客户端，需要精确到「写哪个文件、注哪个 env、上游收到什么请求」。

## 一手来源

全部读自 `earendil-works/pi`，git tag `v0.84.2`，SHA `914cf1472e715297caa30db4b9535d534a9eb718`（`git clone --depth 1 --branch v0.84.2`）。

| 来源 | 用途 |
|------|------|
| `README.md` | 包结构、license、构建方式 |
| `packages/coding-agent/docs/quickstart.md` | 安装命令、认证入口、配置目录 |
| `packages/coding-agent/docs/providers.md` | provider 清单、API key env 表、auth.json、凭证解析顺序 |
| `packages/coding-agent/docs/models.md` | `models.json` 完整 schema、内置 provider 覆盖、compat 开关 |
| `packages/coding-agent/docs/environment-variables.md` | pi 自身读取的 env 变量表 |
| `packages/coding-agent/docs/custom-provider.md` | extension 方式注册 provider |
| `packages/ai/src/providers/anthropic.ts` / `openai.ts` / `zai.ts` | 内置 provider 的 `baseUrl` 与 auth 解析实现 |
| `packages/ai/src/env-api-keys.ts` | env 变量 ↔ provider id 真值源 |
| `packages/ai/src/api/anthropic-messages.ts` | Anthropic 侧 HTTP client 构造与 header |
| `packages/coding-agent/src/config.ts` | `models.json` 路径解析 |
| `packages/coding-agent/src/core/remote-catalog-provider.ts` | 远端模型目录拉取 |
| `packages/coding-agent/src/core/provider-attribution.ts` | 归因 header |
| `packages/coding-agent/src/cli/args.ts` | CLI 参数 |
| 官网文档 | https://pi.dev/docs/latest（未作为本文引用依据，结论均取自仓库内文件） |

---

## 1. pi 是什么

TypeScript monorepo（`README.md:1-40`），MIT license（`README.md:112`）。npm 全局安装：

```bash
npm install -g --ignore-scripts @earendil-works/pi-coding-agent
```

（`packages/coding-agent/docs/quickstart.md:9-11`）

三个核心包：`@earendil-works/pi-coding-agent`（交互式 CLI，命令名 `pi`）、`@earendil-works/pi-agent-core`（agent runtime）、`@earendil-works/pi-ai`（多 provider 统一 LLM API，`README.md:15-19`）。版本节奏很快，仓库有 50 个 tag，当前最新 `v0.84.2`。也提供 Bun 打包的独立二进制（`README.md:71-84`）。

配置目录：`~/.pi/agent/`（`quickstart.md:33`），可用 `PI_CODING_AGENT_DIR` 覆盖（`environment-variables.md:81`）。

---

## 2. 自定义 API endpoint —— 关键结论

**pi 没有 `ANTHROPIC_BASE_URL` / `OPENAI_BASE_URL` 这类通用 base URL env 变量。** 全仓库 grep `BASE_URL` 只命中：`AZURE_OPENAI_BASE_URL`（`packages/ai/src/api/azure-openai-responses.ts:226`）、`LLAMA_BASE_URL`（`packages/coding-agent/src/extensions/llama/provider.ts:16`）、`AWS_ENDPOINT_URL_BEDROCK_RUNTIME`（`providers.md:241`），以及各 provider 内部硬编码常量。CLI 也**没有** `--base-url` 参数——`args.ts` 只有 `--provider`、`--api-key`、`--model`（`packages/coding-agent/src/cli/args.ts:90-94, 264-266`）。

改 endpoint 的唯一通用路径是配置文件 **`~/.pi/agent/models.json`**：

```ts
// packages/coding-agent/src/config.ts:528-530
/** Get path to models.json */
return join(getAgentDir(), "models.json");
```

只有这一个全局路径（`agentDir/models.json`，见 `core/agent-session-services.ts:144`、`core/model-runtime.ts:175`、`core/sdk.ts:177`）；**没有项目级 `./.pi/models.json`**。`PI_CODING_AGENT_DIR` 会连带移动它。

### 2a. 覆盖内置 provider 的 baseUrl（推荐给代理场景）

```json
{
  "providers": {
    "anthropic": {
      "baseUrl": "https://my-proxy.example.com"
    }
  }
}
```

「All built-in Anthropic models remain available. Existing OAuth or API key auth continues to work.」（`models.md:300-314`）只给 `baseUrl` / `headers` 而不给 `models`，内置模型清单与认证全部保留（`custom-provider.md:119`）。

### 2b. 新增自定义 provider

```json
{
  "providers": {
    "my-gateway": {
      "baseUrl": "https://proxy.example.com/v1",
      "api": "openai-completions",
      "apiKey": "$MY_KEY",
      "authHeader": true,
      "headers": { "x-foo": "$BAR" },
      "models": [{ "id": "some-model" }]
    }
  }
}
```

provider 字段表（`models.md:132-145`）：`baseUrl` / `api` / `apiKey` / `oauth` / `headers` / `authHeader`（置 `true` 自动加 `Authorization: Bearer <apiKey>`）/ `models` / `modelOverrides` / `compat`。非内置 provider 带 `models` 时必须给 `baseUrl` 且 provider 或 model 级必须有 `api`。

文件热重载：「The file reloads each time you open `/model`. Edit during session; no restart needed.」（`models.md:92`）—— aidog 改写 `models.json` 后用户无需重启 pi。

### 2c. 值解析（apiKey / headers 通用）

`"!cmd"` 执行 shell 取 stdout；`"$VAR"` / `"${VAR}"` 取 env；`"$$"`/`"$!"` 转义；其余按字面量（`models.md:147-176`）。**`models.json` 里的 shell 命令在每次请求时求值，pi 不做 TTL 或缓存**（`models.md:172-174`）。

### 2d. 优先级

凭证解析顺序（`providers.md:310-317`）：

1. CLI `--api-key`
2. `~/.pi/agent/auth.json`
3. 环境变量
4. `models.json` 的 provider key

注意方向：**`auth.json` 优先于 env**（`providers.md:139`）。endpoint 维度没有等价链条——`baseUrl` 只有 `models.json`（及 extension）一处，会覆盖内置常量。

---

## 3. Token / 认证

env 变量 ↔ provider id 真值源是 `packages/ai/src/env-api-keys.ts` 的 `getApiKeyEnvVars`（`providers.md:107` 明确指向该文件）。与 aidog 相关的几个：

| provider id | env 变量 | auth.json key |
|---|---|---|
| `anthropic` | `ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_OAUTH_TOKEN`、`ANTHROPIC_API_KEY` | `anthropic` |
| `openai` | `OPENAI_API_KEY` | `openai` |
| `zai` | `ZAI_API_KEY` | `zai` |
| `kimi-coding` | `KIMI_API_KEY` | `kimi-coding` |
| `minimax` / `minimax-cn` | `MINIMAX_API_KEY` / `MINIMAX_CN_API_KEY` | 同名 |
| `xiaomi` 等 | `XIAOMI_API_KEY` … | 同名 |

（完整 40 项见 `providers.md:69-105`；代码见 `env-api-keys.ts:79-116`）

**`ANTHROPIC_AUTH_TOKEN` 走 Bearer，且优先级最高**：

```ts
// packages/ai/src/providers/anthropic.ts:24-31
const authToken = await ctx.env(ANTHROPIC_AUTH_TOKEN_ENV);
if (authToken) {
  return {
    auth: { headers: { Authorization: `Bearer ${authToken}` } },
    source: ANTHROPIC_AUTH_TOKEN_ENV,
  };
}
```

顺序是：`auth.json` 存的 `credential.key` → `ANTHROPIC_AUTH_TOKEN`（Bearer header）→ `ANTHROPIC_OAUTH_TOKEN` → `ANTHROPIC_API_KEY`（`anthropic.ts:19-38`）。`env-api-keys.ts:73-77` 注释写明「`getEnvApiKey()` skips it because requests must pass it as `Authorization: Bearer`」。

线上实际发出的 header（`packages/ai/src/api/anthropic-messages.ts:862-955`），三条分支：

- `github-copilot`：`authToken`（Bearer）
- token 判定为 OAuth（`isOAuthToken`）：`authToken` Bearer + `anthropic-beta: claude-code-20250219,oauth-2025-04-20` + `user-agent: claude-cli/<version>` + `x-app: cli`
- 其余（API key 或 header-owned auth）：`new Anthropic({ apiKey, authToken: null, baseURL: model.baseUrl, ... })` → 由官方 SDK 发 `x-api-key`

三条分支都固定带 `accept: application/json` 与 `anthropic-dangerous-direct-browser-access: true`；`anthropic-version` 由 `@anthropic-ai/sdk` 自动附加。

`auth.json`（`0600`，`providers.md:139`）格式：

```json
{ "anthropic": { "type": "api_key", "key": "sk-ant-..." } }
```

`auth.json` 条目还可带 provider 作用域的 `env` 对象，其值**先于**进程 env 用于解析 key、header 和 provider 配置（`providers.md:141-157`）。

订阅式登录（`/login`）支持 Claude Pro/Max、ChatGPT Codex、GitHub Copilot、xAI、OpenRouter、Radius，token 存 `auth.json` 并自动刷新（`providers.md:15-26`）。

---

## 4. 线路协议

pi 是多协议客户端，按 provider/model 的 `api` 字段决定（`models.md:121-130`）：

| `api` 值 | 协议 |
|---|---|
| `openai-completions` | OpenAI Chat Completions（最兼容） |
| `openai-responses` | OpenAI Responses API |
| `anthropic-messages` | Anthropic Messages API |
| `google-generative-ai` | Google Generative AI |

实现均为「官方 SDK + `baseURL: model.baseUrl`」，路径由 SDK 拼接：

```ts
// packages/ai/src/api/anthropic-messages.ts:945-952
const client = new Anthropic({
  apiKey: apiKey ?? null,
  authToken: null,
  baseURL: model.baseUrl,
  ...
});
```

### baseUrl 是否含版本前缀 —— 按协议不同，别搞混

- `anthropic-messages`：内置 `anthropic` provider 的 `baseUrl` 是 **`https://api.anthropic.com`**（无 `/v1`，`packages/ai/src/providers/anthropic.ts:47`），`@anthropic-ai/sdk` 自己补 `/v1/messages`。所以给 pi 的 Anthropic 代理地址应当是**根地址，不带 `/v1`**。
  ⚠️ `models.md:300-329` 的示例写成 `"baseUrl": "https://my-proxy.example.com/v1"`，与内置常量不一致；按 SDK 行为那会打到 `/v1/v1/messages`。以代码为准。
- `openai-completions` / `openai-responses`：内置 `openai` provider 的 `baseUrl` 是 **`https://api.openai.com/v1`**（`packages/ai/src/providers/openai.ts:11`），SDK 再补 `/chat/completions` 或 `/responses`。即 **`/v1` 要写进 baseUrl**——与 aidog 现有约定（`base_url` 含版本前缀，`provider_api_path()` 只返回 `/chat/completions`）一致。
- 参照 `zai` provider：`baseUrl: "https://api.z.ai/api/coding/paas/v4"` + `api: openai-completions`（`packages/ai/src/providers/zai.ts:9-11`），与 aidog 的 `glm_coding` 预设同形。

流式：Anthropic 侧调用 `MessageCreateParamsStreaming`（`anthropic-messages.ts:957-962`，`stream` 为导出的主入口 `:502`），即**默认 SSE 流式**。

### 代理必须容忍的请求特征

- 每工具默认带 `tools[].eager_input_streaming: true`；后端不认时需 `compat.supportsEagerToolInputStreaming: false`，pi 改发 `fine-grained-tool-streaming-2025-05-14` beta header（`models.md:391`）
- `cache_control` prompt caching 标记；`PI_CACHE_RETENTION=long` 时用 `cache_control.ttl: "1h"`（`environment-variables.md:87`、`models.md:427`）。测试注明「should add ttl for non-`api.anthropic.com` baseUrl by default」（`packages/ai/test/cache-retention.test.ts:99`）——即代理地址下默认就带 ttl，可用 `compat.supportsLongCacheRetention: false` 关掉
- 缓存开启时可能发 `x-session-affinity: <sessionId>`（`anthropic-messages.ts:932-933`；OpenAI 侧 `sessionAffinityFormat` 见 `models.md:474`）
- `adaptive` thinking（`thinking.type: "adaptive"` + `output_config.effort`）、strict JSON-schema tools，均有 compat 开关（`models.md:424-432`）
- 归因 header：仅对 OpenRouter / NVIDIA NIM / Cloudflare 等特定 host 追加（`core/provider-attribution.ts:34-60`），且受 `PI_TELEMETRY` 控制（`environment-variables.md:86`）
- 代理相关：pi 读 `HTTP_PROXY` / `HTTPS_PROXY`（`environment-variables.md:92`）

---

## 5. Provider 抽象

pi 天然是「多 provider 并列」模型，`packages/ai/src/providers/` 下 40+ 个内置 provider（`anthropic.ts`、`openai.ts`、`zai.ts`、`kimi-coding.ts`、`minimax.ts`、`xiaomi*.ts` …），每个由 `createProvider({ id, name, baseUrl, auth, models, api })` 定义（`providers/anthropic.ts:41-56`）。

用户层三种扩展方式：

1. `models.json` 的 `providers` 映射（增新 provider 或覆盖内置）——见第 2 节
2. `modelOverrides`：只改内置模型的元数据（`name`/`cost`/`contextWindow`/`headers`/`compat`…），不动模型清单（`models.md:337-360`）
3. extension（需要自定义 API 实现或 OAuth 流时）——`custom-provider.md`

选择运行时 provider/model：`pi --provider anthropic --model <id>`，或 `--model provider/model` 前缀形式（`args.ts:339-341`），交互内 `/model`、`Ctrl+L`。当前值可从 bash 工具 env 读到：`PI_PROVIDER` / `PI_MODEL`（`environment-variables.md:28-29`）。

---

## 6. 模型 id 来源

三层合并：

1. **内置生成目录**：`packages/ai/src/models.generated.ts` + 每 provider 的 `*.models.ts`（如 `anthropic.models.ts`）。构建期刷新：`npm run build` 会「Refresh model data」，`npm run build:offline` 用已有快照（`README.md:60-62`）
2. **pi.dev 远端目录覆盖**：`GET https://pi.dev/api/models/providers/<providerId>`，带 ETag revalidate，刷新间隔 4 小时，缓存到 `~/.pi/agent/models-store.json`（`core/remote-catalog-provider.ts:6-7, 43-80`；`providers.md:3`）
3. **用户 `models.json`**：`models` 数组 upsert（同 `id` 替换内置，新 `id` 追加，`models.md:331-335`）；`modelOverrides` 改元数据

**pi 不向上游 provider 发 `/models` 请求来发现模型。** 也就是说 aidog 代理的 `GET /models` 端点对 pi 无用；模型清单要么来自 pi 内置/pi.dev 目录，要么由 aidog 写进 `models.json`。

模型字段（`models.md:199-212`）：`id`（必填，直接传给 API）、`name`、`api`、`reasoning`、`thinkingLevelMap`、`input`、`contextWindow`（默认 128000）、`maxTokens`（默认 16384）、`samplingParams`、`cost`（每百万 token，支持 `tiers` 阈值分档）、`compat`。

---

## 7. 启动期网络行为与其它

- `PI_OFFLINE`：关闭全部启动期网络操作（更新检查、包更新、安装/更新 telemetry）
- `PI_SKIP_VERSION_CHECK`：只关 pi.dev 最新版本请求
- `PI_TELEMETRY=0`：关 telemetry 与 provider 归因 header
- 进程标记：`AI_AGENT=pi`、`PI_CODING_AGENT=true`，子进程继承（`environment-variables.md:11-18`）——**aidog 若要检测「是否跑在 pi 里」用这两个**
- pi 无内置权限系统，默认以启动用户权限运行（`README.md:44-46`）
- 上下文文件：`~/.pi/agent/AGENTS.md`（全局）+ 逐层 `AGENTS.md` / `CLAUDE.md`，`AGENTS.override.md` 可替换该目录的（`quickstart.md:98-103`）

---

## 8. 对 aidog 的直接含义（研究结论，非设计决定）

1. **不能只靠注 env**。pi 的 endpoint 只认 `~/.pi/agent/models.json`（或 `PI_CODING_AGENT_DIR` 下同名文件）。aidog 必须写这个 JSON —— 类比物是 Claude Code 的 `settings.{group}.json`，但 pi 只有**单一全局文件**，没有 per-project 变体，多分组切换需要重写同一文件或改 `PI_CODING_AGENT_DIR`。
2. **token 可以只靠注 env**：`ANTHROPIC_AUTH_TOKEN=<group_name>` 会被 pi 当 Bearer 直发，与 aidog 现有 Claude Code 注入方式完全一致。但**如果用户 `auth.json` 里存了 `anthropic` 凭证，它会盖掉 env**（`providers.md:139`、`anthropic.ts:19-22`）——这是一个真实冲突点。
3. **最小改动接法**：`models.json` 只覆盖内置 provider 的 `baseUrl`（第 2a 节），内置模型和认证全保留，无需 aidog 维护 pi 的模型清单。代价是只能挂在 pi 已有的 provider id 上。
4. **协议侧无新工作**：pi 说的就是 Anthropic Messages / OpenAI Chat Completions / OpenAI Responses / Google GenAI，aidog 已有的转换层覆盖前两个。
5. **注意 baseUrl 的 `/v1`**：`anthropic-messages` 要根地址（不带 `/v1`），`openai-*` 要带 `/v1`。与 aidog「`base_url` 含版本前缀」的约定在 Anthropic 分支上是反的。
6. **`GET /models` 无用**，但 `/` 健康探测同样不会被 pi 命中——pi 启动只探 pi.dev，不探上游。

## UNKNOWN（一手来源未覆盖）

- pi 的超时与重试策略具体数值：请求侧未定位到统一常量（`utils/management-http.ts` 的 `fetchWithRetry` 只用于管理面/目录拉取，不是 LLM 请求路径）。**UNKNOWN — not found in primary sources**
- `models.json` 是否有官方 JSON Schema 文件可供 aidog 校验：代码里有 schema 校验（`core/model-config.ts:276` 报 `Invalid models.json schema`），但未定位到可导出的 schema 文件。**UNKNOWN — not found in primary sources**
