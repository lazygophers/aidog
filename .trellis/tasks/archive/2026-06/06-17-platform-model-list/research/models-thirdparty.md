# Research: 第三方/中转组候选模型列表

- **Query**: 为 26 个第三方/中转平台定候选模型列表（多为 Claude Code 代理 → 候选 = 当前 Claude 旗舰系列最新 API id；有自有模型的列其自有）
- **Scope**: external（核 Anthropic 官方）+ internal（核 Platforms.tsx 平台预设）
- **Date**: 2026-06-17
- **核查日期**: 2026-06-17（模型名月级腐化，以此日期为准 + fetchModels 兜底）

---

## 一、Claude 当前旗舰 API id（Anthropic 官方权威）

| 系列 | 当前旗舰 API id（alias） | 带日期完整 id | 上一代（仍可用） |
|---|---|---|---|
| Opus | **`claude-opus-4-8`** | `claude-opus-4-8`（无独立日期后缀，alias 即完整 id） | `claude-opus-4-7` / `claude-opus-4-6` / `claude-opus-4-5` |
| Sonnet | **`claude-sonnet-4-6`** | `claude-sonnet-4-6` | `claude-sonnet-4-5`（`claude-sonnet-4-5-20250929`） |
| Haiku | **`claude-haiku-4-5`** | `claude-haiku-4-5-20251001` | （无 4.6；4.5 即当前最新） |

**来源**:
1. Anthropic 官方 models overview — `https://platform.claude.com/docs/en/about-claude/models/overview.md`（2026-06-17 WebFetch 实测）。原文表格行:
   - `| **Claude API alias** | claude-opus-4-8 | claude-sonnet-4-6 | claude-haiku-4-5 |`
   - `| **Claude API ID** | ... | ... | claude-haiku-4-5-20251001 |`
2. claude-api skill `shared/models.md`（Current Models 表）：Opus 4.8 / Sonnet 4.6 / Haiku 4.5 均为 Active；**无 Haiku 4.6 条目**。
3. 本仓 `data/models.json`（LiteLLM 同步，generated_at 2026-06-17）：含 `claude-opus-4.8` / `claude-sonnet-4.6` / `claude-haiku-4.5`；`claude-haiku-4.6` **不存在**。

> ⚠️ **纠错**: 现有 `src/pages/Platforms.tsx:381` anthropic 预设写 `haiku: "claude-haiku-4-6"` —— **该值错误**，Anthropic 无 Haiku 4.6，当前最新 Haiku 仍是 `claude-haiku-4-5`。本组候选 haiku 槽应取 `claude-haiku-4-5`。

> **id 格式约定**: API id 用连字符（`claude-opus-4-8`），不是点号（点号 `claude-opus-4.8` 仅 LiteLLM 定价表 key 用法）。下游 Claude Code 透传平台必须用连字符形式。

### 本组「Claude 旗舰候选列表」（默认值，下文各平台引用此）

```
claude-opus-4-8        # opus 槽 / default 首选
claude-sonnet-4-6      # sonnet 槽
claude-haiku-4-5       # haiku 槽
```
可选补充上一代（部分中转仍挂旧版，给用户下拉选）：`claude-opus-4-7`、`claude-opus-4-6`、`claude-opus-4-5`、`claude-sonnet-4-5`。

---

## 二、各平台候选（26 个）

> **判定依据**: 平台预设端点见 `src/pages/Platforms.tsx:275-362`（getDefaultEndpoints）。
> - 绝大多数为 `protocol: "anthropic"` + `client_type: "claude_code"` 的纯 Claude Code 中转 → 候选 = 上面「Claude 旗舰候选列表」。
> - 极少数有自有/多协议模型，单独标注。
> - 无公开自有模型清单来源的，统一标「推测:」+ 以 fetchModels 为主源。

### 默认组：纯 Claude Code 中转（候选 = Claude 旗舰列表）

下列平台预设均为单一 anthropic 端点 + claude_code，定位「Claude Code 代理/中转」，候选列表统一为 `claude-opus-4-8` / `claude-sonnet-4-6` / `claude-haiku-4-5`（+ 可选旧版）。来源 = 平台 base_url 端点协议（Platforms.tsx）+ 平台命名/定位（透传 Claude Code）；具体上游实际暴露 model id **以 fetchModels 为主源**。

| 平台 key | base_url（anthropic 端点） | 候选来源 | 说明 |
|---|---|---|---|
| `pateway` | `https://api.pateway.ai` | Claude 旗舰列表 | 纯 anthropic/claude_code 中转 |
| `ccsub` | `https://www.ccsub.net` | Claude 旗舰列表 | 名即 "CC Sub"=Claude Code 订阅中转 |
| `apikeyfun` | `https://api.apikey.fun` | Claude 旗舰列表 | 纯 anthropic/claude_code |
| `apinebula` | `https://apinebula.com` | Claude 旗舰列表 | 纯 anthropic/claude_code |
| `sudocode` | `https://sudocode.us` | Claude 旗舰列表 | 名即 Claude Code 编程中转 |
| `claudeapi` | `https://gw.claudeapi.com` | Claude 旗舰列表 | 名即 ClaudeAPI |
| `claudecn` | `https://claudecn.top` | Claude 旗舰列表 | 名即 ClaudeCN |
| `runapi` | `https://runapi.co` | Claude 旗舰列表 | 纯 anthropic/claude_code |
| `relaxycode` | `https://www.relaxycode.com` | Claude 旗舰列表 | Claude Code 编程中转 |
| `crazyrouter` | `https://cn.crazyrouter.com` | Claude 旗舰列表 | Claude Code 路由中转 |
| `sssaicode` | `https://node-hk.sssaicodeapi.com/api` | Claude 旗舰列表 | Claude Code 编程中转 |
| `micu` | `https://www.micuapi.ai` | Claude 旗舰列表 | 纯 anthropic/claude_code |
| `ctok` | `https://api.ctok.ai` | Claude 旗舰列表 | 纯 anthropic/claude_code |
| `eflowcode` | `https://e-flowcode.cc` | Claude 旗舰列表 | Claude Code 编程中转 |
| `lemondata` | `https://api.lemondata.cc` | Claude 旗舰列表 | 纯 anthropic/claude_code |
| `pipellm` | `https://cc-api.pipellm.ai` | Claude 旗舰列表 | base_url 含 `cc-api`=Claude Code API |
| `aigocode` | `https://api.aigocode.com`（另有 openai/gemini 同址） | Claude 旗舰列表 | 主打 Claude Code；openai/gemini 端点存在但同址中转，候选仍以 Claude 旗舰为主 + fetchModels |
| `packycode` | `https://www.packyapi.com`（+ openai `/v1` + gemini） | Claude 旗舰列表 | 多协议中转，主力 Claude；openai 槽可填 `gpt-5.5`/gemini 槽留 fetchModels |
| `cubence` | `https://api.cubence.com`（+ openai `/v1` + gemini） | Claude 旗舰列表 | 多协议中转，同上 |
| `rightcode` | `https://www.right.codes/claude`（+ openai `/codex/v1`） | Claude 旗舰列表 | anthropic 走 Claude；openai/codex 端点候选 `gpt-5.5-codex`（推测，核 fetchModels） |

> 上述 20 个 anthropic 端点均无公开「自有模型」——它们透传 Anthropic 模型，故候选直接 = Claude 旗舰列表，无独立来源标注需求；实际可用 id 以各平台 fetchModels（`GET /v1/models` 或等价）为准。

### 特殊判定平台

#### `aicodemirror`
- 端点: anthropic `https://api.aicodemirror.com/api/claudecode` + openai `.../api/codex/backend-api/codex` + gemini `.../api/gemini`（Platforms.tsx:294-298）。
- 候选: **Claude 旗舰列表**（anthropic/claude_code 主路径）；codex 端点候选 `gpt-5.5-codex`（推测）、gemini 端点候选 Gemini 旗舰（留 fetchModels）。
- 说明: 三协议镜像中转，无自有模型，透传上游官方模型。

#### `compshare`（优云 / UCloud ModelVerse）
- 端点: anthropic `https://api.modelverse.cn` + claude_code（Platforms.tsx:335-337）。
- 候选: **有自有/聚合模型**。compshare（优云智算 ModelVerse）是 UCloud 的模型聚合平台，除中转 Claude 外还聚合开源模型（DeepSeek、Qwen 等）。
- 来源: **推测:** 未实测 ModelVerse 模型清单 API；查过定位为 anthropic 兼容端点。建议候选 = Claude 旗舰列表（因预设走 anthropic/claude_code）+ **fetchModels 为主源**拉取其聚合清单。
- 说明: 与 `compshare_coding` 区分（见下）。

#### `compshare_coding`（优云 Coding Plan）
- 端点: anthropic `https://cp.compshare.cn` + claude_code（Platforms.tsx:338-340）。
- 候选: **Claude 旗舰列表**。优云 Coding Plan 是面向 Claude Code 的订阅套餐，透传 Claude 模型。
- 来源: 端点协议（anthropic/claude_code）。具体 id 以 fetchModels 为主。

#### `opencode`（OpenCode Zen / Go）
- 端点: **openai** `https://opencode.ai/zen/go` + `codex_tui`（Platforms.tsx:356-357）—— 注意：**非 anthropic 端点**。
- 候选: **有自有模型路由**。OpenCode Zen 是 opencode.ai 的模型网关，`/zen/go` 是其特定模型/套餐路由（"Go" 套餐），通过 OpenAI 兼容协议暴露。
- 来源: **推测:** 未实测 opencode.ai/zen 的 `/v1/models` 返回清单；查过其为 OpenAI 兼容网关，背后可路由多家模型（含 Claude、GPT、开源）。候选**不应**默认 Claude 旗舰列表（端点是 openai 协议、套餐路由），应**完全交给 fetchModels**，或给一个占位（如 `gpt-5.5` 推测）让用户下拉。
- 说明: 本组里最特殊的一个 —— openai 协议 + 自有套餐路由，与其余 25 个 Claude Code anthropic 中转性质不同。

#### `newapi`（New API / One API 中转面板）
- 端点: openai `https://your-newapi-instance.com/v1` + `codex_tui`（Platforms.tsx:360-361，base_url 为占位需用户自填）。
- 候选: **完全自定义，无固定候选**。New API / One API 是自建中转聚合面板，用户自部署，背后挂什么模型（Claude/GPT/Gemini/开源任意组合）完全由部署者决定。
- 来源: 不适用固定列表。**候选必须完全依赖 fetchModels**（New API 标准暴露 `/v1/models`）；内置候选可给空或给最常见组合占位。
- 说明: 与 opencode 一样不是 Claude 专用中转；不要硬填 Claude 旗舰列表。

#### `claude_code`（Claude Code 订阅透传）
- 端点: anthropic `https://api.anthropic.com` + `client_type: "default"`（纯透传，Platforms.tsx:365-366）。
- 候选: **Claude 旗舰列表**（直连 Anthropic 官方，模型即官方全系）。
- 来源: Anthropic 官方（同第一节）。
- 说明: 这是订阅透传到官方 api.anthropic.com，候选 = 官方当前全系 `claude-opus-4-8` / `claude-sonnet-4-6` / `claude-haiku-4-5`（+ 旧版可选）。

---

## 三、汇总结论

- **Claude 旗舰 API id 已 Anthropic 官方确认**（2026-06-17 WebFetch overview.md + claude-api skill + LiteLLM 定价数据三源一致）：
  - opus = `claude-opus-4-8`，sonnet = `claude-sonnet-4-6`，haiku = `claude-haiku-4-5`。
- **26 平台全覆盖**：
  - **22 个**纯 Claude Code anthropic 中转 → 候选 = Claude 旗舰列表（packycode, cubence, aigocode, rightcode, aicodemirror, pateway, ccsub, apikeyfun, apinebula, sudocode, claudeapi, claudecn, runapi, relaxycode, crazyrouter, sssaicode, compshare_coding, micu, ctok, eflowcode, lemondata, pipellm, claude_code）—— 注：claude_code 直连官方、aicodemirror 多协议镜像，均归此类主路径。
  - **3 个有自有/聚合模型或非 Claude 专用**，候选不应硬填 Claude 旗舰，须以 fetchModels 为主源：`compshare`（优云聚合）、`opencode`（Zen openai 网关套餐）、`newapi`（自建聚合面板）。
- **关键纠错**: 现有预设 `claude-haiku-4-6` 错误，应改 `claude-haiku-4-5`。

---

## 四、Caveats / Not Found / 推测项

- **未实测各第三方平台的 `/v1/models` 返回**：所有「自有模型」判定（compshare/opencode/newapi 背后实际清单）均为 **推测:**，基于平台定位与端点协议推断，未逐个 curl 验证（无凭证、避免外部请求）。落地时这些平台的真实候选**必须以 fetchModels 为主源**。
- **rightcode/aicodemirror 的 codex/openai 端点候选**（`gpt-5.5-codex` 等）为 **推测:** —— codex 变体确切 API id 截至 2026-06 未从官方 docs 确认（与 Platforms.tsx:383 既有 TODO 一致）。
- **packycode/cubence/aigocode 的 gemini 端点**：Gemini 旗舰 id 未在本次核查范围，留 fetchModels / 用户填。
- **未找到** compshare（ModelVerse）、opencode（Zen）、newapi 三家的公开官方模型清单文档；查过其端点协议（modelverse.cn=anthropic、opencode.ai/zen/go=openai、newapi=自建 openai），但未取到 model id 列表。
