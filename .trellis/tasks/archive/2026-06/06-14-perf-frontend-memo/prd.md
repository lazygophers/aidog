# perf 前端 memo（问题 4/5）

> parent: `06-14-deep-perf-optimization`。文件集与 child-A(后端) 不相交，可并行。仅改 `Logs.tsx`/`Platforms.tsx`。

## Goal

削减 aidog 前端两个巨石页的无谓重渲染/重建。只变快、不变行为、不变 UI。

## 范围（2 项，独立）

### 问题 4 — Logs 列表行优化
- 位置：`src/pages/Logs.tsx:496`（`logs.map(log => <tr>...)`，0 个 memo，27 useState/useEffect）。
- 改：(a) 抽 `LogRow` 子组件 + `React.memo`（按 `log.id` 稳定）；(b) 行内固定 `style={{...}}` 对象（`:499/511/516/520`）提模块级常量；(c) onClick 用 useCallback 或传 id 而非闭包（否则 memo 失效）。
- 虚拟化：当前 232 行 + 分页，单页通常 <100 行 → 行 memo 优先，虚拟化按需（单页>100 行才上 react-window）。

### 问题 5 — Platforms 派生值 memo
- 位置：`src/pages/Platforms.tsx:1432`（42 useState，27 处 .map/.filter/.sort，仅 1 处 useMemo）。
- 已就位：`PlatformCard` 已 `memo`(`:1110`) + `cardActions` 已 `useMemo`(`:1903`)。
- 改：渲染体内 `.filter/.sort` 派生值（平台过滤/协议选项等）包 `useMemo`，依赖明确；传给 `PlatformCard` 的 handler 包 `useCallback`（保 memo 生效，若传新闭包 memo 会被击穿）。
- 禁：整文件拆分（out of scope）。

## Acceptance Criteria

- [ ] `yarn build` 过；`yarn check:i18n` 过（若动文案，本任务预期不动）。
- [ ] before/after：Logs 行重建/style 对象重建次数下降；Platforms 派生计算次数下降（用 React DevTools 或推理说明）。
- [ ] 无回归：Logs 行展示/点击/实时刷新行为不变；Platforms 列表筛选/排序/卡片操作行为不变、UI 不变。

## Technical Notes

- 全部改动落 worktree `.trellis/worktrees/06-14-perf-frontend-memo`。
- memo 击穿陷阱：传给 memo 子组件的对象/函数 props 必须稳定引用，否则 memo 无效——这是本任务核心。
- 不碰 `api.ts`/后端（那是 child-A）。
