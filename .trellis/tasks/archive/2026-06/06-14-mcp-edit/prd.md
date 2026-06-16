# MCP 编辑功能

## 目标
MCP 页支持编辑已导入 server 的全字段（含改名 + transport 切换）。

## 范围（用户确认）
- 全字段可编辑：name / transport / command / args / env / url / headers
- 允许改名（级联 agent 配置）

## 后端（src-tauri/src/gateway/mcp.rs + db.rs + lib.rs）
新增 `update_server(db, old_name, payload)`：
1. 读旧 row（保留 id + created_at，取 enabled_agents + 原 env/headers 明文）
2. **脱敏 merge**：payload.env/headers 中值为 `"***"` 的 key → 从旧 DB row 取明文；其他 → 用 payload 新值（前端 *** 占位不可信，复用导入同款处理）
3. **transport 兼容重算**：新 transport 下不支持的 enabled agent（如 codex + http）→ 从 enabled 移除 + 该 agent 配置 remove（旧 name）
4. **改名级联**：old_name != new_name → 旧 name 所有 enabled agent 配置 remove；DB 旧名删行
5. upsert 新 row（new_name + 新字段 + 重算后 enabled_agents csv，保留 id/created_at）
6. 对重算后仍 enabled 的 agent：write(new_name, new_cfg)

新增 `McpUpdatePayload` struct（camelCase）：name / transport / command / args(Vec) / env(Map) / url / headers(Map)。

db.rs：复用 `upsert_mcp_server`（ON CONFLICT name）+ `delete_mcp_server`（改名时删旧）。无需新 DB 函数。

lib.rs 新命令 `mcp_update(db, old_name, payload)` → 注册 generate_handler。

## 前端（src/pages/Mcp.tsx + api.ts + locales）
- `McpRow` 加「编辑」按钮（icon 按钮，btn-ghost）
- 编辑 modal（复用现有 modal 样式 scan/delete portal 模式）：
  - name input
  - transport select（stdio/http/sse）
  - stdio 字段：command input + args textarea（每行一 arg）+ env dynamic rows（key/value/删/加）
  - http/sse 字段：url input + headers dynamic rows
  - env/headers 值显示脱敏 ***（未改保留），用户改则新值
- submit → `mcpApi.update(oldName, payload)` → refresh
- optimistic / loading 态同现有

api.ts：`McpUpdatePayload` 类型 + `mcpApi.update`。

i18n：`mcp.edit` / `mcp.editTitle` / `mcp.field.*`（name/transport/command/args/env/url/headers）/ `mcp.addArg` / `mcp.addRow` / `mcp.save`，8 语言。

## 验证
- `cargo clippy` 无 warning + `cargo test`（mcp.rs 加 update 相关单测：脱敏 merge / transport 重算）
- `yarn build` 过
- `python3 scripts/check-i18n.mjs`（若存在）零缺失
- 手动：编辑 stdio server 改 command → agent 配置同步；改名 → 旧名配置删 + 新名写；改 transport http → codex 自动 disable

## 资源
mcp.rs / db.rs / lib.rs / Mcp.tsx / api.ts / locales/*.json / icons.tsx（编辑 icon）

## 复用
- backend trait write/remove 已有
- McpServerRow.to_raw_cfg / mask_env / transport.supported_by
- Mcp.tsx modal portal 样式
