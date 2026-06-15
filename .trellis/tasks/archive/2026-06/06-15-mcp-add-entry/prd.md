# MCP 主动添加入口

## 背景
MCP 管理仅有「扫描导入」（从 agent 配置拉）+ 编辑/删除。缺手动新建 server 入口。用户要能主动添加。

## 方案
### 后端
1. `mcp.rs`: `add_server(db, payload: McpUpdatePayload) -> McpServerInfo`
   - name 非空校验
   - name 不重复（get_mcp_server 返回 Some → 报错 "already exists"）
   - 构造 McpServerRow(id=0, enabled_agents="") → upsert_mcp_server
   - enabled 空 → 不写 agent 配置（用户后续 toggle）
   - 返回 get_mcp_server(payload.name)
2. `lib.rs`: `mcp_add` command + 注册 invoke_handler

### 前端
3. `api.ts`: `mcpApi.add(payload)`
4. `Mcp.tsx`:
   - 新 `editOpen` state（替代 `editTarget &&` modal 条件）
   - header 加「添加」按钮（btnGhost，scan 按钮旁）
   - `openAdd()`: editForm 清空默认(transport=stdio) + editTarget=null + editOpen=true
   - `handleEditSave` 分流: editTarget=null → mcpApi.add, else update
   - busyKey add 模式 `add::`
5. i18n: mcp.add / mcp.addTitle（7 语言全覆盖）

## 验证
- cargo test + clippy + tsc + check-i18n.mjs 全过。
- 手动：添加按钮 → 填表 → 保存 → 列表出现新 server（enabled 空）→ toggle agent 启用。

## 非目标
- 不改 update/scan/import 逻辑。
