# S2: Platforms 页植入 GroupsEmbedded + 平台归属 badge

## 五要素

- **目标**：Platforms.tsx 列表视图（L2963+）顶部渲染 `<GroupsEmbedded onNavigate={onNavigate} />`；平台列表项加「所属分组」badge 列（一平台可属多分组，N:N）。
- **产出**：`src/pages/Platforms.tsx`：
  - import `GroupsEmbedded` from `./Groups`。
  - 列表视图顶部插入分组段（`<GroupsEmbedded/>` + 分隔）。
  - 平台归属映射：`groupDetailApi.list()` 取所有分组 → 构建 `platformId → groupNames[]`；平台列表项渲染归属 badge（`badge badge-muted`，复用 Groups 卡片 badge 样式）。
  - GroupsEmbedded 内分组编辑改平台后需刷新平台列表归属 → 通过 `onProxyLogUpdated` 或 props 回调 lift refresh（最简：GroupsEmbedded 内 load() 后 event bus；或 Platforms 传 refresh 回调）。
- **验证**：dev 平台页顶部见分组段全功能；平台列表项见所属分组 badge；分组内增删平台 → 平台 badge 同步。
- **资源**：`src/pages/Platforms.tsx`（列表 L2963+）；`groupDetailApi.list()`（api.ts）；`src/pages/Groups.tsx` GroupsEmbedded（S1 产出）。
- **依赖**：S1（GroupsEmbedded 导出）。

## 现状线索

- Platforms 列表视图：`Platforms.tsx:2963` `// ── List view ──`。
- Platforms 可能已加载 group 数据（auto-group 功能）—— grep `groupDetailApi|groupApi` in Platforms.tsx 确认，避免重复加载。
- 平台列表项渲染：grep 列表 renderItem，定位插入 badge 位置。
- 刷新联动：GroupsEmbedded 保存分组后调自身 load()；Platforms 需感知 → 用 Tauri event（`onProxyLogUpdated` 已有）或自定义 event；最简方案 GroupsEmbedded 接受 `onGroupsChanged?` 回调，Platforms 传刷新归属映射函数。

## dispatch prompt

```
目标：Platforms.tsx 列表视图顶部植入 GroupsEmbedded；平台列表项加所属分组 badge（N:N）。
已知：列表视图在 L2963+。GroupsEmbedded 由 S1 从 ./Groups 导出。平台归属 = groupDetailApi.list() 构建 platformId→groupNames[]（先 grep Platforms.tsx 是否已加载 group 数据复用）。badge 用 badge badge-muted 复用 Groups 卡片样式。分组编辑改平台后须刷新平台 badge → GroupsEmbedded 接受 onGroupsChanged 回调，Platforms 传刷新归属映射函数（禁全局 event 滥用，优先 props 回调）。
工作目录与范围：仅 src/pages/Platforms.tsx。禁改 Groups.tsx（S1 已定接口）/编辑表单/后端。
输出格式：diff + 平台页布局描述 + badge 同步机制说明。
验收标准：分组段全功能；平台 badge 正确且 N:N 多归属显示；分组改平台后 badge 同步；tsc 0 error。
失败处理：Platforms 已有 group 加载逻辑 → 复用禁重复；badge 同步失效 → 加 onGroupsChanged 回调显式刷新；布局拥挤 → 分组段可折叠默认展开。
```
