# 首页请求趋势改曲线图

## 需求
首页「请求趋势·今日」当前是柱状图（commit 16e9ae6），改为**曲线图**（SVG 折线 + 可选面积填充），不要柱状。

## 现状
- `src/pages/Home.tsx` 趋势块（~:167-205）：纯 div 柱状（高度按 total_requests/peak）。数据 `trendBuckets: StatsBucket[]`（今日 hourly 24 桶，total_requests），派生 trendPeak/trendTotal/hasTrend 已有。

## 实现（只改图形态，数据/取数/空态不动）
- 柱状 div 换 **SVG 曲线图**：
  - x = 各小时桶索引均布，y = total_requests 归一到 peak。
  - **折线**（polyline 或平滑 path）+ **面积填充**（折线下方到底边，单 --accent 低透明渐变 area，禁硬编码 hex，用 color-mix/oklch 或 accent + opacity）。线用 var(--accent)。
  - viewBox 自适应容器宽 + 固定高(~72-96px)，preserveAspectRatio none 或按比例；responsive。
  - 末点/峰值点可加小圆点高亮（可选，克制）。
  - x 轴每 4 桶标整点小时（沿用现标注），hover 显时刻·请求数（title 或简单 tooltip）。
  - 顶部峰值/总请求标注保留。
- **空态不变**：hasTrend=false → 「今日暂无请求」。
- 成功/失败着色：原柱状按 error 比例叠 danger——曲线图可简化为单 total_requests 折线（首页克制概览）；若易做可叠一条 error 细线，否则单线即可，回报取舍。

## 约束 / 范围
- **只改 `src/pages/Home.tsx`**（趋势块渲染部分）。数据/取数/i18n key 复用不变（无新文案）。
- ⚠️ 并发会话在改 Stats.tsx/db.rs——**禁碰** api.ts/Stats.tsx/src-tauri。提交**路径限定** `git add src/pages/Home.tsx`，禁 git add -A。
- 复用单 --accent + 语义色，禁硬编码 hex/图表库/紫渐变。

## 验收
- `yarn build` 过；`git diff --name-only` 仅 Home.tsx。
- 行为：趋势区显**曲线图**（折线+面积），非柱状；有数据画曲线 + 峰值/总数标注；无数据空态；随 onProxyLogUpdated 刷新；风格一致无 slop。

## 失败处理
- 平滑曲线易出诡异控制点 → 退直折线 polyline（更稳）。
- 单点/全 0 → 空态或平直线兜底不崩。
- 门禁红修到绿；卡住标 `需要:`。
