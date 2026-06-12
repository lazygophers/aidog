# Research: OpenAI Codex CLI 配置体系

- **Query**: 抽取 Codex CLI 完整 config schema + 推荐默认，供 aidog 设置页加「Codex 设置」
- **Scope**: external（5 份官方文档 WebFetch）+ internal（aidog Claude Code 配置生成对照）
- **Date**: 2026-06-12
- **来源**（全部 HTTP 200，已抓取）:
  - https://developers.openai.com/codex/config-basic
  - https://developers.openai.com/codex/config-advanced
  - https://developers.openai.com/codex/config-reference （权威全量字段表，~250 keys）
  - https://developers.openai.com/codex/environment-variables
  - https://developers.openai.com/codex/config-sample
- **可信度标注**: 下文除明确标「推测」处，均为官方文档直引。`config.toml` 字段类型/默认值取自 config-reference 与 config-sample，对后端 TOML 解析准确。

---

## 1. 配置文件：路径、格式、加载机制、层级

### 文件位置（官方明确）

| 层级 | 路径 | 说明 |
|---|---|---|
| 用户级（主） | `~/.codex/config.toml` | 个人默认值。根由 `CODEX_HOME` 决定（默认 `~/.codex`） |
| 项目级 | `<repo>/.codex/config.toml` | 项目/子目录覆盖。**仅当项目被 trust 时才加载** |
| Profile | `$CODEX_HOME/<profile-name>.config.toml` | 命名层，`--profile <name>` 选择（注意命名：`<name>.config.toml`，非子目录） |
| 系统级 | `/etc/codex/config.toml`（Unix） | 若存在 |
| 管理强制 | `requirements.toml` | 管理员强制约束（见 §8） |

- **格式**: TOML。**TOML 硬约束**：根级（top-level）键必须出现在所有 table（`[xxx]`）之前，否则解析报错（config-sample 注释明确：`Root keys must appear before tables in TOML`）。
- **CODEX_HOME** 若手动设置，目录必须已存在（Codex 不会创建）。

### 配置优先级（高 → 低，config-basic 明确）

1. CLI flags 和 `-c/--config` 覆盖
2. 项目 config：`.codex/config.toml`（从 repo root 向 cwd 走，越近越优先；仅 trusted 项目）
3. Profile 文件 `--profile <name>` → `~/.codex/<name>.config.toml`
4. 用户 config `~/.codex/config.toml`
5. 系统 config `/etc/codex/config.toml`
6. 内置默认

### 项目级 config 不能覆盖的键（关键，影响 aidog 集成路径选择）

项目本地 `.codex/config.toml` **会被忽略并打印 startup warning** 的键：
`openai_base_url`、`chatgpt_base_url`、`apps_mcp_product_sku`、`model_provider`、`model_providers`、`notify`、`profile`、`profiles`、`experimental_realtime_ws_base_url`、`otel`。

**含义对 aidog**：provider 重定向（`model_providers` / `model_provider` / `openai_base_url`）**只能写在用户级 `~/.codex/config.toml` 或 profile 文件**，不能靠生成项目级 `.codex/config.toml` 来切 provider。这直接决定了 aidog 集成形态（见 §7）。

### Profile 机制（config-advanced 明确）

- `--profile deep-review` → 加载 `~/.codex/config.toml` 然后叠加 `~/.codex/deep-review.config.toml`。
- Profile 文件用**顶层键**，**不要**嵌套在 `[profiles.<name>]` 下。
- Profile 名允许：字母、数字、连字符、下划线。
- **版本注意**：Codex **0.134.0+** 起，`--profile` 不再读 `config.toml` 里的 `[profiles.<name>]`，顶层 `profile = "..."` 选择器也废弃。旧写法须迁移到独立 profile 文件。
- Profile 层位于「用户 config 之上、项目/CLI config 之下」，只需写与基线不同的值。

---

## 2. 完整 config 字段树（按分组）

> 标注：**[B]**=config-basic 出现（基础常用）；**[A]**=config-advanced；其余=config-reference 全量表。「默认」列取自 reference/sample。

### 2.1 Core Model（模型选择）

| Key | 类型 / 可选值 | 默认 | 含义 |
|---|---|---|---|
| `model` **[B]** | string | `gpt-5.5`（sample 推荐） | 默认模型 |
| `model_provider` | string | `openai` | 从 `[model_providers]` 选 provider id |
| `oss_provider` | `lmstudio`\|`ollama` | unset（提示） | `--oss` 默认本地 provider |
| `service_tier` | string（`flex`\|`fast`+catalog） | unset | 服务等级 |
| `review_model` | string | unset（用会话模型） | `/review` 覆盖模型 |
| `model_context_window` | number | auto | 上下文窗口 token |
| `model_auto_compact_token_limit` | number | model 默认 | 触发自动压缩的 token 阈值 |
| `tool_output_token_limit` | number | — | 单条 tool 输出存 history 的 token 预算 |
| `model_catalog_json` | string(path) | unset | 启动时加载的 JSON model catalog |

### 2.2 Reasoning & Verbosity（Responses API 模型）

| Key | 类型 / 可选值 | 默认 | 含义 |
|---|---|---|---|
| `model_reasoning_effort` **[B]** | `minimal`\|`low`\|`medium`\|`high`\|`xhigh` | model 默认 | 推理强度（仅 Responses API；xhigh 取决于模型） |
| `plan_mode_reasoning_effort` | `none`\|`minimal`\|`low`\|`medium`\|`high`\|`xhigh` | 预设默认 | plan mode 专用推理强度 |
| `model_reasoning_summary` | `auto`\|`concise`\|`detailed`\|`none` | — | 推理摘要详细度 |
| `model_verbosity` | `low`\|`medium`\|`high` | 模型/预设默认 | GPT-5 Responses 文本详细度 |
| `model_supports_reasoning_summaries` | boolean | — | 强制开/关推理元数据 |

### 2.3 Approval & Sandbox（审批与沙箱）

| Key | 类型 / 可选值 | 默认 | 含义 |
|---|---|---|---|
| `approval_policy` **[B]** | `untrusted`\|`on-request`\|`never`\|`{ granular = {...} }` | `on-request`（sample） | 何时暂停请求审批。`on-failure` 已废弃 |
| `approval_policy.granular.{sandbox_approval, rules, mcp_elicitations, request_permissions, skill_approval}` | bool | — | 按类放行/自动拒绝特定审批提示 |
| `approvals_reviewer` | `user`\|`auto_review` | `user` | 谁审批（auto_review 用 reviewer subagent） |
| `sandbox_mode` **[B]** | `read-only`\|`workspace-write`\|`danger-full-access` | `read-only` | 文件/网络沙箱策略 |
| `allow_login_shell` | boolean | `true` | 允许 login-shell 语义 |
| `default_permissions` **[B]** | string（`:read-only`\|`:workspace`\|`:danger-full-access`\|自定义名） | unset | 命名权限 profile。**不要与 `sandbox_mode`/`[sandbox_workspace_write]` 混用** |
| `[sandbox_workspace_write].writable_roots` | array<string> | `[]` | workspace-write 模式额外可写根 |
| `[sandbox_workspace_write].network_access` | boolean | `false` | 沙箱内允许出站网络 |
| `[sandbox_workspace_write].exclude_tmpdir_env_var` | boolean | `false` | 排除 `$TMPDIR` |
| `[sandbox_workspace_write].exclude_slash_tmp` | boolean | `false` | 排除 `/tmp` |

> Permission profiles（`[permissions.<name>]`）是 beta 高级特性，含 `.filesystem`、`.network.*`、`.workspace_roots`、`.extends`、`.description` 等子键（详见 reference）。aidog MVP 可不暴露。

### 2.4 Model Providers（自定义 provider，**aidog 核心**，详见 §5）

`[model_providers.<id>]` 子键（reserved id：`openai`/`ollama`/`lmstudio`/`amazon-bedrock` 不可覆盖）：

| Key | 类型 / 可选值 | 默认 | 含义 |
|---|---|---|---|
| `name` | string | — | 显示名 |
| `base_url` | string | — | API base URL |
| `wire_api` | `responses` | `responses` | **协议；responses 是唯一支持值** |
| `env_key` | string | — | 提供 API key 的环境变量名 |
| `env_key_instructions` | string | — | key 设置指引 |
| `experimental_bearer_token` | string | — | 直接 bearer token（不推荐，用 env_key） |
| `requires_openai_auth` | boolean | `false` | 用 OpenAI auth |
| `http_headers` | map<string,string> | — | 静态 HTTP headers |
| `env_http_headers` | map<string,string> | — | 由环境变量填充的 headers |
| `query_params` | map<string,string> | — | 额外 query 参数（如 Azure api-version） |
| `request_max_retries` | number | `4`（max 100） | HTTP 重试次数 |
| `stream_max_retries` | number | `5`（max 100） | SSE 流重试次数 |
| `stream_idle_timeout_ms` | number | `300000` | SSE 空闲超时 |
| `supports_websockets` | boolean | — | 是否支持 Responses WS 传输 |
| `[model_providers.<id>.auth]` | table | — | 命令式 bearer token（`command`/`args`/`cwd`/`timeout_ms`(5000)/`refresh_interval_ms`(300000)）。**不可与 env_key/experimental_bearer_token/requires_openai_auth 同用** |
| `[model_providers.amazon-bedrock.aws].{profile, region}` | string | — | 内置 bedrock 选项 |

相关顶层键：
| Key | 类型 | 含义 |
|---|---|---|
| `openai_base_url` **[A]** | string | 内置 openai provider 的 base URL 覆盖（**指向代理的轻量方案**，无需建新 provider） |
| `chatgpt_base_url` | string | ChatGPT 登录流 base URL |

### 2.5 MCP Servers（`[mcp_servers.<id>]`）

stdio：`command`(必)、`args`、`env`(map)、`env_vars`(array)、`cwd`、`experimental_environment`(`local`\|`remote`)、`startup_timeout_sec`(10)/`startup_timeout_ms`、`tool_timeout_sec`(60)。
HTTP：`url`(必)、`bearer_token_env_var`、`http_headers`、`env_http_headers`、`oauth_resource`、`scopes`。
通用：`enabled`(true)、`required`、`enabled_tools`/`disabled_tools`、`default_tools_approval_mode`(`auto`\|`prompt`\|`approve`)、`tools.<tool>.approval_mode`。
OAuth 全局：`mcp_oauth_credentials_store`(`auto`\|`file`\|`keyring`)、`mcp_oauth_callback_port`、`mcp_oauth_callback_url`。

### 2.6 TUI（`[tui]`）

| Key | 类型/值 | 默认 | 含义 |
|---|---|---|---|
| `notifications` | boolean \| array<string> | `true` | 桌面通知（可按事件类型过滤） |
| `notification_method` | `auto`\|`osc9`\|`bel` | `auto` | 通知机制 |
| `notification_condition` | `unfocused`\|`always` | `unfocused` | 触发条件 |
| `animations` | boolean | `true` | 动画 |
| `show_tooltips` | boolean | `true` | 欢迎屏 tooltip |
| `alternate_screen` | `auto`\|`always`\|`never` | `auto` | alt screen（auto 在 Zellij 跳过） |
| `theme` | string(kebab-case) | — | 语法高亮主题 |
| `vim_mode_default` | boolean | `false` | 默认 Vim normal |
| `raw_output_mode` | boolean | `false` | raw scrollback |
| `status_line` | array<string> \| null | `["model-with-reasoning","context-remaining","current-dir"]` | 底栏项；`[]` 隐藏 |
| `terminal_title` | array<string> \| null | `["spinner","project"]` | 终端标题项 |
| `keymap.<context>.<action>` **[B]** | string \| array<string> | — | 键位（context：global/chat/composer/editor/vim_*/pager/list/approval）；`[]` 解绑 |

### 2.7 Features（`[features]`，基础页有表，maturity 标注）

| Key | 默认 | Maturity | 含义 |
|---|---|---|---|
| `apps` | false | Experimental | ChatGPT Apps/connectors |
| `codex_git_commit` | false | Experimental | Codex 生成 git commit + 署名 |
| `hooks` | true | Stable | 生命周期 hooks |
| `fast_mode` | true | Stable | Fast 模式 / `service_tier="fast"` |
| `memories` | false | Stable | Memories |
| `multi_agent` | true | Stable | subagent 协作工具 |
| `personality` | true | Stable | personality 选择 |
| `shell_snapshot` | true | Stable | shell 环境快照加速 |
| `shell_tool` | true | Stable | 默认 shell 工具 |
| `unified_exec` | true（Windows 除外） | Stable | 统一 PTY exec |
| `undo` | false | Stable | per-turn git ghost 快照 undo |
| `network_proxy` | false | Experimental | 沙箱网络（table 形态含 `domains`/`proxy_url`(127.0.0.1:3128)/`socks_url`(127.0.0.1:8081)/`enable_socks5` 等） |
| `enable_request_compression` | true | Stable | zstd 压缩请求体 |
| `skill_mcp_dependency_install` | true | Stable | 安装缺失 MCP 依赖 |
| `prevent_idle_sleep` | false | Experimental | 运行时防休眠 |
| `web_search`/`web_search_cached`/`web_search_request` | — | Deprecated | 旧 toggle，改用顶层 `web_search` |

开启方式：`[features]` 下 `key = true`，或 CLI `codex --enable <feature>`。

### 2.8 Web Search（顶层）

| Key | 值 | 默认 | 含义 |
|---|---|---|---|
| `web_search` **[B]** | `disabled`\|`cached`\|`live` | `cached` | cached=OpenAI 预索引（抗注入）；live=实时（=`--search`）；`--yolo`/full-access 时默认 live |
| `tools.web_search` | bool \| object（context_size/allowed_domains/location） | — | web search 工具配置 |
| `personality` **[B]** | `none`\|`friendly`\|`pragmatic` | — | 沟通风格 |

### 2.9 其余分组（高级/管理，aidog MVP 可选）

- **Agents**（`[agents]`）：`max_threads`(6)、`max_depth`(1)、`job_max_runtime_seconds`(1800)、`<name>.{config_file,description,nickname_candidates}`。
- **Memories**（`[memories]`）：`generate_memories`、`use_memories`、`disable_on_external_context` 等 ~12 个调参键。
- **Shell env policy**（`[shell_environment_policy]`）**[B]**：`inherit`(all/core/none)、`include_only`、`exclude`、`set`、`ignore_default_excludes`、`experimental_use_profile`。
- **History**（`[history]`）：`persistence`(save-all/none)、`max_bytes`。
- **OTEL**（`[otel]`）：exporter/trace_exporter/metrics_exporter + endpoint/headers/tls。
- **Analytics/Feedback/Notice**：`[analytics].enabled`、`[feedback].enabled`、`[notice].*`。
- **Skills**（`[[skills.config]]`）：`path` + `enabled`。
- **Projects**（`[projects."<path>"].trust_level` = `trusted`\|`untrusted`）。
- **Hooks**（`[hooks]` 内联或 `hooks.json`）：事件 PreToolUse/PostToolUse/PermissionRequest/PreCompact/SessionStart/Stop 等。
- **Windows**（`[windows].sandbox` = `unelevated`\|`elevated`，推荐 elevated）**[B]**。
- **Misc 顶层**：`file_opener`(vscode 默认)、`log_dir`(`$CODEX_HOME/log`)、`sqlite_home`、`cli_auth_credentials_store`(file/keyring/auto)、`forced_login_method`(chatgpt/api)、`check_for_update_on_startup`(true)、`hide_agent_reasoning`、`show_raw_agent_reasoning`、`disable_paste_burst`、`project_doc_max_bytes`(32768)、`project_doc_fallback_filenames`、`project_root_markers`([".git"])、`developer_instructions`、`model_instructions_file`、`compact_prompt`、`commit_attribution`、`notify`(argv array)、`suppress_unstable_features_warning`。

---

## 3. 环境变量（environment-variables 全量）

> Codex 偏好 config.toml 持久化；env var 用于 shell 范围覆盖/自动化密钥/安装器/诊断。**provider-specific 密钥变量名由你自己在 `env_key` 里定，不是固定 Codex env var。**

| 变量 | 用于 | 默认 | 含义 |
|---|---|---|---|
| `CODEX_HOME` | CLI/IDE/app-server/installer | `~/.codex` | Codex 状态根（config/auth/logs/sessions/skills）。**设置则目录须已存在** |
| `CODEX_SQLITE_HOME` | CLI/app-server state | `CODEX_HOME` | SQLite 状态目录（`sqlite_home` config 优先） |
| `CODEX_NON_INTERACTIVE` | install 脚本 | `false` | `1`/`true`/`yes` 跳过安装器提示 |
| `CODEX_INSTALL_DIR` | install 脚本 | `~/.local/bin`（mac/Linux）/ `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin`（Win） | codex 命令安装目录 |
| `CODEX_API_KEY` | `codex exec` | — | 单次非交互运行的 API key（**仅 codex exec 支持**） |
| `CODEX_ACCESS_TOKEN` | CLI/app-server/自动化 | — | ChatGPT/Codex access token（持久化用 `codex login --with-access-token`） |
| `CODEX_CA_CERTIFICATE` | HTTPS/login/WS | — | PEM CA bundle（企业 TLS 拦截）；优先于 SSL_CERT_FILE |
| `SSL_CERT_FILE` | HTTPS/login/WS | — | CA bundle 回退路径 |
| `RUST_LOG` | CLI/app-server | error（exec） | Rust 日志过滤，如 `codex_core=debug,codex_tui=debug` |

**与 config 关系**：env var 多为 shell 范围覆盖；provider API key 走 config `env_key` 命名的变量（Codex 读那个变量，变量名非固定）。`CODEX_API_KEY` 仅 `codex exec`。

---

## 4. config-sample 官方完整内容

官方 `config-sample` 是一份带注释的全量 `config.toml`（含大多数键 + 默认 + 推荐）。**关键节选已逐字保存**，原文结构分区：Core Model Selection → Reasoning & Verbosity → Instruction Overrides → Notifications → Approval & Sandbox → Authentication & Login → Project Doc → History & File Opener → UI/Misc → Web Search → `[agents]` → `[[skills.config]]` → `[sandbox_workspace_write]` → `[shell_environment_policy]` → 沙箱网络注释 → `[history]` → `[tui]` → `[analytics]`/`[feedback]`/`[notice]` → `[features]` → `[memories]` → `[hooks]` → `[mcp_servers]` → `[model_providers]` → `[apps]` → Config Profiles → `[projects]` → `[tools]` → `[otel]` → `[windows]`。

sample 顶部硬规注释（原文）：
```
# Notes
# - Root keys must appear before tables in TOML.
# - Optional keys that default to "unset" are shown commented out with notes.
# - MCP servers, profile files, and model providers are examples; remove or edit.
```

**model_providers 官方示例（逐字，4 种形态）**：
```toml
# --- OpenAI data residency (explicit base URL / headers) ---
# [model_providers.openaidr]
# name = "OpenAI Data Residency"
# base_url = "https://us.api.openai.com/v1"
# wire_api = "responses"   # only supported value
# # requires_openai_auth = true
# # request_max_retries = 4
# # http_headers = { "X-Example" = "value" }
# # env_http_headers = { "OpenAI-Organization" = "OPENAI_ORGANIZATION" }

# --- Azure / OpenAI-compatible ---
# [model_providers.azure]
# name = "Azure"
# base_url = "https://YOUR_PROJECT_NAME.openai.azure.com/openai"
# wire_api = "responses"
# query_params = { api-version = "2025-04-01-preview" }
# env_key = "AZURE_OPENAI_API_KEY"
# env_key_instructions = "Set AZURE_OPENAI_API_KEY in your environment"

# --- Command-backed bearer token (LLM proxy) ---
# [model_providers.proxy]
# name = "OpenAI using LLM proxy"
# base_url = "https://proxy.example.com/v1"
# wire_api = "responses"
# [model_providers.proxy.auth]
# command = "/usr/local/bin/fetch-codex-token"
# args = ["--audience", "codex"]
# timeout_ms = 5000
# refresh_interval_ms = 300000

# --- Local OSS (Ollama-compatible) ---
# [model_providers.local_ollama]
# name = "Ollama"
# base_url = "http://localhost:11434/v1"
# wire_api = "responses"
```

---

## 5. 自定义 model provider（aidog 代理接入核心）

### 怎么配自定义 provider 指向本地代理

两种路径：

**路径 A — 自定义 provider（推荐，可命名 + 多分组）**：
```toml
model_provider = "aidog"
model = "gpt-5.5"

[model_providers.aidog]
name = "aidog proxy"
base_url = "http://127.0.0.1:<port>/proxy"   # aidog 本地代理端点
wire_api = "responses"                         # 唯一支持值
env_key = "AIDOG_KEY"                          # 指向存 token 的环境变量名
```
> Codex 发请求时 = `base_url` + Responses API 路径（`/responses`）。env_key 命名的环境变量提供 bearer token。

**路径 B — 仅改内置 openai provider 的 base URL（最轻量）**：
```toml
openai_base_url = "http://127.0.0.1:<port>/proxy"
```
config-advanced 原文：「If you just need to point the built-in OpenAI provider at an LLM proxy/router/data-residency project, set `openai_base_url`」。但仍用 OpenAI 内置 auth（ChatGPT/API key），鉴权不灵活。

### 对 aidog 的关键约束（务必注意）

1. **`wire_api` 只支持 `responses`** —— Codex CLI 对自定义 provider 走 **OpenAI Responses API**（POST `<base_url>/responses`），**不是 `/chat/completions`**。
   - aidog proxy 已能识别入站 `/v1/responses`（`detect_source_protocol` 在 `src-tauri/src/gateway/proxy.rs:1346` 把 `/v1/responses` 归为 openai 协议），且有 `Protocol::OpenAIResponses`（`models.rs:11-12`）。aidog 后端需确认对入站 Responses 请求的转换/转发完整。
2. **`model_providers` 不能写项目级 `.codex/config.toml`** —— 被忽略 + warning。aidog 必须写**用户级 `~/.codex/config.toml`** 或 **profile 文件**。
3. **鉴权**：用 `env_key`（指向环境变量名），或 `[model_providers.<id>.auth]` 命令式取 token，或 `experimental_bearer_token`（不推荐）。aidog 现为 Claude Code 用「group name 作 token」（`ANTHROPIC_AUTH_TOKEN=group_name`），Codex 侧等价做法是把 group name 作为 `env_key` 指向的环境变量值，或用 auth command。

---

## 6. 推荐默认（aidog 版 RECOMMENDED Codex config）

> 类比 aidog 给 Claude Code 的 `defaults/settings.json`。下表给「合理默认 + 理由」。带 **[代理]** 的与「指向 aidog」直接相关。

```toml
# ~/.codex/config.toml （或 profile 文件 ~/.codex/<group>.config.toml）

# ── Core ──
model = "gpt-5.5"                       # 官方 sample 推荐
model_provider = "aidog"                # [代理] 指向自定义 provider
model_reasoning_effort = "medium"       # 平衡速度/质量（与 aidog Claude 默认 EFFORT_LEVEL=medium 一致）

# ── Approval & Sandbox ──
approval_policy = "on-request"          # 官方默认，交互式安全
sandbox_mode = "workspace-write"        # 比默认 read-only 实用：可写工作区，仍隔离系统
# 注：若要无审批全自动，approval_policy="never" + sandbox_mode 谨慎；对应 aidog Claude 的 bypassPermissions

[sandbox_workspace_write]
network_access = false                  # 默认不放开沙箱出站网络（安全）

# ── Web search ──
web_search = "cached"                   # 官方默认，抗 prompt injection

# ── UI ──
personality = "pragmatic"               # 简洁
# file_opener = "vscode"                # 默认即 vscode

# ── 诊断（可选，对齐 aidog 隐私偏好）──
[analytics]
enabled = false                         # [对齐 aidog DO_NOT_TRACK 偏好] 关分析

# ── Provider 指向 aidog 代理 ──
[model_providers.aidog]                 # [代理]
name = "aidog proxy"
base_url = "http://127.0.0.1:<PORT>/proxy"   # PORT = aidog proxy 端口（注入）
wire_api = "responses"
env_key = "AIDOG_KEY"                   # 环境变量提供 token（值 = 分组名 或 平台 api_key）
```

理由要点：
- `model_provider=aidog` + `[model_providers.aidog]`：把 Codex 全部上游流量导向 aidog 本地代理，复用 aidog 的分组路由/统计/日志。**[代理核心]**
- `base_url` 末尾建议带 `/proxy`（对齐 aidog Claude 用的 `http://127.0.0.1:{port}/proxy`），group 路由靠 path 前缀或 token。
- `wire_api="responses"` 必填（唯一值），且决定 aidog 必须支持入站 Responses。
- `analytics.enabled=false` 对齐 aidog 全局隐私偏好（CLAUDE.md 里 `DO_NOT_TRACK=1` / 关 telemetry）。
- `sandbox_mode` 选 `workspace-write` 而非默认 `read-only`：让 Codex 实际能改代码，又不放系统全权（比 `danger-full-access` 安全）。**推测**：具体取舍可让 aidog 用户在设置页选。

---

## 7. aidog 集成点（对照现有 Claude Code 生成模式）

### aidog 现状（Claude Code）

- `do_sync_group_settings`（`src-tauri/src/lib.rs:759`）：遍历所有分组，为每个分组生成 `~/.aidog/settings.{group}.json`。
- 注入 `env.ANTHROPIC_BASE_URL = http://127.0.0.1:{port}/proxy`、`env.ANTHROPIC_AUTH_TOKEN = {group_name}`；单平台分组额外注入 `AIDOG_INFO_URL`/`AIDOG_GROUP`/`AIDOG_KEY`。
- base 配置来自 DB（`global/claude_code` setting）或编译内置 `defaults/settings.json`。
- 删除分组时清理对应文件。

### Codex 是否支持等价的 per-profile/per-group 配置？

**支持，且 profile 机制天然对应 aidog 分组**：

| aidog Claude Code | Codex CLI 等价 |
|---|---|
| `~/.aidog/settings.{group}.json` | `~/.codex/{group}.config.toml`（profile 文件） |
| 用户用 `--settings <file>` 或 env 指定 | `codex --profile {group}` 选择 |
| `ANTHROPIC_BASE_URL → /proxy` | `[model_providers.aidog].base_url → /proxy`（或 `openai_base_url`） |
| `ANTHROPIC_AUTH_TOKEN = group_name` | `env_key` 指向的环境变量值 = group_name，或 `[model_providers.<id>.auth]` command |
| base 模板 `defaults/settings.json` | 内置 `defaults/config.toml` 模板 |

**推荐集成形态（结合「项目级不能写 model_providers」约束）**：
- aidog 为每个分组生成 **profile 文件** `~/.codex/{group}.config.toml`（顶层键，含 `model_provider` + `[model_providers.aidog]`）。注意 profile 文件**也是用户级**，不受「项目级忽略 provider」限制 —— **推测**：profile 文件作为「config.toml 之上的用户层」应允许 provider 键（文档未显式说 profile 文件禁 provider，仅说项目级 `.codex/` 禁）。**需验证**：建议实测 `~/.codex/<name>.config.toml` 里写 `[model_providers]` 是否生效。
- 或维护单一 `~/.codex/config.toml`，把 provider 定义放基线，用 profile 仅覆盖 `model`/`model_reasoning_effort` 等差异。
- 区别于 Claude Code：Codex profile 切换是 `--profile <group>` CLI 参数（用户手动），不像 Claude 那样靠 env/settings 文件路径。aidog 设置页应提示用户 `codex --profile <group>` 用法。

### TOML 生成注意（影响后端实现）

- 用 TOML 库（Rust `toml` crate）而非 JSON，root 键必须在 table 之前。
- profile 文件命名严格：`<name>.config.toml`（非 `<name>/config.toml`，非纯 `<name>.toml`）。
- provider id reserved：`openai`/`ollama`/`lmstudio`/`amazon-bedrock` 不可用作自定义 id（用 `aidog` 等）。

---

## 8. requirements.toml（管理强制层，了解即可）

管理员强制约束安全敏感设置，键如 `allowed_approval_policies`、`allowed_sandbox_modes`、`allowed_web_search_modes`、`allowed_permission_profiles`、`allow_managed_hooks_only`、`rules.prefix_rules[]`、`windows.allowed_sandbox_implementations`。aidog 个人场景一般不涉及。

---

## Caveats / Not Found

- **Codex 版本流动**：文档示例模型 `gpt-5.5`、profile 机制在 0.134.0 有破坏性变更（弃 `[profiles]`）。aidog 生成的 config 须适配较新 Codex，旧版本用户可能不兼容。
- **profile 文件能否含 `model_providers`**：文档明确「项目级 `.codex/config.toml` 忽略 provider 键」，但**未显式说 profile 文件（`~/.codex/<name>.config.toml`）是否允许 provider 键**。profile 是用户级层，**推测允许**，但建议后端实现前实测一次（影响 per-group provider 方案可行性）。
- **`base_url` 末尾路径**：Codex 自定义 provider 用 Responses API（`/responses`），但文档未逐字说明它如何拼接 `base_url`（是否自动加 `/responses`）。sample 里 OpenAI 兼容 base_url 多带 `/v1`（如 `http://localhost:11434/v1`）。**需验证** aidog 代理端点该写成 `/proxy` 还是 `/proxy/v1`，以及 Codex 实际 POST 的完整路径。
- **aidog 入站 Responses 完整度**：aidog 有 `Protocol::OpenAIResponses` 且 `detect_source_protocol` 识别 `/v1/responses`，但本次未深入核对 converter.rs 对 Responses 请求体的双向转换完整度 —— 集成前应确认 aidog 能正确代理 Responses 流量。
