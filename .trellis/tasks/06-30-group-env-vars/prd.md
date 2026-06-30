# PRD: 分组配置支持环境变量设置

## 背景

每分组 sync 到 `~/.aidog/settings.{group_key}.json`，其 `env` block 已被 aidog 强写
`ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN`（proxy 路由字段，`sync_settings.rs:259-268`）。
Claude Code 原生读 `settings.json.env` 作为会话环境变量。当前分组**无自定义 env 入口**，
用户无法按分组注入 `ANTHROPIC_DEFAULT_OPUS_MODEL` / `CLAUDE_CODE_*` / hook 自用变量等。

## 目标

分组维度支持自定义环境变量（key-value 列表），随分组配置持久化，sync 时注入到：

1. **Claude settings.env**（主）：合并进 `settings.{group}.json` 的 env block
2. **Codex 侧**（条件）：
   - research 验证 Codex `<group>.config.toml` 是否支持 env 注入
   - 支持 → 写入 config.toml
   - 不支持 → fallback：`buildCodexCommand`（`Groups.tsx:399`）前置 `KEY=VALUE ` 导出

## 非目标

- 不做全局（非分组级）env 配置（全局走 Claude Code 自身 settings）
- 不影响 proxy 进程自身环境（仅影响下游 Claude/Codex 会话）
- 不做 env 变量跨分组继承/模板

## 设计决策（用户已裁定）

| 决策点 | 选择 |
|---|---|
| 注入目标 | Claude settings.env + Codex 双写（条件） |
| 冲突保护 | **禁止覆盖** `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN`（aidog 强写字段，用户同名 key 丢弃 + 警告日志） |
| UI 位置 | Groups 页分组详情/编辑面板新增「环境变量」区块（与 model_mappings 同级） |
| 导入导出 | 纳入 `.aidogx` 容器（group 序列化随行，AES-256-GCM 已加密） |
| Codex 不支持时 | config.toml 跳过；fallback 到 buildCodexCommand 前置 export |

## 数据模型

```rust
// models/group.rs — Group 新增字段（内联 JSON 数组，仿 model_mappings）
#[serde(default)]
pub env_vars: Vec<EnvVar>,

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}
```

CreateGroup / UpdateGroup 同步加 `env_vars` 字段。

## 触点清单

### 后端 (Rust)
- `models/group.rs`: Group / CreateGroup / UpdateGroup 加 `env_vars`
- `db/schema_late.rs`: groups 表加列 `env_vars TEXT NOT NULL DEFAULT '[]'`（migration + 既有行回填）
- `db.rs` (或 group 相关 db 文件): create_group / update_group / list_groups / get_group 的 SQL + 序列化
- `commands/sync_settings.rs::do_sync_group_settings`: 循环内 merge 用户 env_vars 到 env block；**保护字段过滤**（跳过 ANTHROPIC_BASE_URL/ANTHROPIC_AUTH_TOKEN，warn 日志）
- `gateway/codex.rs::build_group_profile_toml` / `write_group_profile`: 若 research 确认支持 → 接收 env_vars 写入；不支持 → 保留原签名（fallback 走前端）
- `lib.rs`: create_group / update_group command 参数透传（如已是 Group struct 入参则自动）

### 前端 (TS/React)
- `services/api.ts`: Group 类型加 `env_vars: EnvVar[]`；createGroup / updateGroup args 加字段（camelCase）
- `pages/Groups.tsx`:
  - 分组编辑面板新增「环境变量」区块（key-value 行编辑器 + 增删行）
  - `buildCodexCommand`（line 399）: Codex config.toml 不支持时，前置 `KEY=VALUE ` 导出（读取 group.env_vars）
- i18n: `src/locales/*.json` 7 语言补 key（`group.envVars.*`）

### 导入导出
- `import_export/`: group 序列化已含全部字段，验证 env_vars 随行（无 strip 逻辑即可）

## 验收标准

1. 新建/编辑分组可增删改 env 变量对，保存后重启仍在（DB 持久化）
2. 该分组 sync 后，`~/.aidog/settings.{group_key}.json` 的 env block 含用户变量
3. 用户变量 key = `ANTHROPIC_BASE_URL` 或 `ANTHROPIC_AUTH_TOKEN` → 被丢弃 + 后端 warn 日志，proxy 路由字段不被覆盖
4. research 结论落档：Codex config.toml env 支持情况（引用 Codex 官方文档）
5. Codex 不支持时，「复制 Codex 命令」输出前置用户 env export
6. cargo clippy / cargo test 全绿；yarn build 通过；check-i18n 7 语言全覆盖
7. 导出 .aidogx 含 env_vars，导入后还原

## 风险

- **Codex env 支持未知**：research 子任务须先验证，结论决定 Codex 侧实现路径（config.toml vs buildCodexCommand export）
- **保护字段清单可能不止 2 个**：research 时确认是否还有其它 aidog 强写 env key（如 statusline 相关 ANTHROPIC_*）
- **env value 含特殊字符**：buildCodexCommand 前置 export 须正确 shell 转义（复用 `shellSquote`）
