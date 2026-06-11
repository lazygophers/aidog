# S1 — @dnd-kit 基建 + 通用 SortableList 封装

**依赖**: 无（前置，S2/S3/S4 全部依赖本 subtask）

## 目标
引入 @dnd-kit，提供一个项目通用的可复用排序列表封装，供 statusline / tray / group 三处拖拽统一调用。

## 产出
- `package.json` 新增依赖：`@dnd-kit/core` + `@dnd-kit/sortable` + `@dnd-kit/utilities`
- 新建 `src/components/SortableList.tsx`：通用泛型排序列表组件
  - props: `items: T[]`（含稳定 `id`）/ `onReorder(next: T[])` / `renderItem(item, dragHandleProps)` 
  - 内部用 `DndContext` + `SortableContext` + `useSortable`
  - 支持「仅拖拽手柄可拖」（renderItem 暴露 dragHandleProps，业务挂到 handle 元素上，避免 Toggle/按钮误触）
  - 拖拽时占位/视觉反馈（@dnd-kit 内置 transform + transition）
  - 键盘可访问（@dnd-kit KeyboardSensor）

## 验收
- `yarn tsc --noEmit` 退出码 0
- SortableList 单独可编译，泛型 + dragHandleProps 类型正确
- 组件遵循项目约定：named export `export function SortableList<T>(...)`、inline style + glass class、放 `src/components/`（见 frontend/conventions.md + code-reuse-rules.md）

## 资源
- 现有原生 drag 参考：`Settings.tsx:2046-2102`（segment list 结构）
- @dnd-kit sortable 标准用法（vertical list + restrictToVerticalAxis 可选）

## 失败处理
- @dnd-kit 版本与 React 19 不兼容 → 标记 `需要: <版本问题>` 由 main 转达，先确认兼容版本再装
