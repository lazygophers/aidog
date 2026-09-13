# Spec：声明式图表引擎（九种图表）

> 来源：wayfinder 地图 [#22](https://github.com/lazygophers/aidog/issues/22)，决策全集见其 Decisions so far 与各票 resolution（#23–#30）。
> 本文档只汇编，决策真值在各票内；冲突时以票为准。
> 骨架原型已落地验证：commit `a127e43`（recharts 3.10.1 + ShareDonut 进统计页）。

## A. 架构与底座（#29 + #26）

### A1. 渲染底座

- **7/9 种**（折线/柱状/堆叠面积/饼/环形/堆叠柱/散点）：shadcn/ui Charts（Recharts v3，SVG，JSX 声明式）
- **2 种自研 DOM+CSS 小组件**：热力图×2（时刻热力、维度热力）+ 仪表盘（快照+趋势）——CSS grid + conic/radial gradient，零 canvas
- ECharts 才对的条件：热力/仪表盘交互变重（brush/dataZoom）时才重开选型

### A2. Recharts v3 升级事实（#26 实测）

- shadcn `new-york` registry 仍钉 recharts 2.15.4（registry JSON 实测）；v3 组件直接取官方 v4 仓库源码 `apps/v4/registry/new-york-v4/ui/chart.tsx`，改一行 `import { cn } from "cn"` → `"@/lib/utils"`。已落地 `src/components/ui/chart.tsx`
- v3 要点：chart token 引用写 `var(--chart-1)` 不带 `hsl()`；`ChartContainer` 必须带高度（`h-*`/`min-h-*`/`aspect-*`）；`initialDimension` 支持首帧测量兜底
- 官方升级文档：https://ui.shadcn.com/docs/components/base/chart 「Updating to Recharts v3」节

### A3. 系列色板（#26 定案）

- **主线琥珀**：首位/主系列 = `var(--primary)`（mono 萤火虫主题 dark #e8c547 / light #c49a3c）
- **辅线灰阶**：`var(--chart-2..5)`（现值零饱和 oklch，globals.css:105-109 与 810-814）
- **不动 globals.css 的 `--chart-N` 定义**——色板语义在 ChartConfig 层表达（主系列引用 `--primary`，辅系列引用 `--chart-2..5`），主题变量保持纯灰阶
- 热力图色带：琥珀 alpha 阶梯 `rgba(232,197,71, .06→.92)`（原型已验证）
- tooltip 定制范式：`ChartTooltipContent` 传自定义 `formatter` 渲染名称+格式化值（ShareDonut 已示范）

## B. 组件 API 与公共层（#24）

### B1. 九个声明式组件

`<LineChart data=…/>` `<BarChart/>` `<StackedAreaChart/>` `<PieChart/>` `<DonutChart/>` `<StackedBarChart/>` `<HeatmapChart/>` `<ScatterChart/>` `<GaugeChart/>`——props 以 `ChartConfig` + data 数组为主，允许组合 Recharts 子组件穿透。

### B2. 公共层职责

- **轴**：nice-ticks、时间轴标签（走 `utils/formatters.ts`）、双 Y 轴——进公共层，组件不各自实现
- **tooltip**：单一实现（最近点吸附 + 悬浮卡，RTL 安全 + 容器溢出防裁剪），基于 shadcn `ChartTooltip/ChartTooltipContent`
- **动画**：路径生长（stroke-dasharray draw-in）；玻璃卡 hover 琥珀描边流光（复用 `.glass:hover` conic flow-border 既有模式）
- **空态**：无数据诚实空态（复用 Home/Stats 现有文案模式），不画零值假图
- **迁移清单**：旧三处图表迁入引擎后删旧实现——`src/components/shared/CostTrendChart.tsx`、Home 内联 SVG 趋势、`PopoverCards.tsx` 内联曲线

## C. 落点与信息架构（#25 + #30）

### C1. Stats 页四 tab

时间序列 / 占比 / 密度 / 配额（现有单页筛选条保留，tab 只切主图区）。

### C2. Popover 浮窗

曲线 + 环形 + 迷你热力条（三件套，数据走 `statsApi.queryBatch` 既有批量通道）。

### C3. Home 页 = 命令面板版（#30 用户选定方向二，Raycast 参照）

- 单块命令面板：琥珀渐变眉条（`#64521d → #e8c547 → #f2dc8a`）+ 搜索栏式状态行（运行态点 + 端口 + ⌘C 复制地址）
- 四 KPI 紧凑行，每格**行内 sparkline**（花费=琥珀，其余灰阶）
- 24h 趋势**紧凑双线**区（琥珀主线 + 灰阶虚线辅线）——旧版放大三曲线主图不再保留（深分析归 Stats 时间序列 tab）
- 平台 Top4 列表行：迷你环形 + 行内占比条 + 等宽数字
- 总余额行 + 快捷键 footer（⌘N/⌘S/⌘L/⌘C，键位绑定属实现阶段）
- 视觉 token：分层中性面 s1 `#0e0e0e` / s2 `#151514`、行线 `rgba(255,255,255,.07)`、SF Mono 数字栈
- 初稿资产：`.scratch/home-redesign/direction-approved.md`（选定方向 HTML：`.scratch/home-redesign/design-demos/reference-raycast.html`）

### C4. 仪表盘（配额）

快照 + 趋势两态：DOM+CSS 自研，数据依赖 D3。

## D. 后端数据链（#28）

### D1. 交叉聚合（批次二前置）

- `StatsQuery` 加可选 `series_by: Option<String>`（值 = `platform`/`model`/`group`）
- `StatsResult` 附 `series: [{name, buckets[]}]`
- 实现：聚合表 SQL 加一维 GROUP BY，**零 schema 改动**（stats_agg_hourly 已是 (time_hour, model, group_key, platform_id) 四元组，aidog_db/src/lib.rs:59-76）；minute 路径（proxy_log 裸扫）桶+维度双键
- 维度基数沿用 `dimension_data` LIMIT（见 D3）

### D2. 散点直方图

服务端 `(duration_bin × cost_bin)` 二维直方图独立 command（非 stats_query 扩展），前端 ScatterChart 直接渲染 bin 矩阵。

### D3. dimension LIMIT 参数化

`StatsQuery` 可选 `limit`，缺省 50（现硬编码 query_stats.rs:357/413）。

### D4. 配额历史落库

- 事件式 `quota_snapshot` 表：真实余额查询成功时顺手插一行（平台、est_balance_remaining、时间戳），**无定时器**
- retention 对齐 90d（与 proxy_log retention_days 同策略）
- 供 C4 仪表盘趋势与 Stats 配额 tab

## E. 交付分批（#27 用户确认）

- **批次一（零后端）**：折线、柱状、饼、环形、时刻热力、快照仪表盘——现有 `statsApi.query` 直接可喂（矩阵结论：aidog_db/src/models/stats.rs:53-72 参数面足够）
- **批次二（交叉聚合）**：堆叠面积、堆叠柱、维度热力——依赖 D1
- **批次三（独立小票）**：散点直方图（D2）、配额趋势（D4）、LIMIT 参数化（D3）
- 批次二三后端小票可与批次一前端并行，非阻塞链

## F. 收尾两项（已拍板 2026-09-13，用户确认按建议）

### F1. 大点数性能（定案）

- 折线/面积/柱状：minute 粒度仅 ≤24h 窗（后端已限），最坏 1440 点；**前端 >500 点时降采样**（LTTB 或桶 max），阈值常量放公共层
- 散点：服务端 bin 化（D2）后点数 = bin 网格，客户端无上限问题
- 热力图：固定 24×7=168 格（时刻）/ 维度×时间受 D3 LIMIT 约束
- 仪表盘/环形/饼：小点数，无策略需要

### F2. 测试策略（定案）

- 公共层纯函数（nice-ticks、降采样、色板映射）→ vitest 单测，覆盖分支 95%+（对齐项目现有约定）
- 组件 → vitest + @testing-library/react role 查询（`CostTrendChart.test.tsx` 既有模式），不做像素快照
- D1–D4 后端 → Rust 集成测试（既有 aidog_test_util 模式），聚合正确性 SQL 断言
- i18n/RTL → `check:i18n` 门禁 + ar-SA 冒烟（手动）
