# PRD — 平台复制功能

## 目标
Platforms 页每个平台卡片增加「复制」操作: 点击后**复用源平台全部配置**填入表单, **直接进入编辑页**, 用户审阅/微调后点保存才真正新建一条平台。

## 用户决策 (brainstorm 已定)
1. **落库时机**: 进编辑页, 保存才新建 (`editing=null`, 不立即写库, 可中途取消)。
2. **命名**: 完全照搬原名 (`platform.name` 无 UNIQUE 约束, 允许同名)。
3. **复制范围**: 连分组归属一起复制 (复用协议/base_url/apikey/模型/端点/extra/熔断/手动预算 + 源平台当前所在手动分组)。

## 行为规格
- 卡片操作区 (查看日志 / 编辑 / 删除 旁) 新增「复制」图标按钮, tooltip = `platform.duplicate`。
- 点击复制 → 等价于 `handleEdit(p)` 的**全字段 state 灌入**, 但 **`setEditing(null)`** (而非 `setEditing(p)`):
  - `name` = 源平台原名 (照搬, 不加后缀)
  - `protocol / apiKey / models / availableModels / endpoints / extra / mockConfig / newApiConfig / manualBudgets / breaker(failure/open/halfOpen)` 全部照搬
  - `codingPlan` 按 endpoints 是否含 coding_plan 推导 (同 handleEdit)
  - `joinGroupIds` = 源平台当前手动分组 (复用 handleEdit 的反查逻辑: 排除 auto 组, 取 membership 命中的组)
  - `lockedGroupId = null`; `autoGroup` 维持默认 (true) — 注: 因 `joinGroupIds` 已照搬, 新平台保存时会加入这些手动组; auto_group=true 另建默认 auto 组 (与新建平台默认行为一致)
  - Claude Code 配置: 复用源平台 `platform:<id>` 的 claude_code override (prefill `claudeConfigJson`); 仅当源平台确有非空 override diff 时 `setShowClaudeConfig(true)` 令其随保存落到新平台 id (保存逻辑见 handleSave L2080-2096), 无 override 则维持折叠不强开。
  - `setShowForm(true)` 进入编辑页。
- 保存走既有 `handleSave` 的 `editing==null` → `platformApi.create` 分支 (L2067-2078), 已覆盖全部 create 字段 (含 auto_group / join_group_ids), **后端无需改动**。

## 范围边界
- **纯前端**, 不改 Rust 后端 / 不改 db / 不加 Tauri command (create 命令已支持全字段)。
- 主列表卡片必做; GroupsEmbedded 分组卡内的复制按钮**可选不做** (本轮仅主列表, 避免扩散)。
- 不做批量复制 / 不做跨设备导出 (已有导入导出模块负责)。

## 验收标准
1. 平台卡片有「复制」按钮, hover 显 tooltip (8 locale 全覆盖, 无裸 key)。
2. 点复制 → 表单打开且为**新建态** (标题/保存按钮表现为创建, 非更新); 全字段已按源平台预填, 名称=原名。
3. 不点保存关闭 → 不新增任何平台 (无副作用写库)。
4. 点保存 → 列表新增一条独立平台 (独立 id), 配置与源平台一致, 分组归属一致。
5. `yarn build` (tsc + vite) 通过; `scripts/check-i18n.mjs` 通过 (无新增裸 key / locale 缺失)。
