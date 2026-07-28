# 07 UI 页面驻留态 CPU 归因

Type: task
Status: open
Blocked by: 01
Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](../map.md)

## Question

停在某些页面上 CPU 就高——是哪些页面、高在哪？

逐页驻留采样（每页停留 1 分钟，不操作），至少覆盖：
- **Logs** —— 大列表、可能的实时追加
- **Stats** —— 图表渲染、聚合计算
- **Platforms** —— `PlatformCard.tsx`（944 行）逐卡计算 `isPeak`、`getDefaultModels` 等 async 调用；卡片数量多时的重渲染放大
- **Groups** —— `Groups.tsx:813` 的 `fetchGroupStats` 对每个 group 各调一次后端
- **Settings** —— `formSections.tsx`（1070 行）巨石表单

对每个热页面区分三个来源：
1. **前端 JS**（React 重渲染 / 计算）—— React DevTools Profiler 或 Time Profiler 的 JS 栈
2. **合成 / 绘制**（GPU helper）—— liquid glass 的 `backdrop-filter`、大列表未虚拟化
3. **后端 IPC**（Rust）—— 页面驻留期间是否仍在反复 invoke（如 Groups 的 per-group 统计）

**这张票是 task 不是 grilling**：只量不改。它会 graduate map 中「前端具体改法」那片 fog。

注意红线 3：UI 流畅度不得下降，所以任何「降频 / 懒加载」候选都要标出对交互响应的影响，留给后续票判。

## 验收

- 每个页面一份驻留 CPU 数据，按 JS / 合成 / IPC 三源拆分
- 热点定位到 file:line（组件名 + 触发原因：props 变化 / 未 memo / 无虚拟化 / 重复 invoke）
- 标出改动候选清单（不做决定），每项注明潜在的红线 3 风险
