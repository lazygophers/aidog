# S4 — Groups.tsx 拖拽迁移到 @dnd-kit

**依赖**: S1（用 SortableList）

## 目标
`src/pages/Groups.tsx` 的 group 内 platform 排序从原生 HTML5 drag 迁移到 S1 的 SortableList。**覆盖搁置的 group-drag-sort task**。

## 产出
- `Groups.tsx:406-411` draggable platform 行
- 移除原生 `draggable`/`onDragStart`/`onDragOver`/`onDrop`/`onDragEnd` + `dragIndex`/`dragOverIndex` state
- 改用 `<SortableList>`，`onReorder` 调用现有 `reorderPlatforms(from, to)` 逻辑
- 保留 dragOver 视觉反馈（用 dnd-kit 内置 over 态替代手写 dragOverIndex）

## 验收
- `yarn tsc --noEmit` 退出码 0
- `grep -nE 'draggable|onDragStart' Groups.tsx` 清零
- 手测：group 内 platform 拖拽排序正常，排序持久化（reorderPlatforms 仍生效）

## 资源
- 现状：`Groups.tsx:406-411`（draggable 行）+ `reorderPlatforms` 函数
- 项目约定 [[group-stats-aggregation]]（Groups 页结构参考）
- S1 SortableList API

## 失败处理
- reorderPlatforms 签名与 SortableList onReorder（返回 next[]）不匹配 → 适配层转换 from/to ↔ next[]
