# S1: Groups.tsx 重构为内嵌组件

## 五要素

- **目标**：Groups.tsx 导出可内嵌的 `GroupsEmbedded` 组件（去掉整页 `section-header` 外框/页标题，保留分组卡片列表 + 编辑表单 + 添加表单 + 拖拽 + 统计 + 模型映射 + 中间件规则全部逻辑）。原 `Groups` 命名导出保留为薄壳（兼容潜在引用，grep 确认无外部引用则删）。
- **产出**：`src/pages/Groups.tsx`：
  - 顶层加子区块标题「分组」+ 分组计数（用 i18n 键 `page.groups` 已有，复用）。
  - 移除最外层 `section-header`（含 page 标题 `page.groups` 描述行），保留「+ 添加分组」按钮与代理 base_url 条（移入子区块标题行右侧）。
  - 其余（load/refreshStats/reducer/edit page/list view/SortableList）原样。
  - 导出 `export function GroupsEmbedded({ onNavigate })` 复用同一渲染。
- **验证**：dev 单独渲染 `<GroupsEmbedded/>`（临时或在 S2 内）显示分组卡片列表且所有交互正常；tsc 0 error。
- **资源**：`src/pages/Groups.tsx`（全文 980 行；页头 L718-742；列表 L776+；编辑 L438+）；i18n `page.groups`/`group.*` 现有键。
- **依赖**：无。

## 现状线索

- 页头：`Groups.tsx:720-742` `section-header`（标题 `page.groups` + 计数 + proxyBaseUrl + 添加按钮）。
- 组件签名：`export function Groups({ onNavigate })`（L260）。
- 外部引用：grep `import.*Groups.*from.*pages/Groups` — 仅 App.tsx:6。重构后 App.tsx 不再渲染 Groups（S3 移除），GroupsEmbedded 由 Platforms.tsx 引用（S2）。

## dispatch prompt

```
目标：Groups.tsx 重构导出 GroupsEmbedded 内嵌组件，去掉整页 section-header 外框，保留全部分组管理逻辑。
已知：页头在 L720-742 section-header。组件 export function Groups(L260)。外部仅 App.tsx 引用（S3 会移除）。重构后由 Platforms.tsx 引用 GroupsEmbedded。子区块标题复用 i18n page.groups。代理 base_url 条 + 添加分组按钮移到子区块标题行右侧。
工作目录与范围：仅 src/pages/Groups.tsx。禁改逻辑（reducer/saveEdit/sortable/stats/mappings/middleware）。禁改后端/i18n 文件。
输出格式：重构后组件签名 + 删/改行号说明 + dev 自测描述。
验收标准：GroupsEmbedded 渲染分组卡片列表交互全正常；拖拽/编辑/添加/统计/映射/中间件零回归；tsc 0 error。
失败处理：section-header 内 proxyBaseUrl/create 按钮布局难调 → 保留原 section-header 仅删最外层标题描述行；导出名冲突 → grep 全仓确认。
```
