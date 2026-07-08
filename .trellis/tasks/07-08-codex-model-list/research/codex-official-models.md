# Research: OpenAI Codex 协议 model_list 真值（Codex CLI / chatgpt backend 视角）

- **Query**: 调研 OpenAI Codex 协议（= Codex CLI / Coding Plan，走 chatgpt backend 订阅端点）官方支持的 model id 全集，为 aidog `platform-presets.json` codex 协议 `model_list.default` 提供真值
- **Scope**: external（OpenAI 官方文档 + openai/codex Rust 源码全仓搜索）
- **Date**: 2026-07-08（方向修正版 v2）

---

## ⚡ 方向修正（v2，必读）

用户/team-lead 澄清：**aidog 的 codex 协议 = Codex CLI / Coding Plan（走 chatgpt backend 订阅端点）**，与 **openai 协议（标准 API, api.openai.com/v1 + API key）独立**，model_list 不混。team-lead 假设「aidog preset 的 `gpt-5.5-codex` 可能不是抄错，而是 Codex CLI 内部约定/别名」。

**本版用 Codex CLI 源码全仓搜索（`gh search code --repo openai/codex`，已认证）证伪了该假设**：

> 🔴 **`gpt-5.5-codex` 在 openai/codex 全仓源码中完全不存在**（`gh search code "gpt-5.5-codex" --repo openai/codex` 返回空，2026-07-08）。它不是 Codex CLI 内部约定/别名 —— aidog preset 确实是抄错/外推占位。

v1 的「标准 API 视角」结论**依然成立且与源码一致**（Codex CLI chatgpt backend 与标准 API 共用同一套 model id 命名空间，见下）。下方新增「Codex CLI / chatgpt backend 源码视角」作为决定性证据。

---

## TL;DR（综合结论）

1. **codex 协议当前旗舰 model id = `gpt-5.5`（无 `-codex` 后缀）**，不是 `gpt-5.5-codex`。官方文档与 Codex CLI 源码双重确认。
2. **`-codex` 后缀是 5.1 / 5.2 / 5.3 代的历史命名**（codex 优化变体），5.4 代起 OpenAI 统一用 base id（gpt-5.x），**5.4/5.5 代不存在任何 `-codex` 后缀变体**（源码搜索空）。
3. **Codex CLI chatgpt backend 与标准 API 共用 model id 命名空间** —— 不存在「chatgpt backend 专用」的 model id。差别仅在端点（chatgpt backend 额外有 spark preview）。
4. **Codex CLI 不硬编码默认 model** —— `model` 是 `Option<String>`，默认从 model catalog 动态加载（`show_in_picker` 控制可见）。源码中仅有的默认 model 常量是 memory 子系统的 `gpt-5.4` / `gpt-5.4-mini`。
5. **chatgpt backend URL = `https://chatgpt.com/backend-api/`**（config/mod.rs:3906-3908），推理走 `/responses`（与标准 API 同 Responses API 协议，端点不同）。
6. **建议 codex `model_list.default`**：`["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"]`（+ 可选 `gpt-5.3-codex-spark` preview）。与 openai 协议 model_list 几乎一致（差别：codex 可含 spark，不含 nano）。

---

## Findings

### A. Codex CLI / chatgpt backend 源码视角（决定性证据）

#### A1. chatgpt backend 端点确认

| 证据 | 文件:行 | 原文 |
|---|---|---|
| chatgpt backend base URL 默认值 | `codex-rs/core/src/config/mod.rs:3906-3908` | `chatgpt_base_url: cfg.chatgpt_base_url.unwrap_or("https://chatgpt.com/backend-api/".to_string())` |
| Responses 推理端点 | `codex-rs/core/src/client.rs:158` | `const RESPONSES_ENDPOINT: &str = "/responses";` |
| ChatGPT backend 请求需 Codex backend auth | `codex-rs/chatgpt/src/chatgpt_client.rs:34,38` | `"ChatGPT backend requests require Codex backend auth"` / `"ChatGPT account ID not available, please re-run codex login"` |
| chatgpt backend 管理请求 header | `codex-rs/chatgpt/src/chatgpt_client.rs:9,52` | `OAI_PRODUCT_SKU_HEADER` / `CODEX_PRODUCT_SKU`（订阅 sku 标识）|

#### A2. `gpt-5.5-codex` 全仓不存在（证伪 team-lead 假设）

```
$ gh search code "gpt-5.5-codex" --repo openai/codex
(空 — 无任何命中，含测试代码)
```
> 结论：`gpt-5.5-codex` 不是 Codex CLI 内部约定 / 别名 / 路由 key。aidog preset 抄错外推（据 5.1-5.3 代的 `-codex` 命名规律外推到 5.5，但 5.4 代起该命名已废弃）。

#### A3. 当前代（5.4/5.5）model id —— 全部无 `-codex` 后缀

`gh search code "gpt-5" --repo openai/codex --language rust` 全量命中中，**5.4/5.5 代字面量**：

| model id | 文件:行 | 用途 |
|---|---|---|
| `gpt-5.5` | `codex-rs/model-provider/src/amazon_bedrock/catalog.rs` `GPT_5_5_OPENAI_MODEL_ID = "gpt-5.5"` | bedrock provider 映射到 openai canonical id |
| `gpt-5.4` | `codex-rs/model-provider/src/amazon_bedrock/catalog.rs` `GPT_5_4_OPENAI_MODEL_ID = "gpt-5.4"` | 同上 |
| `gpt-5.4` | `codex-rs/model-provider/src/provider.rs` `DEFAULT_MEMORY_CONSOLIDATION_PREFERRED_MODEL = "gpt-5.4"` | **默认常量**：memory consolidation 偏好 model |
| `gpt-5.4-mini` | `codex-rs/model-provider/src/provider.rs` `DEFAULT_MEMORY_EXTRACTION_PREFERRED_MODEL = "gpt-5.4-mini"` | **默认常量**：memory extraction 偏好 model |
| `gpt-5.4` | `codex-rs/core/src/guardian/metrics.rs` `model: "gpt-5.4"` | guardian 子系统 model |

> 5.4/5.5 代**无任何** `5.4-codex` / `5.5-codex` / `5.4-codex-mini` / `5.4-codex-max` 字面量（`gh search code "5.4-codex" --repo openai/codex` 返回空）。OpenAI 在 5.4 代放弃 `-codex` 后缀命名。

#### A4. `-codex` 后缀是历史命名（5.1/5.2/5.3 代，已 deprecated + 迁移提示）

| 历史 model id | 文件:行 | 状态 |
|---|---|---|
| `gpt-5-codex` | `codex-rs/tui/src/model_migration.rs` | 初代 codex 旗舰，**已迁移到 `gpt-5.1-codex-max`** |
| `gpt-5-codex-mini` | `codex-rs/tui/src/model_migration.rs` | 初代 codex mini，**已迁移到 `gpt-5.1-codex-mini`** |
| `gpt-5.1-codex-max` | `codex-rs/tui/src/model_migration.rs` / `config/src/types.rs` | 5.1 代 codex 旗舰，**当前仍有迁出提示**（`HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG`） |
| `gpt-5.1-codex-mini` | `codex-rs/tui/src/model_migration.rs` | 5.1 代 codex mini |
| `gpt-5.2-codex` | `codex-rs/models-manager/src/model_info.rs`（`"gpt-5.2-codex" | "exp-codex-personality" => ...`）/ `codex-api/src/rate_limits.rs` | 5.2 代，官方 Codex Models 页明列 deprecated |
| `gpt-5.2-codex-sonic` | `codex-rs/codex-api/src/rate_limits.rs` | rate limit 名称 |
| `gpt-5.3-codex` | `codex-rs/app-server/src/outgoing_message.rs`（测试）/ 官方文档 | 5.3 代，官方 deprecated |

**迁移 snapshot 证据**（`codex-rs/tui/src/snapshots/`）：
```
model_migration_prompt_gpt5_codex.snap:
  "Codex just got an upgrade. Introducing gpt-5.1-codex-max.
   We recommend switching from gpt-5-codex to gpt-5.1-codex-max.
   Codex-optimized flagship for deep and fast reasoning."

model_migration_prompt_gpt5_codex_mini.snap:
  "Introducing gpt-5.1-codex-mini. We recommend switching from gpt-5-codex-mini to
   gpt-5.1-codex-mini. Optimized for codex. Cheaper, faster, but less capable."
```

**当前迁移提示仅 5.1 代**（`codex-rs/tui/src/app/startup_prompts.rs:162-167`）：
- `HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG`
- `HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG`
> 即 5.1 代 codex 模型也已被推荐迁出至更新代（5.4/5.5 base id）。

#### A5. Codex CLI 不硬编码默认 model —— 动态 catalog

| 证据 | 文件:行 | 说明 |
|---|---|---|
| `model: Option<String>` | `codex-rs/core/src/config/mod.rs:621` | config.toml model 字段可选，无默认字面量 |
| 默认从 catalog 取 | `codex-rs/models-manager/src/manager.rs` `fn get_default_model` + `DEFAULT_MODEL_CACHE_TTL: Duration = 300s` | 动态查询 model catalog（后端 `/models` endpoint 或 `model_catalog_json`），TTL 5min 缓存 |
| `show_in_picker` 控可见 | `codex-rs/tui/src/app/startup_prompts.rs:179` `.find(|preset| preset.model == target_model && preset.show_in_picker)` | model picker 列表来自 catalog preset，`show_in_picker` 决定是否展示 |
| `--model` flag 接受任意字符串 | `codex-rs/cli/src/main.rs:1940` `model: shared.model` | 无枚举限制（测试用 `"gpt-5.1-test"` 证明任意串可传） |
| `model_catalog_json` 配置 | `developers.openai.com/codex/config-reference` | "Optional path to a JSON model catalog loaded on startup" |

> 含义：Codex CLI 客户端 model 列表是**服务端 catalog 驱动**的，客户端可传任意 model id（不合法则后端报错）。aidog 静态 model_list 只是给客户端探测 UI 用，应填**后端真实接受**的 canonical id。

### B. 标准 API 视角（v1 结论，仍成立，与源码一致）

#### B.1 官方 Codex Models 页「Recommended models」（developers.openai.com/codex/models）

| model id | 角色 | 状态 | 来源 |
|---|---|---|---|
| `gpt-5.5` | 当前旗舰（complex coding / computer use / research） | GA | https://developers.openai.com/codex/models |
| `gpt-5.4` | 上一代旗舰（professional coding / reasoning） | GA | 同上 |
| `gpt-5.4-mini` | mini 变体（responsive coding / subagents） | GA | 同上 |
| `gpt-5.3-codex-spark` | text-only 实时迭代 | **Preview**（仅 ChatGPT Pro） | 同上 |
| `gpt-5.3-codex` | 上一代 specialized codex variant | **Deprecated**（ChatGPT 登录）/ API-key 下仍可用 | 同上「Deprecated Codex models」段 |
| `gpt-5.2` | 上一代 base | **Deprecated**（Codex ChatGPT 登录） | 同上 |
| `codex-mini-latest` | 早期 codex 别名 | **Deprecated**（2025-11-17 通知） | https://platform.openai.com/docs/deprecations |

官方原句：
> "For most tasks in Codex, start with gpt-5.5." / "Deprecated Codex models — gpt-5.2, gpt-5.3-codex models are deprecated in Codex when you sign in with ChatGPT. ... Some models that are deprecated for ChatGPT sign-in may still be available in the API."

#### B.2 config-reference（developers.openai.com/codex/config-reference）

> `model` — "Model to use (e.g., `gpt-5.5`)."  （官方配置示例用无后缀 `gpt-5.5`，非 `gpt-5.5-codex`）

---

## 建议补入 codex 协议 `model_list.default`（旗舰优先）

**A. 必补 — 官方 Recommended GA + 源码默认常量**
1. `gpt-5.5` — 官方当前旗舰，Codex 推荐起点（"For most tasks in Codex, start with gpt-5.5"）
2. `gpt-5.4` — 上一代旗舰仍 GA；Codex CLI 源码 `DEFAULT_MEMORY_CONSOLIDATION_PREFERRED_MODEL`
3. `gpt-5.4-mini` — mini 变体；Codex CLI 源码 `DEFAULT_MEMORY_EXTRACTION_PREFERRED_MODEL`

**B. 可选补 — Preview（chatgpt backend 独有）**
4. `gpt-5.3-codex-spark` — 官方 research preview，仅 ChatGPT Pro / text-only。源码无字面量（catalog 动态），但官方 Codex Models 页明列。属 chatgpt backend 独有（标准 API 无）。

> 建议终值：`["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex-spark"]`（spark 视是否要含 preview 定）。若保守只 GA：`["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"]`。

## 不应补入的 id + 原因

| id | 不补原因 | 证据 |
|---|---|---|
| `gpt-5.5-codex` | **全仓源码不存在**，非 Codex CLI 约定/别名 | `gh search code "gpt-5.5-codex" --repo openai/codex` 空 |
| `gpt-5.1-codex-max` / `gpt-5.1-codex-mini` | 5.1 代历史，**当前有迁出提示** | startup_prompts.rs:162-167 migration prompt |
| `gpt-5.2-codex` / `gpt-5.2-codex-sonic` | 官方 deprecated | Codex Models 页「Deprecated Codex models」段 |
| `gpt-5.3-codex` | 官方 deprecated（ChatGPT 登录场景） | 同上 |
| `gpt-5-codex` / `gpt-5-codex-mini` | 初代，已迁移到 5.1（更老） | model_migration.rs snapshot |
| `codex-mini-latest` | 2025-11-17 官方弃用通知 | platform.openai.com/docs/deprecations |
| `gpt-5.4-nano` | Codex 不主推 nano（标准 API 才有），官方 Codex Recommended 未列 | developers.openai.com/codex/models |
| `chatgpt/*` / `azure/*` / `openai/gpt-*` 前缀 | provider-specific 非 canonical（codex 协议走 chatgpt backend，model id 不带 provider 前缀） | — |

## 🔴 `gpt-5.5-codex` 去留（需 main + 用户决策）

aidog preset 现用 `gpt-5.5-codex` 作 `models.default.gpt` 默认指向 + `STATIC_MODEL_IDS`。源码 + 官方文档双重证伪其存在。三个选项：

| 选项 | 影响 | 推荐度 |
|---|---|---|
| (a) 改默认指向官方 `gpt-5.5`，model_list 用 `gpt-5.5` | 与官方真值对齐，客户端探测/转发均正确 | 推荐 |
| (b) 保留 `gpt-5.5-codex` 作 aidog 内部别名，转发时映射到 `gpt-5.5` | 需在 aidog 代理层加 alias→canonical 映射；model_list 仍需补真实 id | 复杂，无收益 |
| (c) 维持现状不动 | model_list 探测返回不存在的 id，客户端可能调用失败 | 不推荐 |

> 本 task prd Out of Scope 原写「不改 `models.default.gpt` 指向（仍 gpt-5.5-codex）」—— 该决策基于「gpt-5.5-codex 是 Codex CLI 约定」的错误前提。**前提已证伪**，建议 main 据此重审 Out of Scope，至少 model_list 补集用官方 `gpt-5.5`。

---

## C. Responses API caveat（追加，回 team-lead 追问 5/6/7）

源码全部确认：**Codex CLI 仅支持 Responses API，Chat Completions 已移除**。

### C.1 请求路径 —— 所有 auth 模式统一 `/responses`

`to_api_provider`（`codex-rs/model-provider-info/src/lib.rs:241-259`）按 auth_mode 选 base_url：

| auth mode | base_url | 完整推理路径 |
|---|---|---|
| Chatgpt（订阅登录）/ ChatgptAuthTokens / Headers / AgentIdentity / PersonalAccessToken (PAT) | `CHATGPT_CODEX_BASE_URL` = `https://chatgpt.com/backend-api/codex`（lib.rs:38） | `/backend-api/codex/responses` |
| API key（CODEX_API_KEY / OPENAI_API_KEY） | `https://api.openai.com/v1` | `/v1/responses` |

+ `RESPONSES_ENDPOINT = "/responses"`（client.rs:158）。**两种模式统一 `/responses`，无 `/chat/completions`**。

> 注：`CHATGPT_CODEX_BASE_URL`（lib.rs:38，带 `/codex` 子路径）≠ 通用 `chatgpt_base_url`（config/mod.rs:3906 默认 `https://chatgpt.com/backend-api/`，无 `/codex`）。前者是 Codex 推理专用端点，后者是 ChatGPT 通用管理 API base。

### C.2 `wire_api` 唯一合法值 = `responses`

- `pub enum WireApi`（lib.rs:57）+ doc「The Responses API exposed by OpenAI at `/v1/responses`」（lib.rs:58）
- 配置 `wire_api = "chat"` → `Err(CHAT_WIRE_API_REMOVED_ERROR)`（lib.rs:50/:80）：「`wire_api = "chat"` is no longer supported. set `wire_api = "responses"`. https://github.com/openai/codex/discussions/7782」
- 默认硬编码 `wire_api: WireApi::Responses`（lib.rs:338/380，所有 provider 创建处；`config/src/thread_config.rs: wire_api = "responses"`）
- `ollama-chat` provider 亦移除（lib.rs:51-52，同 7782）
- 含义：**Codex CLI 已无 Chat Completions 能力**，任何 inbound/outbound 必须 Responses API

### C.3 model 字段位置 —— 请求体顶层 `model`（非嵌套）

`ResponsesApiRequest`（`codex-rs/codex-api/src/common.rs:216-220`）：
```rust
pub struct ResponsesApiRequest {
    pub model: String,           // ← 顶层字段
    pub instructions: String,
    pub input: Vec<ResponseItem>,
    ...
}
```
请求体形如 `{"model":"gpt-5.5","instructions":"...","input":[...]}`。**仍是顶层 `model`**（Responses API 标准），非 `responses.model` 嵌套。model_list 里填的 id（gpt-5.5 等）= 此字段值。

### C.4 对 aidog codex task 的 scope 含义（回 team-lead 担心）

- Codex CLI 作客户端连 aidog 时，发的是 `/responses` 请求体（顶层 `model`）。aidog 若支持 Codex CLI inbound，需 `/responses` 入站处理。
- aidog CLAUDE.md 约定 openai 协议 `provider_api_path()` 仅 `/chat/completions` —— 这是 **outbound**（aidog → 上游）。Codex CLI inbound `/responses` 处理属 codex_tui client_type 职责，与 openai 协议 outbound 独立。
- **本 model_list task 不触及 converter**（prd Out of Scope 第 62 行已正确隔离）：补 model_list 只影响静态 `/v1/models` + `/proxy/models` 探测返回的 id 列表，不改请求转发路径。
- **结论**：team-lead 担心的 scope 膨胀（「补 chatgpt backend endpoint 需先改 converter」）在本 task **不触发** —— 本 task 只改 preset JSON 的 model id 字符串列表 + STATIC_MODEL_IDS 常量。converter 适配另立 task。

---

## Caveats / Not Found

1. **model catalog 数据未直接获取**：Codex CLI 的 model picker 列表来自服务端 catalog（chatgpt backend `/models` endpoint 或 `model_catalog_json`），非源码硬编码。本调研证据是「源码字面量 + 官方文档页」交叉，未抓取实际 catalog JSON。推测: catalog 内容 = 官方 Codex Models 页 Recommended（gpt-5.5/5.4/5.4-mini/spark）。如需 100% 确认可让 aidog 代理抓一次 chatgpt backend `/models` 响应。
2. **`gpt-5.3-codex-spark` 源码无字面量**：`gh search code "codex-spark" --repo openai/codex` 返回空。但官方 Codex Models 页明列为 research preview。推测: spark 是 chatgpt backend catalog 动态项，不在客户端源码硬编码。补不补入 model_list 视是否要含 preview。
3. **aidog gateway 无 model alias 映射**（main 2026-07-08 核查）：`needs_model_remap`（forward.rs:80）= 路由级 actual_model≠requested_model 替换（如 claude→doubao 跨平台），**非协议级 alias→canonical**；grep `model_alias` / `canonical_model` / `codex.*alias` 全 0 命中。→ b 案（保留 gpt-5.5-codex 作 alias + 代理映射）不可行，需新开发映射逻辑。
4. **精确 sunset 日期未逐一摘录**：`codex-mini-latest` / `gpt-5.x-codex` 精确 shutdown 日期在 platform.openai.com/docs/deprecations 表格内。退役政策：GA ≥6 个月、specialized variant ≥3 个月（官方政策段）。
5. Codex CLI chatgpt backend 推理走 Responses API（同标准 API 协议），WebSocket 连接头 `responses_websockets=2026-02-06`（client.rs:155）；model 字段在 ResponsesApiRequest 透传，无后端专用改写。

## 官方源 URL 清单（≥3 独立源）

1. https://developers.openai.com/codex/models — Codex Models Recommended 权威列表
2. https://developers.openai.com/codex/config-reference — config.toml `model` 字段示例 `gpt-5.5`
3. https://platform.openai.com/docs/models — base 模型 gpt-5.4/5.4-mini/5.4-nano/5.5 + GPT-5.6 preview
4. https://platform.openai.com/docs/deprecations — codex-mini-latest(2025-11-17) / gpt-5.3-codex 弃用
5. https://github.com/openai/codex （源码 grep）— chatgpt_base_url 默认值、model_migration.rs、默认 model 常量、全仓无 gpt-5.5-codex

## Related Code / Spec（aidog 侧）

- `src-tauri/defaults/platform-presets.json:106-153` — codex 协议 preset（现 `model_list.default: ["gpt-5.5-codex"]`）
- `src-tauri/src/gateway/proxy/passthrough.rs:233-239` — `STATIC_MODEL_IDS`（含 `gpt-5.5-codex` + `gpt-5.5`）
- `.trellis/tasks/07-08-codex-model-list/prd.md` — 本 task 需求
- `.trellis/tasks/07-08-openai-endpoints-models/prd.md` — 姊妹 task（openai 协议 model_list，结论与本文一致：gpt-5.5/5.4/5.4-mini/5.4-nano）
