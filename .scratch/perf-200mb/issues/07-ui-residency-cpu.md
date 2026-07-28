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

## 从 [01] 转来的口径与缺口

[01] 的 **CPU 场景 C（Logs / Stats / Platforms 各驻留 1 分钟）未采**，并入本票。量测手法已就绪（`assets/measure.sh cpu <label> [secs]` 与 `stacks`），只缺切页动作 —— aidog 未对 WKWebView 开 accessibility，`System Events` 拿不到侧栏元素，`screencapture` 无权限，**自动化点不了页，需人工驱动 UI 或应用侧开 AX**。

[01] 已定的基线，本票只需测**相对增量**：
- 空闲前台（未指定页面）总 CPU **50.2% / 51.3%**（两次），窗口隐藏 **0.2%**
- 其中 GPU helper 就占 36.9%，且 **54% 的 GPU 采样落在 `CA::CG::DrawConicGradient::draw_color`**
- 根因是全局的、与页面无关的：`src/styles/globals.css:828-870` 的 `@property --flow-ang` 逐帧动画 → conic-gradient + `mask-composite` 软件光栅化；选择器 `.glass, .glass-surface` 覆盖全仓 116 处

**推论：本票要回答的不是「哪页 CPU 高」，而是「扣掉这个全局 ~50% 底噪后，各页面还额外贡献多少」**。别把底噪重复记到某个页面头上。

窗口缩小实验（2304×1265 → 500×400）：GPU 36.9%→16.6%（面积相关），但 WebContent 9.6%→12.1% **反升** —— 存在与面积无关的重绘驱动源，本票需查明。

## 验收

- 每个页面一份驻留 CPU 数据，按 JS / 合成 / IPC 三源拆分
- 热点定位到 file:line（组件名 + 触发原因：props 变化 / 未 memo / 无虚拟化 / 重复 invoke）
- 标出改动候选清单（不做决定），每项注明潜在的红线 3 风险
