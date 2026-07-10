# client-types.json simulation header 全补全（~/.aidog 真实请求驱动）

## Goal

`src-tauri/defaults/client-types.json` 真值源补 claude_code 家族 5 entry 的 anthropic 协议特征 header 指纹（DB 24578 行 proxy_log 真实入站样本驱动），删 codex 家族透传类 session-id（按用户「session-id 透传不注入」规则），顺手修 `claude_code_sdk_ts` UA 后缀 bug（DB 实测 sdk-ts 0 命中，sdk-cli 128 行命中）。

**行为变更**：simulation 注入面扩张（claude_code 家族 anthropic 协议从「仅 x-api-key」→「x-api-key + 11 项特征 header + anthropic-beta」）；codex 家族注入面收缩（删 conversation_id/session_id 透传类）；公共契约层零改（Rust struct 字段名 / serde / command 签名 / 前端 TS 类型不动）。

## 用户澄清决策（AskUserQuestion 已答，2026-07-10）

| 问题 | 用户选 |
|---|---|
| anthropic-beta 注入策略 | **全补静态快照**（12 项全补含 beta，月级腐化手更同 STATIC_MODEL_IDS 模式，frontmatter last_updated 滚动） |
| stainless 跨机器值（runtime-version/os/arch） | **固定实测值 macOS/arm64/node v26.3.0**（多数 Claude Code 用户在 mac，上游不校验 stainless 自报指纹） |
| codex 现有 conversation_id/session_id `{uuid}` | **删**（按 session-id 透传不注入规则，入站 codex 客户端带的这俩原样透传更真实；model_test/curl 直接打代理无入站时上游可能拒但可接受） |
| claude_code_sdk_ts UA bug | **本 task 顺手修**（1 行 UA 后缀 sdk-ts→sdk-cli，同文件改动合并） |

## grill 硬门2 决策（AskUserQuestion 已答，2026-07-10）

| 弱点 | 用户选 |
|---|---|
| UA 版本号 scope（原仅 sdk_ts，扩全家族） | **全家族 5 entry 刷 2.1.204**（DB 实测主流 9410 行最高命中，bundled 旧值 1.0.117 偏离真实，同字段同性质改动合并） |
| simulation 配置消费单测 | **grep + 缺则补**（exec agent 先 grep baseline 有无 client-types simulation 单测，有跑过即验；无则加 1-2 条配置消费单测防 regression） |
| last_updated 滚动值 | **exec 时 `date +%s` 实取**（非硬编码，防远端 sync 比对回退） |

## Scope

### claude_code 家族 5 entry（cli / vscode / sdk_ts / sdk_py / gh_action）anthropic 协议补 12 项特征 header

DB 实测 5 entry 指纹**完全一致**（仅 UA 后缀异，特征 header 集相同），共用清单（research `client-type-headers.md:88-105` + `:141-154`）：

| header name | value | 类别 |
|---|---|---|
| `anthropic-version` | `2023-06-01` | 静态 |
| `anthropic-beta` | `claude-code-20250219,interleaved-thinking-2025-05-14,thinking-token-count-2026-05-13,context-management-2025-06-27,prompt-caching-scope-2026-01-05,mid-conversation-system-2026-04-07,advanced-tool-use-2025-11-20,effort-2025-11-24,extended-cache-ttl-2025-04-11` | 静态快照（月级腐化，frontmatter last_updated 滚动） |
| `anthropic-dangerous-direct-browser-access` | `true` | 静态 |
| `x-app` | `cli` | 静态 |
| `x-stainless-arch` | `arm64` | 静态（固定 macOS 实测值） |
| `x-stainless-lang` | `js` | 静态 |
| `x-stainless-os` | `MacOS` | 静态（固定实测值） |
| `x-stainless-package-version` | `0.94.0` | 静态快照（跟 anthropic SDK 版本，月级腐化） |
| `x-stainless-retry-count` | `0` | 静态 |
| `x-stainless-runtime` | `node` | 静态 |
| `x-stainless-runtime-version` | `v26.3.0` | 静态（固定实测值，跨机器 node 版本不同） |
| `x-stainless-timeout` | `600` | 静态 |

注入位置：`simulation.auth.anthropic[]` 数组（现有 `x-api-key` 保留，12 项追加其后）。

**不注入**：`x-claude-code-session-id`（uuid 透传类，用户规则禁注入，入站 claude_code 客户端本就带此头透传到上游）。

### openai/gemini/default 协议（claude_code 家族）

**保守不补 stainless 指纹**（research `client-type-headers.md:156-162`）：x-stainless-* 由 anthropic JS SDK 发，OpenAI/Gemini 客户端不发，cross-protocol 时这些头在 OpenAI 端是「跨协议噪音」（上游忽略未知头不报错，但无意义）。仅保留现状 UA + auth headers。

### codex 家族 4 entry（cli/tui/desktop/vscode）openai 协议

删 `conversation_id: {uuid}` + `session_id: {uuid}`（透传类，按用户规则）。保留 `OpenAI-Beta: responses=experimental` + auth headers。

### claude_code_sdk_ts UA 后缀修

`simulation.user_agent`：`claude-cli/1.0.117 (external, sdk-ts)` → `claude-cli/2.1.204 (external, sdk-cli)`（DB 实测 TS SDK 发 sdk-cli 后缀，2.1.204 是当前主流版本）。

**注**：UA 版本号 `1.0.117` 是 bundled 旧值，全 claude_code 家族 5 entry 同步刷新到 DB 实测主流 `2.1.204`（research `:56` 9410 行最高命中）—— 顺手统一（同字段同性质改动，不拆 task）。

### last_updated 滚动

`client-types.json` 顶层 `last_updated` 手更到当前提交 Unix 秒（同 platform-presets 手维护模式，触发远端 sync 比对更新）。

## Requirements

### R1 改 client-types.json（worktree 内）

- claude_code 家族 5 entry 的 `simulation.auth.anthropic[]` 追加 12 项特征 header（上表）
- codex 家族 4 entry 的 `simulation.auth.openai[]` 删 `conversation_id` + `session_id` 2 项
- claude_code 家族 5 entry 的 `simulation.user_agent` 版本号统一刷 `2.1.204` + sdk_ts 后缀 `sdk-ts`→`sdk-cli`
- 顶层 `last_updated` 滚动到当前 Unix 秒

### R2 Rust 引擎零改（公共契约层禁改）

- `headers.rs` `fill_placeholder` L253-257 已支持 `{api_key}` + `{uuid}`（research caveat #6「{uuid} 未实现」是误判，核代码 L253 已 `.replace("{uuid}", &uuid_sim())`）—— 本 task 不动引擎
- `apply_client_headers` / `build_upstream_headers` 不改（配置驱动，新 header 由现有循环 `for h in headers { rb.header(&h.name, fill_placeholder(&h.value, api_key)) }` 自动消费）
- `strip_anthropic_beta_for_third_party` L94-96 保留（第三方 anthropic 端点剔 anthropic-beta，simulation 注入仅官方 api.anthropic.com 生效 —— 已有行为，本 task 不动）

### R3 验证

- `cargo build --workspace` + `cargo test --workspace`（baseline 1395+，无回归）
- `cargo clippy --workspace --all-targets` 无新 warning
- `yarn build` 全绿
- `yarn check:i18n`（client-types.json label/desc 未改，i18n 无影响 —— 验证无悬空）
- client-types.json 仍合法 JSON（`python3 -c 'import json; json.load(open(...))'`）
- grep 验收：
  - claude_code 家族 5 entry 的 `simulation.auth.anthropic[]` 各含 13 项（1 x-api-key + 12 特征）
  - codex 家族 4 entry 的 `simulation.auth.openai[]` 各含 3 项（Authorization + api-key + OpenAI-Beta），无 `conversation_id` / `session_id`
  - grep `sdk-ts` 在 client-types.json = 0
  - grep `1.0.117` 在 client-types.json = 0（全刷 2.1.204）
- 行为等价 test 矩阵（如 baseline 已有 client-types simulation 单测，跑过即验配置驱动消费正常）

## Acceptance Criteria

- [ ] claude_code 家族 5 entry anthropic 协议补全 12 项特征 header（research 清单）
- [ ] codex 家族 4 entry openai 协议删 conversation_id/session_id
- [ ] claude_code_sdk_ts UA 后缀 sdk-ts→sdk-cli + 全家族版本号刷 2.1.204
- [ ] client-types.json last_updated 滚动
- [ ] cargo build/test/clippy --workspace 全绿，无新 warning
- [ ] yarn build 全绿
- [ ] Rust 引擎零改（headers.rs fill_placeholder / apply_client_headers diff 0）
- [ ] 公共契约层零改（Rust struct 字段名 / serde / command 签名 / 前端 TS 类型 diff 0）
- [ ] 主仓零改动（worktree 内）

## Out of Scope

- codex 家族 / cursor / windsurf 特征 header 补全（DB 0 样本，禁臆造；需用户采集真实流量后另开 task）
- x-claude-code-session-id 注入（uuid 透传类，用户规则禁注入）
- stainless 跨机器值占位符化（`{node_version}`/`{os}`/`{arch}` 引擎扩 —— 用户选固定值，过度工程避）
- anthropic-beta 占位符化（同上）
- apply_client_headers「入站优先」逻辑改（用户选删 codex session，不改引擎）
- 远端同步链改动（last_updated 滚动触发 sync，reader deep merge 补缺，已有 7 件套不改）
- client-types.json label/desc/name/group 字段（展示层，不动）
- platform-presets.json（独立 task `07-10-presets-desc-remove-format`）

## Technical Notes

- **真值源约定**（CLAUDE.md）：`src-tauri/defaults/client-types.json` 手维护，远端同步 7 件套（spec `backend/remote-json-sync.md`），reader deep merge（app data 优先 + bundled 补缺 key）
- **配置驱动执行引擎范式**（spec `guides/cross-layer-rules.md`）：match 臂硬编码 → JSON 真值源 per-entry 配置 + 通用占位符引擎 `{api_key}`/`{uuid}` + OnceLock 启动加载 + 行为等价 test 矩阵。本 task 是该范式第 3 次实例（header 指纹补全，纯配置扩，引擎零改）
- **simulation 透传 vs 注入语义**（research `:40-48`）：
  - `passthrough_convert_headers` 透传入站头底座（剔 stripped：host/content-length/hop-by-hop/auth/UA/CT + 第三方端点剔 anthropic-beta）
  - `apply_client_headers` 用 simulation 覆盖 UA + auth + 特征头（`.header()` 单值覆盖入站同名）
  - 入站客户端发的 x-stainless-* / anthropic-version / x-app **全部原样透传**；simulation 注入的特征 header 仅在「入站缺这些 header」时生效（如 model_test、curl 直接打代理、客户端不发指纹）
- **anthropic-beta 第三方端点剔除**（headers.rs:94-96）：上游 host ≠ api.anthropic.com 时剔 anthropic-beta（GLM/中转站不认新 beta token，原样透传触发 400 code 1210）。simulation 注入 anthropic-beta 仅对官方 api.anthropic.com 生效 —— 已有行为，本 task 不动
- **{uuid} placeholder 已实现**（headers.rs:253-257 `fill_placeholder`）：research caveat #6「未实现」是误判（agent 读旧版或误读 L310），核代码 L253 已 `.replace("{uuid}", &uuid_sim())`。codex 现有 {uuid} 实际正常生成 uuid 发上游 —— 本 task 删这俩是「按规则不注入」非「修 bug」
- **UA 版本号刷新**：bundled 旧值 1.0.117，DB 实测主流 2.1.204（research `:56` 9410 行最高命中）。全家族 5 entry 统一刷（同字段同性质，不拆 task）
- **跨层契约**（spec `guides/cross-layer-rules.md`）：本 task 纯 JSON 配置扩，Rust struct `Simulation`/`SimulationHeader`/`AuthMatrix` 字段不动，serde 字段名不动，前端 TS 类型不动（前端不消费 simulation，仅消费 label/group/name 展示层）

## 数据流（强制）

```
client-types.json（claude_code 家族补 12 项 + codex 删 2 项 + UA 刷 + last_updated 滚动）
  ↓ include_str! bundled
Rust SIMULATION_CACHE OnceLock（headers.rs:189 启动加载，禁每请求读盘）
  ↓ apply_client_headers / build_upstream_headers（配置驱动消费，引擎零改）
上游请求（claude_code 家族官方端点带全指纹；第三方端点 anthropic-beta 被剔其余 11 项生效）
  ↓
proxy_log.upstream_request_headers（日志镜像实发，fill_placeholder {api_key}→redact_key 脱敏）
```
