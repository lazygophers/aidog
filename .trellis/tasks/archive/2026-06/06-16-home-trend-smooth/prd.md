# 首页趋势曲线改平滑

## 需求
首页请求趋势曲线当前是**直折线**（polyline，commit cca7e9d），改为**平滑曲线**（更圆滑）。

## 现状
- `src/pages/Home.tsx` 趋势 SVG（~:189-258）：`<polyline>` 折线 + `<path>` 面积（直线段到底边）。点坐标 `xAt(i)/yAt(v)` 已有，归一到 trendPeak，viewBox 1000×80。

## 实现（只改路径生成，不动数据/取数/空态/标注/颜色）
- 折线 polyline → 平滑 `<path>`：用 **Catmull-Rom → 三次贝塞尔** 生成平滑 d（经过所有数据点，圆滑过渡）。
  - 实现一个 `smoothPath(points: {x,y}[]): string`：相邻点用 Catmull-Rom 切线算控制点转 cubic bezier（`C c1x c1y c2x c2y x y`）。张力适中（0.5 经典 Catmull-Rom）避免过冲。
  - 线 stroke path 用此 d；**面积 path** 复用同平滑曲线 + 闭合到底边（`L lastX,H L firstX,H Z`）。
  - 保留 `vector-effect:non-scaling-stroke`、var(--accent)、面积 linearGradient、峰值点 circle、hover rect、x 轴标注，全不变。
- 边界：单点/双点 → 退化为直线/平直（不崩）；全 0 → 空态（不变）。

## 约束/范围
- **只改 `src/pages/Home.tsx`**（曲线路径生成部分）。无新 i18n。⚠️ 并发会话改 api.ts/Stats.tsx/src-tauri——**禁碰**。提交路径限定 `git add src/pages/Home.tsx`，禁 git add -A。
- 禁图表库；纯 SVG path 数学生成。

## 验收
- `yarn build` 过；`git diff --name-only` 仅 Home.tsx。
- 行为：趋势为**平滑曲线**（非直折线棱角），经过数据点，面积同平滑；峰值点/hover/标注/颜色/空态不变；无过冲诡异。

## 失败处理
- Catmull-Rom 过冲（曲线冲出 0~peak）→ clamp y 或降张力。
- 点数 <2 → 直线兜底。门禁红修到绿；卡住标 `需要:`。
