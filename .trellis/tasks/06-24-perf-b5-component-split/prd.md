# PRD — 巨石组件拆分 (Groups renderItem → SortableRow memo 化)

## 背景

前一 task (06-24-06-24-perf-frontend-hotpath) 已完成 A1-A4（quota memo 修复 / 派生数组 useMemo / prepare_cached / Groups 保存慢修复），体感主导瓶颈已收窄。

B5 未做的原因：Groups `renderItem` 大闭包耦合 5 个微妙状态机 + worktree 基线 merge 风险，保守推迟独立 task 处理。

本 task = B5 巨石组件拆分，分步保守抽子组件，缩小重渲范围，保 5 状态机不破。

## 目标

把 `src/pages/Groups.tsx`（1831 行）的 `renderItem` 大闭包重渲瓶颈拆成 memo 化子组件，每步可编译可验收。`src/pages/Platforms.tsx`（3311 行）可视 Groups 拆分经验决定是否跟进。

## 范围

### MUST（分步执行，顺序强制）

1. **读源码定位**：审计 Groups.tsx `renderItem` 闭包当前依赖项（哪些 state 触发重渲，哪些真正用到）；记录 SortableList / SortableRow 现有结构。

2. **抽纯展示 `GroupCardHeader`**（第一步，最安全）
   - 无状态、无闭包 state 捕获，props 全为稳引用（id / name / isExpanded 等基元）。
   - `React.memo` 包裹，父 `renderItem` 不再在渲染期重建此部分。
   - 每步后 `yarn build` 验证。

3. **memo 化 `SortableRow` / `renderItem` 回调**（第二步）
   - 当前 `renderItem` 是渲染期内联箭头函数 → 每渲染重建，SortableList 拿到新引用 → 全行重渲。
   - 用 `useCallback([...deps])` 稳定 `renderItem` 引用（deps 只含真正需要的 state）；或把整个 item 渲染提成 memo 化 `GroupRow` 组件接收稳定 props。

4. **局部态下沉**（第三步，如有需要）
   - 组内编辑态（展开/折叠/行内编辑）若仍在父 state，下沉到对应子组件 local state，减少父重渲传播。
   - 仅在收益明确时做（不强求）。

5. **Platforms.tsx 评估**（可选，A1 已修主要瓶颈）
   - 若 Groups 拆分顺利，同理评估 Platforms `renderItem` / `PlatformEditModal` 下沉编辑态。
   - 若风险高则记 open_issues，不在本 task 强做。

### 5 个不可破坏的微妙状态机（逐一保真验证）

| 状态机 | 注意点 |
|---|---|
| 拖拽 pointer hit-test | WKWebView HTML5 DnD 失效，拖拽必须 pointer 事件 + elementFromPoint，禁改拖拽容器结构 |
| navContext 导航入口透传 | App.tsx 每页渲染处显式传 initialFilter；同页编辑入口直调父 handler 不走 onNavigate 往返（见 [[navcontext-render-passthrough]] / [[navcontext-edit-retrigger-stale]]）|
| epoch generation 守卫 | 局部刷新乐观 setState 须携带 epoch 防慢后端覆盖（见 [[platforms-partial-refresh-epoch-guard]]）|
| dirtyRef + cancelled 守卫 | mount get() 晚到 resolve 须 cancelled 守卫防回弹（见 [[mount-fetch-late-resolve-overwrites-optimistic]]）|
| groupDetails 同步刷新 | 改分组归属须刷 groupDetails 不只 load() platforms（见 [[platforms-groupdetails-refresh-gap]]）|

### MUST NOT

- 禁一次性大爆改（每步必须 `yarn build` + 功能自查后才下一步）。
- 禁改拖拽容器的事件处理模型（pointer hit-test 是非标准实现，改了必测 macOS WKWebView）。
- 禁把 navContext / epoch / dirtyRef 守卫逻辑从父移走（它们是跨子组件的全局不变量，必须留父级）。
- 不改已完成的 A1-A4 改动。

## 验收标准

- `yarn build` 通过（tsc && vite build 0 error）。
- 功能无回归：拖拽排序 / 分组展开折叠 / 平台增删改 / navContext 编辑入口 / Groups 保存局部刷新均正常。
- 体感：≥5 分组时，单组展开/折叠/重命名不触发全列表卡重渲（React DevTools Profiler 验证重渲范围收窄，可 tar 截图记录）。
- 无 console 残留 / 无 tsc any 兜底掩盖。

## 风险 / 备注

- 拖拽是最高风险点：只要不改 SortableList 的 pointer 事件结构，memo 化 `renderItem` 回调通常安全。
- `useCallback` 依赖项漏写 = stale 闭包 bug（比不优化更糟），须逐一确认 deps 齐全。
- 若 Groups 拆分中途发现 renderItem 闭包依赖链条太深（难以安全 memo 化），partial 收尾 + open_issues 记录，不强求 100% 拆完。
