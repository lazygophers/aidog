# S2 — Settings.tsx 全部改动（R1+R2+R3+R4）

**依赖**: S1（用 SortableList 替换 statusline 拖拽）

## 目标
单文件 `src/pages/Settings.tsx`（+ locale）集中完成 4 类 Settings 问题修复。单执行者串行，避免同文件冲突。

## 产出（4 块）

### R1 导入嵌套粒度
- `handleImportFromClaudeCode`（`Settings.tsx:4110`）+ DiffModal（`Settings.tsx:2336`）+ `applyImport`（`Settings.tsx:4135`）
- 对 object/array 类型的 top-level key 展开到**子项级 diff**（如 `permissions.allow[2]` / `env.FOO` / `hooks.PostToolUse`）
- DiffModal 支持嵌套树形勾选：父项半选态、子项独立 toggle
- `applyImport` 按选中子项做深度合并（仅写入选中子路径，未选中子项保留 current）

### R2 去 icon
- 删左侧导航 `SectionIcon`（`Settings.tsx:4437`）
- 删 statusline segment 行 `SectionIcon`（`Settings.tsx:2076`）

### R3 状态行
- statusline segment list（`Settings.tsx:2046-2102`）：移除原生 `draggable`/`onDragStart`/`onDragOver`/`onDrop`/`onDragEnd` + `dragIdx` state，改用 S1 的 `<SortableList>`
- segment 卡片 UI 优化（布局/间距/视觉，对齐 Liquid Glass）
- SegmentEditModal（`Settings.tsx:1815`）：实跑确认"弹窗问题"具体表现并修复

### R4 去头部文案
- 删 `Settings.tsx:4273-4279` 的 title + desc 渲染块（或仅保留 mode 切换按钮的 header 右侧）
- 删 `locales/zh-CN.json:105-106` + `locales/en-US.json:105-106` 的 `settings.title` / `settings.desc`（若他处仍引用 title，保留 key 仅清空 desc —— 执行时 grep 确认引用点）

## 验收
- `yarn tsc --noEmit` 退出码 0
- `grep -n 'SectionIcon' Settings.tsx` 在导航(4437)+segment(2076) 两处清零
- `grep -nE 'draggable|onDragStart' Settings.tsx` 清零（statusline 改 SortableList）
- 导入弹窗嵌套 key 可子项级勾选（手测 permissions/env）
- Settings 头部无标题/副标题文案
- 文案删除后无 i18n missing key 报错（grep 确认无残留引用）

## 资源
- spec: frontend/conventions.md（组件/状态/类型约定）、code-reuse-rules.md、cross-layer-rules.md
- S1 产出的 SortableList API

## 失败处理
- settings.title 被他处引用 → 保留 key，仅删 desc + header 渲染
- 嵌套 diff 深度合并逻辑复杂 → 先 MVP 支持一层嵌套（permissions/env/hooks 已知结构），深层递归标 TODO
