# Design: 分组内嵌进平台页

## 模块表

| 模块 | 文件 | 执行层 | 资源边界 |
| --- | --- | --- | --- |
| Groups 内嵌重构 | `src/pages/Groups.tsx` | frontend | 独占（S1） |
| Platforms 植入 + 归属 badge | `src/pages/Platforms.tsx` | frontend | 独占（S2，依赖 S1 接口） |
| 侧栏/Home 清理 | `src/App.tsx` / `src/pages/Home.tsx` | frontend | 独占（S3） |

## 契约

### GroupsEmbedded 接口（S1 定，S2 用）
```ts
export function GroupsEmbedded({
  onNavigate?,
  onGroupsChanged?: () => void   // 分组平台成员变更时通知宿主刷新归属映射
}: { ... })
```
`onGroupsChanged` 回调由 Platforms 传入，触发其重算 `platformId → groupNames[]`。禁全局 event 滥用。

### 平台归属映射（S2 派生）
```
groups = await groupDetailApi.list()
membership: Map<platformId, string[]>  // group names
for g of groups: for gp of g.platforms: membership.get(gp.platform.id).push(g.group.name)
```
Platforms.tsx 若已加载 group 数据（auto-group）则复用，禁重复 HTTP。

### 跳转契约（不变）
`onNavigate('platforms', { platformId, platformName })` — GroupsEmbedded 内点平台跳编辑沿用。

## 执行层选择

全 frontend 单层。S1→S2 串行（接口依赖）；S3 独立并行 S1。无需 worktree（无同文件并发）。

## 资源互斥

| 组 | 成员 | 互斥 |
| --- | --- | --- |
| 并行 | S1（Groups.tsx）+ S3（App/Home） | 文件不交 |
| 串行 | S1 → S2 | S2 依赖 GroupsEmbedded 导出 |

时序：S1 + S3 并行起 → S1 完 → S2。

## 风险

| 风险 | 缓解 |
| --- | --- |
| Groups.tsx 重构破坏卡片/编辑/统计逻辑 | S1 仅改外层页头包装，禁动 reducer/saveEdit/sortable；dev 逐项手测 |
| Platforms.tsx 3085 行植入引入回归 | S2 仅列表视图顶部插入 + 列表项加 badge，禁动编辑表单 |
| 归属 badge 不同步 | onGroupsChanged 显式回调刷新；不靠 event 滥用 |
| Platforms 重复加载 group 数据 | S2 先 grep 复用现有加载 |
| Home 删按钮布局空 | S3 调整网格列数 |
