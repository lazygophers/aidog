# Research: OpenAI Codex CLI 官方模型清单 + 端点/认证方式

- **Query**: 查 Codex CLI（@openai/codex）全部官方支持的模型 + 端点/认证方式，判现 aidog preset 遗漏
- **Scope**: external（官方文档）+ internal（preset 对比）
- **Date**: 2026-07-09
- **官方真值源**:
  - 模型页: https://developers.openai.com/codex/models
  - 认证页: https://developers.openai.com/codex/auth
  - 配置参考: https://developers.openai.com/codex/config-reference
  - 高级配置: https://developers.openai.com/codex/config-advanced
  - GitHub 镜像（config.md 仅做导航跳转）: https://github.com/openai/codex/blob/main/docs/config.md

## Findings

### 1. 全模型清单（官方推荐 + 弃用）

来源: https://developers.openai.com/codex/models

| model id | 用途 | 推荐等级 | 可用渠道 | 备注 |
|---|---|---|---|---|
| `gpt-5.5` | 最新前沿模型，复杂编码 / computer use / 研究 | **首选默认**（"For most tasks in Codex, start with gpt-5.5"） | CLI & SDK / App & IDE / Cloud / ChatGPT Credits / API Access | 文档明示 `model = "gpt-5.5"` 示例 |
| `gpt-5.4` | 旗舰前沿模型，强编码 / 推理 / 工具调用 | 推荐 | 同上 | |
| `gpt-5.4-mini` | 快速高效 mini 模型，轻量编码 / subagent | 推荐（更低成本） | 同上 | |
| `gpt-5.3-codex-spark` | 文本-only research preview，近实时迭代 | **研究预览**，仅 ChatGPT Pro | CLI & SDK / App & IDE / Cloud（无 API Access 列） | 优化"near-instant, real-time coding iteration" |
| ~~`gpt-5.2`~~ | 旧版 | **已弃用**（ChatGPT 登录场景） | 可能仍走 API key（见 API models 页） | 文档要求更新脚本到上述最新模型 |
| ~~`gpt-5.3-codex`~~ | 旧 codex 变体 | **已弃用**（ChatGPT 登录场景） | 可能仍走 API key | 同上 |

文档原文关键句：
> "You can also point Codex at any model and provider that supports either the Chat Completions or Responses APIs to fit your specific use case. Support for the Chat Completions API is deprecated and will be removed in future releases of Codex."

→ Codex CLI 默认走 **Responses API**（`/v1/responses`）；Chat Completions 兼容但已标记 deprecation。

**关于"codex 专用变体"（任务研究项 3）**:
- 历史确有 `gpt-5.3-codex`（已弃用）和现存的 `gpt-5.3-codex-spark`（research preview，仅 ChatGPT Pro，无 API 访问列）。
- **未在文档中发现** `gpt-5-codex` / `codex-mini-latest` 这类 id（推测: 这两个 id 不存在，属社区或早期误传；文档无引用）。
- 文档明示推荐的 codex 命名变体只有 `gpt-5.3-codex-spark` 一个。

### 2. 端点 / 认证方式（OpenAI 模型场景）

来源: https://developers.openai.com/codex/auth + https://developers.openai.com/codex/config-advanced

| 认证方式 | base_url / 端点 | wire_api / protocol | 认证头 | 备注 |
|---|---|---|---|---|
| **Sign in with ChatGPT**（订阅） | `chatgpt_base_url`（默认 chatgpt.com 后端）+ openai provider 走 `https://api.openai.com/v1` | Responses | OAuth 流程拿到的 access token（Bearer） | CLI 默认路径（无有效 session 时默认走这条）；Codex Cloud 强制要求；支持企业 RBAC / 保留策略 |
| **API key**（按量付费） | `https://api.openai.com/v1`（内置 openai provider） | Responses | `Authorization: Bearer $OPENAI_API_KEY`（`env_key = "OPENAI_API_KEY"`） | 标准平台费率；推荐用于 CI/CD 等程序化场景；部分依赖 ChatGPT workspace 的功能不可用 |
| **Codex access token**（企业自动化） | 同 ChatGPT 后端 | Responses | Bearer access token | ChatGPT Enterprise 管理员发放；用于可信非交互脚本；`printenv CODEX_ACCESS_TOKEN \| codex login --with-access-token` |
| **数据驻留（US/EU Projects）** | `https://us.api.openai.com/v1`（或对应前缀） | Responses | 同上 | 通过 `openai_base_url` 或自定义 `model_providers.openaidr` |
| **第三方 / 自定义 provider**（Anthropic / Google / Mistral / Azure / Ollama 等） | 任意 `base_url` | `responses` 或 `chat`（`wire_api` 配置） | `env_key` / `http_headers` / `experimental_bearer_token` / 命令式 `auth.command` | 见下表 |

#### 内置 provider id（不可覆盖）
来源: config-advanced `model_providers` 段

| provider id | base_url 默认 | 备注 |
|---|---|---|
| `openai` | `https://api.openai.com/v1` | **禁** 用 `[model_providers.openai]` 覆盖；改用 `openai_base_url` 顶层键 |
| `ollama` | `http://localhost:11434/v1` | 配合 `--oss` |
| `lmstudio` | 本地 | 配合 `--oss` |
| `amazon-bedrock` | AWS Bedrock | 内置，仅支持 `[model_providers.amazon-bedrock.aws]` profile/region 覆盖 |

#### 自定义 provider 示例（文档原文摘录）
```toml
model = "gpt-5.4"
model_provider = "proxy"

[model_providers.proxy]
name = "OpenAI using LLM proxy"
base_url = "http://proxy.example.com"
env_key = "OPENAI_API_KEY"

[model_providers.local_ollama]
name = "Ollama"
base_url = "http://localhost:11434/v1"

[model_providers.mistral]
name = "Mistral"
base_url = "https://api.mistral.ai/v1"
env_key = "MISTRAL_API_KEY"
```

→ **文档并未**列举 Anthropic / Google / Groq 的"官方推荐 base_url"，这些只是举例 Mistral + Ollama；任何 Responses/Chat 兼容端点都可挂。Azure OpenAI 有专门示例（`base_url = https://YOUR_PROJECT_NAME.openai.azure.com/openai`，`query_params.api-version`，`wire_api = "responses"`）。

### 3. Responses API vs Chat Completions

- Codex CLI **默认走 Responses API**（`/v1/responses`），内置 `openai` provider 隐式 `wire_api = "responses"`。
- 第三方 provider 通过 `wire_api = "responses"` 或 `wire_api = "chat"` 二选一。
- **Chat Completions 已标记 deprecation**，未来版本移除（文档原话见第 1 节引用）。
- aidog preset 中 codex 协议 `protocol = "openai_responses"` ✓ 与官方默认一致。

## 现 preset vs 官方清单对比

### 模型清单对比

来源: `src-tauri/defaults/platform-presets.json:111-160`

| 模型 | 官方状态 | preset 有? | 建议 |
|---|---|---|---|
| `gpt-5.5` | 首选默认 | ✅（且 `models.default.gpt = "gpt-5.5"`） | 保留 |
| `gpt-5.4` | 推荐 | ✅ | 保留 |
| `gpt-5.4-mini` | 推荐（mini） | ✅ | 保留 |
| `gpt-5.3-codex-spark` | research preview（仅 ChatGPT Pro，无 API） | ❌ **遗漏** | **建议补**（用户若 ChatGPT Pro 订阅可用；非 API key 渠道） |
| `gpt-5.2` | 已弃用（ChatGPT）/ 可能仍 API 可用 | ❌ | 不补 |
| `gpt-5.3-codex` | 已弃用 | ❌ | 不补 |

### 端点对比

| 端点 / 认证 | 官方支持 | preset 有? | 建议 |
|---|---|---|---|
| OpenAI Responses API（`api.openai.com/v1`）+ codex_tui | ✓ 主路径 | ✅（`endpoints.default[0]`） | 保留 |
| ChatGPT 订阅认证（chatgpt.com） | ✓ CLI 默认 | ❌ | **不建议补**：aidog 是代理路由器，ChatGPT OAuth 流程依赖 CLI 自身完成；aidog 端只需在 OpenAI Responses 端点上代理即可。OAuth token 由用户 codex CLI 持有，与 aidog 代理无关。 |
| 数据驻留 US 端点（`us.api.openai.com/v1`） | ✓ | ❌ | **不建议补**：企业用户少数需求，可用 platform.extra 自行覆盖 |
| 第三方 provider（Mistral / Azure / Bedrock / Ollama） | ✓ 通过 `model_providers` | ❌ | **不建议在 codex preset 补**：这些是用户自定义场景，已有独立协议项（mistral / azure / ollama 等）或应通过 `platform.extra` 自定义。Codex CLI 自身的多 provider 路由是 CLI 内部行为，aidog 代理层无需复刻。 |

### `models.default.gpt` vs `model_list.default`
- preset `models.default.gpt = "gpt-5.5"` ✓ 与文档示例 `model = "gpt-5.5"` 一致。
- `model_list.default` 仅做下拉展示用，非路由键。

## 推荐

### 推荐 `model_list.default` 最终清单

```json
["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex-spark"]
```

**理由**:
1. 前三项保留——官方明示推荐三件套，覆盖默认/旗舰/mini 三档。
2. **补 `gpt-5.3-codex-spark`**：官方在「Recommended models」段第四位列出，属真实可用模型（非弃用、非「Other」段）。虽仅 ChatGPT Pro 可用，但用户若以 ChatGPT 订阅 + API key 双通道，仍可能切到该模型；aidog 作为透明代理不阻拦。
3. 不补 `gpt-5.2` / `gpt-5.3-codex`：官方明示弃用，列入会误导用户。
4. 不补任意「Other models」段项：该段指用户可挂任意 Responses/Chat 兼容模型，但那些是用户自定义场景，无固定 id 清单。

### 推荐 `endpoints.default` 最终清单

**保持现状**（单 endpoint，不增 ChatGPT/数据驻留/第三方）：

```json
[{"protocol": "openai_responses", "base_url": "https://api.openai.com/v1", "client_type": "codex_tui"}]
```

**理由**:
1. aidog preset 的角色是"默认配置 + 用户可覆盖起点"，非 Codex CLI 全功能镜像。
2. ChatGPT OAuth 流程由 codex CLI 客户端自身驱动，不经过 HTTP 代理层；aidog 代理的 `api.openai.com/v1` 端点对 ChatGPT/API key 两种认证透明（Bearer 头原样透传），故单一 endpoint 足够。
3. 数据驻留 / 第三方 provider 属少数企业 / 高级用户场景，CLAUDE.md 已明示「`platform.extra` 可手工启用」——这些不应污染默认 preset。
4. 如未来需暴露更多认证路径，建议走 `platform.extra` UI 编辑表单（已有机制），而非扩 preset default endpoints。

## Caveats / Not Found

- **`gpt-5.3-codex-spark` 渠道确认**: 官方模型页能力矩阵未给该模型「API Access」勾选（仅 CLI/SDK/App/IDE/Cloud/ChatGPT Credits），意味着 API key 认证下可能无法调用。若 aidog 用户的 codex CLI 走 ChatGPT 订阅认证，则可用；走 API key 则不可用。补入 `model_list.default` 仅影响下拉展示，不影响路由行为。
- **`openai_responses` protocol 含义**: aidog 内部协议名，对应 Codex `wire_api = "responses"`，走 `/v1/responses` 路径（非 `/v1/chat/completions`）。preset 该字段正确，无需改。
- **未读取 codex-rs 源码**: 任务未要求查 Rust 源码 `model_providers` 模块；如需精确内置 provider 清单（含 OpenAI 内部硬编码的 base_url），可进一步读 `openai/codex` 仓库 `codex-rs/` 源码——但官方文档已列 4 个内置 id（openai/ollama/lmstudio/amazon-bedrock），足够。
- **需要**: 若 main 担心 `gpt-5.3-codex-spark` 在 API key 渠道被路由后产生 4xx，建议在 PlatformCard UI 加 hint 而非从 preset 删除——但这是 UX 决策，归 main。
