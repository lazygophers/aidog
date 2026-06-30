# Research: Codex CLI config.toml 是否支持注入任意环境变量到会话进程

- **Query**: 验证 OpenAI Codex CLI 的 config.toml（含 profile 文件 `<name>.config.toml`）是否支持注入任意环境变量到 codex 会话进程。结论决定 aidog 分组 env_vars 的 Codex 侧实现路径。
- **Scope**: external（OpenAI 官方 Codex 仓库源码 + 官方文档站）
- **Date**: 2026-06-30
- **仓库基线**: `openai/codex` 分支 `main`（default_branch=main, primary language Rust）

## 结论行

**不支持。** Codex `config.toml` 与 `<name>.config.toml` profile 文件**均无原生能力注入任意环境变量到 codex 会话进程自身**。最接近的 `shell_environment_policy.set`（map<string,string>）只作用于 codex **派生的 shell 工具子进程**，不进入 codex 自身 runtime / MCP-server-spawn / model-provider-auth 环境。profile 文件还用 `deny_unknown_fields` 硬拒未知键，连扩展都不行。

**判定**：实现走 **`buildCodexCommand` 前置 `KEY=VALUE codex ...` export fallback**（`Groups.tsx:399`），profile TOML 不写 env。

## 证据

### 1. 官方 Config Reference（359 个 key 全量扫描）

URL: https://developers.openai.com/codex/config-reference （页面内嵌 schema JSON，359 个 config key）

全量 top-level key 段（359 键聚合）**无 `[env]` 表、无顶级 `env` 字段**。所有含 `env` 的 key 共 16 个，全部**作用于子作用域**：

| Key | Type | 作用域（非会话进程） |
|---|---|---|
| `shell_environment_policy.set` | `map<string,string>` | **shell 工具派生子进程**（见下方原文） |
| `shell_environment_policy.inherit` | `all\|core\|none` | 子进程环境基线继承 |
| `shell_environment_policy.exclude` / `include_only` / `ignore_default_excludes` / `experimental_use_profile` | — | 子进程环境过滤 |
| `mcp_servers.<id>.env` | `map<string,string>` | 单个 MCP server 子进程 |
| `mcp_servers.<id>.env_vars` / `bearer_token_env_var` / `env_http_headers` / `experimental_environment` | — | MCP server 子进程 |
| `model_providers.<id>.env_key` | `string` | provider token 读取的变量**名**（不是设值） |
| `model_providers.<id>.env_key_instructions` / `env_http_headers` | — | provider HTTP 头填充 |
| `sandbox_workspace_write.exclude_tmpdir_env_var` | `boolean` | 沙盒写根排除 |
| `otel.environment` | `string` | OTel 事件标签（`dev` 等，非 OS env） |

**官方对 `shell_environment_policy.set` 的原文描述**（config-reference 页 schema）：
> "Explicit environment overrides injected into **every subprocess**."

明确是 "every subprocess"（每个子进程），不是会话进程本身。

### 2. Rust 源码：profile 结构体根本没 env 字段 + 硬拒未知键

URL: https://raw.githubusercontent.com/openai/codex/main/codex-rs/config/src/profile_toml.rs

`ConfigProfile` 结构体（`<name>.config.toml` 与 `[profiles.<name>]` 都反序列化成它），字段清单（profile_toml.rs:24-71）：

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]   // ← 硬拒未知键
pub struct ConfigProfile {
    pub model: Option<String>,
    pub service_tier: Option<String>,
    pub model_provider: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub sandbox_mode: Option<SandboxMode>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub plan_mode_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub model_catalog_json: Option<AbsolutePathBuf>,
    pub personality: Option<Personality>,
    pub chatgpt_base_url: Option<String>,
    pub model_instructions_file: Option<AbsolutePathBuf>,
    // (两个 deprecated: js_repl_node_path / js_repl_node_module_dirs，#[schemars(skip)])
    pub experimental_compact_prompt_file: Option<AbsolutePathBuf>,
    pub include_permissions_instructions: Option<bool>,
    pub include_apps_instructions: Option<bool>,
    pub include_collaboration_mode_instructions: Option<bool>,
    pub include_environment_context: Option<bool>,   // ← 名字唬人，实为「注入 <environment_context> 提示块」开关，与 OS env 无关
    pub experimental_use_unified_exec_tool: Option<bool>,
    pub tools: Option<ToolsToml>,
    pub web_search: Option<WebSearchMode>,
    pub analytics: Option<AnalyticsConfigToml>,
    pub tui: Option<ProfileTui>,
    pub windows: Option<WindowsToml>,
    pub features: Option<FeaturesToml>,
    pub oss_provider: Option<String>,
    // ← 无 env / shell_environment_policy 字段
}
```

**关键**：
- **无 `env` 字段**、**无 `shell_environment_policy` 字段**（policy 仅在顶层 `ConfigToml`，profile 不继承）
- `#[schemars(deny_unknown_fields)]` → 在 `<name>.config.toml` 里写 `[env]` / `env = {...}` / `shell_environment_policy = {...}` 会被 codex **报错拒绝**（profile 是 strict 解析）
- `include_environment_context` 名字像但其实是「是否把环境上下文写进发给模型的 system prompt」的开关，**不设任何 OS env**

### 3. Rust 源码：shell_environment_policy 只灌子进程，不灌会话

URL: https://raw.githubusercontent.com/openai/codex/main/codex-rs/protocol/src/config_types.rs:205-231

`ShellEnvironmentPolicy` 运行时结构体，**官方文档注释原文**（config_types.rs:205-210）：
```rust
/// Policy for building the `env` when spawning a process via shell-like tools.
///
/// Algorithm:
/// 1. Create an initial map based on the `inherit` policy.
/// 2. If `ignore_default_excludes` is false, filter ... default exclude pattern(s):
///    `"*KEY*"`, `"*SECRET*"`, and `"*TOKEN*"`.
/// 3. If `exclude` is not empty, filter the map using the provided patterns.
/// 4. Insert any entries from `r#set` into the map.
/// 5. If non-empty, filter the map using the `include_only` patterns.
pub struct ShellEnvironmentPolicy { ... }
```

URL: https://raw.githubusercontent.com/openai/codex/main/codex-rs/core/src/exec_env.rs:15-31
```rust
/// Construct an environment map based on the rules in the specified policy. The
/// resulting map can be passed directly to `Command::envs()` after calling
/// `env_clear()` to ensure no unintended variables are leaked to the spawned
/// process.
pub fn create_env(policy: &ShellEnvironmentPolicy, thread_id: Option<ThreadId>) -> HashMap<String, String> { ... }
```

URL: https://raw.githubusercontent.com/openai/codex/main/codex-rs/core/src/spawn.rs:51-76
```rust
pub struct SpawnChildRequest<'a> {
    ...
    pub env: HashMap<String, String>,   // ← 上游已构造好的 env
}

pub(crate) async fn spawn_child_async(request: SpawnChildRequest<'_>) -> std::io::Result<Child> {
    ...
    let mut cmd = Command::new(&program);
    ...
    cmd.env_clear();   // ← 清空
    cmd.envs(env);     // ← 只灌入 policy 构造的 env
    ...
}
```

**结论**：`shell_environment_policy.set` 的值经 `create_env` 构造成 HashMap，经 `spawn_child_async` 的 `env_clear() + envs()` **只灌进 codex 派生的 shell 子进程**。codex 自身 `std::env` 不会被 config.toml 改写。MCP server spawn / model provider env_key 读取走的是 codex 自身进程 env，**不经此 policy**。

### 4. Profile 不继承 shell_environment_policy（双重确认）

URL: https://raw.githubusercontent.com/openai/codex/main/codex-rs/config/src/config_toml.rs:185-186
```rust
#[serde(default)]
pub shell_environment_policy: ShellEnvironmentPolicyToml,   // ← 仅顶层 ConfigToml 字段
```

`ConfigProfile`（profile_toml.rs）无此字段 → 即使能接受，也只能写在用户级 `config.toml` 根，**不能写在 `<group>.config.toml` profile 文件里**（profile strict 解析会拒）。这把「在 group profile 里写 policy」的路也堵死了。

## Fallback 方案确认（前置 export 可行，不被覆盖）

URL: https://raw.githubusercontent.com/openai/codex/main/codex-rs/cli/src/main.rs:2220-2231
```rust
let auth_token = get_var(env_var_name)
    .map_err(|_| anyhow::anyhow!("environment variable `{env_var_name}` is not set"))?;
...
fn read_remote_auth_token_from_env_var(env_var_name: &str) -> anyhow::Result<String> {
    read_remote_auth_token_from_env_var_with(env_var_name, |name| std::env::var(name))
}
```

**结论**：codex 读 `env_key` 指向的变量走 `std::env::var(name)` —— 标准 Rust 进程继承调用方环境。codex 自身**从不写入/覆盖自身进程 env**（所有 env 改写都在 `spawn_child_async` 里作用于子进程 Command）。

→ **`KEY=VALUE codex -p <group> ...` 前置 export 100% 可行，且不会被 codex 内部覆盖**。这正是 aidog 当前 `buildCodexCommand`（`Groups.tsx:399`）已用 `AIDOG_KEY=${groupKey}` 的同款机制。

## 保护字段清单（aidog 当前在 codex 侧强写/依赖的 env key）

来源：`src-tauri/src/gateway/codex.rs:54-88` + `src/pages/Groups.tsx:399-410`

aidog 在 codex profile 强写的字段（profile TOML 内）：
| 字段 | 值 | 用户 env_vars 同名须跳过？ |
|---|---|---|
| `model_provider` | `"aidog"` | 非 env key，但同语义保护（禁覆盖） |
| `[model_providers.aidog].base_url` | `http://127.0.0.1:{port}/proxy` | 非 env key |
| `[model_providers.aidog].wire_api` | `"responses"` | 非 env key |
| `[model_providers.aidog].env_key` | `"AIDOG_KEY"` | **是** — 用户若注入 `AIDOG_KEY` 会冲掉路由 token |

aidog 在 `buildCodexCommand` 前置的 env（用户 env_vars 同名**必须跳过**，否则破坏代理路由）：
| env key | 值 | 用途 |
|---|---|---|
| `AIDOG_KEY` | `<group_key>`（分组名，作 Authorization Bearer token） | aidog 代理路由依据 |

**实现须过滤**：用户 `env_vars` 中 key == `AIDOG_KEY` → 丢弃 + 后端 warn 日志（与 Claude 侧 `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN` 同等级保护）。当前 `buildCodexCommand` 已硬写 `AIDOG_KEY=${groupKey}`，用户变量须在其后追加，**不得重复前置同名 key**（shell 后者覆盖前者，会破坏路由）。

## 实现判定（可执行）

**Codex 侧走 `buildCodexCommand` 前置 export fallback**（不写 config.toml）：

1. `src/pages/Groups.tsx::buildCodexCommand(groupKey)` 签名扩展为接收 `env_vars: EnvVar[]`
2. 过滤掉 key == `"AIDOG_KEY"`（保护字段，warn 由后端 create/update_group 落）
3. 在现有 `AIDOG_KEY=... codex ...` 前 / 后追加用户变量，每条 `KEY=VALUE` 形式
4. value 含特殊字符 → 复用 `shellSquote`（PRD 已点名，line 86 风险项）
5. 后端 `codex.rs::build_group_profile_toml` / `write_group_profile` **保持原签名不动**（不接 env_vars，profile 不支持）

## 外部参考（URL 清单，查过的全列）

- https://developers.openai.com/codex/config-reference — 官方 Config Reference（359 key schema，本结论主证据）
- https://developers.openai.com/codex/config-basic — 基础配置（指向页）
- https://developers.openai.com/codex/config-advanced — 高级配置（指向页）
- https://developers.openai.com/codex/config-sample — 示例配置（指向页）
- https://developers.openai.com/codex/auth — 鉴权（指向页）
- https://github.com/openai/codex（default_branch=main, Rust）— 源码仓库
- `codex-rs/config/src/profile_toml.rs` — ConfigProfile 结构体（deny_unknown_fields）
- `codex-rs/config/src/config_toml.rs:185-186` — shell_environment_policy 仅顶层字段
- `codex-rs/config/src/types.rs:924-941` — ShellEnvironmentPolicyToml 结构体 + 文档注释
- `codex-rs/protocol/src/config_types.rs:205-244` — ShellEnvironmentPolicy 运行时 + 算法注释
- `codex-rs/core/src/exec_env.rs:15-31` — create_env 文档（Command::envs() + env_clear）
- `codex-rs/core/src/spawn.rs:51-76` — spawn_child_async（env_clear + envs）
- `codex-rs/cli/src/main.rs:2220-2231` — env_key 经 std::env::var 读取（继承调用方）

## Caveats / Not Found

- **`shell_environment_policy.set` 语义边界**：能注入到 codex 跑 shell 命令的子进程，若用户的 env_vars 目标是「我跑 `codex exec` 时，让 codex 调用的工具脚本读到 MY_VAR」—— 那其实可以走 `shell_environment_policy.set`（写在**用户级** `config.toml`，非 profile）。但 aidog 的需求是「分组维度」注入，而 policy 不能进 profile 文件 → 仍然走不了分组隔离，**结论不变**：fallback export。
- **profile 文件层叠语义**：`codex -p <name>` 加载 `$CODEX_HOME/<name>.config.toml` 层叠在用户 `config.toml` 之上（aidog 当前用法，codex.rs:41-44 注释）。profile 的 strict 解析 + deny_unknown_fields 决定它不能扩展未知 key。
- 未实测 codex 二进制跑一遍验 `deny_unknown_fields` 报错信息（源码层面已确证，`#[schemars(deny_unknown_fields)]` + serde 反序列化对未知字段报错是 schemars 标准行为）。
