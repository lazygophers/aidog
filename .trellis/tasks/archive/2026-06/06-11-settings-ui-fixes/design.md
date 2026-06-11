# Design — Settings UI 修复 + 拖拽统一

## 架构决策

### D1 — @dnd-kit 作为唯一拖拽方案
- 用户确认引入 @dnd-kit，统一替换全项目 3 处原生 HTML5 drag
- 收口到单个通用组件 `src/components/SortableList.tsx`（code-reuse-rules：同一逻辑 ≥2 调用点必须提取共享函数 —— 此处 3 处，强制提取）
- 原生 HTML5 drag 的痛点（无占位反馈、按钮误触、无键盘支持）由 @dnd-kit sensor + dragHandle 模式解决

### D2 — SortableList 通用封装契约
```
SortableList<T extends { id: string }>({
  items: T[],
  onReorder: (next: T[]) => void,
  renderItem: (item: T, handle: DragHandleProps) => ReactNode,
  strategy?: 'vertical' | 'grid',   // tray two-line 用 grid
})
```
- dragHandleProps 让业务决定拖拽热区（statusline 用 ☰ handle，group/tray 按需）
- 三处调用方各自适配数据 → next[] 转换

### D3 — 嵌套导入 diff（R1）
- 现状 top-level 替换 → 改为路径级（dot-path）diff
- MVP：对 `permissions`/`env`/`hooks` 等已知 object/array 顶层 key 展开一层子项；DiffModal 渲染父-子树，父项半选态
- applyImport 改深度合并：按选中 dot-path 写入，未选中子项保留 current 值
- 深层递归（>1 层）标 TODO，不阻塞 MVP

### D4 — Settings.tsx 单文件串行
- R1+R2+R3+R4 全在 Settings.tsx → S2 单执行者串行完成，禁拆多 agent 并改同文件
- S2 依赖 S1（SortableList 就绪后才能迁移 statusline 拖拽）

## 执行策略（trellisx 异步并行）

```
S1 (基建, 串行前置)
  └─ 完成 → 同一回复一次性并行派发:
       S2 (Settings.tsx)  ┐
       S3 (TrayConfigTab) ├ 3 文件互斥, 真并行, 各带 isolation:worktree
       S4 (Groups.tsx)    ┘
  └─ 三者全 done → 整体 trellis-check → commit
```

- 文件互斥已验证：S2=Settings.tsx+locale / S3=TrayConfigTab.tsx / S4=Groups.tsx，无交集
- S1 必须先单独完成并落 worktree（其余依赖其 SortableList + package.json）

## group-drag-sort task 处置
- 搁置的 `06-11-06-11-group-drag-sort` 为空 planning（无 prd），就是 Groups.tsx 拖拽
- 本 task S4 完全覆盖 → 本 task finish 后 `task.py archive 06-11-06-11-group-drag-sort`（零信息损失）
- 其 worktree `.trellis/worktrees/06-11-06-11-group-drag-sort` 一并清理

## 风险
- @dnd-kit × React 19 兼容性：装前确认版本（S1 失败处理已含）
- tray two-line grid 排序策略（S3 失败处理已含）
- SegmentEditModal "弹窗问题" 表现不明 → S2 实跑诊断
