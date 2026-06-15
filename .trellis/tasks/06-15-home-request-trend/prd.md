# 首页加请求趋势图

## 需求
首页（Home.tsx，总览仪表盘）新增「请求趋势」——展示一段时间内请求量走势（轻量趋势图）。

## 数据源（现成，零后端改动）
- `statsApi.query({ start, end, granularity })`→`StatsResult{overview, buckets:StatsBucket[]}`（api.ts:1151，命令 stats_query）。
- `StatsBucket{ time_bucket, total_requests, success_count, error_count, total_cost, ... }`（api.ts:1121）。
- 取「今日 hourly」：start=今日 0 点、end=now、granularity="hourly" → 24 个桶（参考 Stats.tsx:90-91 的 today→hourly 联动）。

## 设计（融入现有首页，huashu 原则：从现有系统长出、无 slop）
- 在 Home.tsx 现有「今日概览」区之后/「速览」之前，加一块 glass-surface「请求趋势 · 今日」。
- **轻量 SVG 图**（不引图表库）：横轴 = 今日各小时桶，纵轴 = total_requests。形态择一（实现选清爽者）：面积图 / 柱状 / sparkline 折线。**单 accent 主色**（var(--accent)）；成功/失败可叠色（成功 accent、失败用 --color-error/danger 语义色，参考 usageColor/colorScale 体系，不硬编码 hex）。
- 顶部小字标注峰值/总请求（复用 formatters.formatNumber）。
- **空态**：今日无请求（buckets 全 0 或空）→ 诚实空态「今日暂无请求」，不画假曲线。
- 加载与 Home 其它区一致：并入现有 Promise.all 并行拉取（statsApi.query 加进去）+ 独立 catch 兜底（失败该块空态，不崩）。onProxyLogUpdated 重载时一并刷新。
- 克制：首页只做「今日请求走势一眼概览」，深度（多粒度/环比/维度）留 Stats 页，不重复堆。

## 约束
- 复用现有：glass-surface、formatters、usageColor/colorScale 语义色、单 --accent；禁紫渐变/emoji/假数据/硬编码 hex/引图表库。
- 时间计算参考 Stats.tsx 的 today 范围算法（本地 0 点→now），保持口径一致。
- 纯前端，**不碰后端**（statsApi 现成）。
- ⚠️ 另一会话在改 Stats.tsx/db.rs/api.ts —— 本任务**只改 Home.tsx + 必要 i18n locale**，不碰 api.ts/Stats.tsx/db.rs（statsApi 已有，无需改）。提交走**路径限定**（只 add Home.tsx + locale），禁 git add -A。

## 验收
- `yarn build` + `yarn check:i18n` 过；locale 无新增重复 key。
- `git diff --name-only` 不含 src-tauri、不含 api.ts/Stats.tsx（只 Home.tsx + locale）。
- 行为：首页显今日请求趋势图（hourly），有数据画走势 + 峰值/总数标注；无数据空态；失败该块兜底不崩；刷新随 onProxyLogUpdated 更新；风格与首页一致无 slop。

## 失败处理
- statsApi.query 失败 → 趋势块空态，不影响首页其它区。
- 今日 0 点时间算法边界 → 镜像 Stats.tsx today 算法。
- 新增 i18n 文案（请求趋势/今日/峰值/总请求/暂无请求）8 locale 全补 + Counter 查重。
- 门禁红修到绿；卡住标 `需要:`。
