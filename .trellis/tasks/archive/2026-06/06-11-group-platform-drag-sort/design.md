# Design: 分组平台拖动排序

## 前端（Groups.tsx editPlatformIds 列表）
- 当前 :339 区渲染 selected platforms 有序列表（editPlatformIds.map）
- 加拖拽：HTML5 native drag（draggable + onDragStart/onDragOver/onDrop）轻量实现，无需引第三方库（避免依赖）：
  - 每项 `draggable`，onDragStart 记 dragIndex，onDragOver preventDefault，onDrop 把 dragIndex 项移到 targetIndex → setEditPlatformIds 重排数组
  - 拖拽视觉：drag over 项高亮/插入线，dragging 项半透明
- save 不变：saveEdit(:194) 已按 editPlatformIds 顺序设 priority(i+1)
- 拖拽手柄（⠿ icon）或整行可拖

## 不改
- 后端 set_group_platforms / priority / ORDER BY gp.priority 已支持
- 仅 Groups.tsx 编辑区

## 验证
- tsc 0；拖拽重排 editPlatformIds + save priority 持久化 + 重载顺序正确；Liquid Glass 风格
