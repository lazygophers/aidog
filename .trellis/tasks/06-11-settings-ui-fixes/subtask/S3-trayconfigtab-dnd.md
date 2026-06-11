# S3 — TrayConfigTab.tsx 拖拽迁移到 @dnd-kit

**依赖**: S1（用 SortableList）

## 目标
`src/pages/TrayConfigTab.tsx` 的 tray 列拖拽（single-line + two-line 两种布局）从原生 HTML5 drag 迁移到 S1 的 SortableList。

## 产出
- `TrayConfigTab.tsx:379`（single-line columns）+ `:453`（two-line grid）两处 draggable 列
- 移除原生 `draggable`/`onDrop`/dragIndex state，改用 `<SortableList>`
- 保留原有「draggable + clickable」语义：拖拽排序 + 点击编辑（用 dragHandleProps 分离拖拽区与点击区）
- single-line / two-line 两种布局都迁移（注意 two-line 是 grid，确认 dnd-kit 在 grid 下排序正常或用合适 strategy）

## 验收
- `yarn tsc --noEmit` 退出码 0
- `grep -nE 'draggable|onDragStart' TrayConfigTab.tsx` 清零
- 手测：tray 列 single-line + two-line 两种模式拖拽排序正常 + 点击编辑不被拖拽误触

## 资源
- 现状：`TrayConfigTab.tsx:379-` single-line / `:453-` two-line
- 项目记忆 [[tray-two-line-alignment]]：two-line 列对齐方案（LeftTabStop 两行共用）—— 迁移时勿破坏对齐
- S1 SortableList API

## 失败处理
- two-line grid 布局与 dnd-kit vertical strategy 不匹配 → 用 `rectSortingStrategy` 或标记 `需要: <grid 排序策略>` 由 main 协调
