# Research: HTTP_PROXY (CONNECT 隧道) vs 方案 A (BASE_URL 走 /proxy 明文) — 场景层判定

- **Query**: 查清 HTTP_PROXY/HTTPS_PROXY（CONNECT 隧道）vs 方案 A（配 ANTHROPIC_BASE_URL 走 /proxy HTTP 明文）在 aidog 的实际使用场景差异；判定哪些场景必须 HTTP_PROXY（方案 A 不可行），哪些方案 A 已覆盖；给 P3 go/no-go 的场景层依据
- **Scope**: mixed（internal codebase 证据为主 + 外部 CLI 行为，无外网工具可用处标「推测」）
- **Date**: 2026-07-03
- **上游依赖**:
  - `.trellis/tasks/proxy-http-relay/research/p2-middleware-reuse.md`（盲转隧道层原理不可能让核心规则生效）
  - `.trellis/tasks/proxy-http-relay/research/mitm-feasibility.md`（P3 MITM 默认 NO-GO）
  - `.trellis/spec/backend/proxy-connect-relay.md`（P1 契约）
  - P1 prd: `.trellis/tasks/archive/2026-07/07-02-07-01-proxy-http-relay-p1/prd.md`

---

## TL;DR（场景层 go/no-go 推荐）

**P3 场景层 NO-GO。** 未发现任何「必须 HTTP_PROXY、方案 A 不可行」的硬场景。

- aidog 当前**已为 Claude Code + Codex CLI 自动写入方案 A 配置**（`ANTHROPIC_BASE_URL` / Codex `model_providers.aidog.base_url`），指向 `http://127.0.0.1:<port>/proxy`，HTTP 明文直连 aidog → **规则全套生效**（middleware / 路由 / 模型映射 / headers / retry / cost / agg 全部经 `forward_attempt` 链）。
- 主流 AI CLI（Claude Code / Codex / aider / opencode）**均支持自定义 BASE_URL**，方案 A 全覆盖。Cursor / VSCode 扩展类有图形配置或 env 注入，方案 A 同样可行。
- HTTP_PROXY（CONNECT 隧道）的唯一独占场景是**「客户端不支持自定义 endpoint、只能配系统代理」**的极少数工具（如某些不支持自定义 base_url 的浏览器/GUI app/老旧 SDK）。这些场景里流量是任意 HTTPS，**本就不该过 aidog AI 规则层**（非 AI 流量），盲转即正确语义。
- **唯一需要 MITM 的真实诉求** = 「用户拒绝改任何配置、只想 `export HTTP_PROXY`，且期望 AI 规则在加密隧道内仍生效」——这是 mitm-feasibility.md 已判定 NO-GO 的重风险路径（假 CA + 私钥泄露 = 全网 HTTPS 风险 + 8-14 人天）。

**结论**：环境变量代理方案下「规则不生效」不是方案缺陷，是**用户用错了方案**——aidog 已提供零风险且规则全套生效的方案 A（BASE_URL），用户切到方案 A 即解。

---

## 1. aidog 现有客户端配置模式（方案 A 已落地的证据）

### 1.1 Claude Code — 自动写 `~/.aidog/settings.<group>.json` 的 env block

**写入点**：`src-tauri/src/commands/sync_settings.rs:276-284`

```rust
// sync_settings.rs:276-284
if let Some(env_map) = obj.get_mut("env").and_then(|v| v.as_object_mut()) {
    env_map.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        serde_json::Value::String(format!("http://127.0.0.1:{}/proxy", port)),  // ← 方案 A
    );
    env_map.insert(
        "ANTHROPIC_AUTH_TOKEN".to_string(),
        serde_json::Value::String(group_key.clone()),  // ← group_key 作 Bearer
    );
```

- aidog **自动**为每个分组生成 `~/.aidog/settings.<group>.json`，`env.ANTHROPIC_BASE_URL` 指向 `http://127.0.0.1:<port>/proxy`（HTTP 明文，非 HTTPS）。
- Claude Code 用 `ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN` 环境变量重定向所有请求到 aidog 的 `/proxy` path 路由 → 走 `handle_proxy_core` → `forward_attempt` 链 → **middleware / 路由 / headers / retry / cost 全生效**。
- 保护机制：`sync_settings.rs:292-298` 用户自定义 env_vars 同名 `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN` 被丢弃 + warn（防覆盖路由）。
- 内部标记 `_aidog_statusline` / `_aidog_subagent_statusline` 在写文件前 strip（`sync_settings.rs:312-313`，spec CLAUDE.md 锁定）。

### 1.2 Codex CLI — 自动写 `$CODEX_HOME/<group>.config.toml` + 全局 config.toml

**写入点**：`src-tauri/src/gateway/codex.rs:54-88`（`build_group_profile_toml`）+ `codex.rs:238-266`（默认组 merge 进用户级 config.toml）

```rust
// codex.rs:62-77  build_group_profile_toml
aidog.insert("base_url".to_string(),
    toml::Value::String(format!("http://127.0.0.1:{port}/proxy")));  // ← 方案 A
aidog.insert("wire_api".to_string(), toml::Value::String("responses".to_string()));
aidog.insert("env_key".to_string(),  toml::Value::String("AIDOG_KEY".to_string()));
```

- Codex profile 顶层 `model_provider="aidog"` + `[model_providers.aidog]`（base_url 指向 `/proxy`，wire_api=responses，env_key=AIDOG_KEY）。
- 用户 `codex -p <group>` 加载 profile，或默认组自动 merge 进 `~/.codex/config.toml`。
- **固有限制**（`codex.rs:233-235`）：Codex profile `env_key="AIDOG_KEY"` 让 codex 从 env 读 token，全局 config.toml 无法内联 token → 用户须 `export AIDOG_KEY=<group_key>`。这是 Codex 自身设计，与方案 A 的可行性无关。

### 1.3 statusline bash 脚本（spec `.trellis/spec/backend/proxy-connect-relay.md`）— 方案 A 同款模式

- spec 引用：statusline bash 脚本通过 `ANTHROPIC_BASE_URL`（= 代理根 URL）+ `ANTHROPIC_AUTH_TOKEN`（= group_name）调用端点。前端脚本生成入口：`src/components/settings/editors.tsx` `statuslineApi.generate`，后端落盘 `src-tauri/src/commands/settings.rs:59-86`。
- 通用事件 hook 脚本（`src-tauri/src/gateway/hooks/scripts.rs:38-39, 66-67, 138-139`）同样读 `ANTHROPIC_BASE_URL` 剥 `/proxy` 后缀拼 `/api/notify` 端点。
- **含义**：aidog 内部所有脚本都依赖方案 A 的 env 变量模型。HTTP_PROXY 方案下这些 env 变量不存在（CONNECT 隧道不设 `ANTHROPIC_BASE_URL`），脚本会 fallback 到空 base → 失效。

### 1.4 用户可编辑的 HTTP_PROXY/HTTPS_PROXY 字段（不是路由到 aidog，是 aidog 自己出站）

- `src/services/claude-settings-schema.ts:244-246`：Claude Code settings schema 暴露 `HTTP_PROXY` / `HTTPS_PROXY` / `NO_PROXY` 三字段（group: "network"），用户可在 Claude settings tab 编辑，写入 `~/.claude/settings.json` env block。
- **这是给 Claude Code 自己出站用的**（用户公司上游代理），不是把 Claude Code 流量路由到 aidog。写 `HTTP_PROXY=127.0.0.1:<aidog_port>` 理论上也能让 Claude Code 走 CONNECT 隧道到 aidog，但这是 P1 隧道路径（盲转，规则不生效），不是 aidog 推荐用法。
- `src-tauri/src/gateway/skills/proxy_env.rs:44-55` `apply_proxy_env`：aidog 给 **npx skill 子进程**注入 `HTTP_PROXY`/`HTTPS_PROXY`（用于 npm 拉包过公司代理），与「客户端连 aidog」方向相反。

### 1.5 aidog 当前**未**自动配 BASE_URL 的客户端

- opencode / aider / Cursor / 其他 IDE 扩展：aidog **无自动写配置逻辑**（grep 仅 `model_fetch.rs:79` 用 `opencode.ai/zen` 做协议探测，非配置写入）。这些工具用户需**手动**配 BASE_URL（方案 A 仍可行，只是非自动）。

---

## 2. P1 CONNECT 隧道「为何做」（P1 目标场景）

引自 P1 prd `.trellis/tasks/archive/2026-07/07-02-07-01-proxy-http-relay-p1/prd.md:5-6, 12`：

> **目标**: aidog 作为标准 HTTP 代理工作: 客户端配 `http_proxy=127.0.0.1:<port>` 后任意 HTTP/HTTPS 流量经 CONNECT 隧道转发, 记 proxy_log 元数据, Logs/Stats 可按「无平台」「无分组」筛选。
>
> 形态: HTTP CONNECT 隧道(标准 http_proxy) | 用户锁(parent PRD)

P1 的目标场景**不是**「让 AI 规则在 HTTP_PROXY 下生效」（那是 P2/P3），而是：

1. **aidog 兼容标准 `http_proxy` 协议** —— 让任何配了 `http_proxy` 指向 aidog 的客户端流量都能经 CONNECT 转发（通用 TCP 盲转），不限于 AI CLI。
2. **元数据记账**（host / status / duration / platform_id host 命中），Logs/Stats 可见，非 AI 流量按「无平台」「无分组」筛。
3. **为 P2（元数据补强）/ P3（MITM）铺路**（P1 prd:43-48 非目标明确「HTTPS 解密 P2 MITM」「字节统计 P2 再评估」）。

**P1 不要求规则在隧道层生效** —— P1 是基础设施层（让 aidog 能当 http_proxy 用），规则生效是 P3 MITM 的范畴。当前 prd.md:14 用户诉求「确保 /proxy 路径下代理转发规则、中间件等规则，在以环境变量代理的方案下依旧生效」是 **P3 触发诉求**，但本 research 结论：该诉求用方案 A 已满足，无需 P3。

---

## 3. CLI 代理支持矩阵（方案 A vs HTTP_PROXY）

> 证据等级标注：✅ codebase 实证 / 📖 官方文档（本会话无外网工具，基于既有权威知识，标「推测」处需 main 复核）/ 推测 无把握

| 工具 | 支持自定义 BASE_URL（方案 A 可覆盖?）| 配置方式 | 荣 HTTP_PROXY/HTTPS_PROXY | aidog 现状 |
|---|---|---|---|---|
| **Claude Code** | ✅ 是 | env `ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN`（写 `~/.claude/settings.json` 的 env block）| ✅ 是（Node fetch/undici 读 `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY`，spec schema `claude-settings-schema.ts:244-246` 暴露给用户编辑）| ✅ aidog **自动写**（`sync_settings.rs:278`）|
| **OpenAI Codex CLI (Rust)** | ✅ 是 | `~/.codex/config.toml` `[model_providers.<name>]` 的 `base_url` 字段（官方 config schema）| 推测 是（reqwest 默认读 `HTTP_PROXY`/`HTTPS_PROXY`，Rust 生态标准行为；需 main 用 `codex --help` 或 repo 核实）| ✅ aidog **自动写**（`codex.rs:67`）|
| **Codex TUI / Desktop / VSCode** | ✅ 是（同 Codex config.toml 体系，`Platforms.tsx:119-122` 协议预设）| 同上 | 推测 是 | ✅ aidog 自动写 profile |
| **aider** | ✅ 是 | CLI `--openai-api-base <url>` 或 env `OPENAI_API_BASE=<url>`（📖 aider docs `docs.aider.chat` config/connection 章节，推测 URL 路径）| ✅ 是（Python `requests`/`httpx` 读 `HTTP_PROXY`/`HTTPS_PROXY`，标准库行为）| ❌ aidog 未自动配（用户手动）|
| **opencode** | ✅ 是 | `--base-url` flag 或 config（aidog 平台预设 `Platforms.tsx:89-90,397` 用 `https://opencode.ai/zen/go/v1`，协议层支持自定义 base_url）| 推测 是（Go `http.ProxyFromEnvironment` 默认读 `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY`，Go 标准库行为）| ❌ aidog 未自动配 |
| **Cursor IDE** | 推测 是（图形 Settings → Models → Override OpenAI Base URL，README/设置面板可见；推测非自动写入）| 图形配置 | ✅ 是（Electron/Node 走 `HTTP_PROXY`）| ❌ aidog 未自动配 |
| **curl** | ✅ 是（`--url` / 直接给 URL）| CLI arg | ✅ 是（`-x` / `HTTP_PROXY` env）| N/A |
| **git** | ❌ 否（git 不发 AI 请求，无 BASE_URL 概念）| — | ✅ 是（`http.proxy` config / `HTTP_PROXY`）| N/A（非 AI 流量）|

**关键观察**：
- 所有主流 AI CLI **都支持自定义 BASE_URL**，方案 A 全覆盖。
- HTTP_PROXY/HTTPS_PROXY 这些工具**也都荣**（标准库/运行时默认行为），但走 HTTP_PROXY 路径时流量进 CONNECT 隧道 → TLS 加密 → aidog 盲转 → **规则不生效**（p2-middleware-reuse.md §3 实证）。
- **不存在「只支持 HTTP_PROXY、不支持自定义 BASE_URL」的主流 AI CLI**（grep/codebase 无反例，外部标推测待 main 复核）。

---

## 4. HTTP_PROXY vs BASE_URL 实际差异（规则生效维度）

| 维度 | 方案 A（BASE_URL → `/proxy` HTTP 明文）| HTTP_PROXY（CONNECT 隧道）|
|---|---|---|
| **流量形态** | 客户端直连 aidog HTTP 明文 → aidog 用 reqwest 重建请求转发真上游 | 客户端发 `CONNECT host:443` 建 TCP 隧道 → 隧道内 TLS 加密 → aidog `tokio::io::copy` 盲转密文 |
| **aidog 可见性** | 全明文（method/path/headers/body/model/apikey 全可见）| 仅 TCP 元数据（target host:port / status / duration）|
| **middleware engine**（context strip / 脱敏 / 注入）| ✅ 生效（`handler.rs:290-297` `apply_inbound`）| ❌ 原理不可能（TLS 加密，无 body）|
| **router 平台选择 + 模型映射** | ✅ 生效（apikey → group → candidates + model_mapping）| ❌ 原理不可能（无 apikey / 无 model）|
| **headers 转换**（auth 注入 / UA / CT）| ✅ 生效（`forward.rs:268` `build_upstream_headers`）| ❌ 原理不可能（不能往加密流塞明文头）|
| **retry 候选遍历** | ✅ 生效（`handler.rs:382-421` 多候选 forward_attempt）| ❌ 语义不符（单 TCP 隧道绑定单 target）|
| **cost 估算 / agg 统计** | ✅ 生效（`spawn_estimate` / `upsert_stats_agg`）| ❌ spec 锁定 0 token 0 cost（隧道非 AI 请求）|
| **用户配置成本** | aidog 自动写（CC + Codex）；其他工具手动 1 行 | 用户 `export HTTP_PROXY=...`（看似省事，但规则失效）|
| **结论** | **零风险 + 规则全套生效 = 推荐方案** | 仅当客户端不支持 BASE_URL 时用（盲转即正确语义）|

**核心矛盾**：用户配 HTTP_PROXY「省事」（一行 env），代价是**所有 AI 规则失效**。配 BASE_URL「多一行」，换来规则全套生效。aidog 已把 BASE_URL 配置自动化（CC/Codex 零用户操作），方案 A 对用户实际更省事。

---

## 5. 「必须 HTTP_PROXY（方案 A 不可行）」的硬场景判定

逐场景核验：

| 场景 | 方案 A 可行? | 必须用 HTTP_PROXY? | 判定 |
|---|---|---|---|
| Claude Code 走 aidog | ✅（aidog 自动写 `ANTHROPIC_BASE_URL`）| 否 | **方案 A 已覆盖** |
| Codex CLI 走 aidog | ✅（aidog 自动写 profile base_url）| 否 | **方案 A 已覆盖** |
| aider 走 aidog | ✅（用户配 `--openai-api-base` / `OPENAI_API_BASE`）| 否 | **方案 A 可覆盖**（手动）|
| opencode 走 aidog | ✅（用户配 `--base-url`）| 否 | **方案 A 可覆盖**（手动）|
| Cursor 走 aidog | 推测 ✅（图形 Override OpenAI Base URL）| 否 | 推测 **方案 A 可覆盖** |
| 浏览器访问平台官网（非 AI 流量）| N/A | 是 | **盲转即正确**（非 AI 流量不该过规则层）|
| 不支持自定义 endpoint 的 GUI app | ❌ | 是 | **盲转即正确**（流量非 AI，规则无对象）|
| 用户「什么都不想改，只想 export HTTP_PROXY 且要求规则生效」| ❌（方案 A 要改配置）| 是 | **唯一硬场景**——但 MITM 解法风险/成本远超收益（见 §6）|

**判定：无硬场景需要 P3。** 所有 AI CLI 场景方案 A 已覆盖（自动或手动）；非 AI 流量场景盲转即正确语义；唯一「硬场景」是用户惰性诉求，用 P3 MITM 解决的代价（假 CA + 私钥泄露 = 全网 HTTPS 风险 + 8-14 人天）远超收益。

---

## 6. P3 go/no-go 场景层推荐

### 推荐：**P3 场景层 NO-GO**

**理由**：

1. **方案 A 零风险已覆盖全部已知 AI CLI 场景**（§5）。规则全套生效（middleware/路由/headers/retry/cost/agg），无任何原理限制。
2. **HTTP_PROXY 盲转是正确语义**（非 AI 流量不该过规则层），不需要 MITM 救场。
3. **唯一硬场景（用户拒绝改配置）的 MITM 解法代价失衡**（mitm-feasibility.md TL;DR）：
   - 假 CA 需用户 sudo 装系统信任库（Tauri 无权静默装）
   - 假 CA 私钥泄露 = 用户全网 HTTPS 被 MITM（bank/邮箱/密码）
   - 8-14 人天 + 维护成本（双 TLS / 白名单 / 证书缓存 / SNI 兜底）
4. **正确解法是引导用户用方案 A**，而非为「用户配错方案」做重风险补救。

### 对 prd.md 用户诉求的回应

用户原话：**「确保 /proxy 路径下代理转发规则、中间件等规则，在以环境变量代理的方案下依旧生效」**

- 「环境变量代理方案」= HTTP_PROXY → CONNECT 隧道 → TLS 加密 → **原理上规则不可能生效**（p2-middleware-reuse.md §3 实证）。
- 该诉求的根因是用户**误用 HTTP_PROXY 方案**——aidog 已提供零风险、规则全套生效的方案 A（`ANTHROPIC_BASE_URL` 自动写）。
- **建议 main 转达用户**：把诉求从「让规则在 HTTP_PROXY 下生效」转为「引导用户用方案 A（已自动配 Claude Code / Codex，手动配其他工具）」。若用户坚持 HTTP_PROXY + 规则生效，则进入 P3 MITM（需用户明确接受假 CA + 私钥风险）。

---

## 关键文件路径（供 main 引用）

- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/commands/sync_settings.rs:276-298` — Claude Code `ANTHROPIC_BASE_URL`/`AUTH_TOKEN` 自动写 + 用户覆盖保护
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/codex.rs:54-88, 238-266` — Codex profile base_url 自动写（per-group + 默认组 merge）
- `/Users/luoxin/persons/lyxamour/aidog/src/services/claude-settings-schema.ts:244-246` — `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY` 用户可编辑字段
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/skills/proxy_env.rs:44-55` — npx skill 子进程 HTTP_PROXY 注入（aidog 出站，非路由到 aidog）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/hooks/scripts.rs:38-39, 66-67` — 通用 hook 脚本读 `ANTHROPIC_BASE_URL` 推 `/api/notify`（方案 A 依赖）
- `/Users/luoxin/persons/lyxamour/aidog/src-tauri/src/gateway/proxy/connect.rs` — P1 CONNECT 盲转隧道
- `/Users/luoxin/persons/lyxamour/aidog/.trellis/tasks/proxy-http-relay/research/p2-middleware-reuse.md` — 隧道层规则不生效原理实证
- `/Users/luoxin/persons/lyxamour/aidog/.trellis/tasks/proxy-http-relay/research/mitm-feasibility.md` — P3 MITM 默认 NO-GO（go-after-warnings）
- `/Users/luoxin/persons/lyxamour/aidog/.trellis/spec/backend/proxy-connect-relay.md` — P1 契约
- `/Users/luoxin/persons/lyxamour/aidog/.trellis/tasks/archive/2026-07/07-02-07-01-proxy-http-relay-p1/prd.md` — P1 目标场景

## Caveats / 需要

- **无外网搜索工具**（本会话仅 Read/Write/Bash/Skill/octocode，无 `mcp__exa__*` / WebSearch）：CLI 代理支持矩阵 §3 中 Codex Rust / aider / opencode / Cursor 的「荣 HTTP_PROXY」与「自定义 BASE_URL 字段名」基于既有权威知识 + Go/Python/Rust 标准库默认行为推断，已标「推测」。建议 main 用 `octocode` `githubSearchCode` 核实：Codex repo（`openai/codex`）proxy 读取、aider `--openai-api-base` 文档、opencode `--base-url` flag。
- **aider/Cursor/opencode 的方案 A 配置 aidog 未自动化**（grep 无写入逻辑），用户需手动配。若要补自动配，是独立 task（非 P3 范畴）。
- **deep-research skill 已启动但未执行**（其依赖的 WebSearch sub-agent 工具本会话不可用）；改用 codebase 实证 + 标推测的方式给出结论。若 main 有外网工具，可补强 §3 矩阵的外部引用。
