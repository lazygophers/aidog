# PRD — MCP 管理模块 (06-14-mcp-management)

## 背景

aidog 已有 Skills 管理模块（per-agent 启用切换 + 统一列表），现新增 **MCP 管理模块**，对称地管理 Claude Code / Codex 的 MCP server 配置。MCP（Model Context Protocol）server 配置当前散落在各 agent 配置文件中（`~/.claude.json` mcpServers / `~/.codex/config.toml` [mcp_servers.*]），无集中管理入口。

## 目标

用户通过 aidog GUI：
1. **per-agent 启用/禁用** 某 MCP（控制该 MCP 是否写入某 agent 配置）
2. **导入所有 agent 平台的 MCP**：扫描 Claude Code + Codex 各自配置的所有 MCP，去重合并，列出候选，用户勾选导入哪些到 aidog 集中管理
3. **删除 MCP**：从 aidog DB + 所有 agent 配置彻底删除

## 范围

### Agent 范围
- **现支持**: Claude Code (`claude-code`) + Codex (`codex`)，对齐 skills 模块两 agent
- **扩展点**: `McpAgent` enum（slug→配置后端映射），后续按需加 Cursor/Windsurf 变体 + 各自 backend 实现

### 存储模型（Q3=A）
- aidog DB 新 `mcp_server` 表（集中存），含 `enabled_agents` 字段（逗号分隔 agent slug）
- **启用** 某 agent = 把 MCP 写入该 agent 配置文件
- **禁用** 某 agent = 从该 agent 配置文件移除（DB 记录保留，可再启用）
- **删除** = 从所有 enabled agent 配置移除 + DB 删行
- **导入** = 扫两 agent 配置 → 去重 → 勾选 → 入 DB（enabled_agents 初始 = 来源 agent）

### 非 P0（后续 task）
- 手动新增 MCP 表单（command/args/env 手填）— 当前需求只含导入/启用切换/删除
- Cursor/Windsurf 等其他 agent 支持
- MCP 健康检查（`claude mcp list` 连通性）— 仅展示配置，不探活

## 架构

### 后端

**新模块** `src-tauri/src/gateway/mcp.rs`：

```rust
// agent slug 枚举（扩展点）
pub enum McpAgent { ClaudeCode, Codex }
impl McpAgent {
    fn slug(&self) -> &'static str  // "claude-code" | "codex"
    fn display(&self) -> &'static str
    fn all() -> [Self; 2]
}

// 传输类型
pub enum McpTransport { Stdio, Http, Sse }  // claude type 字段: stdio/http/sse; codex 仅 stdio

// agent 配置后端 trait
trait McpAgentBackend {
    fn read_all(&self) -> Result<Vec<RawMcpEntry>>;       // 读该 agent 所有 MCP
    fn write(&self, name: &str, cfg: &McpConfig) -> Result<()>;   // 写/覆盖
    fn remove(&self, name: &str) -> Result<()>;           // 删（不存在=ok）
}

struct ClaudeCodeBackend;  // ~/.claude.json → mcpServers
struct CodexBackend;        // ~/.codex/config.toml → [mcp_servers.*]
```

**ClaudeCodeBackend** (`~/.claude.json`):
- 读: JSON → `mcpServers` object (name → {type?, command?, args?, env?, url?, headers?})
- 写: 反序列化 → 改 mcpServers → 序列化回写（保留其他所有 key 不动）
- **决策：直写 JSON，不用 `claude mcp add` CLI**（CLI 对 env/headers 参数支持有限 + spawn 开销 + 参数转义坑；直写与 Codex 对称）

**CodexBackend** (`~/.codex/config.toml`):
- 读: TOML → 取所有 `[mcp_servers.<name>]` 段（command, args=[], `[mcp_servers.<name>.env]` map）
- 写: 读全文 → toml 编辑（增改 mcp_servers 段）→ 写回（保留其他段不动）。codex.rs 不解析 mcp_servers，本模块独立处理；复用 codex.rs 的 TOML 读写 utility 若有，否则直接 `toml` crate。
- Codex TOML mcp 仅 stdio（无 http/sse 字段）；导入 claude 的 http/sse MCP 到 codex 时 → 仅记 DB，enabled_agents 不含 codex（或标记不兼容）

**DB** (`db.rs` migration 020):
```sql
CREATE TABLE IF NOT EXISTS mcp_server (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  name          TEXT NOT NULL UNIQUE,
  transport     TEXT NOT NULL DEFAULT 'stdio',  -- stdio|http|sse
  command       TEXT NOT NULL DEFAULT '',       -- stdio: 可执行命令
  args_json     TEXT NOT NULL DEFAULT '[]',     -- stdio: 参数 JSON array
  env_json      TEXT NOT NULL DEFAULT '{}',     -- stdio: env map (含敏感值)
  url           TEXT NOT NULL DEFAULT '',       -- http/sse: endpoint
  headers_json  TEXT NOT NULL DEFAULT '{}',     -- http/sse: headers
  enabled_agents TEXT NOT NULL DEFAULT '',      -- 逗号分隔 slug
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);
```
+ CRUD 函数：`list_mcp_servers` / `get_mcp_server` / `upsert_mcp_server` / `delete_mcp_server` / `set_mcp_agent_enabled`

**Tauri commands** (`lib.rs`, 注册到 invoke_handler):
- `mcp_list() → Vec<McpServerInfo>` — DB 全列 + 解析 enabled_agents → Vec<McpAgent>
- `mcp_scan() → Vec<McpScanItem>` — 扫两 agent 配置去重合并；每项 {name, config, found_in_agents, already_imported}
- `mcp_import(items: Vec<McpImportPayload>) → ImportReport` — 批量入 DB；每项 {name, config, source_agent}；enabled_agents = source_agent
- `mcp_set_agent(name, agent, enabled) → ()` — 改 DB enabled_agents + 同步写/删 agent 配置
- `mcp_delete(name) → ()` — 从所有 enabled agent 配置删 + DB 删行（破坏性，前端二次确认）

**敏感数据处理**:
- env map 可能含密钥（如 AUGMENT_SESSION_AUTH, API_KEY）→ 列表展示/日志**脱敏**（显示 `KEY=***`，完整值仅存 DB + 写配置文件，永不回传前端完整值、不进 tracing）
- `McpServerInfo` 前端类型：env 为 `Record<string, string>` 但后端在 `mcp_list` 返回时 mask 敏感值（value→`***` 若 key 含 token/key/secret/auth/password/pass，或 value 长度>40 含 base64 样式）；导入/写配置时用 DB 原值（不 mask）

### 前端

**新顶页** `src/pages/Mcp.tsx`（对齐 Skills 顶页，App.tsx 侧栏加入口）：
- 顶栏：标题 + 「扫描导入」按钮（→ 弹 modal）
- 列表：每行
  - 左：name + transport badge（stdio/http/sse 不同色）+ command/url 摘要（env 显示 `KEY=***` 脱敏）
  - 右：per-agent toggle 图标（claude-code.svg / openai.svg，启用=accent 实心，禁用=grayscale 描边，点击切换）+ 删除按钮（trash icon，二次确认 modal）
- 空态：提示「点扫描导入」
- 扫描导入 modal：候选列表（checkbox + name + 来源 agent badge + transport + 已导入标记 disabled）+ 全选/反选 + 确认导入

**i18n** (`src/i18n/locales/*/mcp.json` 或并入主 bundle)：7 语言（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP）全 key。阿拉伯语 RTL。

**主题**: 全用项目 CSS 变量（`--accent`/`--border`/`--bg-elevated`/`--success`/`--danger`/`--accent-subtle`），禁硬编码色值（见 theme-css-var-names memory）。

**乐观更新**: toggle 点击即时反馈，失败回滚 + toast（对齐 skills-toggle-ux）。

### 跨层契约 (`services/api.ts`)
新增类型 + invoke 封装：`McpServerInfo` / `McpScanItem` / `McpImportPayload` / `ImportReport` + `mcpApi.{list,scan,import,setAgent,delete}`。字段名 Rust↔TS 严格对齐（serde rename_all 默认 camelCase 或 snake_case，对齐现有 api.ts 约定）。

## 验收标准

1. `yarn build` (tsc + vite) 无错无 warning
2. `cd src-tauri && cargo clippy` 无 warning；`cargo test` 通过（新 MCP 模块单测：agent 配置读写 parse/serialize、DB CRUD、scan 去重、set_agent 同步、敏感脱敏）
3. 启动 `yarn tauri dev`：MCP 页可见；扫描导入可拉出两 agent 配置的 MCP；勾选导入后列表展示；per-agent toggle 可切换（claude/codex 各自图标）；删除可从 DB + agent 配置移除
4. env 敏感值前端展示脱敏（`***`），不泄露到 tracing/前端完整值
5. i18n 7 语言全覆盖（check-i18n.mjs 过）
6. 6 主题 × light/dark 适配（无硬编码色值）
7. 不破坏现有 codex.toml / claude.json 其他字段（写回保留所有无关 key/段）

## 资源 / 依赖

- 参考模块: `gateway/skills.rs`（per-agent 切换 + invoke 模式）、`gateway/codex.rs`（TOML 读写）、`gateway/db.rs`（migration 模式 #020）、`components/settings/ImportExport.tsx`（导入 modal + 冲突 UI 参考）、`pages/Skills.tsx`（顶页 + per-agent toggle UI 参考）
- 现有 deps: `toml` crate（codex.rs 已用）、`serde_json`（claude.json）、`rusqlite`（DB）
- 无新 Cargo dep

## 风险

- **codex.toml 写回丢格式**: toml 反序列化→序列化可能丢注释/格式。缓解：codex.rs 已有 TOML 读写实践，遵循同模式；若 codex.rs 用原始文本编辑则同样处理。需检查 codex.rs 实际 TOML 写法（implement 阶段确认）。
- **claude.json 大文件**（211KB）：读写需保留所有无关 key。用 serde_json::Value 全量解析→改 mcpServers→写回，禁只读写 mcpServers 段。
- **agent slug 与 skills 一致**：claude→`claude-code`（非 "claude"），codex→`codex`（见 npx-skills-cli memory）。
- **并发写 agent 配置**：MCP 写 + codex.rs/settings 同步可能竞争同文件。缓解：写操作串行（文件级互斥或 aidog 内 Mutex），implement 阶段评估。
