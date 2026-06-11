# PRD: 分组路由 AI 平台拖动排序

## 需求
group 内关联平台列表支持拖动排序，顺序决定路由优先级（priority）。

## 现状
- Groups.tsx:339 注释「Selected platforms — reorderable by drag later, for now ordered list」—— 当前有序列表，**待加拖拽**
- priority 持久化已有：saveEdit(:194) 按 editPlatformIds 数组 index 设 priority(i+1)；后端 set_group_platforms(db.rs:433) DELETE+INSERT 接受 priority
- get_group_platforms(db.rs:458) `ORDER BY gp.priority` —— 读取按 priority 排序

## 方案（纯前端）
- editPlatformIds 列表加拖拽重排（HTML5 draggable 或轻量实现）
- 拖拽后更新 editPlatformIds 顺序，saveEdit 时按新顺序设 priority（复用 :194 逻辑，无需改）
- **后端无改动**（set_group_platforms / priority 持久化 / ORDER BY 已支持）

## 验收
- group 编辑：关联平台列表可拖动重排，视觉反馈
- save 后顺序持久化（priority），重开顺序正确
- tsc 0 / yarn build

## Subtask
- ST1: editPlatformIds 列表拖拽重排 UI
- ST2: 验证（拖拽 → save → priority 持久化 → 重载顺序）
