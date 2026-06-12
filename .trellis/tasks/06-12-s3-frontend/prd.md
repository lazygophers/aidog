# S3 前端响应性优化

> Parent: [06-12-p0-p1](../06-12-p0-p1/prd.md) · 与 S1/S2 无文件交集，可并行。

## Goal

消除前端无谓 re-render、后台轮询浪费、搜索卡顿，提升 UI 响应性与实时性。功能零回归。

## Requirements（P0）

- R3.1 三处 setInterval 接 Page Visibility API，不可见时暂停、可见恢复：Logs.tsx:177（3s 列表）、Logs.tsx:209（2s 详情）、TrayConfigTab.tsx:162（30s）。
- R3.2 `AppContext` value useMemo 包裹（AppContext.tsx:90-103），稳定依赖。
- R3.3 pinyin 搜索 debounce 300ms（Platforms.tsx:1744 输入）+ utils/pinyin.ts 加 LRU 缓存。
- R3.4 Platforms 列表抽 `PlatformCard` 独立 `React.memo` 组件（Platforms.tsx:2065），computeQuotaDisplay 等重算 useMemo。

## Requirements（P1）

- R4.1 i18n 改按需加载 / 仅打包当前 locale + en-US fallback（locales/index.ts）。
- R4.2 Groups 编辑态 11 个 useState → useReducer（Groups.tsx:113）。
- R4.3 手写平台拖拽 requestAnimationFrame throttle（Platforms.tsx:994）或复用 SortableList。

## Acceptance Criteria

- [ ] `tsc && vite build` 0 error 0 warning。
- [ ] 后台不可见时三处轮询停止（DevTools/日志验证）。
- [ ] AppContext 变更不触发无关全树重渲（React DevTools Profiler）。
- [ ] 100+ 模型搜索无明显卡顿；pinyin 重复查询命中缓存。
- [ ] 50+ 平台列表滚动/展开流畅；功能（拖拽排序/编辑/配额显示）不变。

## Out of Scope

- 虚拟列表 react-window（平台 >100 才需）。
- 改后端批量 stats API（本轮前端侧 memo 缓解即可）。

## Technical Notes

- 遵 [frontend/conventions.md](../../spec/frontend/conventions.md)：目录/组件/状态/类型规则。
- 新增 UI 文案走 i18n `t()`，禁硬编码。
- formatters/colorScale 统一走 utils，禁页内重复定义。
